//! Admin API 业务逻辑服务

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};

use crate::anthropic::PromptCacheRuntime;
use crate::common::log_ring::{LogEntry, LogFilter, LogKind, LogRing, ModelCallStats};
use crate::kiro::model::credentials::KiroCredentials;
use crate::kiro::token_manager::MultiTokenManager;
use crate::model::config::CompressionConfig;

use super::error::AdminServiceError;
use super::types::{
    AddCredentialRequest, AddCredentialResponse, BalanceResponse, CachedBalanceItem,
    CachedBalancesResponse, ConfigFieldError, ConfigRawResponse, ConfigSchemaEnumOption,
    ConfigSchemaField, ConfigSchemaGroup, ConfigSchemaResponse, ConfigUpdateResponse,
    ConfigValidateResponse, CredentialStatusItem, CredentialsStatusResponse, ImportAction,
    ImportItemResult, ImportSummary, ImportTokenJsonRequest, ImportTokenJsonResponse,
    LoadBalancingModeResponse, PromptCacheConfigResponse, SetLoadBalancingModeRequest,
    TokenJsonItem, UpdatePromptCacheConfigRequest,
};
use crate::model::config::Config;

/// 余额缓存过期时间（秒），5 分钟
const BALANCE_CACHE_TTL_SECS: i64 = 300;

/// 缓存的余额条目（含时间戳）
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedBalance {
    /// 缓存时间（Unix 秒）
    cached_at: f64,
    /// 缓存的余额数据
    data: BalanceResponse,
}

/// Admin 服务
///
/// 封装所有 Admin API 的业务逻辑
pub struct AdminService {
    token_manager: Arc<MultiTokenManager>,
    balance_cache: Mutex<HashMap<u64, CachedBalance>>,
    cache_path: Option<PathBuf>,
    /// 已注册的端点名称集合（用于 add_credential 校验）
    known_endpoints: HashSet<String>,
    /// 输入压缩与图片处理配置（阶段 5.2：与 anthropic AppState 共享的热加载源）
    compression_config: Arc<RwLock<CompressionConfig>>,
    /// Prompt Cache 运行时（阶段 5.2：与 anthropic AppState 共享）
    prompt_cache_runtime: Arc<RwLock<PromptCacheRuntime>>,
    /// 阶段 7：anthropic AppState 的 api_key 共享锁（支持热轮换）
    api_key_shared: Arc<RwLock<String>>,
    /// 阶段 7：AdminState 的 admin_api_key 共享锁（支持热轮换）
    admin_api_key_shared: Arc<RwLock<String>>,
    /// 阶段 7：anthropic AppState 的 extract_thinking 共享锁
    extract_thinking_shared: Arc<RwLock<bool>>,
    /// 阶段 7：config.json 文件路径（PUT /config 写盘用）
    config_path: PathBuf,
    /// 阶段 7：串行化 PUT /config 写流程，防并发覆盖
    config_write_lock: Mutex<()>,
    /// 阶段 7.9：日志环形缓冲（与 main.rs tracing layer 共用同一实例）
    log_ring: Arc<LogRing>,
}

