//! Runtime scalar observations require the original receiver, not an attachment.
use super::*;
use checked_trees::{CheckedBooleanExpression, CheckedScalarComputationKind};

pub(in crate::flow::terminal_unit) fn reads_receiver(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
) -> bool {
    let Some(position) = program
        .state_parameters(state)
        .iter()
        .position(|parameter| parameter.is_self && is_reference(program, parameter.type_reference))
    else {
        return false;
    };
    let Ok(position) = u32::try_from(position) else {
        return false;
    };
    if facts
        .values
        .scalar_expressions
        .expressions
        .iter()
        .any(|expression| {
            expression.state == state.symbol && scalar_reads(&expression.expression, position)
        })
    {
        return true;
    }
    let computations = &facts.values.scalar_computations;
    let mut pending = computations
        .roots
        .iter()
        .filter(|(_, root)| root.state == state.symbol)
        .map(|(_, root)| root.root)
        .collect::<Vec<_>>();
    // Builder appends fresh operand nodes before their parents. This runs on
    // that freshly produced graph, before externally retained checked plans;
    // it needs no cycle-discovery or per-node visited lookup.
    while let Some(handle) = pending.pop() {
        if !computations.nodes.is_valid(handle) {
            continue;
        }
        let node = computations.nodes.get(handle);
        match &node.kind {
            CheckedScalarComputationKind::CaseMembership {
                subject:
                    checked_trees::CheckedScalarComputationStructuralArgument::Place(_)
                    | checked_trees::CheckedScalarComputationStructuralArgument::Array { .. },
                ..
            } => {}
            CheckedScalarComputationKind::CaseMembership {
                subject: checked_trees::CheckedScalarComputationStructuralArgument::Case(subject),
                ..
            } => {
                pending.extend(
                    computations
                        .case_fields
                        .span_or_empty(subject.fields)
                        .iter()
                        .map(|field| field.value),
                );
            }
            CheckedScalarComputationKind::SelectedComparison { left, right, .. } => {
                pending.extend([*left, *right])
            }
            CheckedScalarComputationKind::Value(expression) => {
                if scalar_reads(expression, position) {
                    return true;
                }
            }
            CheckedScalarComputationKind::Apply { operands, .. } => {
                pending.extend_from_slice(computations.operands.span_or_empty(*operands));
            }
            CheckedScalarComputationKind::Call { arguments, .. } => {
                pending.extend_from_slice(computations.operands.span_or_empty(*arguments));
            }
            CheckedScalarComputationKind::Select {
                condition,
                when_true,
                when_false,
                ..
            } => {
                pending.extend([*condition, *when_true, *when_false]);
            }
            CheckedScalarComputationKind::Qualification { operand, .. } => pending.push(*operand),
            CheckedScalarComputationKind::Dispatch { subject, arms, .. } => {
                pending.push(*subject);
                for arm in computations.dispatch_arms.span_or_empty(*arms) {
                    if let checked_trees::CheckedScalarDispatchPattern::Value(pattern) = arm.pattern
                    {
                        pending.push(pattern);
                    }
                    pending.push(arm.value);
                }
            }
        }
    }
    false
}

fn scalar_reads(expression: &CheckedScalarExpression, receiver: u32) -> bool {
    match expression {
        CheckedScalarExpression::StructuralParameterByteLength { parameter_position }
        | CheckedScalarExpression::StructuralParameterField {
            parameter_position, ..
        } => *parameter_position == receiver,
        CheckedScalarExpression::StructuralParameterIndexedRead {
            parameter_position,
            index,
            ..
        } => *parameter_position == receiver || scalar_reads(index, receiver),
        CheckedScalarExpression::IntegerBinary { left, right, .. } => {
            scalar_reads(left, receiver) || scalar_reads(right, receiver)
        }
        CheckedScalarExpression::IntegerBitwiseNot { operand, .. }
        | CheckedScalarExpression::IntegerWiden { operand, .. }
        | CheckedScalarExpression::IntegerExactCast { operand, .. }
        | CheckedScalarExpression::IntegerWrappingCast { operand, .. }
        | CheckedScalarExpression::IntegerTrappingCast { operand, .. } => {
            scalar_reads(operand, receiver)
        }
        CheckedScalarExpression::Boolean(expression) => boolean_reads(expression, receiver),
        CheckedScalarExpression::StorageRead { .. }
        | CheckedScalarExpression::Parameter { .. }
        | CheckedScalarExpression::Local { .. }
        | CheckedScalarExpression::IntegerLiteral { .. }
        | CheckedScalarExpression::IeeeFloatLiteral { .. } => false,
    }
}

fn boolean_reads(expression: &CheckedBooleanExpression, receiver: u32) -> bool {
    match expression {
        CheckedBooleanExpression::StructuralParameterField {
            parameter_position, ..
        } => *parameter_position == receiver,
        CheckedBooleanExpression::Not(operand) => boolean_reads(operand, receiver),
        CheckedBooleanExpression::Equal { left, right }
        | CheckedBooleanExpression::And { left, right }
        | CheckedBooleanExpression::Or { left, right } => {
            boolean_reads(left, receiver) || boolean_reads(right, receiver)
        }
        CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
            scalar_reads(left, receiver) || scalar_reads(right, receiver)
        }
        CheckedBooleanExpression::IeeeFloatComparison { left, right, .. }
        | CheckedBooleanExpression::ByteSequenceEqual { left, right }
        | CheckedBooleanExpression::PayloadlessSumEqual { left, right, .. } => {
            left.parameter_position == receiver || right.parameter_position == receiver
        }
        CheckedBooleanExpression::StructuralCaseMembership { subject, .. } => {
            subject.parameter_position == receiver
        }
        CheckedBooleanExpression::StorageRead { .. }
        | CheckedBooleanExpression::Constant(_)
        | CheckedBooleanExpression::Parameter { .. }
        | CheckedBooleanExpression::Local { .. } => false,
    }
}
