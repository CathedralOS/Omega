//! Current section-relative machine bytes, spans, and resolved internal calls.
//! The record is independent of its producer and grants no publication authority.

mod identity;
mod publication;
pub use identity::relocation_free_text_section_identity;
pub use publication::{
    FunctionFragmentTextSectionManifest, FunctionFragmentTextSectionManifestDecodeError,
    FunctionFragmentTextSectionStage, FunctionFragmentTextSectionStatistics,
    FunctionFragmentTextSectionUnavailableData,
};

use optimization_core::{
    FunctionFragmentEmissionIdentity, TerminalRelocationFreeTextSectionIdentity,
};
use selected_instructions::{
    MachineAlternativeKey, SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity,
};
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use target::NativeTarget;
use terminal_psi::TerminalPsiIdentity;

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
    /// The section still carries normalized-foreign-call fields bound only at
    /// object construction to declared import symbols. Internal control is
    /// fully resolved; foreign imports remain object relocations, never
    /// in-section patches.
    UnresolvedNormalizedForeignImportFieldsV1,
    /// The section carries byte-literal image address fields, with or
    /// without normalized-foreign import fields. Object construction binds
    /// every one of them to a declared symbol; none is patched in section.
    UnresolvedObjectSymbolFieldsV1,
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
    /// Section-relative evidence for every normalized foreign call whose
    /// import field stays unresolved. Object construction binds each row to
    /// the declared import symbol for the roster entry the `{boundary,
    /// ordinal}` pair names; these fields are object relocations, never
    /// in-section patches.
    pub unresolved_normalized_foreign_calls: Vec<PlacedNormalizedForeignCallResolution>,
    /// Section-relative evidence for every byte-literal image address whose
    /// fields object construction binds to the literal's read-only symbol.
    pub unresolved_image_addresses: Vec<crate::PlacedImageAddress>,
    pub relocation_requirements: TextSectionRelocationRequirements,
}

impl RelocationFreeTextSectionPlacement {
    /// The placed fragment for `machine`, when the section carries it.
    pub fn function(&self, machine: MachineId) -> Option<&PlacedFunctionFragment> {
        self.functions
            .iter()
            .find(|function| function.machine == machine)
    }

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

impl PlacedFunctionFragment {
    /// Every block's instruction spans, in block order.
    pub fn instructions(&self) -> impl Iterator<Item = &PlacedInstructionSpan> + '_ {
        self.blocks.iter().flat_map(|block| &block.instructions)
    }

    /// The span covering `instruction`, when the placed fragment carries one.
    pub fn instruction_span(
        &self,
        instruction: SelectedInstructionId,
    ) -> Option<&PlacedInstructionSpan> {
        self.instructions()
            .find(|span| span.instruction == instruction)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedInstructionSpan {
    pub instruction: SelectedInstructionId,
    pub alternative: MachineAlternativeKey,
    pub function_offset: u64,
    pub section_offset: u64,
    pub byte_count: u64,
}

/// The resolution kind is the selected-form fixup kind verbatim.
pub type InternalMachineCallResolutionKind = crate::SelectedFormInternalMachineFixupKind;

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
    pub owner: target_operations::CallSiteOwner,
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

/// The resolution kind is the selected-form fixup kind verbatim.
pub type NormalizedForeignCallResolutionKind = crate::SelectedFormNormalizedForeignCallFixupKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalizedForeignCallResolutionState {
    /// The field is a canonical zero placeholder awaiting its object-level
    /// import-symbol relocation.
    UnresolvedImportFieldV1,
}

/// How one placed callback-address materialization names its unresolved
/// field(s), in both function- and section-relative coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacedCallbackAddressEncoding {
    /// `lea reg, [rip + rel32]`; the disp32 is the private-symbol field.
    X86_64Relative32 {
        relocation_function_offset: u64,
        relocation_section_offset: u64,
    },
    /// `adrp` + `add #lo12`; both immediate fields are private-symbol fields.
    Aarch64PageAddress {
        page_relocation_function_offset: u64,
        page_relocation_section_offset: u64,
        page_offset_relocation_function_offset: u64,
        page_offset_relocation_section_offset: u64,
    },
}

/// Placement evidence for the private-function-address materialization
/// emitted inside one callback-bearing normalized foreign call, ahead of the
/// call bytes in the same instruction's span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacedCallbackAddressMaterialization {
    /// Where the materialized address lands — the exact register or outgoing
    /// stack slot the evaluated callback application names.
    pub destination: crate::CallbackAddressDestination,
    /// Function/section offsets where the materialization bytes begin.
    pub code_function_offset: u64,
    pub code_section_offset: u64,
    /// Total bytes the materialization occupies.
    pub code_byte_count: u64,
    pub encoding: PlacedCallbackAddressEncoding,
}

/// Generic, ISA-tagged placement evidence for one normalized foreign call
/// whose import field object construction still binds. The `{boundary,
/// ordinal}` pair names the selected roster row owning the evaluated locator;
/// the raw foreign coordinate never enters the section record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedNormalizedForeignCallResolution {
    pub kind: NormalizedForeignCallResolutionKind,
    pub state: NormalizedForeignCallResolutionState,
    pub caller: MachineId,
    pub block: SelectedBlockId,
    pub instruction: SelectedInstructionId,
    pub invocation_site: semantic_vocabulary::BoundaryInvocationSite,
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub ordinal: u32,
    pub call_function_offset: u64,
    pub call_section_offset: u64,
    pub call_byte_count: u64,
    pub opcode_function_offset: u64,
    pub opcode_section_offset: u64,
    pub field_function_offset: u64,
    pub field_section_offset: u64,
    pub next_instruction_function_offset: u64,
    pub next_instruction_section_offset: u64,
    pub field_byte_width: u8,
    pub addend: i64,
    /// The callback materializations emitted inside this instruction's span,
    /// in the selected call's roster order; empty for a plain call.
    pub callback_materializations: Vec<PlacedCallbackAddressMaterialization>,
}