impl AdminService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        token_manager: Arc<MultiTokenManager>,
        known_endpoints: impl IntoIterator<Item = String>,
        compression_config: Arc<RwLock<CompressionConfig>>,
        prompt_cache_runtime: Arc<RwLock<PromptCacheRuntime>>,
        api_key_shared: Arc<RwLock<String>>,
        admin_api_key_shared: Arc<RwLock<String>>,
        extract_thinking_shared: Arc<RwLock<bool>>,
        config_path: PathBuf,
        log_ring: Arc<LogRing>,
    ) -> Self {
        let cache_path = token_manager
            .cache_dir()
            .map(|d| d.join("kiro_balance_cache.json"));

        let balance_cache = Self::load_balance_cache_from(&cache_path);

        Self {
            token_manager,
            balance_cache: Mutex::new(balance_cache),
            cache_path,
            known_endpoints: known_endpoints.into_iter().collect(),
            compression_config,
            prompt_cache_runtime,
            api_key_shared,
            admin_api_key_shared,
            extract_thinking_shared,
            config_path,
            config_write_lock: Mutex::new(()),
            log_ring,
        }
    }

    // ============ 阶段 5.2: 全局配置热加载 ============

    /// 获取当前 CompressionConfig 快照
    pub fn get_compression_config(&self) -> CompressionConfig {
        self.compression_config.read().clone()
    }

    /// 全量替换 CompressionConfig（PUT 语义）
    ///
    /// 阶段 7：在内存更新同时持久化到 config.json，保持磁盘为单一真相源
    pub fn update_compression_config(&self, new_config: CompressionConfig) {
        *self.compression_config.write() = new_config.clone();
        if let Err(e) = self.persist_field(|cfg| cfg.compression = new_config) {
            tracing::warn!("CompressionConfig 持久化失败（内存已更新）: {}", e);
        } else {
            tracing::info!("CompressionConfig 已通过 admin API 热更新并持久化");
        }
    }

    /// 获取当前 Prompt Cache 配置
    pub fn get_prompt_cache_config(&self) -> PromptCacheConfigResponse {
        let rt = self.prompt_cache_runtime.read();
        PromptCacheConfigResponse {
            ttl_seconds: rt.ttl_seconds(),
            accounting_enabled: rt.accounting_enabled(),
        }
    }

    /// 全量替换 Prompt Cache 配置（TTL 变化会重建 cache_tracker）
    ///
    /// 阶段 7：内存更新后持久化到 config.json
    pub fn update_prompt_cache_config(&self, req: UpdatePromptCacheConfigRequest) {
        let UpdatePromptCacheConfigRequest {
            ttl_seconds,
            accounting_enabled,
        } = req;
        self.prompt_cache_runtime
            .write()
            .update(Some(ttl_seconds), Some(accounting_enabled));
        if let Err(e) = self.persist_field(|cfg| {
            cfg.prompt_cache_ttl_seconds = ttl_seconds;
            cfg.prompt_cache_accounting_enabled = accounting_enabled;
        }) {
            tracing::warn!("PromptCacheRuntime 持久化失败（内存已更新）: {}", e);
        } else {
            tracing::info!(
                ttl_seconds,
                accounting_enabled,
                "PromptCacheRuntime 已通过 admin API 热更新并持久化"
            );
        }
    }

    /// 加载磁盘配置 → 应用闭包修改 → 写回。串行化由 `config_write_lock` 保证。
    fn persist_field<F>(&self, mutate: F) -> anyhow::Result<()>
    where
        F: FnOnce(&mut Config),
    {
        let _guard = self.config_write_lock.lock();
        let mut config = Config::load(&self.config_path)?;
        config.set_config_path(self.config_path.clone());
        mutate(&mut config);
        config.save()?;
        Ok(())
    }

    // ========================================================================
    // 阶段 5.3a: 批量导入 token.json
    // ========================================================================

    /// 批量导入 token.json 数组
    ///
    /// 逐项验证、去重、加入凭据池。任一项失败不影响其他项继续处理。
    /// dry_run=true 时只做预览不实际写入。
    pub async fn import_token_json(
        &self,
        req: ImportTokenJsonRequest,
    ) -> ImportTokenJsonResponse {
        let items = req.items.into_vec();
        let dry_run = req.dry_run;

        let mut results = Vec::with_capacity(items.len());
        let mut added = 0usize;
        let mut skipped = 0usize;
        let mut invalid = 0usize;

        for (index, item) in items.into_iter().enumerate() {
            let result = self.process_token_json_item(index, item, dry_run).await;
            match result.action {
                ImportAction::Added => added += 1,
                ImportAction::Skipped => skipped += 1,
                ImportAction::Invalid => invalid += 1,
            }
            results.push(result);
        }

        tracing::info!(
            parsed = results.len(),
            added,
            skipped,
            invalid,
            dry_run,
            "批量 import_token_json 完成"
        );

        ImportTokenJsonResponse {
            summary: ImportSummary {
                parsed: results.len(),
                added,
                skipped,
                invalid,
            },
            items: results,
        }
    }

    async fn process_token_json_item(
        &self,
        index: usize,
        item: TokenJsonItem,
        dry_run: bool,
    ) -> ImportItemResult {
        let fingerprint = Self::generate_fingerprint(&item);

        // 验证必填字段
        let refresh_token = match &item.refresh_token {
            Some(rt) if !rt.is_empty() => rt.clone(),
            _ => {
                return ImportItemResult {
                    index,
                    fingerprint,
                    action: ImportAction::Invalid,
                    reason: Some("缺少 refreshToken".to_string()),
                    credential_id: None,
                };
            }
        };

        let auth_method = Self::map_auth_method(&item);

        // IdC 需要 clientId 和 clientSecret
        if auth_method == "idc" && (item.client_id.is_none() || item.client_secret.is_none()) {
            return ImportItemResult {
                index,
                fingerprint,
                action: ImportAction::Invalid,
                reason: Some(format!("{} 认证需要 clientId 和 clientSecret", auth_method)),
                credential_id: None,
            };
        }

        // 通过 refresh_token 前 32 字符前缀去重
        if self.token_manager.has_refresh_token_prefix(&refresh_token) {
            return ImportItemResult {
                index,
                fingerprint,
                action: ImportAction::Skipped,
                reason: Some("凭据已存在".to_string()),
                credential_id: None,
            };
        }

        if dry_run {
            return ImportItemResult {
                index,
                fingerprint,
                action: ImportAction::Added,
                reason: Some("预览模式".to_string()),
                credential_id: None,
            };
        }

        // trim region 字段（空字符串视为 None）
        let region = item
            .region
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let api_region = item
            .api_region
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let new_cred = KiroCredentials {
            refresh_token: Some(refresh_token),
            auth_method: Some(auth_method),
            client_id: item.client_id,
            client_secret: item.client_secret,
            priority: item.priority,
            region,
            api_region,
            machine_id: item.machine_id,
            ..KiroCredentials::default()
        };

        match self.token_manager.add_credential(new_cred).await {
            Ok(credential_id) => ImportItemResult {
                index,
                fingerprint,
                action: ImportAction::Added,
                reason: None,
                credential_id: Some(credential_id),
            },
            Err(e) => ImportItemResult {
                index,
                fingerprint,
                action: ImportAction::Invalid,
                reason: Some(e.to_string()),
                credential_id: None,
            },
        }
    }

    /// 生成凭据指纹（refresh_token 前 16 字符，可读且不泄漏完整 token）
    fn generate_fingerprint(item: &TokenJsonItem) -> String {
        item.refresh_token
            .as_deref()
            .map(|rt| {
                if rt.len() >= 16 {
                    let end = crate::common::utf8::floor_char_boundary(rt, 16);
                    format!("{}...", &rt[..end])
                } else {
                    rt.to_string()
                }
            })
            .unwrap_or_else(|| "(empty)".to_string())
    }

    /// 映射 provider/authMethod 字段到标准 authMethod
    fn map_auth_method(item: &TokenJsonItem) -> String {
        if let Some(auth) = &item.auth_method {
            let auth_lower = auth.to_lowercase();
            return match auth_lower.as_str() {
                "idc" | "builder-id" | "builderid" => "idc".to_string(),
                "social" => "social".to_string(),
                _ => auth_lower,
            };
        }

        if let Some(provider) = &item.provider {
            let provider_lower = provider.to_lowercase();
            return match provider_lower.as_str() {
                "builderid" | "builder-id" | "idc" => "idc".to_string(),
                "social" => "social".to_string(),
                _ => "social".to_string(),
            };
        }

        "social".to_string()
    }

    /// 获取所有凭据状态
    pub fn get_all_credentials(&self) -> CredentialsStatusResponse {
        let snapshot = self.token_manager.snapshot();
        let default_endpoint = self.token_manager.config().default_endpoint.clone();

        let mut credentials: Vec<CredentialStatusItem> = snapshot
            .entries
            .into_iter()
            .map(|entry| CredentialStatusItem {
                id: entry.id,
                priority: entry.priority,
                disabled: entry.disabled,
                failure_count: entry.failure_count,
                is_current: entry.id == snapshot.current_id,
                expires_at: entry.expires_at,
                auth_method: entry.auth_method,
                has_profile_arn: entry.has_profile_arn,
                refresh_token_hash: entry.refresh_token_hash,
                api_key_hash: entry.api_key_hash,
                masked_api_key: entry.masked_api_key,
                email: entry.email,
                success_count: entry.success_count,
                last_used_at: entry.last_used_at.clone(),
                has_proxy: entry.has_proxy,
                proxy_url: entry.proxy_url,
                refresh_failure_count: entry.refresh_failure_count,
                disabled_reason: entry.disabled_reason,
                effective_endpoint: entry
                    .endpoint
                    .clone()
                    .unwrap_or_else(|| default_endpoint.clone()),
                endpoint: entry.endpoint,
                region: entry.region,
                api_region: entry.api_region,
            })
            .collect();

        // 按优先级排序（数字越小优先级越高）
        credentials.sort_by_key(|c| c.priority);

        CredentialsStatusResponse {
            total: snapshot.total,
            available: snapshot.available,
            current_id: snapshot.current_id,
            credentials,
        }
    }

    /// 设置凭据禁用状态
    pub fn set_disabled(&self, id: u64, disabled: bool) -> Result<(), AdminServiceError> {
        // 先获取当前凭据 ID，用于判断是否需要切换
        let snapshot = self.token_manager.snapshot();
        let current_id = snapshot.current_id;

        self.token_manager
            .set_disabled(id, disabled)
            .map_err(|e| self.classify_error(e, id))?;

        // 只有禁用的是当前凭据时才尝试切换到下一个
        if disabled && id == current_id {
            let _ = self.token_manager.switch_to_next();
        }
        Ok(())
    }

    /// 设置凭据优先级
    pub fn set_priority(&self, id: u64, priority: u32) -> Result<(), AdminServiceError> {
        self.token_manager
            .set_priority(id, priority)
            .map_err(|e| self.classify_error(e, id))
    }

    /// 设置凭据 endpoint（None = 清除，回退默认）
    pub fn set_endpoint(
        &self,
        id: u64,
        endpoint: Option<String>,
    ) -> Result<(), AdminServiceError> {
        let endpoint = endpoint
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        if let Some(name) = endpoint.as_deref() {
            if !self.known_endpoints.contains(name) {
                let mut known: Vec<&str> =
                    self.known_endpoints.iter().map(|s| s.as_str()).collect();
                known.sort_unstable();
                return Err(AdminServiceError::InvalidCredential(format!(
                    "未知端点 \"{}\"，已注册: {:?}",
                    name, known
                )));
            }
        }

        self.token_manager
            .set_endpoint(id, endpoint)
            .map_err(|e| self.classify_error(e, id))
    }

    /// 设置凭据 Region 与 API Region（任意 None = 清除该字段）
    pub fn set_region(
        &self,
        id: u64,
        region: Option<String>,
        api_region: Option<String>,
    ) -> Result<(), AdminServiceError> {
        let region = region
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let api_region = api_region
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        self.token_manager
            .set_region(id, region, api_region)
            .map_err(|e| self.classify_error(e, id))
    }

    /// 重置失败计数并重新启用
    pub fn reset_and_enable(&self, id: u64) -> Result<(), AdminServiceError> {
        self.token_manager
            .reset_and_enable(id)
            .map_err(|e| self.classify_error(e, id))
    }

    /// 获取凭据余额（带缓存）
    pub async fn get_balance(&self, id: u64) -> Result<BalanceResponse, AdminServiceError> {
        // 先查缓存
        {
            let cache = self.balance_cache.lock();
            if let Some(cached) = cache.get(&id) {
                let now = Utc::now().timestamp() as f64;
                if (now - cached.cached_at) < BALANCE_CACHE_TTL_SECS as f64 {
                    tracing::debug!("凭据 #{} 余额命中缓存", id);
                    return Ok(cached.data.clone());
                }
            }
        }

        // 缓存未命中或已过期，从上游获取
        let balance = self.fetch_balance(id).await?;

        // 更新缓存
        {
            let mut cache = self.balance_cache.lock();
            cache.insert(
                id,
                CachedBalance {
                    cached_at: Utc::now().timestamp() as f64,
                    data: balance.clone(),
                },
            );
        }
        self.save_balance_cache();

        Ok(balance)
    }

    /// 从上游获取余额（无缓存）
    async fn fetch_balance(&self, id: u64) -> Result<BalanceResponse, AdminServiceError> {
        let usage = self
            .token_manager
            .get_usage_limits_for(id)
            .await
            .map_err(|e| self.classify_balance_error(e, id))?;

        let current_usage = usage.current_usage();
        let usage_limit = usage.usage_limit();
        let remaining = (usage_limit - current_usage).max(0.0);
        let usage_percentage = if usage_limit > 0.0 {
            (current_usage / usage_limit * 100.0).min(100.0)
        } else {
            0.0
        };

        Ok(BalanceResponse {
            id,
            subscription_title: usage.subscription_title().map(|s| s.to_string()),
            current_usage,
            usage_limit,
            remaining,
            usage_percentage,
            next_reset_at: usage.next_date_reset,
        })
    }

    /// 读取所有凭据的缓存余额快照（不触发任何上游请求）
    ///
    /// 阶段 5.3c：纯读 AdminService 已有的磁盘缓存（由 `get_balance` 写入），
    /// 不访问 token_manager 选号路径、不持锁号池关键字段，对号池零影响。
    pub fn get_cached_balances(&self) -> CachedBalancesResponse {
        let cache = self.balance_cache.lock();
        let ttl_secs = BALANCE_CACHE_TTL_SECS.max(0) as u64;
        let balances = cache
            .iter()
            .map(|(id, cached)| CachedBalanceItem {
                id: *id,
                remaining: cached.data.remaining,
                usage_limit: cached.data.usage_limit,
                usage_percentage: cached.data.usage_percentage,
                subscription_title: cached.data.subscription_title.clone(),
                cached_at: (cached.cached_at * 1000.0).max(0.0) as u64,
                ttl_secs,
            })
            .collect();
        CachedBalancesResponse { balances }
    }

    /// 添加新凭据
    pub async fn add_credential(
        &self,
        req: AddCredentialRequest,
    ) -> Result<AddCredentialResponse, AdminServiceError> {
        // 校验端点名：未指定则默认合法，指定则必须已注册
        if let Some(ref name) = req.endpoint {
            if !self.known_endpoints.contains(name) {
                let mut known: Vec<&str> =
                    self.known_endpoints.iter().map(|s| s.as_str()).collect();
                known.sort();
                return Err(AdminServiceError::InvalidCredential(format!(
                    "未知端点 \"{}\"，已注册端点: {:?}",
                    name, known
                )));
            }
        }

        // 构建凭据对象
        let email = req.email.clone();
        let new_cred = KiroCredentials {
            id: None,
            access_token: None,
            refresh_token: req.refresh_token,
            profile_arn: None,
            expires_at: None,
            auth_method: Some(req.auth_method),
            client_id: req.client_id,
            client_secret: req.client_secret,
            priority: req.priority,
            region: req.region,
            auth_region: req.auth_region,
            api_region: req.api_region,
            machine_id: req.machine_id,
            email: req.email,
            subscription_title: None, // 将在首次获取使用额度时自动更新
            proxy_url: req.proxy_url,
            proxy_username: req.proxy_username,
            proxy_password: req.proxy_password,
            disabled: false, // 新添加的凭据默认启用
            kiro_api_key: req.kiro_api_key,
            endpoint: req.endpoint,
        };

        // 调用 token_manager 添加凭据
        let credential_id = self
            .token_manager
            .add_credential(new_cred)
            .await
            .map_err(|e| self.classify_add_error(e))?;

        // 主动获取订阅等级，避免首次请求时 Free 账号绕过 Opus 模型过滤
        if let Err(e) = self.token_manager.get_usage_limits_for(credential_id).await {
            tracing::warn!("添加凭据后获取订阅等级失败（不影响凭据添加）: {}", e);
        }

        Ok(AddCredentialResponse {
            success: true,
            message: format!("凭据添加成功，ID: {}", credential_id),
            credential_id,
            email,
        })
    }

    /// 删除凭据
    pub fn delete_credential(&self, id: u64) -> Result<(), AdminServiceError> {
        self.token_manager
            .delete_credential(id)
            .map_err(|e| self.classify_delete_error(e, id))?;

        // 清理已删除凭据的余额缓存
        {
            let mut cache = self.balance_cache.lock();
            cache.remove(&id);
        }
        self.save_balance_cache();

        Ok(())
    }

    /// 获取负载均衡模式
    pub fn get_load_balancing_mode(&self) -> LoadBalancingModeResponse {
        LoadBalancingModeResponse {
            mode: self.token_manager.get_load_balancing_mode(),
        }
    }

    /// 设置负载均衡模式
    pub fn set_load_balancing_mode(
        &self,
        req: SetLoadBalancingModeRequest,
    ) -> Result<LoadBalancingModeResponse, AdminServiceError> {
        // 验证模式值
        if req.mode != "priority" && req.mode != "balanced" {
            return Err(AdminServiceError::InvalidCredential(
                "mode 必须是 'priority' 或 'balanced'".to_string(),
            ));
        }

        self.token_manager
            .set_load_balancing_mode(req.mode.clone())
            .map_err(|e| AdminServiceError::InternalError(e.to_string()))?;

        Ok(LoadBalancingModeResponse { mode: req.mode })
    }

    /// 强制刷新指定凭据的 Token
    pub async fn force_refresh_token(&self, id: u64) -> Result<(), AdminServiceError> {
        self.token_manager
            .force_refresh_token_for(id)
            .await
            .map_err(|e| self.classify_balance_error(e, id))
    }

    // ============ 余额缓存持久化 ============

    fn load_balance_cache_from(cache_path: &Option<PathBuf>) -> HashMap<u64, CachedBalance> {
        let path = match cache_path {
            Some(p) => p,
            None => return HashMap::new(),
        };

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return HashMap::new(),
        };

        // 文件中使用字符串 key 以兼容 JSON 格式
        let map: HashMap<String, CachedBalance> = match serde_json::from_str(&content) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("解析余额缓存失败，将忽略: {}", e);
                return HashMap::new();
            }
        };

        let now = Utc::now().timestamp() as f64;
        map.into_iter()
            .filter_map(|(k, v)| {
                let id = k.parse::<u64>().ok()?;
                // 丢弃超过 TTL 的条目
                if (now - v.cached_at) < BALANCE_CACHE_TTL_SECS as f64 {
                    Some((id, v))
                } else {
                    None
                }
            })
            .collect()
    }

    fn save_balance_cache(&self) {
        let path = match &self.cache_path {
            Some(p) => p,
            None => return,
        };

        // 持有锁期间完成序列化和写入，防止并发损坏
        let cache = self.balance_cache.lock();
        let map: HashMap<String, &CachedBalance> =
            cache.iter().map(|(k, v)| (k.to_string(), v)).collect();

        match serde_json::to_string_pretty(&map) {
            Ok(json) => {
                if let Err(e) = std::fs::write(path, json) {
                    tracing::warn!("保存余额缓存失败: {}", e);
                }
            }
            Err(e) => tracing::warn!("序列化余额缓存失败: {}", e),
        }
    }

    // ============ 错误分类 ============

    /// 分类简单操作错误（set_disabled, set_priority, reset_and_enable）
    fn classify_error(&self, e: anyhow::Error, id: u64) -> AdminServiceError {
        let msg = e.to_string();
        if msg.contains("不存在") {
            AdminServiceError::NotFound { id }
        } else {
            AdminServiceError::InternalError(msg)
        }
    }

    /// 分类余额查询错误（可能涉及上游 API 调用）
    fn classify_balance_error(&self, e: anyhow::Error, id: u64) -> AdminServiceError {
        let msg = e.to_string();

        // 1. 凭据不存在
        if msg.contains("不存在") {
            return AdminServiceError::NotFound { id };
        }

        // 2. API Key 凭据不支持刷新：客户端请求错误，映射为 400
        if msg.contains("API Key 凭据不支持刷新") {
            return AdminServiceError::InvalidCredential(msg);
        }

        // 3. 上游服务错误特征：HTTP 响应错误或网络错误
        let is_upstream_error =
            // HTTP 响应错误（来自 refresh_*_token 的错误消息）
            msg.contains("凭证已过期或无效") ||
            msg.contains("权限不足") ||
            msg.contains("已被限流") ||
            msg.contains("服务器错误") ||
            msg.contains("Token 刷新失败") ||
            msg.contains("暂时不可用") ||
            // 网络错误（reqwest 错误）
            msg.contains("error trying to connect") ||
            msg.contains("connection") ||
            msg.contains("timeout") ||
            msg.contains("timed out");

        if is_upstream_error {
            AdminServiceError::UpstreamError(msg)
        } else {
            // 4. 默认归类为内部错误（本地验证失败、配置错误等）
            // 包括：缺少 refreshToken、refreshToken 已被截断、无法生成 machineId 等
            AdminServiceError::InternalError(msg)
        }
    }

    /// 分类添加凭据错误
    fn classify_add_error(&self, e: anyhow::Error) -> AdminServiceError {
        let msg = e.to_string();

        // 凭据验证失败（refreshToken 无效、格式错误等）
        let is_invalid_credential = msg.contains("缺少 refreshToken")
            || msg.contains("refreshToken 为空")
            || msg.contains("refreshToken 已被截断")
            || msg.contains("凭据已存在")
            || msg.contains("refreshToken 重复")
            || msg.contains("kiroApiKey 重复")
            || msg.contains("缺少 kiroApiKey")
            || msg.contains("kiroApiKey 为空")
            || msg.contains("凭证已过期或无效")
            || msg.contains("权限不足")
            || msg.contains("已被限流");

        if is_invalid_credential {
            AdminServiceError::InvalidCredential(msg)
        } else if msg.contains("error trying to connect")
            || msg.contains("connection")
            || msg.contains("timeout")
        {
            AdminServiceError::UpstreamError(msg)
        } else {
            AdminServiceError::InternalError(msg)
        }
    }

    /// 分类删除凭据错误
    fn classify_delete_error(&self, e: anyhow::Error, id: u64) -> AdminServiceError {
        let msg = e.to_string();
        if msg.contains("不存在") {
            AdminServiceError::NotFound { id }
        } else if msg.contains("只能删除已禁用的凭据") || msg.contains("请先禁用凭据") {
            AdminServiceError::InvalidCredential(msg)
        } else {
            AdminServiceError::InternalError(msg)
        }
    }

    // ========================================================================
    // 阶段 7: 配置面板（Raw JSON / 可视化共用此后端 API）
    // ========================================================================

    /// `GET /config` — 返回当前 config.json 内容（结构化）
    ///
    /// 单一真相源：以磁盘文件为准。运行期间通过其他 admin 端点修改过的
    /// compression / prompt_cache / load_balancing 也会同步写盘，所以这里读到的
    /// 就是当前生效状态。
    pub fn get_config(&self) -> Result<Config, AdminServiceError> {
        Config::load(&self.config_path)
            .map_err(|e| AdminServiceError::InternalError(format!("读取配置失败: {}", e)))
    }

    /// `GET /config/schema` — 字段元数据，前端按此渲染可视化表单
    pub fn get_config_schema(&self) -> ConfigSchemaResponse {
        build_config_schema()
    }

    // ============ 阶段 7.9：日志查询 ============
    // 类型定义（用于本节方法签名）见本文件底部 LogsResponse 等。
    // 这里直接 use crate-internal scope。


    /// `GET /admin/logs` — 查询日志环形缓冲
    pub fn query_logs(&self, filter: LogFilter) -> LogsResponse {
        let entries = self.log_ring.query(&filter);
        let total = self.log_ring.len();
        // 默认窗口 5 分钟统计
        let stats = self.log_ring.model_call_stats(5 * 60_000);
        LogsResponse {
            entries,
            total_buffered: total,
            capacity: self.log_ring.capacity(),
            stats,
        }
    }

    /// `DELETE /admin/logs` — 清空缓冲
    pub fn clear_logs(&self) {
        self.log_ring.clear();
    }

    /// 拿一份 Arc<LogRing> 给 provider 用于追加 ModelCall 记录
    #[allow(dead_code)]
    pub fn log_ring(&self) -> Arc<LogRing> {
        self.log_ring.clone()
    }

    /// `GET /config/raw` — 返回原始 JSON 文本
    pub fn get_config_raw(&self) -> Result<ConfigRawResponse, AdminServiceError> {
        let content = std::fs::read_to_string(&self.config_path).map_err(|e| {
            AdminServiceError::InternalError(format!("读取配置文件失败: {}", e))
        })?;
        Ok(ConfigRawResponse {
            content,
            path: self.config_path.display().to_string(),
        })
    }

    /// `POST /config/validate` — 仅校验，不写盘
    pub fn validate_config(&self, new_config: Config) -> ConfigValidateResponse {
        let current = match self.get_config() {
            Ok(c) => c,
            Err(_) => {
                return ConfigValidateResponse {
                    valid: false,
                    errors: vec![ConfigFieldError {
                        path: "".into(),
                        message: "无法读取当前配置进行对比".into(),
                    }],
                    needs_restart: Vec::new(),
                    hot_reload: Vec::new(),
                };
            }
        };

        let errors = validate_config_invariants(&new_config);
        let (needs_restart, hot_reload) = diff_config_fields(&current, &new_config);

        ConfigValidateResponse {
            valid: errors.is_empty(),
            errors,
            needs_restart,
            hot_reload,
        }
    }

    /// `PUT /config` — 全量替换：校验 → 写盘 → 投射 A 类（热生效）字段
    pub fn update_config(
        &self,
        new_config: Config,
    ) -> Result<ConfigUpdateResponse, AdminServiceError> {
        let _guard = self.config_write_lock.lock();

        // 校验
        let errors = validate_config_invariants(&new_config);
        if !errors.is_empty() {
            let summary = errors
                .iter()
                .map(|e| format!("{}: {}", e.path, e.message))
                .collect::<Vec<_>>()
                .join("; ");
            return Err(AdminServiceError::InvalidCredential(format!(
                "配置校验失败: {}",
                summary
            )));
        }

        let old_config = self.get_config()?;

        // 写盘（带上 config_path 以确保 save 能找到目标）
        let mut to_save = new_config.clone();
        to_save.set_config_path(self.config_path.clone());
        to_save
            .save()
            .map_err(|e| AdminServiceError::InternalError(format!("写入配置文件失败: {}", e)))?;

        // 投射 A 类（热生效）字段到运行时状态
        let (needs_restart, hot_reload) = diff_config_fields(&old_config, &new_config);

        if new_config.compression != old_config.compression {
            *self.compression_config.write() = new_config.compression.clone();
        }
        if new_config.prompt_cache_ttl_seconds != old_config.prompt_cache_ttl_seconds
            || new_config.prompt_cache_accounting_enabled
                != old_config.prompt_cache_accounting_enabled
        {
            self.prompt_cache_runtime.write().update(
                Some(new_config.prompt_cache_ttl_seconds),
                Some(new_config.prompt_cache_accounting_enabled),
            );
        }
        if new_config.load_balancing_mode != old_config.load_balancing_mode {
            // token_manager 内部会再次写盘（idempotent，因为我们刚写过相同值）
            if let Err(e) = self
                .token_manager
                .set_load_balancing_mode(new_config.load_balancing_mode.clone())
            {
                tracing::warn!("投射 load_balancing_mode 到 token_manager 失败: {}", e);
            }
        }
        if new_config.extract_thinking != old_config.extract_thinking {
            *self.extract_thinking_shared.write() = new_config.extract_thinking;
        }
        if new_config.credential_rpm != old_config.credential_rpm
            || new_config.daily_max_requests != old_config.daily_max_requests
        {
            self.token_manager.update_rate_limit_config(
                new_config.credential_rpm,
                new_config.daily_max_requests,
            );
        }
        if new_config.log_buffer_capacity != old_config.log_buffer_capacity {
            let cap = new_config
                .log_buffer_capacity
                .unwrap_or(crate::common::log_ring::DEFAULT_LOG_CAPACITY);
            self.log_ring.resize(cap);
            tracing::info!(capacity = cap, "日志缓冲容量已热更新");
        }

        // 鉴权类热轮换（即使在 needs_restart 集合外，也立刻生效）
        let mut new_admin_api_key = None;
        let mut new_api_key = None;
        if let Some(new_key) = &new_config.api_key {
            if old_config.api_key.as_deref() != Some(new_key.as_str()) {
                *self.api_key_shared.write() = new_key.clone();
                new_api_key = Some(new_key.clone());
            }
        }
        if let Some(new_admin) = &new_config.admin_api_key {
            if old_config.admin_api_key.as_deref() != Some(new_admin.as_str())
                && !new_admin.trim().is_empty()
            {
                *self.admin_api_key_shared.write() = new_admin.clone();
                new_admin_api_key = Some(new_admin.clone());
            }
        }

        tracing::info!(
            needs_restart = ?needs_restart,
            hot_reload = ?hot_reload,
            "config.json 已通过 admin API 更新"
        );

        Ok(ConfigUpdateResponse {
            ok: true,
            message: "配置已保存".to_string(),
            needs_restart,
            hot_reload,
            new_admin_api_key,
            new_api_key,
        })
    }
}

