use super::model::ReviewOnlyRootPolicyDisposition;
use super::resolution::{
    ReviewOnlyRootPolicyResolution, ReviewOnlyRootPolicyResolutionError,
    resolve_review_only_root_policy_decisions,
};
use crate::review::ReviewOnlyCapabilityConflictSet;
use std::fmt;

const ROOT_POLICY_RECORD_HEADER: &str = "OMEGA_PACKAGE_ROOT_POLICY_RESOLUTION_V1";
const ROOT_POLICY_ACCEPT_TOKEN: &str = "accept_candidate_change";
const ROOT_POLICY_REJECT_TOKEN: &str = "reject_candidate_change";
const ROOT_POLICY_RECORD_END: &str = "end_root_policy_resolution";

impl ReviewOnlyRootPolicyResolution {
    /// Encode this complete review-only policy result in canonical bytes.
    ///
    /// The record is restart-stable policy state, not package evidence or an
    /// accepted-lock record. Recovery must match every fingerprint against a
    /// newly reconstructed conflict set and rerun the ordinary validator.
    pub fn encode_canonical(
        &self,
        limits: ReviewOnlyRootPolicyRecordLimits,
    ) -> Result<Vec<u8>, ReviewOnlyRootPolicyRecordError> {
        if self.decisions().len() > limits.maximum_decisions {
            return Err(ReviewOnlyRootPolicyRecordError::DecisionLimitExceeded {
                declared: self.decisions().len(),
                maximum: limits.maximum_decisions,
            });
        }
        let encoded_length = root_policy_record_encoded_length(self)?;
        if encoded_length > limits.maximum_bytes {
            return Err(ReviewOnlyRootPolicyRecordError::ByteLimitExceeded {
                length: encoded_length,
                maximum: limits.maximum_bytes,
            });
        }
        let mut record = String::new();
        record
            .try_reserve_exact(encoded_length)
            .map_err(|_| ReviewOnlyRootPolicyRecordError::AllocationFailed)?;
        record.push_str(ROOT_POLICY_RECORD_HEADER);
        record.push('\n');
        record.push_str("candidate_closure ");
        push_digest_hex(&mut record, &self.candidate_closure().digest());
        record.push('\n');
        record.push_str("decision_count ");
        record.push_str(&self.decisions().len().to_string());
        record.push('\n');
        for decision in self.decisions() {
            record.push_str("decision ");
            push_digest_hex(&mut record, &decision.conflict().digest());
            record.push(' ');
            record.push_str(disposition_token(decision.disposition()));
            record.push('\n');
        }
        record.push_str("resolution_commitment ");
        push_digest_hex(&mut record, &self.commitment().digest());
        record.push('\n');
        record.push_str(ROOT_POLICY_RECORD_END);
        record.push('\n');
        debug_assert_eq!(record.len(), encoded_length);
        Ok(record.into_bytes())
    }
}

/// Independent byte and row ceilings for restart-stable root-policy records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReviewOnlyRootPolicyRecordLimits {
    maximum_bytes: usize,
    maximum_decisions: usize,
    maximum_conflicts: usize,
}

impl ReviewOnlyRootPolicyRecordLimits {
    pub const fn new(
        maximum_bytes: usize,
        maximum_decisions: usize,
        maximum_conflicts: usize,
    ) -> Self {
        Self {
            maximum_bytes,
            maximum_decisions,
            maximum_conflicts,
        }
    }

    pub const fn maximum_bytes(self) -> usize {
        self.maximum_bytes
    }

    pub const fn maximum_decisions(self) -> usize {
        self.maximum_decisions
    }

    pub const fn maximum_conflicts(self) -> usize {
        self.maximum_conflicts
    }
}

