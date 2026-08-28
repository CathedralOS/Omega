use omega_optimization_core::{AcceptedObligationFactIdentity, OptimizationUnitIdentity};
use omega_optimization_unit::{EffectLink, FuelSettlement, OwnershipEvent, PsiProvenance};
use omega_register_model::{
    RegisterConstraintCatalogIdentity, RegisterConstraintFamily, RegisterConstraintKey,
    RegisterUnitId, TargetRegisterEnvironmentIdentity,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_selected_instructions::{
    TerminalMachineAlternative, TerminalMachineAlternativeApplicability,
    TerminalMachineAlternativeFamily, TerminalMachineAlternativeKey, TerminalMachineBarrier,
    TerminalMachineCallEffect, TerminalMachineCleanupEffect, TerminalMachineEffectCatalogIdentity,
    TerminalMachineEncodedControlEffect, TerminalMachineEncodedEffects,
    TerminalMachineEncodedMemoryEffect, TerminalMachineEncodedStackEffect,
    TerminalMachineEncodedTrapBehavior, TerminalMachineLatencyKnowledge,
    TerminalMachineMemoryEffect, TerminalMachineSizeKnowledge, TerminalMachineTrapBehavior,
    TerminalSelectedBlockId, TerminalSelectedInstructionId, TerminalSelectedInstructionKind,
    TerminalSelectedInstructionPlanIdentity, TerminalSelectedInstructionProvenance,
    TerminalSelectedMicrosoftX64OwnedIndirectPairLayout,
    TerminalSelectedStructuralUnitIndirectBinding, TerminalStructuralUnitCallBarrier,
    TerminalStructuralUnitCallEffect, TerminalStructuralUnitCallEffectDeclaration,
    TerminalStructuralUnitCallFrameEffect, TerminalStructuralUnitCallMemoryEffect,
};
use psi_core::{
    ClaimId, EdgeId, FuelScheduleIdentity, IntegerValue, MachineId, ObligationId, OperationId,
    PlaceId, StructuralTypeId, ValueId,
};

use crate::{
    TerminalBlockMachineEffects, TerminalFunctionMachineEffects, TerminalInstructionMachineEffects,
    TerminalPreAllocationMachineEffectIdentity, TerminalPreAllocationMachineEffectPlan,
    TerminalStructuralUnitCallMachineEffects, TerminalStructuralUnitFunctionMachineEffects,
    terminal_pre_allocation_machine_effect_identity,
};

const MAGIC: &[u8; 8] = b"OMGMFX\0\0";
const VERSION: u32 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalPreAllocationMachineEffectDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    InvalidField,
    InvalidIdentity,
    TrailingBytes,
}

impl std::fmt::Display for TerminalPreAllocationMachineEffectDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid pre-allocation machine-effect artifact: {self:?}"
        )
    }
}

impl std::error::Error for TerminalPreAllocationMachineEffectDecodeError {}

pub(crate) fn encode_terminal_pre_allocation_machine_effect_plan(
    plan: &TerminalPreAllocationMachineEffectPlan,
) -> Vec<u8> {
    let content =
        crate::effect_identity::encode_terminal_pre_allocation_machine_effect_content(plan);
    let mut encoded = Vec::with_capacity(44 + content.len());
    encoded.extend_from_slice(MAGIC);
    encoded.extend_from_slice(&VERSION.to_le_bytes());
    encoded.extend_from_slice(&plan.identity.bytes());
    encoded.extend_from_slice(&content);
    encoded
}

pub(crate) fn decode_terminal_pre_allocation_machine_effect_plan(
    encoded: &[u8],
) -> Result<TerminalPreAllocationMachineEffectPlan, TerminalPreAllocationMachineEffectDecodeError> {
    let mut cursor = Cursor::new(encoded);
    if cursor.take(8)? != MAGIC {
        return Err(TerminalPreAllocationMachineEffectDecodeError::WrongMagic);
    }
    let version = cursor.u32()?;
    if version != VERSION {
        return Err(TerminalPreAllocationMachineEffectDecodeError::UnsupportedVersion(version));
    }
    let identity = TerminalPreAllocationMachineEffectIdentity::from_bytes(cursor.array()?);
    let selected = TerminalSelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
    let optimization_unit = OptimizationUnitIdentity::from_bytes(cursor.array()?);
    let fuel_schedule = FuelScheduleIdentity::new(cursor.u32()?)
        .ok_or(TerminalPreAllocationMachineEffectDecodeError::InvalidField)?;
    let target = decode_target(&mut cursor)?;
    let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
    let register_constraints = RegisterConstraintCatalogIdentity::from_bytes(cursor.array()?);
    let machine_effect_catalog = TerminalMachineEffectCatalogIdentity::from_bytes(cursor.array()?);
    let function_count = cursor.length()?;
    let mut functions = Vec::with_capacity(function_count.min(cursor.remaining()));
    for _ in 0..function_count {
        let machine = MachineId::new(cursor.u64()?)
            .ok_or(TerminalPreAllocationMachineEffectDecodeError::InvalidField)?;
        let block_count = cursor.length()?;
        let mut blocks = Vec::with_capacity(block_count.min(cursor.remaining()));
        for _ in 0..block_count {
            let block = TerminalSelectedBlockId(cursor.u32()?);
            let instruction_count = cursor.length()?;
            let mut instructions = Vec::with_capacity(instruction_count.min(cursor.remaining()));
            for _ in 0..instruction_count {
                instructions.push(decode_instruction(&mut cursor)?);
            }
            blocks.push(TerminalBlockMachineEffects {
                block,
                instructions,
            });
        }
        functions.push(TerminalFunctionMachineEffects { machine, blocks });
    }
    let structural_count = cursor.length()?;
    let mut structural_unit_functions =
        Vec::with_capacity(structural_count.min(cursor.remaining()));
    for _ in 0..structural_count {
        structural_unit_functions.push(decode_structural_function(&mut cursor)?);
    }
    if cursor.remaining() != 0 {
        return Err(TerminalPreAllocationMachineEffectDecodeError::TrailingBytes);
    }
    let plan = TerminalPreAllocationMachineEffectPlan {
        identity,
        selected,
        optimization_unit,
        fuel_schedule,
        target,
        register_environment,
        register_constraints,
        machine_effect_catalog,
        functions,
        structural_unit_functions,
    };
    if plan.identity != terminal_pre_allocation_machine_effect_identity(&plan) {
        return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidIdentity);
    }
    Ok(plan)
}

