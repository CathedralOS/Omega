use omega_optimization_core::PostAllocationOptimizationManifestIdentity;

use crate::analyses::pre_allocation_effects::codec as effect_codec;
use omega_regalloc::{AllocationLegalityIdentity, LiveRangeIdentity, RegisterHomeIdentity};
use omega_register_model::{
    PhysicalRegisterModelIdentity, RegisterClassId, RegisterConstraintCatalogIdentity,
    RegisterOperandAccess, RegisterViewId, RegisterWriteSemantics,
    TargetRegisterEnvironmentIdentity,
};
use omega_selected_instructions::{
    MachineEffectCatalogIdentity, SelectedBlockId, SelectedInstructionId,
    SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use psi_core::MachineId;

use crate::{
    MachineAlternativeChoiceRule, PhysicalOperandFootprint, PostAllocationMachineBlock,
    PostAllocationMachineFunction, PostAllocationMachineIdentity, PostAllocationMachineInstruction,
    PostAllocationMachinePlan, PostAllocationStructuralUnitFunction,
    PreAllocationMachineEffectIdentity, post_allocation_machine_identity,
};

const MAGIC: &[u8; 8] = b"OMGPMX\0\0";
const VERSION: u32 = 3;

/// Failure while decoding a framed post-allocation machine-plan artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostAllocationMachineDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    InvalidField,
    InvalidIdentity,
    TrailingBytes,
}

impl std::fmt::Display for PostAllocationMachineDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid post-allocation machine artifact: {self:?}"
        )
    }
}

impl std::error::Error for PostAllocationMachineDecodeError {}

pub(crate) fn encode_terminal_post_allocation_machine_plan(
    plan: &PostAllocationMachinePlan,
) -> Vec<u8> {
    let content = super::identity::encode_terminal_post_allocation_machine_content(plan);
    let mut encoded = Vec::with_capacity(44 + content.len());
    encoded.extend_from_slice(MAGIC);
    encoded.extend_from_slice(&VERSION.to_le_bytes());
    encoded.extend_from_slice(&plan.identity.bytes());
    encoded.extend_from_slice(&content);
    encoded
}

