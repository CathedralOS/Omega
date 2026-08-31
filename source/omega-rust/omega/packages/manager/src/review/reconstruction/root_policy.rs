//! Exact in-memory association of fresh package obligations with root policy.

use super::{
    CanonicalPackageReconstructionQuestionError, CanonicalPackageReconstructionQuestionLimits,
    LocallyComposedPackageObligationResults,
};
use crate::resolution::graph::ResolvedPackageSourceClosure;
use crate::review::{
    CompilerIssuedPackageReviewSet, ReviewOnlyCapabilityConflictChange,
    ReviewOnlyCapabilityConflictError, ReviewOnlyCapabilityConflictLimits,
    ReviewOnlyCapabilityConflictSet, ReviewOnlyRootPolicyResolution,
    ReviewOnlyRootPolicyResolutionError, compare_review_only_initial_capabilities,
    resolve_review_only_root_policy_decisions,
};
use omega_package_evidence::record::{
    PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk,
};
use std::fmt;

/// Freshly reconstructed obligations whose exact blocking review rows have
/// been accepted by root policy.
///
/// This is only an in-memory policy association. It is not complete package
/// evidence, an accepted lock row, a `PackageInstance`, or permission to
/// mutate project files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FreshPackageRootPolicyAcceptance {
    obligations: LocallyComposedPackageObligationResults,
    conflicts: ReviewOnlyCapabilityConflictSet,
    root_policy: Option<ReviewOnlyRootPolicyResolution>,
}

impl FreshPackageRootPolicyAcceptance {
    pub const fn obligations(&self) -> &LocallyComposedPackageObligationResults {
        &self.obligations
    }

    pub const fn conflicts(&self) -> &ReviewOnlyCapabilityConflictSet {
        &self.conflicts
    }

    /// Complete policy only when the fresh candidate has blocking rows.
    /// Audit recommendations alone do not manufacture a policy record.
    pub const fn root_policy(&self) -> Option<&ReviewOnlyRootPolicyResolution> {
        self.root_policy.as_ref()
    }
}

#[derive(Debug)]
pub enum FreshPackageRootPolicyError {
    Reconstruction(CanonicalPackageReconstructionQuestionError),
    ConflictComparison(ReviewOnlyCapabilityConflictError),
    MissingRootPolicy,
    UnexpectedRootPolicy,
    InvalidRootPolicy(ReviewOnlyRootPolicyResolutionError),
    RootPolicyReplayMismatch,
    RejectedBlockingConflict,
    AcceptedClaimConflictShapeMismatch,
    AcceptedClaimConflictSetMismatch,
    AllocationFailed,
}

impl fmt::Display for FreshPackageRootPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reconstruction(error) => write!(
                formatter,
                "fresh package root-policy reconstruction failed: {error}"
            ),
            Self::ConflictComparison(error) => write!(
                formatter,
                "fresh package root-policy conflict comparison failed: {error}"
            ),
            Self::MissingRootPolicy => formatter.write_str(
                "fresh package candidate has blocking rows but no complete root policy",
            ),
            Self::UnexpectedRootPolicy => formatter.write_str(
                "fresh package candidate has no blocking rows but was given root policy",
            ),
            Self::InvalidRootPolicy(error) => {
                write!(formatter, "fresh package root policy is invalid: {error}")
            }
            Self::RootPolicyReplayMismatch => formatter.write_str(
                "fresh package root policy differs from its canonical replay",
            ),
            Self::RejectedBlockingConflict => formatter.write_str(
                "fresh package root policy rejects at least one exact blocking row",
            ),
            Self::AcceptedClaimConflictShapeMismatch => formatter.write_str(
                "fresh accepted-claim conflict is not an added blocking row against the empty admission baseline",
            ),
            Self::AcceptedClaimConflictSetMismatch => formatter.write_str(
                "fresh accepted-claim conflicts are not bijective with reconstructed open claims",
            ),
            Self::AllocationFailed => formatter.write_str(
                "fresh package root-policy association allocation failed",
            ),
        }
    }
}

impl std::error::Error for FreshPackageRootPolicyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Reconstruction(error) => Some(error),
            Self::ConflictComparison(error) => Some(error),
            Self::InvalidRootPolicy(error) => Some(error),
            Self::MissingRootPolicy
            | Self::UnexpectedRootPolicy
            | Self::RootPolicyReplayMismatch
            | Self::RejectedBlockingConflict
            | Self::AcceptedClaimConflictShapeMismatch
            | Self::AcceptedClaimConflictSetMismatch
            | Self::AllocationFailed => None,
        }
    }
}

