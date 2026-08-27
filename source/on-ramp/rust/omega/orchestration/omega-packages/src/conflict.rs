use crate::record_file::{
    RecordFileError, RecordFileLimits, RecordFileRoot, is_portable_record_file_name,
};
use crate::{
    ReviewOnlyCandidateClosureCommitment, ReviewOnlyCapabilityConflict,
    ReviewOnlyCapabilityConflictFingerprint, ReviewOnlyCapabilityConflictSet,
    ReviewOnlyPackageCapabilityConflicts,
};
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::{Path, PathBuf};

const ROOT_POLICY_DECISION_SET_DOMAIN: &[u8] = b"OMEGA-PACKAGE-ROOT-POLICY-DECISIONS\0";
const ROOT_POLICY_DECISION_SET_VERSION: u16 = 1;
const ROOT_POLICY_RECORD_HEADER: &str = "OMEGA_PACKAGE_ROOT_POLICY_RESOLUTION_V1";
const ROOT_POLICY_ACCEPT_TOKEN: &str = "accept_candidate_change";
const ROOT_POLICY_REJECT_TOKEN: &str = "reject_candidate_change";
const ROOT_POLICY_RECORD_END: &str = "end_root_policy_resolution";
const ROOT_POLICY_NAME_MAXIMUM_BYTES: usize = 255;

/// Root-project policy for one exact blocking candidate change.
///
/// This records only whether that candidate change is permitted. It is not a
/// prompt, reviewer identity, audit receipt, signature, or proof that anyone
/// inspected the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReviewOnlyRootPolicyDisposition {
    AcceptCandidateChange,
    RejectCandidateChange,
}

/// One candidate-bound root-policy decision.
///
/// Construction is available only through the package conflict that owns the
/// exact blocking fingerprint, preventing arbitrary strings or digests from
/// becoming policy decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReviewOnlyRootPolicyDecision {
    candidate_closure: ReviewOnlyCandidateClosureCommitment,
    conflict: ReviewOnlyCapabilityConflictFingerprint,
    disposition: ReviewOnlyRootPolicyDisposition,
}

impl ReviewOnlyRootPolicyDecision {
    pub const fn candidate_closure(&self) -> ReviewOnlyCandidateClosureCommitment {
        self.candidate_closure
    }

    pub const fn conflict(&self) -> ReviewOnlyCapabilityConflictFingerprint {
        self.conflict
    }

    pub const fn disposition(&self) -> ReviewOnlyRootPolicyDisposition {
        self.disposition
    }
}

impl ReviewOnlyPackageCapabilityConflicts {
    /// Bind root policy to one exact blocking conflict in this package.
    pub fn root_policy_decision(
        &self,
        conflict: &ReviewOnlyCapabilityConflict,
        disposition: ReviewOnlyRootPolicyDisposition,
    ) -> Result<ReviewOnlyRootPolicyDecision, ReviewOnlyRootPolicyResolutionError> {
        let Some(owned_conflict) = self
            .conflicts()
            .iter()
            .find(|candidate| candidate.fingerprint() == conflict.fingerprint())
        else {
            return Err(
                ReviewOnlyRootPolicyResolutionError::ConflictDoesNotBelongToPackage {
                    conflict: conflict.fingerprint(),
                },
            );
        };
        if !owned_conflict.is_blocking() {
            return Err(ReviewOnlyRootPolicyResolutionError::NonBlockingConflict {
                conflict: owned_conflict.fingerprint(),
            });
        }
        Ok(ReviewOnlyRootPolicyDecision {
            candidate_closure: self.candidate_closure(),
            conflict: owned_conflict.fingerprint(),
            disposition,
        })
    }
}

/// Commitment to one complete, canonical decision set for one candidate
/// closure. This is policy state only; it cannot mint package evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReviewOnlyRootPolicyResolutionCommitment([u8; 32]);

impl ReviewOnlyRootPolicyResolutionCommitment {
    pub const fn digest(self) -> [u8; 32] {
        self.0
    }
}

