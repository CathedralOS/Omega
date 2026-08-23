use omega_calling_conventions::{IndirectPointerLocation, ValueLocation, ValuePlacement};
use omega_target::{Architecture, NativeTarget};
use omega_terminal_assigned_target_operations::{
    TerminalAssignedBooleanControl, TerminalAssignedBooleanExpression,
    TerminalAssignedCallArgument, TerminalAssignedCallDestination,
    TerminalAssignedConditionalBooleanArm, TerminalAssignedConditionalIntegerArm,
    TerminalAssignedIntegerControl, TerminalAssignedIntegerExpression,
    TerminalAssignedScalarExpression, TerminalAssignedScalarLocation, TerminalExpressionFrame,
};
use omega_terminal_machine_code::{
    TerminalAarch64ReturnLinkEvidence, TerminalBooleanStructuralFieldRead,
    TerminalInternalCallRelocation, TerminalScalarCallStackEvidence,
    TerminalScalarConditionalCondition,
};
use omega_terminal_target_operations::{MachineRegister, TerminalCallSiteOwner};
use psi_core::{IntegerSign, IntegerType, MachineId, ValueId};

use super::shared::{
    EmissionFragment, boolean_expression_source, emit_native_crash, expression_source,
    integer_bits, native_integer_bounds, outgoing_stack_bytes, require_native_integer_width,
    top_level_integer_conditional_evidence,
};
use crate::{
    EmissionError, aarch64_load_base, aarch64_unit_memory_access, aarch64_unit_register,
    aarch64_unit_stack_access, stack_adjustment_pair,
};

pub(crate) fn emit_aarch64_conditional_integer_control(
    condition_source: ValueId,
    condition_location: TerminalAssignedScalarLocation,
    scalar_type: IntegerType,
    when_true: &TerminalAssignedConditionalIntegerArm,
    when_false: &TerminalAssignedConditionalIntegerArm,
    target: NativeTarget,
) -> Result<EmissionFragment, EmissionError> {
    let (mut bytes, condition_register) =
        emit_aarch64_condition_load(condition_source, condition_location)?;
    let true_fragment = emit_aarch64_integer_control(scalar_type, &when_true.control, target)?;
    let false_fragment = emit_aarch64_integer_control(scalar_type, &when_false.control, target)?;
    let branch_words = true_fragment
        .bytes
        .len()
        .checked_div(4)
        .and_then(|words| words.checked_add(1))
        .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
    if branch_words > 0x3ffff {
        return Err(EmissionError::ConditionalBranchDistanceNotEncodable);
    }
    let cbz = 0x3400_0000_u32 | ((branch_words as u32) << 5) | u32::from(condition_register);
    let branch_offset = bytes.len();
    bytes.extend_from_slice(&cbz.to_le_bytes());
    let false_arm_offset = bytes
        .len()
        .checked_add(true_fragment.bytes.len())
        .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
    let mut fragment = EmissionFragment::without_calls(bytes);
    fragment.conditional = Some(top_level_integer_conditional_evidence(
        TerminalScalarConditionalCondition::Parameter,
        branch_offset,
        4,
        false_arm_offset,
        true_fragment.conditional.as_deref(),
        false_fragment.conditional.as_deref(),
    )?);
    fragment.append(true_fragment)?;
    fragment.append(false_fragment)?;
    Ok(fragment)
}

fn emit_aarch64_integer_control(
    scalar_type: IntegerType,
    control: &TerminalAssignedIntegerControl,
    target: NativeTarget,
) -> Result<EmissionFragment, EmissionError> {
    match control {
        TerminalAssignedIntegerControl::Crash { .. } => Ok(EmissionFragment::without_calls(
            emit_native_crash(Architecture::Aarch64),
        )),
        TerminalAssignedIntegerControl::Return {
            source_value,
            frame,
            expression,
            ..
        } => {
            require_native_integer_width(*source_value, scalar_type)?;
            let mut internal_calls = Vec::new();
            let bytes = emit_aarch64_integer_expression(
                scalar_type,
                frame,
                expression,
                Some((&mut internal_calls, target)),
            )?;
            Ok(EmissionFragment {
                bytes,
                internal_calls,
                conditional: None,
            })
        }
        TerminalAssignedIntegerControl::Conditional {
            condition_source,
            condition_location,
            when_true,
            when_false,
            ..
        } => emit_aarch64_conditional_integer_control(
            *condition_source,
            *condition_location,
            scalar_type,
            when_true,
            when_false,
            target,
        ),
        TerminalAssignedIntegerControl::ConditionalExpression {
            condition_frame,
            condition,
            when_true,
            when_false,
            ..
        } => emit_aarch64_conditional_integer_expression_control(
            condition_frame,
            condition,
            scalar_type,
            when_true,
            when_false,
            target,
        ),
    }
}

pub(crate) fn emit_aarch64_conditional_integer_expression_control(
    condition_frame: &TerminalExpressionFrame,
    condition: &TerminalAssignedBooleanExpression,
    scalar_type: IntegerType,
    when_true: &TerminalAssignedConditionalIntegerArm,
    when_false: &TerminalAssignedConditionalIntegerArm,
    target: NativeTarget,
) -> Result<EmissionFragment, EmissionError> {
    let mut internal_calls = Vec::new();
    let mut bytes = emit_aarch64_boolean_condition_value(
        condition_frame,
        condition,
        Some((&mut internal_calls, target)),
        None,
    )?;
    let true_fragment = emit_aarch64_integer_control(scalar_type, &when_true.control, target)?;
    let false_fragment = emit_aarch64_integer_control(scalar_type, &when_false.control, target)?;
    let branch_words = true_fragment
        .bytes
        .len()
        .checked_div(4)
        .and_then(|words| words.checked_add(1))
        .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
    if branch_words > 0x3ffff {
        return Err(EmissionError::ConditionalBranchDistanceNotEncodable);
    }
    let branch_equal = 0x5400_0000_u32 | ((branch_words as u32) << 5); // b.eq false
    let branch_offset = bytes.len();
    bytes.extend_from_slice(&branch_equal.to_le_bytes());
    let false_arm_offset = bytes
        .len()
        .checked_add(true_fragment.bytes.len())
        .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
    let conditional = top_level_integer_conditional_evidence(
        TerminalScalarConditionalCondition::Expression,
        branch_offset,
        4,
        false_arm_offset,
        true_fragment.conditional.as_deref(),
        false_fragment.conditional.as_deref(),
    )?;
    let mut fragment = EmissionFragment {
        bytes,
        internal_calls,
        conditional: Some(conditional),
    };
    fragment.append(true_fragment)?;
    fragment.append(false_fragment)?;
    Ok(fragment)
}

pub(crate) fn emit_aarch64_conditional_boolean_control(
    condition_source: ValueId,
    condition_location: TerminalAssignedScalarLocation,
    when_true: &TerminalAssignedConditionalBooleanArm,
    when_false: &TerminalAssignedConditionalBooleanArm,
    target: NativeTarget,
) -> Result<EmissionFragment, EmissionError> {
    let (mut bytes, condition_register) =
        emit_aarch64_condition_load(condition_source, condition_location)?;
    let true_fragment = emit_aarch64_boolean_control(&when_true.control, target)?;
    let false_fragment = emit_aarch64_boolean_control(&when_false.control, target)?;
    let branch_words = true_fragment
        .bytes
        .len()
        .checked_div(4)
        .and_then(|words| words.checked_add(1))
        .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
    if branch_words > 0x3ffff {
        return Err(EmissionError::ConditionalBranchDistanceNotEncodable);
    }
    let cbz = 0x3400_0000_u32 | ((branch_words as u32) << 5) | u32::from(condition_register);
    let branch_offset = bytes.len();
    bytes.extend_from_slice(&cbz.to_le_bytes());
    let false_arm_offset = bytes
        .len()
        .checked_add(true_fragment.bytes.len())
        .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
    let mut fragment = EmissionFragment::without_calls(bytes);
    fragment.conditional = Some(top_level_integer_conditional_evidence(
        TerminalScalarConditionalCondition::Parameter,
        branch_offset,
        4,
        false_arm_offset,
        true_fragment.conditional.as_deref(),
        false_fragment.conditional.as_deref(),
    )?);
    fragment.append(true_fragment)?;
    fragment.append(false_fragment)?;
    Ok(fragment)
}

