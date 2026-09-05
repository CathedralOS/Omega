use super::PackagePolicyDecisionError as Error;

/// Hard-clamped aggregate limits. Requested storage includes obligation and
/// decision tables and output text; borrowed inputs and allocator overhead are
/// not counted. The scan ceiling includes packages, rows, and root-role change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackagePolicyDecisionLimits {
    pub maximum_bytes: usize,
    pub maximum_decisions: usize,
    pub maximum_owned_bytes: usize,
    pub maximum_changes: usize,
}
impl Default for PackagePolicyDecisionLimits {
    fn default() -> Self {
        Self {
            maximum_bytes: 8 * 1024 * 1024,
            maximum_decisions: 65_536,
            maximum_owned_bytes: 32 * 1024 * 1024,
            maximum_changes: 131_072,
        }
    }
}
pub(super) struct Budget {
    pub(super) limits: PackagePolicyDecisionLimits,
    owned: usize,
}
impl Budget {
    pub(super) fn new(limits: PackagePolicyDecisionLimits) -> Self {
        let hard = PackagePolicyDecisionLimits::default();
        Self {
            limits: PackagePolicyDecisionLimits {
                maximum_bytes: limits.maximum_bytes.min(hard.maximum_bytes),
                maximum_decisions: limits.maximum_decisions.min(hard.maximum_decisions),
                maximum_owned_bytes: limits.maximum_owned_bytes.min(hard.maximum_owned_bytes),
                maximum_changes: limits.maximum_changes.min(hard.maximum_changes),
            },
            owned: 0,
        }
    }
    pub(super) fn decisions(&self, count: usize) -> Result<(), Error> {
        if count > self.limits.maximum_decisions {
            Err(Error::DecisionLimitExceeded)
        } else {
            Ok(())
        }
    }
    pub(super) fn bytes(&self, count: usize) -> Result<(), Error> {
        if count > self.limits.maximum_bytes {
            Err(Error::ByteLimitExceeded)
        } else {
            Ok(())
        }
    }
    pub(super) fn owned(&mut self, count: usize) -> Result<(), Error> {
        self.owned = self.owned.checked_add(count).ok_or(Error::LengthOverflow)?;
        if self.owned > self.limits.maximum_owned_bytes {
            Err(Error::OwnedLimitExceeded)
        } else {
            Ok(())
        }
    }
    pub(super) fn vector<T>(&mut self, count: usize) -> Result<Vec<T>, Error> {
        self.owned(
            count
                .checked_mul(std::mem::size_of::<T>())
                .ok_or(Error::LengthOverflow)?,
        )?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(count)
            .map_err(|_| Error::AllocationFailed)?;
        Ok(result)
    }
}
