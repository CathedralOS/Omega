use omega_calling_conventions::{IndirectPointerLocation, ValueLocation, ValuePlacement};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_assigned_target_operations::{
    TerminalAssignedBooleanControl, TerminalAssignedBooleanExpression,
    TerminalAssignedCallArgument, TerminalAssignedCallDestination,
    TerminalAssignedConditionalBooleanArm, TerminalAssignedConditionalIntegerArm,
    TerminalAssignedIntegerControl, TerminalAssignedIntegerExpression,
    TerminalAssignedScalarExpression, TerminalAssignedScalarLocation, TerminalExpressionFrame,
};
use omega_terminal_machine_code::{
    TerminalBooleanStructuralFieldRead, TerminalInternalCallRelocation,
    TerminalScalarCallStackEvidence, TerminalScalarConditionalCondition,
};
use omega_terminal_target_operations::{MachineRegister, TerminalCallSiteOwner};
use psi_core::{IntegerSign, IntegerType, MachineId, ValueId};

use super::shared::{
    EmissionFragment, boolean_expression_source, emit_native_crash, expression_source,
    integer_bits, native_integer_bounds, outgoing_stack_bytes, require_native_integer_width,
    top_level_integer_conditional_evidence,
};
use crate::{EmissionError, stack_adjustment_pair, x86_unit_register};

pub(crate) fn emit_x86_64_conditional_integer_control(
    condition_source: ValueId,
    condition_location: TerminalAssignedScalarLocation,
    scalar_type: IntegerType,
    when_true: &TerminalAssignedConditionalIntegerArm,
    when_false: &TerminalAssignedConditionalIntegerArm,
    target: NativeTarget,
) -> Result<EmissionFragment, EmissionError> {
    let mut bytes = emit_x86_64_parameter_return(condition_source, false, condition_location)?;
    if bytes.pop() != Some(0xc3) {
        return Err(EmissionError::ConditionalBranchEncodingInvalid);
    }
    bytes.extend_from_slice(&[0x85, 0xc0]); // test eax, eax
    let true_fragment = emit_x86_64_integer_control(scalar_type, &when_true.control, target)?;
    let false_fragment = emit_x86_64_integer_control(scalar_type, &when_false.control, target)?;
    let displacement = i32::try_from(true_fragment.bytes.len())
        .map_err(|_| EmissionError::ConditionalBranchDistanceNotEncodable)?;
    let branch_offset = bytes.len();
    bytes.extend_from_slice(&[0x0f, 0x84]); // jz false arm
    bytes.extend_from_slice(&displacement.to_le_bytes());
    let false_arm_offset = bytes
        .len()
        .checked_add(true_fragment.bytes.len())
        .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
    let mut fragment = EmissionFragment::without_calls(bytes);
    fragment.conditional = Some(top_level_integer_conditional_evidence(
        TerminalScalarConditionalCondition::Parameter,
        branch_offset,
        6,
        false_arm_offset,
        true_fragment.conditional.as_deref(),
        false_fragment.conditional.as_deref(),
    )?);
    fragment.append(true_fragment)?;
    fragment.append(false_fragment)?;
    Ok(fragment)
}

