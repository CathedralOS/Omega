use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use super::model::invalid_input;

/// One compiler-owned aggregate byte budget shared by every endpoint route in
/// a source resolution. The counter covers tunneled bytes accepted by the
/// broker in both directions; CONNECT framing is excluded.
#[derive(Debug, Clone)]
pub struct ResolverExecutionTransferBudget {
    ceiling: u64,
    observed: Arc<AtomicU64>,
}

impl ResolverExecutionTransferBudget {
    pub fn new(ceiling: u64) -> io::Result<Self> {
        if ceiling == 0 {
            return Err(invalid_input("resolver transfer byte ceiling is zero"));
        }
        Ok(Self {
            ceiling,
            observed: Arc::new(AtomicU64::new(0)),
        })
    }

    pub const fn ceiling(&self) -> u64 {
        self.ceiling
    }

    pub fn observed(&self) -> u64 {
        self.observed.load(Ordering::Acquire)
    }

    pub(super) fn charge(&self, count: usize) -> Result<(), ()> {
        let count = u64::try_from(count).map_err(|_| ())?;
        self.observed
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current
                    .checked_add(count)
                    .filter(|attempted| *attempted <= self.ceiling)
            })
            .map(|_| ())
            .map_err(|_| ())
    }
}
