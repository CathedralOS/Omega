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
    UnresolvedLaterDischarge(PackageReviewCanonicalRowKind),
    OpenObligationConflictShapeMismatch(PackageReviewCanonicalRowKind),
    OpenObligationConflictSetMismatch(PackageReviewCanonicalRowKind),
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
            Self::MissingRootPolicy => formatter
                .write_str("fresh package candidate has blocking rows but no complete root policy"),
            Self::UnexpectedRootPolicy => formatter.write_str(
                "fresh package candidate has no blocking rows but was given root policy",
            ),
            Self::InvalidRootPolicy(error) => {
                write!(formatter, "fresh package root policy is invalid: {error}")
            }
            Self::RootPolicyReplayMismatch => {
                formatter.write_str("fresh package root policy differs from its canonical replay")
            }
            Self::RejectedBlockingConflict => formatter
                .write_str("fresh package root policy rejects at least one exact blocking row"),
            Self::UnresolvedLaterDischarge(kind) => write!(
                formatter,
                "fresh {kind:?} obligation requires a concrete later discharge and cannot be admitted by root policy",
            ),
            Self::OpenObligationConflictShapeMismatch(kind) => write!(
                formatter,
                "fresh {kind:?} conflict is not an added blocking row against the empty admission baseline",
            ),
            Self::OpenObligationConflictSetMismatch(kind) => write!(
                formatter,
                "fresh {kind:?} conflicts are not bijective with reconstructed open obligations",
            ),
            Self::AllocationFailed => {
                formatter.write_str("fresh package root-policy association allocation failed")
            }
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
            | Self::UnresolvedLaterDischarge(_)
            | Self::OpenObligationConflictShapeMismatch(_)
            | Self::OpenObligationConflictSetMismatch(_)
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
    if obligations
        .root_open_contract_entailment_obligations()
        .next()
        .is_some()
    {
        return Err(FreshPackageRootPolicyError::UnresolvedLaterDischarge(
            PackageReviewCanonicalRowKind::ContractEntailmentOpenObligation,
        ));
    }
    let conflicts = compare_review_only_initial_capabilities(reviews, closure, conflict_limits)
        .map_err(FreshPackageRootPolicyError::ConflictComparison)?;

    validate_open_obligation_conflicts(&obligations, &conflicts)?;

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

type OpenObligationCoordinate<'a> = (&'a crate::declarations::PackageKey, &'a [u8], &'a [u8]);

fn validate_open_obligation_conflicts(
    obligations: &LocallyComposedPackageObligationResults,
    conflicts: &ReviewOnlyCapabilityConflictSet,
) -> Result<(), FreshPackageRootPolicyError> {
    validate_open_obligation_kind(
        obligations,
        conflicts,
        PackageReviewCanonicalRowKind::AcceptedClaim,
        PackageReviewCanonicalRowRisk::Blocking,
    )?;
    validate_open_obligation_kind(
        obligations,
        conflicts,
        PackageReviewCanonicalRowKind::ExternalExecutableSupply,
        PackageReviewCanonicalRowRisk::OpaqueBlocking,
    )?;
    validate_open_obligation_kind(
        obligations,
        conflicts,
        PackageReviewCanonicalRowKind::DangerousAuthority,
        PackageReviewCanonicalRowRisk::Blocking,
    )
}

fn validate_open_obligation_kind<'a>(
    obligations: &'a LocallyComposedPackageObligationResults,
    conflicts: &'a ReviewOnlyCapabilityConflictSet,
    kind: PackageReviewCanonicalRowKind,
    expected_risk: PackageReviewCanonicalRowRisk,
) -> Result<(), FreshPackageRootPolicyError> {
    let open_count = match kind {
        PackageReviewCanonicalRowKind::AcceptedClaim => {
            obligations.root_open_accepted_claims().len()
        }
        PackageReviewCanonicalRowKind::ExternalExecutableSupply => {
            obligations.root_open_external_executable_supplies().len()
        }
        PackageReviewCanonicalRowKind::DangerousAuthority => {
            obligations.root_open_dangerous_authorities().len()
        }
        _ => 0,
    };
    let conflict_count = conflicts
        .packages()
        .iter()
        .flat_map(|package| package.conflicts())
        .filter(|conflict| conflict.kind() == kind)
        .count();

    let mut open_obligations = Vec::new();
    open_obligations
        .try_reserve_exact(open_count)
        .map_err(|_| FreshPackageRootPolicyError::AllocationFailed)?;
    match kind {
        PackageReviewCanonicalRowKind::AcceptedClaim => {
            for (package, claim) in obligations.root_open_accepted_claims() {
                open_obligations.push((
                    package,
                    claim.row().key_bytes(),
                    claim.row().canonical_bytes(),
                ));
            }
        }
        PackageReviewCanonicalRowKind::ExternalExecutableSupply => {
            for (package, supply) in obligations.root_open_external_executable_supplies() {
                open_obligations.push((
                    package,
                    supply.row().key_bytes(),
                    supply.row().canonical_bytes(),
                ));
            }
        }
        PackageReviewCanonicalRowKind::DangerousAuthority => {
            for (package, authority) in obligations.root_open_dangerous_authorities() {
                open_obligations.push((
                    package,
                    authority.row().key_bytes(),
                    authority.row().canonical_bytes(),
                ));
            }
        }
        _ => return Ok(()),
    }

    let mut matching_conflicts = Vec::new();
    matching_conflicts
        .try_reserve_exact(conflict_count)
        .map_err(|_| FreshPackageRootPolicyError::AllocationFailed)?;
    for package in conflicts.packages() {
        for conflict in package
            .conflicts()
            .iter()
            .filter(|conflict| conflict.kind() == kind)
        {
            if !package.baseline().is_empty_admission()
                || conflict.change() != ReviewOnlyCapabilityConflictChange::Added
                || conflict.risk() != expected_risk
                || conflict.baseline_row().is_some()
            {
                return Err(FreshPackageRootPolicyError::OpenObligationConflictShapeMismatch(kind));
            }
            let candidate_row = conflict
                .candidate_row()
                .ok_or(FreshPackageRootPolicyError::OpenObligationConflictShapeMismatch(kind))?;
            matching_conflicts.push((package.key(), conflict.row_key(), candidate_row));
        }
    }

    sort_open_obligation_coordinates(&mut open_obligations);
    sort_open_obligation_coordinates(&mut matching_conflicts);
    if open_obligations != matching_conflicts {
        return Err(FreshPackageRootPolicyError::OpenObligationConflictSetMismatch(kind));
    }
    Ok(())
}

fn sort_open_obligation_coordinates(coordinates: &mut [OpenObligationCoordinate<'_>]) {
    coordinates.sort_unstable_by(|left, right| {
        left.0
            .cmp(right.0)
            .then_with(|| left.1.cmp(right.1))
            .then_with(|| left.2.cmp(right.2))
    });
}
