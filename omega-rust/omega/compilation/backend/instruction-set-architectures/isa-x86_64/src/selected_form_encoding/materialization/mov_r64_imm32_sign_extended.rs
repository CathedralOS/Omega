use register_model::RegisterViewId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum X86_64MovR64Imm32SignExtendedI64MaterializationError {
    NonCanonicalPhysicalModel,
    UnknownOrNonGpr64View(RegisterViewId),
    IntegerOutsideSignExtendedI32Bits,
    MalformedEncoding,
    EncodedFormMismatch,
}

impl std::fmt::Display for X86_64MovR64Imm32SignExtendedI64MaterializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid x86-64 MOV-r64-imm32 sign-extended i64 materialization: {self:?}"
        )
    }
}
impl std::error::Error for X86_64MovR64Imm32SignExtendedI64MaterializationError {}
