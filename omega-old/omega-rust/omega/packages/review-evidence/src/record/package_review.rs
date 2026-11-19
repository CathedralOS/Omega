// Source-free results of reviewing one checked package activation.
//
// `PackageReviewEvidence` carries compiler findings rather than accepted
// package policy; the package manager joins these to resolver custody, scopes
// nominations, and obtains consumer decisions. `PackageReviewSourceFacts`
// records carry/cache source units from the same activation; `None` cannot
// authorize cache publication.

use super::{PackagePolicyBaseline, SemanticBindingReviewCandidate};
use package_compilation::{
    AcceptedSemanticBinding, BuildEvaluationUsage, BuildObservationSummary, ConsumedSourceUnit,
    PackageBuildHandoff, PackageSourceConsumptionCommitment, RestrictedBuildRequest,
};

#[derive(Debug)]
pub struct PackageReviewEvidence {
    pub source_consumption_commitment: PackageSourceConsumptionCommitment,
    pub selected_build_machine_identity: String,
    pub build_evaluation_usage: Option<BuildEvaluationUsage>,
    pub build_observation_summary: Option<BuildObservationSummary>,
    pub restricted_build_requests: Vec<RestrictedBuildRequest>,
    pub semantic_bindings: Vec<AcceptedSemanticBinding>,
    pub service_binding_candidates: Vec<SemanticBindingReviewCandidate>,
    pub hosted_exit_binding_candidates: Vec<SemanticBindingReviewCandidate>,
    pub build_handoff: PackageBuildHandoff,
    pub policy: PackagePolicyBaseline,
    pub policy_canonical_bytes: usize,
}

#[derive(Debug)]
pub struct PackageReviewSourceFacts {
    pub consumed_units: Option<Vec<ConsumedSourceUnit>>,
}
