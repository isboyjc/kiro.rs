//! Kiro API Provider
//!
//! 核心组件，负责与 Kiro API 通信
//! 支持流式和非流式请求
//! 支持多凭据故障转移和重试
//! 支持按凭据级 endpoint 切换不同 Kiro API 端点

use reqwest::Client;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

use crate::http_client::{ProxyConfig, build_client};
use crate::kiro::cooldown::CooldownReason;
use crate::kiro::endpoint::{KiroEndpoint, RequestContext};
use crate::kiro::machine_id;
use crate::kiro::model::credentials::KiroCredentials;
use crate::kiro::token_manager::MultiTokenManager;
use crate::model::config::TlsBackend;
use parking_lot::Mutex;

/// 每个凭据的最大重试次数
const MAX_RETRIES_PER_CREDENTIAL: usize = 3;

/// 总重试次数硬上限（避免无限重试）
const MAX_TOTAL_RETRIES: usize = 9;

/// Kiro API Provider
///
/// 核心组件，负责与 Kiro API 通信
/// 支持多凭据故障转移和重试机制
/// 按凭据 `endpoint` 字段选择 [`KiroEndpoint`] 实现
pub struct KiroProvider {
    token_manager: Arc<MultiTokenManager>,
    /// 全局代理配置（用于凭据无自定义代理时的回退）
    global_proxy: Option<ProxyConfig>,
    /// Client 缓存：key = effective proxy config, value = reqwest::Client
    /// 不同代理配置的凭据使用不同的 Client，共享相同代理的凭据复用 Client
    client_cache: Mutex<HashMap<Option<ProxyConfig>, Client>>,
    /// TLS 后端配置
    tls_backend: TlsBackend,
    /// 端点实现注册表（key: endpoint 名称）
    endpoints: HashMap<String, Arc<dyn KiroEndpoint>>,
    /// 默认端点名称（凭据未指定 endpoint 时使用）
    default_endpoint: String,
    /// 阶段 7.9：日志环形缓冲，每次 call 完成后追加一条 ModelCall 记录（可选注入）
    log_ring: Option<Arc<crate::common::log_ring::LogRing>>,
}

/// 阶段 7.14 / 7.15：API 调用结果——回传上游响应 + 实际凭据 ID + 调用元数据，
/// 供 handler 做 prompt cache 记账（按凭据隔离）和成功调用的 ModelCall 日志（带 token）。
pub struct ApiCallResult {
    pub response: reqwest::Response,
    pub credential_id: u64,
    /// 上游响应耗时（ms，到响应头）
    pub duration_ms: u32,
    /// 实际使用的端点名（ide / cli）
    pub endpoint_name: String,
    /// 请求模型（用于日志）
    pub model: Option<String>,
    /// 第几次重试命中（0 = 首次）
    pub retry_attempt: u32,
}

impl KiroProvider {
    /// 创建带代理配置和端点注册表的 KiroProvider 实例
    ///
    /// # Arguments
    /// * `token_manager` - 多凭据 Token 管理器
    /// * `proxy` - 全局代理配置
    /// * `endpoints` - 端点名 → 实现的注册表（至少包含 `default_endpoint` 对应条目）
    /// * `default_endpoint` - 凭据未显式指定 endpoint 时使用的名称
    pub fn with_proxy(
        token_manager: Arc<MultiTokenManager>,
        proxy: Option<ProxyConfig>,
        endpoints: HashMap<String, Arc<dyn KiroEndpoint>>,
        default_endpoint: String,
    ) -> Self {
        assert!(
            endpoints.contains_key(&default_endpoint),
            "默认端点 {} 未在 endpoints 注册表中",
            default_endpoint
        );
        let tls_backend = token_manager.config().tls_backend;
        // 预热：构建全局代理对应的 Client
        let initial_client = build_client(proxy.as_ref(), 720, tls_backend)
            .expect("创建 HTTP 客户端失败");
        let mut cache = HashMap::new();
        cache.insert(proxy.clone(), initial_client);

        Self {
            token_manager,
            global_proxy: proxy,
            client_cache: Mutex::new(cache),
            tls_backend,
            endpoints,
            default_endpoint,
            log_ring: None,
        }
    }

