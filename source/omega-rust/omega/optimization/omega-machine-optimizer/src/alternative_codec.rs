use omega_optimization_core::PostAllocationOptimizationManifestIdentity;
use omega_regalloc::{
    TerminalAllocationLegalityIdentity, TerminalLiveRangeIdentity, TerminalRegisterHomeIdentity,
};
use omega_register_model::{
    PhysicalRegisterModelIdentity, RegisterClassId, RegisterConstraintCatalogIdentity,
    RegisterOperandAccess, RegisterViewId, RegisterWriteSemantics,
    TargetRegisterEnvironmentIdentity,
};
use omega_terminal_selected_instructions::{
    TerminalMachineEffectCatalogIdentity, TerminalSelectedBlockId, TerminalSelectedInstructionId,
    TerminalSelectedInstructionPlanIdentity, TerminalVirtualRegisterId,
};
use psi_core::MachineId;

use crate::{
    TerminalMachineAlternativeChoiceRule, TerminalPhysicalOperandFootprint,
    TerminalPostAllocationMachineBlock, TerminalPostAllocationMachineFunction,
    TerminalPostAllocationMachineIdentity, TerminalPostAllocationMachineInstruction,
    TerminalPostAllocationMachinePlan, TerminalPreAllocationMachineEffectIdentity,
    terminal_post_allocation_machine_identity,
};

const MAGIC: &[u8; 8] = b"OMGPMX\0\0";
const VERSION: u32 = 2;

/// Failure while decoding a framed post-allocation machine-plan artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalPostAllocationMachineDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    InvalidField,
    InvalidIdentity,
    TrailingBytes,
}

impl std::fmt::Display for TerminalPostAllocationMachineDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid post-allocation machine artifact: {self:?}"
        )
    }
}

impl std::error::Error for TerminalPostAllocationMachineDecodeError {}

pub(crate) fn encode_terminal_post_allocation_machine_plan(
    plan: &TerminalPostAllocationMachinePlan,
) -> Vec<u8> {
    let content =
        crate::alternative_identity::encode_terminal_post_allocation_machine_content(plan);
    let mut encoded = Vec::with_capacity(44 + content.len());
    encoded.extend_from_slice(MAGIC);
    encoded.extend_from_slice(&VERSION.to_le_bytes());
    encoded.extend_from_slice(&plan.identity.bytes());
    encoded.extend_from_slice(&content);
    encoded
}

pub(crate) fn decode_terminal_post_allocation_machine_plan(
    encoded: &[u8],
) -> Result<TerminalPostAllocationMachinePlan, TerminalPostAllocationMachineDecodeError> {
    let mut cursor = crate::effect_codec::Cursor::new(encoded);
    if take(&mut cursor, MAGIC.len())? != MAGIC {
        return Err(TerminalPostAllocationMachineDecodeError::WrongMagic);
    }
    let version = u32_field(&mut cursor)?;
    if version != VERSION {
        return Err(TerminalPostAllocationMachineDecodeError::UnsupportedVersion(version));
    }
    let identity = TerminalPostAllocationMachineIdentity::from_bytes(array(&mut cursor)?);
    let selected = TerminalSelectedInstructionPlanIdentity::from_bytes(array(&mut cursor)?);
    let effects = TerminalPreAllocationMachineEffectIdentity::from_bytes(array(&mut cursor)?);
    let ranges = TerminalLiveRangeIdentity::from_bytes(array(&mut cursor)?);
    let legality = TerminalAllocationLegalityIdentity::from_bytes(array(&mut cursor)?);
    let homes = TerminalRegisterHomeIdentity::from_bytes(array(&mut cursor)?);
    let post_allocation_manifest =
        PostAllocationOptimizationManifestIdentity::from_bytes(array(&mut cursor)?);
    let target = crate::effect_codec::decode_target(&mut cursor).map_err(map_field_error)?;
    let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(array(&mut cursor)?);
    let physical_register_model = PhysicalRegisterModelIdentity::from_bytes(array(&mut cursor)?);
    let register_constraints = RegisterConstraintCatalogIdentity::from_bytes(array(&mut cursor)?);
    let machine_effect_catalog =
        TerminalMachineEffectCatalogIdentity::from_bytes(array(&mut cursor)?);
    let choice_rule = match byte(&mut cursor)? {
        0 => TerminalMachineAlternativeChoiceRule::UniqueApplicableInCatalogOrderV1,
        _ => return Err(TerminalPostAllocationMachineDecodeError::InvalidField),
    };
    let function_count = length(&mut cursor)?;
    let mut functions = Vec::with_capacity(function_count.min(cursor.remaining()));
    for _ in 0..function_count {
        let machine = MachineId::new(u64_field(&mut cursor)?)
            .ok_or(TerminalPostAllocationMachineDecodeError::InvalidField)?;
        let block_count = length(&mut cursor)?;
        let mut blocks = Vec::with_capacity(block_count.min(cursor.remaining()));
        for _ in 0..block_count {
            let block = TerminalSelectedBlockId(u32_field(&mut cursor)?);
            let instruction_count = length(&mut cursor)?;
            let mut instructions = Vec::with_capacity(instruction_count.min(cursor.remaining()));
            for _ in 0..instruction_count {
                instructions.push(decode_instruction(&mut cursor)?);
            }
            blocks.push(TerminalPostAllocationMachineBlock {
                block,
                instructions,
            });
        }
        functions.push(TerminalPostAllocationMachineFunction { machine, blocks });
    }
    if cursor.remaining() != 0 {
        return Err(TerminalPostAllocationMachineDecodeError::TrailingBytes);
    }
    let plan = TerminalPostAllocationMachinePlan {
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
    };
    if plan.identity != terminal_post_allocation_machine_identity(&plan) {
        return Err(TerminalPostAllocationMachineDecodeError::InvalidIdentity);
    }
    Ok(plan)
}