pub(crate) fn decode_terminal_post_allocation_machine_plan(
    encoded: &[u8],
) -> Result<PostAllocationMachinePlan, PostAllocationMachineDecodeError> {
    let mut cursor = effect_codec::Cursor::new(encoded);
    if take(&mut cursor, MAGIC.len())? != MAGIC {
        return Err(PostAllocationMachineDecodeError::WrongMagic);
    }
    let version = u32_field(&mut cursor)?;
    if version != VERSION {
        return Err(PostAllocationMachineDecodeError::UnsupportedVersion(
            version,
        ));
    }
    let identity = PostAllocationMachineIdentity::from_bytes(array(&mut cursor)?);
    let selected = SelectedInstructionPlanIdentity::from_bytes(array(&mut cursor)?);
    let effects = PreAllocationMachineEffectIdentity::from_bytes(array(&mut cursor)?);
    let ranges = LiveRangeIdentity::from_bytes(array(&mut cursor)?);
    let legality = AllocationLegalityIdentity::from_bytes(array(&mut cursor)?);
    let homes = RegisterHomeIdentity::from_bytes(array(&mut cursor)?);
    let post_allocation_manifest =
        PostAllocationOptimizationManifestIdentity::from_bytes(array(&mut cursor)?);
    let target = effect_codec::decode_target(&mut cursor).map_err(map_field_error)?;
    let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(array(&mut cursor)?);
    let physical_register_model = PhysicalRegisterModelIdentity::from_bytes(array(&mut cursor)?);
    let register_constraints = RegisterConstraintCatalogIdentity::from_bytes(array(&mut cursor)?);
    let machine_effect_catalog = MachineEffectCatalogIdentity::from_bytes(array(&mut cursor)?);
    let choice_rule = match byte(&mut cursor)? {
        0 => MachineAlternativeChoiceRule::UniqueApplicableInCatalogOrderV1,
        _ => return Err(PostAllocationMachineDecodeError::InvalidField),
    };
    let function_count = length(&mut cursor)?;
    let mut functions = Vec::with_capacity(function_count.min(cursor.remaining()));
    for _ in 0..function_count {
        let machine = MachineId::new(u64_field(&mut cursor)?)
            .ok_or(PostAllocationMachineDecodeError::InvalidField)?;
        let block_count = length(&mut cursor)?;
        let mut blocks = Vec::with_capacity(block_count.min(cursor.remaining()));
        for _ in 0..block_count {
            let block = SelectedBlockId(u32_field(&mut cursor)?);
            let instruction_count = length(&mut cursor)?;
            let mut instructions = Vec::with_capacity(instruction_count.min(cursor.remaining()));
            for _ in 0..instruction_count {
                instructions.push(decode_instruction(&mut cursor)?);
            }
            blocks.push(PostAllocationMachineBlock {
                block,
                instructions,
            });
        }
        functions.push(PostAllocationMachineFunction { machine, blocks });
    }
    let structural_count = length(&mut cursor)?;
    let mut structural_unit_functions =
        Vec::with_capacity(structural_count.min(cursor.remaining()));
    for _ in 0..structural_count {
        let machine = MachineId::new(u64_field(&mut cursor)?)
            .ok_or(PostAllocationMachineDecodeError::InvalidField)?;
        let block = SelectedBlockId(u32_field(&mut cursor)?);
        let call = match byte(&mut cursor)? {
            0 => None,
            1 => Some(effect_codec::decode_structural_call(&mut cursor).map_err(map_field_error)?),
            _ => return Err(PostAllocationMachineDecodeError::InvalidField),
        };
        let return_instruction = decode_instruction(&mut cursor)?;
        let return_provenance =
            effect_codec::decode_provenance(&mut cursor).map_err(map_field_error)?;
        let return_effect =
            effect_codec::decode_effect_link(&mut cursor).map_err(map_field_error)?;
        let return_ownership =
            effect_codec::decode_ownership(&mut cursor).map_err(map_field_error)?;
        structural_unit_functions.push(PostAllocationStructuralUnitFunction {
            machine,
            block,
            call,
            return_instruction,
            return_provenance,
            return_effect,
            return_ownership,
        });
    }
    if cursor.remaining() != 0 {
        return Err(PostAllocationMachineDecodeError::TrailingBytes);
    }
    let plan = PostAllocationMachinePlan {
        identity,
        selected,
        effects,
        ranges,
        legality,
        homes,
        post_allocation_manifest,
        target,
        register_environment,
        physical_register_model,
        register_constraints,
        machine_effect_catalog,
        choice_rule,
        functions,
        structural_unit_functions,
    };
    if plan.identity != post_allocation_machine_identity(&plan) {
        return Err(PostAllocationMachineDecodeError::InvalidIdentity);
    }
    Ok(plan)
}

fn decode_instruction(
    cursor: &mut effect_codec::Cursor<'_>,
) -> Result<PostAllocationMachineInstruction, PostAllocationMachineDecodeError> {
    let instruction = SelectedInstructionId(u32_field(cursor)?);
    let alternative = effect_codec::decode_alternative(cursor).map_err(map_field_error)?;
    let operand_count = length(cursor)?;
    let mut operands = Vec::with_capacity(operand_count.min(cursor.remaining()));
    for _ in 0..operand_count {
        operands.push(decode_operand(cursor)?);
    }
    Ok(PostAllocationMachineInstruction {
        instruction,
        alternative,
        operands,
        implicit_unit_uses: decode_units(cursor)?,
        implicit_unit_defs: decode_units(cursor)?,
        implicit_unit_clobbers: decode_units(cursor)?,
        unit_uses: decode_units(cursor)?,
        unit_defs: decode_units(cursor)?,
        unit_clobbers: decode_units(cursor)?,
    })
}

fn decode_operand(
    cursor: &mut effect_codec::Cursor<'_>,
) -> Result<PhysicalOperandFootprint, PostAllocationMachineDecodeError> {
    let operand = u16_field(cursor)?;
    let virtual_register = VirtualRegisterId(u32_field(cursor)?);
    let class = RegisterClassId(u16_field(cursor)?);
    let view = RegisterViewId(u16_field(cursor)?);
    let access = match byte(cursor)? {
        0 => RegisterOperandAccess::Use,
        1 => RegisterOperandAccess::Def,
        2 => RegisterOperandAccess::UseDef,
        _ => return Err(PostAllocationMachineDecodeError::InvalidField),
    };
    let storage_units = decode_units(cursor)?;
    let read_units = decode_units(cursor)?;
    let write_units = decode_units(cursor)?;
    let write_semantics = match byte(cursor)? {
        0 => None,
        1 => Some(match byte(cursor)? {
            0 => RegisterWriteSemantics::ExactView,
            1 => RegisterWriteSemantics::PreservesUnwritten,
            2 => RegisterWriteSemantics::ZeroExtendsParent,
            3 => RegisterWriteSemantics::ZeroExtendsWithinUnit,
            4 => RegisterWriteSemantics::Discards,
            5 => RegisterWriteSemantics::InstructionDefined,
            _ => return Err(PostAllocationMachineDecodeError::InvalidField),
        }),
        _ => return Err(PostAllocationMachineDecodeError::InvalidField),
    };
    Ok(PhysicalOperandFootprint {
        operand,
        virtual_register,
        class,
        view,
        access,
        storage_units,
        read_units,
        write_units,
        write_semantics,
    })
}

