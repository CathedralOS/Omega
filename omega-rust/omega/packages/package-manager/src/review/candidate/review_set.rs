use super::rows::RetainedReviewRows;
use crate::declarations::PackageKey;
use build_evaluation::{BuildEvaluationUsage, BuildObservationSummary, RestrictedBuildRequest};
use package_compilation::{
    AcceptedSemanticBinding, PackageGeneratedSourceBundle, PackageSourceConsumptionCommitment,
};
use package_evidence::ledger::{
    OrdinaryPackageObligationLedger, OrdinaryPackageObligationResultSet,
};
use package_evidence::record::{CheckedPackageReviewProjection, PackageReviewCanonicalRow};
use package_source::ImmutableSourceResolution;

use super::semantic_bindings::SemanticBindingReviewCandidate;

/// Compiler-issued review material for one exact package source selection.
///
/// There is deliberately no public constructor. The source resolution and
/// review projection are joined only by compiling resolver-owned custody in
/// `compile_resolved_package_reviews`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerIssuedPackageReview {
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
    pub(super) projection: CheckedPackageReviewProjection,
    pub(super) policy: package_evidence::record::PackagePolicyBaseline,
    pub(super) canonical_review_bytes: Vec<u8>,
    pub(super) canonical_rows: RetainedReviewRows,
    pub(super) obligations: OrdinaryPackageObligationLedger,
    pub(super) obligation_results: OrdinaryPackageObligationResultSet,
}

impl CompilerIssuedPackageReview {
    /// The checked activation that produced these findings. Equal package
    /// sources do not make build and product reviews interchangeable.
    pub fn checked_context(&self) -> crate::lock::PackageCheckedContext {
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
    pub fn semantic_binding_candidates(&self) -> &[SemanticBindingReviewCandidate] {
        &self.semantic_binding_candidates
    }

    /// Exact explicit generated-source handoffs from the same checked run as
    /// this review. This is replay input for later dependency compilation, not
    /// an accepted source or package instance.
    pub const fn generated_source_bundle(&self) -> &PackageGeneratedSourceBundle {
        &self.generated_source_bundle
    }

    pub fn projection(&self) -> &CheckedPackageReviewProjection {
        &self.projection
    }

    /// Complete normalized comparison meaning from this same checked package,
    /// target, and consumed source. This is not an accepted lock or a new
    /// authorization; legacy comparison and obligation evidence remain separate.
    pub const fn policy(&self) -> &package_evidence::record::PackagePolicyBaseline {
        &self.policy
    }

    pub fn canonical_review_bytes(&self) -> &[u8] {
        &self.canonical_review_bytes
    }

    pub fn canonical_rows(&self) -> &[PackageReviewCanonicalRow] {
        &self.canonical_rows.0
    }

    /// Exact schema-bound replay question reconstructed from this package's
    /// checked source. It remains compiler-issued review material, not a
    /// discharge result, admission decision, package instance, or lock row.
    pub const fn obligations(&self) -> &OrdinaryPackageObligationLedger {
        &self.obligations
    }

    /// Locally reconstructed result for the supported ordinary obligation
    /// lanes. Accepted claims, dangerous authorities, and external executable
    /// supplies remain explicitly open; this is not admission.
    pub const fn obligation_results(&self) -> &OrdinaryPackageObligationResultSet {
        &self.obligation_results
    }
}

/// Complete review-only compiler output for one resolved source closure.
///
/// Rows are dependency-first and deterministic. This remains review material,
/// not an accepted package instance, certificate, or lock payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerIssuedPackageReviewSet {
    pub(super) reviews: Vec<CompilerIssuedPackageReview>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageSourceVerificationPhase {
    BeforeCompilation,
    AfterCompilation,
}

impl CompilerIssuedPackageReviewSet {
    pub fn reviews(&self) -> &[CompilerIssuedPackageReview] {
        &self.reviews
    }

    /// Return the package's review only when it has one checked occurrence.
    /// Callers handling both roles must select a purpose or visit every review.
    pub fn review(&self, key: &PackageKey) -> Option<&CompilerIssuedPackageReview> {
        let mut matching = self.reviews.iter().filter(|review| review.key() == key);
        let review = matching.next()?;
        matching.next().is_none().then_some(review)
    }

    pub fn reviews_for<'a>(
        &'a self,
        key: &'a PackageKey,
    ) -> impl Iterator<Item = &'a CompilerIssuedPackageReview> {
        self.reviews
            .iter()
            .filter(move |review| review.key() == key)
    }

    pub fn review_occurrence(
        &self,
        key: &PackageKey,
        purpose: crate::declarations::DependencyPurpose,
    ) -> Option<&CompilerIssuedPackageReview> {
        self.reviews
            .iter()
            .find(|review| review.key() == key && review.checked_context().purpose() == purpose)
    }
}