fn decode_structural_function(
    cursor: &mut Cursor<'_>,
) -> Result<
    TerminalStructuralUnitFunctionMachineEffects,
    TerminalPreAllocationMachineEffectDecodeError,
> {
    let machine = decode_machine(cursor)?;
    let block = TerminalSelectedBlockId(cursor.u32()?);
    let call = match cursor.byte()? {
        0 => None,
        1 => Some(decode_structural_call(cursor)?),
        _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    };
    let return_instruction = decode_instruction(cursor)?;
    let return_effect = decode_effect_link(cursor)?;
    let return_ownership = decode_ownership(cursor)?;
    Ok(TerminalStructuralUnitFunctionMachineEffects {
        machine,
        block,
        call,
        return_instruction,
        return_effect,
        return_ownership,
    })
}

fn decode_structural_call(
    cursor: &mut Cursor<'_>,
) -> Result<TerminalStructuralUnitCallMachineEffects, TerminalPreAllocationMachineEffectDecodeError>
{
    let instruction = TerminalSelectedInstructionId(cursor.u32()?);
    let operation = OperationId::new(cursor.u64()?)
        .ok_or(TerminalPreAllocationMachineEffectDecodeError::InvalidField)?;
    let callee = decode_machine(cursor)?;
    let constraint = decode_constraint_key(cursor)?;
    let unit_uses = decode_units(cursor)?;
    let unit_defs = decode_units(cursor)?;
    let unit_clobbers = decode_units(cursor)?;
    let layout = decode_structural_layout(cursor)?;
    let effect = decode_effect_link(cursor)?;
    let ownership = decode_ownership(cursor)?;
    let transfer_count = cursor.length()?;
    let mut claim_transfers = Vec::with_capacity(transfer_count.min(cursor.remaining()));
    for _ in 0..transfer_count {
        claim_transfers.push(psi_terminal::ClaimTransfer {
            claim: ClaimId::new(cursor.u64()?)
                .ok_or(TerminalPreAllocationMachineEffectDecodeError::InvalidField)?,
            argument_index: cursor.u32()?,
        });
    }
    let provenance = decode_provenance(cursor)?;
    let declaration = decode_structural_declaration(cursor)?;
    Ok(TerminalStructuralUnitCallMachineEffects {
        instruction,
        operation,
        callee,
        constraint,
        unit_uses,
        unit_defs,
        unit_clobbers,
        layout,
        effect,
        ownership,
        claim_transfers,
        provenance,
        declaration,
    })
}

fn decode_structural_layout(
    cursor: &mut Cursor<'_>,
) -> Result<
    TerminalSelectedMicrosoftX64OwnedIndirectPairLayout,
    TerminalPreAllocationMachineEffectDecodeError,
> {
    let shadow_byte_count = cursor.u32()?;
    let outgoing_frame_byte_count = cursor.u32()?;
    let pre_call_stack_alignment = cursor.u16()?;
    let mut bindings = Vec::with_capacity(2);
    for _ in 0..2 {
        bindings.push(TerminalSelectedStructuralUnitIndirectBinding {
            parameter_index: usize::try_from(cursor.u64()?)
                .map_err(|_| TerminalPreAllocationMachineEffectDecodeError::InvalidField)?,
            pointer: decode_machine_register(cursor)?,
            copy_stack_byte_offset: cursor.u32()?,
            byte_count: cursor.u16()?,
            alignment: cursor.u16()?,
        });
    }
    Ok(TerminalSelectedMicrosoftX64OwnedIndirectPairLayout {
        shadow_byte_count,
        outgoing_frame_byte_count,
        pre_call_stack_alignment,
        bindings: bindings
            .try_into()
            .map_err(|_| TerminalPreAllocationMachineEffectDecodeError::InvalidField)?,
    })
}

fn decode_machine_register(
    cursor: &mut Cursor<'_>,
) -> Result<
    omega_terminal_target_operations::MachineRegister,
    TerminalPreAllocationMachineEffectDecodeError,
> {
    use omega_terminal_target_operations::MachineRegister as R;
    Ok(match cursor.byte()? {
        0 => R::X86Rax,
        1 => R::X86Rcx,
        2 => R::X86Rdx,
        3 => R::X86Rbx,
        4 => R::X86Rsp,
        5 => R::X86Rbp,
        6 => R::X86Rsi,
        7 => R::X86Rdi,
        8 => R::X86R8,
        9 => R::X86R9,
        10 => R::X86R10,
        11 => R::X86R11,
        12 => R::X86R12,
        13 => R::X86R13,
        14 => R::X86R14,
        15 => R::X86R15,
        16 => R::X86Xmm(cursor.byte()?),
        17 => R::Aarch64X(cursor.byte()?),
        18 => R::Aarch64V(cursor.byte()?),
        _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    })
}