pub(crate) fn emit_aarch64_conditional_boolean_expression_control(
    condition_frame: &TerminalExpressionFrame,
    condition: &TerminalAssignedBooleanExpression,
    when_true: &TerminalAssignedConditionalBooleanArm,
    when_false: &TerminalAssignedConditionalBooleanArm,
    target: NativeTarget,
) -> Result<EmissionFragment, EmissionError> {
    let mut internal_calls = Vec::new();
    let mut bytes = emit_aarch64_boolean_condition_value(
        condition_frame,
        condition,
        Some((&mut internal_calls, target)),
        None,
    )?;
    let true_fragment = emit_aarch64_boolean_control(&when_true.control, target)?;
    let false_fragment = emit_aarch64_boolean_control(&when_false.control, target)?;
    let branch_words = true_fragment
        .bytes
        .len()
        .checked_div(4)
        .and_then(|words| words.checked_add(1))
        .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
    if branch_words > 0x3ffff {
        return Err(EmissionError::ConditionalBranchDistanceNotEncodable);
    }
    let branch_equal = 0x5400_0000_u32 | ((branch_words as u32) << 5); // b.eq false
    let branch_offset = bytes.len();
    bytes.extend_from_slice(&branch_equal.to_le_bytes());
    let false_arm_offset = bytes
        .len()
        .checked_add(true_fragment.bytes.len())
        .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
    let conditional = top_level_integer_conditional_evidence(
        TerminalScalarConditionalCondition::Expression,
        branch_offset,
        4,
        false_arm_offset,
        true_fragment.conditional.as_deref(),
        false_fragment.conditional.as_deref(),
    )?;
    let mut fragment = EmissionFragment {
        bytes,
        internal_calls,
        conditional: Some(conditional),
    };
    fragment.append(true_fragment)?;
    fragment.append(false_fragment)?;
    Ok(fragment)
}

pub(crate) fn emit_aarch64_boolean_control(
    control: &TerminalAssignedBooleanControl,
    target: NativeTarget,
) -> Result<EmissionFragment, EmissionError> {
    match control {
        TerminalAssignedBooleanControl::Crash { .. } => Ok(EmissionFragment::without_calls(
            emit_native_crash(Architecture::Aarch64),
        )),
        TerminalAssignedBooleanControl::ReturnImmediate { value, .. } => Ok(
            EmissionFragment::without_calls(emit_aarch64_boolean_return(*value)),
        ),
        TerminalAssignedBooleanControl::ReturnParameter {
            source_value,
            location,
            ..
        } => Ok(EmissionFragment::without_calls(
            emit_aarch64_parameter_return(*source_value, false, *location)?,
        )),
        TerminalAssignedBooleanControl::ReturnNotParameter {
            source_value,
            location,
            ..
        } => Ok(EmissionFragment::without_calls(
            emit_aarch64_boolean_not_parameter_return(*source_value, *location)?,
        )),
        TerminalAssignedBooleanControl::ReturnExpression {
            frame, expression, ..
        } => {
            let mut internal_calls = Vec::new();
            let bytes = emit_aarch64_boolean_expression(
                frame,
                expression,
                Some((&mut internal_calls, target)),
            )?;
            Ok(EmissionFragment {
                bytes,
                internal_calls,
                conditional: None,
            })
        }
        TerminalAssignedBooleanControl::Conditional {
            condition_source,
            condition_location,
            when_true,
            when_false,
            ..
        } => emit_aarch64_conditional_boolean_control(
            *condition_source,
            *condition_location,
            when_true,
            when_false,
            target,
        ),
        TerminalAssignedBooleanControl::ConditionalExpression {
            condition_frame,
            condition,
            when_true,
            when_false,
            ..
        } => emit_aarch64_conditional_boolean_expression_control(
            condition_frame,
            condition,
            when_true,
            when_false,
            target,
        ),
    }
}

pub(crate) fn emit_aarch64_boolean_not_parameter_return(
    source: ValueId,
    location: TerminalAssignedScalarLocation,
) -> Result<Vec<u8>, EmissionError> {
    let mut bytes = emit_aarch64_parameter_return(source, false, location)?;
    if bytes.len() < 4 || bytes[bytes.len() - 4..] != 0xd65f_03c0_u32.to_le_bytes() {
        return Err(EmissionError::BooleanNotEncodingInvalid);
    }
    bytes.truncate(bytes.len() - 4);
    bytes.extend_from_slice(&0x5200_0000_u32.to_le_bytes()); // eor w0, w0, #1
    bytes.extend_from_slice(&0xd65f_03c0_u32.to_le_bytes()); // ret
    Ok(bytes)
}

pub(crate) fn emit_aarch64_condition_load(
    source: ValueId,
    location: TerminalAssignedScalarLocation,
) -> Result<(Vec<u8>, u8), EmissionError> {
    match location {
        TerminalAssignedScalarLocation::Register(MachineRegister::Aarch64X(register))
            if register < 31 =>
        {
            Ok((Vec::new(), register))
        }
        TerminalAssignedScalarLocation::Register(register) => {
            Err(EmissionError::ParameterRegisterArchitectureMismatch {
                value: source,
                register,
                architecture: Architecture::Aarch64,
            })
        }
        TerminalAssignedScalarLocation::IncomingStack { byte_offset } => {
            if byte_offset > 0xfff {
                return Err(EmissionError::IncomingStackOffsetNotEncodable {
                    value: source,
                    byte_offset,
                });
            }
            let register = 16_u8;
            let ldrb = 0x3940_0000_u32 | (byte_offset << 10) | (31 << 5) | u32::from(register);
            Ok((ldrb.to_le_bytes().to_vec(), register))
        }
        TerminalAssignedScalarLocation::FrameSpill { .. } => {
            Err(EmissionError::AssignedFrameSpillOutsideExpression(source))
        }
    }
}

pub(crate) fn emit_aarch64_boolean_return(value: bool) -> Vec<u8> {
    let mov_w0 = 0x5280_0000_u32 | (u32::from(value) << 5);
    [mov_w0, 0xd65f_03c0]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect()
}

pub(crate) fn emit_aarch64_parameter_return(
    source: ValueId,
    is_64: bool,
    location: TerminalAssignedScalarLocation,
) -> Result<Vec<u8>, EmissionError> {
    let instruction = match location {
        TerminalAssignedScalarLocation::Register(MachineRegister::Aarch64X(register))
            if register < 31 =>
        {
            if register == 0 {
                None
            } else {
                let base = if is_64 { 0xaa00_03e0 } else { 0x2a00_03e0 };
                Some(base | (u32::from(register) << 16))
            }
        }
        TerminalAssignedScalarLocation::Register(register) => {
            return Err(EmissionError::ParameterRegisterArchitectureMismatch {
                value: source,
                register,
                architecture: Architecture::Aarch64,
            });
        }
        TerminalAssignedScalarLocation::IncomingStack { byte_offset } => {
            let scale = if is_64 { 8 } else { 4 };
            if byte_offset % scale != 0 || byte_offset / scale > 0xfff {
                return Err(EmissionError::IncomingStackOffsetNotEncodable {
                    value: source,
                    byte_offset,
                });
            }
            let base = if is_64 { 0xf940_0000 } else { 0xb940_0000 };
            Some(base | ((byte_offset / scale) << 10) | (31 << 5))
        }
        TerminalAssignedScalarLocation::FrameSpill { .. } => {
            return Err(EmissionError::AssignedFrameSpillOutsideExpression(source));
        }
    };
    Ok(instruction
        .into_iter()
        .chain([0xd65f_03c0])
        .flat_map(u32::to_le_bytes)
        .collect())
}

