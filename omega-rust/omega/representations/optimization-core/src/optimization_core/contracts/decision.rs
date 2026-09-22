use super::CoreContractDecodeError;

/// Independent validation strength required before a candidate may commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum OptimizationSafetyClass {
    StructuralIdentity = 1,
    ExactOperationSemantics = 2,
    ProofCertified = 3,
    OwnershipCertified = 4,
    TranslationValidated = 5,
}

impl OptimizationSafetyClass {
    pub const fn encode(self) -> [u8; 1] {
        [self as u8]
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, CoreContractDecodeError> {
        let [tag] = encoded else {
            return Err(CoreContractDecodeError::WrongLength {
                expected: 1,
                actual: encoded.len(),
            });
        };
        match tag {
            1 => Ok(Self::StructuralIdentity),
            2 => Ok(Self::ExactOperationSemantics),
            3 => Ok(Self::ProofCertified),
            4 => Ok(Self::OwnershipCertified),
            5 => Ok(Self::TranslationValidated),
            tag => Err(CoreContractDecodeError::UnknownSafetyClass(*tag)),
        }
    }
}

/// Stable, machine-readable explanation for a deterministic pass decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum OptimizationReasonCode {
    Inapplicable = 1,
    NotProfitable = 2,
    MissingAnalysis = 3,
    UnsupportedVocabulary = 4,
    ValidationFailed = 5,
    ProofIncomplete = 6,
    OwnershipEvidenceIncomplete = 7,
    WorkBudgetExhausted = 8,
    CandidateLimitReached = 9,
    ConvergenceLimitReached = 10,
    PolicyRejected = 11,
    Superseded = 12,
}

impl OptimizationReasonCode {
    pub const ALL: [Self; 12] = [
        Self::Inapplicable,
        Self::NotProfitable,
        Self::MissingAnalysis,
        Self::UnsupportedVocabulary,
        Self::ValidationFailed,
        Self::ProofIncomplete,
        Self::OwnershipEvidenceIncomplete,
        Self::WorkBudgetExhausted,
        Self::CandidateLimitReached,
        Self::ConvergenceLimitReached,
        Self::PolicyRejected,
        Self::Superseded,
    ];

    const fn from_tag(tag: u8) -> Result<Self, CoreContractDecodeError> {
        match tag {
            1 => Ok(Self::Inapplicable),
            2 => Ok(Self::NotProfitable),
            3 => Ok(Self::MissingAnalysis),
            4 => Ok(Self::UnsupportedVocabulary),
            5 => Ok(Self::ValidationFailed),
            6 => Ok(Self::ProofIncomplete),
            7 => Ok(Self::OwnershipEvidenceIncomplete),
            8 => Ok(Self::WorkBudgetExhausted),
            9 => Ok(Self::CandidateLimitReached),
            10 => Ok(Self::ConvergenceLimitReached),
            11 => Ok(Self::PolicyRejected),
            12 => Ok(Self::Superseded),
            tag => Err(CoreContractDecodeError::UnknownReasonCode(tag)),
        }
    }
}

/// Final independent disposition of one proposed rewrite candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OptimizationCandidateVerdict {
    Applied,
    Skipped(OptimizationReasonCode),
    Rejected(OptimizationReasonCode),
}

impl OptimizationCandidateVerdict {
    pub const fn encode(self) -> [u8; 2] {
        match self {
            Self::Applied => [1, 0],
            Self::Skipped(reason) => [2, reason as u8],
            Self::Rejected(reason) => [3, reason as u8],
        }
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, CoreContractDecodeError> {
        let [disposition, reason] = encoded else {
            return Err(CoreContractDecodeError::WrongLength {
                expected: 2,
                actual: encoded.len(),
            });
        };
        match (*disposition, *reason) {
            (1, 0) => Ok(Self::Applied),
            (1, reason) => Err(CoreContractDecodeError::UnexpectedReason(reason)),
            (2, reason) => Ok(Self::Skipped(OptimizationReasonCode::from_tag(reason)?)),
            (3, reason) => Ok(Self::Rejected(OptimizationReasonCode::from_tag(reason)?)),
            (disposition, _) => Err(CoreContractDecodeError::UnknownVerdict(disposition)),
        }
    }
}