fn decode_effect_link(
    cursor: &mut Cursor<'_>,
) -> Result<EffectLink, TerminalPreAllocationMachineEffectDecodeError> {
    Ok(EffectLink {
        input: cursor.u64()?,
        output: cursor.u64()?,
    })
}

fn decode_structural_declaration(
    cursor: &mut Cursor<'_>,
) -> Result<
    TerminalStructuralUnitCallEffectDeclaration,
    TerminalPreAllocationMachineEffectDecodeError,
> {
    let constraint = decode_constraint_key(cursor)?;
    let memory = match cursor.byte()? {
        1 => TerminalStructuralUnitCallMemoryEffect::ReadOwnedIndirectPairWriteCallerCopiesV1 {
            root_byte_count: cursor.u16()?,
            copy_stack_byte_offsets: [cursor.u32()?, cursor.u32()?],
        },
        _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    };
    let frame = match cursor.byte()? {
        1 => TerminalStructuralUnitCallFrameEffect::BalancedCallerFrameV1 {
            frame_byte_count: cursor.u32()?,
            shadow_byte_count: cursor.u32()?,
            pre_call_stack_alignment: cursor.u16()?,
        },
        _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    };
    let trap = match cursor.byte()? {
        0 => TerminalMachineTrapBehavior::NeverV1,
        1 => TerminalMachineTrapBehavior::MayArchitecturalFaultV1,
        _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    };
    if cursor.byte()? != 1 || cursor.byte()? != 1 || cursor.byte()? != 0 {
        return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField);
    }
    Ok(TerminalStructuralUnitCallEffectDeclaration {
        constraint,
        memory,
        frame,
        trap,
        barrier: TerminalStructuralUnitCallBarrier::CallV1,
        call: TerminalStructuralUnitCallEffect::DirectInternalUnitV1,
        cleanup: TerminalMachineCleanupEffect::NoneV1,
    })
}

fn decode_instruction(
    cursor: &mut Cursor<'_>,
) -> Result<TerminalInstructionMachineEffects, TerminalPreAllocationMachineEffectDecodeError> {
    let instruction = TerminalSelectedInstructionId(cursor.u32()?);
    let kind = decode_kind(cursor)?;
    let constraint = decode_constraint_key(cursor)?;
    let unit_uses = decode_units(cursor)?;
    let unit_defs = decode_units(cursor)?;
    let unit_clobbers = decode_units(cursor)?;
    if cursor.byte()? != 0 || cursor.byte()? != 0 {
        return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField);
    }
    let barrier = match cursor.byte()? {
        0 => TerminalMachineBarrier::None,
        1 => TerminalMachineBarrier::ControlFlow,
        _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    };
    if cursor.byte()? != 0 || cursor.byte()? != 0 {
        return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField);
    }
    let provenance = decode_provenance(cursor)?;
    let alternative_count = cursor.length()?;
    let mut alternatives = Vec::with_capacity(alternative_count.min(cursor.remaining()));
    for _ in 0..alternative_count {
        alternatives.push(decode_alternative(cursor)?);
    }
    Ok(TerminalInstructionMachineEffects {
        instruction,
        kind,
        constraint,
        unit_uses,
        unit_defs,
        unit_clobbers,
        memory: TerminalMachineMemoryEffect::NoneV1,
        trap: TerminalMachineTrapBehavior::NeverV1,
        barrier,
        call: TerminalMachineCallEffect::NoneV1,
        cleanup: TerminalMachineCleanupEffect::NoneV1,
        provenance,
        alternatives,
    })
}

fn decode_kind(
    cursor: &mut Cursor<'_>,
) -> Result<TerminalSelectedInstructionKind, TerminalPreAllocationMachineEffectDecodeError> {
    Ok(match cursor.byte()? {
        0 => TerminalSelectedInstructionKind::CompareI64Zero,
        1 => TerminalSelectedInstructionKind::MaterializeI64 {
            value: decode_integer(cursor)?,
        },
        2 => TerminalSelectedInstructionKind::CopyI64,
        3 => TerminalSelectedInstructionKind::ExactAddI64 {
            obligation: decode_obligation(cursor)?,
            accepted_fact: AcceptedObligationFactIdentity::from_bytes(cursor.array()?),
        },
        4 => TerminalSelectedInstructionKind::ExactAddI64Immediate {
            immediate: decode_integer(cursor)?,
            obligation: decode_obligation(cursor)?,
            accepted_fact: AcceptedObligationFactIdentity::from_bytes(cursor.array()?),
        },
        5 => TerminalSelectedInstructionKind::ExactSubtractI64 {
            obligation: decode_obligation(cursor)?,
            accepted_fact: AcceptedObligationFactIdentity::from_bytes(cursor.array()?),
        },
        6 => TerminalSelectedInstructionKind::ConditionalBranchNonZero,
        7 => TerminalSelectedInstructionKind::ReturnI64,
        8 => TerminalSelectedInstructionKind::ExactSubtractI64Immediate {
            immediate: decode_integer(cursor)?,
            obligation: decode_obligation(cursor)?,
            accepted_fact: AcceptedObligationFactIdentity::from_bytes(cursor.array()?),
        },
        9 => TerminalSelectedInstructionKind::ReturnUnit,
        _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    })
}

fn decode_integer(
    cursor: &mut Cursor<'_>,
) -> Result<IntegerValue, TerminalPreAllocationMachineEffectDecodeError> {
    match cursor.byte()? {
        0 => Ok(IntegerValue::Signed(i128::from_le_bytes(cursor.array()?))),
        1 => Ok(IntegerValue::Unsigned(u128::from_le_bytes(cursor.array()?))),
        _ => Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    }
}

