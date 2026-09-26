//! Candidate-bound root-policy decisions for blocking review conflicts.

mod policy;
mod record;
mod resolution;

use crate::review::{
    ReviewOnlyCandidateClosureCommitment, ReviewOnlyCapabilityConflict,
    ReviewOnlyCapabilityConflictFingerprint, ReviewOnlyPackageCapabilityConflicts,
};
pub use policy::{
    PackagePolicyDecision, PackagePolicyDecisionError, PackagePolicyDecisionSubject,
    PackagePolicyResolution, PackagePolicyReviewError, recover_package_policy_review,
    render_package_policy_review, resolve_package_policy_decisions,
};
pub use record::{
    ReviewOnlyRootPolicyRecordError, ReviewOnlyRootPolicyRecordLimits,
    recover_review_only_root_policy_resolution,
};
pub use resolution::{
    ReviewOnlyRootPolicyResolution, ReviewOnlyRootPolicyResolutionCommitment,
    ReviewOnlyRootPolicyResolutionError, resolve_review_only_root_policy_decisions,
};

/// Root-project policy for one exact blocking candidate change.
///
/// This records only whether that candidate change is permitted. It is not a
/// prompt, reviewer identity, audit receipt, signature, or proof that anyone
/// inspected the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReviewOnlyRootPolicyDisposition {
    AcceptCandidateChange,
    RejectCandidateChange,
}

/// One candidate-bound root-policy decision.
///
/// Construction is available only through the package conflict that owns the
/// exact blocking fingerprint, preventing arbitrary strings or digests from
/// becoming policy decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReviewOnlyRootPolicyDecision {
    candidate_closure: ReviewOnlyCandidateClosureCommitment,
    conflict: ReviewOnlyCapabilityConflictFingerprint,
    disposition: ReviewOnlyRootPolicyDisposition,
}

impl ReviewOnlyRootPolicyDecision {
    pub const fn candidate_closure(&self) -> ReviewOnlyCandidateClosureCommitment {
        self.candidate_closure
    }

    pub const fn conflict(&self) -> ReviewOnlyCapabilityConflictFingerprint {
        self.conflict
    }

    pub const fn disposition(&self) -> ReviewOnlyRootPolicyDisposition {
        self.disposition
    }
}

impl ReviewOnlyPackageCapabilityConflicts {
    /// Bind root policy to one exact blocking conflict in this package.
    pub fn root_policy_decision(
        &self,
        conflict: &ReviewOnlyCapabilityConflict,
        disposition: ReviewOnlyRootPolicyDisposition,
    ) -> Result<ReviewOnlyRootPolicyDecision, ReviewOnlyRootPolicyResolutionError> {
        let Some(owned_conflict) = self
            .conflicts()
            .iter()
            .find(|candidate| candidate.fingerprint() == conflict.fingerprint())
        else {
            return Err(
                ReviewOnlyRootPolicyResolutionError::ConflictDoesNotBelongToPackage {
                    conflict: conflict.fingerprint(),
                },
            );
        };
        if !owned_conflict.is_blocking() {
            return Err(ReviewOnlyRootPolicyResolutionError::NonBlockingConflict {
                conflict: owned_conflict.fingerprint(),
            });
        }
        Ok(ReviewOnlyRootPolicyDecision {
            candidate_closure: self.candidate_closure(),
            conflict: owned_conflict.fingerprint(),
            disposition,
        })
    }
}
