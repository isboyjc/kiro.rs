//! Admin API 类型定义

use serde::{Deserialize, Serialize};

// ============ 凭据状态 ============

/// 所有凭据状态响应
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialsStatusResponse {
    /// 凭据总数
    pub total: usize,
    /// 可用凭据数量（未禁用）
    pub available: usize,
    /// 当前活跃凭据 ID
    pub current_id: u64,
    /// 各凭据状态列表
    pub credentials: Vec<CredentialStatusItem>,
}

/// 单个凭据的状态信息
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStatusItem {
    /// 凭据唯一 ID
    pub id: u64,
    /// 优先级（数字越小优先级越高）
    pub priority: u32,
    /// 是否被禁用
    pub disabled: bool,
    /// 连续失败次数
    pub failure_count: u32,
    /// 是否为当前活跃凭据
    pub is_current: bool,
    /// Token 过期时间（RFC3339 格式）
    pub expires_at: Option<String>,
    /// 认证方式
    pub auth_method: Option<String>,
    /// 是否有 Profile ARN
    pub has_profile_arn: bool,
    /// refreshToken 的 SHA-256 哈希（仅 OAuth 凭据，用于前端去重）
    pub refresh_token_hash: Option<String>,
    /// kiroApiKey 的 SHA-256 哈希（仅 API Key 凭据，用于前端去重）
    pub api_key_hash: Option<String>,
    /// kiroApiKey 的脱敏展示（仅 API Key 凭据，用于前端显示）
    pub masked_api_key: Option<String>,
    /// 用户邮箱（用于前端显示）
    pub email: Option<String>,
    /// API 调用成功次数
    pub success_count: u64,
    /// 最后一次 API 调用时间（RFC3339 格式）
    pub last_used_at: Option<String>,
    /// 是否配置了凭据级代理
    pub has_proxy: bool,
    /// 代理 URL（用于前端展示）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_url: Option<String>,
    /// Token 刷新连续失败次数
    pub refresh_failure_count: u32,
    /// 禁用原因
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_reason: Option<String>,
    /// 凭据级 endpoint（None = 使用全局 defaultEndpoint）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// 实际生效的 endpoint（None 时回退到 defaultEndpoint）
    pub effective_endpoint: String,
    /// 凭据级 Region（None = 使用全局 region）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    /// 凭据级 API Region（None = 使用全局 / 凭据 region）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_region: Option<String>,
}

// ============ 操作请求 ============

/// 启用/禁用凭据请求
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetDisabledRequest {
    /// 是否禁用
    pub disabled: bool,
}

/// 修改优先级请求
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetPriorityRequest {
    /// 新优先级值
    pub priority: u32,
}

/// 修改 endpoint 请求（None = 清除，回退默认）
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetEndpointRequest {
    pub endpoint: Option<String>,
}

/// 修改 Region 请求（任意一项 None = 清除该字段）
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetRegionRequest {
    pub region: Option<String>,
    pub api_region: Option<String>,
}

/// 添加凭据请求
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddCredentialRequest {
    /// 刷新令牌（OAuth 凭据必填，API Key 凭据不需要）
    pub refresh_token: Option<String>,

    /// 认证方式（可选，默认 social）
    #[serde(default = "default_auth_method")]
    pub auth_method: String,

    /// OIDC Client ID（IdC 认证需要）
    pub client_id: Option<String>,

    /// OIDC Client Secret（IdC 认证需要）
    pub client_secret: Option<String>,

    /// 优先级（可选，默认 0）
    #[serde(default)]
    pub priority: u32,

    /// 凭据级 Region 配置（用于 OIDC token 刷新）
    /// 未配置时回退到 config.json 的全局 region
    pub region: Option<String>,

    /// 凭据级 Auth Region（用于 Token 刷新）
    pub auth_region: Option<String>,

    /// 凭据级 API Region（用于 API 请求）
    pub api_region: Option<String>,

    /// 凭据级 Machine ID（可选，64 位字符串）
    /// 未配置时回退到 config.json 的 machineId
    pub machine_id: Option<String>,

    /// 用户邮箱（可选，用于前端显示）
    pub email: Option<String>,

    /// 凭据级代理 URL（可选，特殊值 "direct" 表示不使用代理）
    pub proxy_url: Option<String>,

    /// 凭据级代理认证用户名（可选）
    pub proxy_username: Option<String>,

    /// 凭据级代理认证密码（可选）
    pub proxy_password: Option<String>,

    /// Kiro API Key（API Key 凭据必填，格式: ksk_xxxxxxxx）
    /// 设置后直接作为 Bearer Token 使用，无需 refreshToken
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kiro_api_key: Option<String>,

    /// 端点名称（可选，未配置时使用 config.defaultEndpoint）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
}

