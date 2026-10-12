//! Retained applications from the ordinary calling-compatibility producer.
//!
//! Equal validated plans do not collapse the two public requirement identities.
//! Authored policy applications retain their produced commitment; ordinary
//! native applications use the same canonical encoder at their original
//! evaluation point. This pair describes checked compatibility and grants no
//! receiving consent, conformance or runtime installation authority.

use effects::provider_plan::BoundaryCallingPlanCommitment;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssumedBindingCallingCompatibility {
    pub(super) required: BoundaryCallingPlanCommitment,
    pub(super) supplied: BoundaryCallingPlanCommitment,
}

impl AssumedBindingCallingCompatibility {
    pub const fn required(self) -> BoundaryCallingPlanCommitment {
        self.required
    }

    pub const fn supplied(self) -> BoundaryCallingPlanCommitment {
        self.supplied
    }
}