fn decode_provenance(
    cursor: &mut Cursor<'_>,
) -> Result<TerminalSelectedInstructionProvenance, TerminalPreAllocationMachineEffectDecodeError> {
    let operations = decode_ids(cursor, OperationId::new)?;
    let values = decode_ids(cursor, ValueId::new)?;
    let edges = decode_ids(cursor, EdgeId::new)?;
    let obligations = decode_ids(cursor, ObligationId::new)?;
    let fuel_count = cursor.length()?;
    let mut fuel = Vec::with_capacity(fuel_count.min(cursor.remaining()));
    for _ in 0..fuel_count {
        let site_tag = cursor.byte()?;
        let raw = cursor.u64()?;
        let site = match site_tag {
            0 => PsiProvenance::Operation(
                OperationId::new(raw)
                    .ok_or(TerminalPreAllocationMachineEffectDecodeError::InvalidField)?,
            ),
            1 => PsiProvenance::Edge(
                EdgeId::new(raw)
                    .ok_or(TerminalPreAllocationMachineEffectDecodeError::InvalidField)?,
            ),
            _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
        };
        fuel.push(FuelSettlement {
            site,
            units: cursor.u64()?,
        });
    }
    Ok(TerminalSelectedInstructionProvenance {
        operations,
        values,
        edges,
        obligations,
        fuel,
    })
}

pub(crate) fn decode_alternative(
    cursor: &mut Cursor<'_>,
) -> Result<TerminalMachineAlternative, TerminalPreAllocationMachineEffectDecodeError> {
    let family = match cursor.byte()? {
        0 => TerminalMachineAlternativeFamily::CompareI64Zero,
        1 => TerminalMachineAlternativeFamily::MaterializeI64,
        2 => TerminalMachineAlternativeFamily::CopyI64,
        3 => TerminalMachineAlternativeFamily::ExactAddI64,
        4 => TerminalMachineAlternativeFamily::ExactAddI64Immediate,
        5 => TerminalMachineAlternativeFamily::ExactSubtractI64,
        6 => TerminalMachineAlternativeFamily::ConditionalBranchNonZero,
        7 => TerminalMachineAlternativeFamily::ReturnI64,
        8 => TerminalMachineAlternativeFamily::ExactSubtractI64Immediate,
        9 => TerminalMachineAlternativeFamily::ReturnUnit,
        _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    };
    let key = TerminalMachineAlternativeKey {
        family,
        variant: cursor.u32()?,
    };
    let applicability = match cursor.byte()? {
        0 => TerminalMachineAlternativeApplicability::Always,
        1 => TerminalMachineAlternativeApplicability::ResultAliasesOperand {
            result: cursor.u16()?,
            operand: cursor.u16()?,
        },
        2 => TerminalMachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
            result: cursor.u16()?,
            aliased_operand: cursor.u16()?,
            distinct_operand: cursor.u16()?,
        },
        3 => TerminalMachineAlternativeApplicability::ResultAliasesOperands {
            result: cursor.u16()?,
            left: cursor.u16()?,
            right: cursor.u16()?,
        },
        4 => TerminalMachineAlternativeApplicability::ResultDistinctFromOperands {
            result: cursor.u16()?,
            left: cursor.u16()?,
            right: cursor.u16()?,
        },
        5 => TerminalMachineAlternativeApplicability::AtLeastOneOperandDoesNotAliasView {
            left: cursor.u16()?,
            right: cursor.u16()?,
            excluded_view: omega_register_model::RegisterViewId(cursor.u16()?),
        },
        _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    };
    let size = match cursor.byte()? {
        0 => TerminalMachineSizeKnowledge::ExactBytes(cursor.u16()?),
        1 => {
            let minimum_bytes = cursor.u16()?;
            let maximum_bytes = match cursor.byte()? {
                0 => None,
                1 => Some(cursor.u16()?),
                _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
            };
            TerminalMachineSizeKnowledge::EncoderResolved {
                minimum_bytes,
                maximum_bytes,
            }
        }
        _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    };
    if cursor.byte()? != 0 {
        return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField);
    }
    let encoded = decode_encoded_effects(cursor)?;
    Ok(TerminalMachineAlternative {
        key,
        applicability,
        size,
        latency: TerminalMachineLatencyKnowledge::StableBaselineUnavailable,
        encoded,
    })
}

fn decode_encoded_effects(
    cursor: &mut Cursor<'_>,
) -> Result<TerminalMachineEncodedEffects, TerminalPreAllocationMachineEffectDecodeError> {
    let external_operand_reads = decode_u16s(cursor)?;
    let external_operand_writes = decode_u16s(cursor)?;
    let implicit_unit_uses = decode_units(cursor)?;
    let implicit_unit_defs = decode_units(cursor)?;
    let implicit_unit_clobbers = decode_units(cursor)?;
    let memory = match cursor.byte()? {
        0 => TerminalMachineEncodedMemoryEffect::NoneV1,
        1 => TerminalMachineEncodedMemoryEffect::ReadActivationStackV1 {
            stack_pointer: omega_register_model::RegisterViewId(cursor.u16()?),
            byte_count: cursor.u16()?,
        },
        _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    };
    let stack = match cursor.byte()? {
        0 => TerminalMachineEncodedStackEffect::UnchangedV1,
        1 => TerminalMachineEncodedStackEffect::PopBytesV1 {
            stack_pointer: omega_register_model::RegisterViewId(cursor.u16()?),
            byte_count: cursor.u16()?,
        },
        _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    };
    let trap = match cursor.byte()? {
        0 => TerminalMachineEncodedTrapBehavior::NeverV1,
        1 => TerminalMachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    };
    let control = match cursor.byte()? {
        0 => TerminalMachineEncodedControlEffect::FallThroughV1,
        1 => TerminalMachineEncodedControlEffect::ConditionalRelativeBranchV1,
        2 => TerminalMachineEncodedControlEffect::ReturnFromActivationStackV1,
        3 => TerminalMachineEncodedControlEffect::ReturnIndirectRegisterV1 {
            target: omega_register_model::RegisterViewId(cursor.u16()?),
        },
        _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    };
    Ok(TerminalMachineEncodedEffects {
        external_operand_reads,
        external_operand_writes,
        implicit_unit_uses,
        implicit_unit_defs,
        implicit_unit_clobbers,
        memory,
        stack,
        trap,
        control,
    })
}

