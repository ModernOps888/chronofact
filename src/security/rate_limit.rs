use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone)]
pub struct RateLimiter {
    state: Arc<Mutex<RateLimiterState>>,
}

struct RateLimiterState {
    tokens: f64,
    max_tokens: f64,
    refill_rate_per_sec: f64,
    last_update: Instant,
}

impl RateLimiter {
    pub fn new(max_tokens: f64, refill_rate_per_sec: f64) -> Self {
        Self {
            state: Arc::new(Mutex::new(RateLimiterState {
                tokens: max_tokens,
                max_tokens,
                refill_rate_per_sec,
                last_update: Instant::now(),
            })),
        }
    }

    pub fn acquire(&self, amount: f64) -> bool {
        let mut state = self.state.lock().unwrap();
        let now = Instant::now();
        let elapsed = now.duration_since(state.last_update).as_secs_f64();
        state.last_update = now;

        // Refill tokens
        state.tokens = (state.tokens + elapsed * state.refill_rate_per_sec).min(state.max_tokens);

        if state.tokens >= amount {
            state.tokens -= amount;
            true
        } else {
            false
        }
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        // Default: 60 requests per minute with burst of 15
        Self::new(15.0, 1.0)
    }
}
