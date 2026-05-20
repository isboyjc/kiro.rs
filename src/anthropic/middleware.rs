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

    /// 当前 TTL（秒）。阶段 5.2 admin GET 端点用
    pub fn ttl_seconds(&self) -> u64 {
        self.ttl_seconds
    }

    /// 当前 accounting 开关。阶段 5.2 admin GET 端点用
    pub fn accounting_enabled(&self) -> bool {
        self.accounting_enabled
    }

    /// 热加载：调整 TTL 或 accounting 开关。TTL 变化会重建 tracker。
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
    /// API 密钥（阶段 7：`Arc<RwLock<String>>` 支持 admin 热轮换，
    /// 修改后下次请求生效，无需重启）
    pub api_key: Arc<RwLock<String>>,
    /// Kiro Provider（可选，用于实际 API 调用）
    /// 内部使用 MultiTokenManager，已支持线程安全的多凭据管理
    pub kiro_provider: Option<Arc<KiroProvider>>,
    /// 是否开启非流式响应的 thinking 块提取（阶段 7：升级为 RwLock 支持热切换）
    pub extract_thinking: Arc<RwLock<bool>>,
    /// 输入压缩与图片处理配置（阶段 3.2 接入，阶段 5.1 升级为 RwLock）
    ///
    /// `Arc<RwLock<CompressionConfig>>` 支持运行时热更新：阶段 5.2 的
    /// admin PATCH 端点写入该锁后，下次请求即生效。读侧需 `.read().clone()`
    /// 拿快照后再传给 converter（避免 RwLockReadGuard 跨越 .await 边界）。
    pub compression_config: Arc<RwLock<CompressionConfig>>,
    /// Prompt Cache 运行时（阶段 3.3 引入）
    ///
    /// 持有 `Arc<RwLock<PromptCacheRuntime>>` 以预留阶段 5 热加载能力。
    /// 当前 stream/handlers/websearch 尚未接入 cache 拆分，模块就位供未来使用。
    pub prompt_cache_runtime: Arc<RwLock<PromptCacheRuntime>>,
    /// 阶段 7.15：日志环形缓冲（与 main.rs / provider / admin 共用），
    /// 用于在 handler 层记录带 token 的成功 ModelCall 日志
    pub log_ring: Option<Arc<crate::common::log_ring::LogRing>>,
}

impl AppState {
    /// 创建新的应用状态
    pub fn new(api_key: impl Into<String>, extract_thinking: bool) -> Self {
        Self {
            api_key: Arc::new(RwLock::new(api_key.into())),
            kiro_provider: None,
            extract_thinking: Arc::new(RwLock::new(extract_thinking)),
            compression_config: Arc::new(RwLock::new(CompressionConfig::default())),
            // 默认 TTL 300s（5m），accounting 启用——与 Anthropic ephemeral 默认值对齐
            prompt_cache_runtime: Arc::new(RwLock::new(PromptCacheRuntime::new(300, true))),
            log_ring: None,
        }
    }

    /// 阶段 7.15：注入日志环形缓冲
    pub fn with_log_ring(mut self, log_ring: Arc<crate::common::log_ring::LogRing>) -> Self {
        self.log_ring = Some(log_ring);
        self
    }

    /// 设置 KiroProvider
    pub fn with_kiro_provider(mut self, provider: KiroProvider) -> Self {
        self.kiro_provider = Some(Arc::new(provider));
        self
    }

    /// 设置共享的 api_key RwLock（admin 与 anthropic 共用同一把锁实现热轮换）
    pub fn with_api_key_shared(mut self, api_key: Arc<RwLock<String>>) -> Self {
        self.api_key = api_key;
        self
    }

    /// 设置共享的 extract_thinking RwLock
    pub fn with_extract_thinking_shared(mut self, extract_thinking: Arc<RwLock<bool>>) -> Self {
        self.extract_thinking = extract_thinking;
        self
    }

    /// 设置压缩与图片处理配置（接收外部共享的 RwLock，便于 admin 与
    /// anthropic 路由共用同一份热更新源）
    pub fn with_compression_config_shared(
        mut self,
        config: Arc<RwLock<CompressionConfig>>,
    ) -> Self {
        self.compression_config = config;
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

    /// 读取当前 extract_thinking 快照
    pub fn extract_thinking_snapshot(&self) -> bool {
        *self.extract_thinking.read()
    }
}

/// API Key 认证中间件
pub async fn auth_middleware(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let current_key = state.api_key.read().clone();
    match auth::extract_api_key(&request) {
        Some(key) if auth::constant_time_eq(&key, &current_key) => next.run(request).await,
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