impl Default for ReviewOnlyRootPolicyRecordLimits {
    fn default() -> Self {
        Self::new(8 * 1024 * 1024, 65_536, 65_536)
    }
}
/// Closed failure vocabulary for canonical root-policy encoding and recovery.
///
/// Parsed record text is never included in diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewOnlyRootPolicyRecordError {
    AllocationFailed,
    LengthOverflow,
    ByteLimitExceeded {
        length: usize,
        maximum: usize,
    },
    DecisionLimitExceeded {
        declared: usize,
        maximum: usize,
    },
    ConflictLimitExceeded {
        current: usize,
        maximum: usize,
    },
    InvalidUtf8,
    InvalidHeader,
    InvalidFraming,
    InvalidCandidateClosure,
    InvalidDecisionCount,
    InvalidDecision,
    InvalidFingerprint,
    InvalidDisposition,
    InvalidCommitment,
    CandidateClosureMismatch {
        expected: [u8; 32],
        actual: [u8; 32],
    },
    UnknownConflictFingerprint {
        digest: [u8; 32],
    },
    CommitmentMismatch {
        expected: [u8; 32],
        actual: [u8; 32],
    },
    NonCanonicalEncoding,
    Resolution(ReviewOnlyRootPolicyResolutionError),
}

impl fmt::Display for ReviewOnlyRootPolicyRecordError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllocationFailed => formatter.write_str("root-policy record allocation failed"),
            Self::LengthOverflow => formatter.write_str("root-policy record length overflow"),
            Self::ByteLimitExceeded { length, maximum } => write!(
                formatter,
                "root-policy record uses {length} bytes; the limit is {maximum}"
            ),
            Self::DecisionLimitExceeded { declared, maximum } => write!(
                formatter,
                "root-policy record declares {declared} decisions; the limit is {maximum}"
            ),
            Self::ConflictLimitExceeded { current, maximum } => write!(
                formatter,
                "root-policy recovery has {current} current conflicts; the limit is {maximum}"
            ),
            Self::InvalidUtf8 => formatter.write_str("root-policy record is not UTF-8"),
            Self::InvalidHeader => formatter.write_str("invalid root-policy record header"),
            Self::InvalidFraming => formatter.write_str("invalid root-policy record framing"),
            Self::InvalidCandidateClosure => {
                formatter.write_str("invalid root-policy candidate closure")
            }
            Self::InvalidDecisionCount => formatter.write_str("invalid root-policy decision count"),
            Self::InvalidDecision => formatter.write_str("invalid root-policy decision row"),
            Self::InvalidFingerprint => {
                formatter.write_str("invalid root-policy conflict fingerprint")
            }
            Self::InvalidDisposition => {
                formatter.write_str("root-policy record has an invalid disposition")
            }
            Self::InvalidCommitment => formatter.write_str("invalid root-policy commitment"),
            Self::CandidateClosureMismatch { expected, actual } => write!(
                formatter,
                "root-policy record belongs to candidate {} rather than {}",
                digest_hex(actual),
                digest_hex(expected)
            ),
            Self::UnknownConflictFingerprint { digest } => write!(
                formatter,
                "root-policy record references unknown conflict {}",
                digest_hex(digest)
            ),
            Self::CommitmentMismatch { expected, actual } => write!(
                formatter,
                "root-policy record commitment {} does not match reconstructed {}",
                digest_hex(actual),
                digest_hex(expected)
            ),
            Self::NonCanonicalEncoding => {
                formatter.write_str("root-policy record is not canonically encoded")
            }
            Self::Resolution(error) => write!(formatter, "invalid root-policy decisions: {error}"),
        }
    }
}

