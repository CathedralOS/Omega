use crate::declarations::PackageKey;
use build_evaluation::{BuildEvaluationUsage, BuildObservationSummary, RestrictedBuildRequest};
use package_compilation::{
    AcceptedSemanticBinding, PackageGeneratedSourceBundle, PackageSourceConsumptionCommitment,
};
use package_evidence::record::{PackagePolicyBaseline, PackagePolicyRow};
use package_source::ImmutableSourceResolution;

use package_evidence::record::SemanticBindingReviewCandidate;

/// Compiler-issued review material for one exact package source selection.
///
/// There is deliberately no public constructor. The source resolution and
/// review projection are joined only by compiling resolver-owned custody in
/// `compile_resolved_package_reviews`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompilerIssuedPackageReview {
    pub(super) key: PackageKey,
    pub(super) resolution: ImmutableSourceResolution,
    pub(super) source_consumption_commitment: PackageSourceConsumptionCommitment,
    pub(super) selected_build_machine_identity: String,
    pub(super) build_evaluation_usage: Option<BuildEvaluationUsage>,
    pub(super) build_observation_summary: Option<BuildObservationSummary>,
    /// The normalized restricted build-host requests this package's admitted
    /// build activation asked of the host before it executed. This is
    /// admission intent, not execution evidence, and remains review
    /// material — never an accepted grant.
    pub(super) restricted_build_requests: Vec<RestrictedBuildRequest>,
    pub(super) semantic_bindings: Vec<AcceptedSemanticBinding>,
    pub(super) semantic_binding_candidates: Vec<SemanticBindingReviewCandidate>,
    pub(super) generated_source_bundle: PackageGeneratedSourceBundle,
    pub(super) policy: PackagePolicyBaseline,
    /// The policy's complete canonical rows, derived once from `policy`.
    pub(super) policy_rows: Vec<PackagePolicyRow>,
    /// SHA-256 over the policy's canonical bytes.
    pub(super) policy_commitment: [u8; 32],
}

impl CompilerIssuedPackageReview {
    /// The checked activation that produced these findings. Equal package
    /// sources do not make build and product reviews interchangeable.
    pub(crate) fn checked_context(&self) -> crate::lock::PackageCheckedContext {
        crate::lock::PackageCheckedContext::new(
            self.generated_source_bundle.purpose(),
            self.generated_source_bundle.target(),
            self.generated_source_bundle.build_execution_profile(),
        )
    }

    pub fn key(&self) -> &PackageKey {
        &self.key
    }

    pub fn resolution(&self) -> &ImmutableSourceResolution {
        &self.resolution
    }

    pub const fn source_consumption_commitment(&self) -> PackageSourceConsumptionCommitment {
        self.source_consumption_commitment
    }

    /// Canonical semantic identity of the build machine whose evaluation
    /// produced this review.
    pub fn selected_build_machine_identity(&self) -> &str {
        &self.selected_build_machine_identity
    }

    /// Deterministic evaluator accounting from this package's checked build.
    /// This is not a CPU-time or process-memory receipt.
    pub const fn build_evaluation_usage(&self) -> Option<BuildEvaluationUsage> {
        self.build_evaluation_usage
    }

    /// Selected build-machine execution evidence. This is deliberately
    /// separate from canonical capability/API comparison bytes.
    pub const fn build_observation_summary(&self) -> Option<&BuildObservationSummary> {
        self.build_observation_summary.as_ref()
    }

    /// The restricted build-host requests this package's admitted build
    /// activation asked of the host before it executed. They name the
    /// restricted operation, logical resource roots, bounds, and activation
    /// profiles in compiler vocabulary — host paths and live grants never
    /// enter review material.
    pub fn restricted_build_requests(&self) -> &[RestrictedBuildRequest] {
        &self.restricted_build_requests
    }

    /// Exact consumer-policy bindings resolved by this package's checked
    /// compilation. These remain review provenance until root policy accepts
    /// every resulting blocking row.
    pub fn semantic_bindings(&self) -> &[AcceptedSemanticBinding] {
        &self.semantic_bindings
    }

    /// Compiler-issued, non-authoritative binding surfaces from the
    /// preliminary pass. Consumers may inspect their exact checked schemas to
    /// author policy for a final recompilation; the candidates themselves are
    /// neither accepted bindings nor permission grants.
    pub(crate) fn semantic_binding_candidates(&self) -> &[SemanticBindingReviewCandidate] {
        &self.semantic_binding_candidates
    }

    /// Exact explicit generated-source handoffs from the same checked run as
    /// this review. This is replay input for later dependency compilation, not
    /// an accepted source or package instance.
    pub const fn generated_source_bundle(&self) -> &PackageGeneratedSourceBundle {
        &self.generated_source_bundle
    }

    /// Complete normalized comparison meaning from this same checked package,
    /// target, and consumed source. This is not an accepted lock or a new
    /// authorization.
    pub const fn policy(&self) -> &PackagePolicyBaseline {
        &self.policy
    }

    pub const fn policy_commitment(&self) -> [u8; 32] {
        self.policy_commitment
    }
}

/// Derive a review's canonical policy rows and commitment once, at
/// construction. Fails when the policy exceeds the row budget.
pub(super) fn policy_rows_and_commitment(
    policy: &PackagePolicyBaseline,
) -> Result<(Vec<PackagePolicyRow>, [u8; 32]), package_evidence::encoding::PackageReviewEncodingError>
{
    let (rows, _) = policy
        .canonical_rows_with_limits(package_evidence::record::PackagePolicyRowLimits::default())?;
    let commitment = crate::digest_encoder::content_identity(&policy.canonical_bytes()?).into();
    Ok((rows, commitment))
}

/// Complete review-only compiler output for one resolved source closure.
///
/// Rows are dependency-first and deterministic. This remains review material,
/// not an accepted package instance, certificate, or lock payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompilerIssuedPackageReviewSet {
    pub(super) reviews: Vec<CompilerIssuedPackageReview>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PackageSourceVerificationPhase {
    BeforeCompilation,
    AfterCompilation,
}

impl CompilerIssuedPackageReviewSet {
    pub fn reviews(&self) -> &[CompilerIssuedPackageReview] {
        &self.reviews
    }

    pub(crate) fn reviews_for<'a>(
        &'a self,
        key: &'a PackageKey,
    ) -> impl Iterator<Item = &'a CompilerIssuedPackageReview> {
        self.reviews
            .iter()
            .filter(move |review| review.key() == key)
    }

    pub(crate) fn review_occurrence(
        &self,
        key: &PackageKey,
        purpose: crate::declarations::DependencyPurpose,
    ) -> Option<&CompilerIssuedPackageReview> {
        self.reviews
            .iter()
            .find(|review| review.key() == key && review.checked_context().purpose() == purpose)
    }
}
