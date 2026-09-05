use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone)]
pub struct BoundedCaptureBudget {
    ceiling: u64,
    observed: Arc<AtomicU64>,
}

impl BoundedCaptureBudget {
    pub fn new(ceiling: u64) -> Self {
        Self {
            ceiling,
            observed: Arc::new(AtomicU64::new(0)),
        }
    }

    pub const fn ceiling(&self) -> u64 {
        self.ceiling
    }

    pub fn observed(&self) -> u64 {
        self.observed.load(Ordering::Acquire)
    }

    pub(crate) fn charge(&self, count: usize) -> Result<(), BoundedCaptureBudgetExceeded> {
        let count = u64::try_from(count).map_err(|_| BoundedCaptureBudgetExceeded {
            ceiling: self.ceiling,
            attempted: u64::MAX,
        })?;
        let mut current = self.observed();
        loop {
            let attempted = current
                .checked_add(count)
                .ok_or(BoundedCaptureBudgetExceeded {
                    ceiling: self.ceiling,
                    attempted: u64::MAX,
                })?;
            if attempted > self.ceiling {
                return Err(BoundedCaptureBudgetExceeded {
                    ceiling: self.ceiling,
                    attempted,
                });
            }
            match self.observed.compare_exchange_weak(
                current,
                attempted,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(()),
                Err(actual) => current = actual,
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundedCaptureBudgetExceeded {
    ceiling: u64,
    attempted: u64,
}

impl BoundedCaptureBudgetExceeded {
    pub const fn ceiling(self) -> u64 {
        self.ceiling
    }

    pub const fn attempted(self) -> u64 {
        self.attempted
    }
}