impl std::error::Error for ReviewOnlyRootPolicyRecordError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resolution(error) => Some(error),
            _ => None,
        }
    }
}
/// Recover restart-stable root policy only against the exact live conflict set.
///
/// Parsed fingerprints never become freestanding decisions. Each row is
/// matched back to its owning compiler-derived conflict, constructed through
/// that package's decision API, and passed through complete resolution again.
pub fn recover_review_only_root_policy_resolution(
    conflicts: &ReviewOnlyCapabilityConflictSet,
    bytes: &[u8],
    limits: ReviewOnlyRootPolicyRecordLimits,
) -> Result<ReviewOnlyRootPolicyResolution, ReviewOnlyRootPolicyRecordError> {
    if bytes.len() > limits.maximum_bytes {
        return Err(ReviewOnlyRootPolicyRecordError::ByteLimitExceeded {
            length: bytes.len(),
            maximum: limits.maximum_bytes,
        });
    }
    if conflicts.conflict_count() > limits.maximum_conflicts {
        return Err(ReviewOnlyRootPolicyRecordError::ConflictLimitExceeded {
            current: conflicts.conflict_count(),
            maximum: limits.maximum_conflicts,
        });
    }
    let text =
        std::str::from_utf8(bytes).map_err(|_| ReviewOnlyRootPolicyRecordError::InvalidUtf8)?;
    let Some(body) = text.strip_suffix('\n') else {
        return Err(ReviewOnlyRootPolicyRecordError::InvalidFraming);
    };
    if body.contains('\r') {
        return Err(ReviewOnlyRootPolicyRecordError::InvalidFraming);
    }
    let mut lines = body.split('\n');
    if lines.next() != Some(ROOT_POLICY_RECORD_HEADER) {
        return Err(ReviewOnlyRootPolicyRecordError::InvalidHeader);
    }

    let candidate_closure = parse_prefixed_digest(
        lines.next(),
        "candidate_closure ",
        ReviewOnlyRootPolicyRecordError::InvalidCandidateClosure,
    )?;
    let decision_count = parse_canonical_usize(
        lines
            .next()
            .and_then(|line| line.strip_prefix("decision_count "))
            .ok_or(ReviewOnlyRootPolicyRecordError::InvalidDecisionCount)?,
    )
    .ok_or(ReviewOnlyRootPolicyRecordError::InvalidDecisionCount)?;
    if decision_count > limits.maximum_decisions {
        return Err(ReviewOnlyRootPolicyRecordError::DecisionLimitExceeded {
            declared: decision_count,
            maximum: limits.maximum_decisions,
        });
    }
    if decision_count > conflicts.conflict_count() {
        return Err(ReviewOnlyRootPolicyRecordError::InvalidDecisionCount);
    }

    let Some(first_package) = conflicts.packages().first() else {
        return Err(ReviewOnlyRootPolicyRecordError::Resolution(
            ReviewOnlyRootPolicyResolutionError::NoBlockingConflicts,
        ));
    };
    let expected_candidate = first_package.candidate_closure();
    if candidate_closure != expected_candidate.digest() {
        return Err(ReviewOnlyRootPolicyRecordError::CandidateClosureMismatch {
            expected: expected_candidate.digest(),
            actual: candidate_closure,
        });
    }

    let mut known_conflicts = Vec::new();
    known_conflicts
        .try_reserve_exact(conflicts.conflict_count())
        .map_err(|_| ReviewOnlyRootPolicyRecordError::AllocationFailed)?;
    for package in conflicts.packages() {
        for conflict in package.conflicts() {
            known_conflicts.push((conflict.fingerprint(), package, conflict));
        }
    }
    known_conflicts.sort_unstable_by_key(|(fingerprint, _, _)| *fingerprint);

    let mut decisions = Vec::new();
    decisions
        .try_reserve_exact(decision_count)
        .map_err(|_| ReviewOnlyRootPolicyRecordError::AllocationFailed)?;
    for _ in 0..decision_count {
        let line = lines
            .next()
            .ok_or(ReviewOnlyRootPolicyRecordError::InvalidDecision)?;
        let remainder = line
            .strip_prefix("decision ")
            .ok_or(ReviewOnlyRootPolicyRecordError::InvalidDecision)?;
        let Some((fingerprint_text, disposition_text)) = remainder.split_once(' ') else {
            return Err(ReviewOnlyRootPolicyRecordError::InvalidDecision);
        };
        if disposition_text.contains(' ') {
            return Err(ReviewOnlyRootPolicyRecordError::InvalidDecision);
        }
        let fingerprint = parse_digest(fingerprint_text)
            .ok_or(ReviewOnlyRootPolicyRecordError::InvalidFingerprint)?;
        let disposition = parse_disposition(disposition_text)?;
        let index = known_conflicts
            .binary_search_by(|(known, _, _)| known.digest().cmp(&fingerprint))
            .map_err(
                |_| ReviewOnlyRootPolicyRecordError::UnknownConflictFingerprint {
                    digest: fingerprint,
                },
            )?;
        let (_, package, conflict) = known_conflicts[index];
        decisions.push(
            package
                .root_policy_decision(conflict, disposition)
                .map_err(ReviewOnlyRootPolicyRecordError::Resolution)?,
        );
    }

    let encoded_commitment = parse_prefixed_digest(
        lines.next(),
        "resolution_commitment ",
        ReviewOnlyRootPolicyRecordError::InvalidCommitment,
    )?;
    if lines.next() != Some(ROOT_POLICY_RECORD_END) || lines.next().is_some() {
        return Err(ReviewOnlyRootPolicyRecordError::InvalidFraming);
    }

    let resolution = resolve_review_only_root_policy_decisions(conflicts, &decisions)
        .map_err(ReviewOnlyRootPolicyRecordError::Resolution)?;
    let expected_commitment = resolution.commitment().digest();
    if encoded_commitment != expected_commitment {
        return Err(ReviewOnlyRootPolicyRecordError::CommitmentMismatch {
            expected: expected_commitment,
            actual: encoded_commitment,
        });
    }
    if resolution.encode_canonical(limits)?.as_slice() != bytes {
        return Err(ReviewOnlyRootPolicyRecordError::NonCanonicalEncoding);
    }
    Ok(resolution)
}

