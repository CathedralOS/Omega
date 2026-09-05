use super::model::{
    HistoricalPackagePolicyDecision, HistoricalPackagePolicyDecisionSubject,
    HistoricalPackagePolicyDecisions, HistoricalPackagePolicyError as Error,
    HistoricalPackagePolicyLimits, HistoricalPackagePolicyRecoveryUsage,
};
use crate::resolution::graph::CanonicalSourceClosureSubject;
use crate::review::ReviewOnlyRootPolicyDisposition;

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
        self.canonical_text_with_usage(subject, limits, usize::MAX)
            .map(|(text, _)| text)
    }

    /// Account returned text, validation scratch and canonical recovery under
    /// one caller-owned storage ceiling.
    pub fn canonical_text_with_usage(
        &self,
        subject: &CanonicalSourceClosureSubject,
        limits: HistoricalPackagePolicyLimits,
        maximum_owned_bytes: usize,
    ) -> Result<(String, HistoricalPackagePolicyRecoveryUsage), Error> {
        super::normalized::write::text(self, subject, limits, maximum_owned_bytes)
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
        if text.starts_with(super::normalized::HEADER) {
            return super::normalized::read::text(text, subject, limits, maximum_owned_bytes);
        }
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
                subject: HistoricalPackagePolicyDecisionSubject::CandidatePackage { package_index },
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
                comparison: None,
            },
            usage,
        ))
    }
}

pub(super) fn validate_decisions(
    decisions: &[HistoricalPackagePolicyDecision],
    package_count: usize,
) -> Result<(), Error> {
    if decisions
        .iter()
        .any(|decision| !matches!(decision.subject, HistoricalPackagePolicyDecisionSubject::CandidatePackage { package_index } if package_index < package_count))
    {
        return Err(Error::UnknownPackage);
    }
    if decisions.windows(2).any(|pair| {
        (pair[0].package_index(), pair[0].conflict) >= (pair[1].package_index(), pair[1].conflict)
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

pub(super) fn parse_number(text: &str) -> Result<usize, Error> {
    if text.is_empty()
        || (text.len() > 1 && text.starts_with('0'))
        || !text.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(Error::InvalidFraming);
    }
    text.parse().map_err(|_| Error::InvalidFraming)
}

pub(super) fn parse_digest(text: &str) -> Result<[u8; 32], Error> {
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

pub(super) fn disposition_token(disposition: ReviewOnlyRootPolicyDisposition) -> &'static str {
    match disposition {
        ReviewOnlyRootPolicyDisposition::AcceptCandidateChange => "accept",
        ReviewOnlyRootPolicyDisposition::RejectCandidateChange => "reject",
    }
}