fn map_field_error(
    error: crate::PreAllocationMachineEffectDecodeError,
) -> PostAllocationMachineDecodeError {
    match error {
        crate::PreAllocationMachineEffectDecodeError::Truncated => {
            PostAllocationMachineDecodeError::Truncated
        }
        _ => PostAllocationMachineDecodeError::InvalidField,
    }
}

fn take<'a>(
    cursor: &mut effect_codec::Cursor<'a>,
    count: usize,
) -> Result<&'a [u8], PostAllocationMachineDecodeError> {
    cursor.take(count).map_err(map_field_error)
}

fn array<const N: usize>(
    cursor: &mut effect_codec::Cursor<'_>,
) -> Result<[u8; N], PostAllocationMachineDecodeError> {
    cursor.array().map_err(map_field_error)
}

fn byte(cursor: &mut effect_codec::Cursor<'_>) -> Result<u8, PostAllocationMachineDecodeError> {
    cursor.byte().map_err(map_field_error)
}

fn u16_field(
    cursor: &mut effect_codec::Cursor<'_>,
) -> Result<u16, PostAllocationMachineDecodeError> {
    cursor.u16().map_err(map_field_error)
}

fn u32_field(
    cursor: &mut effect_codec::Cursor<'_>,
) -> Result<u32, PostAllocationMachineDecodeError> {
    cursor.u32().map_err(map_field_error)
}

fn u64_field(
    cursor: &mut effect_codec::Cursor<'_>,
) -> Result<u64, PostAllocationMachineDecodeError> {
    cursor.u64().map_err(map_field_error)
}

fn length(
    cursor: &mut effect_codec::Cursor<'_>,
) -> Result<usize, PostAllocationMachineDecodeError> {
    cursor.length().map_err(map_field_error)
}

fn decode_units(
    cursor: &mut effect_codec::Cursor<'_>,
) -> Result<Vec<omega_register_model::RegisterUnitId>, PostAllocationMachineDecodeError> {
    effect_codec::decode_units(cursor).map_err(map_field_error)
}

#[cfg(test)]
mod tests {
    use omega_optimization_core::PostAllocationOptimizationManifestIdentity;
    use omega_optimization_unit::{EffectLink, OwnershipEvent};
    use omega_regalloc::{AllocationLegalityIdentity, LiveRangeIdentity, RegisterHomeIdentity};
    use omega_register_model::{
        PhysicalRegisterModelIdentity, RegisterClassId, RegisterConstraintCatalogIdentity,
        RegisterConstraintFamily, RegisterConstraintKey, RegisterOperandAccess, RegisterUnitId,
        RegisterViewId, RegisterWriteSemantics, TargetRegisterEnvironmentIdentity,
    };
    use omega_selected_instructions::{
        MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
        MachineAlternativeKey, MachineEffectCatalogIdentity, MachineEncodedControlEffect,
        MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
        MachineEncodedTrapBehavior, MachineLatencyKnowledge, MachineSizeKnowledge, SelectedBlockId,
        SelectedInstructionId, SelectedInstructionPlanIdentity, SelectedInstructionProvenance,
        SelectedMicrosoftX64OwnedIndirectPairLayout, SelectedStructuralUnitIndirectBinding,
        StructuralUnitCallBarrier, StructuralUnitCallEffect, StructuralUnitCallEffectDeclaration,
        StructuralUnitCallFrameEffect, StructuralUnitCallMemoryEffect, VirtualRegisterId,
    };
    use omega_target::NativeTarget;
    use omega_target_operations::MachineRegister;
    use psi_core::{ClaimId, MachineId, OperationId};

