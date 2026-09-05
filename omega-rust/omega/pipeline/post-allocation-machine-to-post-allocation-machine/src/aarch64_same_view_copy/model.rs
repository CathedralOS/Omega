//! Admitted same-view-copy result and its representation-owned evidence.

use crate::ValidatedAarch64SameViewCopyElision;
use physical_instructions::Aarch64SameViewCopyElisionCustodyReceipt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedAarch64SameViewCopyElision {
    elision: ValidatedAarch64SameViewCopyElision,
    custody: Aarch64SameViewCopyElisionCustodyReceipt,
}

impl StagedOptimizedAarch64SameViewCopyElision {
    pub(super) const fn new(
        elision: ValidatedAarch64SameViewCopyElision,
        custody: Aarch64SameViewCopyElisionCustodyReceipt,
    ) -> Self {
        Self { elision, custody }
    }

    pub const fn elision(&self) -> &ValidatedAarch64SameViewCopyElision {
        &self.elision
    }

    pub const fn custody(&self) -> Aarch64SameViewCopyElisionCustodyReceipt {
        self.custody
    }
}