pub(crate) fn emit_aarch64_return(scalar_type: IntegerType, bits: u64) -> Vec<u8> {
    let is_64 = scalar_type.bits() > 32;
    let chunk_count = if is_64 { 4 } else { 2 };
    let movz_base = if is_64 { 0xd280_0000 } else { 0x5280_0000 };
    let movk_base = if is_64 { 0xf280_0000 } else { 0x7280_0000 };
    let mut instructions = Vec::new();
    for chunk in 0..chunk_count {
        let immediate = ((bits >> (chunk * 16)) & 0xffff) as u32;
        if chunk == 0 || immediate != 0 {
            let base = if chunk == 0 { movz_base } else { movk_base };
            instructions.push(base | ((chunk as u32) << 21) | (immediate << 5));
        }
    }
    instructions.push(0xd65f_03c0); // ret x30
    instructions
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect()
}

pub(crate) fn emit_aarch64_boolean_expression(
    frame: &TerminalExpressionFrame,
    expression: &TerminalAssignedBooleanExpression,
    internal_calls: Option<(&mut Vec<TerminalInternalCallRelocation>, NativeTarget)>,
) -> Result<Vec<u8>, EmissionError> {
    let mut bytes = emit_aarch64_boolean_expression_value(frame, expression, internal_calls)?;
    bytes.extend_from_slice(&0xd65f_03c0_u32.to_le_bytes()); // ret x30
    Ok(bytes)
}

fn emit_aarch64_boolean_expression_value(
    frame: &TerminalExpressionFrame,
    expression: &TerminalAssignedBooleanExpression,
    mut internal_calls: Option<(&mut Vec<TerminalInternalCallRelocation>, NativeTarget)>,
) -> Result<Vec<u8>, EmissionError> {
    if frame.byte_size == 0 && !frame.register_spills.is_empty() {
        return Err(EmissionError::AssignedFrameSizeMismatch);
    }
    let mut instructions = Vec::new();
    if frame.byte_size != 0 {
        emit_aarch64_adjust_sp(&mut instructions, frame.byte_size, false)?;
        for spill in &frame.register_spills {
            instructions.push(aarch64_stack_access(
                0xf900_0000,
                aarch64_spill_register(spill.source_value, spill.register)?,
                spill.source_value,
                spill.byte_offset,
            )?);
        }
    }
    let mut structural_reads = None;
    emit_aarch64_boolean_expression_node(
        &mut instructions,
        expression,
        frame,
        0,
        &mut internal_calls,
        &mut structural_reads,
    )?;
    if frame.byte_size != 0 {
        emit_aarch64_adjust_sp(&mut instructions, frame.byte_size, true)?;
    }
    Ok(instructions
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect())
}

pub(crate) fn emit_aarch64_boolean_condition_value(
    frame: &TerminalExpressionFrame,
    expression: &TerminalAssignedBooleanExpression,
    mut internal_calls: Option<(&mut Vec<TerminalInternalCallRelocation>, NativeTarget)>,
    structural_reads: Option<&mut Vec<TerminalBooleanStructuralFieldRead>>,
) -> Result<Vec<u8>, EmissionError> {
    if frame.byte_size == 0 && !frame.register_spills.is_empty() {
        return Err(EmissionError::AssignedFrameSizeMismatch);
    }
    let mut instructions = Vec::new();
    if frame.byte_size != 0 {
        emit_aarch64_adjust_sp(&mut instructions, frame.byte_size, false)?;
        for spill in &frame.register_spills {
            instructions.push(aarch64_stack_access(
                0xf900_0000,
                aarch64_spill_register(spill.source_value, spill.register)?,
                spill.source_value,
                spill.byte_offset,
            )?);
        }
    }
    let mut structural_reads = structural_reads;
    emit_aarch64_boolean_expression_node(
        &mut instructions,
        expression,
        frame,
        0,
        &mut internal_calls,
        &mut structural_reads,
    )?;
    instructions.push(0x7100_001f); // cmp w0, #0
    for spill in &frame.register_spills {
        instructions.push(aarch64_stack_access(
            0xf940_0000,
            aarch64_spill_register(spill.source_value, spill.register)?,
            spill.source_value,
            spill.byte_offset,
        )?);
    }
    if frame.byte_size != 0 {
        emit_aarch64_adjust_sp(&mut instructions, frame.byte_size, true)?;
    }
    Ok(instructions
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect())
}

