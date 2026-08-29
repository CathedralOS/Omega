use crate::identity::PackageKey;
use crate::review::records::ReviewOnlyCanonicalRow;
use omega_build_evaluation::BuildObservationSummary;
use omega_package_compilation::{PackageGeneratedSourceBundle, PackageSourceConsumptionCommitment};
use omega_package_review::evidence::{CheckedPackageReviewProjection, PackageReviewCanonicalRow};
use omega_package_review::obligation_ledger::OrdinaryPackageObligationLedger;
use omega_package_source::ImmutableSourceResolution;

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
    pub(super) build_observation_summary: Option<BuildObservationSummary>,
    pub(super) generated_source_bundle: PackageGeneratedSourceBundle,
    pub(super) projection: CheckedPackageReviewProjection,
    pub(super) canonical_review_bytes: Vec<u8>,
    pub(super) canonical_rows: Vec<PackageReviewCanonicalRow>,
    pub(super) obligation_ledger: OrdinaryPackageObligationLedger,
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

    /// Selected build-machine execution evidence. This is deliberately
    /// separate from canonical capability/API comparison bytes.
    pub const fn build_observation_summary(&self) -> Option<&BuildObservationSummary> {
        self.build_observation_summary.as_ref()
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
    pub const fn obligation_ledger(&self) -> &OrdinaryPackageObligationLedger {
        &self.obligation_ledger
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
