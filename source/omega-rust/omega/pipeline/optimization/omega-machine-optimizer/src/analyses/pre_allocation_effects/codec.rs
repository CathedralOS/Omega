use omega_optimization_core::{AcceptedObligationFactIdentity, OptimizationUnitIdentity};

use super::identity;
use omega_optimization_unit::{EffectLink, FuelSettlement, OwnershipEvent, PsiProvenance};
use omega_register_model::{
    RegisterConstraintCatalogIdentity, RegisterConstraintFamily, RegisterConstraintKey,
    RegisterUnitId, TargetRegisterEnvironmentIdentity,
};
use omega_selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineAlternativeKey, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectCatalogIdentity, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
    MachineLatencyKnowledge, MachineMemoryEffect, MachineSizeKnowledge, MachineTrapBehavior,
    SelectedBlockId, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionPlanIdentity, SelectedInstructionProvenance,
    SelectedMicrosoftX64OwnedIndirectPairLayout, SelectedStructuralUnitIndirectBinding,
    StructuralUnitCallBarrier, StructuralUnitCallEffect, StructuralUnitCallEffectDeclaration,
    StructuralUnitCallFrameEffect, StructuralUnitCallMemoryEffect,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use psi_core::{
    ClaimId, EdgeId, FuelScheduleIdentity, IntegerValue, MachineId, ObligationId, OperationId,
    PlaceId, StructuralTypeId, ValueId,
};

use crate::{
    BlockMachineEffects, FunctionMachineEffects, InstructionMachineEffects,
    PreAllocationMachineEffectIdentity, PreAllocationMachineEffectPlan,
    StructuralUnitCallMachineEffects, StructuralUnitFunctionMachineEffects,
    pre_allocation_machine_effect_identity,
};

const MAGIC: &[u8; 8] = b"OMGMFX\0\0";
const VERSION: u32 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreAllocationMachineEffectDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    InvalidField,
    InvalidIdentity,
    TrailingBytes,
}

impl std::fmt::Display for PreAllocationMachineEffectDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid pre-allocation machine-effect artifact: {self:?}"
        )
    }
}

impl std::error::Error for PreAllocationMachineEffectDecodeError {}

pub(crate) fn encode_terminal_pre_allocation_machine_effect_plan(
    plan: &PreAllocationMachineEffectPlan,
) -> Vec<u8> {
    let content = identity::encode_terminal_pre_allocation_machine_effect_content(plan);
    let mut encoded = Vec::with_capacity(44 + content.len());
    encoded.extend_from_slice(MAGIC);
    encoded.extend_from_slice(&VERSION.to_le_bytes());
    encoded.extend_from_slice(&plan.identity.bytes());
    encoded.extend_from_slice(&content);
    encoded
}

