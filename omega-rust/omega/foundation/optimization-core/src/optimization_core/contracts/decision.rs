// Closed validation strength (`OptimizationSafetyClass`), machine-readable
// decision reason (`OptimizationReasonCode`), and 2-byte candidate disposition
// (`OptimizationCandidateVerdict`).

use super::CoreContractDecodeError;

crate::tagged_vocabulary! {
    5;
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum OptimizationSafetyClass {
        StructuralIdentity = 1,
        ExactOperationSemantics = 2,
        ProofCertified = 3,
        OwnershipCertified = 4,
        TranslationValidated = 5,
    }
}

crate::tag_from_all!(
    OptimizationSafetyClass,
    CoreContractDecodeError => CoreContractDecodeError::UnknownSafetyClass
);

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
        Self::from_tag(*tag)
    }
}

crate::tagged_vocabulary! {
    12;
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum OptimizationReasonCode {
        Inapplicable = 1 => "inapplicable",
        NotProfitable = 2 => "not profitable",
        MissingAnalysis = 3 => "missing analysis",
        UnsupportedVocabulary = 4 => "unsupported vocabulary",
        ValidationFailed = 5 => "validation failed",
        ProofIncomplete = 6 => "proof incomplete",
        OwnershipEvidenceIncomplete = 7 => "ownership evidence incomplete",
        WorkBudgetExhausted = 8 => "work budget exhausted",
        CandidateLimitReached = 9 => "candidate limit reached",
        ConvergenceLimitReached = 10 => "convergence limit reached",
        PolicyRejected = 11 => "policy rejected",
        Superseded = 12 => "superseded",
    }
}

crate::tag_from_all!(
    OptimizationReasonCode,
    CoreContractDecodeError => CoreContractDecodeError::UnknownReasonCode
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OptimizationCandidateVerdict {
    Applied,
    Skipped(OptimizationReasonCode),
    Rejected(OptimizationReasonCode),
}

impl OptimizationCandidateVerdict {
    pub const ENCODED_WIDTH: usize = 2;

    pub const fn encode(self) -> [u8; Self::ENCODED_WIDTH] {
        match self {
            Self::Applied => [1, 0],
            Self::Skipped(reason) => [2, reason as u8],
            Self::Rejected(reason) => [3, reason as u8],
        }
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, CoreContractDecodeError> {
        let [disposition, reason] = encoded else {
            return Err(CoreContractDecodeError::WrongLength {
                expected: Self::ENCODED_WIDTH,
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