    /// 阶段 7.9：注入日志环形缓冲（必须在 admin 模块创建后调用）
    pub fn with_log_ring(mut self, ring: Arc<crate::common::log_ring::LogRing>) -> Self {
        self.log_ring = Some(ring);
        self
    }

    /// 阶段 7.9 / 7.15：追加一条 ModelCall 记录到日志环形缓冲（若已注入）
    ///
    /// provider 只记录**失败/网络错**（无 token 数据）；成功调用的记录移到
    /// handler 层（拿到 input/output/cache token 后再记，见 handlers.rs）。
    #[allow(clippy::too_many_arguments)]
    fn record_model_call(
        &self,
        credential_id: u64,
        model: Option<String>,
        endpoint_name: &str,
        api_type: &str,
        status: u16,
        duration_ms: u32,
        retry_attempt: u32,
        is_stream: bool,
        error_summary: Option<String>,
    ) {
        let ring = match &self.log_ring {
            Some(r) => r,
            None => return,
        };
        let summary_body = error_summary.as_deref().map(|s| {
            let trimmed = s.trim();
            if trimmed.len() > 200 {
                format!("{}...", &trimmed[..crate::common::utf8::floor_char_boundary(trimmed, 200)])
            } else {
                trimmed.to_string()
            }
        });
        ring.record_model_call(crate::common::log_ring::ModelCallMeta {
            credential_id,
            model,
            endpoint: endpoint_name.to_string(),
            api_type: api_type.to_string(),
            status,
            duration_ms,
            retry_attempt,
            is_stream,
            error_summary: summary_body,
            input_tokens: None,
            output_tokens: None,
            cache_read_input_tokens: None,
            cache_creation_input_tokens: None,
        });
    }

    /// 根据凭据的代理配置获取（或创建并缓存）对应的 reqwest::Client
    fn client_for(&self, credentials: &KiroCredentials) -> anyhow::Result<Client> {
        let effective = credentials.effective_proxy(self.global_proxy.as_ref());
        let mut cache = self.client_cache.lock();
        if let Some(client) = cache.get(&effective) {
            return Ok(client.clone());
        }
        let client = build_client(effective.as_ref(), 720, self.tls_backend)?;
        cache.insert(effective, client.clone());
        Ok(client)
    }

