//! Shared captured-output accounting for one Git resolution.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone)]
pub(crate) struct GitCapturedOutputBudget {
    pub(crate) ceiling: u64,
    pub(crate) observed: Arc<AtomicU64>,
}

impl GitCapturedOutputBudget {
    pub(crate) fn new(ceiling: u64) -> Self {
        Self {
            ceiling,
            observed: Arc::new(AtomicU64::new(0)),
        }
    }

    pub(crate) fn observed(&self) -> u64 {
        self.observed.load(Ordering::Acquire)
    }

    pub(crate) fn charge(&self, count: usize) -> Result<(), CapturedOutputLimitExceeded> {
        let count = u64::try_from(count).map_err(|_| CapturedOutputLimitExceeded {
            ceiling: self.ceiling,
            attempted: u64::MAX,
        })?;
        let mut current = self.observed();
        loop {
            let attempted = current
                .checked_add(count)
                .ok_or(CapturedOutputLimitExceeded {
                    ceiling: self.ceiling,
                    attempted: u64::MAX,
                })?;
            if attempted > self.ceiling {
                return Err(CapturedOutputLimitExceeded {
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
pub(crate) struct CapturedOutputLimitExceeded {
    pub(crate) ceiling: u64,
    pub(crate) attempted: u64,
}