/// Reconstruct a fresh candidate and bind every exact blocking review row to
/// the root policy that resolved that same candidate.
///
/// The conflict set is deliberately rederived here. A caller cannot pair
/// obligations from one source closure with decisions displayed for another.
pub fn bind_fresh_package_root_policy(
    closure: &ResolvedPackageSourceClosure,
    reviews: &CompilerIssuedPackageReviewSet,
    reconstruction_limits: CanonicalPackageReconstructionQuestionLimits,
    conflict_limits: ReviewOnlyCapabilityConflictLimits,
    root_policy: Option<&ReviewOnlyRootPolicyResolution>,
) -> Result<FreshPackageRootPolicyAcceptance, FreshPackageRootPolicyError> {
    let obligations = LocallyComposedPackageObligationResults::from_resolved_and_reviews(
        closure,
        reviews,
        reconstruction_limits,
    )
    .map_err(FreshPackageRootPolicyError::Reconstruction)?;
    let conflicts = compare_review_only_initial_capabilities(reviews, closure, conflict_limits)
        .map_err(FreshPackageRootPolicyError::ConflictComparison)?;

    validate_open_claim_conflicts(&obligations, &conflicts)?;

    let has_blocking_conflicts = conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .any(|conflict| conflict.is_blocking());
    let accepted_policy = match (has_blocking_conflicts, root_policy) {
        (false, None) => None,
        (false, Some(_)) => return Err(FreshPackageRootPolicyError::UnexpectedRootPolicy),
        (true, None) => return Err(FreshPackageRootPolicyError::MissingRootPolicy),
        (true, Some(policy)) => {
            let replayed =
                resolve_review_only_root_policy_decisions(&conflicts, policy.decisions())
                    .map_err(FreshPackageRootPolicyError::InvalidRootPolicy)?;
            if &replayed != policy {
                return Err(FreshPackageRootPolicyError::RootPolicyReplayMismatch);
            }
            if !replayed.all_blocking_rows_accepted() {
                return Err(FreshPackageRootPolicyError::RejectedBlockingConflict);
            }
            Some(replayed)
        }
    };

    Ok(FreshPackageRootPolicyAcceptance {
        obligations,
        conflicts,
        root_policy: accepted_policy,
    })
}

type AcceptedClaimCoordinate<'a> = (&'a crate::declarations::PackageKey, &'a [u8], &'a [u8]);

fn validate_open_claim_conflicts(
    obligations: &LocallyComposedPackageObligationResults,
    conflicts: &ReviewOnlyCapabilityConflictSet,
) -> Result<(), FreshPackageRootPolicyError> {
    let open_claim_count = obligations.root_open_accepted_claims().len();
    let accepted_conflict_count = conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .filter(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::AcceptedClaim)
        .count();

    let mut open_claims = Vec::new();
    open_claims
        .try_reserve_exact(open_claim_count)
        .map_err(|_| FreshPackageRootPolicyError::AllocationFailed)?;
    for (package, claim) in obligations.root_open_accepted_claims() {
        open_claims.push((
            package,
            claim.row().key_bytes(),
            claim.row().canonical_bytes(),
        ));
    }

    let mut accepted_conflicts = Vec::new();
    accepted_conflicts
        .try_reserve_exact(accepted_conflict_count)
        .map_err(|_| FreshPackageRootPolicyError::AllocationFailed)?;
    for package in conflicts.packages() {
        for conflict in package
            .conflicts()
            .iter()
            .filter(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::AcceptedClaim)
        {
            if !package.baseline().is_empty_admission()
                || conflict.change() != ReviewOnlyCapabilityConflictChange::Added
                || conflict.risk() != PackageReviewCanonicalRowRisk::Blocking
                || conflict.baseline_row().is_some()
            {
                return Err(FreshPackageRootPolicyError::AcceptedClaimConflictShapeMismatch);
            }
            let candidate_row = conflict
                .candidate_row()
                .ok_or(FreshPackageRootPolicyError::AcceptedClaimConflictShapeMismatch)?;
            accepted_conflicts.push((package.key(), conflict.row_key(), candidate_row));
        }
    }

    sort_claim_coordinates(&mut open_claims);
    sort_claim_coordinates(&mut accepted_conflicts);
    if open_claims != accepted_conflicts {
        return Err(FreshPackageRootPolicyError::AcceptedClaimConflictSetMismatch);
    }
    Ok(())
}

fn sort_claim_coordinates(coordinates: &mut [AcceptedClaimCoordinate<'_>]) {
    coordinates.sort_unstable_by(|left, right| {
        left.0
            .cmp(right.0)
            .then_with(|| left.1.cmp(right.1))
            .then_with(|| left.2.cmp(right.2))
    });
}
