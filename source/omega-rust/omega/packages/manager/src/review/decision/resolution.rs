use super::model::{ReviewOnlyRootPolicyDecision, ReviewOnlyRootPolicyDisposition};
use crate::review::{
    ReviewOnlyCandidateClosureCommitment, ReviewOnlyCapabilityConflictFingerprint,
    ReviewOnlyCapabilityConflictSet,
};
use sha2::{Digest, Sha256};
use std::fmt;

const ROOT_POLICY_DECISION_SET_DOMAIN: &[u8] = b"OMEGA-PACKAGE-ROOT-POLICY-DECISIONS\0";
const ROOT_POLICY_DECISION_SET_VERSION: u16 = 1;

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
        if decision.candidate_closure() != candidate_closure {
            return Err(ReviewOnlyRootPolicyResolutionError::WrongCandidateClosure {
                expected: candidate_closure,
                actual: decision.candidate_closure(),
            });
        }
        let Ok(index) = known_conflicts
            .binary_search_by_key(&decision.conflict(), |(fingerprint, _)| *fingerprint)
        else {
            return Err(
                ReviewOnlyRootPolicyResolutionError::StaleOrForeignConflict {
                    conflict: decision.conflict(),
                },
            );
        };
        if !known_conflicts[index].1 {
            return Err(ReviewOnlyRootPolicyResolutionError::NonBlockingConflict {
                conflict: decision.conflict(),
            });
        }
        canonical_decisions.push(*decision);
    }
    canonical_decisions.sort_unstable_by_key(|decision| decision.conflict());
    for repeated in canonical_decisions.windows(2) {
        if repeated[0].conflict() == repeated[1].conflict() {
            return Err(ReviewOnlyRootPolicyResolutionError::DuplicateDecision {
                conflict: repeated[0].conflict(),
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
            .is_none_or(|decision| decision.conflict() != *fingerprint)
        {
            return Err(ReviewOnlyRootPolicyResolutionError::MissingDecision {
                conflict: *fingerprint,
            });
        }
        decision_index += 1;
    }

    let all_blocking_rows_accepted = canonical_decisions.iter().all(|decision| {
        decision.disposition() == ReviewOnlyRootPolicyDisposition::AcceptCandidateChange
    });
    let commitment = derive_resolution_commitment(candidate_closure, &canonical_decisions);
    Ok(ReviewOnlyRootPolicyResolution {
        candidate_closure,
        decisions: canonical_decisions,
        commitment,
        all_blocking_rows_accepted,
    })
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
        digest.update(decision.conflict().digest());
        digest.update([disposition_tag(decision.disposition())]);
    }
    ReviewOnlyRootPolicyResolutionCommitment(digest.finalize().into())
}

const fn disposition_tag(disposition: ReviewOnlyRootPolicyDisposition) -> u8 {
    match disposition {
        ReviewOnlyRootPolicyDisposition::AcceptCandidateChange => 0,
        ReviewOnlyRootPolicyDisposition::RejectCandidateChange => 1,
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