fn emit_aarch64_boolean_expression_node(
    instructions: &mut Vec<u32>,
    expression: &TerminalAssignedBooleanExpression,
    frame: &TerminalExpressionFrame,
    stack_depth: u32,
    internal_calls: &mut Option<(&mut Vec<TerminalInternalCallRelocation>, NativeTarget)>,
    structural_reads: &mut Option<&mut Vec<TerminalBooleanStructuralFieldRead>>,
) -> Result<(), EmissionError> {
    match expression {
        TerminalAssignedBooleanExpression::Call {
            psi_operation,
            source_value,
            callee,
            arguments,
        } => {
            emit_aarch64_call(
                instructions,
                *psi_operation,
                *source_value,
                *callee,
                arguments,
                frame,
                stack_depth,
                internal_calls,
            )?;
            instructions.push(0x1200_0000); // and w0, w0, #1
        }
        TerminalAssignedBooleanExpression::Immediate { value, .. } => {
            emit_aarch64_mov_immediate(instructions, 0, u64::from(*value));
        }
        TerminalAssignedBooleanExpression::Parameter {
            source_value,
            location,
            ..
        } => {
            let byte_offset = match location {
                TerminalAssignedScalarLocation::FrameSpill { byte_offset } => {
                    stack_depth.checked_add(*byte_offset)
                }
                TerminalAssignedScalarLocation::IncomingStack { byte_offset } => stack_depth
                    .checked_add(frame.byte_size)
                    .and_then(|offset| offset.checked_add(*byte_offset)),
                TerminalAssignedScalarLocation::Register(_) => {
                    return Err(EmissionError::AssignedFrameArchitectureMismatch(
                        Architecture::Aarch64,
                    ));
                }
            }
            .ok_or(EmissionError::IncomingStackOffsetNotEncodable {
                value: *source_value,
                byte_offset: match location {
                    TerminalAssignedScalarLocation::Register(_)
                    | TerminalAssignedScalarLocation::FrameSpill { .. } => 0,
                    TerminalAssignedScalarLocation::IncomingStack { byte_offset } => *byte_offset,
                },
            })?;
            instructions.push(aarch64_stack_access(
                0xf940_0000,
                0,
                *source_value,
                byte_offset,
            )?);
            instructions.push(0x1200_0000); // and w0, w0, #1
        }
        TerminalAssignedBooleanExpression::StructuralField {
            psi_operation,
            source_value,
            source,
            field,
            source_placement,
            field_byte_offset,
        } => {
            let code_offset = instructions.len() * 4;
            emit_aarch64_boolean_structural_field(
                instructions,
                *source_value,
                source_placement,
                *field_byte_offset,
                frame,
                stack_depth,
            )?;
            instructions.push(0x1200_0000);
            if let Some(reads) = structural_reads.as_deref_mut() {
                reads.push(TerminalBooleanStructuralFieldRead {
                    psi_operation: *psi_operation,
                    source: *source,
                    field: *field,
                    field_byte_offset: *field_byte_offset,
                    code_offset,
                    byte_count: instructions.len() * 4 - code_offset,
                });
            }
        }
        TerminalAssignedBooleanExpression::Not { operand, .. } => {
            emit_aarch64_boolean_expression_node(
                instructions,
                operand,
                frame,
                stack_depth,
                internal_calls,
                structural_reads,
            )?;
            instructions.push(0x5200_0000); // eor w0, w0, #1
        }
        TerminalAssignedBooleanExpression::Equal { left, right, .. } => {
            emit_aarch64_boolean_expression_node(
                instructions,
                left,
                frame,
                stack_depth,
                internal_calls,
                structural_reads,
            )?;
            emit_aarch64_adjust_sp(instructions, 16, false)?;
            instructions.push(aarch64_stack_access(
                0xf900_0000,
                0,
                boolean_expression_source(left),
                0,
            )?);
            let nested_depth = stack_depth.checked_add(16).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: boolean_expression_source(left),
                },
            )?;
            emit_aarch64_boolean_expression_node(
                instructions,
                right,
                frame,
                nested_depth,
                internal_calls,
                structural_reads,
            )?;
            instructions.push(aarch64_stack_access(
                0xf940_0000,
                9,
                boolean_expression_source(left),
                0,
            )?);
            emit_aarch64_adjust_sp(instructions, 16, true)?;
            instructions.push(0x6b00_013f); // cmp w9, w0
            instructions.push(0x1a9f_17e0); // cset w0, eq
        }
        TerminalAssignedBooleanExpression::IntegerEqual {
            scalar_type,
            left,
            right,
            ..
        } => {
            emit_aarch64_expression_node(
                instructions,
                *scalar_type,
                left,
                frame,
                stack_depth,
                internal_calls,
            )?;
            emit_aarch64_adjust_sp(instructions, 16, false)?;
            instructions.push(aarch64_stack_access(
                0xf900_0000,
                0,
                expression_source(left),
                0,
            )?);
            let nested_depth = stack_depth.checked_add(16).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_aarch64_expression_node(
                instructions,
                *scalar_type,
                right,
                frame,
                nested_depth,
                internal_calls,
            )?;
            instructions.push(aarch64_stack_access(
                0xf940_0000,
                9,
                expression_source(left),
                0,
            )?);
            emit_aarch64_adjust_sp(instructions, 16, true)?;
            instructions.push(0xeb00_013f); // cmp x9, x0
            instructions.push(0x1a9f_17e0); // cset w0, eq
        }
        TerminalAssignedBooleanExpression::IntegerLessThan {
            scalar_type,
            left,
            right,
            ..
        }
        | TerminalAssignedBooleanExpression::IntegerLessOrEqual {
            scalar_type,
            left,
            right,
            ..
        } => {
            emit_aarch64_expression_node(
                instructions,
                *scalar_type,
                left,
                frame,
                stack_depth,
                internal_calls,
            )?;
            emit_aarch64_adjust_sp(instructions, 16, false)?;
            instructions.push(aarch64_stack_access(
                0xf900_0000,
                0,
                expression_source(left),
                0,
            )?);
            let nested_depth = stack_depth.checked_add(16).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_aarch64_expression_node(
                instructions,
                *scalar_type,
                right,
                frame,
                nested_depth,
                internal_calls,
            )?;
            instructions.push(aarch64_stack_access(
                0xf940_0000,
                9,
                expression_source(left),
                0,
            )?);
            emit_aarch64_adjust_sp(instructions, 16, true)?;
            instructions.push(0xeb00_013f); // cmp x9, x0
            let inclusive = matches!(
                expression,
                TerminalAssignedBooleanExpression::IntegerLessOrEqual { .. }
            );
            instructions.push(match (scalar_type.sign(), inclusive) {
                (IntegerSign::Signed, false) => 0x1a9f_a7e0, // cset w0, lt
                (IntegerSign::Unsigned, false) => 0x1a9f_27e0, // cset w0, lo
                (IntegerSign::Signed, true) => 0x1a9f_c7e0,  // cset w0, le
                (IntegerSign::Unsigned, true) => 0x1a9f_87e0, // cset w0, ls
            });
        }
    }
    Ok(())
}

fn emit_aarch64_boolean_structural_field(
    instructions: &mut Vec<u32>,
    source_value: ValueId,
    placement: &ValuePlacement,
    field_byte_offset: u32,
    frame: &TerminalExpressionFrame,
    stack_depth: u32,
) -> Result<(), EmissionError> {
    if field_byte_offset >= u32::from(placement.shape.byte_size) {
        return Err(EmissionError::UnsupportedAggregatePlacement);
    }
    if let [ValueLocation::Indirect { pointer, .. }] = placement.locations.as_slice() {
        let base = match *pointer {
            IndirectPointerLocation::Register(register) => {
                let base = aarch64_unit_register(register)?;
                if base == 0 {
                    return Err(EmissionError::ExpressionScratchRegisterConflict {
                        value: source_value,
                        register,
                    });
                }
                base
            }
            IndirectPointerLocation::Stack {
                stack_byte_offset, ..
            } => {
                let incoming = stack_depth
                    .checked_add(frame.byte_size)
                    .and_then(|offset| offset.checked_add(stack_byte_offset))
                    .ok_or(EmissionError::IncomingStackOffsetNotEncodable {
                        value: source_value,
                        byte_offset: stack_byte_offset,
                    })?;
                instructions.push(aarch64_unit_stack_access(0xf940_0000, 9, incoming, 8)?);
                9
            }
        };
        instructions.push(aarch64_unit_memory_access(
            aarch64_load_base(1)?,
            0,
            base,
            field_byte_offset,
            1,
        )?);
        return Ok(());
    }
    let location = placement
        .locations
        .iter()
        .find(|location| match location {
            ValueLocation::Register {
                value_byte_offset,
                byte_size,
                ..
            }
            | ValueLocation::Stack {
                value_byte_offset,
                byte_size,
                ..
            } => {
                let start = u32::from(*value_byte_offset);
                field_byte_offset >= start && field_byte_offset < start + u32::from(*byte_size)
            }
            ValueLocation::Indirect { .. } => false,
        })
        .ok_or(EmissionError::UnsupportedAggregatePlacement)?;
    match *location {
        ValueLocation::Register {
            register,
            value_byte_offset,
            ..
        } => {
            let register = aarch64_unit_register(register)?;
            if register == 0 {
                return Err(EmissionError::ExpressionScratchRegisterConflict {
                    value: source_value,
                    register: MachineRegister::Aarch64X(0),
                });
            }
            let shift = (field_byte_offset - u32::from(value_byte_offset)) * 8;
            instructions.push(0xd340_fc00 | (shift << 16) | (u32::from(register) << 5));
            Ok(())
        }
        ValueLocation::Stack {
            stack_byte_offset,
            value_byte_offset,
            ..
        } => {
            let incoming = stack_depth
                .checked_add(frame.byte_size)
                .and_then(|offset| offset.checked_add(stack_byte_offset))
                .and_then(|offset| {
                    offset.checked_add(field_byte_offset - u32::from(value_byte_offset))
                })
                .ok_or(EmissionError::IncomingStackOffsetNotEncodable {
                    value: source_value,
                    byte_offset: stack_byte_offset,
                })?;
            instructions.push(aarch64_unit_stack_access(
                aarch64_load_base(1)?,
                0,
                incoming,
                1,
            )?);
            Ok(())
        }
        ValueLocation::Indirect { .. } => Err(EmissionError::UnsupportedAggregatePlacement),
    }
}

