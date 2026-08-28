use omega_assigned_target_operations::{
    AssignedBooleanControl, AssignedBooleanExpression, AssignedCallArgument,
    AssignedCallDestination, AssignedConditionalBooleanArm, AssignedConditionalIntegerArm,
    AssignedIntegerControl, AssignedIntegerExpression, AssignedScalarExpression,
};
use omega_machine_code::{
    BooleanStructuralConditionEvidence, InternalCallRelocation, ScalarCleanupPreservationEvidence,
    ScalarConditionalBranchEvidence, ScalarConditionalCondition, ScalarControlFlowEvidence,
    ScalarDivisionBranchEvidence, ScalarJoinBranchEvidence, ScalarStackEvidence,
    ScalarStackMutation, ScalarStackMutationKind,
};
use omega_target::Architecture;
use psi_core::{IntegerSign, IntegerType, IntegerValue, ValueId};

use super::aarch64::{
    emit_aarch64_boolean_condition_value, emit_aarch64_boolean_return, emit_aarch64_condition_load,
};
use super::x86_64::{
    emit_x86_64_boolean_condition_value, emit_x86_64_boolean_return, emit_x86_64_parameter_return,
    x86_adjustment_immediate, x86_preserving_release_immediate,
};
use crate::EmissionError;

pub(crate) fn emit_native_crash(architecture: Architecture) -> Vec<u8> {
    match architecture {
        Architecture::Aarch64 => vec![0x00, 0x00, 0x20, 0xd4], // brk #0
        Architecture::X86_64 => vec![0x0f, 0x0b],              // ud2
    }
}

pub(crate) struct EmissionFragment {
    pub(crate) bytes: Vec<u8>,
    pub(crate) internal_calls: Vec<InternalCallRelocation>,
    pub(crate) conditional: Option<Vec<ScalarConditionalBranchEvidence>>,
}

impl EmissionFragment {
    pub(crate) fn without_calls(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            internal_calls: Vec::new(),
            conditional: None,
        }
    }

    pub(crate) fn append(&mut self, mut fragment: Self) -> Result<(), EmissionError> {
        let base = self.bytes.len();
        for relocation in &mut fragment.internal_calls {
            relocation.offset = relocation
                .offset
                .checked_add(base)
                .ok_or(EmissionError::InternalCallRelocationOffsetNotEncodable)?;
            if let Some(stack) = &mut relocation.scalar_stack {
                if let Some(outbound) = &mut stack.outbound {
                    outbound.allocation_offset = outbound
                        .allocation_offset
                        .checked_add(base)
                        .ok_or(EmissionError::InternalCallRelocationOffsetNotEncodable)?;
                    outbound.release_offset = outbound
                        .release_offset
                        .checked_add(base)
                        .ok_or(EmissionError::InternalCallRelocationOffsetNotEncodable)?;
                }
                if let Some(link) = &mut stack.aarch64_return_link {
                    link.store_offset = link
                        .store_offset
                        .checked_add(base)
                        .ok_or(EmissionError::InternalCallRelocationOffsetNotEncodable)?;
                    link.load_offset = link
                        .load_offset
                        .checked_add(base)
                        .ok_or(EmissionError::InternalCallRelocationOffsetNotEncodable)?;
                }
            }
        }
        self.bytes.append(&mut fragment.bytes);
        self.internal_calls.append(&mut fragment.internal_calls);
        Ok(())
    }
}

pub(crate) fn top_level_integer_conditional_evidence(
    condition: ScalarConditionalCondition,
    branch_offset: usize,
    branch_byte_count: usize,
    false_arm_offset: usize,
    true_conditional: Option<&[ScalarConditionalBranchEvidence]>,
    false_conditional: Option<&[ScalarConditionalBranchEvidence]>,
) -> Result<Vec<ScalarConditionalBranchEvidence>, EmissionError> {
    let root = ScalarConditionalBranchEvidence {
        condition,
        branch_offset,
        branch_byte_count,
        false_arm_offset,
    };
    let shifted = |evidence: &[ScalarConditionalBranchEvidence], base: usize| {
        evidence
            .iter()
            .map(|branch| {
                Ok(ScalarConditionalBranchEvidence {
                    condition: branch.condition,
                    branch_offset: branch
                        .branch_offset
                        .checked_add(base)
                        .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?,
                    branch_byte_count: branch.branch_byte_count,
                    false_arm_offset: branch
                        .false_arm_offset
                        .checked_add(base)
                        .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?,
                })
            })
            .collect::<Result<Vec<_>, _>>()
    };
    let true_arm_offset = branch_offset
        .checked_add(branch_byte_count)
        .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
    let mut decisions = vec![root];
    if let Some(true_conditional) = true_conditional {
        decisions.extend(shifted(true_conditional, true_arm_offset)?);
    }
    if let Some(false_conditional) = false_conditional {
        decisions.extend(shifted(false_conditional, false_arm_offset)?);
    }
    Ok(decisions)
}

