//! Source-free results of reviewing one checked package activation.

use super::{PackagePolicyBaseline, SemanticBindingReviewCandidate};
use package_compilation::{
    AcceptedSemanticBinding, BuildEvaluationUsage, BuildObservationSummary, ConsumedSourceUnit,
    PackageGeneratedSourceBundle, PackageSourceConsumptionCommitment, RestrictedBuildRequest,
};

/// Compiler findings, not accepted package policy. The package manager joins
/// these to resolver custody, scopes nominations, and obtains consumer decisions.
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
    pub generated_source_bundle: PackageGeneratedSourceBundle,
    pub policy: PackagePolicyBaseline,
    pub policy_canonical_bytes: usize,
}

/// Carry/cache facts from the same activation as the review. Consumed units
/// exist only for a package subject; absence cannot authorize cache publication.
#[derive(Debug)]
pub struct PackageReviewSourceFacts {
    pub consumed_units: Option<Vec<ConsumedSourceUnit>>,
}
