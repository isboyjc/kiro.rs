//! Anthropic API 路由配置

use std::sync::Arc;

use axum::{
    Router,
    extract::DefaultBodyLimit,
    middleware,
    routing::{get, post},
};
use parking_lot::RwLock;

use crate::kiro::provider::KiroProvider;
use crate::model::config::CompressionConfig;

use super::{
    PromptCacheRuntime,
    handlers::{count_tokens, get_models, post_messages, post_messages_cc},
    middleware::{AppState, auth_middleware, cors_layer},
};

/// 请求体最大大小限制 (50MB)
const MAX_BODY_SIZE: usize = 50 * 1024 * 1024;

/// 创建 Anthropic API 路由
///
/// # 端点
/// - `GET /v1/models` - 获取可用模型列表
/// - `POST /v1/messages` - 创建消息（对话）
/// - `POST /v1/messages/count_tokens` - 计算 token 数量
///
/// # 认证
/// 所有 `/v1` 路径需要 API Key 认证，支持：
/// - `x-api-key` header
/// - `Authorization: Bearer <token>` header
///
/// # 参数
/// - `api_key`: API 密钥，用于验证客户端请求
/// - `kiro_provider`: 可选的 KiroProvider，用于调用上游 API

/// 创建带有 KiroProvider 的 Anthropic API 路由
///
/// `api_key` / `extract_thinking` 接收外部共享的 `Arc<RwLock<>>`——admin 端
/// 写入后下次请求即生效，无需重启。
#[allow(clippy::too_many_arguments)]
pub fn create_router_with_provider(
    api_key: Arc<RwLock<String>>,
    kiro_provider: Option<KiroProvider>,
    extract_thinking: Arc<RwLock<bool>>,
    compression_config: Arc<RwLock<CompressionConfig>>,
    prompt_cache_runtime: Arc<RwLock<PromptCacheRuntime>>,
    log_ring: Option<Arc<crate::common::log_ring::LogRing>>,
) -> Router {
    let mut state = AppState::new(String::new(), false)
        .with_api_key_shared(api_key)
        .with_extract_thinking_shared(extract_thinking)
        .with_compression_config_shared(compression_config)
        .with_prompt_cache_runtime(prompt_cache_runtime);
    if let Some(provider) = kiro_provider {
        state = state.with_kiro_provider(provider);
    }
    if let Some(ring) = log_ring {
        state = state.with_log_ring(ring);
    }

    // 需要认证的 /v1 路由
    let v1_routes = Router::new()
        .route("/models", get(get_models))
        .route("/messages", post(post_messages))
        .route("/messages/count_tokens", post(count_tokens))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // 需要认证的 /cc/v1 路由（Claude Code 兼容端点）
    // 与 /v1 的区别：流式响应会等待 contextUsageEvent 后再发送 message_start
    let cc_v1_routes = Router::new()
        .route("/messages", post(post_messages_cc))
        .route("/messages/count_tokens", post(count_tokens))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    Router::new()
        .nest("/v1", v1_routes)
        .nest("/cc/v1", cc_v1_routes)
        .layer(cors_layer())
        .layer(DefaultBodyLimit::max(MAX_BODY_SIZE))
        .with_state(state)
}
