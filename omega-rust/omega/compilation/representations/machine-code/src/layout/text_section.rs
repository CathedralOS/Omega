/*
Section-relative machine text placement, internal call resolution evidence,
unresolved foreign import and image address coordinates, and canonical text
section identity computation.

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

#[path = "text_section_publication.rs"]
mod publication;

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
use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
use target::NativeTarget;
use terminal_psi::TerminalPsiIdentity;

use crate::identity_encoding::{encode_call_owner, encode_invocation_site, encode_nested_identity};

const TEXT_SECTION_SCHEMA: &[u8] = b"omega.terminal.relocation-free-text-section.v13";

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
    pub identity: TerminalRelocationFreeTextSectionIdentity,
    pub source_fragments: FunctionFragmentEmissionIdentity,
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

pub fn relocation_free_text_section_identity(
    section: &RelocationFreeTextSectionPlacement,
) -> TerminalRelocationFreeTextSectionIdentity {
    let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(TEXT_SECTION_SCHEMA);
    hasher.raw(&section.source_fragments.bytes());
    hasher.raw(&section.psi.vocabulary_marker.get().to_le_bytes());
    hasher.raw(section.psi.program_fingerprint.as_bytes());
    hasher.raw(&section.work_schedule.marker().to_le_bytes());
    hasher.raw(&section.selected.bytes());
    hasher.raw(&section.target.wire_identity_bytes());
    hasher.raw(&section.semantic_entry.get().to_le_bytes());
    hasher.raw(&section.semantic_entry_offset.to_le_bytes());
    hasher.byte(match section.policy {
        TextSectionPlacementPolicy::DenseValidatedFragmentOrderNoPaddingV1 => 1,
    });
    hasher.raw(&section.section_alignment.to_le_bytes());
    hasher.raw(&section.byte_count.to_le_bytes());
    hasher.bytes(&section.bytes);
    hasher.u64(section.functions.len() as u64);
    for function in &section.functions {
        hasher.raw(&function.source_function_index.to_le_bytes());
        hasher.raw(&function.machine.get().to_le_bytes());
        hasher.raw(&function.section_offset.to_le_bytes());
        hasher.raw(&function.byte_count.to_le_bytes());
        hasher.u64(function.blocks.len() as u64);
        for block in &function.blocks {
            hasher.raw(&block.block.0.to_le_bytes());
            hasher.raw(&block.function_offset.to_le_bytes());
            hasher.raw(&block.section_offset.to_le_bytes());
            hasher.raw(&block.byte_count.to_le_bytes());
            hasher.u64(block.instructions.len() as u64);
            for instruction in &block.instructions {
                hasher.raw(&instruction.instruction.0.to_le_bytes());
                selected_instructions::encode_machine_alternative_key_identity(
                    &mut hasher,
                    instruction.alternative,
                );
                hasher.raw(&instruction.function_offset.to_le_bytes());
                hasher.raw(&instruction.section_offset.to_le_bytes());
                hasher.raw(&instruction.byte_count.to_le_bytes());
            }
        }
    }
    hasher.u64(section.resolved_internal_machine_calls.len() as u64);
    for resolution in &section.resolved_internal_machine_calls {
        hasher.byte(match resolution.kind {
            InternalMachineCallResolutionKind::X86Relative32FromNextInstructionToInternalMachineV1 => 1,
            InternalMachineCallResolutionKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1 => 2,
        });
        hasher.byte(match resolution.state {
            InternalMachineCallResolutionState::ResolvedInSectionV1 => 1,
        });
        hasher.raw(&resolution.caller.get().to_le_bytes());
        hasher.raw(&resolution.block.0.to_le_bytes());
        hasher.raw(&resolution.instruction.0.to_le_bytes());
        encode_call_owner(&mut hasher, resolution.owner);
        hasher.raw(&resolution.callee.get().to_le_bytes());
        hasher.raw(&resolution.call_function_offset.to_le_bytes());
        hasher.raw(&resolution.call_section_offset.to_le_bytes());
        hasher.raw(&resolution.call_byte_count.to_le_bytes());
        hasher.raw(&resolution.opcode_function_offset.to_le_bytes());
        hasher.raw(&resolution.opcode_section_offset.to_le_bytes());
        hasher.raw(&resolution.field_function_offset.to_le_bytes());
        hasher.raw(&resolution.field_section_offset.to_le_bytes());
        hasher.raw(&resolution.next_instruction_function_offset.to_le_bytes());
        hasher.raw(&resolution.next_instruction_section_offset.to_le_bytes());
        hasher.raw(&resolution.callee_section_offset.to_le_bytes());
        hasher.byte(resolution.field_byte_width);
        hasher.raw(&resolution.addend.to_le_bytes());
        hasher.raw(&resolution.displacement.to_le_bytes());
    }
    hasher.u64(section.unresolved_normalized_foreign_calls.len() as u64);
    for resolution in &section.unresolved_normalized_foreign_calls {
        hasher.byte(match resolution.kind {
            NormalizedForeignCallResolutionKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1 => 1,
            NormalizedForeignCallResolutionKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1 => 2,
        });
        hasher.byte(match resolution.state {
            NormalizedForeignCallResolutionState::UnresolvedImportFieldV1 => 1,
        });
        hasher.raw(&resolution.caller.get().to_le_bytes());
        hasher.raw(&resolution.block.0.to_le_bytes());
        hasher.raw(&resolution.instruction.0.to_le_bytes());
        encode_invocation_site(&mut hasher, resolution.invocation_site);
        hasher.raw(&resolution.boundary.get().to_le_bytes());
        hasher.raw(&resolution.ordinal.to_le_bytes());
        hasher.raw(&resolution.call_function_offset.to_le_bytes());
        hasher.raw(&resolution.call_section_offset.to_le_bytes());
        hasher.raw(&resolution.call_byte_count.to_le_bytes());
        hasher.raw(&resolution.opcode_function_offset.to_le_bytes());
        hasher.raw(&resolution.opcode_section_offset.to_le_bytes());
        hasher.raw(&resolution.field_function_offset.to_le_bytes());
        hasher.raw(&resolution.field_section_offset.to_le_bytes());
        hasher.raw(&resolution.next_instruction_function_offset.to_le_bytes());
        hasher.raw(&resolution.next_instruction_section_offset.to_le_bytes());
        hasher.byte(resolution.field_byte_width);
        hasher.raw(&resolution.addend.to_le_bytes());
    }
    if !section.unresolved_image_addresses.is_empty() {
        hasher.raw(b"image-addresses-v2");
        hasher.u64(section.unresolved_image_addresses.len() as u64);
        for placed in &section.unresolved_image_addresses {
            encode_nested_identity(&mut hasher, |out| placed.encode_identity(out));
        }
    }
    hasher.byte(match section.relocation_requirements {
        TextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1 => 1,
        TextSectionRelocationRequirements::UnresolvedNormalizedForeignImportFieldsV1 => 2,
        TextSectionRelocationRequirements::UnresolvedObjectSymbolFieldsV1 => 3,
    });
    TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(&hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::{
        FunctionFragmentEmissionIdentity, InternalMachineCallResolutionKind,
        InternalMachineCallResolutionState, MachineId, NativeTarget, PlacedBlockSpan,
        PlacedFunctionFragment, PlacedInstructionSpan, PlacedInternalMachineCallResolution,
        RelocationFreeTextSectionPlacement, SelectedBlockId, SelectedInstructionId,
        SelectedInstructionPlanIdentity, TerminalPsiIdentity,
        TerminalRelocationFreeTextSectionIdentity, TextSectionPlacementPolicy,
        TextSectionRelocationRequirements, WorkScheduleIdentity,
    };
    use selected_instructions::{MachineAlternativeFamily, MachineAlternativeKey};
    use semantic_vocabulary::OperationId;
    use terminal_psi::{SemanticFingerprint, VocabularyMarker};

    #[test]
    fn text_section_identity_binds_zero_spans_and_section_coordinates() {
        let mut section = RelocationFreeTextSectionPlacement {
            identity: TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(b"pending"),
            source_fragments: FunctionFragmentEmissionIdentity::from_canonical_bytes(b"fragments"),
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
            },
            work_schedule: WorkScheduleIdentity::new(1).unwrap(),
            selected: SelectedInstructionPlanIdentity::from_bytes([8; 32]),
            target: NativeTarget::linux_arm64(),
            semantic_entry: MachineId::new(1).unwrap(),
            semantic_entry_offset: 0,
            policy: TextSectionPlacementPolicy::DenseValidatedFragmentOrderNoPaddingV1,
            section_alignment: 4,
            byte_count: 4,
            bytes: vec![0x20, 0, 0, 0xb5],
            functions: vec![PlacedFunctionFragment {
                source_function_index: 0,
                machine: MachineId::new(1).unwrap(),
                section_offset: 0,
                byte_count: 4,
                blocks: vec![PlacedBlockSpan {
                    block: SelectedBlockId(1),
                    function_offset: 0,
                    section_offset: 0,
                    byte_count: 4,
                    instructions: vec![
                        PlacedInstructionSpan {
                            instruction: SelectedInstructionId(1),
                            alternative: MachineAlternativeKey {
                                family: MachineAlternativeFamily::CompareI64Zero,
                                variant: 1,
                            },
                            function_offset: 0,
                            section_offset: 0,
                            byte_count: 0,
                        },
                        PlacedInstructionSpan {
                            instruction: SelectedInstructionId(2),
                            alternative: MachineAlternativeKey {
                                family: MachineAlternativeFamily::ConditionalBranchNonZero,
                                variant: 2,
                            },
                            function_offset: 0,
                            section_offset: 0,
                            byte_count: 4,
                        },
                    ],
                }],
            }],
            resolved_internal_machine_calls: Vec::new(),
            unresolved_normalized_foreign_calls: Vec::new(),
            unresolved_image_addresses: Vec::new(),
            relocation_requirements:
                TextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
        };
        section.identity = section.recomputed_identity();
        let original = section.identity;
        section.functions[0].blocks[0].instructions[0].section_offset = 4;
        assert_ne!(section.recomputed_identity(), original);
    }

    fn structural_section() -> RelocationFreeTextSectionPlacement {
        let caller = MachineId::new(11).unwrap();
        let callee = MachineId::new(12).unwrap();
        let mut bytes = vec![0; 91];
        bytes[80] = 0xe8;
        crate::codec_bytes::patch_encoded(&mut bytes, 81, 5_i32.to_le_bytes());
        bytes[89] = 0xc3;
        bytes[90] = 0xc3;
        let returned = |instruction, function_offset, section_offset| PlacedInstructionSpan {
            instruction: SelectedInstructionId(instruction),
            alternative: MachineAlternativeKey {
                family: MachineAlternativeFamily::ReturnUnit,
                variant: 0,
            },
            function_offset,
            section_offset,
            byte_count: 1,
        };
        let mut section = RelocationFreeTextSectionPlacement {
            identity: TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(b"pending"),
            source_fragments: FunctionFragmentEmissionIdentity::from_canonical_bytes(
                b"structural-fragments",
            ),
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([9; 32]),
            },
            work_schedule: WorkScheduleIdentity::new(1).unwrap(),
            selected: SelectedInstructionPlanIdentity::from_bytes([10; 32]),
            target: NativeTarget::uefi_x64(),
            semantic_entry: caller,
            semantic_entry_offset: 0,
            policy: TextSectionPlacementPolicy::DenseValidatedFragmentOrderNoPaddingV1,
            section_alignment: 1,
            byte_count: 91,
            bytes,
            functions: vec![
                PlacedFunctionFragment {
                    source_function_index: 0,
                    machine: caller,
                    section_offset: 0,
                    byte_count: 90,
                    blocks: vec![PlacedBlockSpan {
                        block: SelectedBlockId(0),
                        function_offset: 0,
                        section_offset: 0,
                        byte_count: 90,
                        instructions: vec![returned(1, 89, 89)],
                    }],
                },
                PlacedFunctionFragment {
                    source_function_index: 1,
                    machine: callee,
                    section_offset: 90,
                    byte_count: 1,
                    blocks: vec![PlacedBlockSpan {
                        block: SelectedBlockId(0),
                        function_offset: 0,
                        section_offset: 90,
                        byte_count: 1,
                        instructions: vec![returned(0, 0, 90)],
                    }],
                },
            ],
            unresolved_normalized_foreign_calls: Vec::new(),
            unresolved_image_addresses: Vec::new(),
            resolved_internal_machine_calls: vec![
                PlacedInternalMachineCallResolution {
                    kind: InternalMachineCallResolutionKind::X86Relative32FromNextInstructionToInternalMachineV1,
                    state: InternalMachineCallResolutionState::ResolvedInSectionV1,
                    caller,
                    block: SelectedBlockId(0),
                    instruction: SelectedInstructionId(0),
                    owner: target_operations::CallSiteOwner::Operation(OperationId::new(21).unwrap()),
                    callee,
                    call_function_offset: 0,
                    call_section_offset: 0,
                    call_byte_count: 89,
                    opcode_function_offset: 80,
                    opcode_section_offset: 80,
                    field_function_offset: 81,
                    field_section_offset: 81,
                    next_instruction_function_offset: 85,
                    next_instruction_section_offset: 85,
                    callee_section_offset: 90,
                    field_byte_width: 4,
                    addend: 0,
                    displacement: 5,
                },
            ],
            relocation_requirements: TextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
        };
        section.identity = section.recomputed_identity();
        section
    }

    fn assert_resolution_mutation_changes_identity(
        mutate: impl FnOnce(&mut PlacedInternalMachineCallResolution),
    ) {
        let mut section = structural_section();
        let original = section.identity;
        mutate(&mut section.resolved_internal_machine_calls[0]);
        assert_ne!(section.recomputed_identity(), original);
    }

    #[test]
    fn text_section_identity_binds_internal_machine_resolution_roster_and_coordinates() {
        let mut without_resolution = structural_section();
        let original = without_resolution.identity;
        without_resolution.resolved_internal_machine_calls.clear();
        assert_ne!(without_resolution.recomputed_identity(), original);

        assert_resolution_mutation_changes_identity(|row| row.caller = MachineId::new(31).unwrap());
        assert_resolution_mutation_changes_identity(|row| row.block = SelectedBlockId(2));
        assert_resolution_mutation_changes_identity(|row| {
            row.instruction = SelectedInstructionId(2)
        });
        assert_resolution_mutation_changes_identity(|row| {
            row.owner = target_operations::CallSiteOwner::Operation(OperationId::new(32).unwrap())
        });
        for (edge, action_ordinal) in [(31, 0), (31, 1), (32, 0)] {
            assert_resolution_mutation_changes_identity(|row| {
                row.owner = target_operations::CallSiteOwner::CleanupAction {
                    edge: semantic_vocabulary::EdgeId::new(edge).unwrap(),
                    action_ordinal,
                };
            });
        }
        assert_resolution_mutation_changes_identity(|row| row.callee = MachineId::new(33).unwrap());
        assert_resolution_mutation_changes_identity(|row| row.call_function_offset += 1);
        assert_resolution_mutation_changes_identity(|row| row.call_section_offset += 1);
        assert_resolution_mutation_changes_identity(|row| row.call_byte_count += 1);
        assert_resolution_mutation_changes_identity(|row| row.opcode_function_offset += 1);
        assert_resolution_mutation_changes_identity(|row| row.opcode_section_offset += 1);
        assert_resolution_mutation_changes_identity(|row| row.field_function_offset += 1);
        assert_resolution_mutation_changes_identity(|row| row.field_section_offset += 1);
        assert_resolution_mutation_changes_identity(|row| {
            row.next_instruction_function_offset += 1
        });
        assert_resolution_mutation_changes_identity(|row| row.next_instruction_section_offset += 1);
        assert_resolution_mutation_changes_identity(|row| row.callee_section_offset += 1);
        assert_resolution_mutation_changes_identity(|row| row.field_byte_width += 1);
        assert_resolution_mutation_changes_identity(|row| row.addend += 1);
        assert_resolution_mutation_changes_identity(|row| row.displacement += 1);

        let mut patched = structural_section();
        let original = patched.identity;
        patched.bytes[81] ^= 1;
        assert_ne!(patched.recomputed_identity(), original);
    }
}
