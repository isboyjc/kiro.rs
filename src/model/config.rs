use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TlsBackend {
    Rustls,
    NativeTls,
}

impl Default for TlsBackend {
    fn default() -> Self {
        Self::Rustls
    }
}

/// KNA 应用配置
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(default = "default_host")]
    pub host: String,

    #[serde(default = "default_port")]
    pub port: u16,

    #[serde(default = "default_region")]
    pub region: String,

    /// Auth Region（用于 Token 刷新），未配置时回退到 region
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_region: Option<String>,

    /// API Region（用于 API 请求），未配置时回退到 region
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_region: Option<String>,

    #[serde(default = "default_kiro_version")]
    pub kiro_version: String,

    #[serde(default)]
    pub machine_id: Option<String>,

    #[serde(default)]
    pub api_key: Option<String>,

    #[serde(default = "default_system_version")]
    pub system_version: String,

    #[serde(default = "default_node_version")]
    pub node_version: String,

    #[serde(default = "default_tls_backend")]
    pub tls_backend: TlsBackend,

    /// 外部 count_tokens API 地址（可选）
    #[serde(default)]
    pub count_tokens_api_url: Option<String>,

    /// count_tokens API 密钥（可选）
    #[serde(default)]
    pub count_tokens_api_key: Option<String>,

    /// count_tokens API 认证类型（可选，"x-api-key" 或 "bearer"，默认 "x-api-key"）
    #[serde(default = "default_count_tokens_auth_type")]
    pub count_tokens_auth_type: String,

    /// HTTP 代理地址（可选）
    /// 支持格式: http://host:port, https://host:port, socks5://host:port
    #[serde(default)]
    pub proxy_url: Option<String>,

    /// 代理认证用户名（可选）
    #[serde(default)]
    pub proxy_username: Option<String>,

    /// 代理认证密码（可选）
    #[serde(default)]
    pub proxy_password: Option<String>,

    /// Admin API 密钥（可选，启用 Admin API 功能）
    #[serde(default)]
    pub admin_api_key: Option<String>,

    /// 负载均衡模式（"priority" 或 "balanced"）
    #[serde(default = "default_load_balancing_mode")]
    pub load_balancing_mode: String,

    /// 单凭据每分钟请求数上限（RPM）—— 阶段 7.6 引入
    ///
    /// 用于凭据级节流：限制单凭据请求频率，主动分流到其他可用凭据，
    /// 降低上游 429 概率。
    ///
    /// - `None` 或 `0`：使用内置自适应策略（默认 1-2 秒随机间隔 + 30% 抖动）
    /// - `>0`：固定请求间隔 = `60_000 / rpm` 毫秒（关闭抖动）
    ///
    /// 其他保护参数（每日上限 / 指数退避 / suspend 关键词识别）独立工作，
    /// 不受此字段影响。
    #[serde(default)]
    pub credential_rpm: Option<u32>,

    /// 单凭据每日成功请求数上限 —— 阶段 7.7 引入
    ///
    /// 模拟人类使用强度，避免单号被 Kiro 风控识别为自动化滥用。计数器
    /// 采用 24 小时滚动窗口，仅 `record_success` 累加（429 / 401 等失败不计）。
    /// 触顶后该凭据阻塞到下次重置，由选号逻辑切换到其他可用凭据。
    ///
    /// - `None` / `0`：使用内置默认 500（保守安全网）
    /// - `>0`：覆盖默认值；大号池 / 高频脚本可调高
    #[serde(default)]
    pub daily_max_requests: Option<u32>,

    /// 是否开启非流式响应的 thinking 块提取（默认 true）
    ///
    /// 启用后，非流式响应中的 `<thinking>...</thinking>` 标签会被解析为
    /// 独立的 `{"type": "thinking", ...}` 内容块,与流式响应行为一致。
    #[serde(default = "default_extract_thinking")]
    pub extract_thinking: bool,

    /// 默认端点名称（凭据未显式指定 endpoint 时使用，默认 "ide"）
    #[serde(default = "default_endpoint")]
    pub default_endpoint: String,

    /// 端点特定的配置
    ///
    /// 键为端点名（如 "ide" / "cli"），值为该端点自由定义的参数对象。
    /// 未在此表出现的端点沿用实现内置默认值。
    #[serde(default)]
    pub endpoints: HashMap<String, serde_json::Value>,

    /// 输入压缩与图片处理配置（阶段 3 起逐步接入）
    ///
    /// 字段 schema 一次性加好；当前阶段 3.1 只有图片处理生效，
    /// 压缩相关字段在阶段 3.2 接入压缩管道时启用。
    #[serde(default)]
    pub compression: CompressionConfig,

    /// Prompt Cache 追踪 TTL（秒），默认 300（5 分钟）
    ///
    /// 阶段 3.3 引入。cache_tracker 模块已就位但 caller 尚未接入；
    /// 阶段 5 admin 热加载或用户主动接入 cache 计费拆分时生效。
    #[serde(default = "default_prompt_cache_ttl_seconds")]
    pub prompt_cache_ttl_seconds: u64,

    /// 是否启用 Prompt Cache 计费追踪，默认 true
    #[serde(default = "default_true")]
    pub prompt_cache_accounting_enabled: bool,

    /// 配置文件路径（运行时元数据，不写入 JSON）
    #[serde(skip)]
    config_path: Option<PathBuf>,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_region() -> String {
    "us-east-1".to_string()
}