pub(crate) fn emit_boolean_shared_convergence(
    architecture: Architecture,
    control: &AssignedBooleanControl,
) -> Result<(Vec<u8>, ScalarControlFlowEvidence), EmissionError> {
    let mut emitted = emit_boolean_shared_convergence_tree(architecture, control)?;
    if emitted.decisions.is_empty()
        || emitted.joins.len() != emitted.decisions.len().checked_add(1).unwrap_or(0)
    {
        return Err(EmissionError::ConditionalBranchEncodingInvalid);
    }
    let fallthrough = emitted
        .joins
        .pop()
        .ok_or(EmissionError::ConditionalBranchEncodingInvalid)?;
    if fallthrough.join_offset + fallthrough.join_byte_count != emitted.bytes.len() {
        return Err(EmissionError::ConditionalBranchEncodingInvalid);
    }
    emitted.bytes.truncate(fallthrough.join_offset);
    let merge_offset = emitted.bytes.len();
    for join in &emitted.joins {
        let join_end = join
            .join_offset
            .checked_add(join.join_byte_count)
            .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
        match architecture {
            Architecture::X86_64 => {
                let displacement = merge_offset
                    .checked_sub(join_end)
                    .and_then(|distance| i32::try_from(distance).ok())
                    .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
                emitted.bytes[join.join_offset] = 0xe9;
                emitted.bytes[join.join_offset + 1..join_end]
                    .copy_from_slice(&displacement.to_le_bytes());
            }
            Architecture::Aarch64 => {
                let words = merge_offset
                    .checked_sub(join.join_offset)
                    .filter(|distance| distance.is_multiple_of(4))
                    .map(|distance| distance / 4)
                    .and_then(|words| u32::try_from(words).ok())
                    .filter(|words| *words <= 0x01ff_ffff)
                    .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
                emitted.bytes[join.join_offset..join_end]
                    .copy_from_slice(&(0x1400_0000 | words).to_le_bytes());
            }
        }
    }
    match architecture {
        Architecture::X86_64 => emitted.bytes.push(0xc3),
        Architecture::Aarch64 => emitted
            .bytes
            .extend_from_slice(&0xd65f_03c0_u32.to_le_bytes()),
    }
    Ok((
        emitted.bytes,
        ScalarControlFlowEvidence::BooleanSharedConvergence {
            decisions: emitted.decisions,
            joins: emitted.joins,
            structural_conditions: emitted.structural_conditions,
            merge_offset,
        },
    ))
}

struct BooleanSharedConvergenceEmission {
    bytes: Vec<u8>,
    decisions: Vec<ScalarConditionalBranchEvidence>,
    joins: Vec<ScalarJoinBranchEvidence>,
    structural_conditions: Vec<BooleanStructuralConditionEvidence>,
}

impl BooleanSharedConvergenceEmission {
    fn append(&mut self, mut child: Self) -> Result<(), EmissionError> {
        let base = self.bytes.len();
        for decision in &mut child.decisions {
            decision.branch_offset = decision
                .branch_offset
                .checked_add(base)
                .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
            decision.false_arm_offset = decision
                .false_arm_offset
                .checked_add(base)
                .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
        }
        for join in &mut child.joins {
            join.join_offset = join
                .join_offset
                .checked_add(base)
                .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
        }
        for condition in &mut child.structural_conditions {
            condition.code_offset = condition
                .code_offset
                .checked_add(base)
                .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
            for read in &mut condition.reads {
                read.code_offset = read
                    .code_offset
                    .checked_add(base)
                    .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
            }
        }
        self.bytes.append(&mut child.bytes);
        self.decisions.append(&mut child.decisions);
        self.joins.append(&mut child.joins);
        self.structural_conditions
            .append(&mut child.structural_conditions);
        Ok(())
    }
}

