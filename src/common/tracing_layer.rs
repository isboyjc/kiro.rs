//! 阶段 7.9：tracing 自定义 Layer，把所有 INFO/WARN/ERROR 事件镜像到 LogRing
//!
//! 与 `tracing_subscriber::fmt::layer()` 并存——stdout 输出不受影响。

use chrono::Utc;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::Arc;
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

use crate::common::log_ring::{LogEntry, LogKind, LogRing};

pub struct LogRingLayer {
    ring: Arc<LogRing>,
}

impl LogRingLayer {
    pub fn new(ring: Arc<LogRing>) -> Self {
        Self { ring }
    }
}

impl<S> Layer<S> for LogRingLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        // 仅捕获 INFO 及以上等级（避免高频 DEBUG/TRACE 撑爆 buffer）
        let level_str = meta.level().to_string();
        if level_str == "TRACE" || level_str == "DEBUG" {
            return;
        }

        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);

        let entry = LogEntry {
            timestamp: Utc::now().timestamp_millis(),
            level: level_str,
            kind: LogKind::Generic,
            target: meta.target().to_string(),
            message: visitor.message.unwrap_or_default(),
            fields: visitor.fields,
            model_call: None,
        };
        self.ring.push(entry);
    }
}

/// 提取事件字段：`message` 单独拿，其余进 HashMap。
#[derive(Default)]
struct FieldVisitor {
    message: Option<String>,
    fields: HashMap<String, String>,
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let name = field.name();
        let formatted = format!("{:?}", value);
        if name == "message" {
            self.message = Some(strip_quotes(&formatted));
        } else {
            self.fields.insert(name.to_string(), strip_quotes(&formatted));
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        let name = field.name();
        if name == "message" {
            self.message = Some(value.to_string());
        } else {
            self.fields.insert(name.to_string(), value.to_string());
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.write_named(field.name(), value);
    }
    fn record_u64(&mut self, field: &Field, value: u64) {
        self.write_named(field.name(), value);
    }
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.write_named(field.name(), value);
    }
    fn record_f64(&mut self, field: &Field, value: f64) {
        self.write_named(field.name(), value);
    }
}

impl FieldVisitor {
    fn write_named<T: std::fmt::Display>(&mut self, name: &str, value: T) {
        let mut s = String::new();
        let _ = write!(s, "{}", value);
        if name == "message" {
            self.message = Some(s);
        } else {
            self.fields.insert(name.to_string(), s);
        }
    }
}

/// tracing Debug 格式输出字符串时会带引号 `"xxx"`，去掉一层。
fn strip_quotes(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}
