use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
struct Window {
    started_at: Instant,
    count: u32,
}

#[derive(Debug)]
pub struct RateLimiter {
    limit: u32,
    window: Duration,
    buckets: Mutex<HashMap<Uuid, Window>>,
}

impl RateLimiter {
    pub fn new(limit_per_minute: u32) -> Self {
        Self {
            limit: limit_per_minute,
            window: Duration::from_secs(60),
            buckets: Mutex::new(HashMap::new()),
        }
    }

    pub fn check(&self, user_id: Uuid) -> Result<(), u64> {
        if self.limit == 0 {
            return Ok(());
        }

        let now = Instant::now();
        let mut buckets = self.buckets.lock().expect("rate limiter mutex poisoned");
        if buckets.len() > 10_000 {
            buckets.retain(|_, window| now.duration_since(window.started_at) < self.window);
        }

        let entry = buckets.entry(user_id).or_insert(Window {
            started_at: now,
            count: 0,
        });

        if now.duration_since(entry.started_at) >= self.window {
            entry.started_at = now;
            entry.count = 0;
        }

        if entry.count >= self.limit {
            let elapsed = now.duration_since(entry.started_at);
            let retry_after = self.window.saturating_sub(elapsed).as_secs().max(1);
            return Err(retry_after);
        }

        entry.count += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_after_limit() {
        let limiter = RateLimiter::new(2);
        let user = Uuid::new_v4();
        assert!(limiter.check(user).is_ok());
        assert!(limiter.check(user).is_ok());
        assert!(limiter.check(user).is_err());
        assert!(limiter.check(Uuid::new_v4()).is_ok());
    }
}