fn default_auth_method() -> String {
    "social".to_string()
}

/// 添加凭据成功响应
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddCredentialResponse {
    pub success: bool,
    pub message: String,
    /// 新添加的凭据 ID
    pub credential_id: u64,
    /// 用户邮箱（如果获取成功）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

// ============ 余额查询 ============

/// 余额查询响应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceResponse {
    /// 凭据 ID
    pub id: u64,
    /// 订阅类型
    pub subscription_title: Option<String>,
    /// 当前使用量
    pub current_usage: f64,
    /// 使用限额
    pub usage_limit: f64,
    /// 剩余额度
    pub remaining: f64,
    /// 使用百分比
    pub usage_percentage: f64,
    /// 下次重置时间（Unix 时间戳）
    pub next_reset_at: Option<f64>,
}

/// 单个凭据的缓存余额条目（用于 GET /credentials/balances/cached）
///
/// 仅从 AdminService 的磁盘缓存（`kiro_balance_cache.json`）读取，
/// 不触发任何上游请求，保证不影响号池核心路径。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedBalanceItem {
    /// 凭据 ID
    pub id: u64,
    /// 缓存的剩余额度
    pub remaining: f64,
    /// 使用限额
    pub usage_limit: f64,
    /// 使用百分比
    pub usage_percentage: f64,
    /// 订阅类型
    pub subscription_title: Option<String>,
    /// 缓存写入时间（Unix 毫秒）
    pub cached_at: u64,
    /// 缓存存活时间（秒），前端用 `cached_at + ttl_secs * 1000` 计算到期时刻
    pub ttl_secs: u64,
}

/// `GET /credentials/balances/cached` 响应
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedBalancesResponse {
    pub balances: Vec<CachedBalanceItem>,
}

// ============ 负载均衡配置 ============

/// 负载均衡模式响应
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadBalancingModeResponse {
    /// 当前模式（"priority" 或 "balanced"）
    pub mode: String,
}

/// 设置负载均衡模式请求
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetLoadBalancingModeRequest {
    /// 模式（"priority" 或 "balanced"）
    pub mode: String,
}

// ============ 通用响应 ============

/// 操作成功响应
#[derive(Debug, Serialize)]
pub struct SuccessResponse {
    pub success: bool,
    pub message: String,
}

impl SuccessResponse {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
        }
    }
}

/// 错误响应
#[derive(Debug, Serialize)]
pub struct AdminErrorResponse {
    pub error: AdminError,
}

#[derive(Debug, Serialize)]
pub struct AdminError {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

impl AdminErrorResponse {
    pub fn new(error_type: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: AdminError {
                error_type: error_type.into(),
                message: message.into(),
            },
        }
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new("invalid_request", message)
    }

    pub fn authentication_error() -> Self {
        Self::new("authentication_error", "Invalid or missing admin API key")
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new("not_found", message)
    }

    pub fn api_error(message: impl Into<String>) -> Self {
        Self::new("api_error", message)
    }

    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::new("internal_error", message)
    }
}

// ============ 阶段 5.2: 全局配置热加载 ============

/// Prompt Cache 配置响应（GET /config/prompt-cache）
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptCacheConfigResponse {
    /// 当前 TTL（秒）
    pub ttl_seconds: u64,
    /// accounting 是否启用
    pub accounting_enabled: bool,
}

/// Prompt Cache 配置更新请求（PUT /config/prompt-cache）
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePromptCacheConfigRequest {
    /// 新 TTL（秒）；TTL 变化会重建 cache_tracker
    pub ttl_seconds: u64,
    /// 新 accounting 开关
    pub accounting_enabled: bool,
}

// ============ 阶段 5.3a: 批量导入 token.json ============

