use std::fmt;

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

impl fmt::Display for CoreContractDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid optimization core contract encoding: {self:?}"
        )
    }
}

impl std::error::Error for CoreContractDecodeError {}
