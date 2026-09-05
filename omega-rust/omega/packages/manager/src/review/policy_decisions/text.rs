//! Strict restart records rejoined to current obligations, never inert history.
mod framing;

use super::{
    PackagePolicyDecision, PackagePolicyDecisionError as Error, PackagePolicyDecisionLimits,
    PackagePolicyDecisionResolution, limits::Budget, obligations, resolution,
};
use crate::review::{PackagePolicyChangeSet, ReviewOnlyRootPolicyDisposition};
use framing::{HEADER, digest, emit, number};
use std::fmt::Write;

impl PackagePolicyDecisionResolution {
    pub fn canonical_text(&self, limits: PackagePolicyDecisionLimits) -> Result<String, Error> {
        let mut budget = Budget::new(limits);
        budget.decisions(self.decisions.len())?;
        let mut count = framing::Counter::default();
        emit(self, &mut count).map_err(|_| Error::LengthOverflow)?;
        budget.bytes(count.bytes)?;
        budget.owned(count.bytes)?;
        let mut output = String::new();
        output
            .try_reserve_exact(count.bytes)
            .map_err(|_| Error::AllocationFailed)?;
        emit(self, &mut output).map_err(|_| Error::InvalidFraming)?;
        debug_assert_eq!(output.len(), count.bytes);
        Ok(output)
    }
}

/// Recover only against the exact CURRENT comparison. The complete blocker
/// bijection is revalidated; the same text cannot authorize a different set.
pub fn recover_package_policy_decisions(
    text: &str,
    changes: &PackagePolicyChangeSet,
    limits: PackagePolicyDecisionLimits,
) -> Result<PackagePolicyDecisionResolution, Error> {
    let mut budget = Budget::new(limits);
    budget.bytes(text.len())?;
    let body = text.strip_suffix('\n').ok_or(Error::InvalidFraming)?;
    let mut lines = body.split('\n');
    let header = lines.next().ok_or(Error::InvalidFraming)?;
    if header != HEADER {
        return Err(if header.starts_with("omega-package-policy-decisions ") {
            Error::UnsupportedVersion
        } else {
            Error::InvalidFraming
        });
    }
    let change_set = digest(value(&mut lines, "change_set ")?)?;
    if change_set != changes.fingerprint().digest() {
        return Err(Error::WrongChangeSet);
    }
    let count = number(value(&mut lines, "decisions ")?)?;
    budget.decisions(count)?;
    let remaining_bytes = lines.clone().try_fold(0usize, |total, line| {
        total
            .checked_add(line.len())
            .and_then(|total| total.checked_add(1))
            .ok_or(Error::LengthOverflow)
    })?;
    const MINIMUM_DECISION_BYTES: usize =
        "decision ".len() + 64 + 1 + 64 + 1 + "accept_candidate_change".len() + 1;
    if count
        .checked_mul(MINIMUM_DECISION_BYTES)
        .ok_or(Error::LengthOverflow)?
        > remaining_bytes
    {
        return Err(Error::InvalidFraming);
    }
    let known = obligations::collect(changes, &mut budget)?;
    resolution::exact_count(count, known.len())?;
    let mut decisions = budget.vector(count)?;
    let mut previous = None;
    for _ in 0..count {
        let mut fields = value(&mut lines, "decision ")?.split(' ');
        let package = digest(fields.next().ok_or(Error::InvalidFraming)?)?;
        let fingerprint = digest(fields.next().ok_or(Error::InvalidFraming)?)?;
        let disposition = match fields.next() {
            Some("accept_candidate_change") => {
                ReviewOnlyRootPolicyDisposition::AcceptCandidateChange
            }
            Some("reject_candidate_change") => {
                ReviewOnlyRootPolicyDisposition::RejectCandidateChange
            }
            _ => return Err(Error::InvalidDisposition),
        };
        if fields.next().is_some() {
            return Err(Error::InvalidFraming);
        }
        if previous.is_some_and(|previous| previous >= fingerprint) {
            return Err(Error::NonCanonicalDecisions);
        }
        previous = Some(fingerprint);
        let index = known
            .binary_search_by_key(&fingerprint, |obligation| obligation.fingerprint().digest())
            .map_err(|_| Error::StaleOrForeignObligation)?;
        let obligation = known[index];
        if package != obligation.package().digest() {
            return Err(Error::ForeignPackage);
        }
        decisions.push(PackagePolicyDecision {
            obligation,
            disposition,
        });
    }
    let expected = digest(value(&mut lines, "resolution ")?)?;
    if lines.next() != Some("end") || lines.next().is_some() {
        return Err(Error::InvalidFraming);
    }
    let result = resolution::finish(changes, &known, decisions)?;
    if result.fingerprint.digest() != expected {
        return Err(Error::ResolutionFingerprintMismatch);
    }
    // Re-encoding compares directly with borrowed input. No independently
    // budgeted record copy or reset occurs after the two charged tables.
    let mut comparison = Comparison(text);
    emit(&result, &mut comparison).map_err(|_| Error::InvalidFraming)?;
    if !comparison.0.is_empty() {
        return Err(Error::InvalidFraming);
    }
    Ok(result)
}

fn value<'a>(lines: &mut impl Iterator<Item = &'a str>, prefix: &str) -> Result<&'a str, Error> {
    lines
        .next()
        .and_then(|line| line.strip_prefix(prefix))
        .ok_or(Error::InvalidFraming)
}
struct Comparison<'a>(&'a str);
impl Write for Comparison<'_> {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        self.0 = self.0.strip_prefix(text).ok_or(std::fmt::Error)?;
        Ok(())
    }
}