/// 单个 token.json 条目（容忍各种字段缺失，由后端做验证）
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenJsonItem {
    pub provider: Option<String>,
    pub refresh_token: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub auth_method: Option<String>,
    #[serde(default)]
    pub priority: u32,
    pub region: Option<String>,
    pub api_region: Option<String>,
    pub machine_id: Option<String>,
}

/// 兼容单对象和数组两种格式（serde untagged 自动分发）
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ImportItems {
    Single(TokenJsonItem),
    Multiple(Vec<TokenJsonItem>),
}

impl ImportItems {
    pub fn into_vec(self) -> Vec<TokenJsonItem> {
        match self {
            ImportItems::Single(item) => vec![item],
            ImportItems::Multiple(items) => items,
        }
    }
}

fn default_dry_run() -> bool {
    false
}

/// 批量导入请求
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportTokenJsonRequest {
    /// dry_run=true 时只做预览，不实际加入凭据池
    #[serde(default = "default_dry_run")]
    pub dry_run: bool,
    pub items: ImportItems,
}

/// 单项导入结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportItemResult {
    /// 在请求数组中的索引（用于前端关联结果）
    pub index: usize,
    /// 凭据指纹（refresh_token 前 16 字符，可读且不泄漏完整 token）
    pub fingerprint: String,
    pub action: ImportAction,
    /// 失败/跳过/dry_run 原因
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// 实际加入的凭据 ID（仅 Added 且非 dry_run 时填）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_id: Option<u64>,
}

/// 导入动作
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportAction {
    /// 成功加入凭据池
    Added,
    /// 已存在（前缀匹配），跳过
    Skipped,
    /// 字段不合法或后端拒绝
    Invalid,
}

/// 整批汇总
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub parsed: usize,
    pub added: usize,
    pub skipped: usize,
    pub invalid: usize,
}

/// 批量导入响应
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportTokenJsonResponse {
    pub summary: ImportSummary,
    pub items: Vec<ImportItemResult>,
}

// ============ 阶段 7.9: 调用日志面板 ============

/// `GET /admin/logs` 响应。
///
/// `entries` 由 LogRing 序列化（包括 fields HashMap），无需在此重新定义结构。
/// 直接使用 `crate::common::log_ring::LogEntry` 序列化即可。
// 类型定义见 src/admin/service.rs 的 LogsResponse

// ============ 阶段 7: 配置面板 ============

/// `GET /config/raw` 响应：返回 config.json 原文
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigRawResponse {
    /// 原始 JSON 字符串（pretty）
    pub content: String,
    /// 配置文件绝对路径
    pub path: String,
}

/// `POST /config/validate` 响应
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigValidateResponse {
    pub valid: bool,
    /// 校验失败的字段错误（path 用 JSON pointer 风格）
    pub errors: Vec<ConfigFieldError>,
    /// 需要重启才能生效的字段名集合（与当前生效值对比得出）
    pub needs_restart: Vec<String>,
    /// 可热生效的字段名集合
    pub hot_reload: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFieldError {
    pub path: String,
    pub message: String,
}

/// `PUT /config` 响应
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigUpdateResponse {
    pub ok: bool,
    pub message: String,
    pub needs_restart: Vec<String>,
    pub hot_reload: Vec<String>,
    /// 若 adminApiKey 被修改，返回新值便于前端自动重连（敏感，仅此一次出现在 response）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_admin_api_key: Option<String>,
    /// 若 apiKey 被修改，返回新值便于前端展示给用户
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_api_key: Option<String>,
}

/// `GET /config/schema` 响应：字段元数据，前端按此渲染可视化表单
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSchemaResponse {
    pub groups: Vec<ConfigSchemaGroup>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSchemaGroup {
    pub id: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub needs_restart: bool,
    pub sensitive: bool,
    pub fields: Vec<ConfigSchemaField>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSchemaField {
    /// 点号路径：`compression.enabled` 或 `host`
    pub key: String,
    pub label: String,
    /// `string` | `number` | `boolean` | `enum`
    #[serde(rename = "type")]
    pub field_type: String,
    pub needs_restart: bool,
    pub sensitive: bool,
    /// 是否可选 null（前端展示"未配置"）
    pub nullable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub enum_options: Vec<ConfigSchemaEnumOption>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSchemaEnumOption {
    pub value: String,
    pub label: String,
}