pub(crate) fn decode_terminal_pre_allocation_machine_effect_plan(
    encoded: &[u8],
) -> Result<PreAllocationMachineEffectPlan, PreAllocationMachineEffectDecodeError> {
    let mut cursor = Cursor::new(encoded);
    if cursor.take(8)? != MAGIC {
        return Err(PreAllocationMachineEffectDecodeError::WrongMagic);
    }
    let version = cursor.u32()?;
    if version != VERSION {
        return Err(PreAllocationMachineEffectDecodeError::UnsupportedVersion(
            version,
        ));
    }
    let identity = PreAllocationMachineEffectIdentity::from_bytes(cursor.array()?);
    let selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
    let optimization_unit = OptimizationUnitIdentity::from_bytes(cursor.array()?);
    let fuel_schedule = FuelScheduleIdentity::new(cursor.u32()?)
        .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?;
    let target = decode_target(&mut cursor)?;
    let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
    let register_constraints = RegisterConstraintCatalogIdentity::from_bytes(cursor.array()?);
    let machine_effect_catalog = MachineEffectCatalogIdentity::from_bytes(cursor.array()?);
    let function_count = cursor.length()?;
    let mut functions = Vec::with_capacity(function_count.min(cursor.remaining()));
    for _ in 0..function_count {
        let machine = MachineId::new(cursor.u64()?)
            .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?;
        let block_count = cursor.length()?;
        let mut blocks = Vec::with_capacity(block_count.min(cursor.remaining()));
        for _ in 0..block_count {
            let block = SelectedBlockId(cursor.u32()?);
            let instruction_count = cursor.length()?;
            let mut instructions = Vec::with_capacity(instruction_count.min(cursor.remaining()));
            for _ in 0..instruction_count {
                instructions.push(decode_instruction(&mut cursor)?);
            }
            blocks.push(BlockMachineEffects {
                block,
                instructions,
            });
        }
        functions.push(FunctionMachineEffects { machine, blocks });
    }
    let structural_count = cursor.length()?;
    let mut structural_unit_functions =
        Vec::with_capacity(structural_count.min(cursor.remaining()));
    for _ in 0..structural_count {
        structural_unit_functions.push(decode_structural_function(&mut cursor)?);
    }
    if cursor.remaining() != 0 {
        return Err(PreAllocationMachineEffectDecodeError::TrailingBytes);
    }
    let plan = PreAllocationMachineEffectPlan {
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
    if plan.identity != pre_allocation_machine_effect_identity(&plan) {
        return Err(PreAllocationMachineEffectDecodeError::InvalidIdentity);
    }
    Ok(plan)
}

fn decode_structural_function(
    cursor: &mut Cursor<'_>,
) -> Result<StructuralUnitFunctionMachineEffects, PreAllocationMachineEffectDecodeError> {
    let machine = decode_machine(cursor)?;
    let block = SelectedBlockId(cursor.u32()?);
    let call = match cursor.byte()? {
        0 => None,
        1 => Some(decode_structural_call(cursor)?),
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    let return_instruction = decode_instruction(cursor)?;
    let return_effect = decode_effect_link(cursor)?;
    let return_ownership = decode_ownership(cursor)?;
    Ok(StructuralUnitFunctionMachineEffects {
        machine,
        block,
        call,
        return_instruction,
        return_effect,
        return_ownership,
    })
}

pub(crate) fn decode_structural_call(
    cursor: &mut Cursor<'_>,
) -> Result<StructuralUnitCallMachineEffects, PreAllocationMachineEffectDecodeError> {
    let instruction = SelectedInstructionId(cursor.u32()?);
    let operation = OperationId::new(cursor.u64()?)
        .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?;
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
                .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
            argument_index: cursor.u32()?,
        });
    }
    let provenance = decode_provenance(cursor)?;
    let declaration = decode_structural_declaration(cursor)?;
    Ok(StructuralUnitCallMachineEffects {
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
) -> Result<SelectedMicrosoftX64OwnedIndirectPairLayout, PreAllocationMachineEffectDecodeError> {
    let shadow_byte_count = cursor.u32()?;
    let outgoing_frame_byte_count = cursor.u32()?;
    let pre_call_stack_alignment = cursor.u16()?;
    let mut bindings = Vec::with_capacity(2);
    for _ in 0..2 {
        bindings.push(SelectedStructuralUnitIndirectBinding {
            parameter_index: usize::try_from(cursor.u64()?)
                .map_err(|_| PreAllocationMachineEffectDecodeError::InvalidField)?,
            pointer: decode_machine_register(cursor)?,
            copy_stack_byte_offset: cursor.u32()?,
            byte_count: cursor.u16()?,
            alignment: cursor.u16()?,
        });
    }
    Ok(SelectedMicrosoftX64OwnedIndirectPairLayout {
        shadow_byte_count,
        outgoing_frame_byte_count,
        pre_call_stack_alignment,
        bindings: bindings
            .try_into()
            .map_err(|_| PreAllocationMachineEffectDecodeError::InvalidField)?,
    })
}

fn decode_machine_register(
    cursor: &mut Cursor<'_>,
) -> Result<omega_target_operations::MachineRegister, PreAllocationMachineEffectDecodeError> {
    use omega_target_operations::MachineRegister as R;
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
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    })
}

pub(crate) fn decode_effect_link(
    cursor: &mut Cursor<'_>,
) -> Result<EffectLink, PreAllocationMachineEffectDecodeError> {
    Ok(EffectLink {
        input: cursor.u64()?,
        output: cursor.u64()?,
    })
}

