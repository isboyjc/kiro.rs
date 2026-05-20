//! 全局 429 背压熔断（Phase C）
//!
//! 当上游频繁返回 429（平台级"high traffic"过载信号）时，让**所有账号的新请求**短暂
//! 统一暂停，给上游喘息、避免 53 个号一起重试形成风暴。已在飞的请求不受影响
//! （它们已持有并发槽 / 已过 acquire 阶段）。
//!
//! 分级退避（窗口内 429 次数越多，暂停越久）：
//!   - 第 1 次       → level1（默认 5s）
//!   - 窗口内第 2 次 → level2（默认 15s）
//!   - 窗口内第 3 次+→ level3 随机区间（默认 30~60s）
//!   - 距上次 429 超过 window → 等级重置为 0
//!
//! `enabled = false` 时本模块完全不介入（默认关闭，确认 429 为平台级后再开）。

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use parking_lot::Mutex;

struct State {
    /// 当前升级等级（0 = 无 429 / 已重置）
    level: u32,
    /// 上次记录 429 的时刻
    last_429_at: Option<Instant>,
    /// 全局暂停截止时刻
    paused_until: Option<Instant>,
}

/// 全局 429 背压熔断器
pub struct GlobalCooldown {
    state: Mutex<State>,
    enabled: AtomicBool,
    window_ms: AtomicU64,
    level1_ms: AtomicU64,
    level2_ms: AtomicU64,
    level3_min_ms: AtomicU64,
    level3_max_ms: AtomicU64,
}

impl GlobalCooldown {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        enabled: bool,
        window_ms: u64,
        level1_ms: u64,
        level2_ms: u64,
        level3_min_ms: u64,
        level3_max_ms: u64,
    ) -> Self {
        Self {
            state: Mutex::new(State {
                level: 0,
                last_429_at: None,
                paused_until: None,
            }),
            enabled: AtomicBool::new(enabled),
            window_ms: AtomicU64::new(window_ms),
            level1_ms: AtomicU64::new(level1_ms),
            level2_ms: AtomicU64::new(level2_ms),
            level3_min_ms: AtomicU64::new(level3_min_ms),
            level3_max_ms: AtomicU64::new(level3_max_ms.max(level3_min_ms)),
        }
    }

    /// 热更新参数
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &self,
        enabled: bool,
        window_ms: u64,
        level1_ms: u64,
        level2_ms: u64,
        level3_min_ms: u64,
        level3_max_ms: u64,
    ) {
        self.enabled.store(enabled, Ordering::Relaxed);
        self.window_ms.store(window_ms, Ordering::Relaxed);
        self.level1_ms.store(level1_ms, Ordering::Relaxed);
        self.level2_ms.store(level2_ms, Ordering::Relaxed);
        self.level3_min_ms.store(level3_min_ms, Ordering::Relaxed);
        self.level3_max_ms
            .store(level3_max_ms.max(level3_min_ms), Ordering::Relaxed);
    }

    /// 记录一次上游 429：按分级策略升级并设置全局暂停。返回本次暂停时长（关闭时返回 0）。
    pub fn record_429(&self) -> Duration {
        if !self.enabled.load(Ordering::Relaxed) {
            return Duration::ZERO;
        }
        let window = Duration::from_millis(self.window_ms.load(Ordering::Relaxed));
        let now = Instant::now();

        let mut st = self.state.lock();
        // 距上次 429 超过窗口 → 重置等级
        if let Some(last) = st.last_429_at
            && now.duration_since(last) > window
        {
            st.level = 0;
        }
        st.level = st.level.saturating_add(1);
        st.last_429_at = Some(now);

        let pause_ms = match st.level {
            1 => self.level1_ms.load(Ordering::Relaxed),
            2 => self.level2_ms.load(Ordering::Relaxed),
            _ => {
                let lo = self.level3_min_ms.load(Ordering::Relaxed);
                let hi = self.level3_max_ms.load(Ordering::Relaxed).max(lo);
                if hi > lo {
                    lo + fastrand::u64(0..=(hi - lo))
                } else {
                    lo
                }
            }
        };
        let pause = Duration::from_millis(pause_ms);
        // 取较晚者，避免并发多次 429 把暂停缩短
        let new_until = now + pause;
        st.paused_until = Some(match st.paused_until {
            Some(existing) if existing > new_until => existing,
            _ => new_until,
        });
        pause
    }

    /// 当前是否处于全局暂停；返回剩余时长（None = 未暂停 / 已关闭）。
    pub fn paused_remaining(&self) -> Option<Duration> {
        if !self.enabled.load(Ordering::Relaxed) {
            return None;
        }
        let now = Instant::now();
        let st = self.state.lock();
        match st.paused_until {
            Some(until) if until > now => Some(until.duration_since(now)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk() -> GlobalCooldown {
        // 用很短的毫秒值便于测试
        GlobalCooldown::new(true, 10_000, 50, 100, 200, 200)
    }

    #[test]
    fn disabled_is_noop() {
        let gc = GlobalCooldown::new(false, 10_000, 50, 100, 200, 300);
        assert_eq!(gc.record_429(), Duration::ZERO);
        assert!(gc.paused_remaining().is_none());
    }

    #[test]
    fn escalates_through_levels() {
        let gc = mk();
        assert_eq!(gc.record_429().as_millis(), 50); // level 1
        assert_eq!(gc.record_429().as_millis(), 100); // level 2
        let l3 = gc.record_429().as_millis(); // level 3（区间，这里 min==max=200）
        assert_eq!(l3, 200);
        assert!(gc.paused_remaining().is_some());
    }

    #[test]
    fn resets_after_window() {
        // window 很短：sleep 后再 429 应回到 level 1
        let gc = GlobalCooldown::new(true, 5, 50, 100, 200, 200);
        gc.record_429();
        std::thread::sleep(Duration::from_millis(15));
        assert_eq!(gc.record_429().as_millis(), 50, "超窗口应重置为 level 1");
    }
}
