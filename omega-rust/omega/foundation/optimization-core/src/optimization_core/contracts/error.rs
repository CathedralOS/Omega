// `CoreContractDecodeError`: closed wire-decode failure vocabulary for rule,
// analysis, budget, and verdict contract encodings.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreContractDecodeError {
    WrongLength { expected: usize, actual: usize },
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownAnalysisBits(u64),
    UnknownSafetyClass(u8),
    UnknownReasonCode(u8),
    UnknownVerdict(u8),
    UnexpectedReason(u8),
    ZeroWorkBudget,
    ZeroRuleVersion,
}

crate::invalid_error_display!(
    CoreContractDecodeError,
    "optimization core contract encoding"
);