fn emit_x86_64_integer_control(
    scalar_type: IntegerType,
    control: &TerminalAssignedIntegerControl,
    target: NativeTarget,
) -> Result<EmissionFragment, EmissionError> {
    match control {
        TerminalAssignedIntegerControl::Crash { .. } => Ok(EmissionFragment::without_calls(
            emit_native_crash(Architecture::X86_64),
        )),
        TerminalAssignedIntegerControl::Return {
            source_value,
            frame,
            expression,
            ..
        } => {
            require_native_integer_width(*source_value, scalar_type)?;
            let mut internal_calls = Vec::new();
            let bytes = emit_x86_64_integer_expression(
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
        } => emit_x86_64_conditional_integer_control(
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
        } => emit_x86_64_conditional_integer_expression_control(
            condition_frame,
            condition,
            scalar_type,
            when_true,
            when_false,
            target,
        ),
    }
}

pub(crate) fn emit_x86_64_conditional_integer_expression_control(
    condition_frame: &TerminalExpressionFrame,
    condition: &TerminalAssignedBooleanExpression,
    scalar_type: IntegerType,
    when_true: &TerminalAssignedConditionalIntegerArm,
    when_false: &TerminalAssignedConditionalIntegerArm,
    target: NativeTarget,
) -> Result<EmissionFragment, EmissionError> {
    let mut internal_calls = Vec::new();
    let mut bytes = emit_x86_64_boolean_condition_value(
        condition_frame,
        condition,
        Some((&mut internal_calls, target)),
        None,
    )?;
    let true_fragment = emit_x86_64_integer_control(scalar_type, &when_true.control, target)?;
    let false_fragment = emit_x86_64_integer_control(scalar_type, &when_false.control, target)?;
    let displacement = i32::try_from(true_fragment.bytes.len())
        .map_err(|_| EmissionError::ConditionalBranchDistanceNotEncodable)?;
    let branch_offset = bytes.len();
    bytes.extend_from_slice(&[0x0f, 0x84]); // jz false arm
    bytes.extend_from_slice(&displacement.to_le_bytes());
    let false_arm_offset = bytes
        .len()
        .checked_add(true_fragment.bytes.len())
        .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
    let conditional = top_level_integer_conditional_evidence(
        TerminalScalarConditionalCondition::Expression,
        branch_offset,
        6,
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

pub(crate) fn emit_x86_64_conditional_boolean_control(
    condition_source: ValueId,
    condition_location: TerminalAssignedScalarLocation,
    when_true: &TerminalAssignedConditionalBooleanArm,
    when_false: &TerminalAssignedConditionalBooleanArm,
    target: NativeTarget,
) -> Result<EmissionFragment, EmissionError> {
    let mut bytes = emit_x86_64_parameter_return(condition_source, false, condition_location)?;
    if bytes.pop() != Some(0xc3) {
        return Err(EmissionError::ConditionalBranchEncodingInvalid);
    }
    bytes.extend_from_slice(&[0x85, 0xc0]); // test eax, eax
    let true_fragment = emit_x86_64_boolean_control(&when_true.control, target)?;
    let false_fragment = emit_x86_64_boolean_control(&when_false.control, target)?;
    let displacement = i32::try_from(true_fragment.bytes.len())
        .map_err(|_| EmissionError::ConditionalBranchDistanceNotEncodable)?;
    let branch_offset = bytes.len();
    bytes.extend_from_slice(&[0x0f, 0x84]); // jz false arm
    bytes.extend_from_slice(&displacement.to_le_bytes());
    let false_arm_offset = bytes
        .len()
        .checked_add(true_fragment.bytes.len())
        .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
    let mut fragment = EmissionFragment::without_calls(bytes);
    fragment.conditional = Some(top_level_integer_conditional_evidence(
        TerminalScalarConditionalCondition::Parameter,
        branch_offset,
        6,
        false_arm_offset,
        true_fragment.conditional.as_deref(),
        false_fragment.conditional.as_deref(),
    )?);
    fragment.append(true_fragment)?;
    fragment.append(false_fragment)?;
    Ok(fragment)
}

pub(crate) fn emit_x86_64_conditional_boolean_expression_control(
    condition_frame: &TerminalExpressionFrame,
    condition: &TerminalAssignedBooleanExpression,
    when_true: &TerminalAssignedConditionalBooleanArm,
    when_false: &TerminalAssignedConditionalBooleanArm,
    target: NativeTarget,
) -> Result<EmissionFragment, EmissionError> {
    let mut internal_calls = Vec::new();
    let mut bytes = emit_x86_64_boolean_condition_value(
        condition_frame,
        condition,
        Some((&mut internal_calls, target)),
        None,
    )?;
    let true_fragment = emit_x86_64_boolean_control(&when_true.control, target)?;
    let false_fragment = emit_x86_64_boolean_control(&when_false.control, target)?;
    let displacement = i32::try_from(true_fragment.bytes.len())
        .map_err(|_| EmissionError::ConditionalBranchDistanceNotEncodable)?;
    let branch_offset = bytes.len();
    bytes.extend_from_slice(&[0x0f, 0x84]); // jz false arm
    bytes.extend_from_slice(&displacement.to_le_bytes());
    let false_arm_offset = bytes
        .len()
        .checked_add(true_fragment.bytes.len())
        .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
    let conditional = top_level_integer_conditional_evidence(
        TerminalScalarConditionalCondition::Expression,
        branch_offset,
        6,
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

pub(crate) fn emit_x86_64_boolean_control(
    control: &TerminalAssignedBooleanControl,
    target: NativeTarget,
) -> Result<EmissionFragment, EmissionError> {
    match control {
        TerminalAssignedBooleanControl::Crash { .. } => Ok(EmissionFragment::without_calls(
            emit_native_crash(Architecture::X86_64),
        )),
        TerminalAssignedBooleanControl::ReturnImmediate { value, .. } => Ok(
            EmissionFragment::without_calls(emit_x86_64_boolean_return(*value)),
        ),
        TerminalAssignedBooleanControl::ReturnParameter {
            source_value,
            location,
            ..
        } => Ok(EmissionFragment::without_calls(
            emit_x86_64_parameter_return(*source_value, false, *location)?,
        )),
        TerminalAssignedBooleanControl::ReturnNotParameter {
            source_value,
            location,
            ..
        } => Ok(EmissionFragment::without_calls(
            emit_x86_64_boolean_not_parameter_return(*source_value, *location)?,
        )),
        TerminalAssignedBooleanControl::ReturnExpression {
            frame, expression, ..
        } => {
            let mut internal_calls = Vec::new();
            let bytes = emit_x86_64_boolean_expression(
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
        } => emit_x86_64_conditional_boolean_control(
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
        } => emit_x86_64_conditional_boolean_expression_control(
            condition_frame,
            condition,
            when_true,
            when_false,
            target,
        ),
    }
}

pub(crate) fn emit_x86_64_boolean_not_parameter_return(
    source: ValueId,
    location: TerminalAssignedScalarLocation,
) -> Result<Vec<u8>, EmissionError> {
    let mut bytes = emit_x86_64_parameter_return(source, false, location)?;
    if bytes.pop() != Some(0xc3) {
        return Err(EmissionError::BooleanNotEncodingInvalid);
    }
    bytes.extend_from_slice(&[0x83, 0xf0, 0x01]); // xor eax, 1
    bytes.push(0xc3); // ret
    Ok(bytes)
}

pub(crate) fn emit_x86_64_boolean_return(value: bool) -> Vec<u8> {
    vec![0xb8, u8::from(value), 0, 0, 0, 0xc3] // mov eax, 0/1; ret
}

pub(crate) fn emit_x86_64_parameter_return(
    source: ValueId,
    is_64: bool,
    location: TerminalAssignedScalarLocation,
) -> Result<Vec<u8>, EmissionError> {
    let mut bytes = Vec::new();
    match location {
        TerminalAssignedScalarLocation::Register(register) => {
            let register = x86_gpr_code(source, register)?;
            let rex = 0x40 | (u8::from(is_64) << 3) | (((register >> 3) & 1) << 2);
            if rex != 0x40 {
                bytes.push(rex);
            }
            bytes.push(0x89); // mov eax/rax, selected argument register
            bytes.push(0xc0 | ((register & 7) << 3));
        }
        TerminalAssignedScalarLocation::IncomingStack { byte_offset } => {
            let displacement = byte_offset.checked_add(8).ok_or(
                EmissionError::IncomingStackOffsetNotEncodable {
                    value: source,
                    byte_offset,
                },
            )?;
            if is_64 {
                bytes.push(0x48);
            }
            bytes.push(0x8b); // mov eax/rax, [rsp + displacement]
            if displacement <= i8::MAX as u32 {
                bytes.extend_from_slice(&[0x44, 0x24, displacement as u8]);
            } else {
                bytes.extend_from_slice(&[0x84, 0x24]);
                bytes.extend_from_slice(&displacement.to_le_bytes());
            }
        }
        TerminalAssignedScalarLocation::FrameSpill { .. } => {
            return Err(EmissionError::AssignedFrameSpillOutsideExpression(source));
        }
    }
    bytes.push(0xc3);
    Ok(bytes)
}

fn x86_gpr_code(source: ValueId, register: MachineRegister) -> Result<u8, EmissionError> {
    Ok(match register {
        MachineRegister::X86Rax => 0,
        MachineRegister::X86Rcx => 1,
        MachineRegister::X86Rdx => 2,
        MachineRegister::X86Rbx => 3,
        MachineRegister::X86Rsp => 4,
        MachineRegister::X86Rbp => 5,
        MachineRegister::X86Rsi => 6,
        MachineRegister::X86Rdi => 7,
        MachineRegister::X86R8 => 8,
        MachineRegister::X86R9 => 9,
        MachineRegister::X86R10 => 10,
        MachineRegister::X86R11 => 11,
        MachineRegister::X86R12 => 12,
        MachineRegister::X86R13 => 13,
        MachineRegister::X86R14 => 14,
        MachineRegister::X86R15 => 15,
        MachineRegister::X86Xmm(_)
        | MachineRegister::Aarch64X(_)
        | MachineRegister::Aarch64V(_) => {
            return Err(EmissionError::ParameterRegisterArchitectureMismatch {
                value: source,
                register,
                architecture: Architecture::X86_64,
            });
        }
    })
}

pub(crate) fn emit_x86_64_return(scalar_type: IntegerType, bits: u64) -> Vec<u8> {
    let mut bytes = Vec::new();
    if scalar_type.bits() <= 32 {
        bytes.push(0xb8); // mov eax, imm32
        bytes.extend_from_slice(&(bits as u32).to_le_bytes());
    } else {
        bytes.extend_from_slice(&[0x48, 0xb8]); // mov rax, imm64
        bytes.extend_from_slice(&bits.to_le_bytes());
    }
    bytes.push(0xc3); // ret
    bytes
}

pub(crate) fn emit_x86_64_boolean_expression(
    frame: &TerminalExpressionFrame,
    expression: &TerminalAssignedBooleanExpression,
    internal_calls: Option<(&mut Vec<TerminalInternalCallRelocation>, NativeTarget)>,
) -> Result<Vec<u8>, EmissionError> {
    let mut bytes = emit_x86_64_boolean_expression_value(frame, expression, internal_calls)?;
    bytes.push(0xc3); // ret
    Ok(bytes)
}

fn emit_x86_64_boolean_expression_value(
    frame: &TerminalExpressionFrame,
    expression: &TerminalAssignedBooleanExpression,
    mut internal_calls: Option<(&mut Vec<TerminalInternalCallRelocation>, NativeTarget)>,
) -> Result<Vec<u8>, EmissionError> {
    if frame.byte_size == 0 && !frame.register_spills.is_empty() {
        return Err(EmissionError::AssignedFrameSizeMismatch);
    }
    let mut bytes = Vec::new();
    if frame.byte_size != 0 {
        emit_x86_64_adjust_sp(&mut bytes, frame.byte_size, false);
        for spill in &frame.register_spills {
            let register = x86_gpr_code(spill.source_value, spill.register)?;
            if register == 4 {
                return Err(EmissionError::ExpressionScratchRegisterConflict {
                    value: spill.source_value,
                    register: spill.register,
                });
            }
            emit_x86_64_stack_store(&mut bytes, register, spill.byte_offset);
        }
    }
    let mut structural_reads = None;
    emit_x86_64_boolean_expression_node(
        &mut bytes,
        expression,
        frame.byte_size,
        0,
        &mut internal_calls,
        &mut structural_reads,
    )?;
    if frame.byte_size != 0 {
        emit_x86_64_adjust_sp(&mut bytes, frame.byte_size, true);
    }
    Ok(bytes)
}

pub(crate) fn emit_x86_64_boolean_condition_value(
    frame: &TerminalExpressionFrame,
    expression: &TerminalAssignedBooleanExpression,
    mut internal_calls: Option<(&mut Vec<TerminalInternalCallRelocation>, NativeTarget)>,
    structural_reads: Option<&mut Vec<TerminalBooleanStructuralFieldRead>>,
) -> Result<Vec<u8>, EmissionError> {
    if frame.byte_size == 0 && !frame.register_spills.is_empty() {
        return Err(EmissionError::AssignedFrameSizeMismatch);
    }
    let mut bytes = Vec::new();
    if frame.byte_size != 0 {
        emit_x86_64_adjust_sp(&mut bytes, frame.byte_size, false);
        for spill in &frame.register_spills {
            let register = x86_gpr_code(spill.source_value, spill.register)?;
            if register == 4 {
                return Err(EmissionError::ExpressionScratchRegisterConflict {
                    value: spill.source_value,
                    register: spill.register,
                });
            }
            emit_x86_64_stack_store(&mut bytes, register, spill.byte_offset);
        }
    }
    let mut structural_reads = structural_reads;
    emit_x86_64_boolean_expression_node(
        &mut bytes,
        expression,
        frame.byte_size,
        0,
        &mut internal_calls,
        &mut structural_reads,
    )?;
    bytes.extend_from_slice(&[0x85, 0xc0]); // test eax, eax
    for spill in &frame.register_spills {
        let register = x86_gpr_code(spill.source_value, spill.register)?;
        emit_x86_64_stack_load(&mut bytes, register, spill.byte_offset);
    }
    if frame.byte_size != 0 {
        emit_x86_64_restore_sp_preserving_flags(&mut bytes, frame.byte_size);
    }
    Ok(bytes)
}

fn emit_x86_64_restore_sp_preserving_flags(bytes: &mut Vec<u8>, byte_size: u32) {
    if byte_size <= i8::MAX as u32 {
        bytes.extend_from_slice(&[0x48, 0x8d, 0x64, 0x24, byte_size as u8]);
    } else {
        bytes.extend_from_slice(&[0x48, 0x8d, 0xa4, 0x24]);
        bytes.extend_from_slice(&byte_size.to_le_bytes());
    }
}

fn emit_x86_64_boolean_expression_node(
    bytes: &mut Vec<u8>,
    expression: &TerminalAssignedBooleanExpression,
    frame_byte_size: u32,
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
            emit_x86_64_call(
                bytes,
                *psi_operation,
                *source_value,
                *callee,
                arguments,
                frame_byte_size,
                stack_depth,
                internal_calls,
            )?;
            bytes.extend_from_slice(&[0x83, 0xe0, 0x01]); // and eax, 1
        }
        TerminalAssignedBooleanExpression::Immediate { value, .. } => {
            bytes.push(0xb8); // mov eax, imm32
            bytes.extend_from_slice(&u32::from(*value).to_le_bytes());
        }
        TerminalAssignedBooleanExpression::Parameter {
            source_value,
            location,
            ..
        } => {
            match location {
                TerminalAssignedScalarLocation::Register(register) => {
                    let register_code = x86_gpr_code(*source_value, *register)?;
                    if matches!(register_code, 0 | 4 | 10 | 11) {
                        return Err(EmissionError::ExpressionScratchRegisterConflict {
                            value: *source_value,
                            register: *register,
                        });
                    }
                    let rex = 0x48 | (((register_code >> 3) & 1) << 2);
                    bytes.extend_from_slice(&[rex, 0x89, 0xc0 | ((register_code & 7) << 3)]);
                }
                TerminalAssignedScalarLocation::FrameSpill { byte_offset } => {
                    let displacement = byte_offset.checked_add(stack_depth).ok_or(
                        EmissionError::IncomingStackOffsetNotEncodable {
                            value: *source_value,
                            byte_offset: *byte_offset,
                        },
                    )?;
                    bytes.extend_from_slice(&[0x48, 0x8b]);
                    if displacement <= i8::MAX as u32 {
                        bytes.extend_from_slice(&[0x44, 0x24, displacement as u8]);
                    } else {
                        bytes.extend_from_slice(&[0x84, 0x24]);
                        bytes.extend_from_slice(&displacement.to_le_bytes());
                    }
                }
                TerminalAssignedScalarLocation::IncomingStack { byte_offset } => {
                    let displacement = byte_offset
                        .checked_add(8)
                        .and_then(|offset| offset.checked_add(frame_byte_size))
                        .and_then(|offset| offset.checked_add(stack_depth))
                        .ok_or(EmissionError::IncomingStackOffsetNotEncodable {
                            value: *source_value,
                            byte_offset: *byte_offset,
                        })?;
                    bytes.extend_from_slice(&[0x48, 0x8b]);
                    if displacement <= i8::MAX as u32 {
                        bytes.extend_from_slice(&[0x44, 0x24, displacement as u8]);
                    } else {
                        bytes.extend_from_slice(&[0x84, 0x24]);
                        bytes.extend_from_slice(&displacement.to_le_bytes());
                    }
                }
            }
            bytes.extend_from_slice(&[0x83, 0xe0, 0x01]); // and eax, 1
        }
        TerminalAssignedBooleanExpression::StructuralField {
            psi_operation,
            source_value,
            source,
            field,
            source_placement,
            field_byte_offset,
        } => {
            let code_offset = bytes.len();
            emit_x86_64_boolean_structural_field(
                bytes,
                *source_value,
                source_placement,
                *field_byte_offset,
                frame_byte_size,
                stack_depth,
            )?;
            bytes.extend_from_slice(&[0x83, 0xe0, 0x01]);
            if let Some(reads) = structural_reads.as_deref_mut() {
                reads.push(TerminalBooleanStructuralFieldRead {
                    psi_operation: *psi_operation,
                    source: *source,
                    field: *field,
                    field_byte_offset: *field_byte_offset,
                    code_offset,
                    byte_count: bytes.len() - code_offset,
                });
            }
        }
        TerminalAssignedBooleanExpression::Not { operand, .. } => {
            emit_x86_64_boolean_expression_node(
                bytes,
                operand,
                frame_byte_size,
                stack_depth,
                internal_calls,
                structural_reads,
            )?;
            bytes.extend_from_slice(&[0x83, 0xf0, 0x01]); // xor eax, 1
        }
        TerminalAssignedBooleanExpression::Equal { left, right, .. } => {
            emit_x86_64_boolean_expression_node(
                bytes,
                left,
                frame_byte_size,
                stack_depth,
                internal_calls,
                structural_reads,
            )?;
            bytes.push(0x50); // push rax
            let nested_depth = stack_depth.checked_add(8).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: boolean_expression_source(left),
                },
            )?;
            emit_x86_64_boolean_expression_node(
                bytes,
                right,
                frame_byte_size,
                nested_depth,
                internal_calls,
                structural_reads,
            )?;
            bytes.extend_from_slice(&[0x41, 0x5a]); // pop r10
            bytes.extend_from_slice(&[0x49, 0x39, 0xc2]); // cmp r10, rax
            bytes.extend_from_slice(&[0x0f, 0x94, 0xc0]); // sete al
            bytes.extend_from_slice(&[0x0f, 0xb6, 0xc0]); // movzx eax, al
        }
        TerminalAssignedBooleanExpression::IntegerEqual {
            scalar_type,
            left,
            right,
            ..
        } => {
            emit_x86_64_expression_node(
                bytes,
                *scalar_type,
                left,
                frame_byte_size,
                stack_depth,
                internal_calls,
            )?;
            bytes.push(0x50); // push rax
            let nested_depth = stack_depth.checked_add(8).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_x86_64_expression_node(
                bytes,
                *scalar_type,
                right,
                frame_byte_size,
                nested_depth,
                internal_calls,
            )?;
            bytes.extend_from_slice(&[0x41, 0x5a]); // pop r10
            bytes.extend_from_slice(&[0x49, 0x39, 0xc2]); // cmp r10, rax
            bytes.extend_from_slice(&[0x0f, 0x94, 0xc0]); // sete al
            bytes.extend_from_slice(&[0x0f, 0xb6, 0xc0]); // movzx eax, al
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
            emit_x86_64_expression_node(
                bytes,
                *scalar_type,
                left,
                frame_byte_size,
                stack_depth,
                internal_calls,
            )?;
            bytes.push(0x50); // push rax
            let nested_depth = stack_depth.checked_add(8).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_x86_64_expression_node(
                bytes,
                *scalar_type,
                right,
                frame_byte_size,
                nested_depth,
                internal_calls,
            )?;
            bytes.extend_from_slice(&[0x41, 0x5a]); // pop r10
            bytes.extend_from_slice(&[0x49, 0x39, 0xc2]); // cmp r10, rax
            let inclusive = matches!(
                expression,
                TerminalAssignedBooleanExpression::IntegerLessOrEqual { .. }
            );
            let setcc = match (scalar_type.sign(), inclusive) {
                (IntegerSign::Signed, false) => 0x9c,   // setl al
                (IntegerSign::Unsigned, false) => 0x92, // setb al
                (IntegerSign::Signed, true) => 0x9e,    // setle al
                (IntegerSign::Unsigned, true) => 0x96,  // setbe al
            };
            bytes.extend_from_slice(&[0x0f, setcc, 0xc0]);
            bytes.extend_from_slice(&[0x0f, 0xb6, 0xc0]); // movzx eax, al
        }
    }
    Ok(())
}

fn emit_x86_64_boolean_structural_field(
    bytes: &mut Vec<u8>,
    source_value: ValueId,
    placement: &ValuePlacement,
    field_byte_offset: u32,
    frame_byte_size: u32,
    stack_depth: u32,
) -> Result<(), EmissionError> {
    if field_byte_offset >= u32::from(placement.shape.byte_size) {
        return Err(EmissionError::UnsupportedAggregatePlacement);
    }
    if let [ValueLocation::Indirect { pointer, .. }] = placement.locations.as_slice() {
        let base = match *pointer {
            IndirectPointerLocation::Register(register) => {
                let base = x86_unit_register(register)?;
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
                let incoming = stack_byte_offset
                    .checked_add(8)
                    .and_then(|offset| offset.checked_add(frame_byte_size))
                    .and_then(|offset| offset.checked_add(stack_depth))
                    .ok_or(EmissionError::IncomingStackOffsetNotEncodable {
                        value: source_value,
                        byte_offset: stack_byte_offset,
                    })?;
                emit_x86_64_stack_load_width(bytes, 11, incoming, 8)?;
                11
            }
        };
        return emit_x86_64_memory_load_width(bytes, 0, base, field_byte_offset, 1);
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
            let register = x86_unit_register(register)?;
            if register == 0 {
                return Err(EmissionError::ExpressionScratchRegisterConflict {
                    value: source_value,
                    register: MachineRegister::X86Rax,
                });
            }
            let rex = 0x48 | (((register >> 3) & 1) << 2);
            bytes.extend_from_slice(&[rex, 0x89, 0xc0 | ((register & 7) << 3)]);
            let shift = (field_byte_offset - u32::from(value_byte_offset)) * 8;
            if shift != 0 {
                bytes.extend_from_slice(&[0x48, 0xc1, 0xe8, shift as u8]);
            }
            Ok(())
        }
        ValueLocation::Stack {
            stack_byte_offset,
            value_byte_offset,
            ..
        } => {
            let incoming = stack_byte_offset
                .checked_add(field_byte_offset - u32::from(value_byte_offset))
                .and_then(|offset| offset.checked_add(8))
                .and_then(|offset| offset.checked_add(frame_byte_size))
                .and_then(|offset| offset.checked_add(stack_depth))
                .ok_or(EmissionError::IncomingStackOffsetNotEncodable {
                    value: source_value,
                    byte_offset: stack_byte_offset,
                })?;
            emit_x86_64_stack_load_width(bytes, 0, incoming, 1)
        }
        ValueLocation::Indirect { .. } => Err(EmissionError::UnsupportedAggregatePlacement),
    }
}

pub(crate) fn emit_x86_64_integer_expression(
    scalar_type: IntegerType,
    frame: &TerminalExpressionFrame,
    expression: &TerminalAssignedIntegerExpression,
    mut internal_calls: Option<(&mut Vec<TerminalInternalCallRelocation>, NativeTarget)>,
) -> Result<Vec<u8>, EmissionError> {
    if frame.byte_size == 0 && !frame.register_spills.is_empty() {
        return Err(EmissionError::AssignedFrameSizeMismatch);
    }
    let mut bytes = Vec::new();
    if frame.byte_size != 0 {
        emit_x86_64_adjust_sp(&mut bytes, frame.byte_size, false);
        for spill in &frame.register_spills {
            let register = x86_gpr_code(spill.source_value, spill.register)?;
            if register == 4 {
                return Err(EmissionError::ExpressionScratchRegisterConflict {
                    value: spill.source_value,
                    register: spill.register,
                });
            }
            emit_x86_64_stack_store(&mut bytes, register, spill.byte_offset);
        }
    }
    emit_x86_64_expression_node(
        &mut bytes,
        scalar_type,
        expression,
        frame.byte_size,
        0,
        &mut internal_calls,
    )?;
    if frame.byte_size != 0 {
        emit_x86_64_adjust_sp(&mut bytes, frame.byte_size, true);
    }
    bytes.push(0xc3); // ret
    Ok(bytes)
}

pub(crate) fn emit_x86_64_adjust_sp(bytes: &mut Vec<u8>, byte_size: u32, add: bool) {
    if byte_size <= i8::MAX as u32 {
        bytes.extend_from_slice(&[0x48, 0x83, if add { 0xc4 } else { 0xec }, byte_size as u8]);
    } else {
        bytes.extend_from_slice(&[0x48, 0x81, if add { 0xc4 } else { 0xec }]);
        bytes.extend_from_slice(&byte_size.to_le_bytes());
    }
}

pub(crate) fn emit_x86_64_stack_store(bytes: &mut Vec<u8>, register: u8, byte_offset: u32) {
    bytes.push(0x48 | (((register >> 3) & 1) << 2));
    bytes.push(0x89); // mov [rsp + displacement], selected incoming register
    if byte_offset <= i8::MAX as u32 {
        bytes.extend_from_slice(&[0x44 | ((register & 7) << 3), 0x24, byte_offset as u8]);
    } else {
        bytes.extend_from_slice(&[0x84 | ((register & 7) << 3), 0x24]);
        bytes.extend_from_slice(&byte_offset.to_le_bytes());
    }
}

pub(crate) fn emit_x86_64_stack_load(bytes: &mut Vec<u8>, register: u8, byte_offset: u32) {
    bytes.push(0x48 | (((register >> 3) & 1) << 2));
    bytes.push(0x8b); // mov selected register, [rsp + displacement]
    if byte_offset <= i8::MAX as u32 {
        bytes.extend_from_slice(&[0x44 | ((register & 7) << 3), 0x24, byte_offset as u8]);
    } else {
        bytes.extend_from_slice(&[0x84 | ((register & 7) << 3), 0x24]);
        bytes.extend_from_slice(&byte_offset.to_le_bytes());
    }
}

pub(crate) fn emit_x86_64_stack_store_width(
    bytes: &mut Vec<u8>,
    register: u8,
    byte_offset: u32,
    byte_size: u16,
) -> Result<(), EmissionError> {
    match byte_size {
        1 => bytes.push(0x40 | (((register >> 3) & 1) << 2)),
        2 => {
            bytes.push(0x66);
            bytes.push(0x40 | (((register >> 3) & 1) << 2));
        }
        4 => bytes.push(0x40 | (((register >> 3) & 1) << 2)),
        8 => bytes.push(0x48 | (((register >> 3) & 1) << 2)),
        width => return Err(EmissionError::UnsupportedAggregateFragmentWidth(width)),
    }
    bytes.push(if byte_size == 1 { 0x88 } else { 0x89 });
    emit_x86_64_rsp_modrm(bytes, register, byte_offset);
    Ok(())
}

pub(crate) fn emit_x86_64_stack_load_width(
    bytes: &mut Vec<u8>,
    register: u8,
    byte_offset: u32,
    byte_size: u16,
) -> Result<(), EmissionError> {
    match byte_size {
        1 => {
            bytes.push(0x40 | (((register >> 3) & 1) << 2));
            bytes.extend_from_slice(&[0x0f, 0xb6]);
        }
        2 => {
            bytes.push(0x66);
            bytes.push(0x40 | (((register >> 3) & 1) << 2));
            bytes.extend_from_slice(&[0x0f, 0xb7]);
        }
        4 => {
            bytes.push(0x40 | (((register >> 3) & 1) << 2));
            bytes.push(0x8b);
        }
        8 => {
            bytes.push(0x48 | (((register >> 3) & 1) << 2));
            bytes.push(0x8b);
        }
        width => return Err(EmissionError::UnsupportedAggregateFragmentWidth(width)),
    }
    emit_x86_64_rsp_modrm(bytes, register, byte_offset);
    Ok(())
}

pub(crate) fn emit_x86_64_memory_load_width(
    bytes: &mut Vec<u8>,
    destination: u8,
    base: u8,
    byte_offset: u32,
    byte_size: u16,
) -> Result<(), EmissionError> {
    match byte_size {
        1 => {
            bytes.push(0x40 | (((destination >> 3) & 1) << 2) | ((base >> 3) & 1));
            bytes.extend_from_slice(&[0x0f, 0xb6]);
        }
        2 => {
            bytes.push(0x66);
            bytes.push(0x40 | (((destination >> 3) & 1) << 2) | ((base >> 3) & 1));
            bytes.extend_from_slice(&[0x0f, 0xb7]);
        }
        4 => {
            bytes.push(0x40 | (((destination >> 3) & 1) << 2) | ((base >> 3) & 1));
            bytes.push(0x8b);
        }
        8 => {
            bytes.push(0x48 | (((destination >> 3) & 1) << 2) | ((base >> 3) & 1));
            bytes.push(0x8b);
        }
        width => return Err(EmissionError::UnsupportedAggregateFragmentWidth(width)),
    }
    if byte_offset == 0 && (base & 7) != 5 {
        bytes.push(((destination & 7) << 3) | (base & 7));
    } else if byte_offset <= i8::MAX as u32 {
        bytes.push(0x40 | ((destination & 7) << 3) | (base & 7));
        bytes.push(byte_offset as u8);
    } else {
        bytes.push(0x80 | ((destination & 7) << 3) | (base & 7));
        bytes.extend_from_slice(&byte_offset.to_le_bytes());
    }
    Ok(())
}

fn emit_x86_64_rsp_modrm(bytes: &mut Vec<u8>, register: u8, byte_offset: u32) {
    if byte_offset <= i8::MAX as u32 {
        bytes.extend_from_slice(&[0x44 | ((register & 7) << 3), 0x24, byte_offset as u8]);
    } else {
        bytes.extend_from_slice(&[0x84 | ((register & 7) << 3), 0x24]);
        bytes.extend_from_slice(&byte_offset.to_le_bytes());
    }
}

fn emit_x86_64_call(
    bytes: &mut Vec<u8>,
    psi_operation: psi_core::OperationId,
    source_value: ValueId,
    callee: MachineId,
    arguments: &[TerminalAssignedCallArgument],
    frame_byte_size: u32,
    stack_depth: u32,
    internal_calls: &mut Option<(&mut Vec<TerminalInternalCallRelocation>, NativeTarget)>,
) -> Result<(), EmissionError> {
    for argument in arguments {
        match &argument.expression {
            TerminalAssignedScalarExpression::Boolean(expression) => {
                emit_x86_64_boolean_expression_node(
                    bytes,
                    expression,
                    frame_byte_size,
                    stack_depth,
                    internal_calls,
                    &mut None,
                )?;
            }
            TerminalAssignedScalarExpression::Integer {
                scalar_type,
                expression,
            } => emit_x86_64_expression_node(
                bytes,
                *scalar_type,
                expression,
                frame_byte_size,
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
        emit_x86_64_stack_store(bytes, 0, byte_offset);
    }
    let Some((relocations, target)) = internal_calls.as_mut() else {
        return Err(EmissionError::CallOutsideDirectReturnExpression);
    };
    let shadow_bytes = if target.object_format == ObjectFormat::Coff {
        32
    } else {
        0
    };
    let outgoing_stack_bytes = outgoing_stack_bytes(source_value, arguments)?.max(shadow_bytes);
    let unaligned_depth = stack_depth.checked_add(outgoing_stack_bytes).ok_or(
        EmissionError::CallStackAreaNotEncodable {
            value: source_value,
            byte_size: outgoing_stack_bytes,
        },
    )?;
    // Entry RSP is 8 modulo 16 after the return address. Expression frames are
    // 16-byte aligned, so the call-time allocation must make the cumulative
    // depth 8 modulo 16 before `call` pushes the next return address.
    let alignment_padding = (8 + 16 - (unaligned_depth % 16)) % 16;
    let call_stack_bytes = outgoing_stack_bytes.checked_add(alignment_padding).ok_or(
        EmissionError::CallStackAreaNotEncodable {
            value: source_value,
            byte_size: outgoing_stack_bytes,
        },
    )?;
    let mut allocation = None;
    if call_stack_bytes != 0 {
        let allocation_offset = bytes.len();
        emit_x86_64_adjust_sp(bytes, call_stack_bytes, false);
        allocation = Some((allocation_offset, bytes.len() - allocation_offset));
    }
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
        emit_x86_64_stack_load(bytes, 0, spill_byte_offset);
        emit_x86_64_stack_store(bytes, 0, byte_offset);
    }
    for argument in arguments {
        let TerminalAssignedCallDestination::Register(register) = argument.destination else {
            continue;
        };
        let register = x86_gpr_code(source_value, register)?;
        if register == 4 {
            return Err(EmissionError::UnsupportedCallArgumentRegister(
                MachineRegister::X86Rsp,
            ));
        }
        let byte_offset = argument
            .spill_byte_offset
            .checked_add(stack_depth)
            .and_then(|offset| offset.checked_add(call_stack_bytes))
            .ok_or(EmissionError::CallStackAreaNotEncodable {
                value: source_value,
                byte_size: call_stack_bytes,
            })?;
        emit_x86_64_stack_load(bytes, register, byte_offset);
    }
    bytes.push(0xe8); // call rel32
    let offset = bytes.len();
    bytes.extend_from_slice(&0_i32.to_le_bytes());
    let mut release = None;
    if call_stack_bytes != 0 {
        let release_offset = bytes.len();
        emit_x86_64_adjust_sp(bytes, call_stack_bytes, true);
        release = Some((release_offset, bytes.len() - release_offset));
    }
    relocations.push(TerminalInternalCallRelocation {
        owner: TerminalCallSiteOwner::Operation(psi_operation),
        target: callee,
        unit_stack: None,
        scalar_stack: Some(TerminalScalarCallStackEvidence {
            outbound: stack_adjustment_pair(call_stack_bytes, allocation, release),
            aarch64_return_link: None,
        }),
        offset,
    });
    Ok(())
}

fn emit_x86_64_expression_node(
    bytes: &mut Vec<u8>,
    scalar_type: IntegerType,
    expression: &TerminalAssignedIntegerExpression,
    frame_byte_size: u32,
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
            emit_x86_64_call(
                bytes,
                *psi_operation,
                *source_value,
                *callee,
                arguments,
                frame_byte_size,
                stack_depth,
                internal_calls,
            )?;
            emit_x86_64_normalize(bytes, scalar_type);
        }
        TerminalAssignedIntegerExpression::Immediate {
            source_value,
            value,
        } => {
            let bits = integer_bits(*source_value, scalar_type, *value)?;
            bytes.extend_from_slice(&[0x48, 0xb8]); // mov rax, imm64
            bytes.extend_from_slice(&bits.to_le_bytes());
            emit_x86_64_normalize(bytes, scalar_type);
        }
        TerminalAssignedIntegerExpression::Parameter {
            source_value,
            location,
            ..
        } => {
            match location {
                TerminalAssignedScalarLocation::Register(register) => {
                    let register_code = x86_gpr_code(*source_value, *register)?;
                    if matches!(register_code, 0 | 4 | 10 | 11) {
                        return Err(EmissionError::ExpressionScratchRegisterConflict {
                            value: *source_value,
                            register: *register,
                        });
                    }
                    let rex = 0x48 | (((register_code >> 3) & 1) << 2);
                    bytes.extend_from_slice(&[rex, 0x89, 0xc0 | ((register_code & 7) << 3)]);
                    // mov rax, selected argument register
                }
                TerminalAssignedScalarLocation::FrameSpill { byte_offset } => {
                    let displacement = byte_offset.checked_add(stack_depth).ok_or(
                        EmissionError::IncomingStackOffsetNotEncodable {
                            value: *source_value,
                            byte_offset: *byte_offset,
                        },
                    )?;
                    bytes.extend_from_slice(&[0x48, 0x8b]); // mov rax, [rsp + spill]
                    if displacement <= i8::MAX as u32 {
                        bytes.extend_from_slice(&[0x44, 0x24, displacement as u8]);
                    } else {
                        bytes.extend_from_slice(&[0x84, 0x24]);
                        bytes.extend_from_slice(&displacement.to_le_bytes());
                    }
                }
                TerminalAssignedScalarLocation::IncomingStack { byte_offset } => {
                    let displacement = byte_offset
                        .checked_add(8)
                        .and_then(|offset| offset.checked_add(frame_byte_size))
                        .and_then(|offset| offset.checked_add(stack_depth))
                        .ok_or(EmissionError::IncomingStackOffsetNotEncodable {
                            value: *source_value,
                            byte_offset: *byte_offset,
                        })?;
                    bytes.extend_from_slice(&[0x48, 0x8b]); // mov rax, [rsp + displacement]
                    if displacement <= i8::MAX as u32 {
                        bytes.extend_from_slice(&[0x44, 0x24, displacement as u8]);
                    } else {
                        bytes.extend_from_slice(&[0x84, 0x24]);
                        bytes.extend_from_slice(&displacement.to_le_bytes());
                    }
                }
            }
            emit_x86_64_normalize(bytes, scalar_type);
        }
        TerminalAssignedIntegerExpression::BitwiseNot { operand, .. } => {
            emit_x86_64_expression_node(
                bytes,
                scalar_type,
                operand,
                frame_byte_size,
                stack_depth,
                internal_calls,
            )?;
            bytes.extend_from_slice(&[0x48, 0xf7, 0xd0]); // not rax
            emit_x86_64_normalize(bytes, scalar_type);
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
            emit_x86_64_expression_node(
                bytes,
                *source_type,
                operand,
                frame_byte_size,
                stack_depth,
                internal_calls,
            )?;
            emit_x86_64_normalize(bytes, scalar_type);
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
            emit_x86_64_expression_node(
                bytes,
                scalar_type,
                value,
                frame_byte_size,
                stack_depth,
                internal_calls,
            )?;
            bytes.push(0x50); // push rax
            let nested_depth = stack_depth.checked_add(8).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(value),
                },
            )?;
            emit_x86_64_expression_node(
                bytes,
                *count_type,
                count,
                frame_byte_size,
                nested_depth,
                internal_calls,
            )?;
            bytes.extend_from_slice(&[0x41, 0x5a]); // pop r10
            bytes.extend_from_slice(&[0x48, 0x89, 0xc1]); // mov rcx, rax
            bytes.extend_from_slice(&[0x83, 0xe1, (scalar_type.bits() - 1) as u8]); // and ecx, width - 1
            match expression {
                TerminalAssignedIntegerExpression::WrappingShiftLeft { .. } => {
                    bytes.extend_from_slice(&[0x49, 0xd3, 0xe2]); // shl r10, cl
                }
                TerminalAssignedIntegerExpression::ExactShiftLeft { .. } => {
                    bytes.extend_from_slice(&[0x49, 0xd3, 0xe2]); // shl r10, cl
                }
                TerminalAssignedIntegerExpression::WrappingShiftRight { .. } => {
                    match scalar_type.sign() {
                        IntegerSign::Signed => {
                            bytes.extend_from_slice(&[0x49, 0xd3, 0xfa]); // sar r10, cl
                        }
                        IntegerSign::Unsigned => {
                            bytes.extend_from_slice(&[0x49, 0xd3, 0xea]); // shr r10, cl
                        }
                    }
                }
                TerminalAssignedIntegerExpression::ExactShiftRight { .. } => {
                    match scalar_type.sign() {
                        IntegerSign::Signed => {
                            bytes.extend_from_slice(&[0x49, 0xd3, 0xfa]); // sar r10, cl
                        }
                        IntegerSign::Unsigned => {
                            bytes.extend_from_slice(&[0x49, 0xd3, 0xea]); // shr r10, cl
                        }
                    }
                }
                _ => unreachable!("outer match admits only integer shifts"),
            }
            bytes.extend_from_slice(&[0x4c, 0x89, 0xd0]); // mov rax, r10
            emit_x86_64_normalize(bytes, scalar_type);
        }
        TerminalAssignedIntegerExpression::WrappingAdd { left, right, .. }
        | TerminalAssignedIntegerExpression::ExactAdd { left, right, .. }
        | TerminalAssignedIntegerExpression::BitwiseAnd { left, right, .. }
        | TerminalAssignedIntegerExpression::BitwiseOr { left, right, .. }
        | TerminalAssignedIntegerExpression::BitwiseXor { left, right, .. }
        | TerminalAssignedIntegerExpression::SaturatingAdd { left, right, .. }
        | TerminalAssignedIntegerExpression::WrappingSubtract { left, right, .. }
        | TerminalAssignedIntegerExpression::ExactSubtract { left, right, .. }
        | TerminalAssignedIntegerExpression::SaturatingSubtract { left, right, .. }
        | TerminalAssignedIntegerExpression::WrappingMultiply { left, right, .. }
        | TerminalAssignedIntegerExpression::ExactMultiply { left, right, .. }
        | TerminalAssignedIntegerExpression::SaturatingMultiply { left, right, .. } => {
            emit_x86_64_expression_node(
                bytes,
                scalar_type,
                left,
                frame_byte_size,
                stack_depth,
                internal_calls,
            )?;
            bytes.push(0x50); // push rax
            let nested_depth = stack_depth.checked_add(8).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_x86_64_expression_node(
                bytes,
                scalar_type,
                right,
                frame_byte_size,
                nested_depth,
                internal_calls,
            )?;
            bytes.extend_from_slice(&[0x41, 0x5a]); // pop r10
            match expression {
                TerminalAssignedIntegerExpression::BitwiseAnd { .. } => {
                    bytes.extend_from_slice(&[0x4c, 0x21, 0xd0]); // and rax, r10
                    emit_x86_64_normalize(bytes, scalar_type);
                }
                TerminalAssignedIntegerExpression::BitwiseOr { .. } => {
                    bytes.extend_from_slice(&[0x4c, 0x09, 0xd0]); // or rax, r10
                    emit_x86_64_normalize(bytes, scalar_type);
                }
                TerminalAssignedIntegerExpression::BitwiseXor { .. } => {
                    bytes.extend_from_slice(&[0x4c, 0x31, 0xd0]); // xor rax, r10
                    emit_x86_64_normalize(bytes, scalar_type);
                }
                TerminalAssignedIntegerExpression::WrappingAdd { .. }
                | TerminalAssignedIntegerExpression::ExactAdd { .. } => {
                    bytes.extend_from_slice(&[0x4c, 0x01, 0xd0]); // add rax, r10
                    emit_x86_64_normalize(bytes, scalar_type);
                }
                TerminalAssignedIntegerExpression::SaturatingAdd { .. } => {
                    emit_x86_64_saturating_add(bytes, scalar_type);
                }
                TerminalAssignedIntegerExpression::WrappingSubtract { .. }
                | TerminalAssignedIntegerExpression::ExactSubtract { .. } => {
                    bytes.extend_from_slice(&[0x49, 0x29, 0xc2]); // sub r10, rax
                    bytes.extend_from_slice(&[0x4c, 0x89, 0xd0]); // mov rax, r10
                    emit_x86_64_normalize(bytes, scalar_type);
                }
                TerminalAssignedIntegerExpression::SaturatingSubtract { .. } => {
                    emit_x86_64_saturating_subtract(bytes, scalar_type);
                }
                TerminalAssignedIntegerExpression::WrappingMultiply { .. }
                | TerminalAssignedIntegerExpression::ExactMultiply { .. } => {
                    bytes.extend_from_slice(&[0x49, 0x0f, 0xaf, 0xc2]); // imul rax, r10
                    emit_x86_64_normalize(bytes, scalar_type);
                }
                TerminalAssignedIntegerExpression::SaturatingMultiply { .. } => {
                    emit_x86_64_saturating_multiply(bytes, scalar_type);
                }
                _ => unreachable!("outer match admits only binary arithmetic nodes"),
            }
        }
        TerminalAssignedIntegerExpression::ExactDivide { left, right, .. } => {
            emit_x86_64_expression_node(
                bytes,
                scalar_type,
                left,
                frame_byte_size,
                stack_depth,
                internal_calls,
            )?;
            bytes.push(0x50); // push rax
            let nested_depth = stack_depth.checked_add(8).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_x86_64_expression_node(
                bytes,
                scalar_type,
                right,
                frame_byte_size,
                nested_depth,
                internal_calls,
            )?;
            bytes.extend_from_slice(&[0x41, 0x5a]); // pop r10
            bytes.push(0x50); // push divisor
            bytes.extend_from_slice(&[0x4c, 0x89, 0xd0]); // mov rax, r10
            match scalar_type.sign() {
                IntegerSign::Signed => {
                    bytes.extend_from_slice(&[0x48, 0x99]); // cqo
                    bytes.extend_from_slice(&[0x48, 0xf7, 0x3c, 0x24]); // idiv qword [rsp]
                }
                IntegerSign::Unsigned => {
                    bytes.extend_from_slice(&[0x31, 0xd2]); // xor edx, edx
                    bytes.extend_from_slice(&[0x48, 0xf7, 0x34, 0x24]); // div qword [rsp]
                }
            }
            bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]); // add rsp, 8
            emit_x86_64_normalize(bytes, scalar_type);
        }
        TerminalAssignedIntegerExpression::ExactRemainder { left, right, .. } => {
            emit_x86_64_expression_node(
                bytes,
                scalar_type,
                left,
                frame_byte_size,
                stack_depth,
                internal_calls,
            )?;
            bytes.push(0x50);
            let nested_depth = stack_depth.checked_add(8).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_x86_64_expression_node(
                bytes,
                scalar_type,
                right,
                frame_byte_size,
                nested_depth,
                internal_calls,
            )?;
            bytes.extend_from_slice(&[0x41, 0x5a]); // pop r10
            bytes.push(0x50); // push divisor
            bytes.extend_from_slice(&[0x4c, 0x89, 0xd0]); // mov rax, r10
            match scalar_type.sign() {
                IntegerSign::Signed => {
                    bytes.extend_from_slice(&[0x48, 0x99]);
                    bytes.extend_from_slice(&[0x48, 0xf7, 0x3c, 0x24]);
                }
                IntegerSign::Unsigned => {
                    bytes.extend_from_slice(&[0x31, 0xd2]);
                    bytes.extend_from_slice(&[0x48, 0xf7, 0x34, 0x24]);
                }
            }
            bytes.extend_from_slice(&[0x48, 0x89, 0xd0]); // mov rax, rdx
            bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]);
            emit_x86_64_normalize(bytes, scalar_type);
        }
        TerminalAssignedIntegerExpression::WrappingDivide { left, right, .. } => {
            emit_x86_64_expression_node(
                bytes,
                scalar_type,
                left,
                frame_byte_size,
                stack_depth,
                internal_calls,
            )?;
            bytes.push(0x50);
            let nested_depth = stack_depth.checked_add(8).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_x86_64_expression_node(
                bytes,
                scalar_type,
                right,
                frame_byte_size,
                nested_depth,
                internal_calls,
            )?;
            bytes.extend_from_slice(&[0x41, 0x5a]); // pop r10
            bytes.push(0x50); // push divisor
            bytes.extend_from_slice(&[0x4c, 0x89, 0xd0]); // mov rax, r10
            match scalar_type.sign() {
                IntegerSign::Unsigned => {
                    bytes.extend_from_slice(&[0x31, 0xd2]);
                    bytes.extend_from_slice(&[0x48, 0xf7, 0x34, 0x24]);
                    bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]);
                    emit_x86_64_normalize(bytes, scalar_type);
                }
                IntegerSign::Signed => {
                    let mut negative_one = vec![0x48, 0xf7, 0xd8]; // neg rax
                    negative_one.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]);
                    emit_x86_64_normalize(&mut negative_one, scalar_type);

                    let mut ordinary = vec![0x48, 0x99]; // cqo
                    ordinary.extend_from_slice(&[0x48, 0xf7, 0x3c, 0x24]);
                    ordinary.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]);
                    emit_x86_64_normalize(&mut ordinary, scalar_type);

                    bytes.extend_from_slice(&[0x48, 0x83, 0x3c, 0x24, 0xff]); // cmp [rsp], -1
                    bytes.extend_from_slice(&[0x0f, 0x85]); // jne ordinary
                    let ordinary_offset = i32::try_from(negative_one.len() + 5)
                        .expect("wrapping-divide branch is small");
                    bytes.extend_from_slice(&ordinary_offset.to_le_bytes());
                    bytes.extend_from_slice(&negative_one);
                    bytes.push(0xe9); // jmp done
                    let done_offset =
                        i32::try_from(ordinary.len()).expect("wrapping-divide branch is small");
                    bytes.extend_from_slice(&done_offset.to_le_bytes());
                    bytes.extend_from_slice(&ordinary);
                }
            }
        }
        TerminalAssignedIntegerExpression::WrappingRemainder { left, right, .. }
        | TerminalAssignedIntegerExpression::SaturatingRemainder { left, right, .. } => {
            emit_x86_64_expression_node(
                bytes,
                scalar_type,
                left,
                frame_byte_size,
                stack_depth,
                internal_calls,
            )?;
            bytes.push(0x50);
            let nested_depth = stack_depth.checked_add(8).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_x86_64_expression_node(
                bytes,
                scalar_type,
                right,
                frame_byte_size,
                nested_depth,
                internal_calls,
            )?;
            bytes.extend_from_slice(&[0x41, 0x5a]); // pop r10
            bytes.push(0x50); // push divisor
            bytes.extend_from_slice(&[0x4c, 0x89, 0xd0]); // mov rax, r10
            match scalar_type.sign() {
                IntegerSign::Unsigned => {
                    bytes.extend_from_slice(&[0x31, 0xd2]);
                    bytes.extend_from_slice(&[0x48, 0xf7, 0x34, 0x24]);
                    bytes.extend_from_slice(&[0x48, 0x89, 0xd0]); // mov rax, rdx
                    bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]);
                    emit_x86_64_normalize(bytes, scalar_type);
                }
                IntegerSign::Signed => {
                    let mut negative_one = vec![0x31, 0xc0]; // xor eax, eax
                    negative_one.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]);
                    emit_x86_64_normalize(&mut negative_one, scalar_type);

                    let mut ordinary = vec![0x48, 0x99]; // cqo
                    ordinary.extend_from_slice(&[0x48, 0xf7, 0x3c, 0x24]);
                    ordinary.extend_from_slice(&[0x48, 0x89, 0xd0]); // mov rax, rdx
                    ordinary.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]);
                    emit_x86_64_normalize(&mut ordinary, scalar_type);

                    bytes.extend_from_slice(&[0x48, 0x83, 0x3c, 0x24, 0xff]); // cmp [rsp], -1
                    bytes.extend_from_slice(&[0x0f, 0x85]); // jne ordinary
                    let ordinary_offset = i32::try_from(negative_one.len() + 5)
                        .expect("wrapping-remainder branch is small");
                    bytes.extend_from_slice(&ordinary_offset.to_le_bytes());
                    bytes.extend_from_slice(&negative_one);
                    bytes.push(0xe9); // jmp done
                    let done_offset =
                        i32::try_from(ordinary.len()).expect("wrapping-remainder branch is small");
                    bytes.extend_from_slice(&done_offset.to_le_bytes());
                    bytes.extend_from_slice(&ordinary);
                }
            }
        }
        TerminalAssignedIntegerExpression::SaturatingDivide { left, right, .. } => {
            emit_x86_64_expression_node(
                bytes,
                scalar_type,
                left,
                frame_byte_size,
                stack_depth,
                internal_calls,
            )?;
            bytes.push(0x50);
            let nested_depth = stack_depth.checked_add(8).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_x86_64_expression_node(
                bytes,
                scalar_type,
                right,
                frame_byte_size,
                nested_depth,
                internal_calls,
            )?;
            bytes.extend_from_slice(&[0x41, 0x5a]); // pop r10
            bytes.push(0x50); // push divisor
            bytes.extend_from_slice(&[0x4c, 0x89, 0xd0]); // mov rax, r10
            match scalar_type.sign() {
                IntegerSign::Unsigned => {
                    bytes.extend_from_slice(&[0x31, 0xd2]);
                    bytes.extend_from_slice(&[0x48, 0xf7, 0x34, 0x24]);
                    bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]);
                    emit_x86_64_normalize(bytes, scalar_type);
                }
                IntegerSign::Signed => {
                    let (_, maximum) = native_integer_bounds(scalar_type);
                    let mut negative_one = vec![0x48, 0xf7, 0xd8]; // neg rax
                    emit_x86_64_mov_r10(&mut negative_one, maximum);
                    if scalar_type.bits() == 64 {
                        negative_one.extend_from_slice(&[0x49, 0x0f, 0x40, 0xc2]); // cmovo rax, r10
                    } else {
                        negative_one.extend_from_slice(&[0x4c, 0x39, 0xd0]); // cmp rax, r10
                        negative_one.extend_from_slice(&[0x49, 0x0f, 0x4f, 0xc2]); // cmovg rax, r10
                    }
                    negative_one.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]);
                    emit_x86_64_normalize(&mut negative_one, scalar_type);

                    let mut ordinary = vec![0x48, 0x99]; // cqo
                    ordinary.extend_from_slice(&[0x48, 0xf7, 0x3c, 0x24]);
                    ordinary.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]);
                    emit_x86_64_normalize(&mut ordinary, scalar_type);

                    bytes.extend_from_slice(&[0x48, 0x83, 0x3c, 0x24, 0xff]); // cmp [rsp], -1
                    bytes.extend_from_slice(&[0x0f, 0x85]); // jne ordinary
                    let ordinary_offset = i32::try_from(negative_one.len() + 5)
                        .expect("saturating-divide branch is small");
                    bytes.extend_from_slice(&ordinary_offset.to_le_bytes());
                    bytes.extend_from_slice(&negative_one);
                    bytes.push(0xe9); // jmp done
                    let done_offset =
                        i32::try_from(ordinary.len()).expect("saturating-divide branch is small");
                    bytes.extend_from_slice(&done_offset.to_le_bytes());
                    bytes.extend_from_slice(&ordinary);
                }
            }
        }
    }
    Ok(())
}