pub(crate) fn emit_aarch64_integer_expression(
    scalar_type: IntegerType,
    frame: &TerminalExpressionFrame,
    expression: &TerminalAssignedIntegerExpression,
    mut internal_calls: Option<(&mut Vec<TerminalInternalCallRelocation>, NativeTarget)>,
) -> Result<Vec<u8>, EmissionError> {
    let mut instructions = Vec::new();
    if frame.byte_size != 0 {
        emit_aarch64_adjust_sp(&mut instructions, frame.byte_size, false)?;
        for spill in &frame.register_spills {
            instructions.push(aarch64_stack_access(
                0xf900_0000,
                aarch64_spill_register(spill.source_value, spill.register)?,
                spill.source_value,
                spill.byte_offset,
            )?); // str xN, [sp, #spill]
        }
    }
    emit_aarch64_expression_node(
        &mut instructions,
        scalar_type,
        expression,
        frame,
        0,
        &mut internal_calls,
    )?;
    if frame.byte_size != 0 {
        emit_aarch64_adjust_sp(&mut instructions, frame.byte_size, true)?;
    }
    instructions.push(0xd65f_03c0); // ret x30
    Ok(instructions
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect())
}

fn aarch64_spill_register(
    source_value: ValueId,
    register: MachineRegister,
) -> Result<u8, EmissionError> {
    match register {
        MachineRegister::Aarch64X(register) if register < 31 => Ok(register),
        _ => Err(EmissionError::ParameterRegisterArchitectureMismatch {
            value: source_value,
            register,
            architecture: Architecture::Aarch64,
        }),
    }
}