fn decode_instruction(
    cursor: &mut crate::effect_codec::Cursor<'_>,
) -> Result<TerminalPostAllocationMachineInstruction, TerminalPostAllocationMachineDecodeError> {
    let instruction = TerminalSelectedInstructionId(u32_field(cursor)?);
    let alternative = crate::effect_codec::decode_alternative(cursor).map_err(map_field_error)?;
    let operand_count = length(cursor)?;
    let mut operands = Vec::with_capacity(operand_count.min(cursor.remaining()));
    for _ in 0..operand_count {
        operands.push(decode_operand(cursor)?);
    }
    Ok(TerminalPostAllocationMachineInstruction {
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
    cursor: &mut crate::effect_codec::Cursor<'_>,
) -> Result<TerminalPhysicalOperandFootprint, TerminalPostAllocationMachineDecodeError> {
    let operand = u16_field(cursor)?;
    let virtual_register = TerminalVirtualRegisterId(u32_field(cursor)?);
    let class = RegisterClassId(u16_field(cursor)?);
    let view = RegisterViewId(u16_field(cursor)?);
    let access = match byte(cursor)? {
        0 => RegisterOperandAccess::Use,
        1 => RegisterOperandAccess::Def,
        2 => RegisterOperandAccess::UseDef,
        _ => return Err(TerminalPostAllocationMachineDecodeError::InvalidField),
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
            _ => return Err(TerminalPostAllocationMachineDecodeError::InvalidField),
        }),
        _ => return Err(TerminalPostAllocationMachineDecodeError::InvalidField),
    };
    Ok(TerminalPhysicalOperandFootprint {
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
    error: crate::TerminalPreAllocationMachineEffectDecodeError,
) -> TerminalPostAllocationMachineDecodeError {
    match error {
        crate::TerminalPreAllocationMachineEffectDecodeError::Truncated => {
            TerminalPostAllocationMachineDecodeError::Truncated
        }
        _ => TerminalPostAllocationMachineDecodeError::InvalidField,
    }
}

fn take<'a>(
    cursor: &mut crate::effect_codec::Cursor<'a>,
    count: usize,
) -> Result<&'a [u8], TerminalPostAllocationMachineDecodeError> {
    cursor.take(count).map_err(map_field_error)
}

fn array<const N: usize>(
    cursor: &mut crate::effect_codec::Cursor<'_>,
) -> Result<[u8; N], TerminalPostAllocationMachineDecodeError> {
    cursor.array().map_err(map_field_error)
}

fn byte(
    cursor: &mut crate::effect_codec::Cursor<'_>,
) -> Result<u8, TerminalPostAllocationMachineDecodeError> {
    cursor.byte().map_err(map_field_error)
}

