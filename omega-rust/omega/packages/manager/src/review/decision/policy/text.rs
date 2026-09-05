//! Restart text maps directly to the existing caller-supplied decision model.
mod framing;

use super::{
    PackagePolicyDecision, PackagePolicyDecisionError as Error, PackagePolicyDecisionLimits,
    PackagePolicyDecisionSubject as Subject, PackagePolicyResolution,
    ReviewOnlyRootPolicyDisposition as Disposition, limits::Budget,
};
use crate::review::PackagePolicyChangeSet;
use std::fmt::Write;

impl PackagePolicyResolution {
    pub fn canonical_text(&self, limits: PackagePolicyDecisionLimits) -> Result<String, Error> {
        let mut budget = Budget::new(limits);
        budget.decisions(self.decisions.len())?;
        let mut counter = framing::Counter::default();
        framing::emit(self, &mut counter).map_err(|_| Error::LengthOverflow)?;
        budget.bytes(counter.bytes)?;
        budget.owned(counter.bytes)?;
        let mut output = String::new();
        output
            .try_reserve_exact(counter.bytes)
            .map_err(|_| Error::AllocationFailed)?;
        framing::emit(self, &mut output).map_err(|_| Error::InvalidFraming)?;
        Ok(output)
    }
}

/// Recover choices only against their retained exact comparison digest. Even
/// an empty decision set cannot silently substitute a new comparison at resume.
pub fn recover_package_policy_decisions(
    text: &str,
    changes: &PackagePolicyChangeSet,
    limits: PackagePolicyDecisionLimits,
) -> Result<PackagePolicyResolution, Error> {
    let mut budget = Budget::new(limits);
    budget.bytes(text.len())?;
    let mut body = text
        .strip_prefix(framing::HEADER)
        .ok_or(Error::UnsupportedVersion)?;
    let comparison = framing::digest(field(&mut body, "comparison")?)?;
    if comparison != changes.fingerprint().digest() {
        return Err(Error::WrongComparison);
    }
    let count = framing::number(field(&mut body, "decisions")?)?;
    budget.decisions(count)?;
    const MINIMUM_ROW_BYTES: usize = "decision root_role accept_candidate_change\n".len();
    if count > body.len() / MINIMUM_ROW_BYTES {
        return Err(Error::InvalidFraming);
    }
    let mut decisions = budget.vector(count)?;
    let mut previous = None;
    for _ in 0..count {
        let mut fields = field(&mut body, "decision")?.split(' ');
        let subject = match fields.next() {
            Some("root_role") => Subject::RootRole,
            Some("row") => Subject::Row(framing::digest(
                fields.next().ok_or(Error::InvalidFraming)?,
            )?),
            _ => return Err(Error::InvalidFraming),
        };
        let disposition = match fields.next() {
            Some("accept_candidate_change") => Disposition::AcceptCandidateChange,
            Some("reject_candidate_change") => Disposition::RejectCandidateChange,
            _ => return Err(Error::InvalidDisposition),
        };
        if fields.next().is_some() {
            return Err(Error::InvalidFraming);
        }
        if previous.is_some_and(|previous| previous >= subject) {
            return Err(Error::NonCanonicalDecisions);
        }
        previous = Some(subject);
        decisions.push(PackagePolicyDecision {
            subject,
            disposition,
        });
    }
    if body != "end\n" {
        return Err(Error::InvalidFraming);
    }
    // Parsing and the existing exact resolver share one monotone allocation
    // budget: parsed choices, comparison index, and returned sorted choices.
    let resolution = super::resolve(changes, comparison, &decisions, &mut budget)?;
    let mut verification = Verification(text);
    framing::emit(&resolution, &mut verification).map_err(|_| Error::InvalidFraming)?;
    if !verification.0.is_empty() {
        return Err(Error::InvalidFraming);
    }
    Ok(resolution)
}

fn field<'text>(body: &mut &'text str, label: &str) -> Result<&'text str, Error> {
    let (line, rest) = body.split_once('\n').ok_or(Error::InvalidFraming)?;
    let (actual, value) = line.split_once(' ').ok_or(Error::InvalidFraming)?;
    if actual != label {
        return Err(Error::InvalidFraming);
    }
    *body = rest;
    Ok(value)
}

struct Verification<'text>(&'text str);
impl Write for Verification<'_> {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        self.0 = self.0.strip_prefix(text).ok_or(std::fmt::Error)?;
        Ok(())
    }
}
