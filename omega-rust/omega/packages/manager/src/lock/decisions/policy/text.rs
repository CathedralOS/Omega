//! Version 2 stores full-comparison subjects, not source-graph row indices.

use super::super::model::HistoricalPackagePolicyRecoveryUsage;
use super::super::text::{disposition_token, parse_digest, parse_number};
use super::{
    Error, HistoricalPackagePolicyDecision, HistoricalPackagePolicyDecisions,
    HistoricalPackagePolicyLimits, Subject,
};
use crate::resolution::graph::CanonicalSourceClosureSubject;
use crate::review::ReviewOnlyRootPolicyDisposition;
use std::fmt::{self, Write};

pub(in super::super) const HEADER: &str = "omega-policy-decisions 2\n";

pub(in super::super) fn encode(
    history: &HistoricalPackagePolicyDecisions,
    limits: HistoricalPackagePolicyLimits,
) -> Result<String, Error> {
    if history.decisions.len() > limits.maximum_decisions {
        return Err(Error::DecisionLimitExceeded);
    }
    let comparison = history.comparison.ok_or(Error::InvalidFraming)?;
    let baseline = history
        .baseline_source_subject
        .map_or_else(|| "none".to_owned(), |digest| Hex(&digest).to_string());
    let prefix = format!(
        "{HEADER}source {}\nbaseline {baseline}\ncomparison {}\ndecisions {}\n",
        history.source_subject.to_hex(),
        Hex(&comparison),
        history.decisions.len()
    );
    let mut length = prefix.len() + "end\n".len();
    let mut previous = None;
    for decision in &history.decisions {
        validate_order(previous, decision.subject)?;
        previous = Some(decision.subject);
        let subject_length = match decision.subject {
            Subject::RootRole => "root-role".len(),
            Subject::SourceReplacement(_) => "source-replacement ".len() + 64,
            Subject::Row(_) => "row ".len() + 64,
            Subject::LegacyConflict { .. } => return Err(Error::NonCanonicalDecisions),
        };
        length = length
            .checked_add(
                "decision ".len()
                    + subject_length
                    + 1
                    + disposition_token(decision.disposition).len()
                    + 1,
            )
            .ok_or(Error::ByteLimitExceeded)?;
    }
    if length > limits.maximum_bytes {
        return Err(Error::ByteLimitExceeded);
    }
    let mut text = String::new();
    text.try_reserve_exact(length)
        .map_err(|_| Error::AllocationFailed)?;
    text.push_str(&prefix);
    for decision in &history.decisions {
        text.push_str("decision ");
        match decision.subject {
            Subject::RootRole => text.push_str("root-role"),
            Subject::SourceReplacement(digest) => {
                write!(text, "source-replacement {}", Hex(&digest)).expect("preallocated text")
            }
            Subject::Row(digest) => {
                write!(text, "row {}", Hex(&digest)).expect("preallocated text")
            }
            Subject::LegacyConflict { .. } => unreachable!("validated modern subject"),
        }
        writeln!(text, " {}", disposition_token(decision.disposition)).expect("preallocated text");
    }
    text.push_str("end\n");
    debug_assert_eq!(text.len(), length);
    Ok(text)
}

pub(in super::super) fn recover(
    text: &str,
    source: &CanonicalSourceClosureSubject,
    limits: HistoricalPackagePolicyLimits,
    maximum_owned_bytes: usize,
) -> Result<
    (
        HistoricalPackagePolicyDecisions,
        HistoricalPackagePolicyRecoveryUsage,
    ),
    Error,
> {
    let mut body = text.strip_prefix(HEADER).ok_or(Error::UnsupportedVersion)?;
    if parse_digest(field(&mut body, "source")?)? != *source.fingerprint().as_bytes() {
        return Err(Error::SourceSubjectMismatch);
    }
    let baseline_source_subject = match field(&mut body, "baseline")? {
        "none" => None,
        digest => Some(parse_digest(digest)?),
    };
    let comparison = parse_digest(field(&mut body, "comparison")?)?;
    let count = parse_number(field(&mut body, "decisions")?)?;
    if count > limits.maximum_decisions {
        return Err(Error::DecisionLimitExceeded);
    }
    if count > body.len() / "decision root-role accept\n".len() {
        return Err(Error::InvalidFraming);
    }
    let usage =
        HistoricalPackagePolicyRecoveryUsage::for_policy_decisions(count, maximum_owned_bytes)?;
    let mut decisions = Vec::new();
    decisions
        .try_reserve_exact(count)
        .map_err(|_| Error::AllocationFailed)?;
    let mut previous = None;
    for _ in 0..count {
        let mut fields = field(&mut body, "decision")?.split(' ');
        let subject = match fields.next() {
            Some("root-role") => Subject::RootRole,
            Some("source-replacement") => Subject::SourceReplacement(parse_digest(
                fields.next().ok_or(Error::InvalidFraming)?,
            )?),
            Some("row") => Subject::Row(parse_digest(fields.next().ok_or(Error::InvalidFraming)?)?),
            _ => return Err(Error::InvalidFraming),
        };
        let disposition = match fields.next() {
            Some("accept") => ReviewOnlyRootPolicyDisposition::AcceptCandidateChange,
            Some("reject") => ReviewOnlyRootPolicyDisposition::RejectCandidateChange,
            _ => return Err(Error::InvalidFraming),
        };
        if fields.next().is_some() {
            return Err(Error::InvalidFraming);
        }
        validate_order(previous, subject)?;
        previous = Some(subject);
        decisions.push(HistoricalPackagePolicyDecision {
            subject,
            disposition,
        });
    }
    if body != "end\n" {
        return Err(Error::InvalidFraming);
    }
    Ok((
        HistoricalPackagePolicyDecisions {
            source_subject: source.fingerprint().clone(),
            baseline_source_subject,
            comparison: Some(comparison),
            decisions,
        },
        usage,
    ))
}

fn field<'text>(body: &mut &'text str, name: &str) -> Result<&'text str, Error> {
    let (line, remaining) = body.split_once('\n').ok_or(Error::InvalidFraming)?;
    let (label, value) = line.split_once(' ').ok_or(Error::InvalidFraming)?;
    if label != name {
        return Err(Error::InvalidFraming);
    }
    *body = remaining;
    Ok(value)
}

fn validate_order(previous: Option<Subject>, subject: Subject) -> Result<(), Error> {
    if matches!(subject, Subject::LegacyConflict { .. })
        || previous.is_some_and(|previous| previous >= subject)
    {
        return Err(Error::NonCanonicalDecisions);
    }
    Ok(())
}

struct Hex<'digest>(&'digest [u8; 32]);
impl fmt::Display for Hex<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}
