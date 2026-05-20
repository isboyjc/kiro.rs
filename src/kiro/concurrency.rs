//! 单凭据并发控制（Phase B）
//!
//! AI 流式响应耗时长，RPM/TPS 无法准确反映负载——真正约束是「同一时刻在飞的请求数」。
//! 本模块用令牌桶语义限制单凭据并发：选号时拿令牌（in-flight +1），请求/流结束归还
//! （RAII `InFlightGuard` 在 Drop 时 -1，覆盖正常结束 / 客户端断连 / panic 退栈）。
//!
//! `max == 0` 表示不限并发。

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use parking_lot::Mutex;

/// 单凭据并发限制器
pub struct ConcurrencyLimiter {
    /// 各凭据当前在飞请求数
    in_flight: Mutex<HashMap<u64, u32>>,
    /// 单凭据最大在飞数（0 = 不限）
    max_per_credential: AtomicU32,
}

impl ConcurrencyLimiter {
    pub fn new(max_per_credential: u32) -> Arc<Self> {
        Arc::new(Self {
            in_flight: Mutex::new(HashMap::new()),
            max_per_credential: AtomicU32::new(max_per_credential),
        })
    }

    /// 热更新单凭据最大并发（0 = 不限）
    pub fn set_max(&self, max_per_credential: u32) {
        self.max_per_credential
            .store(max_per_credential, Ordering::Relaxed);
    }

    /// 该凭据当前在飞请求数
    pub fn in_flight(&self, id: u64) -> u32 {
        self.in_flight.lock().get(&id).copied().unwrap_or(0)
    }

    /// 该凭据是否已达并发上限（选号时用于过滤）
    pub fn is_full(&self, id: u64) -> bool {
        let max = self.max_per_credential.load(Ordering::Relaxed);
        if max == 0 {
            return false;
        }
        self.in_flight.lock().get(&id).copied().unwrap_or(0) >= max
    }

    /// 尝试占用一个并发槽：未满则 in-flight +1 并返回 RAII guard；已满返回 None。
    /// 原子（同一把锁内检查 + 占用），并发安全。
    pub fn try_acquire(self: &Arc<Self>, id: u64) -> Option<InFlightGuard> {
        let max = self.max_per_credential.load(Ordering::Relaxed);
        let mut map = self.in_flight.lock();
        let count = map.entry(id).or_insert(0);
        if max != 0 && *count >= max {
            return None;
        }
        *count += 1;
        Some(InFlightGuard {
            limiter: Arc::clone(self),
            id,
        })
    }

    /// 归还一个并发槽（由 guard Drop 调用）
    fn release(&self, id: u64) {
        let mut map = self.in_flight.lock();
        if let Some(count) = map.get_mut(&id) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                map.remove(&id);
            }
        }
    }
}

/// 在飞请求的 RAII 凭证：存活期间占用一个并发槽，Drop 时归还。
///
/// 必须随请求生命周期流转：非流式请求持有到响应构建完成；流式请求移入
/// `StreamContext`，随 SSE 流结束 / 客户端断连而 Drop。
pub struct InFlightGuard {
    limiter: Arc<ConcurrencyLimiter>,
    id: u64,
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.limiter.release(self.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acquire_release_roundtrip() {
        let lim = ConcurrencyLimiter::new(2);
        assert_eq!(lim.in_flight(1), 0);
        let g1 = lim.try_acquire(1).unwrap();
        assert_eq!(lim.in_flight(1), 1);
        let g2 = lim.try_acquire(1).unwrap();
        assert_eq!(lim.in_flight(1), 2);
        assert!(lim.is_full(1));
        // 已满，第三次拿不到
        assert!(lim.try_acquire(1).is_none());
        drop(g1);
        assert_eq!(lim.in_flight(1), 1);
        assert!(!lim.is_full(1));
        // 释放后又能拿
        let g3 = lim.try_acquire(1).unwrap();
        assert_eq!(lim.in_flight(1), 2);
        drop(g2);
        drop(g3);
        assert_eq!(lim.in_flight(1), 0);
    }

    #[test]
    fn zero_means_unlimited() {
        let lim = ConcurrencyLimiter::new(0);
        let mut guards = Vec::new();
        for _ in 0..100 {
            guards.push(lim.try_acquire(7).unwrap());
        }
        assert_eq!(lim.in_flight(7), 100);
        assert!(!lim.is_full(7));
    }

    #[test]
    fn hot_update_max() {
        let lim = ConcurrencyLimiter::new(1);
        let _g = lim.try_acquire(1).unwrap();
        assert!(lim.is_full(1));
        lim.set_max(3);
        assert!(!lim.is_full(1));
        assert!(lim.try_acquire(1).is_some());
    }

    #[test]
    fn isolated_per_credential() {
        let lim = ConcurrencyLimiter::new(1);
        let _g1 = lim.try_acquire(1).unwrap();
        // 不同凭据互不影响
        let _g2 = lim.try_acquire(2).unwrap();
        assert_eq!(lim.in_flight(1), 1);
        assert_eq!(lim.in_flight(2), 1);
    }
}
