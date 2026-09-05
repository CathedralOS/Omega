//! Monotone owned-storage accounting shared by all source recovery phases.

use super::CanonicalSourceClosureSubjectError as Error;

/// Owned payload/storage charged during recovery, including discarded scratch.
/// This is resource accounting only, not a source or acceptance assertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CanonicalSourceClosureSubjectRecoveryUsage {
    pub(super) owned_bytes: usize,
    pub(super) packages: usize,
    pub(super) authored_dependency_requests: usize,
    pub(super) dependency_requests: usize,
}

impl CanonicalSourceClosureSubjectRecoveryUsage {
    pub const fn owned_bytes(&self) -> usize {
        self.owned_bytes
    }
    pub const fn packages(&self) -> usize {
        self.packages
    }
    pub const fn authored_dependency_requests(&self) -> usize {
        self.authored_dependency_requests
    }
    pub const fn dependency_requests(&self) -> usize {
        self.dependency_requests
    }
}

pub(super) struct Budget {
    maximum: usize,
    pub(super) usage: CanonicalSourceClosureSubjectRecoveryUsage,
}

impl Budget {
    pub(super) fn new(maximum: usize) -> Self {
        Self {
            maximum,
            usage: CanonicalSourceClosureSubjectRecoveryUsage::default(),
        }
    }

    pub(super) fn charge(&mut self, bytes: usize) -> Result<(), Error> {
        self.usage.owned_bytes = self
            .usage
            .owned_bytes
            .checked_add(bytes)
            .filter(|total| *total <= self.maximum)
            .ok_or_else(|| {
                Error::allocation_limit("source-closure recovery exceeds its owned-byte limit")
            })?;
        Ok(())
    }

    pub(super) fn reserve<T>(&mut self, count: usize) -> Result<Vec<T>, Error> {
        self.charge(count.checked_mul(std::mem::size_of::<T>()).ok_or_else(|| {
            Error::allocation_limit("source-closure recovery allocation size overflow")
        })?)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| Error::new("source-closure recovery allocation failed"))?;
        Ok(values)
    }

    pub(super) fn copy_string(&mut self, value: &str) -> Result<String, Error> {
        let mut bytes = self.reserve::<u8>(value.len())?;
        bytes.extend_from_slice(value.as_bytes());
        Ok(String::from_utf8(bytes).expect("copied UTF-8"))
    }
}