fn u16_field(
    cursor: &mut crate::effect_codec::Cursor<'_>,
) -> Result<u16, TerminalPostAllocationMachineDecodeError> {
    cursor.u16().map_err(map_field_error)
}

fn u32_field(
    cursor: &mut crate::effect_codec::Cursor<'_>,
) -> Result<u32, TerminalPostAllocationMachineDecodeError> {
    cursor.u32().map_err(map_field_error)
}

fn u64_field(
    cursor: &mut crate::effect_codec::Cursor<'_>,
) -> Result<u64, TerminalPostAllocationMachineDecodeError> {
    cursor.u64().map_err(map_field_error)
}

fn length(
    cursor: &mut crate::effect_codec::Cursor<'_>,
) -> Result<usize, TerminalPostAllocationMachineDecodeError> {
    cursor.length().map_err(map_field_error)
}

fn decode_units(
    cursor: &mut crate::effect_codec::Cursor<'_>,
) -> Result<Vec<omega_register_model::RegisterUnitId>, TerminalPostAllocationMachineDecodeError> {
    crate::effect_codec::decode_units(cursor).map_err(map_field_error)
}

#[cfg(test)]
mod tests {
    use omega_optimization_core::PostAllocationOptimizationManifestIdentity;
    use omega_regalloc::{
        TerminalAllocationLegalityIdentity, TerminalLiveRangeIdentity, TerminalRegisterHomeIdentity,
    };
    use omega_register_model::{
        PhysicalRegisterModelIdentity, RegisterClassId, RegisterConstraintCatalogIdentity,
        RegisterOperandAccess, RegisterUnitId, RegisterViewId, RegisterWriteSemantics,
        TargetRegisterEnvironmentIdentity,
    };
    use omega_target::NativeTarget;
    use omega_terminal_selected_instructions::{
        TerminalMachineAlternative, TerminalMachineAlternativeApplicability,
        TerminalMachineAlternativeFamily, TerminalMachineAlternativeKey,
        TerminalMachineEffectCatalogIdentity, TerminalMachineEncodedControlEffect,
        TerminalMachineEncodedEffects, TerminalMachineEncodedMemoryEffect,
        TerminalMachineEncodedStackEffect, TerminalMachineEncodedTrapBehavior,
        TerminalMachineLatencyKnowledge, TerminalMachineSizeKnowledge, TerminalSelectedBlockId,
        TerminalSelectedInstructionId, TerminalSelectedInstructionPlanIdentity,
        TerminalVirtualRegisterId,
    };
    use psi_core::MachineId;

    use crate::{
        TerminalMachineAlternativeChoiceRule, TerminalPhysicalOperandFootprint,
        TerminalPostAllocationMachineBlock, TerminalPostAllocationMachineFunction,
        TerminalPostAllocationMachineIdentity, TerminalPostAllocationMachineInstruction,
        TerminalPostAllocationMachinePlan, TerminalPreAllocationMachineEffectIdentity,
        terminal_post_allocation_machine_identity,
    };

    use super::TerminalPostAllocationMachineDecodeError;

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

