//! Functions, blocks, and instruction spans in an unplaced fragment.

use super::{
    FunctionFragmentConditionalBranchEvidence, FunctionFragmentControlProvenance,
    FunctionFragmentInternalMachineFixup,
};
use omega_selected_instructions::{
    MachineAlternativeKey, SelectedBlockId, SelectedInstructionId, SelectedInstructionProvenance,
};
use omega_target_operations::TerminalPsiProvenance;
use psi_core::{MachineId, OperationId};

/// Function fragment for the structural-ABI Unit lane. Its call bytes remain
/// non-executable until whole-text placement discharges every typed fixup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralUnitFunctionFragment {
    pub machine: MachineId,
    pub attachment: Option<psi_core::StructuralTypeId>,
    pub provenance: TerminalPsiProvenance,
    pub byte_count: u64,
    pub bytes: Vec<u8>,
    pub block: StructuralUnitFunctionFragmentBlockSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralUnitFunctionFragmentBlockSpan {
    pub block: SelectedBlockId,
    pub offset: u64,
    pub byte_count: u64,
    pub call: Option<StructuralUnitCallFragmentSpan>,
    pub return_instruction: FunctionFragmentInstructionSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralUnitCallFragmentSpan {
    pub instruction: SelectedInstructionId,
    pub operation: OperationId,
    pub callee: MachineId,
    pub offset: u64,
    pub bytes: Vec<u8>,
    pub provenance: SelectedInstructionProvenance,
    pub fixup: FunctionFragmentInternalMachineFixup,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragment {
    pub machine: MachineId,
    pub attachment: Option<psi_core::StructuralTypeId>,
    pub provenance: TerminalPsiProvenance,
    pub byte_count: u64,
    pub bytes: Vec<u8>,
    pub blocks: Vec<FunctionFragmentBlockSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentBlockSpan {
    pub block: SelectedBlockId,
    pub offset: u64,
    pub byte_count: u64,
    pub instructions: Vec<FunctionFragmentInstructionSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragmentInstructionSpan {
    pub instruction: SelectedInstructionId,
    pub alternative: MachineAlternativeKey,
    pub offset: u64,
    pub bytes: Vec<u8>,
    pub branch: Option<Box<FunctionFragmentConditionalBranchEvidence>>,
    pub internal_machine_fixup: Option<FunctionFragmentInternalMachineFixup>,
    pub provenance: SelectedInstructionProvenance,
    pub control: FunctionFragmentControlProvenance,
}
