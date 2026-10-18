use super::{
    InternalMachineCallResolutionKind, InternalMachineCallResolutionState,
    NormalizedForeignCallResolutionKind, NormalizedForeignCallResolutionState,
    RelocationFreeTextSectionPlacement, TerminalRelocationFreeTextSectionIdentity,
    TextSectionPlacementPolicy, TextSectionRelocationRequirements,
};
use crate::machine_code::identity_encoding::{encode_call_owner, encode_invocation_site};
#[cfg(test)]
use target::NativeTarget;

const TEXT_SECTION_SCHEMA: &[u8] = b"omega.terminal.relocation-free-text-section.v12";

pub fn relocation_free_text_section_identity(
    section: &RelocationFreeTextSectionPlacement,
) -> TerminalRelocationFreeTextSectionIdentity {
    let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(TEXT_SECTION_SCHEMA);
    hasher.raw(&section.source_fragments.bytes());
    hasher.raw(&section.psi.vocabulary_marker.get().to_le_bytes());
    hasher.raw(section.psi.program_fingerprint.as_bytes());
    hasher.raw(&section.fuel_schedule.marker().to_le_bytes());
    hasher.raw(&section.selected.bytes());
    hasher.raw(&section.target.wire_identity_bytes());
    hasher.raw(&section.semantic_entry.get().to_le_bytes());
    hasher.raw(&section.semantic_entry_offset.to_le_bytes());
    hasher.byte(match section.policy {
        TextSectionPlacementPolicy::DenseValidatedFragmentOrderNoPaddingV1 => 1,
    });
    hasher.raw(&section.section_alignment.to_le_bytes());
    hasher.raw(&section.byte_count.to_le_bytes());
    encode_bytes(&mut hasher, &section.bytes);
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
    // A section without image addresses contributes no address roster.
    if !section.unresolved_image_addresses.is_empty() {
        hasher.raw(b"image-addresses-v2");
        hasher.u64(section.unresolved_image_addresses.len() as u64);
        for placed in &section.unresolved_image_addresses {
            let mut identity = Vec::new();
            placed.encode_identity(&mut identity);
            hasher.raw(&identity);
        }
    }
    hasher.byte(match section.relocation_requirements {
        TextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1 => 1,
        TextSectionRelocationRequirements::UnresolvedNormalizedForeignImportFieldsV1 => 2,
        TextSectionRelocationRequirements::UnresolvedObjectSymbolFieldsV1 => 3,
    });
    TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(&hasher.finish())
}

fn encode_bytes(hasher: &mut crate::digest_encoder::DomainDigestEncoder, bytes: &[u8]) {
    hasher.u64(bytes.len() as u64);
    hasher.raw(&bytes);
}

#[cfg(test)]
mod tests {
    use super::super::{
        FuelScheduleIdentity, FunctionFragmentEmissionIdentity, MachineId, PlacedBlockSpan,
        PlacedFunctionFragment, PlacedInstructionSpan, PlacedInternalMachineCallResolution,
        SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity,
        TerminalPsiIdentity,
    };
    use super::{
        InternalMachineCallResolutionKind, InternalMachineCallResolutionState, NativeTarget,
        RelocationFreeTextSectionPlacement, TerminalRelocationFreeTextSectionIdentity,
        TextSectionPlacementPolicy, TextSectionRelocationRequirements,
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
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
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
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
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