fn emit_x86_64_normalize(bytes: &mut Vec<u8>, scalar_type: IntegerType) {
    match (scalar_type.sign(), scalar_type.bits()) {
        (_, 64) => {}
        (IntegerSign::Unsigned, 8) => bytes.extend_from_slice(&[0x25, 0xff, 0, 0, 0]),
        (IntegerSign::Unsigned, 16) => bytes.extend_from_slice(&[0x25, 0xff, 0xff, 0, 0]),
        (IntegerSign::Unsigned, 32) => bytes.extend_from_slice(&[0x89, 0xc0]),
        (IntegerSign::Signed, 8) => bytes.extend_from_slice(&[0x48, 0x0f, 0xbe, 0xc0]),
        (IntegerSign::Signed, 16) => bytes.extend_from_slice(&[0x48, 0x0f, 0xbf, 0xc0]),
        (IntegerSign::Signed, 32) => bytes.extend_from_slice(&[0x48, 0x63, 0xc0]),
        _ => unreachable!("native integer width was checked before expression emission"),
    }
}

fn emit_x86_64_saturating_add(bytes: &mut Vec<u8>, scalar_type: IntegerType) {
    let (minimum, maximum) = native_integer_bounds(scalar_type);
    match (scalar_type.sign(), scalar_type.bits()) {
        (IntegerSign::Unsigned, 64) => {
            bytes.extend_from_slice(&[0x4c, 0x01, 0xd0]); // add rax, r10
            bytes.extend_from_slice(&[0x4d, 0x19, 0xd2]); // sbb r10, r10
            bytes.extend_from_slice(&[0x4c, 0x09, 0xd0]); // or rax, r10
        }
        (IntegerSign::Unsigned, _) => {
            bytes.extend_from_slice(&[0x4c, 0x01, 0xd0]); // add rax, r10
            emit_x86_64_mov_r10(bytes, maximum);
            bytes.extend_from_slice(&[0x4c, 0x39, 0xd0]); // cmp rax, r10
            bytes.extend_from_slice(&[0x49, 0x0f, 0x47, 0xc2]); // cmova rax, r10
        }
        (IntegerSign::Signed, 64) => {
            bytes.extend_from_slice(&[0x4d, 0x89, 0xd3]); // mov r11, r10
            bytes.extend_from_slice(&[0x49, 0xc1, 0xfb, 0x3f]); // sar r11, 63
            bytes.extend_from_slice(&[0x49, 0xf7, 0xd3]); // not r11
            bytes.extend_from_slice(&[0x49, 0x0f, 0xba, 0xfb, 0x3f]); // btc r11, 63
            bytes.extend_from_slice(&[0x4c, 0x01, 0xd0]); // add rax, r10
            bytes.extend_from_slice(&[0x49, 0x0f, 0x40, 0xc3]); // cmovo rax, r11
        }
        (IntegerSign::Signed, _) => {
            bytes.extend_from_slice(&[0x4c, 0x01, 0xd0]); // add rax, r10
            emit_x86_64_mov_r10(bytes, maximum);
            bytes.extend_from_slice(&[0x4c, 0x39, 0xd0]); // cmp rax, r10
            bytes.extend_from_slice(&[0x49, 0x0f, 0x4f, 0xc2]); // cmovg rax, r10
            emit_x86_64_mov_r10(bytes, minimum);
            bytes.extend_from_slice(&[0x4c, 0x39, 0xd0]); // cmp rax, r10
            bytes.extend_from_slice(&[0x49, 0x0f, 0x4c, 0xc2]); // cmovl rax, r10
        }
    }
}