    /// 根据凭据选择 endpoint 实现
    fn endpoint_for(
        &self,
        credentials: &KiroCredentials,
    ) -> anyhow::Result<Arc<dyn KiroEndpoint>> {
        let name = credentials
            .endpoint
            .as_deref()
            .unwrap_or(&self.default_endpoint);
        self.endpoints
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("未知端点: {}", name))
    }

    /// 发送非流式 API 请求
    ///
    /// 支持多凭据故障转移（见 [`Self::call_api_with_retry`]）
    pub async fn call_api(
        &self,
        request_body: &str,
        user_id: Option<&str>,
    ) -> anyhow::Result<ApiCallResult> {
        self.call_api_with_retry(request_body, false, user_id).await
    }

    /// 发送流式 API 请求
    pub async fn call_api_stream(
        &self,
        request_body: &str,
        user_id: Option<&str>,
    ) -> anyhow::Result<ApiCallResult> {
        self.call_api_with_retry(request_body, true, user_id).await
    }

    /// 发送 MCP API 请求（WebSearch 等工具调用）
    pub async fn call_mcp(&self, request_body: &str) -> anyhow::Result<reqwest::Response> {
        self.call_mcp_with_retry(request_body).await
    }

    /// 内部方法：带重试逻辑的 MCP API 调用
    async fn call_mcp_with_retry(&self, request_body: &str) -> anyhow::Result<reqwest::Response> {
        let total_credentials = self.token_manager.total_count();
        let max_retries = (total_credentials * MAX_RETRIES_PER_CREDENTIAL).min(MAX_TOTAL_RETRIES);
        let mut last_error: Option<anyhow::Error> = None;
        let mut force_refreshed: HashSet<u64> = HashSet::new();

        for attempt in 0..max_retries {
            // MCP 调用（WebSearch 等工具）不涉及模型选择，无需按模型过滤凭据
            let ctx = match self.token_manager.acquire_context(None).await {
                Ok(c) => c,
                Err(e) => {
                    last_error = Some(e);
                    continue;
                }
            };

            let config = self.token_manager.config();
            let machine_id = machine_id::generate_from_credentials(&ctx.credentials, config);

            let endpoint = match self.endpoint_for(&ctx.credentials) {
                Ok(e) => e,
                Err(e) => {
                    last_error = Some(e);
                    // endpoint 解析失败：记为失败，换下一张凭据
                    self.token_manager.report_failure(ctx.id);
                    continue;
                }
            };

            let rctx = RequestContext {
                credentials: &ctx.credentials,
                token: &ctx.token,
                machine_id: &machine_id,
                config,
            };

            let url = endpoint.mcp_url(&rctx);
            let body = endpoint
                .transform_mcp_body(request_body, &rctx)
                .unwrap_or_else(|e| {
                    tracing::warn!(
                        endpoint = endpoint.name(),
                        error = %e,
                        "transform_mcp_body 失败，回退到原 body"
                    );
                    request_body.to_string()
                });

            let base = self
                .client_for(&ctx.credentials)?
                .post(&url)
                .body(body)
                .header("content-type", "application/json")
                .header("Connection", "close");
            let request = endpoint.decorate_mcp(base, &rctx);

            // 阶段 7.9：记录 ModelCall 起始时间
            let started_at = std::time::Instant::now();
            let endpoint_name_owned = endpoint.name().to_string();

            let response = match request.send().await {
                Ok(resp) => resp,
                Err(e) => {
                    let dur = started_at.elapsed().as_millis() as u32;
                    let err_msg = e.to_string();
                    self.record_model_call(
                        ctx.id,
                        None,
                        &endpoint_name_owned,
                        "mcp",
                        0,
                        dur,
                        attempt as u32,
                        false,
                        Some(err_msg.clone()),
                    );
                    tracing::warn!(
                        "MCP 请求发送失败（尝试 {}/{}）: {}",
                        attempt + 1,
                        max_retries,
                        err_msg
                    );
                    last_error = Some(e.into());
                    if attempt + 1 < max_retries {
                        sleep(Self::retry_delay(attempt)).await;
                    }
                    continue;
                }
            };

            let status = response.status();

            // 成功响应
            if status.is_success() {
                let dur = started_at.elapsed().as_millis() as u32;
                self.record_model_call(
                    ctx.id,
                    None,
                    &endpoint_name_owned,
                    "mcp",
                    status.as_u16(),
                    dur,
                    attempt as u32,
                    false,
                    None,
                );
                self.token_manager.report_success(ctx.id);
                return Ok(response);
            }

            // 失败响应
            let body = response.text().await.unwrap_or_default();
            let dur = started_at.elapsed().as_millis() as u32;
            // 失败状态码统一记录一条
            self.record_model_call(
                ctx.id,
                None,
                &endpoint_name_owned,
                "mcp",
                status.as_u16(),
                dur,
                attempt as u32,
                false,
                Some(body.clone()),
            );

            // 402 处理：区分超额封顶（软冷却）vs 月度配额耗尽（硬禁用）
            if status.as_u16() == 402 {
                if endpoint.is_overage_limit(&body) {
                    // 阶段 7.12：开了 Kiro 超额且撞到 10000 封顶 → 24h 冷却而非禁用
                    // 等下个计费周期 / 用户开更高额度 / 主动 get_balance 触发自愈
                    self.token_manager.set_credential_cooldown_with_duration(
                        ctx.id,
                        crate::kiro::cooldown::CooldownReason::QuotaExhausted,
                        std::time::Duration::from_secs(24 * 3600),
                    );
                    tracing::warn!(
                        "MCP 请求 #{} 超额封顶 (OVERAGE)，已冷却 24h: {}",
                        ctx.id, body
                    );
                    last_error = Some(anyhow::anyhow!("MCP 请求失败（超额）: {} {}", status, body));
                    continue;
                }
                if endpoint.is_monthly_request_limit(&body) {
                    let has_available = self.token_manager.report_quota_exhausted(ctx.id);
                    if !has_available {
                        anyhow::bail!("MCP 请求失败（所有凭据已用尽）: {} {}", status, body);
                    }
                    last_error = Some(anyhow::anyhow!("MCP 请求失败: {} {}", status, body));
                    continue;
                }
            }

            // 400 Bad Request
            if status.as_u16() == 400 {
                anyhow::bail!("MCP 请求失败: {} {}", status, body);
            }

            // 401/403 凭据问题
            if matches!(status.as_u16(), 401 | 403) {
                // token 被上游失效：先尝试 force-refresh，每凭据仅一次机会
                if endpoint.is_bearer_token_invalid(&body) && !force_refreshed.contains(&ctx.id) {
                    force_refreshed.insert(ctx.id);
                    tracing::info!("凭据 #{} token 疑似被上游失效，尝试强制刷新", ctx.id);
                    if self.token_manager.force_refresh_token_for(ctx.id).await.is_ok() {
                        tracing::info!("凭据 #{} token 强制刷新成功，重试请求", ctx.id);
                        continue;
                    }
                    tracing::warn!("凭据 #{} token 强制刷新失败，计入失败", ctx.id);
                }

                let has_available = self.token_manager.report_failure(ctx.id);
                if !has_available {
                    anyhow::bail!("MCP 请求失败（所有凭据已用尽）: {} {}", status, body);
                }
                last_error = Some(anyhow::anyhow!("MCP 请求失败: {} {}", status, body));
                continue;
            }

            // 429 - 上游限流：Phase A 短退避，不长冻号。普通 429 走毫秒级短冷却
            // （下次 acquire 会短暂跳过该号，0.5~3s 自愈回池），封号才长冷却。
            if status.as_u16() == 429 {
                if crate::kiro::rate_limiter::is_account_suspended_message(&body) {
                    self.token_manager
                        .set_credential_cooldown(ctx.id, CooldownReason::AccountSuspended);
                } else {
                    self.token_manager
                        .set_credential_cooldown(ctx.id, CooldownReason::RateLimitExceeded);
                }
                tracing::warn!(
                    "MCP 请求 429，短退避跳过该号（尝试 {}/{}）: {}",
                    attempt + 1,
                    max_retries,
                    body
                );
                last_error = Some(anyhow::anyhow!("MCP 请求失败: {} {}", status, body));
                if attempt + 1 < max_retries {
                    sleep(Self::retry_delay(attempt)).await;
                }
                continue;
            }

            // 408/5xx - 瞬态错误：重试但不冻不切凭据
            if matches!(status.as_u16(), 408) || status.is_server_error() {
                tracing::warn!(
                    "MCP 请求失败（上游瞬态错误，尝试 {}/{}）: {} {}",
                    attempt + 1,
                    max_retries,
                    status,
                    body
                );
                last_error = Some(anyhow::anyhow!("MCP 请求失败: {} {}", status, body));
                if attempt + 1 < max_retries {
                    sleep(Self::retry_delay(attempt)).await;
                }
                continue;
            }

            // 其他 4xx
            if status.is_client_error() {
                anyhow::bail!("MCP 请求失败: {} {}", status, body);
            }

            // 兜底
            last_error = Some(anyhow::anyhow!("MCP 请求失败: {} {}", status, body));
            if attempt + 1 < max_retries {
                sleep(Self::retry_delay(attempt)).await;
            }
        }

        Err(last_error.unwrap_or_else(|| {
            anyhow::anyhow!("MCP 请求失败：已达到最大重试次数（{}次）", max_retries)
        }))
    }

    /// 内部方法：带重试逻辑的 API 调用
    ///
    /// 重试策略：
    /// - 每个凭据最多重试 MAX_RETRIES_PER_CREDENTIAL 次
    /// - 总重试次数 = min(凭据数量 × 每凭据重试次数, MAX_TOTAL_RETRIES)
    /// - 硬上限 9 次，避免无限重试
    async fn call_api_with_retry(
        &self,
        request_body: &str,
        is_stream: bool,
        user_id: Option<&str>,
    ) -> anyhow::Result<ApiCallResult> {
        let total_credentials = self.token_manager.total_count();
        let max_retries = (total_credentials * MAX_RETRIES_PER_CREDENTIAL).min(MAX_TOTAL_RETRIES);
        let mut last_error: Option<anyhow::Error> = None;
        let mut force_refreshed: HashSet<u64> = HashSet::new();
        // retry 链路排除上次失败的凭据，避免 affinity 命中反复返回同一个失败的绑定凭据。
        let mut failed_ids: Vec<u64> = Vec::new();
        let api_type = if is_stream { "流式" } else { "非流式" };

        // 尝试从请求体中提取模型信息
        let model = Self::extract_model_from_request(request_body);

        for attempt in 0..max_retries {
            // 获取调用上下文：带用户亲和性，同一 user_id 的连续对话尽量复用同一凭据，
            // 复用上游 prompt cache 前缀（否则 balanced 轮换会导致只有写、没有读）。
            let ctx = match self
                .token_manager
                .acquire_context_for_user_excluding(user_id, model.as_deref(), &failed_ids)
                .await
            {
                Ok(c) => c,
                Err(e) => {
                    last_error = Some(e);
                    continue;
                }
            };

            let config = self.token_manager.config();
            let machine_id = machine_id::generate_from_credentials(&ctx.credentials, config);

            let endpoint = match self.endpoint_for(&ctx.credentials) {
                Ok(e) => e,
                Err(e) => {
                    last_error = Some(e);
                    self.token_manager.report_failure(ctx.id);
                    continue;
                }
            };

            let rctx = RequestContext {
                credentials: &ctx.credentials,
                token: &ctx.token,
                machine_id: &machine_id,
                config,
            };

            let url = endpoint.api_url(&rctx);
            let body = endpoint
                .transform_api_body(request_body, &rctx)
                .unwrap_or_else(|e| {
                    tracing::warn!(
                        endpoint = endpoint.name(),
                        error = %e,
                        "transform_api_body 失败，回退到原 body"
                    );
                    request_body.to_string()
                });

            let base = self
                .client_for(&ctx.credentials)?
                .post(&url)
                .body(body)
                .header("content-type", "application/json")
                .header("Connection", "close");
            let request = endpoint.decorate_api(base, &rctx);

            // 阶段 7.9：记录起始时间
            let started_at = std::time::Instant::now();
            let endpoint_name_owned = endpoint.name().to_string();
            let api_kind = if is_stream { "anthropic_stream" } else { "anthropic" };

            let response = match request.send().await {
                Ok(resp) => resp,
                Err(e) => {
                    let dur = started_at.elapsed().as_millis() as u32;
                    let err_msg = e.to_string();
                    self.record_model_call(
                        ctx.id,
                        model.clone(),
                        &endpoint_name_owned,
                        api_kind,
                        0,
                        dur,
                        attempt as u32,
                        is_stream,
                        Some(err_msg.clone()),
                    );
                    tracing::warn!(
                        "API 请求发送失败（尝试 {}/{}）: {}",
                        attempt + 1,
                        max_retries,
                        err_msg
                    );
                    // 网络错误通常是上游/链路瞬态问题，不应导致"禁用凭据"或"切换凭据"
                    // （否则一段时间网络抖动会把所有凭据都误禁用，需要重启才能恢复）
                    last_error = Some(e.into());
                    if attempt + 1 < max_retries {
                        sleep(Self::retry_delay(attempt)).await;
                    }
                    continue;
                }
            };

            let status = response.status();

            // 成功响应
            if status.is_success() {
                let dur = started_at.elapsed().as_millis() as u32;
                // 阶段 7.15：成功记录移到 handler（拿到 token 后再记），这里只回传元数据
                self.token_manager.report_success(ctx.id);
                return Ok(ApiCallResult {
                    response,
                    credential_id: ctx.id,
                    duration_ms: dur,
                    endpoint_name: endpoint_name_owned.clone(),
                    model: model.clone(),
                    retry_attempt: attempt as u32,
                });
            }

            // 失败响应：读取 body 用于日志/错误信息
            let body = response.text().await.unwrap_or_default();
            let dur = started_at.elapsed().as_millis() as u32;
            self.record_model_call(
                ctx.id,
                model.clone(),
                &endpoint_name_owned,
                api_kind,
                status.as_u16(),
                dur,
                attempt as u32,
                is_stream,
                Some(body.clone()),
            );

            // 402 处理：区分超额封顶（软冷却）vs 月度配额耗尽（硬禁用）
            if status.as_u16() == 402 {
                if endpoint.is_overage_limit(&body) {
                    // 阶段 7.12：开了 Kiro 超额且撞到 10000 封顶 → 24h 冷却而非禁用
                    self.token_manager.set_credential_cooldown_with_duration(
                        ctx.id,
                        crate::kiro::cooldown::CooldownReason::QuotaExhausted,
                        std::time::Duration::from_secs(24 * 3600),
                    );
                    tracing::warn!(
                        "{} API 请求 #{} 超额封顶 (OVERAGE)，已冷却 24h（尝试 {}/{}）: {}",
                        api_type, ctx.id, attempt + 1, max_retries, body
                    );
                    last_error = Some(anyhow::anyhow!(
                        "{} API 请求失败（超额）: {} {}", api_type, status, body
                    ));
                    continue;
                }
                if endpoint.is_monthly_request_limit(&body) {
                    tracing::warn!(
                        "API 请求失败（额度已用尽，禁用凭据并切换，尝试 {}/{}）: {} {}",
                        attempt + 1,
                        max_retries,
                        status,
                        body
                    );

                    let has_available = self.token_manager.report_quota_exhausted(ctx.id);
                    if !has_available {
                        anyhow::bail!(
                            "{} API 请求失败（所有凭据已用尽）: {} {}",
                            api_type,
                            status,
                            body
                        );
                    }

                    last_error = Some(anyhow::anyhow!(
                        "{} API 请求失败: {} {}",
                        api_type,
                        status,
                        body
                    ));
                    continue;
                }
            }

            // 400 Bad Request - 请求问题，重试/切换凭据无意义
            if status.as_u16() == 400 {
                anyhow::bail!("{} API 请求失败: {} {}", api_type, status, body);
            }

            // 401/403 - 更可能是凭据/权限问题：计入失败并允许故障转移
            if matches!(status.as_u16(), 401 | 403) {
                tracing::warn!(
                    "API 请求失败（可能为凭据错误，尝试 {}/{}）: {} {}",
                    attempt + 1,
                    max_retries,
                    status,
                    body
                );

                // token 被上游失效：先尝试 force-refresh，每凭据仅一次机会
                if endpoint.is_bearer_token_invalid(&body) && !force_refreshed.contains(&ctx.id) {
                    force_refreshed.insert(ctx.id);
                    tracing::info!("凭据 #{} token 疑似被上游失效，尝试强制刷新", ctx.id);
                    if self.token_manager.force_refresh_token_for(ctx.id).await.is_ok() {
                        tracing::info!("凭据 #{} token 强制刷新成功，重试请求", ctx.id);
                        continue;
                    }
                    tracing::warn!("凭据 #{} token 强制刷新失败，计入失败", ctx.id);
                }

                let has_available = self.token_manager.report_failure(ctx.id);
                if !has_available {
                    anyhow::bail!(
                        "{} API 请求失败（所有凭据已用尽）: {} {}",
                        api_type,
                        status,
                        body
                    );
                }

                last_error = Some(anyhow::anyhow!(
                    "{} API 请求失败: {} {}",
                    api_type,
                    status,
                    body
                ));
                // 凭据可能尚未达到禁用阈值；排除它，避免 affinity 在本次 retry 反复选回
                failed_ids.push(ctx.id);
                continue;
            }

            // 429 - 上游限流：Phase A 改为「短退避，不长冻号」防雪崩。
            // 仅当 body 命中真·封号关键词时才长冷却剔除；普通 429 走毫秒级短退避
            // （cooldown_manager 的 RateLimitExceeded 现为 500ms→3s 可配），
            // 并把该号加入本轮 failed_ids 立刻 failover 到其他号。
            if status.as_u16() == 429 {
                if crate::kiro::rate_limiter::is_account_suspended_message(&body) {
                    self.token_manager
                        .set_credential_cooldown(ctx.id, CooldownReason::AccountSuspended);
                    tracing::warn!(
                        credential_id = ctx.id,
                        "API 请求 429 命中封号关键词，长冷却剔除（尝试 {}/{}）: {}",
                        attempt + 1,
                        max_retries,
                        body
                    );
                } else {
                    let cd = self
                        .token_manager
                        .set_credential_cooldown(ctx.id, CooldownReason::RateLimitExceeded);
                    tracing::warn!(
                        credential_id = ctx.id,
                        cooldown_ms = cd.as_millis() as u64,
                        "API 请求 429 短退避，本轮跳过该号（尝试 {}/{}）: {}",
                        attempt + 1,
                        max_retries,
                        body
                    );
                }
                last_error = Some(anyhow::anyhow!(
                    "{} API 请求失败: {} {}",
                    api_type,
                    status,
                    body
                ));
                failed_ids.push(ctx.id);
                continue;
            }

            // 408/5xx - 瞬态上游错误：重试但不冻不切凭据
            // （避免 502 high load 等瞬态错误把所有凭据锁死）
            if matches!(status.as_u16(), 408) || status.is_server_error() {
                tracing::warn!(
                    "API 请求失败（上游瞬态错误，尝试 {}/{}）: {} {}",
                    attempt + 1,
                    max_retries,
                    status,
                    body
                );
                last_error = Some(anyhow::anyhow!(
                    "{} API 请求失败: {} {}",
                    api_type,
                    status,
                    body
                ));
                if attempt + 1 < max_retries {
                    sleep(Self::retry_delay(attempt)).await;
                }
                continue;
            }

            // 其他 4xx - 通常为请求/配置问题：直接返回，不计入凭据失败
            if status.is_client_error() {
                anyhow::bail!("{} API 请求失败: {} {}", api_type, status, body);
            }

            // 兜底：当作可重试的瞬态错误处理（不切换凭据）
            tracing::warn!(
                "API 请求失败（未知错误，尝试 {}/{}）: {} {}",
                attempt + 1,
                max_retries,
                status,
                body
            );
            last_error = Some(anyhow::anyhow!(
                "{} API 请求失败: {} {}",
                api_type,
                status,
                body
            ));
            if attempt + 1 < max_retries {
                sleep(Self::retry_delay(attempt)).await;
            }
        }

        // 所有重试都失败
        Err(last_error.unwrap_or_else(|| {
            anyhow::anyhow!(
                "{} API 请求失败：已达到最大重试次数（{}次）",
                api_type,
                max_retries
            )
        }))
    }

    /// 从请求体中提取模型信息
    ///
    /// 尝试解析 JSON 请求体，提取 conversationState.currentMessage.userInputMessage.modelId
    fn extract_model_from_request(request_body: &str) -> Option<String> {
        use serde_json::Value;

        let json: Value = serde_json::from_str(request_body).ok()?;

        json.get("conversationState")?
            .get("currentMessage")?
            .get("userInputMessage")?
            .get("modelId")?
            .as_str()
            .map(|s| s.to_string())
    }

    fn retry_delay(attempt: usize) -> Duration {
        // 指数退避 + 少量抖动，避免上游抖动时放大故障
        const BASE_MS: u64 = 200;
        const MAX_MS: u64 = 2_000;
        let exp = BASE_MS.saturating_mul(2u64.saturating_pow(attempt.min(6) as u32));
        let backoff = exp.min(MAX_MS);
        let jitter_max = (backoff / 4).max(1);
        let jitter = fastrand::u64(0..=jitter_max);
        Duration::from_millis(backoff.saturating_add(jitter))
    }
}