fn emit_boolean_shared_convergence_tree(
    architecture: Architecture,
    control: &AssignedBooleanControl,
) -> Result<BooleanSharedConvergenceEmission, EmissionError> {
    let (
        mut prefix,
        condition,
        aarch64_condition_register,
        when_true,
        when_false,
        structural_reads,
    ) = match control {
        AssignedBooleanControl::Conditional {
            condition_source,
            condition_location,
            when_true,
            when_false,
            ..
        } => match architecture {
            Architecture::X86_64 => {
                let mut bytes =
                    emit_x86_64_parameter_return(*condition_source, false, *condition_location)?;
                if bytes.pop() != Some(0xc3) {
                    return Err(EmissionError::ConditionalBranchEncodingInvalid);
                }
                bytes.extend_from_slice(&[0x85, 0xc0]);
                (
                    bytes,
                    ScalarConditionalCondition::Parameter,
                    None,
                    when_true,
                    when_false,
                    Vec::new(),
                )
            }
            Architecture::Aarch64 => {
                let (bytes, register) =
                    emit_aarch64_condition_load(*condition_source, *condition_location)?;
                (
                    bytes,
                    ScalarConditionalCondition::Parameter,
                    Some(register),
                    when_true,
                    when_false,
                    Vec::new(),
                )
            }
        },
        AssignedBooleanControl::ConditionalExpression {
            condition_frame,
            condition,
            when_true,
            when_false,
            ..
        } if linear_boolean_expression(condition) => {
            let mut structural_reads = Vec::new();
            let bytes = match architecture {
                Architecture::X86_64 => emit_x86_64_boolean_condition_value(
                    condition_frame,
                    condition,
                    None,
                    Some(&mut structural_reads),
                )?,
                Architecture::Aarch64 => emit_aarch64_boolean_condition_value(
                    condition_frame,
                    condition,
                    None,
                    Some(&mut structural_reads),
                )?,
            };
            (
                bytes,
                ScalarConditionalCondition::Expression,
                None,
                when_true,
                when_false,
                structural_reads,
            )
        }
        AssignedBooleanControl::ReturnImmediate { value, .. } => {
            let mut bytes = match architecture {
                Architecture::X86_64 => emit_x86_64_boolean_return(*value),
                Architecture::Aarch64 => emit_aarch64_boolean_return(*value),
            };
            match architecture {
                Architecture::X86_64 if bytes.pop() == Some(0xc3) => {}
                Architecture::Aarch64
                    if bytes.len() >= 4
                        && bytes.split_off(bytes.len() - 4) == 0xd65f_03c0_u32.to_le_bytes() => {}
                _ => return Err(EmissionError::ConditionalBranchEncodingInvalid),
            }
            let join_offset = bytes.len();
            let join_byte_count = match architecture {
                Architecture::X86_64 => {
                    bytes.extend_from_slice(&[0xe9, 0, 0, 0, 0]);
                    5
                }
                Architecture::Aarch64 => {
                    bytes.extend_from_slice(&0x1400_0000_u32.to_le_bytes());
                    4
                }
            };
            return Ok(BooleanSharedConvergenceEmission {
                bytes,
                decisions: Vec::new(),
                joins: vec![ScalarJoinBranchEvidence {
                    join_offset,
                    join_byte_count,
                }],
                structural_conditions: Vec::new(),
            });
        }
        _ => return Err(EmissionError::UnsupportedScalarCleanup),
    };
    let when_true = emit_boolean_shared_convergence_tree(architecture, &when_true.control)?;
    let when_false = emit_boolean_shared_convergence_tree(architecture, &when_false.control)?;
    let branch_offset = prefix.len();
    let branch_byte_count = match architecture {
        Architecture::X86_64 => {
            let displacement = i32::try_from(when_true.bytes.len())
                .map_err(|_| EmissionError::ConditionalBranchDistanceNotEncodable)?;
            prefix.extend_from_slice(&[0x0f, 0x84]);
            prefix.extend_from_slice(&displacement.to_le_bytes());
            6
        }
        Architecture::Aarch64 => {
            let branch_words = when_true
                .bytes
                .len()
                .checked_div(4)
                .and_then(|words| words.checked_add(1))
                .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
            if branch_words > 0x3ffff {
                return Err(EmissionError::ConditionalBranchDistanceNotEncodable);
            }
            let branch = match (condition, aarch64_condition_register) {
                (ScalarConditionalCondition::Parameter, Some(register)) => {
                    0x3400_0000_u32 | ((branch_words as u32) << 5) | u32::from(register)
                }
                (ScalarConditionalCondition::Expression, None) => {
                    0x5400_0000_u32 | ((branch_words as u32) << 5)
                }
                _ => return Err(EmissionError::ConditionalBranchEncodingInvalid),
            };
            prefix.extend_from_slice(&branch.to_le_bytes());
            4
        }
    };
    let false_arm_offset = prefix
        .len()
        .checked_add(when_true.bytes.len())
        .ok_or(EmissionError::ConditionalBranchDistanceNotEncodable)?;
    let structural_conditions = if structural_reads.is_empty() {
        Vec::new()
    } else {
        vec![BooleanStructuralConditionEvidence {
            reads: structural_reads,
            code_offset: 0,
            byte_count: branch_offset,
            bytes: prefix[..branch_offset].to_vec(),
        }]
    };
    let mut emitted = BooleanSharedConvergenceEmission {
        bytes: prefix,
        decisions: vec![ScalarConditionalBranchEvidence {
            condition,
            branch_offset,
            branch_byte_count,
            false_arm_offset,
        }],
        joins: Vec::new(),
        structural_conditions,
    };
    emitted.append(when_true)?;
    emitted.append(when_false)?;
    Ok(emitted)
}

pub(crate) fn integer_bits(
    source: ValueId,
    scalar_type: IntegerType,
    value: IntegerValue,
) -> Result<u64, EmissionError> {
    let width = require_native_integer_width(source, scalar_type)?;
    if !scalar_type.admits(value) {
        return Err(EmissionError::IntegerOutsideType(source));
    }
    let mask = if width == 64 {
        u64::MAX
    } else {
        (1_u64 << width) - 1
    };
    let bits = match (scalar_type.sign(), value) {
        (IntegerSign::Signed, IntegerValue::Signed(value)) => value as u128 as u64,
        (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) => value as u64,
        _ => return Err(EmissionError::IntegerSignMismatch(source)),
    };
    Ok(bits & mask)
}

