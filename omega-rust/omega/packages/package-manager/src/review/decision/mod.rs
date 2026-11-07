/*
    We resolve root-project policy choices (`resolve_package_policy_decisions`)
    over a complete `PackagePolicyChangeSet` and delegate editable review
    document rendering and recovery to `document`.

    A comparison already owns checked findings and their source associations.
    `resolve_package_policy_decisions` binds exactly one `PackagePolicyDecision`
    (`AcceptCandidateChange` or `RejectCandidateChange`) to every required row,
    source replacement, and root-role change in that comparison, rejecting
    unknown, missing, duplicate, stale, or advisory-only choices.
*/

/*
    Policy decision resolution belongs to the package review decision boundary
    (`review/decision/mod.rs`).
*/

mod document;

pub(crate) use document::{
    PackagePolicyReviewError, recover_package_policy_review, render_package_policy_review,
};

use crate::review::timings;
use crate::review::{PackagePolicyChangeFingerprint, PackagePolicyChangeSet};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum ReviewOnlyRootPolicyDisposition {
    AcceptCandidateChange,
    RejectCandidateChange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PackagePolicyDecisionSubject {
    RootRole,
    SourceReplacement([u8; 32]),
    Row([u8; 32]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PackagePolicyDecision {
    pub subject: PackagePolicyDecisionSubject,
    pub disposition: ReviewOnlyRootPolicyDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackagePolicyResolution {
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

    pub(crate) fn all_required_changes_accepted(&self) -> bool {
        self.decisions.iter().all(|decision| {
            decision.disposition == ReviewOnlyRootPolicyDisposition::AcceptCandidateChange
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PackagePolicyDecisionError {
    WrongComparison,
    TooManyDecisions,
    UnknownSubject(PackagePolicyDecisionSubject),
    NonBlockingChange(PackagePolicyDecisionSubject),
    DuplicateDecision(PackagePolicyDecisionSubject),
    MissingDecision(PackagePolicyDecisionSubject),
    DuplicateComparisonSubject(PackagePolicyDecisionSubject),
    AllocationFailed,
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
        }
    }
}

impl std::error::Error for PackagePolicyDecisionError {}

pub(crate) fn resolve_package_policy_decisions(
    changes: &PackagePolicyChangeSet,
    comparison: [u8; 32],
    decisions: &[PackagePolicyDecision],
) -> Result<PackagePolicyResolution, PackagePolicyDecisionError> {
    let _stage = timings::stage("policy_decision_resolution");
    use PackagePolicyDecisionError as Error;
    use PackagePolicyDecisionSubject as Subject;

    if comparison != changes.fingerprint().digest() {
        return Err(Error::WrongComparison);
    }
    let row_count = changes
        .packages()
        .iter()
        .try_fold(0usize, |count, package| {
            count
                .checked_add(package.rows().len())
                .ok_or(Error::AllocationFailed)
        })?;
    let count = row_count
        .checked_add(usize::from(changes.root_role_change().is_some()))
        .and_then(|count| count.checked_add(changes.source_replacements().len()))
        .ok_or(Error::AllocationFailed)?;
    let mut subjects = Vec::new();
    crate::reserve!(subjects, count, Error);
    if changes.root_role_change().is_some() {
        subjects.push((Subject::RootRole, true));
    }
    subjects.extend(changes.source_replacements().iter().map(|replacement| {
        (
            Subject::SourceReplacement(replacement.fingerprint().digest()),
            true,
        )
    }));
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
    let mut sorted = Vec::new();
    crate::reserve!(sorted, decisions.len(), Error);
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