fn decode_u16s(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<u16>, TerminalPreAllocationMachineEffectDecodeError> {
    let count = cursor.length()?;
    let mut values = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        values.push(cursor.u16()?);
    }
    Ok(values)
}

pub(crate) fn decode_target(
    cursor: &mut Cursor<'_>,
) -> Result<NativeTarget, TerminalPreAllocationMachineEffectDecodeError> {
    let architecture = match cursor.byte()? {
        0 => Architecture::Aarch64,
        1 => Architecture::X86_64,
        _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    };
    let object_format = match cursor.byte()? {
        0 => ObjectFormat::Elf,
        1 => ObjectFormat::MachO,
        2 => ObjectFormat::Coff,
        _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    };
    let pointer_size = usize::try_from(cursor.u64()?)
        .map_err(|_| TerminalPreAllocationMachineEffectDecodeError::InvalidField)?;
    let pointer_alignment = usize::try_from(cursor.u64()?)
        .map_err(|_| TerminalPreAllocationMachineEffectDecodeError::InvalidField)?;
    Ok(NativeTarget {
        architecture,
        object_format,
        pointer_size,
        pointer_alignment,
    })
}

fn decode_constraint_key(
    cursor: &mut Cursor<'_>,
) -> Result<RegisterConstraintKey, TerminalPreAllocationMachineEffectDecodeError> {
    let family = match cursor.byte()? {
        0 => RegisterConstraintFamily::Call,
        1 => RegisterConstraintFamily::Return,
        2 => RegisterConstraintFamily::SystemCall,
        3 => RegisterConstraintFamily::InlineAssembly,
        4 => RegisterConstraintFamily::Instruction,
        _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    };
    Ok(RegisterConstraintKey {
        family,
        variant: cursor.u32()?,
    })
}

pub(crate) fn decode_units(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<RegisterUnitId>, TerminalPreAllocationMachineEffectDecodeError> {
    let count = cursor.length()?;
    let mut units = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        units.push(RegisterUnitId(cursor.u16()?));
    }
    Ok(units)
}

fn decode_ids<T>(
    cursor: &mut Cursor<'_>,
    constructor: impl Fn(u64) -> Option<T>,
) -> Result<Vec<T>, TerminalPreAllocationMachineEffectDecodeError> {
    let count = cursor.length()?;
    let mut values = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        values.push(
            constructor(cursor.u64()?)
                .ok_or(TerminalPreAllocationMachineEffectDecodeError::InvalidField)?,
        );
    }
    Ok(values)
}

fn decode_machine(
    cursor: &mut Cursor<'_>,
) -> Result<MachineId, TerminalPreAllocationMachineEffectDecodeError> {
    MachineId::new(cursor.u64()?).ok_or(TerminalPreAllocationMachineEffectDecodeError::InvalidField)
}

fn decode_ownership(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<OwnershipEvent>, TerminalPreAllocationMachineEffectDecodeError> {
    let count = cursor.length()?;
    let mut ownership = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        let event = match cursor.byte()? {
            1 => OwnershipEvent::ClaimTransfer(decode_claim_ids(cursor)?),
            2 => OwnershipEvent::ClaimCompletion(decode_claim_ids(cursor)?),
            3 => {
                let action_count = cursor.length()?;
                let mut actions = Vec::with_capacity(action_count.min(cursor.remaining()));
                for _ in 0..action_count {
                    actions.push(decode_cleanup(cursor)?);
                }
                OwnershipEvent::Cleanup(actions)
            }
            4 => OwnershipEvent::StructuralReturn(decode_claim_ids(cursor)?),
            5 => OwnershipEvent::CrashFrontier(decode_claim_ids(cursor)?),
            _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
        };
        ownership.push(event);
    }
    Ok(ownership)
}

