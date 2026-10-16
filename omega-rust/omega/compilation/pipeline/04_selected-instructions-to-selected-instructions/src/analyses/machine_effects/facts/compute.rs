use crate::ValidatedSelectedAnalysis;
use register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile, target_register_environment_identity,
};
use selected_instructions::{
    MachineAlternative, MachineEffectDeclaration, MachineEncodedControlEffect,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
    MachineSemanticKind, SelectedConstraintKeys, SelectedFunction, SelectedInstruction,
    SelectedInstructionKind, ValidatedMachineEffectCatalog,
};

use crate::MachineEffectError;
use selected_instructions::{
    BlockMachineEffects, FunctionMachineEffects, InstructionMachineEffects,
    PreAllocationMachineEffectPlan, pre_allocation_machine_effect_identity,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn compute_terminal_pre_allocation_machine_effects<S: ValidatedSelectedAnalysis>(
    selected: &S,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
    catalog: &ValidatedMachineEffectCatalog,
) -> Result<PreAllocationMachineEffectPlan, MachineEffectError> {
    let source = selected.selected_plan();
    if source.fuel_schedule != selected.fuel_schedule_identity() {
        return Err(MachineEffectError::SelectedRootMismatch);
    }
    if target_register_environment_identity(
        source.target,
        physical,
        constraints,
        reservations,
        selected_keys,
    ) != register_environment
    {
        return Err(MachineEffectError::RegisterEnvironmentMismatch);
    }
    validate_catalog_roots(source.target, constraints, selected_keys, catalog)?;
    let mut functions = Vec::with_capacity(source.functions.len());
    for function in &source.functions {
        let mut blocks = Vec::with_capacity(function.blocks.len());
        for block in &function.blocks {
            let mut instructions = Vec::with_capacity(block.instructions.len() + 1);
            for instruction in &block.instructions {
                instructions.push(compute_instruction(
                    function,
                    instruction,
                    physical,
                    constraints,
                    catalog,
                )?);
            }
            let terminator = selected_instructions::terminator_instruction(&block.terminator);
            instructions.push(compute_instruction(
                function,
                terminator,
                physical,
                constraints,
                catalog,
            )?);
            blocks.push(BlockMachineEffects {
                block: block.id,
                instructions,
            });
        }
        functions.push(FunctionMachineEffects {
            machine: function.machine,
            blocks,
        });
    }
    let mut plan = PreAllocationMachineEffectPlan {
        identity: selected_instructions::PreAllocationMachineEffectIdentity::from_bytes([0; 32]),
        selected: selected.selected_identity(),
        optimization_unit: selected.optimization_unit_identity(),
        fuel_schedule: selected.fuel_schedule_identity(),
        target: source.target,
        register_environment,
        register_constraints: constraints.identity(),
        machine_effect_catalog: catalog.identity(),
        functions,
    };
    plan.identity = pre_allocation_machine_effect_identity(&plan);
    Ok(plan)
}

fn validate_catalog_roots(
    target: target::NativeTarget,
    constraints: &ValidatedRegisterConstraintCatalog,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
    catalog: &ValidatedMachineEffectCatalog,
) -> Result<(), MachineEffectError> {
    if catalog.catalog().target != target {
        return Err(MachineEffectError::CatalogTargetMismatch);
    }
    if catalog.catalog().register_constraints != constraints.identity() {
        return Err(MachineEffectError::CatalogConstraintMismatch);
    }
    if catalog.catalog().selected_keys != terminal_selected_keys(selected_keys) {
        return Err(MachineEffectError::CatalogSelectedKeysMismatch);
    }
    Ok(())
}

fn terminal_selected_keys(
    keys: &TargetRegisterEnvironmentConstraintKeys,
) -> SelectedConstraintKeys {
    SelectedConstraintKeys {
        port_write: keys.port_write,
        system_register_read: keys.system_register_read,
        system_register_write: keys.system_register_write,
        port_register_read: keys.port_register_read,
        port_register_write: keys.port_register_write,
        model_specific_read: keys.model_specific_read,
        model_specific_write: keys.model_specific_write,
        atomic_fence: keys.atomic_fence,
        placed_read: keys.placed_read,
        placed_write: keys.placed_write,
        atomic_load: keys.atomic_load,
        atomic_store: keys.atomic_store,
        atomic_update: keys.atomic_update,
        atomic_bitwise_update: keys.atomic_bitwise_update,
        atomic_compare_exchange: keys.atomic_compare_exchange,
        crash: keys.crash,
        hosted_write_byte_i32: keys.hosted_write_byte_i32,
        save_floating_control: keys.save_floating_control,
        restore_floating_control: keys.restore_floating_control,
        hosted_read_byte: keys.hosted_read_byte,
        hosted_exit_process_i32: keys.hosted_exit_process_i32,
        load64: keys.load64,
        load_packed: keys.load_packed,
        store_packed: keys.store_packed,
        load8: keys.load8,
        load16: keys.load16,
        load32: keys.load32,
        load8_indexed: keys.load8_indexed,
        copy_bytes: keys.copy_bytes,
        zero_bytes: keys.zero_bytes,
        scan_byte_sequence: keys.scan_byte_sequence,
        load_encoded_scalar: keys.load_encoded_scalar,
        store_encoded_scalar: keys.store_encoded_scalar,
        compare_byte_sequences: keys.compare_byte_sequences,
        store: keys.store,
        address_offset: keys.address_offset,
        store64: keys.store64,
        frame_address: keys.frame_address,
        byte_literal_image_address: keys.byte_literal_image_address,
        call_unit: keys.call_unit.clone(),
        call_unit_mixed: keys.call_unit_mixed.clone(),
        call_scalar: keys.call_scalar.clone(),
        call_aggregate: keys.call_aggregate.clone(),
        call_normalized_foreign: keys.call_normalized_foreign.clone(),
        return_aggregate: keys.return_aggregate.clone(),
        materialize_i64: keys.materialize_i64,
        materialize_boolean: keys.materialize_boolean,
        copy_i64: keys.copy_i64,
        float32_to_bits: keys.float32_to_bits,
        float64_to_bits: keys.float64_to_bits,
        bits_to_float32: keys.bits_to_float32,
        bits_to_float64: keys.bits_to_float64,
        ieee_float_arithmetic: keys.ieee_float_arithmetic,
        ieee_float_directed_binary: keys.ieee_float_directed_binary,
        ieee_float_unary: keys.ieee_float_unary,
        ieee_float_conversion_control: keys.ieee_float_conversion_control,
        ieee_float_ternary: keys.ieee_float_ternary,
        add_i64: keys.add_i64,
        subtract_i64: keys.subtract_i64,
        multiply_i64: keys.multiply_i64,
        saturating_subtract_unsigned: keys.saturating_subtract_unsigned,
        saturating_add_u64: keys.saturating_add_u64,
        divide_u64: keys.divide_u64,
        remainder_u64: keys.remainder_u64,
        remainder_i64: keys.remainder_i64,
        divide_i64: keys.divide_i64,
        saturating_add_clamped: keys.saturating_add_clamped,
        saturating_subtract_clamped: keys.saturating_subtract_clamped,
        saturating_divide_signed: keys.saturating_divide_signed,
        saturating_multiply_clamped: keys.saturating_multiply_clamped,
        saturating_multiply_u64: keys.saturating_multiply_u64,
        trapping_binary: keys.trapping_binary,
        trapping_fixed_pair: keys.trapping_fixed_pair,
        trapping_shift: keys.trapping_shift,
        trapping_convert: keys.trapping_convert,
        float_result_trapping: keys.float_result_trapping,
        float_result_saturating: keys.float_result_saturating,
        shift_i64: keys.shift_i64,
        add_i64_immediate: keys.add_i64_immediate,
        subtract_i64_immediate: keys.subtract_i64_immediate,
        compare_i64_zero: keys.compare_i64_zero,
        compare_i64: keys.compare_i64,
        compare_i64_immediate: keys.compare_i64_immediate,
        conditional_branch: keys.conditional_branch,
        jump: keys.jump,
        return_float: keys.return_float.clone(),
        return_i64: keys.return_i64,
        return_unit: keys.return_unit,
    }
}

fn compute_instruction(
    function: &SelectedFunction,
    instruction: &SelectedInstruction,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    catalog: &ValidatedMachineEffectCatalog,
) -> Result<InstructionMachineEffects, MachineEffectError> {
    if matches!(instruction.kind, SelectedInstructionKind::Store { byte_size, .. } if !matches!(byte_size, 1 | 2 | 4 | 8))
    {
        return Err(MachineEffectError::ConstraintEffectMismatch {
            instruction: instruction.id,
        });
    }
    // The produced sidecar copies the retained provenance verbatim; refuse to
    // carry fuel that settles sites the instruction never claimed.
    if !instruction.provenance.fuel_settles_only_claimed_sites() {
        return Err(MachineEffectError::FuelProvenanceMismatch {
            instruction: instruction.id,
        });
    }
    let constraint = super::foreign_call::constraint(function, instruction, physical, constraints)?;
    if instruction.operands.len() != constraint.operands.len() {
        return Err(MachineEffectError::ConstraintOperandMismatch {
            instruction: instruction.id,
        });
    }
    for (operand, expected) in instruction.operands.iter().zip(&constraint.operands) {
        let Some(register) = function
            .virtual_registers
            .get(operand.virtual_register.0 as usize)
        else {
            return Err(MachineEffectError::ConstraintOperandMismatch {
                instruction: instruction.id,
            });
        };
        if operand.operand != expected.operand
            || operand.access != expected.access
            || operand.class != expected.class
            || operand.fixed_view != expected.fixed_view
            || operand.tied_to != expected.tied_to
            || operand.early_clobber != expected.early_clobber
            || register.id != operand.virtual_register
            || register.class != expected.class
        {
            return Err(MachineEffectError::ConstraintOperandMismatch {
                instruction: instruction.id,
            });
        }
    }
    if instruction.implicit_uses != constraint.implicit_uses
        || instruction.implicit_defs != constraint.implicit_defs
        || instruction.clobbers != constraint.clobbers
    {
        return Err(MachineEffectError::ConstraintEffectMismatch {
            instruction: instruction.id,
        });
    }
    let declaration = super::foreign_call::declaration(
        function,
        instruction,
        physical,
        &constraint,
        exact_declaration(instruction, catalog)?,
    )?;
    Ok(InstructionMachineEffects {
        instruction: instruction.id,
        kind: instruction.kind,
        constraint: instruction.constraint,
        unit_uses: instruction.implicit_uses.clone(),
        unit_defs: instruction.implicit_defs.clone(),
        unit_clobbers: instruction.clobbers.clone(),
        memory: declaration.memory,
        trap: declaration.trap,
        barrier: declaration.barrier,
        call: declaration.call,
        cleanup: declaration.cleanup,
        provenance: instruction.provenance.clone(),
        alternatives: declaration.alternatives.clone(),
    })
}

fn exact_declaration<'a>(
    instruction: &SelectedInstruction,
    catalog: &'a ValidatedMachineEffectCatalog,
) -> Result<&'a MachineEffectDeclaration, MachineEffectError> {
    let semantic = machine_semantic_kind(instruction.kind);
    let mut matches = catalog
        .catalog()
        .declarations
        .iter()
        .filter(|row| row.semantic == semantic && row.constraint == instruction.constraint);
    let Some(declaration) = matches.next() else {
        return Err(MachineEffectError::MissingDeclaration {
            instruction: instruction.id,
        });
    };
    if matches.next().is_some() {
        return Err(MachineEffectError::AmbiguousDeclaration {
            instruction: instruction.id,
        });
    }
    Ok(declaration)
}

