//! Admin API HTTP 处理器

use axum::{
    Json,
    extract::{Path, Query, State},
    response::IntoResponse,
};

use crate::model::config::{CompressionConfig, Config};

use super::{
    middleware::AdminState,
    types::{
        AddCredentialRequest, ImportTokenJsonRequest, SetDisabledRequest, SetEndpointRequest,
        SetLoadBalancingModeRequest, SetPriorityRequest, SetRegionRequest, SuccessResponse,
        TestModelRequest, UpdatePromptCacheConfigRequest,
    },
};

/// GET /api/admin/credentials
/// 获取所有凭据状态
pub async fn get_all_credentials(State(state): State<AdminState>) -> impl IntoResponse {
    let response = state.service.get_all_credentials();
    Json(response)
}

/// POST /api/admin/credentials/:id/disabled
/// 设置凭据禁用状态
pub async fn set_credential_disabled(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
    Json(payload): Json<SetDisabledRequest>,
) -> impl IntoResponse {
    match state.service.set_disabled(id, payload.disabled) {
        Ok(_) => {
            let action = if payload.disabled { "禁用" } else { "启用" };
            Json(SuccessResponse::new(format!("凭据 #{} 已{}", id, action))).into_response()
        }
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// POST /api/admin/credentials/:id/priority
/// 设置凭据优先级
pub async fn set_credential_priority(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
    Json(payload): Json<SetPriorityRequest>,
) -> impl IntoResponse {
    match state.service.set_priority(id, payload.priority) {
        Ok(_) => Json(SuccessResponse::new(format!(
            "凭据 #{} 优先级已设置为 {}",
            id, payload.priority
        )))
        .into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// POST /api/admin/credentials/:id/endpoint
/// 设置凭据 endpoint（endpoint 字段 null 表示清除回退到默认）
pub async fn set_credential_endpoint(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
    Json(payload): Json<SetEndpointRequest>,
) -> impl IntoResponse {
    match state.service.set_endpoint(id, payload.endpoint) {
        Ok(_) => Json(SuccessResponse::new(format!("凭据 #{} endpoint 已更新", id)))
            .into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// POST /api/admin/credentials/:id/region
/// 设置凭据 Region 与 API Region
pub async fn set_credential_region(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
    Json(payload): Json<SetRegionRequest>,
) -> impl IntoResponse {
    match state.service.set_region(id, payload.region, payload.api_region) {
        Ok(_) => Json(SuccessResponse::new(format!("凭据 #{} Region 已更新", id)))
            .into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// POST /api/admin/credentials/:id/reset
/// 重置失败计数并重新启用
pub async fn reset_failure_count(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    match state.service.reset_and_enable(id) {
        Ok(_) => Json(SuccessResponse::new(format!(
            "凭据 #{} 失败计数已重置并重新启用",
            id
        )))
        .into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// GET /api/admin/credentials/:id/balance
/// 获取指定凭据的余额
pub async fn get_credential_balance(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    match state.service.get_balance(id).await {
        Ok(response) => Json(response).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// GET /api/admin/credentials/:id/models
/// 阶段 7.16：列出指定凭据的可用模型
pub async fn get_credential_models(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    match state.service.list_models(id).await {
        Ok(models) => Json(serde_json::json!({ "models": models })).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// POST /api/admin/credentials/:id/test-model
/// 阶段 7.17：用指定凭据真实发一条 "hi" 测试某模型是否可用
pub async fn test_credential_model(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
    Json(payload): Json<TestModelRequest>,
) -> impl IntoResponse {
    match state.service.test_model(id, &payload.model_id).await {
        Ok(result) => Json(result).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// GET /api/admin/credentials/balances/cached
/// 读取所有凭据的缓存余额快照（不触发上游请求，前端用于汇总展示）
pub async fn get_cached_balances(State(state): State<AdminState>) -> impl IntoResponse {
    Json(state.service.get_cached_balances())
}

/// POST /api/admin/credentials
/// 添加新凭据
pub async fn add_credential(
    State(state): State<AdminState>,
    Json(payload): Json<AddCredentialRequest>,
) -> impl IntoResponse {
    match state.service.add_credential(payload).await {
        Ok(response) => Json(response).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// DELETE /api/admin/credentials/:id
/// 删除凭据
pub async fn delete_credential(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    match state.service.delete_credential(id) {
        Ok(_) => Json(SuccessResponse::new(format!("凭据 #{} 已删除", id))).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// POST /api/admin/credentials/:id/refresh
/// 强制刷新凭据 Token
pub async fn force_refresh_token(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    match state.service.force_refresh_token(id).await {
        Ok(_) => Json(SuccessResponse::new(format!(
            "凭据 #{} Token 已强制刷新",
            id
        )))
        .into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// GET /api/admin/config/load-balancing
/// 获取负载均衡模式
pub async fn get_load_balancing_mode(State(state): State<AdminState>) -> impl IntoResponse {
    let response = state.service.get_load_balancing_mode();
    Json(response)
}

/// PUT /api/admin/config/load-balancing
/// 设置负载均衡模式
pub async fn set_load_balancing_mode(
    State(state): State<AdminState>,
    Json(payload): Json<SetLoadBalancingModeRequest>,
) -> impl IntoResponse {
    match state.service.set_load_balancing_mode(payload) {
        Ok(response) => Json(response).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

// ============ 阶段 5.2: 全局配置热加载 ============

/// GET /api/admin/config/compression
/// 获取当前 CompressionConfig
pub async fn get_compression_config(State(state): State<AdminState>) -> impl IntoResponse {
    Json(state.service.get_compression_config())
}

/// PUT /api/admin/config/compression
/// 全量替换 CompressionConfig
pub async fn update_compression_config(
    State(state): State<AdminState>,
    Json(new_config): Json<CompressionConfig>,
) -> impl IntoResponse {
    state.service.update_compression_config(new_config);
    Json(SuccessResponse::new("CompressionConfig 已热更新"))
}

/// GET /api/admin/config/prompt-cache
/// 获取当前 Prompt Cache 配置
pub async fn get_prompt_cache_config(State(state): State<AdminState>) -> impl IntoResponse {
    Json(state.service.get_prompt_cache_config())
}

/// PUT /api/admin/config/prompt-cache
/// 全量替换 Prompt Cache 配置（TTL 变化会重建 cache_tracker）
pub async fn update_prompt_cache_config(
    State(state): State<AdminState>,
    Json(req): Json<UpdatePromptCacheConfigRequest>,
) -> impl IntoResponse {
    state.service.update_prompt_cache_config(req);
    Json(SuccessResponse::new("PromptCacheRuntime 已热更新"))
}

/// POST /api/admin/credentials/import-token-json
/// 批量导入 token.json 数组（支持单对象或数组、dry_run 预览）
pub async fn import_token_json(
    State(state): State<AdminState>,
    Json(req): Json<ImportTokenJsonRequest>,
) -> impl IntoResponse {
    let response = state.service.import_token_json(req).await;
    Json(response)
}

// ============ 阶段 7: 配置面板 ============

/// GET /api/admin/config
/// 返回当前 config.json 内容（结构化）
pub async fn get_config(State(state): State<AdminState>) -> impl IntoResponse {
    match state.service.get_config() {
        Ok(c) => Json(c).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// GET /api/admin/config/raw
/// 返回原始 config.json 文本 + 文件路径
pub async fn get_config_raw(State(state): State<AdminState>) -> impl IntoResponse {
    match state.service.get_config_raw() {
        Ok(c) => Json(c).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

/// GET /api/admin/config/schema
/// 字段元数据（前端按此渲染可视化表单）
pub async fn get_config_schema(State(state): State<AdminState>) -> impl IntoResponse {
    Json(state.service.get_config_schema())
}

// ============ 阶段 7.9：日志面板 ============

/// GET /api/admin/logs
pub async fn get_logs(
    State(state): State<AdminState>,
    Query(params): Query<crate::admin::service::LogsQueryParams>,
) -> impl IntoResponse {
    Json(state.service.query_logs(params.into_filter()))
}

/// DELETE /api/admin/logs
pub async fn clear_logs(State(state): State<AdminState>) -> impl IntoResponse {
    state.service.clear_logs();
    Json(SuccessResponse::new("日志缓冲已清空"))
}

/// POST /api/admin/config/validate
/// 仅校验，不写盘
pub async fn validate_config(
    State(state): State<AdminState>,
    Json(new_config): Json<Config>,
) -> impl IntoResponse {
    Json(state.service.validate_config(new_config))
}

/// PUT /api/admin/config
/// 全量替换 + 写盘 + 投射热生效字段
pub async fn update_config(
    State(state): State<AdminState>,
    Json(new_config): Json<Config>,
) -> impl IntoResponse {
    match state.service.update_config(new_config) {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}
