use chrono::{DateTime, Utc};

pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

#[derive(Clone, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

pub struct MockClock {
    pub fixed_time: DateTime<Utc>,
}

impl MockClock {
    pub fn new(fixed_time: DateTime<Utc>) -> Self {
        Self { fixed_time }
    }
}

impl Clock for MockClock {
    fn now(&self) -> DateTime<Utc> {
        self.fixed_time
    }
}