/// 字段不变量校验（结构校验由 serde 完成，这里只检查跨字段约束）
fn validate_config_invariants(c: &Config) -> Vec<ConfigFieldError> {
    let mut errors = Vec::new();
    if c.host.trim().is_empty() {
        errors.push(ConfigFieldError {
            path: "host".into(),
            message: "host 不能为空".into(),
        });
    }
    if c.port == 0 {
        errors.push(ConfigFieldError {
            path: "port".into(),
            message: "port 必须为 1-65535".into(),
        });
    }
    if c.region.trim().is_empty() {
        errors.push(ConfigFieldError {
            path: "region".into(),
            message: "region 不能为空".into(),
        });
    }
    if c.api_key.as_deref().map(str::trim).unwrap_or("").is_empty() {
        errors.push(ConfigFieldError {
            path: "apiKey".into(),
            message: "apiKey 不能为空（否则客户端无法认证）".into(),
        });
    }
    if c.load_balancing_mode != "priority" && c.load_balancing_mode != "balanced" {
        errors.push(ConfigFieldError {
            path: "loadBalancingMode".into(),
            message: "必须是 'priority' 或 'balanced'".into(),
        });
    }
    if let Some(admin) = &c.admin_api_key {
        if !admin.is_empty() && admin.trim().is_empty() {
            errors.push(ConfigFieldError {
                path: "adminApiKey".into(),
                message: "adminApiKey 不能仅为空白字符".into(),
            });
        }
    }
    errors
}

