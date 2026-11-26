/*
    We define `CompilerIssuedPackageReview` and `CompilerIssuedPackageReviewSet`,
    retaining compiler-issued review evidence joined to resolver-owned source
    custody for each checked package occurrence in a resolved closure.

    There is deliberately no public constructor: source resolution, build
    handoff, normalized policy rows, and policy commitments (`encode_policy`)
    are joined only by candidate compilation.
*/

use crate::declarations::PackageKey;
use package_compilation::build_execution::{
    BuildEvaluationUsage, BuildObservationSummary, RestrictedBuildRequest,
};
use package_compilation::{PackageBuildHandoff, PackageSourceConsumptionCommitment};
use provider_acceptance::AcceptedSemanticBinding;
use review_evidence::record::{PackagePolicyBaseline, PackagePolicyRow};
use source_acquisition::ImmutableSourceResolution;

use review_evidence::record::SemanticBindingReviewCandidate;

use project_build::ReviewedPreparation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompilerIssuedPackageReview {
    pub(super) key: PackageKey,
    pub(super) resolution: ImmutableSourceResolution,
    pub(super) source_consumption_commitment: PackageSourceConsumptionCommitment,
    pub(super) selected_build_machine_identity: String,
    pub(super) preparations: Vec<ReviewedPreparation>,
    pub(super) build_evaluation_usage: Option<BuildEvaluationUsage>,
    pub(super) build_observation_summary: Option<BuildObservationSummary>,
    pub(super) restricted_build_requests: Vec<RestrictedBuildRequest>, // Normalized restricted build-host requests asked of the host before execution.
    pub(super) semantic_bindings: Vec<AcceptedSemanticBinding>,
    pub(super) semantic_binding_candidates: Vec<SemanticBindingReviewCandidate>,
    pub(super) build_handoff: PackageBuildHandoff,
    pub(super) policy: PackagePolicyBaseline,
    pub(super) policy_rows: Vec<PackagePolicyRow>, // Complete canonical rows derived once from `policy`.
    pub(super) policy_commitment: [u8; 32],        // SHA-256 over the policy's canonical bytes.
}

impl CompilerIssuedPackageReview {
    pub(crate) fn checked_context(&self) -> crate::lock::PackageCheckedContext {
        crate::lock::PackageCheckedContext::new(
            self.build_handoff.purpose(),
            self.build_handoff.target(),
            self.build_handoff.build_execution_profile(),
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

    pub fn preparations(&self) -> &[ReviewedPreparation] {
        &self.preparations
    }

    pub fn selected_build_machine_identity(&self) -> &str {
        &self.selected_build_machine_identity
    }

    pub const fn build_evaluation_usage(&self) -> Option<BuildEvaluationUsage> {
        self.build_evaluation_usage
    }

    pub const fn build_observation_summary(&self) -> Option<&BuildObservationSummary> {
        self.build_observation_summary.as_ref()
    }

    pub fn restricted_build_requests(&self) -> &[RestrictedBuildRequest] {
        &self.restricted_build_requests
    }

    pub fn semantic_bindings(&self) -> &[AcceptedSemanticBinding] {
        &self.semantic_bindings
    }

    pub const fn build_handoff(&self) -> &PackageBuildHandoff {
        &self.build_handoff
    }

    pub const fn policy(&self) -> &PackagePolicyBaseline {
        &self.policy
    }

    pub const fn policy_commitment(&self) -> [u8; 32] {
        self.policy_commitment
    }
}

pub(super) struct EncodedReviewPolicy {
    pub rows: Vec<PackagePolicyRow>,
    pub bytes: Vec<u8>,
    pub commitment: [u8; 32],
}

/// Derive the canonical rows, budget charge and commitment from one policy.
pub(super) fn encode_policy(
    policy: &PackagePolicyBaseline,
) -> Result<EncodedReviewPolicy, review_evidence::encoding::PackageReviewEncodingError> {
    let (rows, _) = policy
        .canonical_rows_with_limits(review_evidence::record::PackagePolicyRowLimits::default())?;
    let bytes = policy.canonical_bytes()?;
    let commitment = crate::digest_encoder::content_identity(&bytes);
    Ok(EncodedReviewPolicy {
        rows,
        bytes,
        commitment,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompilerIssuedPackageReviewSet {
    pub(super) reviews: Vec<CompilerIssuedPackageReview>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageSourceVerificationPhase {
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
