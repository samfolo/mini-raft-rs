use tokio::time;

pub struct TimeoutRange {
    min: time::Duration,
    max: time::Duration,
}

impl TimeoutRange {
    pub fn new(min: u64, max: u64) -> Self {
        Self {
            min: time::Duration::from_millis(min),
            max: time::Duration::from_millis(max),
        }
    }

    pub fn random(&self) -> time::Duration {
        rand::random_range(self.min..=self.max)
    }
}
