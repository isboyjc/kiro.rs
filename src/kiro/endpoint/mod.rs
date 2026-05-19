//! Kiro 端点抽象
//!
//! 不同 Kiro 端点（如 `ide` / `cli`）在 URL、请求头、请求体上存在差异，
//! 但共享凭据池、Token 刷新、重试逻辑和 AWS event-stream 响应解码。
//!
//! [`KiroEndpoint`] 抽象了请求侧的差异点；`KiroProvider` 持有一个 endpoint 注册表，
//! 按凭据的 `endpoint` 字段选择对应实现。

use reqwest::RequestBuilder;

use crate::kiro::model::credentials::KiroCredentials;
use crate::model::config::Config;

pub mod cli;
pub mod ide;

// 阶段 4 接入 token_manager 后 *_ENDPOINT_NAME 会有内部使用；目前仅作 public API 暴露
#[allow(unused_imports)]
pub use cli::{CLI_ENDPOINT_NAME, CliEndpoint};
#[allow(unused_imports)]
pub use ide::{IDE_ENDPOINT_NAME, IdeEndpoint};

/// `getUsageLimits` 请求所需的 URL 与端点特有头部
///
/// 由 endpoint 实现产出，caller 负责加上凭据无关的通用头与 body。
#[allow(dead_code)] // 阶段 2 引入，阶段 4 重构 token_manager 时接入
pub struct UsageRequestParts {
    /// 完整 URL（含 query string）
    pub url: String,
    /// 端点特有的请求头列表（key 用 &'static str，value 为运行时拼装）
    pub headers: Vec<(&'static str, String)>,
}

/// Kiro 端点
///
/// 同一个 `KiroProvider` 可持有多个 endpoint 实现，按凭据级字段切换。
pub trait KiroEndpoint: Send + Sync {
    /// 端点名称（对应 credentials.endpoint / config.defaultEndpoint 的取值）
    fn name(&self) -> &'static str;

