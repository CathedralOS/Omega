use register_model::RegisterViewId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum X86_64MovR32Imm32I64MaterializationError {
    NonCanonicalPhysicalModel,
    UnknownOrNonGpr64View(RegisterViewId),
    IntegerOutsideZeroExtendedU32Bits,
    MalformedEncoding,
    EncodedFormMismatch,
}

impl std::fmt::Display for X86_64MovR32Imm32I64MaterializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid x86-64 MOV-r32-imm32 i64 materialization: {self:?}"
        )
    }
}

impl std::error::Error for X86_64MovR32Imm32I64MaterializationError {}
