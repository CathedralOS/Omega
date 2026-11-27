/*
Section-relative machine text placement, internal call resolution evidence, and
unresolved foreign import and image address coordinates.

We represent one relocation-free, section-relative concatenation of validated
function fragments (`RelocationFreeTextSectionPlacement`). Functions remain in
their validated fragment order (`TextSectionPlacementPolicy`), with section
offsets recorded on every placed function (`PlacedFunctionFragment`), block
(`PlacedBlockSpan`), and instruction span (`PlacedInstructionSpan`). Internal
machine calls are resolved in-section (`PlacedInternalMachineCallResolution`),
while normalized foreign calls (`PlacedNormalizedForeignCallResolution`) and
byte-literal image addresses (`PlacedImageAddress`) retain their unresolved
object-relocation coordinates for object construction.
*/

use selected_instructions::{
    MachineAlternativeKey, SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity,
};
use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
use target::NativeTarget;
use terminal_psi::TerminalPsiIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSectionPlacementPolicy {
    DenseValidatedFragmentOrderNoPaddingV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSectionRelocationRequirements {
    ProvenNoneForFullyResolvedInternalControlV1,
    UnresolvedNormalizedForeignImportFieldsV1, // Internal control resolved; foreign imports remain object relocations
    UnresolvedObjectSymbolFieldsV1, // Carries byte-literal image address fields (with or without foreign import fields)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelocationFreeTextSectionPlacement {
    pub psi: TerminalPsiIdentity,
    pub work_schedule: WorkScheduleIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub target: NativeTarget,
    pub semantic_entry: MachineId,
    pub semantic_entry_offset: u64,
    pub policy: TextSectionPlacementPolicy,
    pub section_alignment: u64,
    pub byte_count: u64,
    pub bytes: Vec<u8>,
    pub functions: Vec<PlacedFunctionFragment>,
    pub resolved_internal_machine_calls: Vec<PlacedInternalMachineCallResolution>, // Section-relative evidence for internal calls discharged in-section
    pub unresolved_normalized_foreign_calls: Vec<PlacedNormalizedForeignCallResolution>, // Object-level import relocations named by `{boundary, ordinal}`
    pub unresolved_image_addresses: Vec<crate::PlacedImageAddress>, // Object-level read-only symbol relocations for byte literals
    pub relocation_requirements: TextSectionRelocationRequirements,
}

impl RelocationFreeTextSectionPlacement {
    pub fn function(&self, machine: MachineId) -> Option<&PlacedFunctionFragment> {
        self.functions
            .iter()
            .find(|function| function.machine == machine)
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

impl PlacedFunctionFragment {
    pub fn instructions(&self) -> impl Iterator<Item = &PlacedInstructionSpan> + '_ {
        self.blocks.iter().flat_map(|block| &block.instructions)
    }

    pub fn instruction_span(
        &self,
        instruction: SelectedInstructionId,
    ) -> Option<&PlacedInstructionSpan> {
        self.instructions()
            .find(|span| span.instruction == instruction)
    }
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

pub type InternalMachineCallResolutionKind = crate::SelectedFormInternalMachineFixupKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InternalMachineCallResolutionState {
    ResolvedInSectionV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacedInternalMachineCallResolution {
    pub kind: InternalMachineCallResolutionKind,
    pub state: InternalMachineCallResolutionState,
    pub caller: MachineId,
    pub block: SelectedBlockId,
    pub instruction: SelectedInstructionId,
    pub owner: target_vocabulary::CallSiteOwner,
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

pub type NormalizedForeignCallResolutionKind = crate::SelectedFormNormalizedForeignCallFixupKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalizedForeignCallResolutionState {
    UnresolvedImportFieldV1, // Canonical zero placeholder awaiting object-level import relocation
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacedCallbackAddressEncoding {
    X86_64Relative32 {
        relocation_function_offset: u64,
        relocation_section_offset: u64,
    },
    Aarch64PageAddress {
        page_relocation_function_offset: u64,
        page_relocation_section_offset: u64,
        page_offset_relocation_function_offset: u64,
        page_offset_relocation_section_offset: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedCallbackAddressMaterialization {
    pub destination: crate::CallbackAddressDestination, // Exact register or outgoing stack slot for the callback address
    pub code_function_offset: u64,
    pub code_section_offset: u64,
    pub code_byte_count: u64,
    pub encoding: PlacedCallbackAddressEncoding,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedNormalizedForeignCallResolution {
    pub kind: NormalizedForeignCallResolutionKind,
    pub state: NormalizedForeignCallResolutionState,
    pub caller: MachineId,
    pub block: SelectedBlockId,
    pub instruction: SelectedInstructionId,
    pub invocation_site: semantic_vocabulary::InvocationSite,
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
    pub callback_materializations: Vec<PlacedCallbackAddressMaterialization>, // Emitted ahead of call bytes in roster order
}
