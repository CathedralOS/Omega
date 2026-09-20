//! Exact in-memory association of fresh obligations with accepted project policy.

use super::{
    CanonicalPackageReconstructionQuestion, CanonicalPackageReconstructionQuestionError,
    CanonicalPackageReconstructionQuestionLimits, LocallyComposedPackageObligationResults,
};
use crate::lock::PackageLockTarget;
use crate::resolution::graph::ExactTargetPackageSourceClosure;
use crate::review::{
    CompilerIssuedPackageReview, CompilerIssuedPackageReviewSet, PackagePolicyChangeError,
    PackagePolicyChangeLimits, PackagePolicyChangeSet, ReviewOnlyCapabilityConflictError,
    ReviewOnlyCapabilityConflictLimits, compare_package_policy_changes,
    render_package_policy_review,
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
    policy_changes: PackagePolicyChangeSet,
}

impl FreshPackageRootPolicyAcceptance {
    pub const fn obligations(&self) -> &LocallyComposedPackageObligationResults {
        &self.obligations
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
                "fresh {kind:?} row has the wrong obligation kind or risk",
            ),
            Self::OpenObligationConflictSetMismatch(kind) => write!(
                formatter,
                "fresh {kind:?} rows are not bijective with reconstructed occurrence-local open obligations",
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
/// Obligations and policy are derived from the same exact occurrence reviews.
/// A caller cannot pair obligations from one context with policy for another.
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
    validate_open_obligation_rows(&obligations, &associated_reviews, conflict_limits)?;

    // Both projections belong to the same immutable compiler-issued reviews.
    // Canonical rows witness the occurrence-local obligation bijection; project intent
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
            policy_changes,
        },
        associated_reviews,
    ))
}

/// Check each occurrence independently: equal row bytes from another role are
/// not a witness for this occurrence's open obligations.
fn validate_open_obligation_rows(
    obligations: &LocallyComposedPackageObligationResults,
    reviews: &[&CompilerIssuedPackageReview],
    limits: ReviewOnlyCapabilityConflictLimits,
) -> Result<(), FreshPackageRootPolicyError> {
    use crate::review::compare::resources::{ComparisonInputBudget, account_review_resources};
    let mut input_budget = ComparisonInputBudget::default();
    for review in reviews {
        account_review_resources(std::slice::from_ref(*review), limits, &mut input_budget)
            .map_err(FreshPackageRootPolicyError::ConflictComparison)?;
    }
    let mut remaining_rows = limits.maximum_conflicts();
    let mut remaining_bytes = limits.maximum_changed_row_bytes();
    for (entry, review) in obligations.entries().iter().zip(reviews) {
        if entry.package() != review.key() || entry.context() != review.checked_context() {
            return Err(FreshPackageRootPolicyError::Reconstruction(
                CanonicalPackageReconstructionQuestionError::new(
                    "open obligation occurrence association differs",
                ),
            ));
        }
        let results = entry.results();
        validate_kind_rows(
            review,
            PackageReviewCanonicalRowKind::AcceptedClaim,
            PackageReviewCanonicalRowRisk::Blocking,
            results
                .open_accepted_claims()
                .iter()
                .map(|value| value.row()),
            &mut remaining_rows,
            &mut remaining_bytes,
        )?;
        validate_kind_rows(
            review,
            PackageReviewCanonicalRowKind::ExternalExecutableSupply,
            PackageReviewCanonicalRowRisk::OpaqueBlocking,
            results
                .open_external_executable_supplies()
                .iter()
                .map(|value| value.row()),
            &mut remaining_rows,
            &mut remaining_bytes,
        )?;
        validate_kind_rows(
            review,
            PackageReviewCanonicalRowKind::DangerousAuthority,
            PackageReviewCanonicalRowRisk::Blocking,
            results
                .open_dangerous_authorities()
                .iter()
                .map(|value| value.row()),
            &mut remaining_rows,
            &mut remaining_bytes,
        )?;
        validate_kind_rows(
            review,
            PackageReviewCanonicalRowKind::TerminalAuthorityPermission,
            PackageReviewCanonicalRowRisk::Blocking,
            results
                .open_terminal_authority_permissions()
                .iter()
                .map(|value| value.row()),
            &mut remaining_rows,
            &mut remaining_bytes,
        )?;
    }
    Ok(())
}

fn validate_kind_rows<'row>(
    review: &'row CompilerIssuedPackageReview,
    kind: PackageReviewCanonicalRowKind,
    expected_risk: PackageReviewCanonicalRowRisk,
    open: impl ExactSizeIterator<Item = &'row package_evidence::ledger::OrdinaryPackageObligationRow>,
    remaining_rows: &mut usize,
    remaining_bytes: &mut usize,
) -> Result<(), FreshPackageRootPolicyError> {
    let count = review
        .canonical_rows()
        .iter()
        .filter(|row| row.kind() == kind)
        .count();
    if count != open.len() {
        return Err(FreshPackageRootPolicyError::OpenObligationConflictSetMismatch(kind));
    }
    *remaining_rows = remaining_rows
        .checked_sub(count)
        .ok_or(FreshPackageRootPolicyError::AllocationFailed)?;
    let mut expected = Vec::new();
    let mut actual = Vec::new();
    expected
        .try_reserve_exact(count)
        .map_err(|_| FreshPackageRootPolicyError::AllocationFailed)?;
    actual
        .try_reserve_exact(count)
        .map_err(|_| FreshPackageRootPolicyError::AllocationFailed)?;
    for row in review
        .canonical_rows()
        .iter()
        .filter(|row| row.kind() == kind)
    {
        if row.risk() != expected_risk {
            return Err(FreshPackageRootPolicyError::OpenObligationConflictShapeMismatch(kind));
        }
        let bytes = row
            .key_bytes()
            .len()
            .checked_add(row.canonical_bytes().len())
            .ok_or(FreshPackageRootPolicyError::AllocationFailed)?;
        *remaining_bytes = remaining_bytes
            .checked_sub(bytes)
            .ok_or(FreshPackageRootPolicyError::AllocationFailed)?;
        expected.push((row.key_bytes(), row.canonical_bytes()));
    }
    for row in open {
        if row.kind() != kind || row.risk() != expected_risk {
            return Err(FreshPackageRootPolicyError::OpenObligationConflictShapeMismatch(kind));
        }
        actual.push((row.key_bytes(), row.canonical_bytes()));
    }
    expected.sort_unstable();
    actual.sort_unstable();
    if expected.windows(2).any(|pair| pair[0].0 == pair[1].0)
        || actual.windows(2).any(|pair| pair[0].0 == pair[1].0)
        || expected != actual
    {
        return Err(FreshPackageRootPolicyError::OpenObligationConflictSetMismatch(kind));
    }
    Ok(())
}