pub(crate) fn require_native_integer_width(
    source: ValueId,
    scalar_type: IntegerType,
) -> Result<u16, EmissionError> {
    let width = scalar_type.bits();
    if !matches!(width, 8 | 16 | 32 | 64) {
        return Err(EmissionError::IntegerWidthNotNativelySupported {
            value: source,
            bits: width,
        });
    }
    Ok(width)
}

pub(crate) fn outgoing_stack_bytes(
    source_value: ValueId,
    arguments: &[AssignedCallArgument],
) -> Result<u32, EmissionError> {
    arguments.iter().try_fold(0, |byte_size, argument| {
        let AssignedCallDestination::OutgoingStack { byte_offset } = argument.destination else {
            return Ok(byte_size);
        };
        let end = byte_offset
            .checked_add(8)
            .ok_or(EmissionError::CallStackAreaNotEncodable {
                value: source_value,
                byte_size: byte_offset,
            })?;
        Ok(byte_size.max(end))
    })
}

pub(crate) fn linear_boolean_expression(expression: &AssignedBooleanExpression) -> bool {
    match expression {
        AssignedBooleanExpression::Call { arguments, .. } => arguments
            .iter()
            .all(|argument| linear_scalar_expression(&argument.expression)),
        AssignedBooleanExpression::Immediate { .. }
        | AssignedBooleanExpression::Parameter { .. }
        | AssignedBooleanExpression::StructuralField { .. } => true,
        AssignedBooleanExpression::Not { operand, .. } => linear_boolean_expression(operand),
        AssignedBooleanExpression::Equal { left, right, .. } => {
            linear_boolean_expression(left) && linear_boolean_expression(right)
        }
        AssignedBooleanExpression::IntegerEqual { left, right, .. }
        | AssignedBooleanExpression::IntegerLessThan { left, right, .. }
        | AssignedBooleanExpression::IntegerLessOrEqual { left, right, .. } => {
            linear_integer_expression(left) && linear_integer_expression(right)
        }
    }
}

fn linear_integer_expression(expression: &AssignedIntegerExpression) -> bool {
    match expression {
        AssignedIntegerExpression::Call { arguments, .. } => arguments
            .iter()
            .all(|argument| linear_scalar_expression(&argument.expression)),
        AssignedIntegerExpression::WrappingDivide { .. }
        | AssignedIntegerExpression::WrappingRemainder { .. }
        | AssignedIntegerExpression::SaturatingDivide { .. }
        | AssignedIntegerExpression::SaturatingRemainder { .. } => false,
        AssignedIntegerExpression::Immediate { .. }
        | AssignedIntegerExpression::Parameter { .. } => true,
        AssignedIntegerExpression::BitwiseNot { operand, .. }
        | AssignedIntegerExpression::IntegerWiden { operand, .. }
        | AssignedIntegerExpression::IntegerExactCast { operand, .. } => {
            linear_integer_expression(operand)
        }
        AssignedIntegerExpression::WrappingAdd { left, right, .. }
        | AssignedIntegerExpression::ExactAdd { left, right, .. }
        | AssignedIntegerExpression::BitwiseAnd { left, right, .. }
        | AssignedIntegerExpression::BitwiseOr { left, right, .. }
        | AssignedIntegerExpression::BitwiseXor { left, right, .. }
        | AssignedIntegerExpression::WrappingShiftLeft {
            value: left,
            count: right,
            ..
        }
        | AssignedIntegerExpression::WrappingShiftRight {
            value: left,
            count: right,
            ..
        }
        | AssignedIntegerExpression::ExactShiftLeft {
            value: left,
            count: right,
            ..
        }
        | AssignedIntegerExpression::ExactShiftRight {
            value: left,
            count: right,
            ..
        }
        | AssignedIntegerExpression::SaturatingAdd { left, right, .. }
        | AssignedIntegerExpression::WrappingSubtract { left, right, .. }
        | AssignedIntegerExpression::ExactSubtract { left, right, .. }
        | AssignedIntegerExpression::SaturatingSubtract { left, right, .. }
        | AssignedIntegerExpression::WrappingMultiply { left, right, .. }
        | AssignedIntegerExpression::ExactMultiply { left, right, .. }
        | AssignedIntegerExpression::SaturatingMultiply { left, right, .. }
        | AssignedIntegerExpression::ExactDivide { left, right, .. }
        | AssignedIntegerExpression::ExactRemainder { left, right, .. } => {
            linear_integer_expression(left) && linear_integer_expression(right)
        }
    }
}

