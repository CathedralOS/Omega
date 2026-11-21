//! Encode and recover the bounded decision record for one source graph and comparison.

use super::{
    HistoricalPackagePolicyDecision, HistoricalPackagePolicyDecisionSubject as Subject,
    HistoricalPackagePolicyDecisions, HistoricalPackagePolicyError as Error,
    HistoricalPackagePolicyLimits, HistoricalPackagePolicyRecoveryUsage,
};
use crate::resolution::graph::CanonicalSourceClosureSubject;
use crate::review::ReviewOnlyRootPolicyDisposition;
use crate::{Hex, hex_digest};
use std::fmt::Write;

const HEADER: &str = "omega-policy-decisions 2\n";

impl HistoricalPackagePolicyDecisions {
    pub fn canonical_text(
        &self,
        subject: &CanonicalSourceClosureSubject,
        limits: HistoricalPackagePolicyLimits,
    ) -> Result<String, Error> {
        let limits = limits.bounded();
        if self.source_subject() != subject.fingerprint() {
            return Err(Error::SourceSubjectMismatch);
        }
        if self.decisions.len() > limits.maximum_decisions {
            return Err(Error::DecisionLimitExceeded);
        }
        let baseline = self
            .baseline_source_subject
            .map_or_else(|| "none".to_owned(), |digest| Hex(&digest).to_string());
        let prefix = format!(
            "{HEADER}source {}\nbaseline {baseline}\ncomparison {}\ndecisions {}\n",
            self.source_subject.to_hex(),
            Hex(&self.comparison),
            self.decisions.len()
        );
        let mut length = prefix.len() + "end\n".len();
        let mut previous = None;
        for decision in &self.decisions {
            validate_order(previous, decision.subject)?;
            previous = Some(decision.subject);
            let subject_length = match decision.subject {
                Subject::RootRole => "root-role".len(),
                Subject::SourceReplacement(_) => "source-replacement ".len() + 64,
                Subject::Row(_) => "row ".len() + 64,
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
        crate::reserve!(text, length, Error);
        text.push_str(&prefix);
        for decision in &self.decisions {
            text.push_str("decision ");
            match decision.subject {
                Subject::RootRole => text.push_str("root-role"),
                Subject::SourceReplacement(digest) => {
                    write!(text, "source-replacement {}", Hex(&digest)).expect("preallocated text")
                }
                Subject::Row(digest) => {
                    write!(text, "row {}", Hex(&digest)).expect("preallocated text")
                }
            }
            writeln!(text, " {}", disposition_token(decision.disposition))
                .expect("preallocated text");
        }
        text.push_str("end\n");
        debug_assert_eq!(text.len(), length);
        Ok(text)
    }

    pub fn recover_text_with_usage(
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
        let limits = limits.bounded();
        if text.len() > limits.maximum_bytes {
            return Err(Error::ByteLimitExceeded);
        }
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
        crate::reserve!(decisions, count, Error);
        let mut previous = None;
        for _ in 0..count {
            let mut fields = field(&mut body, "decision")?.split(' ');
            let subject = match fields.next() {
                Some("root-role") => Subject::RootRole,
                Some("source-replacement") => Subject::SourceReplacement(parse_digest(
                    fields.next().ok_or(Error::InvalidFraming)?,
                )?),
                Some("row") => {
                    Subject::Row(parse_digest(fields.next().ok_or(Error::InvalidFraming)?)?)
                }
                _ => return Err(Error::InvalidFraming),
            };
            let disposition = parse_disposition(&mut fields)?;
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
                comparison,
                decisions,
            },
            usage,
        ))
    }
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
    if previous.is_some_and(|previous| previous >= subject) {
        return Err(Error::NonCanonicalDecisions);
    }
    Ok(())
}

fn parse_number(text: &str) -> Result<usize, Error> {
    if text.is_empty()
        || (text.len() > 1 && text.starts_with('0'))
        || !text.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(Error::InvalidFraming);
    }
    text.parse().map_err(|_| Error::InvalidFraming)
}

fn parse_digest(text: &str) -> Result<[u8; 32], Error> {
    hex_digest(text).ok_or(Error::InvalidFraming)
}

fn parse_disposition(
    fields: &mut std::str::Split<'_, char>,
) -> Result<ReviewOnlyRootPolicyDisposition, Error> {
    match fields.next() {
        Some("accept") => Ok(ReviewOnlyRootPolicyDisposition::AcceptCandidateChange),
        Some("reject") => Ok(ReviewOnlyRootPolicyDisposition::RejectCandidateChange),
        _ => Err(Error::InvalidFraming),
    }
}

fn disposition_token(disposition: ReviewOnlyRootPolicyDisposition) -> &'static str {
    match disposition {
        ReviewOnlyRootPolicyDisposition::AcceptCandidateChange => "accept",
        ReviewOnlyRootPolicyDisposition::RejectCandidateChange => "reject",
    }
}