    fn plan() -> TerminalPostAllocationMachinePlan {
        let mut plan = TerminalPostAllocationMachinePlan {
            identity: TerminalPostAllocationMachineIdentity::from_bytes([0; 32]),
            selected: TerminalSelectedInstructionPlanIdentity::from_bytes(identity(1)),
            effects: TerminalPreAllocationMachineEffectIdentity::from_bytes(identity(2)),
            ranges: TerminalLiveRangeIdentity::from_bytes(identity(3)),
            legality: TerminalAllocationLegalityIdentity::from_bytes(identity(4)),
            homes: TerminalRegisterHomeIdentity::from_bytes(identity(5)),
            post_allocation_manifest: PostAllocationOptimizationManifestIdentity::from_bytes(
                identity(6),
            ),
            target: NativeTarget::linux_x64(),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes(identity(7)),
            physical_register_model: PhysicalRegisterModelIdentity::from_bytes(identity(8)),
            register_constraints: RegisterConstraintCatalogIdentity::from_bytes(identity(9)),
            machine_effect_catalog: TerminalMachineEffectCatalogIdentity::from_bytes(identity(10)),
            choice_rule: TerminalMachineAlternativeChoiceRule::UniqueApplicableInCatalogOrderV1,
            functions: vec![TerminalPostAllocationMachineFunction {
                machine: MachineId::new(17).unwrap(),
                blocks: vec![TerminalPostAllocationMachineBlock {
                    block: TerminalSelectedBlockId(23),
                    instructions: vec![TerminalPostAllocationMachineInstruction {
                        instruction: TerminalSelectedInstructionId(29),
                        alternative: TerminalMachineAlternative {
                            key: TerminalMachineAlternativeKey {
                                family: TerminalMachineAlternativeFamily::ExactSubtractI64Immediate,
                                variant: 31,
                            },
                            applicability:
                                TerminalMachineAlternativeApplicability::ResultAliasesOperand {
                                    result: 2,
                                    operand: 0,
                                },
                            size: TerminalMachineSizeKnowledge::EncoderResolved {
                                minimum_bytes: 3,
                                maximum_bytes: Some(7),
                            },
                            latency: TerminalMachineLatencyKnowledge::StableBaselineUnavailable,
                            encoded: TerminalMachineEncodedEffects {
                                external_operand_reads: vec![0, 1],
                                external_operand_writes: vec![2],
                                implicit_unit_uses: vec![RegisterUnitId(37)],
                                implicit_unit_defs: vec![RegisterUnitId(41)],
                                implicit_unit_clobbers: vec![RegisterUnitId(43)],
                                memory: TerminalMachineEncodedMemoryEffect::ReadActivationStackV1 {
                                    stack_pointer: RegisterViewId(47),
                                    byte_count: 8,
                                },
                                stack: TerminalMachineEncodedStackEffect::PopBytesV1 {
                                    stack_pointer: RegisterViewId(47),
                                    byte_count: 8,
                                },
                                trap: TerminalMachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                                control:
                                    TerminalMachineEncodedControlEffect::ReturnIndirectRegisterV1 {
                                        target: RegisterViewId(53),
                                    },
                            },
                        },
                        operands: vec![TerminalPhysicalOperandFootprint {
                            operand: 2,
                            virtual_register: TerminalVirtualRegisterId(59),
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
        };
        plan.identity = terminal_post_allocation_machine_identity(&plan);
        plan
    }

    #[test]
    fn post_allocation_codec_is_deterministic_and_round_trips_every_field() {
        let plan = plan();
        let first = plan.encode();
        let second = plan.encode();

        assert_eq!(first, second);
        assert_eq!(TerminalPostAllocationMachinePlan::decode(&first), Ok(plan));
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
            TerminalPostAllocationMachinePlan::decode(&wrong_magic),
            Err(TerminalPostAllocationMachineDecodeError::WrongMagic)
        );

        let mut unsupported_version = encoded.clone();
        unsupported_version[8..12].copy_from_slice(&3_u32.to_le_bytes());
        assert_eq!(
            TerminalPostAllocationMachinePlan::decode(&unsupported_version),
            Err(TerminalPostAllocationMachineDecodeError::UnsupportedVersion(3))
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
                TerminalPostAllocationMachinePlan::decode(&invalid),
                Err(TerminalPostAllocationMachineDecodeError::InvalidField),
                "closed field at byte {offset} was accepted"
            );
        }

        let mut zero_machine = encoded.clone();
        zero_machine[MACHINE_OFFSET..MACHINE_OFFSET + 8].fill(0);
        assert_eq!(
            TerminalPostAllocationMachinePlan::decode(&zero_machine),
            Err(TerminalPostAllocationMachineDecodeError::InvalidField)
        );

        assert_eq!(
            TerminalPostAllocationMachinePlan::decode(&encoded[..encoded.len() - 1]),
            Err(TerminalPostAllocationMachineDecodeError::Truncated)
        );

        let mut trailing = encoded;
        trailing.push(0);
        assert_eq!(
            TerminalPostAllocationMachinePlan::decode(&trailing),
            Err(TerminalPostAllocationMachineDecodeError::TrailingBytes)
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
                TerminalPostAllocationMachinePlan::decode(&corrupted),
                Err(TerminalPostAllocationMachineDecodeError::InvalidIdentity),
                "identity-bearing bytes at offset {offset} were accepted"
            );
        }
    }
}