fn decode_claim_ids(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<ClaimId>, TerminalPreAllocationMachineEffectDecodeError> {
    decode_ids(cursor, ClaimId::new)
}

fn decode_cleanup(
    cursor: &mut Cursor<'_>,
) -> Result<psi_terminal::TerminalAffineCleanupAction, TerminalPreAllocationMachineEffectDecodeError>
{
    Ok(match cursor.byte()? {
        1 => psi_terminal::TerminalAffineCleanupAction::DiscardRoot(decode_place(cursor)?),
        2 => psi_terminal::TerminalAffineCleanupAction::DiscardResidual(
            psi_terminal::StructuralAffineDiscard {
                place: decode_place(cursor)?,
                path: decode_path(cursor)?,
                structural_type: decode_structural_type(cursor)?,
            },
        ),
        3 => {
            let place = decode_place(cursor)?;
            let structural_type = decode_structural_type(cursor)?;
            let cleanup_machine = decode_machine(cursor)?;
            let cleanup_receiver = match cursor.byte()? {
                0 => None,
                1 => Some(decode_place(cursor)?),
                _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
            };
            let requirement_obligations = decode_ids(cursor, ObligationId::new)?;
            psi_terminal::TerminalAffineCleanupAction::InvokeNominal(
                psi_terminal::NominalAffineCleanup {
                    place,
                    structural_type,
                    cleanup_machine,
                    cleanup_receiver,
                    requirement_obligations,
                },
            )
        }
        _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
    })
}

fn decode_path(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<psi_terminal::StructuralPathSegment>, TerminalPreAllocationMachineEffectDecodeError>
{
    let count = cursor.length()?;
    let mut path = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        path.push(match cursor.byte()? {
            1 => {
                let length = cursor.length()?;
                let bytes = cursor.take(length)?;
                let name = std::str::from_utf8(bytes)
                    .map_err(|_| TerminalPreAllocationMachineEffectDecodeError::InvalidField)?;
                psi_terminal::StructuralPathSegment::Field(name.to_owned())
            }
            2 => psi_terminal::StructuralPathSegment::FixedIndex(cursor.u64()?),
            _ => return Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField),
        });
    }
    Ok(path)
}

fn decode_place(
    cursor: &mut Cursor<'_>,
) -> Result<PlaceId, TerminalPreAllocationMachineEffectDecodeError> {
    PlaceId::new(cursor.u64()?).ok_or(TerminalPreAllocationMachineEffectDecodeError::InvalidField)
}

fn decode_structural_type(
    cursor: &mut Cursor<'_>,
) -> Result<StructuralTypeId, TerminalPreAllocationMachineEffectDecodeError> {
    StructuralTypeId::new(cursor.u64()?)
        .ok_or(TerminalPreAllocationMachineEffectDecodeError::InvalidField)
}

fn decode_obligation(
    cursor: &mut Cursor<'_>,
) -> Result<ObligationId, TerminalPreAllocationMachineEffectDecodeError> {
    ObligationId::new(cursor.u64()?)
        .ok_or(TerminalPreAllocationMachineEffectDecodeError::InvalidField)
}

pub(crate) struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    pub(crate) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    pub(crate) fn take(
        &mut self,
        count: usize,
    ) -> Result<&'a [u8], TerminalPreAllocationMachineEffectDecodeError> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(TerminalPreAllocationMachineEffectDecodeError::Truncated)?;
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or(TerminalPreAllocationMachineEffectDecodeError::Truncated)?;
        self.position = end;
        Ok(bytes)
    }

    pub(crate) fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], TerminalPreAllocationMachineEffectDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| TerminalPreAllocationMachineEffectDecodeError::Truncated)
    }

    pub(crate) fn byte(&mut self) -> Result<u8, TerminalPreAllocationMachineEffectDecodeError> {
        Ok(self.take(1)?[0])
    }

    pub(crate) fn u16(&mut self) -> Result<u16, TerminalPreAllocationMachineEffectDecodeError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub(crate) fn u32(&mut self) -> Result<u32, TerminalPreAllocationMachineEffectDecodeError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub(crate) fn u64(&mut self) -> Result<u64, TerminalPreAllocationMachineEffectDecodeError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    pub(crate) fn length(
        &mut self,
    ) -> Result<usize, TerminalPreAllocationMachineEffectDecodeError> {
        usize::try_from(self.u64()?)
            .map_err(|_| TerminalPreAllocationMachineEffectDecodeError::InvalidField)
    }

    pub(crate) fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.position)
    }
}

#[cfg(test)]
mod tests {
    use omega_optimization_core::OptimizationUnitIdentity;
    use omega_optimization_unit::{FuelSettlement, PsiProvenance};
    use omega_register_model::{
        RegisterConstraintCatalogIdentity, RegisterConstraintFamily, RegisterConstraintKey,
        TargetRegisterEnvironmentIdentity,
    };
    use omega_terminal_selected_instructions::{
        TerminalMachineAlternativeKey, TerminalMachineEffectCatalogIdentity,
        TerminalSelectedInstructionPlanIdentity,
    };
    use psi_core::{EdgeId, FuelScheduleIdentity, MachineId, ObligationId, OperationId, ValueId};

    use super::*;

