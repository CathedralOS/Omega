use omega_isa_aarch64::{Aarch64ScalarCallTemplateError, Aarch64SelectedFormEncodingError};
use omega_isa_x86_64::X86_64MovR32Imm32I64MaterializationError;
use omega_isa_x86_64::X86_64MovR64Imm32SignExtendedI64MaterializationError;
use omega_isa_x86_64::{
    X86_64ScalarCallTemplateError, X86_64SelectedFormEncodingError,
    X86_64StructuralUnitCallTemplateError,
};
use omega_selected_instructions::SelectedInstructionId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedSelectedFormEncodingError {
    SelectedRootMismatch,
    PhysicalModelMismatch,
    FunctionRosterMismatch,
    BlockRosterMismatch,
    InstructionRosterMismatch,
    StructuralFunctionRosterMismatch,
    StructuralConstraintCatalogMismatch,
    StructuralCallRosterMismatch(SelectedInstructionId),
    StructuralReturnRosterMismatch(SelectedInstructionId),
    OperandFootprintMismatch(SelectedInstructionId),
    ImplicitFootprintMismatch(SelectedInstructionId),
    SizeDeclarationMismatch(SelectedInstructionId),
    CountOverflow,
    X86_64(X86_64SelectedFormEncodingError),
    X86_64ScalarCall(X86_64ScalarCallTemplateError),
    X86_64MovR32Imm32(X86_64MovR32Imm32I64MaterializationError),
    X86_64MovR64Imm32SignExtended(X86_64MovR64Imm32SignExtendedI64MaterializationError),
    X86_64Structural(X86_64StructuralUnitCallTemplateError),
    Aarch64(Aarch64SelectedFormEncodingError),
    Aarch64ScalarCall(Aarch64ScalarCallTemplateError),
    ArtifactMismatch,
}

impl std::fmt::Display for OptimizedSelectedFormEncodingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized selected-form encoding failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedSelectedFormEncodingError {}
