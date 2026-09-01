use super::super::placement::{require_register_architecture, x86_expression_scratch_conflict};
use super::super::shared::*;
use super::boolean::assign_boolean_expression;
use super::integer::assign_expression;
use super::parameters::{
    boolean_expression_parameter_locations, expression_parameter_locations,
    merge_expression_locations,
};

fn assign_expression_locations(
    architecture: Architecture,
    locations: &BTreeMap<usize, (ValueId, ScalarParameterLocation)>,
    force_register_spills: bool,
) -> Result<(ExpressionFrame, BTreeMap<usize, AssignedScalarLocation>), AssignmentError> {
    let mut register_spills = Vec::new();
    let mut assigned = BTreeMap::new();
    for (&parameter_index, &(source_value, location)) in locations {
        match location {
            ScalarParameterLocation::Register(register) => {
                require_register_architecture(source_value, register, architecture)?;
                if architecture == Architecture::X86_64 && register == MachineRegister::X86Rsp {
                    return Err(AssignmentError::ExpressionRegisterCannotHoldParameter {
                        value: source_value,
                        register,
                    });
                }
                if force_register_spills
                    || architecture == Architecture::Aarch64
                    || x86_expression_scratch_conflict(register)
                {
                    let byte_offset = u32::try_from(register_spills.len())
                        .ok()
                        .and_then(|count| count.checked_mul(8))
                        .ok_or(AssignmentError::ExpressionStackFrameNotEncodable)?;
                    register_spills.push(EntryRegisterSpill {
                        source_value,
                        parameter_index,
                        register,
                        byte_offset,
                    });
                    assigned.insert(
                        parameter_index,
                        AssignedScalarLocation::FrameSpill { byte_offset },
                    );
                } else {
                    assigned.insert(parameter_index, AssignedScalarLocation::Register(register));
                }
            }
            ScalarParameterLocation::IncomingStack { byte_offset } => {
                assigned.insert(
                    parameter_index,
                    AssignedScalarLocation::IncomingStack { byte_offset },
                );
            }
        }
    }
    let used_bytes = u32::try_from(register_spills.len())
        .ok()
        .and_then(|count| count.checked_mul(8))
        .ok_or(AssignmentError::ExpressionStackFrameNotEncodable)?;
    let byte_size = used_bytes
        .checked_add(15)
        .map(|bytes| bytes & !15)
        .ok_or(AssignmentError::ExpressionStackFrameNotEncodable)?;
    if byte_size > 0xfff {
        return Err(AssignmentError::ExpressionStackFrameNotEncodable);
    }
    Ok((
        ExpressionFrame {
            byte_size,
            register_spills,
        },
        assigned,
    ))
}

pub(crate) fn assign_integer_expression_frame(
    expression: &TargetIntegerExpression,
    architecture: Architecture,
) -> Result<(ExpressionFrame, AssignedIntegerExpression), AssignmentError> {
    let locations = expression_parameter_locations(expression)?;
    let (mut frame, assigned_locations) = assign_expression_locations(
        architecture,
        &locations,
        integer_expression_contains_call(expression),
    )?;
    let mut next_spill = frame.byte_size;
    let expression = assign_expression(
        expression,
        &assigned_locations,
        architecture,
        &mut next_spill,
    )?;
    frame.byte_size = aligned_frame_size(next_spill)?;
    Ok((frame, expression))
}

pub(crate) fn assign_boolean_expression_frame(
    expression: &TargetBooleanExpression,
    architecture: Architecture,
) -> Result<(ExpressionFrame, AssignedBooleanExpression), AssignmentError> {
    assign_boolean_expression_frame_preserving(expression, architecture, BTreeMap::new())
}

pub(crate) fn assign_boolean_expression_frame_preserving(
    expression: &TargetBooleanExpression,
    architecture: Architecture,
    preserved: BTreeMap<usize, (ValueId, ScalarParameterLocation)>,
) -> Result<(ExpressionFrame, AssignedBooleanExpression), AssignmentError> {
    let mut locations = boolean_expression_parameter_locations(expression)?;
    merge_expression_locations(&mut locations, preserved)?;
    let (mut frame, assigned_locations) = assign_expression_locations(
        architecture,
        &locations,
        boolean_expression_contains_call(expression),
    )?;
    let mut next_spill = frame.byte_size;
    let expression = assign_boolean_expression(
        expression,
        &assigned_locations,
        architecture,
        &mut next_spill,
    )?;
    frame.byte_size = aligned_frame_size(next_spill)?;
    Ok((frame, expression))
}

fn aligned_frame_size(used_bytes: u32) -> Result<u32, AssignmentError> {
    let byte_size = used_bytes
        .checked_add(15)
        .map(|bytes| bytes & !15)
        .ok_or(AssignmentError::ExpressionStackFrameNotEncodable)?;
    if byte_size > 0xfff {
        return Err(AssignmentError::ExpressionStackFrameNotEncodable);
    }
    Ok(byte_size)
}

fn integer_expression_contains_call(expression: &TargetIntegerExpression) -> bool {
    match expression {
        TargetIntegerExpression::Call { .. } => true,
        TargetIntegerExpression::Immediate { .. }
        | TargetIntegerExpression::Parameter { .. }
        | TargetIntegerExpression::StructuralField { .. } => false,
        TargetIntegerExpression::BitwiseNot { operand, .. }
        | TargetIntegerExpression::IntegerWiden { operand, .. }
        | TargetIntegerExpression::IntegerExactCast { operand, .. } => {
            integer_expression_contains_call(operand)
        }
        TargetIntegerExpression::WrappingAdd { left, right, .. }
        | TargetIntegerExpression::ExactAdd { left, right, .. }
        | TargetIntegerExpression::BitwiseAnd { left, right, .. }
        | TargetIntegerExpression::BitwiseOr { left, right, .. }
        | TargetIntegerExpression::BitwiseXor { left, right, .. }
        | TargetIntegerExpression::WrappingShiftLeft {
            value: left,
            count: right,
            ..
        }
        | TargetIntegerExpression::WrappingShiftRight {
            value: left,
            count: right,
            ..
        }
        | TargetIntegerExpression::ExactShiftLeft {
            value: left,
            count: right,
            ..
        }
        | TargetIntegerExpression::ExactShiftRight {
            value: left,
            count: right,
            ..
        }
        | TargetIntegerExpression::SaturatingAdd { left, right, .. }
        | TargetIntegerExpression::WrappingSubtract { left, right, .. }
        | TargetIntegerExpression::ExactSubtract { left, right, .. }
        | TargetIntegerExpression::SaturatingSubtract { left, right, .. }
        | TargetIntegerExpression::WrappingMultiply { left, right, .. }
        | TargetIntegerExpression::ExactMultiply { left, right, .. }
        | TargetIntegerExpression::SaturatingMultiply { left, right, .. }
        | TargetIntegerExpression::ExactDivide { left, right, .. }
        | TargetIntegerExpression::ExactRemainder { left, right, .. }
        | TargetIntegerExpression::WrappingDivide { left, right, .. }
        | TargetIntegerExpression::WrappingRemainder { left, right, .. }
        | TargetIntegerExpression::SaturatingDivide { left, right, .. }
        | TargetIntegerExpression::SaturatingRemainder { left, right, .. } => {
            integer_expression_contains_call(left) || integer_expression_contains_call(right)
        }
    }
}

fn boolean_expression_contains_call(expression: &TargetBooleanExpression) -> bool {
    match expression {
        TargetBooleanExpression::Call { .. } => true,
        TargetBooleanExpression::Immediate { .. }
        | TargetBooleanExpression::Parameter { .. }
        | TargetBooleanExpression::StructuralField { .. } => false,
        TargetBooleanExpression::Not { operand, .. } => boolean_expression_contains_call(operand),
        TargetBooleanExpression::Equal { left, right, .. } => {
            boolean_expression_contains_call(left) || boolean_expression_contains_call(right)
        }
        TargetBooleanExpression::IntegerEqual { left, right, .. }
        | TargetBooleanExpression::IntegerLessThan { left, right, .. }
        | TargetBooleanExpression::IntegerLessOrEqual { left, right, .. } => {
            integer_expression_contains_call(left) || integer_expression_contains_call(right)
        }
    }
}

pub(super) fn assign_call_arguments(
    arguments: &[TargetCallArgument],
    locations: &BTreeMap<usize, AssignedScalarLocation>,
    architecture: Architecture,
    next_spill: &mut u32,
) -> Result<Vec<AssignedCallArgument>, AssignmentError> {
    arguments
        .iter()
        .map(|argument| {
            let expression = match &argument.expression {
                TargetScalarExpression::Boolean(expression) => AssignedScalarExpression::Boolean(
                    assign_boolean_expression(expression, locations, architecture, next_spill)?,
                ),
                TargetScalarExpression::Integer {
                    scalar_type,
                    expression,
                } => AssignedScalarExpression::Integer {
                    scalar_type: *scalar_type,
                    expression: assign_expression(expression, locations, architecture, next_spill)?,
                },
            };
            let destination = match argument.location {
                ScalarParameterLocation::Register(register) => {
                    let valid = match architecture {
                        Architecture::Aarch64 => {
                            matches!(register, MachineRegister::Aarch64X(0..=30))
                        }
                        Architecture::X86_64 => matches!(
                            register,
                            MachineRegister::X86Rax
                                | MachineRegister::X86Rcx
                                | MachineRegister::X86Rdx
                                | MachineRegister::X86Rbx
                                | MachineRegister::X86Rsp
                                | MachineRegister::X86Rbp
                                | MachineRegister::X86Rsi
                                | MachineRegister::X86Rdi
                                | MachineRegister::X86R8
                                | MachineRegister::X86R9
                                | MachineRegister::X86R10
                                | MachineRegister::X86R11
                                | MachineRegister::X86R12
                                | MachineRegister::X86R13
                                | MachineRegister::X86R14
                                | MachineRegister::X86R15
                        ),
                    };
                    if !valid || register == MachineRegister::X86Rsp {
                        return Err(AssignmentError::UnsupportedCallArgumentRegister(register));
                    }
                    AssignedCallDestination::Register(register)
                }
                ScalarParameterLocation::IncomingStack { byte_offset } => {
                    AssignedCallDestination::OutgoingStack { byte_offset }
                }
            };
            let spill_byte_offset = *next_spill;
            *next_spill = next_spill
                .checked_add(8)
                .ok_or(AssignmentError::ExpressionStackFrameNotEncodable)?;
            Ok(AssignedCallArgument {
                scalar_type: argument.scalar_type,
                destination,
                spill_byte_offset,
                expression,
            })
        })
        .collect()
}