fn emit_x86_64_saturating_subtract(bytes: &mut Vec<u8>, scalar_type: IntegerType) {
    let (minimum, maximum) = native_integer_bounds(scalar_type);
    match (scalar_type.sign(), scalar_type.bits()) {
        (IntegerSign::Unsigned, _) => {
            bytes.extend_from_slice(&[0x49, 0x29, 0xc2]); // sub r10, rax
            bytes.extend_from_slice(&[0xb8, 0, 0, 0, 0]); // mov eax, 0 (flags unchanged)
            bytes.extend_from_slice(&[0x49, 0x0f, 0x43, 0xc2]); // cmovae rax, r10
        }
        (IntegerSign::Signed, 64) => {
            bytes.extend_from_slice(&[0x4d, 0x89, 0xd3]); // mov r11, r10
            bytes.extend_from_slice(&[0x49, 0xc1, 0xfb, 0x3f]); // sar r11, 63
            bytes.extend_from_slice(&[0x49, 0xf7, 0xd3]); // not r11
            bytes.extend_from_slice(&[0x49, 0x0f, 0xba, 0xfb, 0x3f]); // btc r11, 63
            bytes.extend_from_slice(&[0x49, 0x29, 0xc2]); // sub r10, rax
            bytes.extend_from_slice(&[0x4c, 0x89, 0xd0]); // mov rax, r10
            bytes.extend_from_slice(&[0x49, 0x0f, 0x40, 0xc3]); // cmovo rax, r11
        }
        (IntegerSign::Signed, _) => {
            bytes.extend_from_slice(&[0x49, 0x29, 0xc2]); // sub r10, rax
            bytes.extend_from_slice(&[0x4c, 0x89, 0xd0]); // mov rax, r10
            emit_x86_64_mov_r10(bytes, maximum);
            bytes.extend_from_slice(&[0x4c, 0x39, 0xd0]); // cmp rax, r10
            bytes.extend_from_slice(&[0x49, 0x0f, 0x4f, 0xc2]); // cmovg rax, r10
            emit_x86_64_mov_r10(bytes, minimum);
            bytes.extend_from_slice(&[0x4c, 0x39, 0xd0]); // cmp rax, r10
            bytes.extend_from_slice(&[0x49, 0x0f, 0x4c, 0xc2]); // cmovl rax, r10
        }
    }
}

