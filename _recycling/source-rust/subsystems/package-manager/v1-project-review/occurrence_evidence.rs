//! Complete findings delivered to the candidate review pass with its project Build handoff.

use package_compilation::{
    BuildEvaluationUsage, BuildObservationSummary, PackageBuildHandoff,
    PackageSourceConsumptionCommitment, RestrictedBuildRequest,
};
use provider_acceptance::AcceptedSemanticBinding;
use review_evidence::record::{PackagePolicyBaseline, SemanticBindingReviewCandidate};

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