/// The direct-return WCSU lane additionally admits division and remainder.
/// Compiler-generated x86-64 control flow is retained separately from the
/// language-level conditional evidence. Typed call arguments use the same
/// accountable expression rail.
pub(crate) fn accountable_direct_integer_expression(
    expression: &AssignedIntegerExpression,
) -> bool {
    match expression {
        AssignedIntegerExpression::Call { arguments, .. } => arguments
            .iter()
            .all(|argument| accountable_call_argument_expression(&argument.expression)),
        AssignedIntegerExpression::Immediate { .. }
        | AssignedIntegerExpression::Parameter { .. } => true,
        AssignedIntegerExpression::BitwiseNot { operand, .. }
        | AssignedIntegerExpression::IntegerWiden { operand, .. }
        | AssignedIntegerExpression::IntegerExactCast { operand, .. } => {
            accountable_direct_integer_expression(operand)
        }
        AssignedIntegerExpression::WrappingAdd { left, right, .. }
        | AssignedIntegerExpression::ExactAdd { left, right, .. }
        | AssignedIntegerExpression::BitwiseAnd { left, right, .. }
        | AssignedIntegerExpression::BitwiseOr { left, right, .. }
        | AssignedIntegerExpression::BitwiseXor { left, right, .. }
        | AssignedIntegerExpression::WrappingShiftLeft {
            value: left,
            count: right,
            ..
        }
        | AssignedIntegerExpression::WrappingShiftRight {
            value: left,
            count: right,
            ..
        }
        | AssignedIntegerExpression::ExactShiftLeft {
            value: left,
            count: right,
            ..
        }
        | AssignedIntegerExpression::ExactShiftRight {
            value: left,
            count: right,
            ..
        }
        | AssignedIntegerExpression::SaturatingAdd { left, right, .. }
        | AssignedIntegerExpression::WrappingSubtract { left, right, .. }
        | AssignedIntegerExpression::ExactSubtract { left, right, .. }
        | AssignedIntegerExpression::SaturatingSubtract { left, right, .. }
        | AssignedIntegerExpression::WrappingMultiply { left, right, .. }
        | AssignedIntegerExpression::ExactMultiply { left, right, .. }
        | AssignedIntegerExpression::SaturatingMultiply { left, right, .. }
        | AssignedIntegerExpression::ExactDivide { left, right, .. }
        | AssignedIntegerExpression::ExactRemainder { left, right, .. }
        | AssignedIntegerExpression::WrappingDivide { left, right, .. }
        | AssignedIntegerExpression::WrappingRemainder { left, right, .. }
        | AssignedIntegerExpression::SaturatingDivide { left, right, .. }
        | AssignedIntegerExpression::SaturatingRemainder { left, right, .. } => {
            accountable_direct_integer_expression(left)
                && accountable_direct_integer_expression(right)
        }
    }
}

fn accountable_call_argument_expression(expression: &AssignedScalarExpression) -> bool {
    match expression {
        AssignedScalarExpression::Boolean(expression) => linear_boolean_expression(expression),
        AssignedScalarExpression::Integer { expression, .. } => {
            accountable_direct_integer_expression(expression)
        }
    }
}

pub(crate) fn collect_x86_division_branch_evidence(
    bytes: &[u8],
) -> Result<Vec<ScalarDivisionBranchEvidence>, EmissionError> {
    let mut decoder = iced_x86::Decoder::with_ip(64, bytes, 0, iced_x86::DecoderOptions::NONE);
    let mut instructions = Vec::new();
    while decoder.can_decode() {
        let instruction = decoder.decode();
        if instruction.is_invalid() {
            return Err(EmissionError::ScalarStackInstructionEncodingInvalid);
        }
        instructions.push(instruction);
    }
    let mut branches = Vec::new();
    for instruction in &instructions {
        if instruction.mnemonic() != iced_x86::Mnemonic::Jne {
            continue;
        }
        let branch_offset = usize::try_from(instruction.ip())
            .map_err(|_| EmissionError::ScalarStackInstructionEncodingInvalid)?;
        let ordinary_arm_offset = usize::try_from(instruction.near_branch_target())
            .map_err(|_| EmissionError::ScalarStackInstructionEncodingInvalid)?;
        let ordinary_index = instructions
            .iter()
            .position(|candidate| candidate.ip() == instruction.near_branch_target())
            .ok_or(EmissionError::ScalarStackInstructionEncodingInvalid)?;
        let join = ordinary_index
            .checked_sub(1)
            .and_then(|index| instructions.get(index))
            .ok_or(EmissionError::ScalarStackInstructionEncodingInvalid)?;
        if join.mnemonic() != iced_x86::Mnemonic::Jmp
            || join.next_ip() != instruction.near_branch_target()
        {
            return Err(EmissionError::ScalarStackInstructionEncodingInvalid);
        }
        let merge_offset = usize::try_from(join.near_branch_target())
            .map_err(|_| EmissionError::ScalarStackInstructionEncodingInvalid)?;
        branches.push(ScalarDivisionBranchEvidence {
            branch_offset,
            branch_byte_count: instruction.len(),
            ordinary_arm_offset,
            join_offset: usize::try_from(join.ip())
                .map_err(|_| EmissionError::ScalarStackInstructionEncodingInvalid)?,
            join_byte_count: join.len(),
            merge_offset,
        });
    }
    Ok(branches)
}

pub(crate) fn conditional_with_terminal_shape(
    decisions: Vec<ScalarConditionalBranchEvidence>,
    crash_leaves: Vec<bool>,
    branches: Vec<ScalarDivisionBranchEvidence>,
) -> Result<ScalarControlFlowEvidence, EmissionError> {
    if decisions.is_empty() || crash_leaves.len() != decisions.len() + 1 {
        return Err(EmissionError::ConditionalBranchEncodingInvalid);
    }
    Ok(ScalarControlFlowEvidence::ConditionalTree {
        decisions,
        crash_leaves,
        branches,
    })
}