fn emit_aarch64_expression_node(
    instructions: &mut Vec<u32>,
    scalar_type: IntegerType,
    expression: &TerminalAssignedIntegerExpression,
    frame: &TerminalExpressionFrame,
    stack_depth: u32,
    internal_calls: &mut Option<(&mut Vec<TerminalInternalCallRelocation>, NativeTarget)>,
) -> Result<(), EmissionError> {
    match expression {
        TerminalAssignedIntegerExpression::Call {
            psi_operation,
            source_value,
            callee,
            arguments,
        } => {
            emit_aarch64_call(
                instructions,
                *psi_operation,
                *source_value,
                *callee,
                arguments,
                frame,
                stack_depth,
                internal_calls,
            )?;
            emit_aarch64_normalize(instructions, scalar_type);
        }
        TerminalAssignedIntegerExpression::Immediate {
            source_value,
            value,
        } => {
            let bits = integer_bits(*source_value, scalar_type, *value)?;
            emit_aarch64_mov_immediate(instructions, 0, bits);
            emit_aarch64_normalize(instructions, scalar_type);
        }
        TerminalAssignedIntegerExpression::Parameter {
            source_value,
            parameter_index: _,
            location,
        } => {
            let byte_offset = match location {
                TerminalAssignedScalarLocation::FrameSpill { byte_offset } => {
                    stack_depth.checked_add(*byte_offset)
                }
                TerminalAssignedScalarLocation::IncomingStack { byte_offset } => stack_depth
                    .checked_add(frame.byte_size)
                    .and_then(|offset| offset.checked_add(*byte_offset)),
                TerminalAssignedScalarLocation::Register(_) => {
                    return Err(EmissionError::AssignedFrameArchitectureMismatch(
                        Architecture::Aarch64,
                    ));
                }
            }
            .ok_or(EmissionError::IncomingStackOffsetNotEncodable {
                value: *source_value,
                byte_offset: match location {
                    TerminalAssignedScalarLocation::Register(_)
                    | TerminalAssignedScalarLocation::FrameSpill { .. } => 0,
                    TerminalAssignedScalarLocation::IncomingStack { byte_offset } => *byte_offset,
                },
            })?;
            instructions.push(aarch64_stack_access(
                0xf940_0000,
                0,
                *source_value,
                byte_offset,
            )?); // ldr x0, [sp, #value]
            emit_aarch64_normalize(instructions, scalar_type);
        }
        TerminalAssignedIntegerExpression::BitwiseNot { operand, .. } => {
            emit_aarch64_expression_node(
                instructions,
                scalar_type,
                operand,
                frame,
                stack_depth,
                internal_calls,
            )?;
            instructions.push(0xaa20_03e0); // mvn x0, x0
            emit_aarch64_normalize(instructions, scalar_type);
        }
        TerminalAssignedIntegerExpression::IntegerWiden {
            source_type,
            operand,
            ..
        }
        | TerminalAssignedIntegerExpression::IntegerExactCast {
            source_type,
            operand,
            ..
        } => {
            emit_aarch64_expression_node(
                instructions,
                *source_type,
                operand,
                frame,
                stack_depth,
                internal_calls,
            )?;
            emit_aarch64_normalize(instructions, scalar_type);
        }
        TerminalAssignedIntegerExpression::WrappingShiftLeft {
            count_type,
            value,
            count,
            ..
        }
        | TerminalAssignedIntegerExpression::WrappingShiftRight {
            count_type,
            value,
            count,
            ..
        }
        | TerminalAssignedIntegerExpression::ExactShiftLeft {
            count_type,
            value,
            count,
            ..
        }
        | TerminalAssignedIntegerExpression::ExactShiftRight {
            count_type,
            value,
            count,
            ..
        } => {
            emit_aarch64_expression_node(
                instructions,
                scalar_type,
                value,
                frame,
                stack_depth,
                internal_calls,
            )?;
            emit_aarch64_adjust_sp(instructions, 16, false)?;
            instructions.push(aarch64_stack_access(
                0xf900_0000,
                0,
                expression_source(value),
                0,
            )?); // str x0, [sp]
            let nested_depth = stack_depth.checked_add(16).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(value),
                },
            )?;
            emit_aarch64_expression_node(
                instructions,
                *count_type,
                count,
                frame,
                nested_depth,
                internal_calls,
            )?;
            instructions.push(aarch64_stack_access(
                0xf940_0000,
                9,
                expression_source(value),
                0,
            )?); // ldr x9, [sp]
            emit_aarch64_adjust_sp(instructions, 16, true)?;
            let count_mask_bits = scalar_type.bits().trailing_zeros();
            instructions.push(0x9240_0000 | ((count_mask_bits - 1) << 10)); // and x0, x0, #width-1
            match expression {
                TerminalAssignedIntegerExpression::WrappingShiftLeft { .. } => {
                    instructions.push(0x9ac0_2120); // lslv x0, x9, x0
                }
                TerminalAssignedIntegerExpression::ExactShiftLeft { .. } => {
                    instructions.push(0x9ac0_2120); // lslv x0, x9, x0
                }
                TerminalAssignedIntegerExpression::WrappingShiftRight { .. } => {
                    instructions.push(match scalar_type.sign() {
                        IntegerSign::Signed => 0x9ac0_2920,   // asrv x0, x9, x0
                        IntegerSign::Unsigned => 0x9ac0_2520, // lsrv x0, x9, x0
                    });
                }
                TerminalAssignedIntegerExpression::ExactShiftRight { .. } => {
                    instructions.push(match scalar_type.sign() {
                        IntegerSign::Signed => 0x9ac0_2920,   // asrv x0, x9, x0
                        IntegerSign::Unsigned => 0x9ac0_2520, // lsrv x0, x9, x0
                    });
                }
                _ => unreachable!("outer match admits only integer shifts"),
            }
            emit_aarch64_normalize(instructions, scalar_type);
        }
        TerminalAssignedIntegerExpression::WrappingAdd { left, right, .. }
        | TerminalAssignedIntegerExpression::BitwiseAnd { left, right, .. }
        | TerminalAssignedIntegerExpression::BitwiseOr { left, right, .. }
        | TerminalAssignedIntegerExpression::BitwiseXor { left, right, .. }
        | TerminalAssignedIntegerExpression::SaturatingAdd { left, right, .. }
        | TerminalAssignedIntegerExpression::WrappingSubtract { left, right, .. }
        | TerminalAssignedIntegerExpression::SaturatingSubtract { left, right, .. }
        | TerminalAssignedIntegerExpression::WrappingMultiply { left, right, .. }
        | TerminalAssignedIntegerExpression::SaturatingMultiply { left, right, .. } => {
            emit_aarch64_expression_node(
                instructions,
                scalar_type,
                left,
                frame,
                stack_depth,
                internal_calls,
            )?;
            emit_aarch64_adjust_sp(instructions, 16, false)?;
            instructions.push(aarch64_stack_access(
                0xf900_0000,
                0,
                expression_source(left),
                0,
            )?); // str x0, [sp]
            let nested_depth = stack_depth.checked_add(16).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_aarch64_expression_node(
                instructions,
                scalar_type,
                right,
                frame,
                nested_depth,
                internal_calls,
            )?;
            instructions.push(aarch64_stack_access(
                0xf940_0000,
                9,
                expression_source(left),
                0,
            )?); // ldr x9, [sp]
            emit_aarch64_adjust_sp(instructions, 16, true)?;
            match expression {
                TerminalAssignedIntegerExpression::BitwiseAnd { .. } => {
                    instructions.push(0x8a00_0120); // and x0, x9, x0
                    emit_aarch64_normalize(instructions, scalar_type);
                }
                TerminalAssignedIntegerExpression::BitwiseOr { .. } => {
                    instructions.push(0xaa00_0120); // orr x0, x9, x0
                    emit_aarch64_normalize(instructions, scalar_type);
                }
                TerminalAssignedIntegerExpression::BitwiseXor { .. } => {
                    instructions.push(0xca00_0120); // eor x0, x9, x0
                    emit_aarch64_normalize(instructions, scalar_type);
                }
                TerminalAssignedIntegerExpression::WrappingAdd { .. } => {
                    instructions.push(0x8b00_0120); // add x0, x9, x0
                    emit_aarch64_normalize(instructions, scalar_type);
                }
                TerminalAssignedIntegerExpression::SaturatingAdd { .. } => {
                    emit_aarch64_saturating_add(instructions, scalar_type);
                }
                TerminalAssignedIntegerExpression::WrappingSubtract { .. } => {
                    instructions.push(0xcb00_0120); // sub x0, x9, x0
                    emit_aarch64_normalize(instructions, scalar_type);
                }
                TerminalAssignedIntegerExpression::SaturatingSubtract { .. } => {
                    emit_aarch64_saturating_subtract(instructions, scalar_type);
                }
                TerminalAssignedIntegerExpression::WrappingMultiply { .. } => {
                    instructions.push(0x9b00_7d20); // mul x0, x9, x0
                    emit_aarch64_normalize(instructions, scalar_type);
                }
                TerminalAssignedIntegerExpression::SaturatingMultiply { .. } => {
                    emit_aarch64_saturating_multiply(instructions, scalar_type);
                }
                _ => unreachable!("outer match admits only binary arithmetic nodes"),
            }
        }
        TerminalAssignedIntegerExpression::ExactDivide { left, right, .. } => {
            emit_aarch64_expression_node(
                instructions,
                scalar_type,
                left,
                frame,
                stack_depth,
                internal_calls,
            )?;
            emit_aarch64_adjust_sp(instructions, 16, false)?;
            instructions.push(aarch64_stack_access(
                0xf900_0000,
                0,
                expression_source(left),
                0,
            )?); // str x0, [sp]
            let nested_depth = stack_depth.checked_add(16).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_aarch64_expression_node(
                instructions,
                scalar_type,
                right,
                frame,
                nested_depth,
                internal_calls,
            )?;
            instructions.push(aarch64_stack_access(
                0xf940_0000,
                9,
                expression_source(left),
                0,
            )?); // ldr x9, [sp]
            emit_aarch64_adjust_sp(instructions, 16, true)?;
            instructions.push(match scalar_type.sign() {
                IntegerSign::Signed => 0x9ac0_0d20,   // sdiv x0, x9, x0
                IntegerSign::Unsigned => 0x9ac0_0920, // udiv x0, x9, x0
            });
            emit_aarch64_normalize(instructions, scalar_type);
        }
        TerminalAssignedIntegerExpression::ExactRemainder { left, right, .. } => {
            emit_aarch64_expression_node(
                instructions,
                scalar_type,
                left,
                frame,
                stack_depth,
                internal_calls,
            )?;
            emit_aarch64_adjust_sp(instructions, 16, false)?;
            instructions.push(aarch64_stack_access(
                0xf900_0000,
                0,
                expression_source(left),
                0,
            )?);
            let nested_depth = stack_depth.checked_add(16).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_aarch64_expression_node(
                instructions,
                scalar_type,
                right,
                frame,
                nested_depth,
                internal_calls,
            )?;
            instructions.push(aarch64_stack_access(
                0xf940_0000,
                9,
                expression_source(left),
                0,
            )?);
            emit_aarch64_adjust_sp(instructions, 16, true)?;
            instructions.push(match scalar_type.sign() {
                IntegerSign::Signed => 0x9ac0_0d2a,   // sdiv x10, x9, x0
                IntegerSign::Unsigned => 0x9ac0_092a, // udiv x10, x9, x0
            });
            instructions.push(0x9b00_a540); // msub x0, x10, x0, x9
            emit_aarch64_normalize(instructions, scalar_type);
        }
        TerminalAssignedIntegerExpression::WrappingDivide { left, right, .. } => {
            emit_aarch64_expression_node(
                instructions,
                scalar_type,
                left,
                frame,
                stack_depth,
                internal_calls,
            )?;
            emit_aarch64_adjust_sp(instructions, 16, false)?;
            instructions.push(aarch64_stack_access(
                0xf900_0000,
                0,
                expression_source(left),
                0,
            )?);
            let nested_depth = stack_depth.checked_add(16).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_aarch64_expression_node(
                instructions,
                scalar_type,
                right,
                frame,
                nested_depth,
                internal_calls,
            )?;
            instructions.push(aarch64_stack_access(
                0xf940_0000,
                9,
                expression_source(left),
                0,
            )?);
            emit_aarch64_adjust_sp(instructions, 16, true)?;
            instructions.push(match scalar_type.sign() {
                IntegerSign::Signed => 0x9ac0_0d20,   // sdiv x0, x9, x0
                IntegerSign::Unsigned => 0x9ac0_0920, // udiv x0, x9, x0
            });
            emit_aarch64_normalize(instructions, scalar_type);
        }
        TerminalAssignedIntegerExpression::WrappingRemainder { left, right, .. }
        | TerminalAssignedIntegerExpression::SaturatingRemainder { left, right, .. } => {
            emit_aarch64_expression_node(
                instructions,
                scalar_type,
                left,
                frame,
                stack_depth,
                internal_calls,
            )?;
            emit_aarch64_adjust_sp(instructions, 16, false)?;
            instructions.push(aarch64_stack_access(
                0xf900_0000,
                0,
                expression_source(left),
                0,
            )?);
            let nested_depth = stack_depth.checked_add(16).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_aarch64_expression_node(
                instructions,
                scalar_type,
                right,
                frame,
                nested_depth,
                internal_calls,
            )?;
            instructions.push(aarch64_stack_access(
                0xf940_0000,
                9,
                expression_source(left),
                0,
            )?);
            emit_aarch64_adjust_sp(instructions, 16, true)?;
            instructions.push(match scalar_type.sign() {
                IntegerSign::Signed => 0x9ac0_0d2a,   // sdiv x10, x9, x0
                IntegerSign::Unsigned => 0x9ac0_092a, // udiv x10, x9, x0
            });
            instructions.push(0x9b00_a540); // msub x0, x10, x0, x9
            emit_aarch64_normalize(instructions, scalar_type);
        }
        TerminalAssignedIntegerExpression::SaturatingDivide { left, right, .. } => {
            emit_aarch64_expression_node(
                instructions,
                scalar_type,
                left,
                frame,
                stack_depth,
                internal_calls,
            )?;
            emit_aarch64_adjust_sp(instructions, 16, false)?;
            instructions.push(aarch64_stack_access(
                0xf900_0000,
                0,
                expression_source(left),
                0,
            )?);
            let nested_depth = stack_depth.checked_add(16).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_aarch64_expression_node(
                instructions,
                scalar_type,
                right,
                frame,
                nested_depth,
                internal_calls,
            )?;
            instructions.push(aarch64_stack_access(
                0xf940_0000,
                9,
                expression_source(left),
                0,
            )?);
            emit_aarch64_adjust_sp(instructions, 16, true)?;
            match scalar_type.sign() {
                IntegerSign::Unsigned => instructions.push(0x9ac0_0920), // udiv x0, x9, x0
                IntegerSign::Signed => {
                    let (minimum, maximum) = native_integer_bounds(scalar_type);
                    instructions.push(0x9ac0_0d2a); // sdiv x10, x9, x0
                    instructions.push(0xcb09_03eb); // neg x11, x9
                    emit_aarch64_mov_immediate(instructions, 12, maximum);
                    if scalar_type.bits() == 64 {
                        emit_aarch64_mov_immediate(instructions, 13, minimum);
                        instructions.push(0xeb0d_013f); // cmp x9, x13
                        instructions.push(aarch64_csel(11, 12, 11, 0)); // min ? max : -value
                    } else {
                        instructions.push(0xeb0c_017f); // cmp x11, x12
                        instructions.push(aarch64_csel(11, 11, 12, 13)); // min(-value, max)
                    }
                    emit_aarch64_mov_immediate(instructions, 13, u64::MAX);
                    instructions.push(0xeb0d_001f); // cmp x0, x13
                    instructions.push(aarch64_csel(0, 11, 10, 0)); // divisor -1 ? clamp : quotient
                }
            }
            emit_aarch64_normalize(instructions, scalar_type);
        }
    }
    Ok(())
}