fn emit_x86_64_saturating_multiply(bytes: &mut Vec<u8>, scalar_type: IntegerType) {
    let (minimum, maximum) = native_integer_bounds(scalar_type);
    match (scalar_type.sign(), scalar_type.bits()) {
        (IntegerSign::Unsigned, 64) => {
            bytes.push(0x52); // push rdx
            bytes.extend_from_slice(&[0x49, 0xf7, 0xe2]); // mul r10 -> rdx:rax
            bytes.extend_from_slice(&[0x48, 0x85, 0xd2]); // test rdx, rdx
            bytes.extend_from_slice(&[0x49, 0xbb]); // mov r11, u64::MAX
            bytes.extend_from_slice(&maximum.to_le_bytes());
            bytes.extend_from_slice(&[0x49, 0x0f, 0x45, 0xc3]); // cmovne rax, r11
            bytes.push(0x5a); // pop rdx
        }
        (IntegerSign::Unsigned, _) => {
            bytes.extend_from_slice(&[0x49, 0x0f, 0xaf, 0xc2]); // imul rax, r10
            emit_x86_64_mov_r10(bytes, maximum);
            bytes.extend_from_slice(&[0x4c, 0x39, 0xd0]); // cmp rax, r10
            bytes.extend_from_slice(&[0x49, 0x0f, 0x47, 0xc2]); // cmova rax, r10
        }
        (IntegerSign::Signed, 64) => {
            bytes.push(0x52); // push rdx
            bytes.extend_from_slice(&[0x41, 0x52]); // push r10
            bytes.extend_from_slice(&[0x49, 0xbb]); // mov r11, maximum
            bytes.extend_from_slice(&maximum.to_le_bytes());
            bytes.extend_from_slice(&[0x48, 0x89, 0xc2]); // mov rdx, rax
            bytes.extend_from_slice(&[0x4c, 0x31, 0xd2]); // xor rdx, r10
            emit_x86_64_mov_r10(bytes, minimum);
            bytes.extend_from_slice(&[0x4d, 0x0f, 0x48, 0xda]); // cmovs r11, r10
            bytes.extend_from_slice(&[0x41, 0x5a]); // pop r10
            bytes.extend_from_slice(&[0x49, 0xf7, 0xea]); // imul r10 -> rdx:rax
            bytes.extend_from_slice(&[0x49, 0x0f, 0x40, 0xc3]); // cmovo rax, r11
            bytes.push(0x5a); // pop rdx
        }
        (IntegerSign::Signed, _) => {
            bytes.extend_from_slice(&[0x49, 0x0f, 0xaf, 0xc2]); // imul rax, r10
            emit_x86_64_mov_r10(bytes, maximum);
            bytes.extend_from_slice(&[0x4c, 0x39, 0xd0]); // cmp rax, r10
            bytes.extend_from_slice(&[0x49, 0x0f, 0x4f, 0xc2]); // cmovg rax, r10
            emit_x86_64_mov_r10(bytes, minimum);
            bytes.extend_from_slice(&[0x4c, 0x39, 0xd0]); // cmp rax, r10
            bytes.extend_from_slice(&[0x49, 0x0f, 0x4c, 0xc2]); // cmovl rax, r10
        }
    }
    emit_x86_64_normalize(bytes, scalar_type);
}

