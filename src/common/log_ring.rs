//! 阶段 7.9：内存环形日志缓冲
//!
//! 一个统一的环形缓冲容纳两种来源：
//! 1. 通用 tracing 事件（通过 [`crate::common::tracing_layer::LogRingLayer`] 自动捕获）
//! 2. 模型/MCP 调用记录（由 provider.rs 显式 push）
//!
//! 容量满后挤出最旧条目，零磁盘 IO，重启丢失。容量可热更新。

use parking_lot::Mutex;
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};

/// 默认容量（约 25 MB 内存）
pub const DEFAULT_LOG_CAPACITY: usize = 50_000;

/// 日志条目分类
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LogKind {
    /// 来自 tracing layer 的通用事件
    Generic,
    /// provider.rs 显式记录的模型/MCP 调用
    ModelCall,
}

/// 模型调用元数据（仅 kind=ModelCall 时填充）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCallMeta {
    /// 凭据 ID
    pub credential_id: u64,
    /// 请求模型（MCP 路径可能为空）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// 端点名称（ide / cli）
    pub endpoint: String,
    /// API 类型：anthropic / count_tokens / mcp
    pub api_type: String,
    /// HTTP 状态码（成功=200；网络错误=0）
    pub status: u16,
    /// 端到端耗时（毫秒）
    pub duration_ms: u32,
    /// 第几次重试（0 = 首次）
    pub retry_attempt: u32,
    /// 是否流式响应
    pub is_stream: bool,
    /// 失败时的错误摘要（截断后）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_summary: Option<String>,
}

/// 统一日志条目
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    /// 单调递增的唯一 ID（buffer 启动起 0，递增；前端用作 React key + 展开状态键）
    pub seq: u64,
    /// Unix 毫秒时间戳
    pub timestamp: i64,
    /// 等级（"INFO" / "WARN" / "ERROR"；模型调用按 status 推断）
    pub level: String,
    /// 来源分类
    pub kind: LogKind,
    /// 模块路径（tracing target）
    pub target: String,
    /// 主消息
    pub message: String,
    /// 结构化字段（如 credential_id=5 reason=...）
    pub fields: HashMap<String, String>,
    /// 仅 kind=ModelCall 时填充
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_call: Option<ModelCallMeta>,
}

/// 查询过滤器
#[derive(Debug, Default, Clone)]
pub struct LogFilter {
    pub kind: Option<LogKind>,
    /// 等级过滤（["INFO", "WARN", "ERROR"]，None = 全部）
    pub levels: Option<Vec<String>>,
    pub q: Option<String>,
    pub credential_id: Option<u64>,
    pub model: Option<String>,
    /// 单个状态码或语义分组：success(<400) / error(>=400)
    pub status: Option<u16>,
    pub only_failed: bool,
    /// 增量查询：> since 的条目
    pub since: Option<i64>,
    pub limit: usize,
}

/// 环形日志缓冲
pub struct LogRing {
    entries: Mutex<VecDeque<LogEntry>>,
    /// 容量，调整后旧条目可能被立刻挤出
    capacity: Mutex<usize>,
    /// 单调递增的 seq 计数器（保证唯一 ID，前端展开状态键）
    next_seq: AtomicU64,
}