fn decode_structural_declaration(
    cursor: &mut Cursor<'_>,
) -> Result<StructuralUnitCallEffectDeclaration, PreAllocationMachineEffectDecodeError> {
    let constraint = decode_constraint_key(cursor)?;
    let memory = match cursor.byte()? {
        1 => StructuralUnitCallMemoryEffect::ReadOwnedIndirectPairWriteCallerCopiesV1 {
            root_byte_count: cursor.u16()?,
            copy_stack_byte_offsets: [cursor.u32()?, cursor.u32()?],
        },
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    let frame = match cursor.byte()? {
        1 => StructuralUnitCallFrameEffect::BalancedCallerFrameV1 {
            frame_byte_count: cursor.u32()?,
            shadow_byte_count: cursor.u32()?,
            pre_call_stack_alignment: cursor.u16()?,
        },
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    let trap = match cursor.byte()? {
        0 => MachineTrapBehavior::NeverV1,
        1 => MachineTrapBehavior::MayArchitecturalFaultV1,
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    if cursor.byte()? != 1 || cursor.byte()? != 1 || cursor.byte()? != 0 {
        return Err(PreAllocationMachineEffectDecodeError::InvalidField);
    }
    Ok(StructuralUnitCallEffectDeclaration {
        constraint,
        memory,
        frame,
        trap,
        barrier: StructuralUnitCallBarrier::CallV1,
        call: StructuralUnitCallEffect::DirectInternalUnitV1,
        cleanup: MachineCleanupEffect::NoneV1,
    })
}

fn decode_instruction(
    cursor: &mut Cursor<'_>,
) -> Result<InstructionMachineEffects, PreAllocationMachineEffectDecodeError> {
    let instruction = SelectedInstructionId(cursor.u32()?);
    let kind = decode_kind(cursor)?;
    let constraint = decode_constraint_key(cursor)?;
    let unit_uses = decode_units(cursor)?;
    let unit_defs = decode_units(cursor)?;
    let unit_clobbers = decode_units(cursor)?;
    if cursor.byte()? != 0 || cursor.byte()? != 0 {
        return Err(PreAllocationMachineEffectDecodeError::InvalidField);
    }
    let barrier = match cursor.byte()? {
        0 => MachineBarrier::None,
        1 => MachineBarrier::ControlFlow,
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    if cursor.byte()? != 0 || cursor.byte()? != 0 {
        return Err(PreAllocationMachineEffectDecodeError::InvalidField);
    }
    let provenance = decode_provenance(cursor)?;
    let alternative_count = cursor.length()?;
    let mut alternatives = Vec::with_capacity(alternative_count.min(cursor.remaining()));
    for _ in 0..alternative_count {
        alternatives.push(decode_alternative(cursor)?);
    }
    Ok(InstructionMachineEffects {
        instruction,
        kind,
        constraint,
        unit_uses,
        unit_defs,
        unit_clobbers,
        memory: MachineMemoryEffect::NoneV1,
        trap: MachineTrapBehavior::NeverV1,
        barrier,
        call: MachineCallEffect::NoneV1,
        cleanup: MachineCleanupEffect::NoneV1,
        provenance,
        alternatives,
    })
}

fn decode_kind(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedInstructionKind, PreAllocationMachineEffectDecodeError> {
    Ok(match cursor.byte()? {
        0 => SelectedInstructionKind::CompareI64Zero,
        1 => SelectedInstructionKind::MaterializeI64 {
            value: decode_integer(cursor)?,
        },
        2 => SelectedInstructionKind::CopyI64,
        3 => SelectedInstructionKind::ExactAddI64 {
            obligation: decode_obligation(cursor)?,
            accepted_fact: AcceptedObligationFactIdentity::from_bytes(cursor.array()?),
        },
        4 => SelectedInstructionKind::ExactAddI64Immediate {
            immediate: decode_integer(cursor)?,
            obligation: decode_obligation(cursor)?,
            accepted_fact: AcceptedObligationFactIdentity::from_bytes(cursor.array()?),
        },
        5 => SelectedInstructionKind::ExactSubtractI64 {
            obligation: decode_obligation(cursor)?,
            accepted_fact: AcceptedObligationFactIdentity::from_bytes(cursor.array()?),
        },
        6 => SelectedInstructionKind::ConditionalBranchNonZero,
        7 => SelectedInstructionKind::ReturnI64,
        8 => SelectedInstructionKind::ExactSubtractI64Immediate {
            immediate: decode_integer(cursor)?,
            obligation: decode_obligation(cursor)?,
            accepted_fact: AcceptedObligationFactIdentity::from_bytes(cursor.array()?),
        },
        9 => SelectedInstructionKind::ReturnUnit,
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    })
}

fn decode_integer(
    cursor: &mut Cursor<'_>,
) -> Result<IntegerValue, PreAllocationMachineEffectDecodeError> {
    match cursor.byte()? {
        0 => Ok(IntegerValue::Signed(i128::from_le_bytes(cursor.array()?))),
        1 => Ok(IntegerValue::Unsigned(u128::from_le_bytes(cursor.array()?))),
        _ => Err(PreAllocationMachineEffectDecodeError::InvalidField),
    }
}

pub(crate) fn decode_provenance(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedInstructionProvenance, PreAllocationMachineEffectDecodeError> {
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
                OperationId::new(raw).ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
            ),
            1 => PsiProvenance::Edge(
                EdgeId::new(raw).ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
            ),
            _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
        };
        fuel.push(FuelSettlement {
            site,
            units: cursor.u64()?,
        });
    }
    Ok(SelectedInstructionProvenance {
        operations,
        values,
        edges,
        obligations,
        fuel,
    })
}

pub(crate) fn decode_alternative(
    cursor: &mut Cursor<'_>,
) -> Result<MachineAlternative, PreAllocationMachineEffectDecodeError> {
    let family = match cursor.byte()? {
        0 => MachineAlternativeFamily::CompareI64Zero,
        1 => MachineAlternativeFamily::MaterializeI64,
        2 => MachineAlternativeFamily::CopyI64,
        3 => MachineAlternativeFamily::ExactAddI64,
        4 => MachineAlternativeFamily::ExactAddI64Immediate,
        5 => MachineAlternativeFamily::ExactSubtractI64,
        6 => MachineAlternativeFamily::ConditionalBranchNonZero,
        7 => MachineAlternativeFamily::ReturnI64,
        8 => MachineAlternativeFamily::ExactSubtractI64Immediate,
        9 => MachineAlternativeFamily::ReturnUnit,
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    let key = MachineAlternativeKey {
        family,
        variant: cursor.u32()?,
    };
    let applicability = match cursor.byte()? {
        0 => MachineAlternativeApplicability::Always,
        1 => MachineAlternativeApplicability::ResultAliasesOperand {
            result: cursor.u16()?,
            operand: cursor.u16()?,
        },
        2 => MachineAlternativeApplicability::ResultAliasesOperandAndDistinctFromOperand {
            result: cursor.u16()?,
            aliased_operand: cursor.u16()?,
            distinct_operand: cursor.u16()?,
        },
        3 => MachineAlternativeApplicability::ResultAliasesOperands {
            result: cursor.u16()?,
            left: cursor.u16()?,
            right: cursor.u16()?,
        },
        4 => MachineAlternativeApplicability::ResultDistinctFromOperands {
            result: cursor.u16()?,
            left: cursor.u16()?,
            right: cursor.u16()?,
        },
        5 => MachineAlternativeApplicability::AtLeastOneOperandDoesNotAliasView {
            left: cursor.u16()?,
            right: cursor.u16()?,
            excluded_view: omega_register_model::RegisterViewId(cursor.u16()?),
        },
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    let size = match cursor.byte()? {
        0 => MachineSizeKnowledge::ExactBytes(cursor.u16()?),
        1 => {
            let minimum_bytes = cursor.u16()?;
            let maximum_bytes = match cursor.byte()? {
                0 => None,
                1 => Some(cursor.u16()?),
                _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
            };
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes,
                maximum_bytes,
            }
        }
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    if cursor.byte()? != 0 {
        return Err(PreAllocationMachineEffectDecodeError::InvalidField);
    }
    let encoded = decode_encoded_effects(cursor)?;
    Ok(MachineAlternative {
        key,
        applicability,
        size,
        latency: MachineLatencyKnowledge::StableBaselineUnavailable,
        encoded,
    })
}

fn decode_encoded_effects(
    cursor: &mut Cursor<'_>,
) -> Result<MachineEncodedEffects, PreAllocationMachineEffectDecodeError> {
    let external_operand_reads = decode_u16s(cursor)?;
    let external_operand_writes = decode_u16s(cursor)?;
    let implicit_unit_uses = decode_units(cursor)?;
    let implicit_unit_defs = decode_units(cursor)?;
    let implicit_unit_clobbers = decode_units(cursor)?;
    let memory = match cursor.byte()? {
        0 => MachineEncodedMemoryEffect::NoneV1,
        1 => MachineEncodedMemoryEffect::ReadActivationStackV1 {
            stack_pointer: omega_register_model::RegisterViewId(cursor.u16()?),
            byte_count: cursor.u16()?,
        },
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    let stack = match cursor.byte()? {
        0 => MachineEncodedStackEffect::UnchangedV1,
        1 => MachineEncodedStackEffect::PopBytesV1 {
            stack_pointer: omega_register_model::RegisterViewId(cursor.u16()?),
            byte_count: cursor.u16()?,
        },
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    let trap = match cursor.byte()? {
        0 => MachineEncodedTrapBehavior::NeverV1,
        1 => MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    let control = match cursor.byte()? {
        0 => MachineEncodedControlEffect::FallThroughV1,
        1 => MachineEncodedControlEffect::ConditionalRelativeBranchV1,
        2 => MachineEncodedControlEffect::ReturnFromActivationStackV1,
        3 => MachineEncodedControlEffect::ReturnIndirectRegisterV1 {
            target: omega_register_model::RegisterViewId(cursor.u16()?),
        },
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    Ok(MachineEncodedEffects {
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

fn decode_u16s(cursor: &mut Cursor<'_>) -> Result<Vec<u16>, PreAllocationMachineEffectDecodeError> {
    let count = cursor.length()?;
    let mut values = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        values.push(cursor.u16()?);
    }
    Ok(values)
}

pub(crate) fn decode_target(
    cursor: &mut Cursor<'_>,
) -> Result<NativeTarget, PreAllocationMachineEffectDecodeError> {
    let architecture = match cursor.byte()? {
        0 => Architecture::Aarch64,
        1 => Architecture::X86_64,
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    let object_format = match cursor.byte()? {
        0 => ObjectFormat::Elf,
        1 => ObjectFormat::MachO,
        2 => ObjectFormat::Coff,
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    let pointer_size = usize::try_from(cursor.u64()?)
        .map_err(|_| PreAllocationMachineEffectDecodeError::InvalidField)?;
    let pointer_alignment = usize::try_from(cursor.u64()?)
        .map_err(|_| PreAllocationMachineEffectDecodeError::InvalidField)?;
    Ok(NativeTarget {
        architecture,
        object_format,
        pointer_size,
        pointer_alignment,
    })
}

fn decode_constraint_key(
    cursor: &mut Cursor<'_>,
) -> Result<RegisterConstraintKey, PreAllocationMachineEffectDecodeError> {
    let family = match cursor.byte()? {
        0 => RegisterConstraintFamily::Call,
        1 => RegisterConstraintFamily::Return,
        2 => RegisterConstraintFamily::SystemCall,
        3 => RegisterConstraintFamily::InlineAssembly,
        4 => RegisterConstraintFamily::Instruction,
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    };
    Ok(RegisterConstraintKey {
        family,
        variant: cursor.u32()?,
    })
}

pub(crate) fn decode_units(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<RegisterUnitId>, PreAllocationMachineEffectDecodeError> {
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
) -> Result<Vec<T>, PreAllocationMachineEffectDecodeError> {
    let count = cursor.length()?;
    let mut values = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        values.push(
            constructor(cursor.u64()?)
                .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
        );
    }
    Ok(values)
}

fn decode_machine(
    cursor: &mut Cursor<'_>,
) -> Result<MachineId, PreAllocationMachineEffectDecodeError> {
    MachineId::new(cursor.u64()?).ok_or(PreAllocationMachineEffectDecodeError::InvalidField)
}

pub(crate) fn decode_ownership(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<OwnershipEvent>, PreAllocationMachineEffectDecodeError> {
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
            _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
        };
        ownership.push(event);
    }
    Ok(ownership)
}

fn decode_claim_ids(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<ClaimId>, PreAllocationMachineEffectDecodeError> {
    decode_ids(cursor, ClaimId::new)
}

fn decode_cleanup(
    cursor: &mut Cursor<'_>,
) -> Result<psi_terminal::TerminalAffineCleanupAction, PreAllocationMachineEffectDecodeError> {
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
                _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
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
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    })
}

fn decode_path(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<psi_terminal::StructuralPathSegment>, PreAllocationMachineEffectDecodeError> {
    let count = cursor.length()?;
    let mut path = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        path.push(match cursor.byte()? {
            1 => {
                let length = cursor.length()?;
                let bytes = cursor.take(length)?;
                let name = std::str::from_utf8(bytes)
                    .map_err(|_| PreAllocationMachineEffectDecodeError::InvalidField)?;
                psi_terminal::StructuralPathSegment::Field(name.to_owned())
            }
            2 => psi_terminal::StructuralPathSegment::FixedIndex(cursor.u64()?),
            _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
        });
    }
    Ok(path)
}

fn decode_place(cursor: &mut Cursor<'_>) -> Result<PlaceId, PreAllocationMachineEffectDecodeError> {
    PlaceId::new(cursor.u64()?).ok_or(PreAllocationMachineEffectDecodeError::InvalidField)
}

fn decode_structural_type(
    cursor: &mut Cursor<'_>,
) -> Result<StructuralTypeId, PreAllocationMachineEffectDecodeError> {
    StructuralTypeId::new(cursor.u64()?).ok_or(PreAllocationMachineEffectDecodeError::InvalidField)
}

fn decode_obligation(
    cursor: &mut Cursor<'_>,
) -> Result<ObligationId, PreAllocationMachineEffectDecodeError> {
    ObligationId::new(cursor.u64()?).ok_or(PreAllocationMachineEffectDecodeError::InvalidField)
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
    ) -> Result<&'a [u8], PreAllocationMachineEffectDecodeError> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(PreAllocationMachineEffectDecodeError::Truncated)?;
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or(PreAllocationMachineEffectDecodeError::Truncated)?;
        self.position = end;
        Ok(bytes)
    }

    pub(crate) fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], PreAllocationMachineEffectDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| PreAllocationMachineEffectDecodeError::Truncated)
    }

    pub(crate) fn byte(&mut self) -> Result<u8, PreAllocationMachineEffectDecodeError> {
        Ok(self.take(1)?[0])
    }

    pub(crate) fn u16(&mut self) -> Result<u16, PreAllocationMachineEffectDecodeError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub(crate) fn u32(&mut self) -> Result<u32, PreAllocationMachineEffectDecodeError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub(crate) fn u64(&mut self) -> Result<u64, PreAllocationMachineEffectDecodeError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    pub(crate) fn length(&mut self) -> Result<usize, PreAllocationMachineEffectDecodeError> {
        usize::try_from(self.u64()?)
            .map_err(|_| PreAllocationMachineEffectDecodeError::InvalidField)
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
    use omega_selected_instructions::{
        MachineAlternativeKey, MachineEffectCatalogIdentity, SelectedInstructionPlanIdentity,
    };
    use psi_core::{EdgeId, FuelScheduleIdentity, MachineId, ObligationId, OperationId, ValueId};

    use super::*;

    fn plan() -> PreAllocationMachineEffectPlan {
        let mut plan = PreAllocationMachineEffectPlan {
            identity: PreAllocationMachineEffectIdentity::from_bytes([0; 32]),
            selected: SelectedInstructionPlanIdentity::from_bytes([1; 32]),
            optimization_unit: OptimizationUnitIdentity::from_bytes([2; 32]),
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            target: NativeTarget::linux_x64(),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([3; 32]),
            register_constraints: RegisterConstraintCatalogIdentity::from_bytes([4; 32]),
            machine_effect_catalog: MachineEffectCatalogIdentity::from_bytes([5; 32]),
            functions: vec![FunctionMachineEffects {
                machine: MachineId::new(1).unwrap(),
                blocks: vec![BlockMachineEffects {
                    block: SelectedBlockId(0),
                    instructions: vec![InstructionMachineEffects {
                        instruction: SelectedInstructionId(0),
                        kind: SelectedInstructionKind::CompareI64Zero,
                        constraint: RegisterConstraintKey {
                            family: RegisterConstraintFamily::Instruction,
                            variant: 4,
                        },
                        unit_uses: vec![RegisterUnitId(0)],
                        unit_defs: vec![RegisterUnitId(1)],
                        unit_clobbers: vec![RegisterUnitId(2)],
                        memory: MachineMemoryEffect::NoneV1,
                        trap: MachineTrapBehavior::NeverV1,
                        barrier: MachineBarrier::None,
                        call: MachineCallEffect::NoneV1,
                        cleanup: MachineCleanupEffect::NoneV1,
                        provenance: SelectedInstructionProvenance {
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
                            MachineAlternative {
                                key: MachineAlternativeKey {
                                    family: MachineAlternativeFamily::CompareI64Zero,
                                    variant: 0,
                                },
                                applicability: MachineAlternativeApplicability::Always,
                                size: MachineSizeKnowledge::ExactBytes(3),
                                latency:
                                    MachineLatencyKnowledge::StableBaselineUnavailable,
                                encoded: MachineEncodedEffects::fallthrough_v1(
                                    vec![0],
                                    vec![],
                                ),
                            },
                            MachineAlternative {
                                key: MachineAlternativeKey {
                                    family: MachineAlternativeFamily::CompareI64Zero,
                                    variant: 1,
                                },
                                applicability: MachineAlternativeApplicability::
                                    ResultAliasesOperandAndDistinctFromOperand {
                                        result: 0,
                                        aliased_operand: 1,
                                        distinct_operand: 2,
                                    },
                                size: MachineSizeKnowledge::EncoderResolved {
                                    minimum_bytes: 2,
                                    maximum_bytes: Some(6),
                                },
                                latency:
                                    MachineLatencyKnowledge::StableBaselineUnavailable,
                                encoded: MachineEncodedEffects::fallthrough_v1(
                                    vec![0, 1],
                                    vec![2],
                                ),
                            },
                            MachineAlternative {
                                key: MachineAlternativeKey {
                                    family: MachineAlternativeFamily::CompareI64Zero,
                                    variant: 2,
                                },
                                applicability: MachineAlternativeApplicability::
                                    AtLeastOneOperandDoesNotAliasView {
                                        left: 0,
                                        right: 1,
                                        excluded_view: omega_register_model::RegisterViewId(12),
                                    },
                                size: MachineSizeKnowledge::ExactBytes(4),
                                latency:
                                    MachineLatencyKnowledge::StableBaselineUnavailable,
                                encoded: MachineEncodedEffects {
                                    external_operand_reads: vec![],
                                    external_operand_writes: vec![],
                                    implicit_unit_uses: vec![RegisterUnitId(0)],
                                    implicit_unit_defs: vec![RegisterUnitId(1)],
                                    implicit_unit_clobbers: vec![],
                                    memory:
                                        MachineEncodedMemoryEffect::ReadActivationStackV1 {
                                            stack_pointer:
                                                omega_register_model::RegisterViewId(12),
                                            byte_count: 8,
                                        },
                                    stack: MachineEncodedStackEffect::PopBytesV1 {
                                        stack_pointer: omega_register_model::RegisterViewId(12),
                                        byte_count: 8,
                                    },
                                    trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                                    control: MachineEncodedControlEffect::ReturnFromActivationStackV1,
                                },
                            },
                        ],
                    }],
                }],
            }],
            structural_unit_functions: Vec::new(),
        };
        let return_instruction = InstructionMachineEffects {
            instruction: SelectedInstructionId(1),
            kind: SelectedInstructionKind::ReturnUnit,
            constraint: RegisterConstraintKey {
                family: RegisterConstraintFamily::Return,
                variant: 3,
            },
            unit_uses: vec![RegisterUnitId(4)],
            unit_defs: vec![RegisterUnitId(4), RegisterUnitId(5)],
            unit_clobbers: Vec::new(),
            memory: MachineMemoryEffect::NoneV1,
            trap: MachineTrapBehavior::NeverV1,
            barrier: MachineBarrier::ControlFlow,
            call: MachineCallEffect::NoneV1,
            cleanup: MachineCleanupEffect::NoneV1,
            provenance: SelectedInstructionProvenance::default(),
            alternatives: Vec::new(),
        };
        let call_constraint = RegisterConstraintKey {
            family: RegisterConstraintFamily::Call,
            variant: 2,
        };
        plan.structural_unit_functions
            .push(StructuralUnitFunctionMachineEffects {
            machine: MachineId::new(6).unwrap(),
            block: SelectedBlockId(0),
            call: Some(StructuralUnitCallMachineEffects {
                instruction: SelectedInstructionId(0),
                operation: OperationId::new(7).unwrap(),
                callee: MachineId::new(8).unwrap(),
                constraint: call_constraint,
                unit_uses: vec![RegisterUnitId(1), RegisterUnitId(2)],
                unit_defs: vec![RegisterUnitId(3)],
                unit_clobbers: vec![RegisterUnitId(4)],
                layout: SelectedMicrosoftX64OwnedIndirectPairLayout {
                    shadow_byte_count: 32,
                    outgoing_frame_byte_count: 72,
                    pre_call_stack_alignment: 16,
                    bindings: [
                        SelectedStructuralUnitIndirectBinding {
                            parameter_index: 0,
                            pointer: omega_target_operations::MachineRegister::X86Rcx,
                            copy_stack_byte_offset: 32,
                            byte_count: 16,
                            alignment: 8,
                        },
                        SelectedStructuralUnitIndirectBinding {
                            parameter_index: 1,
                            pointer: omega_target_operations::MachineRegister::X86Rdx,
                            copy_stack_byte_offset: 48,
                            byte_count: 16,
                            alignment: 8,
                        },
                    ],
                },
                effect: EffectLink {
                    input: 9,
                    output: 10,
                },
                ownership: vec![
                    OwnershipEvent::ClaimTransfer(vec![ClaimId::new(11).unwrap()]),
                    OwnershipEvent::Cleanup(Vec::new()),
                ],
                claim_transfers: vec![psi_terminal::ClaimTransfer {
                    claim: ClaimId::new(11).unwrap(),
                    argument_index: 0,
                }],
                provenance: SelectedInstructionProvenance {
                    operations: vec![OperationId::new(7).unwrap()],
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
                    trap: MachineTrapBehavior::MayArchitecturalFaultV1,
                    barrier: StructuralUnitCallBarrier::CallV1,
                    call: StructuralUnitCallEffect::DirectInternalUnitV1,
                    cleanup: MachineCleanupEffect::NoneV1,
                },
            }),
            return_instruction,
            return_effect: EffectLink {
                input: 10,
                output: 11,
            },
            return_ownership: vec![OwnershipEvent::StructuralReturn(vec![
                ClaimId::new(12).unwrap(),
            ])],
        });
        plan.identity = pre_allocation_machine_effect_identity(&plan);
        plan
    }

    #[test]
    fn codec_round_trips_complete_effect_content() {
        let source = plan();
        let encoded = source.encode();

        assert_eq!(
            PreAllocationMachineEffectPlan::decode(&encoded).unwrap(),
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
            PreAllocationMachineEffectPlan::decode(&wrong_magic),
            Err(PreAllocationMachineEffectDecodeError::WrongMagic)
        );

        let mut unsupported_version = encoded.clone();
        unsupported_version[8..12].copy_from_slice(&2_u32.to_le_bytes());
        assert_eq!(
            PreAllocationMachineEffectPlan::decode(&unsupported_version),
            Err(PreAllocationMachineEffectDecodeError::UnsupportedVersion(2))
        );

        let mut stale_identity = encoded.clone();
        stale_identity[12] ^= 1;
        assert_eq!(
            PreAllocationMachineEffectPlan::decode(&stale_identity),
            Err(PreAllocationMachineEffectDecodeError::InvalidIdentity)
        );

        let mut invalid_target = encoded.clone();
        invalid_target[112] = u8::MAX;
        assert_eq!(
            PreAllocationMachineEffectPlan::decode(&invalid_target),
            Err(PreAllocationMachineEffectDecodeError::InvalidField)
        );

        assert_eq!(
            PreAllocationMachineEffectPlan::decode(&encoded[..encoded.len() - 1]),
            Err(PreAllocationMachineEffectDecodeError::Truncated)
        );

        let mut trailing = encoded;
        trailing.push(0);
        assert_eq!(
            PreAllocationMachineEffectPlan::decode(&trailing),
            Err(PreAllocationMachineEffectDecodeError::TrailingBytes)
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
            pre_allocation_machine_effect_identity(&substituted),
            source.identity
        );
        assert_eq!(
            PreAllocationMachineEffectPlan::decode(&substituted.encode()),
            Err(PreAllocationMachineEffectDecodeError::InvalidIdentity)
        );

        let mut invalid_declaration_tag = source.encode();
        let declaration_tag = invalid_declaration_tag.len() - 58;
        invalid_declaration_tag[declaration_tag] = u8::MAX;
        assert!(matches!(
            PreAllocationMachineEffectPlan::decode(&invalid_declaration_tag),
            Err(PreAllocationMachineEffectDecodeError::InvalidField)
                | Err(PreAllocationMachineEffectDecodeError::InvalidIdentity)
        ));
    }
}