fn linear_scalar_expression(expression: &AssignedScalarExpression) -> bool {
    match expression {
        AssignedScalarExpression::Boolean(expression) => linear_boolean_expression(expression),
        AssignedScalarExpression::Integer { expression, .. } => {
            linear_integer_expression(expression)
        }
    }
}

pub(crate) fn direct_conditional_boolean_shape(
    when_true: &AssignedConditionalBooleanArm,
    when_false: &AssignedConditionalBooleanArm,
) -> Option<Vec<bool>> {
    fn collect(control: &AssignedBooleanControl, crash_leaves: &mut Vec<bool>) -> Option<()> {
        match control {
            AssignedBooleanControl::ReturnImmediate { .. }
            | AssignedBooleanControl::ReturnParameter { .. }
            | AssignedBooleanControl::ReturnNotParameter { .. } => {
                crash_leaves.push(false);
                Some(())
            }
            AssignedBooleanControl::ReturnExpression { expression, .. }
                if accountable_conditional_boolean_expression(expression) =>
            {
                crash_leaves.push(false);
                Some(())
            }
            AssignedBooleanControl::Crash { .. } => {
                crash_leaves.push(true);
                Some(())
            }
            AssignedBooleanControl::Conditional {
                when_true,
                when_false,
                ..
            } => {
                collect(&when_true.control, crash_leaves)?;
                collect(&when_false.control, crash_leaves)
            }
            AssignedBooleanControl::ConditionalExpression {
                condition,
                when_true,
                when_false,
                ..
            } if accountable_conditional_boolean_expression(condition) => {
                collect(&when_true.control, crash_leaves)?;
                collect(&when_false.control, crash_leaves)
            }
            _ => None,
        }
    }

    let mut crash_leaves = Vec::new();
    collect(&when_true.control, &mut crash_leaves)?;
    collect(&when_false.control, &mut crash_leaves)?;
    Some(crash_leaves)
}

pub(crate) fn direct_conditional_integer_shape(
    when_true: &AssignedConditionalIntegerArm,
    when_false: &AssignedConditionalIntegerArm,
) -> Option<Vec<bool>> {
    fn collect(control: &AssignedIntegerControl, crash_leaves: &mut Vec<bool>) -> Option<()> {
        match control {
            AssignedIntegerControl::Return { expression, .. }
                if accountable_conditional_arm_integer_expression(expression) =>
            {
                crash_leaves.push(false);
                Some(())
            }
            AssignedIntegerControl::Crash { .. } => {
                crash_leaves.push(true);
                Some(())
            }
            AssignedIntegerControl::Conditional {
                when_true,
                when_false,
                ..
            } => {
                collect(&when_true.control, crash_leaves)?;
                collect(&when_false.control, crash_leaves)
            }
            AssignedIntegerControl::ConditionalExpression {
                condition,
                when_true,
                when_false,
                ..
            } if accountable_conditional_boolean_expression(condition) => {
                collect(&when_true.control, crash_leaves)?;
                collect(&when_false.control, crash_leaves)
            }
            _ => None,
        }
    }

    let mut crash_leaves = Vec::new();
    collect(&when_true.control, &mut crash_leaves)?;
    collect(&when_false.control, &mut crash_leaves)?;
    Some(crash_leaves)
}

/// Expression-condition WCSU evidence admits division and remainder in the
/// Boolean comparison operands and typed call arguments. Exact x86 division
/// diamonds are retained in the enclosing conditional evidence.
pub(crate) fn accountable_conditional_boolean_expression(
    expression: &AssignedBooleanExpression,
) -> bool {
    match expression {
        AssignedBooleanExpression::Call { arguments, .. } => arguments
            .iter()
            .all(|argument| accountable_conditional_call_argument_expression(&argument.expression)),
        AssignedBooleanExpression::Immediate { .. }
        | AssignedBooleanExpression::Parameter { .. }
        | AssignedBooleanExpression::StructuralField { .. } => true,
        AssignedBooleanExpression::Not { operand, .. } => {
            accountable_conditional_boolean_expression(operand)
        }
        AssignedBooleanExpression::Equal { left, right, .. } => {
            accountable_conditional_boolean_expression(left)
                && accountable_conditional_boolean_expression(right)
        }
        AssignedBooleanExpression::IntegerEqual { left, right, .. }
        | AssignedBooleanExpression::IntegerLessThan { left, right, .. }
        | AssignedBooleanExpression::IntegerLessOrEqual { left, right, .. } => {
            accountable_conditional_arm_integer_expression(left)
                && accountable_conditional_arm_integer_expression(right)
        }
    }
}

fn accountable_conditional_call_argument_expression(expression: &AssignedScalarExpression) -> bool {
    match expression {
        AssignedScalarExpression::Boolean(expression) => {
            accountable_conditional_boolean_expression(expression)
        }
        AssignedScalarExpression::Integer { expression, .. } => {
            accountable_direct_integer_expression(expression)
        }
    }
}