pub(crate) fn machine_semantic_kind(kind: SelectedInstructionKind) -> MachineSemanticKind {
    match kind {
        SelectedInstructionKind::RegisterAccess { operation } => match operation {
            selected_instructions::RegisterOperation::SystemRead(_) => {
                MachineSemanticKind::SystemRegisterRead
            }
            selected_instructions::RegisterOperation::SystemWrite(_) => {
                MachineSemanticKind::SystemRegisterWrite
            }
            selected_instructions::RegisterOperation::PortRead => {
                MachineSemanticKind::PortRegisterRead
            }
            selected_instructions::RegisterOperation::PortWrite => {
                MachineSemanticKind::PortRegisterWrite
            }
            selected_instructions::RegisterOperation::ModelSpecificRead => {
                MachineSemanticKind::ModelSpecificRead
            }
            selected_instructions::RegisterOperation::ModelSpecificWrite => {
                MachineSemanticKind::ModelSpecificWrite
            }
        },
        SelectedInstructionKind::PlacedScalarAccess { form, .. } => {
            MachineSemanticKind::PlacedScalarAccess(form)
        }
        SelectedInstructionKind::PortWrite { .. } => MachineSemanticKind::PortWrite,
        SelectedInstructionKind::AtomicFence => MachineSemanticKind::AtomicFence,
        SelectedInstructionKind::AtomicAccess { form } => MachineSemanticKind::AtomicAccess(form),
        SelectedInstructionKind::NearestIeeeFloatArithmetic { form } => {
            MachineSemanticKind::NearestIeeeFloatArithmetic(form)
        }
        SelectedInstructionKind::CompareI64Zero => MachineSemanticKind::CompareI64Zero,
        SelectedInstructionKind::CallAggregate { .. } => MachineSemanticKind::CallAggregate,
        SelectedInstructionKind::ReturnAggregate { .. } => MachineSemanticKind::ReturnAggregate,
        SelectedInstructionKind::CompareByteSequences => MachineSemanticKind::CompareByteSequences,
        SelectedInstructionKind::CompareI64 => MachineSemanticKind::CompareI64,
        SelectedInstructionKind::CompareI64Immediate { .. } => {
            MachineSemanticKind::CompareI64Immediate
        }
        SelectedInstructionKind::MaterializeI64 { .. } => MachineSemanticKind::MaterializeI64,
        SelectedInstructionKind::CopyI64 => MachineSemanticKind::CopyI64,
        SelectedInstructionKind::Float32ToBits => MachineSemanticKind::Float32ToBits,
        SelectedInstructionKind::Float64ToBits => MachineSemanticKind::Float64ToBits,
        SelectedInstructionKind::BitsToFloat32 => MachineSemanticKind::BitsToFloat32,
        SelectedInstructionKind::BitsToFloat64 => MachineSemanticKind::BitsToFloat64,
        SelectedInstructionKind::ZeroExtendU8 => MachineSemanticKind::ZeroExtendU8,
        SelectedInstructionKind::ZeroExtendU32 => MachineSemanticKind::ZeroExtendU32,
        SelectedInstructionKind::ZeroExtendU16 => MachineSemanticKind::ZeroExtendU16,
        SelectedInstructionKind::SignExtendI8 => MachineSemanticKind::SignExtendI8,
        SelectedInstructionKind::SignExtendI16 => MachineSemanticKind::SignExtendI16,
        SelectedInstructionKind::SignExtendI32 => MachineSemanticKind::SignExtendI32,
        SelectedInstructionKind::MaterializeBooleanEqual => {
            MachineSemanticKind::MaterializeBooleanEqual
        }
        SelectedInstructionKind::MaterializeBooleanU64LessThan => {
            MachineSemanticKind::MaterializeBooleanU64LessThan
        }
        SelectedInstructionKind::MaterializeBooleanI64LessThan => {
            MachineSemanticKind::MaterializeBooleanI64LessThan
        }
        SelectedInstructionKind::MaterializeBooleanU64LessOrEqual => {
            MachineSemanticKind::MaterializeBooleanU64LessOrEqual
        }
        SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => {
            MachineSemanticKind::MaterializeBooleanI64LessOrEqual
        }

        SelectedInstructionKind::SaveFloatingControl { .. } => {
            MachineSemanticKind::SaveFloatingControl
        }
        SelectedInstructionKind::RestoreFloatingControl { .. } => {
            MachineSemanticKind::RestoreFloatingControl
        }
        SelectedInstructionKind::HostedWriteByteI32 { .. } => {
            MachineSemanticKind::HostedWriteByteI32
        }
        SelectedInstructionKind::HostedReadByte { .. } => MachineSemanticKind::HostedReadByte,
        SelectedInstructionKind::ByteViewAddress => MachineSemanticKind::ByteViewAddress,
        SelectedInstructionKind::HostedExitProcessI32 => MachineSemanticKind::HostedExitProcessI32,
        SelectedInstructionKind::ExactAddI64 { .. } => MachineSemanticKind::ExactAddI64,
        SelectedInstructionKind::WrappingAddI64 => MachineSemanticKind::WrappingAddI64,
        SelectedInstructionKind::BitwiseAndI64 => MachineSemanticKind::BitwiseAndI64,
        SelectedInstructionKind::BitwiseXorI64 => MachineSemanticKind::BitwiseXorI64,
        SelectedInstructionKind::ExactDivideU64 { .. } => MachineSemanticKind::ExactDivideU64,
        SelectedInstructionKind::WrappingRemainderI64 { .. } => {
            MachineSemanticKind::WrappingRemainderI64
        }
        SelectedInstructionKind::SaturatingAdd { carrier } => {
            MachineSemanticKind::SaturatingAdd(carrier)
        }
        SelectedInstructionKind::SaturatingSubtract { carrier } => {
            MachineSemanticKind::SaturatingSubtract(carrier)
        }
        SelectedInstructionKind::SaturatingDivide { carrier, .. } => {
            MachineSemanticKind::SaturatingDivide(carrier)
        }
        SelectedInstructionKind::SaturatingRemainder { carrier, .. } => {
            MachineSemanticKind::SaturatingRemainder(carrier)
        }
        SelectedInstructionKind::SaturatingMultiply { carrier } => {
            MachineSemanticKind::SaturatingMultiply(carrier)
        }
        SelectedInstructionKind::TrappingInteger { form } => {
            MachineSemanticKind::TrappingInteger(form)
        }
        SelectedInstructionKind::FloatResultPolicy { form } => {
            MachineSemanticKind::FloatResultPolicy(form)
        }
        SelectedInstructionKind::ExactAddI64Immediate { .. } => {
            MachineSemanticKind::ExactAddI64Immediate
        }
        SelectedInstructionKind::ExactSubtractI64 { .. } => MachineSemanticKind::ExactSubtractI64,
        SelectedInstructionKind::ExactSubtractI64Immediate { .. } => {
            MachineSemanticKind::ExactSubtractI64Immediate
        }
        SelectedInstructionKind::ExactMultiplyI64 { .. } => MachineSemanticKind::ExactMultiplyI64,
        SelectedInstructionKind::ExactRemainderU64 { .. } => MachineSemanticKind::ExactRemainderU64,
        SelectedInstructionKind::WrappingSubtractI64 => MachineSemanticKind::WrappingSubtractI64,
        SelectedInstructionKind::WrappingMultiplyI64 => MachineSemanticKind::WrappingMultiplyI64,
        SelectedInstructionKind::WrappingDivideI64 { .. } => MachineSemanticKind::WrappingDivideI64,
        SelectedInstructionKind::ExactDivideI64 { .. } => MachineSemanticKind::ExactDivideI64,
        SelectedInstructionKind::ExactRemainderI64 { .. } => MachineSemanticKind::ExactRemainderI64,
        SelectedInstructionKind::WrappingShiftLeftI64 => MachineSemanticKind::WrappingShiftLeftI64,
        SelectedInstructionKind::WrappingShiftRightI64 => {
            MachineSemanticKind::WrappingShiftRightI64
        }
        SelectedInstructionKind::WrappingShiftRightU64 => {
            MachineSemanticKind::WrappingShiftRightU64
        }
        SelectedInstructionKind::ExactShiftLeftI64 { .. } => MachineSemanticKind::ExactShiftLeftI64,
        SelectedInstructionKind::ExactShiftRightI64 { .. } => {
            MachineSemanticKind::ExactShiftRightI64
        }
        SelectedInstructionKind::ExactShiftRightU64 { .. } => {
            MachineSemanticKind::ExactShiftRightU64
        }
        SelectedInstructionKind::BitwiseOrI64 => MachineSemanticKind::BitwiseOrI64,
        SelectedInstructionKind::BitwiseNotI64 => MachineSemanticKind::BitwiseNotI64,
        SelectedInstructionKind::ConditionalBranchNonZero => {
            MachineSemanticKind::ConditionalBranchNonZero
        }
        SelectedInstructionKind::ConditionalBranchU64LessThan => {
            MachineSemanticKind::ConditionalBranchU64LessThan
        }
        SelectedInstructionKind::ConditionalBranchI64LessThan => {
            MachineSemanticKind::ConditionalBranchI64LessThan
        }
        SelectedInstructionKind::Jump => MachineSemanticKind::Jump,
        SelectedInstructionKind::ReturnScalar => MachineSemanticKind::ReturnScalar,
        SelectedInstructionKind::Crash => MachineSemanticKind::Crash,
        SelectedInstructionKind::ReturnUnit => MachineSemanticKind::ReturnUnit,
        SelectedInstructionKind::CallScalar { .. } => MachineSemanticKind::CallScalar,
        SelectedInstructionKind::Load64 { .. } => MachineSemanticKind::Load64,
        SelectedInstructionKind::LoadPacked { width, .. } => match width {
            selected_instructions::PackedByteWidth::Three => MachineSemanticKind::LoadPacked3,
            selected_instructions::PackedByteWidth::Five => MachineSemanticKind::LoadPacked5,
            selected_instructions::PackedByteWidth::Six => MachineSemanticKind::LoadPacked6,
            selected_instructions::PackedByteWidth::Seven => MachineSemanticKind::LoadPacked7,
        },
        SelectedInstructionKind::StorePacked { .. } => MachineSemanticKind::StorePacked,
        SelectedInstructionKind::Load8 { .. } => MachineSemanticKind::Load8,
        SelectedInstructionKind::Load16 { .. } => MachineSemanticKind::Load16,
        SelectedInstructionKind::Load32 { .. } => MachineSemanticKind::Load32,
        SelectedInstructionKind::Load8Indexed => MachineSemanticKind::Load8Indexed,
        SelectedInstructionKind::CopyBytes => MachineSemanticKind::CopyBytes,
        SelectedInstructionKind::LoadEncodedScalar => MachineSemanticKind::LoadEncodedScalar,
        SelectedInstructionKind::StoreEncodedScalar => MachineSemanticKind::StoreEncodedScalar,
        SelectedInstructionKind::ZeroBytes => MachineSemanticKind::ZeroBytes,
        SelectedInstructionKind::ScanByteSequence { .. } => MachineSemanticKind::ScanByteSequence,
        SelectedInstructionKind::Store { .. } => MachineSemanticKind::Store,
        SelectedInstructionKind::AddressOffset { .. } => MachineSemanticKind::AddressOffset,
        SelectedInstructionKind::Store64 { .. } => MachineSemanticKind::Store64,
        SelectedInstructionKind::FrameAddress { .. } => MachineSemanticKind::FrameAddress,
        SelectedInstructionKind::ImmutableImageAddress { .. } => {
            MachineSemanticKind::ImmutableImageAddress
        }
        SelectedInstructionKind::CallUnit { .. } => MachineSemanticKind::CallUnit,
        SelectedInstructionKind::NormalizedForeignCall { .. } => {
            MachineSemanticKind::NormalizedForeignCall
        }
    }
}

/// A plain materializable alternative: no implicit effects beyond its operands.
pub(crate) fn machine_alternative_is_transparent(alternative: &MachineAlternative) -> bool {
    let encoded = &alternative.encoded;
    encoded.memory == MachineEncodedMemoryEffect::NoneV1
        && encoded.stack == MachineEncodedStackEffect::UnchangedV1
        && encoded.trap == MachineEncodedTrapBehavior::NeverV1
        && encoded.control == MachineEncodedControlEffect::FallThroughV1
        && encoded.implicit_unit_uses.is_empty()
        && encoded.implicit_unit_defs.is_empty()
        && encoded.implicit_unit_clobbers.is_empty()
}