impl LogRing {
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.max(100);
        Self {
            entries: Mutex::new(VecDeque::with_capacity(cap)),
            capacity: Mutex::new(cap),
            next_seq: AtomicU64::new(0),
        }
    }

    /// 追加一条日志。容量满时挤出最旧。
    /// 自动分配 seq（调用方传入的 seq 字段会被覆盖）。
    pub fn push(&self, mut entry: LogEntry) {
        entry.seq = self.next_seq.fetch_add(1, Ordering::Relaxed);
        let cap = *self.capacity.lock();
        let mut buf = self.entries.lock();
        if buf.len() >= cap {
            buf.pop_front();
        }
        buf.push_back(entry);
    }

    /// 查询并返回过滤后的快照（按时间倒序，最新在前）
    pub fn query(&self, filter: &LogFilter) -> Vec<LogEntry> {
        let buf = self.entries.lock();
        let q_lower = filter.q.as_deref().map(str::to_ascii_lowercase);
        let mut out: Vec<LogEntry> = buf
            .iter()
            .rev() // 最新在前
            .filter(|e| match_filter(e, filter, q_lower.as_deref()))
            .take(filter.limit.max(1))
            .cloned()
            .collect();
        // 按 timestamp 降序保险（query take 已经倒序了，但显式排序更稳）
        out.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        out
    }

    /// 当前缓冲条数
    pub fn len(&self) -> usize {
        self.entries.lock().len()
    }

    pub fn capacity(&self) -> usize {
        *self.capacity.lock()
    }

    /// 清空
    pub fn clear(&self) {
        self.entries.lock().clear();
    }

    /// 热调整容量。变小时立刻挤出最旧。
    pub fn resize(&self, new_capacity: usize) {
        let cap = new_capacity.max(100);
        *self.capacity.lock() = cap;
        let mut buf = self.entries.lock();
        while buf.len() > cap {
            buf.pop_front();
        }
        // VecDeque 不会主动缩内存，这里也不收缩——容量很快会被新条目填回
    }

    /// 统计当前 buffer 内 ModelCall 的简要指标（用于前端实时统计条）
    pub fn model_call_stats(&self, window_ms: i64) -> ModelCallStats {
        let buf = self.entries.lock();
        let now = chrono::Utc::now().timestamp_millis();
        let cutoff = now - window_ms;

        let mut total = 0u32;
        let mut success = 0u32;
        let mut durations: Vec<u32> = Vec::new();

        for e in buf.iter().rev() {
            if e.timestamp < cutoff {
                break;
            }
            if let Some(mc) = &e.model_call {
                total += 1;
                if mc.status < 400 && mc.status > 0 {
                    success += 1;
                }
                durations.push(mc.duration_ms);
            }
        }

        let (avg_ms, p95_ms) = if durations.is_empty() {
            (0, 0)
        } else {
            let sum: u64 = durations.iter().map(|&v| v as u64).sum();
            let avg = (sum / durations.len() as u64) as u32;
            durations.sort_unstable();
            let idx = ((durations.len() as f64) * 0.95).floor() as usize;
            let p95 = durations[idx.min(durations.len() - 1)];
            (avg, p95)
        };

        ModelCallStats {
            window_ms,
            total,
            success,
            failed: total.saturating_sub(success),
            avg_ms,
            p95_ms,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCallStats {
    pub window_ms: i64,
    pub total: u32,
    pub success: u32,
    pub failed: u32,
    pub avg_ms: u32,
    pub p95_ms: u32,
}

fn match_filter(entry: &LogEntry, f: &LogFilter, q_lower: Option<&str>) -> bool {
    if let Some(kind) = f.kind {
        if entry.kind != kind {
            return false;
        }
    }
    if let Some(since) = f.since {
        if entry.timestamp <= since {
            return false;
        }
    }
    if let Some(levels) = &f.levels {
        if !levels.iter().any(|l| l.eq_ignore_ascii_case(&entry.level)) {
            return false;
        }
    }
    if let Some(q) = q_lower {
        let hay = format!("{} {} {}", entry.target, entry.message, entry.level).to_ascii_lowercase();
        if !hay.contains(q) {
            // 也检查 fields
            let in_fields = entry
                .fields
                .iter()
                .any(|(k, v)| k.to_ascii_lowercase().contains(q) || v.to_ascii_lowercase().contains(q));
            if !in_fields {
                return false;
            }
        }
    }
    if let Some(cid) = f.credential_id {
        let matches = entry
            .model_call
            .as_ref()
            .map(|mc| mc.credential_id == cid)
            .unwrap_or(false)
            || entry
                .fields
                .get("credential_id")
                .map(|v| v == &cid.to_string())
                .unwrap_or(false);
        if !matches {
            return false;
        }
    }
    if let Some(model) = &f.model {
        let matches = entry
            .model_call
            .as_ref()
            .and_then(|mc| mc.model.as_deref())
            .map(|m| m.contains(model))
            .unwrap_or(false);
        if !matches {
            return false;
        }
    }
    if let Some(status) = f.status {
        let matches = entry
            .model_call
            .as_ref()
            .map(|mc| mc.status == status)
            .unwrap_or(false);
        if !matches {
            return false;
        }
    }
    if f.only_failed {
        let is_failed = match &entry.model_call {
            Some(mc) => mc.status >= 400 || mc.status == 0,
            None => entry.level.eq_ignore_ascii_case("ERROR") || entry.level.eq_ignore_ascii_case("WARN"),
        };
        if !is_failed {
            return false;
        }
    }
    true
}