    use crate::{
        MachineAlternativeChoiceRule, PhysicalOperandFootprint, PostAllocationMachineBlock,
        PostAllocationMachineFunction, PostAllocationMachineIdentity,
        PostAllocationMachineInstruction, PostAllocationMachinePlan,
        PostAllocationStructuralUnitFunction, PreAllocationMachineEffectIdentity,
        StructuralUnitCallMachineEffects, post_allocation_machine_identity,
    };

    use super::PostAllocationMachineDecodeError;

    const HEADER_IDENTITY_OFFSET: usize = 12;
    const SELECTED_OFFSET: usize = 44;
    const TARGET_OFFSET: usize = 236;
    const CHOICE_RULE_OFFSET: usize = 382;
    const MACHINE_OFFSET: usize = 391;
    const ALTERNATIVE_FAMILY_OFFSET: usize = 423;
    const OPERAND_ACCESS_OFFSET: usize = 524;
    const WRITE_SEMANTICS_PRESENCE_OFFSET: usize = 555;
    const WRITE_SEMANTICS_OFFSET: usize = 556;

    fn identity(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn plan() -> PostAllocationMachinePlan {
        let mut plan = PostAllocationMachinePlan {
            identity: PostAllocationMachineIdentity::from_bytes([0; 32]),
            selected: SelectedInstructionPlanIdentity::from_bytes(identity(1)),
            effects: PreAllocationMachineEffectIdentity::from_bytes(identity(2)),
            ranges: LiveRangeIdentity::from_bytes(identity(3)),
            legality: AllocationLegalityIdentity::from_bytes(identity(4)),
            homes: RegisterHomeIdentity::from_bytes(identity(5)),
            post_allocation_manifest: PostAllocationOptimizationManifestIdentity::from_bytes(
                identity(6),
            ),
            target: NativeTarget::linux_x64(),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes(identity(7)),
            physical_register_model: PhysicalRegisterModelIdentity::from_bytes(identity(8)),
            register_constraints: RegisterConstraintCatalogIdentity::from_bytes(identity(9)),
            machine_effect_catalog: MachineEffectCatalogIdentity::from_bytes(identity(10)),
            choice_rule: MachineAlternativeChoiceRule::UniqueApplicableInCatalogOrderV1,
            functions: vec![PostAllocationMachineFunction {
                machine: MachineId::new(17).unwrap(),
                blocks: vec![PostAllocationMachineBlock {
                    block: SelectedBlockId(23),
                    instructions: vec![PostAllocationMachineInstruction {
                        instruction: SelectedInstructionId(29),
                        alternative: MachineAlternative {
                            key: MachineAlternativeKey {
                                family: MachineAlternativeFamily::ExactSubtractI64Immediate,
                                variant: 31,
                            },
                            applicability: MachineAlternativeApplicability::ResultAliasesOperand {
                                result: 2,
                                operand: 0,
                            },
                            size: MachineSizeKnowledge::EncoderResolved {
                                minimum_bytes: 3,
                                maximum_bytes: Some(7),
                            },
                            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
                            encoded: MachineEncodedEffects {
                                external_operand_reads: vec![0, 1],
                                external_operand_writes: vec![2],
                                implicit_unit_uses: vec![RegisterUnitId(37)],
                                implicit_unit_defs: vec![RegisterUnitId(41)],
                                implicit_unit_clobbers: vec![RegisterUnitId(43)],
                                memory: MachineEncodedMemoryEffect::ReadActivationStackV1 {
                                    stack_pointer: RegisterViewId(47),
                                    byte_count: 8,
                                },
                                stack: MachineEncodedStackEffect::PopBytesV1 {
                                    stack_pointer: RegisterViewId(47),
                                    byte_count: 8,
                                },
                                trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                                control: MachineEncodedControlEffect::ReturnIndirectRegisterV1 {
                                    target: RegisterViewId(53),
                                },
                            },
                        },
                        operands: vec![PhysicalOperandFootprint {
                            operand: 2,
                            virtual_register: VirtualRegisterId(59),
                            class: RegisterClassId(61),
                            view: RegisterViewId(67),
                            access: RegisterOperandAccess::UseDef,
                            storage_units: vec![RegisterUnitId(71)],
                            read_units: vec![RegisterUnitId(73)],
                            write_units: vec![RegisterUnitId(79)],
                            write_semantics: Some(RegisterWriteSemantics::ZeroExtendsParent),
                        }],
                        implicit_unit_uses: vec![RegisterUnitId(83)],
                        implicit_unit_defs: vec![RegisterUnitId(89)],
                        implicit_unit_clobbers: vec![RegisterUnitId(97)],
                        unit_uses: vec![RegisterUnitId(101)],
                        unit_defs: vec![RegisterUnitId(103)],
                        unit_clobbers: vec![RegisterUnitId(107)],
                    }],
                }],
            }],
            structural_unit_functions: Vec::new(),
        };
        let return_instruction = plan.functions[0].blocks[0].instructions[0].clone();
        let call_constraint = RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant: 109,
        };
        plan.structural_unit_functions
            .push(PostAllocationStructuralUnitFunction {
            machine: MachineId::new(113).unwrap(),
            block: SelectedBlockId(127),
            call: Some(StructuralUnitCallMachineEffects {
                instruction: SelectedInstructionId(0),
                operation: OperationId::new(131).unwrap(),
                callee: MachineId::new(137).unwrap(),
                constraint: call_constraint,
                unit_uses: vec![RegisterUnitId(139), RegisterUnitId(149)],
                unit_defs: vec![RegisterUnitId(151)],
                unit_clobbers: vec![RegisterUnitId(157)],
                layout: SelectedMicrosoftX64OwnedIndirectPairLayout {
                    shadow_byte_count: 32,
                    outgoing_frame_byte_count: 72,
                    pre_call_stack_alignment: 16,
                    bindings: [
                        SelectedStructuralUnitIndirectBinding {
                            parameter_index: 0,
                            pointer: MachineRegister::X86Rcx,
                            copy_stack_byte_offset: 32,
                            byte_count: 16,
                            alignment: 8,
                        },
                        SelectedStructuralUnitIndirectBinding {
                            parameter_index: 1,
                            pointer: MachineRegister::X86Rdx,
                            copy_stack_byte_offset: 48,
                            byte_count: 16,
                            alignment: 8,
                        },
                    ],
                },
                effect: EffectLink {
                    input: 163,
                    output: 167,
                },
                ownership: vec![OwnershipEvent::ClaimTransfer(vec![
                    ClaimId::new(173).unwrap(),
                ])],
                claim_transfers: vec![psi_terminal::ClaimTransfer {
                    claim: ClaimId::new(173).unwrap(),
                    argument_index: 0,
                }],
                provenance: SelectedInstructionProvenance {
                    operations: vec![OperationId::new(131).unwrap()],
                    ..Default::default()
                },
                declaration: StructuralUnitCallEffectDeclaration {
                    constraint: call_constraint,
                    memory:
                        StructuralUnitCallMemoryEffect::ReadOwnedIndirectPairWriteCallerCopiesV1 {
                            root_byte_count: 16,
                            copy_stack_byte_offsets: [32, 48],
                        },
                    frame: StructuralUnitCallFrameEffect::BalancedCallerFrameV1 {
                        frame_byte_count: 72,
                        shadow_byte_count: 32,
                        pre_call_stack_alignment: 16,
                    },
                    trap: omega_selected_instructions::MachineTrapBehavior::MayArchitecturalFaultV1,
                    barrier: StructuralUnitCallBarrier::CallV1,
                    call: StructuralUnitCallEffect::DirectInternalUnitV1,
                    cleanup: omega_selected_instructions::MachineCleanupEffect::NoneV1,
                },
            }),
            return_instruction,
            return_provenance: SelectedInstructionProvenance::default(),
            return_effect: EffectLink {
                input: 179,
                output: 181,
            },
            return_ownership: vec![OwnershipEvent::StructuralReturn(vec![
                ClaimId::new(191).unwrap(),
            ])],
        });
        plan.identity = post_allocation_machine_identity(&plan);
        plan
    }