fn emit_x86_64_mov_r10(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&[0x49, 0xba]);
    bytes.extend_from_slice(&value.to_le_bytes());
}

pub(crate) fn x86_preserving_release_immediate(
    bytes: &[u8],
    offset: usize,
    byte_count: usize,
) -> Result<u32, EmissionError> {
    let instruction = bytes
        .get(offset..offset.saturating_add(byte_count))
        .ok_or(EmissionError::ScalarStackInstructionEncodingInvalid)?;
    match instruction {
        [0x48, 0x8d, 0x64, 0x24, immediate] if *immediate != 0 && *immediate <= i8::MAX as u8 => {
            Ok(u32::from(*immediate))
        }
        [0x48, 0x8d, 0xa4, 0x24, immediate @ ..] if immediate.len() == 4 => {
            let byte_size = u32::from_le_bytes(
                immediate
                    .try_into()
                    .map_err(|_| EmissionError::ScalarStackInstructionEncodingInvalid)?,
            );
            (byte_size > i8::MAX as u32)
                .then_some(byte_size)
                .ok_or(EmissionError::ScalarStackInstructionEncodingInvalid)
        }
        _ => Err(EmissionError::ScalarStackInstructionEncodingInvalid),
    }
}

pub(crate) fn x86_adjustment_immediate(
    bytes: &[u8],
    offset: usize,
    byte_count: usize,
) -> Result<u32, EmissionError> {
    match byte_count {
        4 => bytes
            .get(offset + 3)
            .copied()
            .map(u32::from)
            .ok_or(EmissionError::ScalarStackInstructionEncodingInvalid),
        7 => bytes
            .get(offset + 3..offset + 7)
            .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
            .map(u32::from_le_bytes)
            .ok_or(EmissionError::ScalarStackInstructionEncodingInvalid),
        _ => Err(EmissionError::ScalarStackInstructionEncodingInvalid),
    }
}