fn emit_aarch64_normalize(instructions: &mut Vec<u32>, scalar_type: IntegerType) {
    if scalar_type.bits() == 64 {
        return;
    }
    let base = match scalar_type.sign() {
        IntegerSign::Signed => 0x9340_0000,   // sbfm
        IntegerSign::Unsigned => 0xd340_0000, // ubfm
    };
    instructions.push(base | (u32::from(scalar_type.bits() - 1) << 10));
}

fn emit_aarch64_call(
    instructions: &mut Vec<u32>,
    psi_operation: psi_core::OperationId,
    source_value: ValueId,
    callee: MachineId,
    arguments: &[TerminalAssignedCallArgument],
    frame: &TerminalExpressionFrame,
    stack_depth: u32,
    internal_calls: &mut Option<(&mut Vec<TerminalInternalCallRelocation>, NativeTarget)>,
) -> Result<(), EmissionError> {
    for argument in arguments {
        match &argument.expression {
            TerminalAssignedScalarExpression::Boolean(expression) => {
                emit_aarch64_boolean_expression_node(
                    instructions,
                    expression,
                    frame,
                    stack_depth,
                    internal_calls,
                    &mut None,
                )?;
            }
            TerminalAssignedScalarExpression::Integer {
                scalar_type,
                expression,
            } => emit_aarch64_expression_node(
                instructions,
                *scalar_type,
                expression,
                frame,
                stack_depth,
                internal_calls,
            )?,
        }
        let byte_offset = argument.spill_byte_offset.checked_add(stack_depth).ok_or(
            EmissionError::IncomingStackOffsetNotEncodable {
                value: source_value,
                byte_offset: argument.spill_byte_offset,
            },
        )?;
        instructions.push(aarch64_stack_access(
            0xf900_0000,
            0,
            source_value,
            byte_offset,
        )?);
    }
    let Some((relocations, _)) = internal_calls.as_mut() else {
        return Err(EmissionError::CallOutsideDirectReturnExpression);
    };
    let outgoing_stack_bytes = outgoing_stack_bytes(source_value, arguments)?;
    let outgoing_stack_bytes = outgoing_stack_bytes
        .checked_add(15)
        .map(|bytes| bytes & !15)
        .ok_or(EmissionError::CallStackAreaNotEncodable {
            value: source_value,
            byte_size: outgoing_stack_bytes,
        })?;
    let call_stack_bytes =
        outgoing_stack_bytes
            .checked_add(16)
            .ok_or(EmissionError::CallStackAreaNotEncodable {
                value: source_value,
                byte_size: outgoing_stack_bytes,
            })?;
    let allocation_offset = instructions.len() * 4;
    emit_aarch64_adjust_sp(instructions, call_stack_bytes, false)?;
    let link_store_offset = instructions.len() * 4;
    instructions.push(aarch64_stack_access(
        0xf900_0000,
        30,
        source_value,
        outgoing_stack_bytes,
    )?); // str x30 above outgoing arguments
    for argument in arguments {
        let TerminalAssignedCallDestination::OutgoingStack { byte_offset } = argument.destination
        else {
            continue;
        };
        let spill_byte_offset = argument
            .spill_byte_offset
            .checked_add(stack_depth)
            .and_then(|offset| offset.checked_add(call_stack_bytes))
            .ok_or(EmissionError::CallStackAreaNotEncodable {
                value: source_value,
                byte_size: call_stack_bytes,
            })?;
        instructions.push(aarch64_stack_access(
            0xf940_0000,
            0,
            source_value,
            spill_byte_offset,
        )?);
        instructions.push(aarch64_stack_access(
            0xf900_0000,
            0,
            source_value,
            byte_offset,
        )?);
    }
    for argument in arguments {
        let TerminalAssignedCallDestination::Register(register) = argument.destination else {
            continue;
        };
        let register = aarch64_spill_register(source_value, register)?;
        let byte_offset = argument
            .spill_byte_offset
            .checked_add(stack_depth)
            .and_then(|offset| offset.checked_add(call_stack_bytes))
            .ok_or(EmissionError::CallStackAreaNotEncodable {
                value: source_value,
                byte_size: call_stack_bytes,
            })?;
        instructions.push(aarch64_stack_access(
            0xf940_0000,
            register,
            source_value,
            byte_offset,
        )?);
    }
    let offset = instructions.len() * 4;
    instructions.push(0x9400_0000); // bl #0
    let link_load_offset = instructions.len() * 4;
    instructions.push(aarch64_stack_access(
        0xf940_0000,
        30,
        source_value,
        outgoing_stack_bytes,
    )?); // ldr x30 above outgoing arguments
    let release_offset = instructions.len() * 4;
    emit_aarch64_adjust_sp(instructions, call_stack_bytes, true)?;
    relocations.push(TerminalInternalCallRelocation {
        owner: TerminalCallSiteOwner::Operation(psi_operation),
        target: callee,
        unit_stack: None,
        scalar_stack: Some(TerminalScalarCallStackEvidence {
            outbound: stack_adjustment_pair(
                call_stack_bytes,
                Some((allocation_offset, 4)),
                Some((release_offset, 4)),
            ),
            aarch64_return_link: Some(TerminalAarch64ReturnLinkEvidence {
                frame_byte_offset: outgoing_stack_bytes,
                store_offset: link_store_offset,
                load_offset: link_load_offset,
            }),
        }),
        offset,
    });
    Ok(())
}

