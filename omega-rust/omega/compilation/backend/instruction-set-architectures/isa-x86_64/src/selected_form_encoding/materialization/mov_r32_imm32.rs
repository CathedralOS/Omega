use register_model::RegisterViewId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum X86_64MovR32Imm32I64MaterializationError {
    NonCanonicalPhysicalModel,
    UnknownOrNonGpr64View(RegisterViewId),
    IntegerOutsideZeroExtendedU32Bits,
    MalformedEncoding,
    EncodedFormMismatch,
}

optimization_core::invalid_error_display!(
    X86_64MovR32Imm32I64MaterializationError,
    "x86-64 MOV-r32-imm32 i64 materialization"
);
