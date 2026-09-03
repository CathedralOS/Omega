use super::rows::ReviewOnlyCanonicalRow;
use crate::declarations::PackageKey;
use omega_build_evaluation::{BuildEvaluationUsage, BuildObservationSummary};
use omega_package_compilation::{
    AcceptedSemanticBinding, PackageGeneratedSourceBundle, PackageSourceConsumptionCommitment,
};
use omega_package_evidence::ledger::{
    OrdinaryPackageObligationLedger, OrdinaryPackageObligationResultSet,
};
use omega_package_evidence::record::{CheckedPackageReviewProjection, PackageReviewCanonicalRow};
use omega_package_source::ImmutableSourceResolution;
use std::path::PathBuf;

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
    pub(super) semantic_bindings: Vec<AcceptedSemanticBinding>,
    pub(super) semantic_binding_candidates: Vec<AcceptedSemanticBinding>,
    pub(super) generated_source_bundle: PackageGeneratedSourceBundle,
    pub(super) projection: CheckedPackageReviewProjection,
    pub(super) canonical_review_bytes: Vec<u8>,
    pub(super) canonical_rows: Vec<PackageReviewCanonicalRow>,
    pub(super) obligations: OrdinaryPackageObligationLedger,
    pub(super) obligation_results: OrdinaryPackageObligationResultSet,
    pub(super) comparison_rows: Vec<ReviewOnlyCanonicalRow>,
}

impl CompilerIssuedPackageReview {
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

    /// Exact consumer-policy bindings resolved by this package's checked
    /// compilation. These remain review provenance until root policy accepts
    /// every resulting blocking row.
    pub fn semantic_bindings(&self) -> &[AcceptedSemanticBinding] {
        &self.semantic_bindings
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

    pub fn canonical_review_bytes(&self) -> &[u8] {
        &self.canonical_review_bytes
    }

    pub fn canonical_rows(&self) -> &[PackageReviewCanonicalRow] {
        &self.canonical_rows
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

    pub(crate) fn comparison_rows(&self) -> &[ReviewOnlyCanonicalRow] {
        &self.comparison_rows
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

/// One frozen package-review result together with the exact checked root that
/// produced its final policy-bearing rows.
///
/// This carrier is deliberately non-clonable. It is not accepted evidence, a
/// package instance, or publication authority; admission may inspect the
/// review set before consuming the checked root into unpublished production.
#[derive(Debug)]
pub struct ReviewedPackageProductionCandidate {
    pub(super) reviews: CompilerIssuedPackageReviewSet,
    pub(super) root: PackageKey,
    pub(super) root_path: PathBuf,
    pub(super) root_role: omega_package_compilation::BuildDeclarationKind,
    pub(super) target_profile: omega_target::TargetProfile,
    pub(super) checked_root: omega_compiler::CheckedCompilation,
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

    pub fn review(&self, key: &PackageKey) -> Option<&CompilerIssuedPackageReview> {
        self.reviews.iter().find(|review| review.key() == key)
    }
}

impl ReviewedPackageProductionCandidate {
    pub const fn reviews(&self) -> &CompilerIssuedPackageReviewSet {
        &self.reviews
    }

    pub const fn root(&self) -> &PackageKey {
        &self.root
    }

    pub const fn root_role(&self) -> omega_package_compilation::BuildDeclarationKind {
        self.root_role
    }

    pub const fn target_profile(&self) -> omega_target::TargetProfile {
        self.target_profile
    }

    pub(crate) const fn checked_root(&self) -> &omega_compiler::CheckedCompilation {
        &self.checked_root
    }

    pub(crate) fn into_production_parts(
        self,
    ) -> (
        CompilerIssuedPackageReviewSet,
        PathBuf,
        omega_compiler::CheckedCompilation,
    ) {
        (self.reviews, self.root_path, self.checked_root)
    }
}