/// Complete root-policy treatment of every blocking row in a candidate-bound
/// review conflict set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewOnlyRootPolicyResolution {
    candidate_closure: ReviewOnlyCandidateClosureCommitment,
    decisions: Vec<ReviewOnlyRootPolicyDecision>,
    commitment: ReviewOnlyRootPolicyResolutionCommitment,
    all_blocking_rows_accepted: bool,
}

impl ReviewOnlyRootPolicyResolution {
    pub const fn candidate_closure(&self) -> ReviewOnlyCandidateClosureCommitment {
        self.candidate_closure
    }

    pub fn decisions(&self) -> &[ReviewOnlyRootPolicyDecision] {
        &self.decisions
    }

    pub const fn commitment(&self) -> ReviewOnlyRootPolicyResolutionCommitment {
        self.commitment
    }

    /// Whether root policy permits every exact blocking row in this candidate.
    /// This does not imply that source review, admission, or transaction checks
    /// have completed.
    pub const fn all_blocking_rows_accepted(&self) -> bool {
        self.all_blocking_rows_accepted
    }

    /// Encode this complete review-only policy result in canonical bytes.
    ///
    /// The record is restart-stable policy state, not package evidence or an
    /// accepted-lock record. Recovery must match every fingerprint against a
    /// newly reconstructed conflict set and rerun the ordinary validator.
    pub fn encode_canonical(
        &self,
        limits: ReviewOnlyRootPolicyRecordLimits,
    ) -> Result<Vec<u8>, ReviewOnlyRootPolicyRecordError> {
        if self.decisions.len() > limits.maximum_decisions {
            return Err(ReviewOnlyRootPolicyRecordError::DecisionLimitExceeded {
                declared: self.decisions.len(),
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
        push_digest_hex(&mut record, &self.candidate_closure.digest());
        record.push('\n');
        record.push_str("decision_count ");
        record.push_str(&self.decisions.len().to_string());
        record.push('\n');
        for decision in &self.decisions {
            record.push_str("decision ");
            push_digest_hex(&mut record, &decision.conflict.digest());
            record.push(' ');
            record.push_str(disposition_token(decision.disposition));
            record.push('\n');
        }
        record.push_str("resolution_commitment ");
        push_digest_hex(&mut record, &self.commitment.digest());
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

/// Canonical command-selected filename of an authored root-policy record.
///
/// This is one direct child of an explicitly supplied directory capability;
/// nested paths are intentionally unrepresentable. Trusted command
/// orchestration is responsible for opening the root-owned policy directory.
/// The package manager does not discover it from dependency source, and this
/// type deliberately does not prescribe the final command UX or filename.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReviewOnlyRootPolicyName(String);

impl ReviewOnlyRootPolicyName {
    pub fn parse(value: &str) -> Result<Self, ReviewOnlyRootPolicyNameError> {
        if !is_portable_record_file_name(value, ROOT_POLICY_NAME_MAXIMUM_BYTES) {
            return Err(ReviewOnlyRootPolicyNameError::InvalidName);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewOnlyRootPolicyNameError {
    InvalidName,
}

impl fmt::Display for ReviewOnlyRootPolicyNameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("root-policy filename is not canonical and portable")
    }
}

impl std::error::Error for ReviewOnlyRootPolicyNameError {}

/// Explicit directory-capability root for authored review-only policy state.
///
/// Trusted command orchestration must supply the actual root-owned policy
/// directory; this library cannot infer that role from an arbitrary capability.
/// Persisted bytes still have no standing until recovery matches them against
/// the current compiler-derived conflict set.
#[derive(Debug)]
pub struct ReviewOnlyRootPolicyDirectory {
    root: RecordFileRoot,
}

impl ReviewOnlyRootPolicyDirectory {
    /// Bind an already-open root-owned policy directory.
    ///
    /// `display_path` is diagnostic text only; filesystem operations use only
    /// `directory`. Trusted command orchestration is responsible for acquiring
    /// the capability from the actual invocation root.
    pub fn from_capability(
        directory: cap_std::fs::Dir,
        display_path: impl Into<PathBuf>,
    ) -> Result<Self, ReviewOnlyRootPolicyFileError> {
        let root = RecordFileRoot::from_directory(directory, display_path.into())
            .map_err(map_root_policy_file_error)?;
        Ok(Self { root })
    }

    /// Persist one complete resolution as a new authored project-policy file.
    ///
    /// Existing files are never overwritten. This does not authorize lock or
    /// `build.omg` mutation.
    pub fn persist_new_resolution(
        &self,
        name: &ReviewOnlyRootPolicyName,
        resolution: &ReviewOnlyRootPolicyResolution,
        limits: ReviewOnlyRootPolicyRecordLimits,
    ) -> Result<(), ReviewOnlyRootPolicyFileError> {
        let bytes = resolution
            .encode_canonical(limits)
            .map_err(ReviewOnlyRootPolicyFileError::Record)?;
        self.root
            .write_new(
                name.as_path(),
                &bytes,
                RecordFileLimits {
                    maximum_bytes: limits.maximum_bytes(),
                },
            )
            .map_err(map_root_policy_file_error)
    }

    /// Recover authored policy only against the exact current candidate.
    pub fn recover_resolution(
        &self,
        name: &ReviewOnlyRootPolicyName,
        conflicts: &ReviewOnlyCapabilityConflictSet,
        limits: ReviewOnlyRootPolicyRecordLimits,
    ) -> Result<ReviewOnlyRootPolicyResolution, ReviewOnlyRootPolicyFileError> {
        let mut read = self
            .root
            .read(
                name.as_path(),
                RecordFileLimits {
                    maximum_bytes: limits.maximum_bytes(),
                },
            )
            .map_err(map_root_policy_file_error)?;
        let resolution =
            recover_review_only_root_policy_resolution(conflicts, read.bytes(), limits)
                .map_err(ReviewOnlyRootPolicyFileError::Record)?;
        read.verify_current(RecordFileLimits {
            maximum_bytes: limits.maximum_bytes(),
        })
        .map_err(map_root_policy_file_error)?;
        Ok(resolution)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewOnlyRootPolicyFileError {
    Io { path: PathBuf, message: String },
    InvalidDestination { path: PathBuf },
    NotRegularFile { path: PathBuf },
    DestinationExists { path: PathBuf },
    DirectoryCustodyChanged { path: PathBuf },
    PublishedButUnconfirmed { path: PathBuf, message: String },
    ContentsChanged { path: PathBuf },
    ByteLimitExceeded { actual: u64, maximum: usize },
    LengthOverflow,
    AllocationFailed,
    StageNameSpaceExhausted { directory: PathBuf },
    Record(ReviewOnlyRootPolicyRecordError),
}

impl fmt::Display for ReviewOnlyRootPolicyFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, message } => {
                write!(
                    formatter,
                    "root-policy file `{}`: {message}",
                    path.display()
                )
            }
            Self::InvalidDestination { path } => write!(
                formatter,
                "root-policy destination `{}` is invalid",
                path.display()
            ),
            Self::NotRegularFile { path } => write!(
                formatter,
                "root-policy path `{}` is not a regular confined file",
                path.display()
            ),
            Self::DestinationExists { path } => write!(
                formatter,
                "root-policy destination `{}` already exists",
                path.display()
            ),
            Self::DirectoryCustodyChanged { path } => write!(
                formatter,
                "root-policy directory custody changed at `{}`",
                path.display()
            ),
            Self::PublishedButUnconfirmed { path, message } => write!(
                formatter,
                "root-policy destination `{}` was published but could not be confirmed: {message}",
                path.display()
            ),
            Self::ContentsChanged { path } => write!(
                formatter,
                "root-policy file `{}` changed while it was being recovered",
                path.display()
            ),
            Self::ByteLimitExceeded { actual, maximum } => write!(
                formatter,
                "root-policy file uses {actual} bytes; the limit is {maximum}"
            ),
            Self::LengthOverflow => formatter.write_str("root-policy file length overflow"),
            Self::AllocationFailed => formatter.write_str("root-policy file allocation failed"),
            Self::StageNameSpaceExhausted { directory } => write!(
                formatter,
                "root-policy staging names are exhausted beneath `{}`",
                directory.display()
            ),
            Self::Record(error) => write!(formatter, "invalid root-policy record: {error}"),
        }
    }
}

impl std::error::Error for ReviewOnlyRootPolicyFileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Record(error) => Some(error),
            _ => None,
        }
    }
}

fn map_root_policy_file_error(error: RecordFileError) -> ReviewOnlyRootPolicyFileError {
    match error {
        RecordFileError::Io { path, message } => {
            ReviewOnlyRootPolicyFileError::Io { path, message }
        }
        RecordFileError::InvalidDestination { path } => {
            ReviewOnlyRootPolicyFileError::InvalidDestination { path }
        }
        RecordFileError::NotRegularFile { path } => {
            ReviewOnlyRootPolicyFileError::NotRegularFile { path }
        }
        RecordFileError::DestinationExists { path } => {
            ReviewOnlyRootPolicyFileError::DestinationExists { path }
        }
        RecordFileError::ParentDirectoryChanged { path } => {
            ReviewOnlyRootPolicyFileError::DirectoryCustodyChanged { path }
        }
        RecordFileError::PublishedButUnconfirmed { path, message } => {
            ReviewOnlyRootPolicyFileError::PublishedButUnconfirmed { path, message }
        }
        RecordFileError::ContentsChanged { path } => {
            ReviewOnlyRootPolicyFileError::ContentsChanged { path }
        }
        RecordFileError::ByteLimitExceeded { actual, maximum } => {
            ReviewOnlyRootPolicyFileError::ByteLimitExceeded { actual, maximum }
        }
        RecordFileError::LengthOverflow => ReviewOnlyRootPolicyFileError::LengthOverflow,
        RecordFileError::AllocationFailed => ReviewOnlyRootPolicyFileError::AllocationFailed,
        RecordFileError::StageNameSpaceExhausted { directory } => {
            ReviewOnlyRootPolicyFileError::StageNameSpaceExhausted { directory }
        }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewOnlyRootPolicyResolutionError {
    NoBlockingConflicts,
    EmptyDecisionSet,
    ConflictDoesNotBelongToPackage {
        conflict: ReviewOnlyCapabilityConflictFingerprint,
    },
    NonBlockingConflict {
        conflict: ReviewOnlyCapabilityConflictFingerprint,
    },
    WrongCandidateClosure {
        expected: ReviewOnlyCandidateClosureCommitment,
        actual: ReviewOnlyCandidateClosureCommitment,
    },
    StaleOrForeignConflict {
        conflict: ReviewOnlyCapabilityConflictFingerprint,
    },
    DuplicateConflictFingerprint {
        conflict: ReviewOnlyCapabilityConflictFingerprint,
    },
    DuplicateDecision {
        conflict: ReviewOnlyCapabilityConflictFingerprint,
    },
    TooManyDecisions {
        maximum: usize,
    },
    MissingDecision {
        conflict: ReviewOnlyCapabilityConflictFingerprint,
    },
    AllocationFailed,
}

impl fmt::Display for ReviewOnlyRootPolicyResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoBlockingConflicts => {
                formatter.write_str("root policy was asked to resolve a set with no blocking rows")
            }
            Self::EmptyDecisionSet => {
                formatter.write_str("root policy supplied no blocking-row decisions")
            }
            Self::ConflictDoesNotBelongToPackage { conflict } => write!(
                formatter,
                "conflict {} does not belong to the selected package conflict set",
                fingerprint_hex(*conflict)
            ),
            Self::NonBlockingConflict { conflict } => write!(
                formatter,
                "conflict {} recommends review but is not a root-policy blocker",
                fingerprint_hex(*conflict)
            ),
            Self::WrongCandidateClosure { expected, actual } => write!(
                formatter,
                "root-policy decision belongs to candidate {} rather than {}",
                digest_hex(&actual.digest()),
                digest_hex(&expected.digest())
            ),
            Self::StaleOrForeignConflict { conflict } => write!(
                formatter,
                "root-policy decision references stale or foreign conflict {}",
                fingerprint_hex(*conflict)
            ),
            Self::DuplicateConflictFingerprint { conflict } => write!(
                formatter,
                "review conflict set repeats exact fingerprint {}",
                fingerprint_hex(*conflict)
            ),
            Self::DuplicateDecision { conflict } => write!(
                formatter,
                "root policy repeats a decision for conflict {}",
                fingerprint_hex(*conflict)
            ),
            Self::TooManyDecisions { maximum } => write!(
                formatter,
                "root policy supplied more than the {maximum} known conflict decisions"
            ),
            Self::MissingDecision { conflict } => write!(
                formatter,
                "root policy did not resolve blocking conflict {}",
                fingerprint_hex(*conflict)
            ),
            Self::AllocationFailed => {
                formatter.write_str("root-policy conflict resolution allocation failed")
            }
        }
    }
}