fn default_kiro_version() -> String {
    "0.11.107".to_string()
}

fn default_system_version() -> String {
    const SYSTEM_VERSIONS: &[&str] = &["darwin#24.6.0", "win32#10.0.22631"];
    SYSTEM_VERSIONS[fastrand::usize(..SYSTEM_VERSIONS.len())].to_string()
}

fn default_node_version() -> String {
    "22.22.0".to_string()
}

fn default_count_tokens_auth_type() -> String {
    "x-api-key".to_string()
}

fn default_tls_backend() -> TlsBackend {
    TlsBackend::Rustls
}

fn default_load_balancing_mode() -> String {
    "priority".to_string()
}

fn default_extract_thinking() -> bool {
    true
}

fn default_endpoint() -> String {
    crate::kiro::endpoint::ide::IDE_ENDPOINT_NAME.to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            region: default_region(),
            auth_region: None,
            api_region: None,
            kiro_version: default_kiro_version(),
            machine_id: None,
            api_key: None,
            system_version: default_system_version(),
            node_version: default_node_version(),
            tls_backend: default_tls_backend(),
            count_tokens_api_url: None,
            count_tokens_api_key: None,
            count_tokens_auth_type: default_count_tokens_auth_type(),
            proxy_url: None,
            proxy_username: None,
            proxy_password: None,
            admin_api_key: None,
            load_balancing_mode: default_load_balancing_mode(),
            credential_rpm: None,
            daily_max_requests: None,
            extract_thinking: default_extract_thinking(),
            default_endpoint: default_endpoint(),
            endpoints: HashMap::new(),
            compression: CompressionConfig::default(),
            prompt_cache_ttl_seconds: default_prompt_cache_ttl_seconds(),
            prompt_cache_accounting_enabled: true,
            config_path: None,
        }
    }
}

fn default_prompt_cache_ttl_seconds() -> u64 {
    300
}

