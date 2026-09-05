//! Project choices for complete retained-policy comparisons.
//!
//! A comparison already owns checked findings and their source association.
//! Resolving choices checks only their exact coverage and context, without
//! reconstructing the compiler result or manufacturing evidence of review.

mod limits;
mod text;

pub use limits::PackagePolicyDecisionLimits;
pub use text::recover_package_policy_decisions;

use super::ReviewOnlyRootPolicyDisposition;
use crate::review::{PackagePolicyChangeFingerprint, PackagePolicyChangeSet};
use std::fmt;

/// An exact row change, or the separately reported root-role compatibility
/// change. Digests come from the comparison presented for review; they are
/// identifiers, not authorization until checked against that comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicyDecisionSubject {
    RootRole,
    Row([u8; 32]),
}

/// Caller-supplied project choice. A parser or UI may construct it directly;
/// completeness and association are checked by `resolve_package_policy_decisions`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackagePolicyDecision {
    pub subject: PackagePolicyDecisionSubject,
    pub disposition: ReviewOnlyRootPolicyDisposition,
}

/// Complete choices for one comparison, not a proof, audit receipt, or file
/// transaction. A rejection is retained and prevents permission to proceed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyResolution {
    comparison: PackagePolicyChangeFingerprint,
    decisions: Vec<PackagePolicyDecision>,
}

impl PackagePolicyResolution {
    pub const fn comparison(&self) -> PackagePolicyChangeFingerprint {
        self.comparison
    }

    pub fn decisions(&self) -> &[PackagePolicyDecision] {
        &self.decisions
    }