fn parse_prefixed_digest(
    line: Option<&str>,
    prefix: &str,
    error: ReviewOnlyRootPolicyRecordError,
) -> Result<[u8; 32], ReviewOnlyRootPolicyRecordError> {
    line.and_then(|line| line.strip_prefix(prefix))
        .and_then(parse_digest)
        .ok_or(error)
}

fn root_policy_record_encoded_length(
    resolution: &ReviewOnlyRootPolicyResolution,
) -> Result<usize, ReviewOnlyRootPolicyRecordError> {
    let fixed = ROOT_POLICY_RECORD_HEADER
        .len()
        .checked_add(1)
        .and_then(|length| length.checked_add("candidate_closure ".len() + 64 + 1))
        .and_then(|length| length.checked_add("decision_count ".len()))
        .and_then(|length| length.checked_add(decimal_digits(resolution.decisions().len())))
        .and_then(|length| length.checked_add(1))
        .and_then(|length| length.checked_add("resolution_commitment ".len() + 64 + 1))
        .and_then(|length| length.checked_add(ROOT_POLICY_RECORD_END.len() + 1))
        .ok_or(ReviewOnlyRootPolicyRecordError::LengthOverflow)?;
    resolution
        .decisions()
        .iter()
        .try_fold(fixed, |length, decision| {
            length
                .checked_add(
                    "decision ".len()
                        + 64
                        + 1
                        + disposition_token(decision.disposition()).len()
                        + 1,
                )
                .ok_or(ReviewOnlyRootPolicyRecordError::LengthOverflow)
        })
}

const fn decimal_digits(mut value: usize) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

fn parse_digest(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut digest = [0u8; 32];
    for (index, pair) in value.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        digest[index] = (lower_hex_nibble(pair[0])? << 4) | lower_hex_nibble(pair[1])?;
    }
    Some(digest)
}

const fn lower_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn parse_canonical_usize(value: &str) -> Option<usize> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    value.parse().ok()
}

fn parse_disposition(
    value: &str,
) -> Result<ReviewOnlyRootPolicyDisposition, ReviewOnlyRootPolicyRecordError> {
    match value {
        ROOT_POLICY_ACCEPT_TOKEN => Ok(ReviewOnlyRootPolicyDisposition::AcceptCandidateChange),
        ROOT_POLICY_REJECT_TOKEN => Ok(ReviewOnlyRootPolicyDisposition::RejectCandidateChange),
        _ => Err(ReviewOnlyRootPolicyRecordError::InvalidDisposition),
    }
}

const fn disposition_token(disposition: ReviewOnlyRootPolicyDisposition) -> &'static str {
    match disposition {
        ReviewOnlyRootPolicyDisposition::AcceptCandidateChange => ROOT_POLICY_ACCEPT_TOKEN,
        ReviewOnlyRootPolicyDisposition::RejectCandidateChange => ROOT_POLICY_REJECT_TOKEN,
    }
}

fn digest_hex(digest: &[u8; 32]) -> String {
    let mut encoded = String::with_capacity(64);
    push_digest_hex(&mut encoded, digest);
    encoded
}

fn push_digest_hex(encoded: &mut String, digest: &[u8; 32]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in digest {
        encoded.push(HEX[usize::from(byte >> 4)] as char);
        encoded.push(HEX[usize::from(byte & 0x0f)] as char);
    }
}