    #[test]
    fn post_allocation_codec_is_deterministic_and_round_trips_every_field() {
        let plan = plan();
        let first = plan.encode();
        let second = plan.encode();

        assert_eq!(first, second);
        assert_eq!(PostAllocationMachinePlan::decode(&first), Ok(plan));
    }

    #[test]
    fn post_allocation_codec_rejects_bad_framing_and_closed_field_tags() {
        let encoded = plan().encode();
        assert_eq!(encoded[TARGET_OFFSET], 1);
        assert_eq!(encoded[CHOICE_RULE_OFFSET], 0);
        assert_eq!(encoded[ALTERNATIVE_FAMILY_OFFSET], 8);
        assert_eq!(encoded[OPERAND_ACCESS_OFFSET], 2);
        assert_eq!(encoded[WRITE_SEMANTICS_PRESENCE_OFFSET], 1);
        assert_eq!(encoded[WRITE_SEMANTICS_OFFSET], 2);

        let mut wrong_magic = encoded.clone();
        wrong_magic[0] ^= 0xff;
        assert_eq!(
            PostAllocationMachinePlan::decode(&wrong_magic),
            Err(PostAllocationMachineDecodeError::WrongMagic)
        );

        let mut unsupported_version = encoded.clone();
        unsupported_version[8..12].copy_from_slice(&4_u32.to_le_bytes());
        assert_eq!(
            PostAllocationMachinePlan::decode(&unsupported_version),
            Err(PostAllocationMachineDecodeError::UnsupportedVersion(4))
        );

        for offset in [
            TARGET_OFFSET,
            CHOICE_RULE_OFFSET,
            ALTERNATIVE_FAMILY_OFFSET,
            OPERAND_ACCESS_OFFSET,
            WRITE_SEMANTICS_PRESENCE_OFFSET,
            WRITE_SEMANTICS_OFFSET,
        ] {
            let mut invalid = encoded.clone();
            invalid[offset] = 0xff;
            assert_eq!(
                PostAllocationMachinePlan::decode(&invalid),
                Err(PostAllocationMachineDecodeError::InvalidField),
                "closed field at byte {offset} was accepted"
            );
        }

        let mut zero_machine = encoded.clone();
        zero_machine[MACHINE_OFFSET..MACHINE_OFFSET + 8].fill(0);
        assert_eq!(
            PostAllocationMachinePlan::decode(&zero_machine),
            Err(PostAllocationMachineDecodeError::InvalidField)
        );

        assert_eq!(
            PostAllocationMachinePlan::decode(&encoded[..encoded.len() - 1]),
            Err(PostAllocationMachineDecodeError::Truncated)
        );

        let mut trailing = encoded;
        trailing.push(0);
        assert_eq!(
            PostAllocationMachinePlan::decode(&trailing),
            Err(PostAllocationMachineDecodeError::TrailingBytes)
        );
    }

