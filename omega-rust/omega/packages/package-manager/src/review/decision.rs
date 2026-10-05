//! Root-policy decisions over package policy changes.

mod policy;

pub use policy::{
    PackagePolicyDecision, PackagePolicyDecisionError, PackagePolicyDecisionSubject,
    PackagePolicyResolution, PackagePolicyReviewError, recover_package_policy_review,
    render_package_policy_review, resolve_package_policy_decisions,
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