    fn plan() -> TerminalPreAllocationMachineEffectPlan {
        let mut plan = TerminalPreAllocationMachineEffectPlan {
            identity: TerminalPreAllocationMachineEffectIdentity::from_bytes([0; 32]),
            selected: TerminalSelectedInstructionPlanIdentity::from_bytes([1; 32]),
            optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            target: NativeTarget::linux_x64(),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([3; 32]),
            register_constraints: RegisterConstraintCatalogIdentity::from_bytes([4; 32]),
            machine_effect_catalog: TerminalMachineEffectCatalogIdentity::from_bytes([5; 32]),
            functions: vec![TerminalFunctionMachineEffects {
                machine: MachineId::new(1).unwrap(),
                blocks: vec![TerminalBlockMachineEffects {
                    block: TerminalSelectedBlockId(0),
                    instructions: vec![TerminalInstructionMachineEffects {
                        instruction: TerminalSelectedInstructionId(0),
                        kind: TerminalSelectedInstructionKind::CompareI64Zero,
                        constraint: RegisterConstraintKey {
                            family: RegisterConstraintFamily::Instruction,
                            variant: 4,
                        },
                        unit_uses: vec![RegisterUnitId(0)],
                        unit_defs: vec![RegisterUnitId(1)],
                        unit_clobbers: vec![RegisterUnitId(2)],
                        memory: TerminalMachineMemoryEffect::NoneV1,
                        trap: TerminalMachineTrapBehavior::NeverV1,
                        barrier: TerminalMachineBarrier::None,
                        call: TerminalMachineCallEffect::NoneV1,
                        cleanup: TerminalMachineCleanupEffect::NoneV1,
                        provenance: TerminalSelectedInstructionProvenance {
                            operations: vec![OperationId::new(2).unwrap()],
                            values: vec![ValueId::new(3).unwrap()],
                            edges: vec![EdgeId::new(4).unwrap()],
                            obligations: vec![ObligationId::new(5).unwrap()],
                            fuel: vec![
                                FuelSettlement {
                                    site: PsiProvenance::Operation(OperationId::new(2).unwrap()),
                                    units: 7,
                                },
                                FuelSettlement {
                                    site: PsiProvenance::Edge(EdgeId::new(4).unwrap()),
                                    units: 11,
                                },
                            ],
                        },
                        alternatives: vec![
                            TerminalMachineAlternative {
                                key: TerminalMachineAlternativeKey {
                                    family: TerminalMachineAlternativeFamily::CompareI64Zero,
                                    variant: 0,
                                },
                                applicability: TerminalMachineAlternativeApplicability::Always,
                                size: TerminalMachineSizeKnowledge::ExactBytes(3),
                                latency:
                                    TerminalMachineLatencyKnowledge::StableBaselineUnavailable,
                                encoded: TerminalMachineEncodedEffects::fallthrough_v1(
                                    vec![0],
                                    vec![],
                                ),
                            },
                            TerminalMachineAlternative {
                                key: TerminalMachineAlternativeKey {
                                    family: TerminalMachineAlternativeFamily::CompareI64Zero,
                                    variant: 1,
                                },
                                applicability: TerminalMachineAlternativeApplicability::
                                    ResultAliasesOperandAndDistinctFromOperand {
                                        result: 0,
                                        aliased_operand: 1,
                                        distinct_operand: 2,
                                    },
                                size: TerminalMachineSizeKnowledge::EncoderResolved {
                                    minimum_bytes: 2,
                                    maximum_bytes: Some(6),
                                },
                                latency:
                                    TerminalMachineLatencyKnowledge::StableBaselineUnavailable,
                                encoded: TerminalMachineEncodedEffects::fallthrough_v1(
                                    vec![0, 1],
                                    vec![2],
                                ),
                            },
                            TerminalMachineAlternative {
                                key: TerminalMachineAlternativeKey {
                                    family: TerminalMachineAlternativeFamily::CompareI64Zero,
                                    variant: 2,
                                },
                                applicability: TerminalMachineAlternativeApplicability::
                                    AtLeastOneOperandDoesNotAliasView {
                                        left: 0,
                                        right: 1,
                                        excluded_view: omega_register_model::RegisterViewId(12),
                                    },
                                size: TerminalMachineSizeKnowledge::ExactBytes(4),
                                latency:
                                    TerminalMachineLatencyKnowledge::StableBaselineUnavailable,
                                encoded: TerminalMachineEncodedEffects {
                                    external_operand_reads: vec![],
                                    external_operand_writes: vec![],
                                    implicit_unit_uses: vec![RegisterUnitId(0)],
                                    implicit_unit_defs: vec![RegisterUnitId(1)],
                                    implicit_unit_clobbers: vec![],
                                    memory:
                                        TerminalMachineEncodedMemoryEffect::ReadActivationStackV1 {
                                            stack_pointer:
                                                omega_register_model::RegisterViewId(12),
                                            byte_count: 8,
                                        },
                                    stack: TerminalMachineEncodedStackEffect::PopBytesV1 {
                                        stack_pointer: omega_register_model::RegisterViewId(12),
                                        byte_count: 8,
                                    },
                                    trap: TerminalMachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                                    control: TerminalMachineEncodedControlEffect::ReturnFromActivationStackV1,
                                },
                            },
                        ],
                    }],
                }],
            }],
            structural_unit_functions: Vec::new(),
        };
        let return_instruction = TerminalInstructionMachineEffects {
            instruction: TerminalSelectedInstructionId(1),
            kind: TerminalSelectedInstructionKind::ReturnUnit,
            constraint: RegisterConstraintKey {
                family: RegisterConstraintFamily::Return,
                variant: 3,
            },
            unit_uses: vec![RegisterUnitId(4)],
            unit_defs: vec![RegisterUnitId(4), RegisterUnitId(5)],
            unit_clobbers: Vec::new(),
            memory: TerminalMachineMemoryEffect::NoneV1,
            trap: TerminalMachineTrapBehavior::NeverV1,
            barrier: TerminalMachineBarrier::ControlFlow,
            call: TerminalMachineCallEffect::NoneV1,
            cleanup: TerminalMachineCleanupEffect::NoneV1,
            provenance: TerminalSelectedInstructionProvenance::default(),
            alternatives: Vec::new(),
        };
        let call_constraint = RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant: 2,
        };
        plan.structural_unit_functions
            .push(TerminalStructuralUnitFunctionMachineEffects {
                machine: MachineId::new(6).unwrap(),
                block: TerminalSelectedBlockId(0),
                call: Some(TerminalStructuralUnitCallMachineEffects {
                    instruction: TerminalSelectedInstructionId(0),
                    operation: OperationId::new(7).unwrap(),
                    callee: MachineId::new(8).unwrap(),
                    constraint: call_constraint,
                    unit_uses: vec![RegisterUnitId(1), RegisterUnitId(2)],
                    unit_defs: vec![RegisterUnitId(3)],
                    unit_clobbers: vec![RegisterUnitId(4)],
                    layout: TerminalSelectedMicrosoftX64OwnedIndirectPairLayout {
                        shadow_byte_count: 32,
                        outgoing_frame_byte_count: 72,
                        pre_call_stack_alignment: 16,
                        bindings: [
                            TerminalSelectedStructuralUnitIndirectBinding {
                                parameter_index: 0,
                                pointer: omega_terminal_target_operations::MachineRegister::X86Rcx,
                                copy_stack_byte_offset: 32,
                                byte_count: 16,
                                alignment: 8,
                            },
                            TerminalSelectedStructuralUnitIndirectBinding {
                                parameter_index: 1,
                                pointer: omega_terminal_target_operations::MachineRegister::X86Rdx,
                                copy_stack_byte_offset: 48,
                                byte_count: 16,
                                alignment: 8,
                            },
                        ],
                    },
                    effect: EffectLink { input: 9, output: 10 },
                    ownership: vec![
                        OwnershipEvent::ClaimTransfer(vec![ClaimId::new(11).unwrap()]),
                        OwnershipEvent::Cleanup(Vec::new()),
                    ],
                    claim_transfers: vec![psi_terminal::ClaimTransfer {
                        claim: ClaimId::new(11).unwrap(),
                        argument_index: 0,
                    }],
                    provenance: TerminalSelectedInstructionProvenance {
                        operations: vec![OperationId::new(7).unwrap()],
                        ..Default::default()
                    },
                    declaration: TerminalStructuralUnitCallEffectDeclaration {
                        constraint: call_constraint,
                        memory: TerminalStructuralUnitCallMemoryEffect::ReadOwnedIndirectPairWriteCallerCopiesV1 {
                            root_byte_count: 16,
                            copy_stack_byte_offsets: [32, 48],
                        },
                        frame: TerminalStructuralUnitCallFrameEffect::BalancedCallerFrameV1 {
                            frame_byte_count: 72,
                            shadow_byte_count: 32,
                            pre_call_stack_alignment: 16,
                        },
                        trap: TerminalMachineTrapBehavior::MayArchitecturalFaultV1,
                        barrier: TerminalStructuralUnitCallBarrier::CallV1,
                        call: TerminalStructuralUnitCallEffect::DirectInternalUnitV1,
                        cleanup: TerminalMachineCleanupEffect::NoneV1,
                    },
                }),
                return_instruction,
                return_effect: EffectLink { input: 10, output: 11 },
                return_ownership: vec![OwnershipEvent::StructuralReturn(vec![
                    ClaimId::new(12).unwrap(),
                ])],
            });
        plan.identity = terminal_pre_allocation_machine_effect_identity(&plan);
        plan
    }

    #[test]
    fn codec_round_trips_complete_effect_content() {
        let source = plan();
        let encoded = source.encode();

        assert_eq!(
            TerminalPreAllocationMachineEffectPlan::decode(&encoded).unwrap(),
            source
        );
    }

    #[test]
    fn codec_rejects_framing_corruption_and_stale_identity() {
        let source = plan();
        let encoded = source.encode();

        let mut wrong_magic = encoded.clone();
        wrong_magic[0] ^= 1;
        assert_eq!(
            TerminalPreAllocationMachineEffectPlan::decode(&wrong_magic),
            Err(TerminalPreAllocationMachineEffectDecodeError::WrongMagic)
        );

        let mut unsupported_version = encoded.clone();
        unsupported_version[8..12].copy_from_slice(&2_u32.to_le_bytes());
        assert_eq!(
            TerminalPreAllocationMachineEffectPlan::decode(&unsupported_version),
            Err(TerminalPreAllocationMachineEffectDecodeError::UnsupportedVersion(2))
        );

        let mut stale_identity = encoded.clone();
        stale_identity[12] ^= 1;
        assert_eq!(
            TerminalPreAllocationMachineEffectPlan::decode(&stale_identity),
            Err(TerminalPreAllocationMachineEffectDecodeError::InvalidIdentity)
        );

        let mut invalid_target = encoded.clone();
        invalid_target[112] = u8::MAX;
        assert_eq!(
            TerminalPreAllocationMachineEffectPlan::decode(&invalid_target),
            Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField)
        );

        assert_eq!(
            TerminalPreAllocationMachineEffectPlan::decode(&encoded[..encoded.len() - 1]),
            Err(TerminalPreAllocationMachineEffectDecodeError::Truncated)
        );

        let mut trailing = encoded;
        trailing.push(0);
        assert_eq!(
            TerminalPreAllocationMachineEffectPlan::decode(&trailing),
            Err(TerminalPreAllocationMachineEffectDecodeError::TrailingBytes)
        );
    }

    #[test]
    fn structural_call_content_is_authenticated_and_closed() {
        let source = plan();
        let mut substituted = source.clone();
        substituted.structural_unit_functions[0]
            .call
            .as_mut()
            .unwrap()
            .callee = MachineId::new(99).unwrap();
        assert_ne!(
            terminal_pre_allocation_machine_effect_identity(&substituted),
            source.identity
        );
        assert_eq!(
            TerminalPreAllocationMachineEffectPlan::decode(&substituted.encode()),
            Err(TerminalPreAllocationMachineEffectDecodeError::InvalidIdentity)
        );

        let mut invalid_declaration_tag = source.encode();
        let declaration_tag = invalid_declaration_tag.len() - 58;
        invalid_declaration_tag[declaration_tag] = u8::MAX;
        assert!(matches!(
            TerminalPreAllocationMachineEffectPlan::decode(&invalid_declaration_tag),
            Err(TerminalPreAllocationMachineEffectDecodeError::InvalidField)
                | Err(TerminalPreAllocationMachineEffectDecodeError::InvalidIdentity)
        ));
    }
}