impl std::error::Error for ReviewOnlyRootPolicyResolutionError {}

/// Validate and canonically bind one decision for every exact blocking row.
///
/// Rejections are retained as decisions, so a complete result can still deny
/// the candidate. Non-blocking audit recommendations cannot be converted into
/// policy blockers or rubber-stamp decisions through this API.
pub fn resolve_review_only_root_policy_decisions(
    conflicts: &ReviewOnlyCapabilityConflictSet,
    decisions: &[ReviewOnlyRootPolicyDecision],
) -> Result<ReviewOnlyRootPolicyResolution, ReviewOnlyRootPolicyResolutionError> {
    let Some(first_package) = conflicts.packages().first() else {
        return Err(ReviewOnlyRootPolicyResolutionError::NoBlockingConflicts);
    };
    let candidate_closure = first_package.candidate_closure();

    let mut known_conflicts = Vec::new();
    known_conflicts
        .try_reserve_exact(conflicts.conflict_count())
        .map_err(|_| ReviewOnlyRootPolicyResolutionError::AllocationFailed)?;
    for package in conflicts.packages() {
        if package.candidate_closure() != candidate_closure {
            return Err(ReviewOnlyRootPolicyResolutionError::WrongCandidateClosure {
                expected: candidate_closure,
                actual: package.candidate_closure(),
            });
        }
        known_conflicts.extend(
            package
                .conflicts()
                .iter()
                .map(|conflict| (conflict.fingerprint(), conflict.is_blocking())),
        );
    }
    known_conflicts.sort_unstable_by_key(|(fingerprint, _)| *fingerprint);
    for repeated in known_conflicts.windows(2) {
        if repeated[0].0 == repeated[1].0 {
            return Err(
                ReviewOnlyRootPolicyResolutionError::DuplicateConflictFingerprint {
                    conflict: repeated[0].0,
                },
            );
        }
    }

    let blocking_count = known_conflicts
        .iter()
        .filter(|(_, is_blocking)| *is_blocking)
        .count();
    if blocking_count == 0 {
        return Err(ReviewOnlyRootPolicyResolutionError::NoBlockingConflicts);
    }
    if decisions.is_empty() {
        return Err(ReviewOnlyRootPolicyResolutionError::EmptyDecisionSet);
    }
    if decisions.len() > blocking_count {
        return Err(ReviewOnlyRootPolicyResolutionError::TooManyDecisions {
            maximum: blocking_count,
        });
    }

    let mut canonical_decisions = Vec::new();
    canonical_decisions
        .try_reserve_exact(decisions.len())
        .map_err(|_| ReviewOnlyRootPolicyResolutionError::AllocationFailed)?;
    for decision in decisions {
        if decision.candidate_closure != candidate_closure {
            return Err(ReviewOnlyRootPolicyResolutionError::WrongCandidateClosure {
                expected: candidate_closure,
                actual: decision.candidate_closure,
            });
        }
        let Ok(index) = known_conflicts
            .binary_search_by_key(&decision.conflict, |(fingerprint, _)| *fingerprint)
        else {
            return Err(
                ReviewOnlyRootPolicyResolutionError::StaleOrForeignConflict {
                    conflict: decision.conflict,
                },
            );
        };
        if !known_conflicts[index].1 {
            return Err(ReviewOnlyRootPolicyResolutionError::NonBlockingConflict {
                conflict: decision.conflict,
            });
        }
        canonical_decisions.push(*decision);
    }
    canonical_decisions.sort_unstable_by_key(|decision| decision.conflict);
    for repeated in canonical_decisions.windows(2) {
        if repeated[0].conflict == repeated[1].conflict {
            return Err(ReviewOnlyRootPolicyResolutionError::DuplicateDecision {
                conflict: repeated[0].conflict,
            });
        }
    }

    let mut decision_index = 0usize;
    for (fingerprint, is_blocking) in &known_conflicts {
        if !is_blocking {
            continue;
        }
        if canonical_decisions
            .get(decision_index)
            .is_none_or(|decision| decision.conflict != *fingerprint)
        {
            return Err(ReviewOnlyRootPolicyResolutionError::MissingDecision {
                conflict: *fingerprint,
            });
        }
        decision_index += 1;
    }

    let all_blocking_rows_accepted = canonical_decisions.iter().all(|decision| {
        decision.disposition == ReviewOnlyRootPolicyDisposition::AcceptCandidateChange
    });
    let commitment = derive_resolution_commitment(candidate_closure, &canonical_decisions);
    Ok(ReviewOnlyRootPolicyResolution {
        candidate_closure,
        decisions: canonical_decisions,
        commitment,
        all_blocking_rows_accepted,
    })
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
        .and_then(|length| length.checked_add(decimal_digits(resolution.decisions.len())))
        .and_then(|length| length.checked_add(1))
        .and_then(|length| length.checked_add("resolution_commitment ".len() + 64 + 1))
        .and_then(|length| length.checked_add(ROOT_POLICY_RECORD_END.len() + 1))
        .ok_or(ReviewOnlyRootPolicyRecordError::LengthOverflow)?;
    resolution
        .decisions
        .iter()
        .try_fold(fixed, |length, decision| {
            length
                .checked_add(
                    "decision ".len() + 64 + 1 + disposition_token(decision.disposition).len() + 1,
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
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
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

fn derive_resolution_commitment(
    candidate_closure: ReviewOnlyCandidateClosureCommitment,
    decisions: &[ReviewOnlyRootPolicyDecision],
) -> ReviewOnlyRootPolicyResolutionCommitment {
    let mut digest = Sha256::new();
    digest.update(ROOT_POLICY_DECISION_SET_DOMAIN);
    digest.update(ROOT_POLICY_DECISION_SET_VERSION.to_le_bytes());
    digest.update(candidate_closure.digest());
    digest.update(
        u64::try_from(decisions.len())
            .expect("bounded conflict count fits u64")
            .to_le_bytes(),
    );
    for decision in decisions {
        digest.update(decision.conflict.digest());
        digest.update([disposition_tag(decision.disposition)]);
    }
    ReviewOnlyRootPolicyResolutionCommitment(digest.finalize().into())
}

const fn disposition_tag(disposition: ReviewOnlyRootPolicyDisposition) -> u8 {
    match disposition {
        ReviewOnlyRootPolicyDisposition::AcceptCandidateChange => 0,
        ReviewOnlyRootPolicyDisposition::RejectCandidateChange => 1,
    }
}

const fn disposition_token(disposition: ReviewOnlyRootPolicyDisposition) -> &'static str {
    match disposition {
        ReviewOnlyRootPolicyDisposition::AcceptCandidateChange => ROOT_POLICY_ACCEPT_TOKEN,
        ReviewOnlyRootPolicyDisposition::RejectCandidateChange => ROOT_POLICY_REJECT_TOKEN,
    }
}

fn fingerprint_hex(fingerprint: ReviewOnlyCapabilityConflictFingerprint) -> String {
    digest_hex(&fingerprint.digest())
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