fn emit_aarch64_saturating_add(instructions: &mut Vec<u32>, scalar_type: IntegerType) {
    let (minimum, maximum) = native_integer_bounds(scalar_type);
    match (scalar_type.sign(), scalar_type.bits()) {
        (IntegerSign::Unsigned, 64) => {
            instructions.push(0xab00_0120); // adds x0, x9, x0
            emit_aarch64_mov_immediate(instructions, 10, maximum);
            instructions.push(aarch64_csel(0, 0, 10, 3)); // csel x0, x0, x10, cc
        }
        (IntegerSign::Unsigned, _) => {
            instructions.push(0x8b00_0120); // add x0, x9, x0
            emit_aarch64_mov_immediate(instructions, 10, maximum);
            instructions.push(0xeb0a_001f); // cmp x0, x10
            instructions.push(aarch64_csel(0, 0, 10, 9)); // csel x0, x0, x10, ls
        }
        (IntegerSign::Signed, 64) => {
            instructions.push(0x937f_fd2a); // asr x10, x9, 63
            emit_aarch64_mov_immediate(instructions, 11, maximum);
            instructions.push(0xca0b_014a); // eor x10, x10, x11
            instructions.push(0xab00_0120); // adds x0, x9, x0
            instructions.push(aarch64_csel(0, 0, 10, 7)); // csel x0, x0, x10, vc
        }
        (IntegerSign::Signed, _) => {
            instructions.push(0x8b00_0120); // add x0, x9, x0
            emit_aarch64_mov_immediate(instructions, 10, maximum);
            instructions.push(0xeb0a_001f); // cmp x0, x10
            instructions.push(aarch64_csel(0, 0, 10, 13)); // csel x0, x0, x10, le
            emit_aarch64_mov_immediate(instructions, 10, minimum);
            instructions.push(0xeb0a_001f); // cmp x0, x10
            instructions.push(aarch64_csel(0, 0, 10, 10)); // csel x0, x0, x10, ge
        }
    }
}

fn emit_aarch64_saturating_subtract(instructions: &mut Vec<u32>, scalar_type: IntegerType) {
    let (minimum, maximum) = native_integer_bounds(scalar_type);
    match (scalar_type.sign(), scalar_type.bits()) {
        (IntegerSign::Unsigned, _) => {
            instructions.push(0xeb00_0129); // subs x9, x9, x0
            instructions.push(aarch64_csel(0, 9, 31, 2)); // csel x0, x9, xzr, cs
        }
        (IntegerSign::Signed, 64) => {
            instructions.push(0x937f_fd2a); // asr x10, x9, 63
            emit_aarch64_mov_immediate(instructions, 11, maximum);
            instructions.push(0xca0b_014a); // eor x10, x10, x11
            instructions.push(0xeb00_0120); // subs x0, x9, x0
            instructions.push(aarch64_csel(0, 0, 10, 7)); // csel x0, x0, x10, vc
        }
        (IntegerSign::Signed, _) => {
            instructions.push(0xcb00_0120); // sub x0, x9, x0
            emit_aarch64_mov_immediate(instructions, 10, maximum);
            instructions.push(0xeb0a_001f); // cmp x0, x10
            instructions.push(aarch64_csel(0, 0, 10, 13)); // csel x0, x0, x10, le
            emit_aarch64_mov_immediate(instructions, 10, minimum);
            instructions.push(0xeb0a_001f); // cmp x0, x10
            instructions.push(aarch64_csel(0, 0, 10, 10)); // csel x0, x0, x10, ge
        }
    }
}

fn emit_aarch64_saturating_multiply(instructions: &mut Vec<u32>, scalar_type: IntegerType) {
    let (minimum, maximum) = native_integer_bounds(scalar_type);
    match (scalar_type.sign(), scalar_type.bits()) {
        (IntegerSign::Unsigned, 64) => {
            instructions.push(0x9bc0_7d2a); // umulh x10, x9, x0
            instructions.push(0x9b00_7d20); // mul x0, x9, x0
            instructions.push(0xf100_015f); // cmp x10, #0
            emit_aarch64_mov_immediate(instructions, 11, maximum);
            instructions.push(aarch64_csel(0, 0, 11, 0)); // csel x0, x0, x11, eq
        }
        (IntegerSign::Unsigned, _) => {
            instructions.push(0x9b00_7d20); // mul x0, x9, x0
            emit_aarch64_mov_immediate(instructions, 10, maximum);
            instructions.push(0xeb0a_001f); // cmp x0, x10
            instructions.push(aarch64_csel(0, 0, 10, 9)); // csel x0, x0, x10, ls
        }
        (IntegerSign::Signed, 64) => {
            instructions.push(0xca00_012b); // eor x11, x9, x0
            emit_aarch64_mov_immediate(instructions, 10, maximum);
            emit_aarch64_mov_immediate(instructions, 12, minimum);
            instructions.push(0xf100_017f); // cmp x11, #0
            instructions.push(aarch64_csel(11, 12, 10, 4)); // csel x11, x12, x10, mi
            instructions.push(0x9b40_7d2a); // smulh x10, x9, x0
            instructions.push(0x9b00_7d20); // mul x0, x9, x0
            instructions.push(0x937f_fc0c); // asr x12, x0, 63
            instructions.push(0xeb0c_015f); // cmp x10, x12
            instructions.push(aarch64_csel(0, 0, 11, 0)); // csel x0, x0, x11, eq
        }
        (IntegerSign::Signed, _) => {
            instructions.push(0x9b00_7d20); // mul x0, x9, x0
            emit_aarch64_mov_immediate(instructions, 10, maximum);
            instructions.push(0xeb0a_001f); // cmp x0, x10
            instructions.push(aarch64_csel(0, 0, 10, 13)); // csel x0, x0, x10, le
            emit_aarch64_mov_immediate(instructions, 10, minimum);
            instructions.push(0xeb0a_001f); // cmp x0, x10
            instructions.push(aarch64_csel(0, 0, 10, 10)); // csel x0, x0, x10, ge
        }
    }
    emit_aarch64_normalize(instructions, scalar_type);
}

fn emit_aarch64_mov_immediate(instructions: &mut Vec<u32>, register: u8, bits: u64) {
    for chunk in 0..4 {
        let immediate = ((bits >> (chunk * 16)) & 0xffff) as u32;
        if chunk == 0 || immediate != 0 {
            let base = if chunk == 0 { 0xd280_0000 } else { 0xf280_0000 };
            instructions
                .push(base | ((chunk as u32) << 21) | (immediate << 5) | u32::from(register));
        }
    }
}

pub(crate) fn emit_aarch64_adjust_sp(
    instructions: &mut Vec<u32>,
    byte_size: u32,
    add: bool,
) -> Result<(), EmissionError> {
    if byte_size > 0xfff {
        return Err(EmissionError::ExpressionStackFrameNotEncodable);
    }
    let base = if add { 0x9100_03ff } else { 0xd100_03ff };
    instructions.push(base | (byte_size << 10));
    Ok(())
}

fn aarch64_stack_access(
    base: u32,
    register: u8,
    source_value: ValueId,
    byte_offset: u32,
) -> Result<u32, EmissionError> {
    if !byte_offset.is_multiple_of(8) || byte_offset / 8 > 0xfff {
        return Err(EmissionError::IncomingStackOffsetNotEncodable {
            value: source_value,
            byte_offset,
        });
    }
    Ok(base | ((byte_offset / 8) << 10) | (31 << 5) | u32::from(register))
}

pub(crate) const fn aarch64_csel(destination: u8, left: u8, right: u8, condition: u8) -> u32 {
    0x9a80_0000
        | ((right as u32) << 16)
        | ((condition as u32) << 12)
        | ((left as u32) << 5)
        | destination as u32
}
