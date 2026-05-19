//! Admin API 路由配置

use axum::{
    Router, middleware,
    routing::{delete, get, post},
};

use super::{
    handlers::{
        add_credential, delete_credential, force_refresh_token, get_all_credentials,
        get_cached_balances, get_compression_config, get_config, get_config_raw,
        get_config_schema, get_credential_balance, get_load_balancing_mode,
        get_prompt_cache_config, import_token_json, reset_failure_count,
        set_credential_disabled, set_credential_endpoint, set_credential_priority,
        set_credential_region, set_load_balancing_mode, update_compression_config,
        update_config, update_prompt_cache_config, validate_config,
    },
    middleware::{AdminState, admin_auth_middleware},
};

/// 创建 Admin API 路由
///
/// # 端点
/// - `GET /credentials` - 获取所有凭据状态
/// - `POST /credentials` - 添加新凭据
/// - `DELETE /credentials/:id` - 删除凭据
/// - `POST /credentials/:id/disabled` - 设置凭据禁用状态
/// - `POST /credentials/:id/priority` - 设置凭据优先级
/// - `POST /credentials/:id/reset` - 重置失败计数
/// - `POST /credentials/:id/refresh` - 强制刷新 Token
/// - `GET /credentials/:id/balance` - 获取凭据余额
/// - `GET /credentials/balances/cached` - 读取所有凭据的缓存余额（不触发上游请求）
/// - `GET /config/load-balancing` - 获取负载均衡模式
/// - `PUT /config/load-balancing` - 设置负载均衡模式
///
/// # 认证
/// 需要 Admin API Key 认证，支持：
/// - `x-api-key` header
/// - `Authorization: Bearer <token>` header
pub fn create_admin_router(state: AdminState) -> Router {
    Router::new()
        .route(
            "/credentials",
            get(get_all_credentials).post(add_credential),
        )
        // 阶段 5.3a 批量导入端点
        .route("/credentials/import-token-json", post(import_token_json))
        // 阶段 5.3c 缓存余额汇总端点（静态路径需先于 `/credentials/{id}/...` 注册）
        .route("/credentials/balances/cached", get(get_cached_balances))
        .route("/credentials/{id}", delete(delete_credential))
        .route("/credentials/{id}/disabled", post(set_credential_disabled))
        .route("/credentials/{id}/priority", post(set_credential_priority))
        .route("/credentials/{id}/endpoint", post(set_credential_endpoint))
        .route("/credentials/{id}/region", post(set_credential_region))
        .route("/credentials/{id}/reset", post(reset_failure_count))
        .route("/credentials/{id}/refresh", post(force_refresh_token))
        .route("/credentials/{id}/balance", get(get_credential_balance))
        .route(
            "/config/load-balancing",
            get(get_load_balancing_mode).put(set_load_balancing_mode),
        )
        // 阶段 5.2 全局配置热加载端点
        .route(
            "/config/compression",
            get(get_compression_config).put(update_compression_config),
        )
        .route(
            "/config/prompt-cache",
            get(get_prompt_cache_config).put(update_prompt_cache_config),
        )
        // 阶段 7：配置面板（Raw JSON / 可视化共用）
        .route("/config/raw", get(get_config_raw))
        .route("/config/schema", get(get_config_schema))
        .route("/config/validate", post(validate_config))
        .route("/config", get(get_config).put(update_config))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            admin_auth_middleware,
        ))
        .with_state(state)
}