    #[test]
    fn post_allocation_codec_authenticates_header_and_all_content_roots() {
        let encoded = plan().encode();
        let mut root_offsets = vec![HEADER_IDENTITY_OFFSET];
        root_offsets.extend((0..6).map(|index| SELECTED_OFFSET + index * 32));
        root_offsets.extend((0..4).map(|index| 254 + index * 32));

        for offset in root_offsets {
            let mut corrupted = encoded.clone();
            corrupted[offset] ^= 0x80;
            assert_eq!(
                PostAllocationMachinePlan::decode(&corrupted),
                Err(PostAllocationMachineDecodeError::InvalidIdentity),
                "identity-bearing bytes at offset {offset} were accepted"
            );
        }
    }

    #[test]
    fn post_allocation_codec_authenticates_structural_call_content() {
        let source = plan();
        let mut substituted = source.clone();
        substituted.structural_unit_functions[0]
            .call
            .as_mut()
            .unwrap()
            .layout
            .outgoing_frame_byte_count = 80;

        assert_ne!(
            post_allocation_machine_identity(&substituted),
            source.identity
        );
        assert_eq!(
            PostAllocationMachinePlan::decode(&substituted.encode()),
            Err(PostAllocationMachineDecodeError::InvalidIdentity)
        );

        let mut erased = source;
        erased.structural_unit_functions.clear();
        assert_ne!(post_allocation_machine_identity(&erased), plan().identity);
        assert_eq!(
            PostAllocationMachinePlan::decode(&erased.encode()),
            Err(PostAllocationMachineDecodeError::InvalidIdentity)
        );
    }

    #[test]
    fn post_allocation_receipt_counts_atomic_structural_call_and_return() {
        let receipt = crate::post_allocation_receipt(&plan()).unwrap();

        assert_eq!(receipt.function_count(), 2);
        assert_eq!(receipt.block_count(), 2);
        assert_eq!(receipt.instruction_count(), 3);
        assert_eq!(receipt.operand_count(), 1);
        assert_eq!(receipt.unit_action_count(), 10);
    }
}