/// The bounded conditional-division slice permits division in arm expressions
/// and typed call arguments. Exact x86 division diamonds are retained in the
/// enclosing direct or nested conditional evidence.
fn accountable_conditional_arm_integer_expression(expression: &AssignedIntegerExpression) -> bool {
    match expression {
        AssignedIntegerExpression::Call { arguments, .. } => arguments
            .iter()
            .all(|argument| accountable_conditional_call_argument_expression(&argument.expression)),
        AssignedIntegerExpression::Immediate { .. }
        | AssignedIntegerExpression::Parameter { .. } => true,
        AssignedIntegerExpression::BitwiseNot { operand, .. }
        | AssignedIntegerExpression::IntegerWiden { operand, .. }
        | AssignedIntegerExpression::IntegerExactCast { operand, .. } => {
            accountable_conditional_arm_integer_expression(operand)
        }
        AssignedIntegerExpression::WrappingAdd { left, right, .. }
        | AssignedIntegerExpression::ExactAdd { left, right, .. }
        | AssignedIntegerExpression::BitwiseAnd { left, right, .. }
        | AssignedIntegerExpression::BitwiseOr { left, right, .. }
        | AssignedIntegerExpression::BitwiseXor { left, right, .. }
        | AssignedIntegerExpression::WrappingShiftLeft {
            value: left,
            count: right,
            ..
        }
        | AssignedIntegerExpression::WrappingShiftRight {
            value: left,
            count: right,
            ..
        }
        | AssignedIntegerExpression::ExactShiftLeft {
            value: left,
            count: right,
            ..
        }
        | AssignedIntegerExpression::ExactShiftRight {
            value: left,
            count: right,
            ..
        }
        | AssignedIntegerExpression::SaturatingAdd { left, right, .. }
        | AssignedIntegerExpression::WrappingSubtract { left, right, .. }
        | AssignedIntegerExpression::ExactSubtract { left, right, .. }
        | AssignedIntegerExpression::SaturatingSubtract { left, right, .. }
        | AssignedIntegerExpression::WrappingMultiply { left, right, .. }
        | AssignedIntegerExpression::ExactMultiply { left, right, .. }
        | AssignedIntegerExpression::SaturatingMultiply { left, right, .. }
        | AssignedIntegerExpression::ExactDivide { left, right, .. }
        | AssignedIntegerExpression::ExactRemainder { left, right, .. }
        | AssignedIntegerExpression::WrappingDivide { left, right, .. }
        | AssignedIntegerExpression::WrappingRemainder { left, right, .. }
        | AssignedIntegerExpression::SaturatingDivide { left, right, .. }
        | AssignedIntegerExpression::SaturatingRemainder { left, right, .. } => {
            accountable_conditional_arm_integer_expression(left)
                && accountable_conditional_arm_integer_expression(right)
        }
    }
}

pub(crate) fn collect_scalar_stack_evidence(
    architecture: Architecture,
    bytes: &[u8],
    control_flow: ScalarControlFlowEvidence,
    cleanup_preservation: Option<ScalarCleanupPreservationEvidence>,
) -> Result<ScalarStackEvidence, EmissionError> {
    let mutations = match architecture {
        Architecture::X86_64 => {
            let mut decoder =
                iced_x86::Decoder::with_ip(64, bytes, 0, iced_x86::DecoderOptions::NONE);
            let mut mutations = Vec::new();
            while decoder.can_decode() {
                let instruction = decoder.decode();
                if instruction.is_invalid() {
                    return Err(EmissionError::ScalarStackInstructionEncodingInvalid);
                }
                let offset = usize::try_from(instruction.ip())
                    .map_err(|_| EmissionError::ScalarStackInstructionEncodingInvalid)?;
                let kind = match instruction.mnemonic() {
                    iced_x86::Mnemonic::Sub
                        if instruction.op0_register() == iced_x86::Register::RSP =>
                    {
                        Some(ScalarStackMutationKind::Allocate {
                            byte_size: x86_adjustment_immediate(bytes, offset, instruction.len())?,
                        })
                    }
                    iced_x86::Mnemonic::Add
                        if instruction.op0_register() == iced_x86::Register::RSP =>
                    {
                        Some(ScalarStackMutationKind::Release {
                            byte_size: x86_adjustment_immediate(bytes, offset, instruction.len())?,
                        })
                    }
                    iced_x86::Mnemonic::Lea
                        if instruction.op0_register() == iced_x86::Register::RSP =>
                    {
                        Some(ScalarStackMutationKind::X86ReleasePreservingFlags {
                            byte_size: x86_preserving_release_immediate(
                                bytes,
                                offset,
                                instruction.len(),
                            )?,
                        })
                    }
                    iced_x86::Mnemonic::Push => Some(ScalarStackMutationKind::X86Push),
                    iced_x86::Mnemonic::Pop => Some(ScalarStackMutationKind::X86Pop),
                    _ => None,
                };
                if let Some(kind) = kind {
                    mutations.push(ScalarStackMutation {
                        offset,
                        byte_count: instruction.len(),
                        kind,
                    });
                }
            }
            mutations
        }
        Architecture::Aarch64 => {
            if !bytes.len().is_multiple_of(4) {
                return Err(EmissionError::ScalarStackInstructionEncodingInvalid);
            }
            bytes
                .chunks_exact(4)
                .enumerate()
                .filter_map(|(index, encoded)| {
                    let encoded = u32::from_le_bytes(encoded.try_into().expect("four-byte word"));
                    let base = encoded & !(0xfff << 10);
                    let kind = match base {
                        0xd100_03ff => ScalarStackMutationKind::Allocate {
                            byte_size: (encoded >> 10) & 0xfff,
                        },
                        0x9100_03ff => ScalarStackMutationKind::Release {
                            byte_size: (encoded >> 10) & 0xfff,
                        },
                        _ => return None,
                    };
                    Some(ScalarStackMutation {
                        offset: index * 4,
                        byte_count: 4,
                        kind,
                    })
                })
                .collect()
        }
    };
    Ok(ScalarStackEvidence {
        mutations,
        control_flow,
        stack_alignment: 16,
        cleanup_preservation,
    })
}

