//! Exact in-memory association of fresh obligations with accepted project policy.

use super::{
    CanonicalPackageReconstructionQuestion, CanonicalPackageReconstructionQuestionError,
    CanonicalPackageReconstructionQuestionLimits, LocallyComposedPackageObligationResults,
};
use crate::lock::PackageLockTarget;
use crate::resolution::graph::ExactTargetPackageSourceClosure;
use crate::review::{
    CompilerIssuedPackageReview, CompilerIssuedPackageReviewSet, PackagePolicyChangeError,
    PackagePolicyChangeLimits, PackagePolicyChangeSet, ReviewOnlyCapabilityConflictChange,
    ReviewOnlyCapabilityConflictError, ReviewOnlyCapabilityConflictLimits,
    ReviewOnlyCapabilityConflictSet, compare_package_policy_changes,
    compare_review_only_initial_capabilities, render_package_policy_review,
};
use package_evidence::record::{PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk};
use std::fmt;

/// Freshly reconstructed obligations checked against the project's accepted
/// normalized policy without requiring another decision for unchanged requirements.
///
/// This is only an in-memory policy association. It is not complete package
/// evidence, an accepted lock row, a `PackageInstance`, or permission to
/// mutate project files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FreshPackageRootPolicyAcceptance {
    obligations: LocallyComposedPackageObligationResults,
    conflicts: ReviewOnlyCapabilityConflictSet,
    policy_changes: PackagePolicyChangeSet,
}

impl FreshPackageRootPolicyAcceptance {
    pub const fn obligations(&self) -> &LocallyComposedPackageObligationResults {
        &self.obligations
    }

    pub const fn conflicts(&self) -> &ReviewOnlyCapabilityConflictSet {
        &self.conflicts
    }

    /// Fresh comparison against the project's accepted policy, including source
    /// changes that remain visible even when no new decision is required.
    pub const fn policy_changes(&self) -> &PackagePolicyChangeSet {
        &self.policy_changes
    }
}

#[derive(Debug)]
pub enum FreshPackageRootPolicyError {
    Reconstruction(CanonicalPackageReconstructionQuestionError),
    ConflictComparison(ReviewOnlyCapabilityConflictError),
    PolicyComparison(PackagePolicyChangeError),
    ReviewRequired(PackagePolicyChangeSet),
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
            Self::PolicyComparison(error) => {
                write!(formatter, "fresh package policy comparison failed: {error}")
            }
            Self::ReviewRequired(changes) => {
                writeln!(
                    formatter,
                    "package acceptance is missing or current requirements need review; run omega update --project <project> --target <target> and complete the package review"
                )?;
                match render_package_policy_review(changes, 16 * 1024 * 1024) {
                    Ok(review) => formatter.write_str(&review),
                    Err(error) => write!(
                        formatter,
                        "package review findings could not be rendered: {error}"
                    ),
                }
            }
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
            Self::PolicyComparison(error) => Some(error),
            Self::ReviewRequired(_)
            | Self::UnresolvedLaterDischarge(_)
            | Self::OpenObligationConflictShapeMismatch(_)
            | Self::OpenObligationConflictSetMismatch(_)
            | Self::AllocationFailed => None,
        }
    }
}

/// Reconstruct a fresh candidate and check its current requirements against
/// the project's accepted policy.
///
/// The conflict set is deliberately rederived here. A caller cannot pair
/// obligations from one source closure with policy compared for another.
pub fn bind_fresh_package_root_policy(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    reviews: &CompilerIssuedPackageReviewSet,
    reconstruction_limits: CanonicalPackageReconstructionQuestionLimits,
    conflict_limits: ReviewOnlyCapabilityConflictLimits,
    accepted: Option<&PackageLockTarget>,
) -> Result<FreshPackageRootPolicyAcceptance, FreshPackageRootPolicyError> {
    bind_root_policy_with_associated_reviews(
        target_closure,
        reviews,
        reconstruction_limits,
        conflict_limits,
        accepted,
    )
    .map(|(acceptance, _)| acceptance)
}

/// Keep the same immutable review association through final payload assembly.
/// Public callers still start from the resolved closure and compiler reviews.
pub(crate) fn bind_root_policy_with_associated_reviews<'reviews>(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    reviews: &'reviews CompilerIssuedPackageReviewSet,
    reconstruction_limits: CanonicalPackageReconstructionQuestionLimits,
    conflict_limits: ReviewOnlyCapabilityConflictLimits,
    accepted: Option<&PackageLockTarget>,
) -> Result<
    (
        FreshPackageRootPolicyAcceptance,
        Vec<&'reviews CompilerIssuedPackageReview>,
    ),
    FreshPackageRootPolicyError,
> {
    let (question, associated_reviews) =
        CanonicalPackageReconstructionQuestion::associate_resolved_reviews(
            target_closure,
            reviews,
            reconstruction_limits,
        )
        .map_err(FreshPackageRootPolicyError::Reconstruction)?;
    let obligations = LocallyComposedPackageObligationResults::from_associated_reviews(
        question,
        &associated_reviews,
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
    let conflicts =
        compare_review_only_initial_capabilities(reviews, target_closure, conflict_limits)
            .map_err(FreshPackageRootPolicyError::ConflictComparison)?;

    validate_open_obligation_conflicts(&obligations, &conflicts)?;

    // Both projections belong to the same immutable compiler-issued reviews.
    // Initial conflicts witness the obligation bijection only; project intent
    // comes from the retained complete policy and the ordinary comparison.
    let policy_changes = compare_package_policy_changes(
        accepted,
        reviews,
        target_closure,
        PackagePolicyChangeLimits::default(),
    )
    .map_err(FreshPackageRootPolicyError::PolicyComparison)?;
    if accepted.is_none() || policy_changes.requires_decision() {
        return Err(FreshPackageRootPolicyError::ReviewRequired(policy_changes));
    }

    Ok((
        FreshPackageRootPolicyAcceptance {
            obligations,
            conflicts,
            policy_changes,
        },
        associated_reviews,
    ))
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
    )?;
    validate_open_obligation_kind(
        obligations,
        conflicts,
        PackageReviewCanonicalRowKind::TerminalAuthorityPermission,
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
        PackageReviewCanonicalRowKind::TerminalAuthorityPermission => {
            obligations.root_open_terminal_authority_permissions().len()
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
        PackageReviewCanonicalRowKind::TerminalAuthorityPermission => {
            for (package, permission) in obligations.root_open_terminal_authority_permissions() {
                open_obligations.push((
                    package,
                    permission.row().key_bytes(),
                    permission.row().canonical_bytes(),
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
