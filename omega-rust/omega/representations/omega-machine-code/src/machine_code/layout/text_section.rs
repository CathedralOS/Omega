//! Current section-relative machine bytes, spans, and resolved internal calls.
//! The record is independent of its producer and grants no publication authority.

mod identity;
mod publication;
pub use identity::relocation_free_text_section_identity;
pub use publication::*;

use omega_optimization_core::{
    FunctionFragmentEmissionIdentity, TerminalRelocationFreeTextSectionIdentity,
};
use omega_selected_instructions::{
    MachineAlternativeKey, SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity,
};
use omega_target::NativeTarget;
use psi_core::{FuelScheduleIdentity, MachineId, OperationId};
use psi_terminal::TerminalPsiIdentity;

/// Exact deterministic placement used by the first clean Terminal text-section boundary.
///
/// Functions remain in the already validated fragment order. No sorting, padding, symbol
/// assignment, or object-container policy is implied by this value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSectionPlacementPolicy {
    DenseValidatedFragmentOrderNoPaddingV1,
}

/// Closed relocation conclusion for the currently admitted clean Terminal instruction set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSectionRelocationRequirements {
    ProvenNoneForFullyResolvedInternalControlV1,
}

/// One relocation-free, section-relative concatenation of validated function fragments.
///
/// This is representation data only. It is not an object-container plan, has no symbols or external
/// entry point, and grants no object serialization, image, installation, or publication
/// authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelocationFreeTextSectionPlacement {
    pub identity: TerminalRelocationFreeTextSectionIdentity,
    pub source_fragments: FunctionFragmentEmissionIdentity,
    pub psi: TerminalPsiIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub target: NativeTarget,
    pub semantic_entry: MachineId,
    pub semantic_entry_offset: u64,
    pub policy: TextSectionPlacementPolicy,
    pub section_alignment: u64,
    pub byte_count: u64,
    pub bytes: Vec<u8>,
    pub functions: Vec<PlacedFunctionFragment>,
    /// Section-relative evidence for every internal-Machine call discharged
    /// during placement. The final call bytes live in `bytes`; these rows bind
    /// their source spans, exact coordinates, and resolved target equations
    /// without duplicating executable bytes or introducing object relocations.
    pub resolved_internal_machine_calls: Vec<PlacedInternalMachineCallResolution>,
    pub relocation_requirements: TextSectionRelocationRequirements,
}

impl RelocationFreeTextSectionPlacement {
    pub fn recomputed_identity(&self) -> TerminalRelocationFreeTextSectionIdentity {
        relocation_free_text_section_identity(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedFunctionFragment {
    pub source_function_index: u64,
    pub machine: MachineId,
    pub section_offset: u64,
    pub byte_count: u64,
    pub blocks: Vec<PlacedBlockSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedBlockSpan {
    pub block: SelectedBlockId,
    pub function_offset: u64,
    pub section_offset: u64,
    pub byte_count: u64,
    pub instructions: Vec<PlacedInstructionSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedInstructionSpan {
    pub instruction: SelectedInstructionId,
    pub alternative: MachineAlternativeKey,
    pub function_offset: u64,
    pub section_offset: u64,
    pub byte_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InternalMachineCallResolutionKind {
    X86Relative32FromNextInstructionToInternalMachineV1,
    Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InternalMachineCallResolutionState {
    ResolvedInSectionV1,
}

/// Generic, ISA-tagged placement evidence for one fully discharged internal
/// call. Function-relative coordinates retain the fragment source meaning;
/// section-relative coordinates bind the final dense text representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacedInternalMachineCallResolution {
    pub kind: InternalMachineCallResolutionKind,
    pub state: InternalMachineCallResolutionState,
    pub caller: MachineId,
    pub block: SelectedBlockId,
    pub instruction: SelectedInstructionId,
    pub operation: OperationId,
    pub callee: MachineId,
    pub call_function_offset: u64,
    pub call_section_offset: u64,
    pub call_byte_count: u64,
    pub opcode_function_offset: u64,
    pub opcode_section_offset: u64,
    pub field_function_offset: u64,
    pub field_section_offset: u64,
    pub next_instruction_function_offset: u64,
    pub next_instruction_section_offset: u64,
    pub callee_section_offset: u64,
    pub field_byte_width: u8,
    pub addend: i64,
    pub displacement: i32,
}