    /// All represented required choices accept their changes. This alone is
    /// not permission to publish: compiler obligations, source replacements,
    /// and source/project-file consistency need their own transaction checks.
    pub fn all_required_changes_accepted(&self) -> bool {
        self.decisions.iter().all(|decision| {
            decision.disposition == ReviewOnlyRootPolicyDisposition::AcceptCandidateChange
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackagePolicyDecisionError {
    WrongComparison,
    TooManyDecisions,
    UnknownSubject(PackagePolicyDecisionSubject),
    NonBlockingChange(PackagePolicyDecisionSubject),
    DuplicateDecision(PackagePolicyDecisionSubject),
    MissingDecision(PackagePolicyDecisionSubject),
    DuplicateComparisonSubject(PackagePolicyDecisionSubject),
    AllocationFailed,
    DecisionLimitExceeded,
    ChangeLimitExceeded,
    ByteLimitExceeded,
    OwnedLimitExceeded,
    LengthOverflow,
    InvalidFraming,
    UnsupportedVersion,
    InvalidDisposition,
    NonCanonicalDecisions,
}

impl fmt::Display for PackagePolicyDecisionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongComparison => formatter.write_str(
                "package decisions belong to a different baseline, candidate, graph, or target",
            ),
            Self::TooManyDecisions => {
                formatter.write_str("package decisions exceed the number of required choices")
            }
            Self::UnknownSubject(subject) => {
                write!(
                    formatter,
                    "package decision references an unknown change: {subject:?}"
                )
            }
            Self::NonBlockingChange(subject) => {
                write!(
                    formatter,
                    "package change does not require a decision: {subject:?}"
                )
            }
            Self::DuplicateDecision(subject) => {
                write!(formatter, "package decisions repeat a change: {subject:?}")
            }
            Self::MissingDecision(subject) => {
                write!(
                    formatter,
                    "package change needs an explicit decision: {subject:?}"
                )
            }
            Self::DuplicateComparisonSubject(subject) => {
                write!(
                    formatter,
                    "package comparison repeats a decision subject: {subject:?}"
                )
            }
            Self::AllocationFailed => formatter.write_str("package decision allocation failed"),
            Self::DecisionLimitExceeded => formatter.write_str("package decisions exceed the decision-count limit"),
            Self::ChangeLimitExceeded => formatter.write_str("package decision comparison exceeds the scan limit"),
            Self::ByteLimitExceeded => formatter.write_str("package decision text exceeds the byte limit"),
            Self::OwnedLimitExceeded => formatter.write_str("package decisions exceed the owned-storage limit"),
            Self::LengthOverflow => formatter.write_str("package decision resource length overflow"),
            Self::InvalidFraming => formatter.write_str("package decision text has invalid canonical framing"),
            Self::UnsupportedVersion => formatter.write_str("unsupported package decision text version; retain current source pins and recover with a compatible toolchain"),
            Self::InvalidDisposition => formatter.write_str("package decision text has an unknown disposition"),
            Self::NonCanonicalDecisions => formatter.write_str("package decision text repeats or misorders subjects"),
        }
    }
}

impl std::error::Error for PackagePolicyDecisionError {}

/// Bind exactly one choice to every required change in this comparison.
///
/// `comparison` is the digest retained with the user's decisions, not a digest
/// substituted from the current report at resume time. It covers both source
/// subjects and complete policy findings. Unknown, missing, duplicate, stale,
/// and advisory-only choices reject. Removed packages remain ordinary row
/// subjects: neither their checkout nor a candidate package index is needed.
pub fn resolve_package_policy_decisions(
    changes: &PackagePolicyChangeSet,
    comparison: [u8; 32],
    decisions: &[PackagePolicyDecision],
) -> Result<PackagePolicyResolution, PackagePolicyDecisionError> {
    resolve_package_policy_decisions_with_limits(
        changes,
        comparison,
        decisions,
        PackagePolicyDecisionLimits::default(),
    )
}

/// The same exact resolver under lowerable aggregate resource ceilings.
pub fn resolve_package_policy_decisions_with_limits(
    changes: &PackagePolicyChangeSet,
    comparison: [u8; 32],
    decisions: &[PackagePolicyDecision],
    limits: PackagePolicyDecisionLimits,
) -> Result<PackagePolicyResolution, PackagePolicyDecisionError> {
    resolve(
        changes,
        comparison,
        decisions,
        &mut limits::Budget::new(limits),
    )
}

fn resolve(
    changes: &PackagePolicyChangeSet,
    comparison: [u8; 32],
    decisions: &[PackagePolicyDecision],
    budget: &mut limits::Budget,
) -> Result<PackagePolicyResolution, PackagePolicyDecisionError> {
    use PackagePolicyDecisionError as Error;
    use PackagePolicyDecisionSubject as Subject;

    if comparison != changes.fingerprint().digest() {
        return Err(Error::WrongComparison);
    }
    budget.decisions(decisions.len())?;
    // Comparison construction bounds the row count. No caller-provided count
    // controls allocation until it has been checked against that actual set.
    budget.changes(changes.packages().len())?;
    let row_count = changes
        .packages()
        .iter()
        .try_fold(0usize, |count, package| {
            count
                .checked_add(package.rows().len())
                .ok_or(Error::LengthOverflow)
        })?;
    let count = row_count
        .checked_add(usize::from(changes.root_role_change().is_some()))
        .ok_or(Error::LengthOverflow)?;
    budget.changes(
        count
            .checked_add(changes.packages().len())
            .ok_or(Error::LengthOverflow)?,
    )?;
    let mut subjects = budget.vector(count)?;
    if changes.root_role_change().is_some() {
        subjects.push((Subject::RootRole, true));
    }
    for package in changes.packages() {
        subjects.extend(package.rows().iter().map(|row| {
            (
                Subject::Row(row.fingerprint().digest()),
                row.requires_decision(),
            )
        }));
    }
    subjects.sort_unstable_by_key(|(subject, _)| *subject);
    for repeated in subjects.windows(2) {
        if repeated[0].0 == repeated[1].0 {
            return Err(Error::DuplicateComparisonSubject(repeated[0].0));
        }
    }
    let required_count = subjects.iter().filter(|(_, required)| *required).count();
    if decisions.len() > required_count {
        return Err(Error::TooManyDecisions);
    }
    let mut sorted = budget.vector(decisions.len())?;
    for decision in decisions {
        let index = subjects
            .binary_search_by_key(&decision.subject, |(subject, _)| *subject)
            .map_err(|_| Error::UnknownSubject(decision.subject))?;
        if !subjects[index].1 {
            return Err(Error::NonBlockingChange(decision.subject));
        }
        sorted.push(*decision);
    }
    sorted.sort_unstable_by_key(|decision| decision.subject);
    for repeated in sorted.windows(2) {
        if repeated[0].subject == repeated[1].subject {
            return Err(Error::DuplicateDecision(repeated[0].subject));
        }
    }
    let mut selected = sorted.iter();
    for (subject, required) in subjects {
        if required
            && selected
                .next()
                .is_none_or(|decision| decision.subject != subject)
        {
            return Err(Error::MissingDecision(subject));
        }
    }
    Ok(PackagePolicyResolution {
        comparison: changes.fingerprint(),
        decisions: sorted,
    })
}
