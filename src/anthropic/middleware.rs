//! Anthropic API 中间件

use std::sync::Arc;
use std::time::Duration;

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use parking_lot::RwLock;

use crate::common::auth;
use crate::kiro::provider::KiroProvider;
use crate::model::config::CompressionConfig;

use super::cache_tracker::CacheTracker;
use super::types::ErrorResponse;

/// Prompt Cache 运行时快照（克隆便宜，per-request 取一份）
#[derive(Clone)]
#[allow(dead_code)] // 阶段 3.3 引入；caller 接入留待用户主动选择 cache 拆分时
pub(crate) struct PromptCacheSnapshot {
    pub accounting_enabled: bool,
    pub ttl_seconds: u64,
    pub tracker: Arc<CacheTracker>,
}

/// Prompt Cache 运行时配置
///
/// 持有共享 `CacheTracker` 实例。阶段 5 启用 admin 热加载后，
/// `update()` 可在运行时调整 TTL/accounting，无需重启。
pub struct PromptCacheRuntime {
    accounting_enabled: bool,
    ttl_seconds: u64,
    tracker: Arc<CacheTracker>,
}

impl PromptCacheRuntime {
    pub fn new(ttl_seconds: u64, accounting_enabled: bool) -> Self {
        Self {
            accounting_enabled,
            ttl_seconds,
            tracker: Arc::new(CacheTracker::new(Duration::from_secs(ttl_seconds))),
        }
    }

    #[allow(dead_code)] // 阶段 3.3 引入；caller 接入留待用户主动选择 cache 拆分时
    pub(crate) fn snapshot(&self) -> PromptCacheSnapshot {
        PromptCacheSnapshot {
            accounting_enabled: self.accounting_enabled,
            ttl_seconds: self.ttl_seconds,
            tracker: self.tracker.clone(),
        }
    }

    /// 热加载：调整 TTL 或 accounting 开关。TTL 变化会重建 tracker。
    #[allow(dead_code)] // 阶段 5 admin 热加载会调用
    pub fn update(&mut self, ttl_seconds: Option<u64>, accounting_enabled: Option<bool>) {
        if let Some(value) = accounting_enabled {
            self.accounting_enabled = value;
        }

        if let Some(value) = ttl_seconds
            && self.ttl_seconds != value
        {
            self.ttl_seconds = value;
            self.tracker = Arc::new(CacheTracker::new(Duration::from_secs(value)));
        }
    }
}

/// 应用共享状态
#[derive(Clone)]
pub struct AppState {
    /// API 密钥
    pub api_key: String,
    /// Kiro Provider（可选，用于实际 API 调用）
    /// 内部使用 MultiTokenManager，已支持线程安全的多凭据管理
    pub kiro_provider: Option<Arc<KiroProvider>>,
    /// 是否开启非流式响应的 thinking 块提取
    pub extract_thinking: bool,
    /// 输入压缩与图片处理配置（阶段 3.2 接入）
    ///
    /// 当前为不可变 `Arc<CompressionConfig>`；阶段 5 引入热加载时
    /// 将升级为 `Arc<RwLock<CompressionConfig>>`。
    pub compression_config: Arc<CompressionConfig>,
    /// Prompt Cache 运行时（阶段 3.3 引入）
    ///
    /// 持有 `Arc<RwLock<PromptCacheRuntime>>` 以预留阶段 5 热加载能力。
    /// 当前 stream/handlers/websearch 尚未接入 cache 拆分，模块就位供未来使用。
    pub prompt_cache_runtime: Arc<RwLock<PromptCacheRuntime>>,
}

impl AppState {
    /// 创建新的应用状态
    pub fn new(api_key: impl Into<String>, extract_thinking: bool) -> Self {
        Self {
            api_key: api_key.into(),
            kiro_provider: None,
            extract_thinking,
            compression_config: Arc::new(CompressionConfig::default()),
            // 默认 TTL 300s（5m），accounting 启用——与 Anthropic ephemeral 默认值对齐
            prompt_cache_runtime: Arc::new(RwLock::new(PromptCacheRuntime::new(300, true))),
        }
    }

    /// 设置 KiroProvider
    pub fn with_kiro_provider(mut self, provider: KiroProvider) -> Self {
        self.kiro_provider = Some(Arc::new(provider));
        self
    }

    /// 设置压缩与图片处理配置
    pub fn with_compression_config(mut self, config: CompressionConfig) -> Self {
        self.compression_config = Arc::new(config);
        self
    }

    /// 设置 Prompt Cache 运行时（main.rs 注入用户配置）
    pub fn with_prompt_cache_runtime(
        mut self,
        runtime: Arc<RwLock<PromptCacheRuntime>>,
    ) -> Self {
        self.prompt_cache_runtime = runtime;
        self
    }
}

/// API Key 认证中间件
pub async fn auth_middleware(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    match auth::extract_api_key(&request) {
        Some(key) if auth::constant_time_eq(&key, &state.api_key) => next.run(request).await,
        _ => {
            let error = ErrorResponse::authentication_error();
            (StatusCode::UNAUTHORIZED, Json(error)).into_response()
        }
    }
}

/// CORS 中间件层
///
/// **安全说明**：当前配置允许所有来源（Any），这是为了支持公开 API 服务。
/// 如果需要更严格的安全控制，请根据实际需求配置具体的允许来源、方法和头信息。
///
/// # 配置说明
/// - `allow_origin(Any)`: 允许任何来源的请求
/// - `allow_methods(Any)`: 允许任何 HTTP 方法
/// - `allow_headers(Any)`: 允许任何请求头
pub fn cors_layer() -> tower_http::cors::CorsLayer {
    use tower_http::cors::{Any, CorsLayer};

    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
}