/// 输入压缩与图片处理配置（从 feature/master 移植）
///
/// 阶段 3.1 仅图片处理相关字段（`image_max_*`、`image_multi_threshold`）生效；
/// 压缩相关字段为阶段 3.2 预留 schema，当前未接入 caller。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompressionConfig {
    /// 总开关，默认 true。当前仅图片处理路径检查此开关
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 空白压缩（连续空行、行尾空格），默认 true。阶段 3.2 接入
    #[serde(default = "default_true")]
    pub whitespace_compression: bool,
    /// thinking 块处理策略: "discard" | "truncate" | "keep"。阶段 3.2 接入
    #[serde(default = "default_thinking_strategy")]
    pub thinking_strategy: String,
    /// tool_result 截断阈值（字符数），默认 8000。阶段 3.2 接入
    #[serde(default = "default_8000")]
    pub tool_result_max_chars: usize,
    /// 智能截断保留头部行数，默认 80。阶段 3.2 接入
    #[serde(default = "default_80")]
    pub tool_result_head_lines: usize,
    /// 智能截断保留尾部行数，默认 40。阶段 3.2 接入
    #[serde(default = "default_40")]
    pub tool_result_tail_lines: usize,
    /// tool_use input 截断阈值（字符数），默认 6000。阶段 3.2 接入
    #[serde(default = "default_6000")]
    pub tool_use_input_max_chars: usize,
    /// 工具描述截断阈值（字符数），覆盖原 10000 硬编码，默认 4000。阶段 3.2 接入
    #[serde(default = "default_4000")]
    pub tool_description_max_chars: usize,
    /// 历史最大轮数，默认 80（0=不限）。阶段 3.2 接入
    #[serde(default = "default_80_turns")]
    pub max_history_turns: usize,
    /// 历史最大字符数，默认 400000（0=不限）。阶段 3.2 接入
    #[serde(default = "default_400k")]
    pub max_history_chars: usize,
    /// 图片长边最大像素，默认 4000（Anthropic 硬限制 8000，留安全余量）
    #[serde(default = "default_image_max_long_edge")]
    pub image_max_long_edge: u32,
    /// 单张图片最大总像素，默认 4_000_000（2000×2000）
    #[serde(default = "default_image_max_pixels_single")]
    pub image_max_pixels_single: u32,
    /// 多图模式下单张图片最大总像素，默认 4_000_000
    #[serde(default = "default_image_max_pixels_multi")]
    pub image_max_pixels_multi: u32,
    /// 触发多图限制的图片数量阈值，默认 20
    #[serde(default = "default_image_multi_threshold")]
    pub image_multi_threshold: usize,
    /// 请求体最大字节数，超过则直接拒绝（0 = 不限制）。阶段 3.2 接入
    #[serde(default = "default_max_request_body_bytes")]
    pub max_request_body_bytes: usize,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            whitespace_compression: true,
            thinking_strategy: default_thinking_strategy(),
            tool_result_max_chars: default_8000(),
            tool_result_head_lines: default_80(),
            tool_result_tail_lines: default_40(),
            tool_use_input_max_chars: default_6000(),
            tool_description_max_chars: default_4000(),
            max_history_turns: default_80_turns(),
            max_history_chars: default_400k(),
            image_max_long_edge: default_image_max_long_edge(),
            image_max_pixels_single: default_image_max_pixels_single(),
            image_max_pixels_multi: default_image_max_pixels_multi(),
            image_multi_threshold: default_image_multi_threshold(),
            max_request_body_bytes: default_max_request_body_bytes(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_thinking_strategy() -> String {
    "keep".to_string()
}

fn default_8000() -> usize {
    8000
}

fn default_80() -> usize {
    80
}

fn default_40() -> usize {
    40
}

fn default_6000() -> usize {
    6000
}

fn default_4000() -> usize {
    4000
}

fn default_80_turns() -> usize {
    80
}

fn default_400k() -> usize {
    400_000
}

fn default_image_max_long_edge() -> u32 {
    4000
}

fn default_image_max_pixels_single() -> u32 {
    4_000_000
}

fn default_image_max_pixels_multi() -> u32 {
    4_000_000
}

fn default_image_multi_threshold() -> usize {
    20
}

fn default_max_request_body_bytes() -> usize {
    0
}

impl Config {
    /// 获取默认配置文件路径
    pub fn default_config_path() -> &'static str {
        "config.json"
    }

    /// 获取有效的 Auth Region（用于 Token 刷新）
    /// 优先使用 auth_region，未配置时回退到 region
    pub fn effective_auth_region(&self) -> &str {
        self.auth_region.as_deref().unwrap_or(&self.region)
    }

    /// 获取有效的 API Region（用于 API 请求）
    /// 优先使用 api_region，未配置时回退到 region
    pub fn effective_api_region(&self) -> &str {
        self.api_region.as_deref().unwrap_or(&self.region)
    }

    /// 从文件加载配置
    pub fn load<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            // 配置文件不存在，返回默认配置
            let mut config = Self::default();
            config.config_path = Some(path.to_path_buf());
            return Ok(config);
        }

        let content = fs::read_to_string(path)?;
        let mut config: Config = serde_json::from_str(&content)?;
        config.config_path = Some(path.to_path_buf());
        Ok(config)
    }

    /// 获取配置文件路径（如果有）
    pub fn config_path(&self) -> Option<&Path> {
        self.config_path.as_deref()
    }

    /// 设置配置文件路径（用于 admin PUT /config 写盘场景）
    pub fn set_config_path(&mut self, path: PathBuf) {
        self.config_path = Some(path);
    }

    /// 将当前配置写回原始配置文件
    pub fn save(&self) -> anyhow::Result<()> {
        let path = self
            .config_path
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("配置文件路径未知，无法保存配置"))?;

        let content = serde_json::to_string_pretty(self).context("序列化配置失败")?;
        fs::write(path, content).with_context(|| format!("写入配置文件失败: {}", path.display()))?;
        Ok(())
    }
}
