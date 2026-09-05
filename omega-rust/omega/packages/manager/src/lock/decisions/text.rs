use super::model::{
    HistoricalPackagePolicyDecision, HistoricalPackagePolicyDecisions,
    HistoricalPackagePolicyError as Error, HistoricalPackagePolicyLimits,
    HistoricalPackagePolicyRecoveryUsage,
};
use crate::resolution::graph::CanonicalSourceClosureSubject;
use crate::review::ReviewOnlyRootPolicyDisposition;
use std::fmt::Write;

const HEADER: &str = "omega-policy-decisions 1\n";

impl HistoricalPackagePolicyDecisions {
    /// Canonical ASCII section. Package indices reference the complete sorted
    /// source graph bound by `source`; fingerprints identify historical changes,
    /// not the full normalized capability baseline required beside this section.
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
        validate_decisions(&self.decisions, subject.packages().len())?;
        let prefix = format!(
            "{HEADER}source {}\ndecisions {}\n",
            self.source_subject.to_hex(),
            self.decisions.len()
        );
        let length =
            self.decisions
                .iter()
                .try_fold(prefix.len() + "end\n".len(), |length, decision| {
                    length
                        .checked_add(
                            "decision ".len()
                                + decimal_digits(decision.package_index)
                                + 1
                                + 64
                                + 1
                                + disposition_token(decision.disposition).len()
                                + 1,
                        )
                        .ok_or(Error::ByteLimitExceeded)
                })?;
        if length > limits.maximum_bytes {
            return Err(Error::ByteLimitExceeded);
        }
        let mut text = String::new();
        text.try_reserve_exact(length)
            .map_err(|_| Error::AllocationFailed)?;
        text.push_str(&prefix);
        for decision in &self.decisions {
            write!(text, "decision {} ", decision.package_index)
                .expect("write preallocated string");
            for byte in decision.conflict {
                write!(text, "{byte:02x}").expect("write preallocated string");
            }
            writeln!(text, " {}", disposition_token(decision.disposition))
                .expect("write preallocated string");
        }
        text.push_str("end\n");
        debug_assert_eq!(text.len(), length);
        Ok(text)
    }

    /// Recover historical project policy using only the retained source graph.
    /// No old checkout, compiler, conflict set, or proof of acceptance is needed.
    /// The result cannot be supplied as a fresh root-policy resolution.
    pub fn recover_text(
        text: &str,
        subject: &CanonicalSourceClosureSubject,
        limits: HistoricalPackagePolicyLimits,
    ) -> Result<Self, Error> {
        Self::recover_text_with_usage(text, subject, limits, usize::MAX)
            .map(|(decisions, _)| decisions)
    }

    /// Recover one historical section under the remaining enclosing storage
    /// budget. Successful usage must be deducted before recovering another
    /// section; it grants no current candidate or publication authority.
    pub fn recover_text_with_usage(
        text: &str,
        subject: &CanonicalSourceClosureSubject,
        limits: HistoricalPackagePolicyLimits,
        maximum_owned_bytes: usize,
    ) -> Result<(Self, HistoricalPackagePolicyRecoveryUsage), Error> {
        let limits = limits.bounded();
        if text.len() > limits.maximum_bytes {
            return Err(Error::ByteLimitExceeded);
        }
        let body = text.strip_prefix(HEADER).ok_or(Error::UnsupportedVersion)?;
        let (source_line, body) = body.split_once('\n').ok_or(Error::InvalidFraming)?;
        if source_line
            .strip_prefix("source ")
            .and_then(|value| parse_digest(value).ok())
            .as_ref()
            != Some(subject.fingerprint().as_bytes())
        {
            return Err(Error::SourceSubjectMismatch);
        }
        let (count_line, body) = body.split_once('\n').ok_or(Error::InvalidFraming)?;
        let count = parse_number(
            count_line
                .strip_prefix("decisions ")
                .ok_or(Error::InvalidFraming)?,
        )?;
        if count > limits.maximum_decisions {
            return Err(Error::DecisionLimitExceeded);
        }
        // Reject impossible counts before allocating any row storage. Each
        // canonical decision occupies at least 83 bytes, including its newline.
        if count > body.len() / 83 {
            return Err(Error::InvalidFraming);
        }
        // Account both allocations before reserving even the retained rows.
        // Validation below uses one fixed digest per row and no owned keys.
        let usage =
            HistoricalPackagePolicyRecoveryUsage::for_decisions(count, maximum_owned_bytes)?;
        let mut decisions = Vec::new();
        decisions
            .try_reserve_exact(count)
            .map_err(|_| Error::AllocationFailed)?;
        let mut lines = body.split('\n');
        for _ in 0..count {
            let line = lines.next().ok_or(Error::InvalidFraming)?;
            let mut fields = line.split(' ');
            if fields.next() != Some("decision") {
                return Err(Error::InvalidFraming);
            }
            let package_index = parse_number(fields.next().ok_or(Error::InvalidFraming)?)?;
            if package_index >= subject.packages().len() {
                return Err(Error::UnknownPackage);
            }
            let conflict = parse_digest(fields.next().ok_or(Error::InvalidFraming)?)?;
            let disposition = match fields.next() {
                Some("accept") => ReviewOnlyRootPolicyDisposition::AcceptCandidateChange,
                Some("reject") => ReviewOnlyRootPolicyDisposition::RejectCandidateChange,
                _ => return Err(Error::InvalidFraming),
            };
            if fields.next().is_some() {
                return Err(Error::InvalidFraming);
            }
            decisions.push(HistoricalPackagePolicyDecision {
                package_index,
                conflict,
                disposition,
            });
        }
        if lines.next() != Some("end") || lines.next() != Some("") || lines.next().is_some() {
            return Err(Error::InvalidFraming);
        }
        validate_decisions(&decisions, subject.packages().len())?;
        Ok((
            Self {
                source_subject: subject.fingerprint().clone(),
                decisions,
            },
            usage,
        ))
    }
}