pub(crate) fn expression_source(expression: &AssignedIntegerExpression) -> ValueId {
    match expression {
        AssignedIntegerExpression::Call { source_value, .. } => *source_value,
        AssignedIntegerExpression::Immediate { source_value, .. }
        | AssignedIntegerExpression::Parameter { source_value, .. } => *source_value,
        AssignedIntegerExpression::BitwiseNot { operand, .. }
        | AssignedIntegerExpression::IntegerWiden { operand, .. }
        | AssignedIntegerExpression::IntegerExactCast { operand, .. } => expression_source(operand),
        AssignedIntegerExpression::WrappingAdd { left, .. }
        | AssignedIntegerExpression::ExactAdd { left, .. }
        | AssignedIntegerExpression::BitwiseAnd { left, .. }
        | AssignedIntegerExpression::BitwiseOr { left, .. }
        | AssignedIntegerExpression::BitwiseXor { left, .. }
        | AssignedIntegerExpression::WrappingShiftLeft { value: left, .. }
        | AssignedIntegerExpression::WrappingShiftRight { value: left, .. }
        | AssignedIntegerExpression::ExactShiftLeft { value: left, .. }
        | AssignedIntegerExpression::ExactShiftRight { value: left, .. }
        | AssignedIntegerExpression::SaturatingAdd { left, .. }
        | AssignedIntegerExpression::WrappingSubtract { left, .. }
        | AssignedIntegerExpression::ExactSubtract { left, .. }
        | AssignedIntegerExpression::SaturatingSubtract { left, .. }
        | AssignedIntegerExpression::WrappingMultiply { left, .. }
        | AssignedIntegerExpression::ExactMultiply { left, .. }
        | AssignedIntegerExpression::SaturatingMultiply { left, .. } => expression_source(left),
        AssignedIntegerExpression::ExactDivide { left, .. } => expression_source(left),
        AssignedIntegerExpression::ExactRemainder { left, .. } => expression_source(left),
        AssignedIntegerExpression::WrappingDivide { left, .. } => expression_source(left),
        AssignedIntegerExpression::WrappingRemainder { left, .. } => expression_source(left),
        AssignedIntegerExpression::SaturatingDivide { left, .. } => expression_source(left),
        AssignedIntegerExpression::SaturatingRemainder { left, .. } => expression_source(left),
    }
}

pub(crate) fn boolean_expression_source(expression: &AssignedBooleanExpression) -> ValueId {
    match expression {
        AssignedBooleanExpression::Call { source_value, .. } => *source_value,
        AssignedBooleanExpression::Immediate { source_value, .. }
        | AssignedBooleanExpression::Parameter { source_value, .. }
        | AssignedBooleanExpression::StructuralField { source_value, .. } => *source_value,
        AssignedBooleanExpression::Not { operand, .. } => boolean_expression_source(operand),
        AssignedBooleanExpression::Equal { left, .. } => boolean_expression_source(left),
        AssignedBooleanExpression::IntegerEqual { left, .. }
        | AssignedBooleanExpression::IntegerLessThan { left, .. }
        | AssignedBooleanExpression::IntegerLessOrEqual { left, .. } => expression_source(left),
    }
}

pub(crate) fn native_integer_bounds(scalar_type: IntegerType) -> (u64, u64) {
    let width = scalar_type.bits();
    match scalar_type.sign() {
        IntegerSign::Unsigned => {
            let maximum = if width == 64 {
                u64::MAX
            } else {
                (1_u64 << width) - 1
            };
            (0, maximum)
        }
        IntegerSign::Signed => {
            let maximum = if width == 64 {
                i64::MAX as u64
            } else {
                (1_u64 << (width - 1)) - 1
            };
            let minimum = if width == 64 {
                i64::MIN as u64
            } else {
                u64::MAX << (width - 1)
            };
            (minimum, maximum)
        }
    }
}