    /// API endpoint URL
    fn api_url(&self, ctx: &RequestContext<'_>) -> String;

    /// MCP endpoint URL
    fn mcp_url(&self, ctx: &RequestContext<'_>) -> String;

    /// 装饰 API 请求的端点特有 header
    ///
    /// Provider 已经设置好 URL、content-type、Connection 和 body；
    /// 实现负责追加 Authorization、host、user-agent 等端点相关头。
    fn decorate_api(&self, req: RequestBuilder, ctx: &RequestContext<'_>) -> RequestBuilder;

    /// 装饰 MCP 请求的端点特有 header
    fn decorate_mcp(&self, req: RequestBuilder, ctx: &RequestContext<'_>) -> RequestBuilder;

    /// 对已序列化的 API 请求体做端点特有加工（如注入 profileArn、wire-order 重整）
    fn transform_api_body(&self, body: &str, ctx: &RequestContext<'_>) -> anyhow::Result<String>;

    /// 对已序列化的 MCP 请求体做端点特有加工（默认不变）
    fn transform_mcp_body(&self, body: &str, _ctx: &RequestContext<'_>) -> anyhow::Result<String> {
        Ok(body.to_string())
    }

    /// 构造 `getUsageLimits` 请求所需的 URL 与端点特有头部
    ///
    /// caller 应在此基础上追加 Authorization 与凭据无关头，并以 GET 方式发送。
    /// 当前 upstream caller 还未接入（直接用硬编码 URL），方法已就位供阶段 4 重构使用。
    #[allow(dead_code)] // 阶段 2 引入，阶段 4 重构 token_manager 时接入
    fn usage_request_parts(&self, ctx: &RequestContext<'_>) -> anyhow::Result<UsageRequestParts>;

    /// 判断响应体是否表示"月度配额用尽"（禁用凭据并转移）
    fn is_monthly_request_limit(&self, body: &str) -> bool {
        default_is_monthly_request_limit(body)
    }

    /// 阶段 7.12：判断响应体是否表示"超额封顶"（用户开了 Kiro 超额且已到上限）
    ///
    /// 与 `is_monthly_request_limit` 的区别：
    /// - MONTHLY_REQUEST_COUNT: 未开超额时撞到基础订阅额度（如 Pro 1000）→ 当前禁用
    /// - OVERAGE: 开了超额但撞到超额封顶（如 Pro 10000）→ 软冷却 24h 等下个周期
    fn is_overage_limit(&self, body: &str) -> bool {
        default_is_overage_limit(body)
    }

    /// 判断响应体是否表示"上游 bearer token 失效"（触发强制刷新）
    fn is_bearer_token_invalid(&self, body: &str) -> bool {
        default_is_bearer_token_invalid(body)
    }
}

/// 装饰请求时可用的上下文
///
/// 包含单次调用已确定的所有运行时信息。引用形式避免无谓 clone。
pub struct RequestContext<'a> {
    /// 当前凭据
    pub credentials: &'a KiroCredentials,
    /// 有效的 access token（API Key 凭据下即 kiroApiKey）
    pub token: &'a str,
    /// 当前凭据对应的 machineId
    pub machine_id: &'a str,
    /// 全局配置
    pub config: &'a Config,
}

/// 默认的 MONTHLY_REQUEST_COUNT 判断逻辑
///
/// 同时识别顶层 `reason` 字段和嵌套 `error.reason` 字段。
pub fn default_is_monthly_request_limit(body: &str) -> bool {
    if body.contains("MONTHLY_REQUEST_COUNT") {
        return true;
    }

    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return false;
    };

    if value
        .get("reason")
        .and_then(|v| v.as_str())
        .is_some_and(|v| v == "MONTHLY_REQUEST_COUNT")
    {
        return true;
    }

    value
        .pointer("/error/reason")
        .and_then(|v| v.as_str())
        .is_some_and(|v| v == "MONTHLY_REQUEST_COUNT")
}

/// 阶段 7.12：默认的 OVERAGE 判断逻辑
///
/// 与 Kiro-Go 的 `checkOverageError` (handler.go:1297) 对齐——简单关键词匹配，
/// 同时检查 JSON 结构化字段以增强健壮性。
pub fn default_is_overage_limit(body: &str) -> bool {
    let upper = body.to_uppercase();
    if upper.contains("OVERAGE") {
        return true;
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return false;
    };
    if value
        .get("reason")
        .and_then(|v| v.as_str())
        .is_some_and(|v| v.to_uppercase().contains("OVERAGE"))
    {
        return true;
    }
    value
        .pointer("/error/reason")
        .and_then(|v| v.as_str())
        .is_some_and(|v| v.to_uppercase().contains("OVERAGE"))
}

/// 默认的 bearer token 失效判断逻辑
pub fn default_is_bearer_token_invalid(body: &str) -> bool {
    body.contains("The bearer token included in the request is invalid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_monthly_request_limit_detects_reason() {
        let body = r#"{"message":"You have reached the limit.","reason":"MONTHLY_REQUEST_COUNT"}"#;
        assert!(default_is_monthly_request_limit(body));
    }

    #[test]
    fn test_default_monthly_request_limit_nested_reason() {
        let body = r#"{"error":{"reason":"MONTHLY_REQUEST_COUNT"}}"#;
        assert!(default_is_monthly_request_limit(body));
    }

    #[test]
    fn test_default_monthly_request_limit_false() {
        let body = r#"{"message":"nope","reason":"DAILY_REQUEST_COUNT"}"#;
        assert!(!default_is_monthly_request_limit(body));
    }

    // 阶段 7.12：OVERAGE 检测测试
    #[test]
    fn test_default_overage_limit_keyword() {
        assert!(default_is_overage_limit("User exceeded OVERAGE cap"));
        assert!(default_is_overage_limit("overage limit reached"));
    }

    #[test]
    fn test_default_overage_limit_structured_reason() {
        assert!(default_is_overage_limit(r#"{"reason":"OVERAGE_CAP_REACHED"}"#));
        assert!(default_is_overage_limit(r#"{"error":{"reason":"OVERAGE"}}"#));
    }

    #[test]
    fn test_default_overage_limit_false() {
        assert!(!default_is_overage_limit(r#"{"reason":"MONTHLY_REQUEST_COUNT"}"#));
        assert!(!default_is_overage_limit("just a normal message"));
    }

    #[test]
    fn test_default_bearer_token_invalid() {
        assert!(default_is_bearer_token_invalid(
            "The bearer token included in the request is invalid"
        ));
        assert!(!default_is_bearer_token_invalid("unrelated error"));
    }
}