/// 对比新旧 Config，返回 (needs_restart, hot_reload) 字段名集合
fn diff_config_fields(old: &Config, new: &Config) -> (Vec<String>, Vec<String>) {
    let mut needs_restart = Vec::new();
    let mut hot_reload = Vec::new();

    macro_rules! diff {
        ($field:ident, $name:expr, hot) => {
            if old.$field != new.$field {
                hot_reload.push($name.to_string());
            }
        };
        ($field:ident, $name:expr, restart) => {
            if old.$field != new.$field {
                needs_restart.push($name.to_string());
            }
        };
    }

    // 热生效字段（A 类）
    diff!(compression, "compression", hot);
    diff!(prompt_cache_ttl_seconds, "promptCacheTtlSeconds", hot);
    diff!(prompt_cache_accounting_enabled, "promptCacheAccountingEnabled", hot);
    diff!(load_balancing_mode, "loadBalancingMode", hot);
    diff!(extract_thinking, "extractThinking", hot);
    diff!(credential_rpm, "credentialRpm", hot);
    diff!(daily_max_requests, "dailyMaxRequests", hot);
    diff!(log_buffer_capacity, "logBufferCapacity", hot);
    // 鉴权类：通过 RwLock 热轮换
    diff!(api_key, "apiKey", hot);
    diff!(admin_api_key, "adminApiKey", hot);

    // 需重启字段（B 类）
    diff!(host, "host", restart);
    diff!(port, "port", restart);
    diff!(region, "region", restart);
    diff!(auth_region, "authRegion", restart);
    diff!(api_region, "apiRegion", restart);
    diff!(kiro_version, "kiroVersion", restart);
    diff!(machine_id, "machineId", restart);
    diff!(system_version, "systemVersion", restart);
    diff!(node_version, "nodeVersion", restart);
    diff!(tls_backend, "tlsBackend", restart);
    diff!(count_tokens_api_url, "countTokensApiUrl", restart);
    diff!(count_tokens_api_key, "countTokensApiKey", restart);
    diff!(count_tokens_auth_type, "countTokensAuthType", restart);
    diff!(proxy_url, "proxyUrl", restart);
    diff!(proxy_username, "proxyUsername", restart);
    diff!(proxy_password, "proxyPassword", restart);
    diff!(default_endpoint, "defaultEndpoint", restart);
    diff!(endpoints, "endpoints", restart);

    (needs_restart, hot_reload)
}

