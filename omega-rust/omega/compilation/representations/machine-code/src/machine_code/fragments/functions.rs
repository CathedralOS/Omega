//! Functions, blocks, and instruction spans in an unplaced fragment.

use super::{
    FunctionFragmentControlProvenance, FunctionFragmentInternalMachineFixup,
    FunctionFragmentNormalizedForeignCallFixup,
};
use selected_instructions::{
    MachineAlternativeKey, SelectedBlockId, SelectedInstructionId, SelectedInstructionProvenance,
};
use semantic_vocabulary::MachineId;
use target_operations::TerminalPsiProvenance;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFragment {
    pub machine: MachineId,
    pub attachment: Option<semantic_vocabulary::StructuralTypeId>,
    /// Exact owner-designated cleanup subject, independent of attachment or call sites.
    pub nominal_cleanup_subject: Option<semantic_vocabulary::StructuralTypeId>,
    pub provenance: TerminalPsiProvenance,
    pub byte_count: u64,
    pub bytes: Vec<u8>,
    pub blocks: Vec<FunctionFragmentBlockSpan>,
}

impl FunctionFragment {
    /// Every block's instruction spans, in block order.
    pub fn instructions(&self) -> impl Iterator<Item = &FunctionFragmentInstructionSpan> + '_ {
        self.blocks.iter().flat_map(|block| &block.instructions)
    }

    /// The span covering `instruction`, when the fragment carries one.
    pub fn instruction_span(
        &self,
        instruction: SelectedInstructionId,
    ) -> Option<&FunctionFragmentInstructionSpan> {
        self.instructions()
            .find(|span| span.instruction == instruction)
    }
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
    pub branch: Option<Box<super::FunctionFragmentBranchEvidence>>,
    // Forward the same sparse owned payloads through fragment realization.
    // No fixup's maximum size belongs in every ordinary instruction stride.
    pub internal_machine_fixup: Option<Box<FunctionFragmentInternalMachineFixup>>,
    /// Unresolved normalized-foreign-call field awaiting object-level import
    /// binding. Internal calls and foreign imports never share one fixup.
    pub normalized_foreign_call_fixup: Option<Box<FunctionFragmentNormalizedForeignCallFixup>>,
    /// Unresolved byte-literal image address awaiting its object relocation.
    pub image_address_fixup: Option<Box<crate::FunctionFragmentImageAddressFixup>>,
    pub provenance: SelectedInstructionProvenance,
    pub control: FunctionFragmentControlProvenance,
}

impl FunctionFragmentInstructionSpan {
    pub fn internal_machine_fixup(&self) -> Option<FunctionFragmentInternalMachineFixup> {
        self.internal_machine_fixup.as_deref().copied()
    }

    pub fn normalized_foreign_call_fixup(
        &self,
    ) -> Option<&FunctionFragmentNormalizedForeignCallFixup> {
        self.normalized_foreign_call_fixup.as_deref()
    }

    pub fn image_address_fixup(&self) -> Option<crate::FunctionFragmentImageAddressFixup> {
        self.image_address_fixup.as_deref().copied()
    }
}
