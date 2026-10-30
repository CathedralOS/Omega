use register_model::RegisterViewId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum X86_64MovR64Imm32SignExtendedI64MaterializationError {
    NonCanonicalPhysicalModel,
    UnknownOrNonGpr64View(RegisterViewId),
    IntegerOutsideSignExtendedI32Bits,
    MalformedEncoding,
    EncodedFormMismatch,
}

optimization_core::invalid_error_display!(
    X86_64MovR64Imm32SignExtendedI64MaterializationError,
    "x86-64 MOV-r64-imm32 sign-extended i64 materialization"
);