// ============================================================================
// 配置 Schema 构建（手写元数据，前端按此渲染可视化表单）
// ============================================================================

fn s(s: &str) -> String {
    s.to_string()
}

fn field(
    key: &str,
    label: &str,
    field_type: &str,
    needs_restart: bool,
    description: &str,
) -> ConfigSchemaField {
    ConfigSchemaField {
        key: s(key),
        label: s(label),
        field_type: s(field_type),
        needs_restart,
        sensitive: false,
        nullable: false,
        description: Some(s(description)),
        warning: None,
        default_value: None,
        min: None,
        max: None,
        enum_options: Vec::new(),
        placeholder: None,
    }
}

fn build_config_schema() -> ConfigSchemaResponse {
    use serde_json::json;

    let groups = vec![
        // ===== 服务监听 =====
        ConfigSchemaGroup {
            id: s("server"),
            label: s("服务监听"),
            description: Some(s("HTTP 服务的监听地址与端口")),
            needs_restart: true,
            sensitive: false,
            fields: vec![
                ConfigSchemaField {
                    placeholder: Some(s("127.0.0.1")),
                    default_value: Some(json!("127.0.0.1")),
                    warning: Some(s("改成 0.0.0.0 会暴露到公网，请确保配合 apiKey 防护")),
                    ..field("host", "监听地址", "string", true, "服务绑定的 IP")
                },
                ConfigSchemaField {
                    min: Some(1.0),
                    max: Some(65535.0),
                    default_value: Some(json!(8990)),
                    ..field("port", "监听端口", "number", true, "1-65535")
                },
            ],
        },
        // ===== 鉴权 =====
        ConfigSchemaGroup {
            id: s("auth"),
            label: s("鉴权（敏感）"),
            description: Some(s("API Key 与 Admin API Key，改 adminApiKey 当场热轮换")),
            needs_restart: false,
            sensitive: true,
            fields: vec![
                ConfigSchemaField {
                    sensitive: true,
                    needs_restart: false,
                    placeholder: Some(s("sk-kiro-rs-...")),
                    ..field("apiKey", "API Key", "string", false, "客户端调 /v1/messages 时认证用")
                },
                ConfigSchemaField {
                    sensitive: true,
                    nullable: true,
                    needs_restart: false,
                    placeholder: Some(s("sk-admin-...")),
                    warning: Some(s("修改后当前面板会自动用新值重连；其他使用旧值的工具立即失效")),
                    ..field("adminApiKey", "Admin API Key", "string", false, "Admin 端点与 Web 面板认证用")
                },
            ],
        },
        // ===== 区域 =====
        ConfigSchemaGroup {
            id: s("region"),
            label: s("AWS 区域"),
            description: Some(s("Token 刷新与 API 请求的 AWS Region")),
            needs_restart: true,
            sensitive: false,
            fields: vec![
                ConfigSchemaField {
                    default_value: Some(json!("us-east-1")),
                    placeholder: Some(s("us-east-1")),
                    ..field("region", "默认 Region", "string", true, "Auth/API 都未单独配时回退至此")
                },
                ConfigSchemaField {
                    nullable: true,
                    placeholder: Some(s("不配则用 region")),
                    ..field("authRegion", "Auth Region", "string", true, "Token 刷新使用")
                },
                ConfigSchemaField {
                    nullable: true,
                    placeholder: Some(s("不配则用 region")),
                    ..field("apiRegion", "API Region", "string", true, "API 请求使用")
                },
            ],
        },
        // ===== Kiro 元数据 =====
        ConfigSchemaGroup {
            id: s("kiroMeta"),
            label: s("Kiro 请求元数据"),
            description: Some(s("伪装客户端版本号、机器码等，写入上游请求头")),
            needs_restart: true,
            sensitive: false,
            fields: vec![
                ConfigSchemaField {
                    default_value: Some(json!("0.9.2")),
                    ..field("kiroVersion", "Kiro 版本", "string", true, "客户端版本号")
                },
                ConfigSchemaField {
                    nullable: true,
                    placeholder: Some(s("64 位十六进制，不填自动生成")),
                    ..field("machineId", "Machine ID", "string", true, "客户端机器码")
                },
                ConfigSchemaField {
                    ..field("systemVersion", "系统版本", "string", true, "如 darwin#24.6.0")
                },
                ConfigSchemaField {
                    default_value: Some(json!("22.21.1")),
                    ..field("nodeVersion", "Node 版本", "string", true, "Node.js 版本标识")
                },
            ],
        },
        // ===== TLS & 代理 =====
        ConfigSchemaGroup {
            id: s("network"),
            label: s("TLS & 代理"),
            description: Some(s("HTTP 客户端配置，凭据级代理在凭据列表里单独设")),
            needs_restart: true,
            sensitive: false,
            fields: vec![
                ConfigSchemaField {
                    enum_options: vec![
                        ConfigSchemaEnumOption { value: s("rustls"), label: s("rustls") },
                        ConfigSchemaEnumOption { value: s("native-tls"), label: s("native-tls") },
                    ],
                    default_value: Some(json!("rustls")),
                    ..field("tlsBackend", "TLS 后端", "enum", true, "rustls 兼容性好，native-tls 走系统证书库")
                },
                ConfigSchemaField {
                    nullable: true,
                    placeholder: Some(s("http://127.0.0.1:7890")),
                    ..field("proxyUrl", "代理 URL", "string", true, "HTTP / SOCKS5，留空禁用")
                },
                ConfigSchemaField {
                    nullable: true,
                    ..field("proxyUsername", "代理用户名", "string", true, "可选")
                },
                ConfigSchemaField {
                    sensitive: true,
                    nullable: true,
                    ..field("proxyPassword", "代理密码", "string", true, "可选")
                },
            ],
        },
        // ===== Count Tokens =====
        ConfigSchemaGroup {
            id: s("countTokens"),
            label: s("外部 count_tokens API"),
            description: Some(s("用外部 API 替代本地 token 计数（可选）")),
            needs_restart: true,
            sensitive: false,
            fields: vec![
                ConfigSchemaField {
                    nullable: true,
                    placeholder: Some(s("https://api.example.com/v1/messages/count_tokens")),
                    ..field("countTokensApiUrl", "URL", "string", true, "外部 count_tokens 端点")
                },
                ConfigSchemaField {
                    sensitive: true,
                    nullable: true,
                    ..field("countTokensApiKey", "API Key", "string", true, "外部 API 鉴权 key")
                },
                ConfigSchemaField {
                    enum_options: vec![
                        ConfigSchemaEnumOption { value: s("x-api-key"), label: s("x-api-key Header") },
                        ConfigSchemaEnumOption { value: s("bearer"), label: s("Bearer Token") },
                    ],
                    default_value: Some(json!("x-api-key")),
                    ..field("countTokensAuthType", "认证方式", "enum", true, "")
                },
            ],
        },
        // ===== 凭据栈策略 =====
        ConfigSchemaGroup {
            id: s("credPool"),
            label: s("凭据栈策略"),
            description: Some(s("号池选号 / Thinking / 默认端点")),
            needs_restart: false,
            sensitive: false,
            fields: vec![
                ConfigSchemaField {
                    enum_options: vec![
                        ConfigSchemaEnumOption { value: s("priority"), label: s("优先级（按 priority 排序）") },
                        ConfigSchemaEnumOption { value: s("balanced"), label: s("均衡负载（按余额）") },
                    ],
                    default_value: Some(json!("priority")),
                    ..field("loadBalancingMode", "负载均衡", "enum", false, "热生效")
                },
                ConfigSchemaField {
                    nullable: true,
                    min: Some(0.0),
                    max: Some(1200.0),
                    placeholder: Some(s("留空 / 0 = 自适应 1-2 秒")),
                    ..field(
                        "credentialRpm",
                        "凭据 RPM",
                        "number",
                        false,
                        "单凭据每分钟请求数上限（每秒 ≤ 20）。留空/0 用默认自适应（1-2s 随机间隔）；>0 固定间隔 = 60000/rpm 毫秒。突破上限可走 Raw JSON。其他保护（每日上限 / 指数退避 / suspend 检测）独立工作",
                    )
                },
                ConfigSchemaField {
                    nullable: true,
                    min: Some(0.0),
                    placeholder: Some(s("留空 = 500（保守安全网）")),
                    ..field(
                        "dailyMaxRequests",
                        "单凭据每日上限",
                        "number",
                        false,
                        "24h 滚动窗口内单凭据最多成功请求次数。模拟人类使用强度，避免被风控。留空/0 用默认 500；大号池或高频使用可调高（如 1000-2000）",
                    )
                },
                ConfigSchemaField {
                    nullable: true,
                    min: Some(1000.0),
                    max: Some(500_000.0),
                    placeholder: Some(s("留空 = 50000 (~25 MB 内存)")),
                    ..field(
                        "logBufferCapacity",
                        "日志缓冲容量",
                        "number",
                        false,
                        "Admin 日志面板的内存环形缓冲条数。每条约 500 字节。50000 ≈ 25 MB；200000 ≈ 100 MB。调小立刻挤出最旧条目；调大不会回填历史",
                    )
                },
                ConfigSchemaField {
                    default_value: Some(json!(true)),
                    needs_restart: false,
                    ..field("extractThinking", "提取 thinking 块", "boolean", false, "非流式响应中解析 <thinking>")
                },
                ConfigSchemaField {
                    enum_options: vec![
                        ConfigSchemaEnumOption { value: s("ide"), label: s("IDE") },
                        ConfigSchemaEnumOption { value: s("cli"), label: s("CLI") },
                    ],
                    default_value: Some(json!("ide")),
                    ..field("defaultEndpoint", "默认端点", "enum", true, "凭据未指定 endpoint 时回退")
                },
            ],
        },
        // ===== 输入压缩 =====
        ConfigSchemaGroup {
            id: s("compression"),
            label: s("输入压缩 / 图片处理"),
            description: Some(s("阶段 3.2 引入的四层压缩管道，全部热生效")),
            needs_restart: false,
            sensitive: false,
            fields: vec![
                ConfigSchemaField {
                    default_value: Some(json!(true)),
                    ..field("compression.enabled", "总开关", "boolean", false, "关闭后所有压缩与图片处理跳过")
                },
                ConfigSchemaField {
                    default_value: Some(json!(true)),
                    ..field("compression.whitespaceCompression", "压缩多余空白", "boolean", false, "")
                },
                ConfigSchemaField {
                    enum_options: vec![
                        ConfigSchemaEnumOption { value: s("keep"), label: s("保留") },
                        ConfigSchemaEnumOption { value: s("strip"), label: s("剥离") },
                        ConfigSchemaEnumOption { value: s("summarize"), label: s("摘要") },
                    ],
                    default_value: Some(json!("keep")),
                    ..field("compression.thinkingStrategy", "Thinking 策略", "enum", false, "如何处理历史中的 thinking 块")
                },
                ConfigSchemaField {
                    min: Some(0.0),
                    default_value: Some(json!(8000)),
                    ..field("compression.toolResultMaxChars", "工具结果最大字符", "number", false, "超长则首尾截断")
                },
                ConfigSchemaField {
                    min: Some(0.0),
                    default_value: Some(json!(80)),
                    ..field("compression.toolResultHeadLines", "保留头部行数", "number", false, "")
                },
                ConfigSchemaField {
                    min: Some(0.0),
                    default_value: Some(json!(40)),
                    ..field("compression.toolResultTailLines", "保留尾部行数", "number", false, "")
                },
                ConfigSchemaField {
                    min: Some(0.0),
                    default_value: Some(json!(6000)),
                    ..field("compression.toolUseInputMaxChars", "工具调用输入最大字符", "number", false, "")
                },
                ConfigSchemaField {
                    min: Some(0.0),
                    default_value: Some(json!(4000)),
                    ..field("compression.toolDescriptionMaxChars", "工具描述最大字符", "number", false, "")
                },
                ConfigSchemaField {
                    min: Some(0.0),
                    default_value: Some(json!(80)),
                    ..field("compression.maxHistoryTurns", "历史轮次上限", "number", false, "超出则丢弃最早")
                },
                ConfigSchemaField {
                    min: Some(0.0),
                    default_value: Some(json!(400_000)),
                    ..field("compression.maxHistoryChars", "历史字符上限", "number", false, "")
                },
                ConfigSchemaField {
                    min: Some(0.0),
                    default_value: Some(json!(4000)),
                    ..field("compression.imageMaxLongEdge", "图片最长边", "number", false, "缩放阈值")
                },
                ConfigSchemaField {
                    min: Some(0.0),
                    default_value: Some(json!(4_000_000)),
                    ..field("compression.imageMaxPixelsSingle", "单图像素上限", "number", false, "")
                },
                ConfigSchemaField {
                    min: Some(0.0),
                    default_value: Some(json!(4_000_000)),
                    ..field("compression.imageMaxPixelsMulti", "多图像素上限", "number", false, "")
                },
                ConfigSchemaField {
                    min: Some(0.0),
                    default_value: Some(json!(20)),
                    ..field("compression.imageMultiThreshold", "多图触发阈值", "number", false, "图片数量超过则用多图限制")
                },
                ConfigSchemaField {
                    min: Some(0.0),
                    default_value: Some(json!(0)),
                    ..field("compression.maxRequestBodyBytes", "请求体上限（字节）", "number", false, "0 = 无限制")
                },
            ],
        },
        // ===== Prompt Cache =====
        ConfigSchemaGroup {
            id: s("promptCache"),
            label: s("Prompt Cache"),
            description: Some(s("提示词缓存控制（Anthropic ephemeral cache）")),
            needs_restart: false,
            sensitive: false,
            fields: vec![
                ConfigSchemaField {
                    enum_options: vec![
                        ConfigSchemaEnumOption { value: s("300"), label: s("5 分钟（300s）") },
                        ConfigSchemaEnumOption { value: s("3600"), label: s("1 小时（3600s）") },
                    ],
                    default_value: Some(json!(300)),
                    ..field("promptCacheTtlSeconds", "TTL", "enum", false, "Anthropic ephemeral 支持 5m / 1h；改动重建 tracker")
                },
                ConfigSchemaField {
                    default_value: Some(json!(true)),
                    ..field("promptCacheAccountingEnabled", "启用计数", "boolean", false, "记录缓存命中率")
                },
            ],
        },
    ];

    ConfigSchemaResponse { groups }
}

