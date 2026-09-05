//! Editable choices beside compiler-rendered findings, not an audit receipt.

mod output;
mod render;
mod sources;

use super::{
    PackagePolicyDecision, PackagePolicyDecisionError, PackagePolicyDecisionSubject,
    PackagePolicyResolution, ReviewOnlyRootPolicyDisposition, resolve_package_policy_decisions,
};
use crate::review::PackagePolicyChangeSet;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackagePolicyReviewError {
    ByteLimit,
    AllocationFailed,
    ChangedFindings,
    InvalidDecision,
    UnresolvedDecision(PackagePolicyDecisionSubject),
    Decisions(PackagePolicyDecisionError),
}

impl fmt::Display for PackagePolicyReviewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ByteLimit => formatter.write_str("package review document exceeds its byte limit"),
            Self::AllocationFailed => formatter.write_str("package review document allocation failed"),
            Self::ChangedFindings => formatter.write_str(
                "package review findings or framing changed; regenerate the document and edit only decisions",
            ),
            Self::InvalidDecision => formatter.write_str("package review decision must be accept or reject"),
            Self::UnresolvedDecision(subject) => write!(formatter, "package review decision remains pending: {subject:?}"),
            Self::Decisions(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for PackagePolicyReviewError {}

/// Render a restartable review input with `pending` beside every required
/// change. Edit those tokens to `accept` or `reject`; there is no blanket choice.
/// The caller owns file placement and any source-code audit workflow.
pub fn render_package_policy_review(
    changes: &PackagePolicyChangeSet,
    maximum_bytes: usize,
) -> Result<String, PackagePolicyReviewError> {
    Ok(render::template(changes, maximum_bytes)?.text)
}

/// Recover choices against freshly obtained comparison findings. Only the
/// decision tokens may differ from the generated document. This prevents an
/// accidental resume against edited or stale displayed meaning; it does not
/// authenticate the project author or establish whether anybody audited code.
pub fn recover_package_policy_review(
    changes: &PackagePolicyChangeSet,
    text: &str,
    maximum_bytes: usize,
) -> Result<PackagePolicyResolution, PackagePolicyReviewError> {
    use PackagePolicyReviewError as Error;
    if text.len() > maximum_bytes {
        return Err(Error::ByteLimit);
    }
    if !text.ends_with('\n') || text.contains('\r') {
        return Err(Error::ChangedFindings);
    }
    let template = render::template(changes, maximum_bytes)?;
    let mut actual = text.lines();
    let mut subjects = template.subjects.iter();
    let mut decisions = Vec::new();
    let mut unresolved = None;
    decisions
        .try_reserve_exact(template.subjects.len())
        .map_err(|_| Error::AllocationFailed)?;
    for expected in template.text.lines() {
        let line = actual.next().ok_or(Error::ChangedFindings)?;
        if expected.starts_with("comparison ") && line != expected {
            return Err(Error::Decisions(
                PackagePolicyDecisionError::WrongComparison,
            ));
        }
        if expected.starts_with("decision ") {
            let prefix = expected
                .strip_suffix("pending")
                .expect("generated pending choice");
            let subject = *subjects.next().expect("generated choice subject");
            let disposition = match line.strip_prefix(prefix).ok_or(Error::ChangedFindings)? {
                "accept" => ReviewOnlyRootPolicyDisposition::AcceptCandidateChange,
                "reject" => ReviewOnlyRootPolicyDisposition::RejectCandidateChange,
                "pending" => {
                    unresolved.get_or_insert(subject);
                    continue;
                }
                _ => return Err(Error::InvalidDecision),
            };
            decisions.push(PackagePolicyDecision {
                subject,
                disposition,
            });
        } else if line != expected {
            return Err(Error::ChangedFindings);
        }
    }
    if actual.next().is_some() {
        return Err(Error::ChangedFindings);
    }
    if let Some(subject) = unresolved {
        return Err(Error::UnresolvedDecision(subject));
    }
    // The comparison header was checked against the retained document before
    // using this digest. No stale caller digest is silently substituted.
    resolve_package_policy_decisions(changes, changes.fingerprint().digest(), &decisions)
        .map_err(Error::Decisions)
}
