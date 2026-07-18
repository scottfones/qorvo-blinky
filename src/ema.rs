//! Exponential moving average.

/// Exponential moving average (EMA).
#[derive(Clone, Copy)]
pub struct Ema {
    /// Smoothed average.
    smoothed_avg: Option<i64>,
    /// EMA window size, 2^k (default: 3).
    window_exp: usize,
}

impl Default for Ema {
    fn default() -> Self {
        Self {
            smoothed_avg: None,
            window_exp: 3,
        }
    }
}

impl Ema {
    /// Add a new sample to the EMA and return its updated value.
    pub const fn sample(&mut self, sample: i64) -> i64 {
        let smoothed_avg = match self.smoothed_avg {
            Some(current) => current + sample - (current >> self.window_exp),
            None => sample << self.window_exp,
        };
        self.smoothed_avg = Some(smoothed_avg);
        smoothed_avg >> self.window_exp
    }
}