// ============================================================================
// 阶段 7.9：日志查询 DTO
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsResponse {
    pub entries: Vec<LogEntry>,
    /// 当前 buffer 内总条数（未过滤前）
    pub total_buffered: usize,
    /// buffer 容量
    pub capacity: usize,
    /// 最近 5 分钟 ModelCall 统计
    pub stats: ModelCallStats,
}

/// `GET /admin/logs` 查询参数（query string）
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsQueryParams {
    /// generic / model_call / all
    pub kind: Option<String>,
    /// 等级过滤，逗号分隔（如 "WARN,ERROR"）
    pub levels: Option<String>,
    pub q: Option<String>,
    pub credential_id: Option<u64>,
    pub model: Option<String>,
    pub status: Option<u16>,
    pub only_failed: Option<bool>,
    pub since: Option<i64>,
    pub limit: Option<usize>,
}

impl LogsQueryParams {
    pub fn into_filter(self) -> LogFilter {
        let kind = self.kind.and_then(|k| match k.as_str() {
            "model_call" | "modelCall" => Some(LogKind::ModelCall),
            "generic" => Some(LogKind::Generic),
            _ => None,
        });
        let levels = self.levels.map(|s| {
            s.split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect()
        });
        LogFilter {
            kind,
            levels,
            q: self.q,
            credential_id: self.credential_id,
            model: self.model,
            status: self.status,
            only_failed: self.only_failed.unwrap_or(false),
            since: self.since,
            limit: self.limit.unwrap_or(200).min(2000),
        }
    }
}