fn validate_decisions(
    decisions: &[HistoricalPackagePolicyDecision],
    package_count: usize,
) -> Result<(), Error> {
    if decisions
        .iter()
        .any(|decision| decision.package_index >= package_count)
    {
        return Err(Error::UnknownPackage);
    }
    if decisions.windows(2).any(|pair| {
        (pair[0].package_index, pair[0].conflict) >= (pair[1].package_index, pair[1].conflict)
    }) {
        return Err(Error::NonCanonicalDecisions);
    }
    // One exact conflict has one owner even when a record tries to repeat it
    // under another valid package index.
    let mut fingerprints = Vec::new();
    fingerprints
        .try_reserve_exact(decisions.len())
        .map_err(|_| Error::AllocationFailed)?;
    fingerprints.extend(decisions.iter().map(|decision| decision.conflict));
    fingerprints.sort_unstable();
    if fingerprints.windows(2).any(|pair| pair[0] == pair[1]) {
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
    if text.len() != 64 {
        return Err(Error::InvalidFraming);
    }
    let mut digest = [0; 32];
    for (index, pair) in text.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        digest[index] = nibble(pair[0])? * 16 + nibble(pair[1])?;
    }
    Ok(digest)
}

fn nibble(byte: u8) -> Result<u8, Error> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(Error::InvalidFraming),
    }
}

fn decimal_digits(mut number: usize) -> usize {
    let mut digits = 1;
    while number >= 10 {
        number /= 10;
        digits += 1;
    }
    digits
}

fn disposition_token(disposition: ReviewOnlyRootPolicyDisposition) -> &'static str {
    match disposition {
        ReviewOnlyRootPolicyDisposition::AcceptCandidateChange => "accept",
        ReviewOnlyRootPolicyDisposition::RejectCandidateChange => "reject",
    }
}
