//! Runtime-input custody for shared Boolean convergence.
//!
//! Arithmetic composes through its operation tree. Collecting roots grants no
//! arithmetic proof: Terminal production must discharge every emitted operation,
//! including intermediate operations whose results are later erased.
use super::{
    BTreeSet, CheckFacts, CheckedIntegerBinaryKind, CheckedScalarBinding, CheckedScalarExpression,
    CheckedScalarExpressionRole, CheckedStructuralScalarReturnCleanupAction, PrimitiveType,
    SymbolHandle,
};
use crate::execution::terminal_unit::returns::checked_boolean_contains_short_circuit;

pub(super) fn checked_shared_boolean_convergence(
    facts: &CheckFacts,
    state: SymbolHandle,
    bindings: &[CheckedScalarBinding],
    return_expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
    cleanup_actions: &[CheckedStructuralScalarReturnCleanupAction],
) -> Option<checked_trees::CheckedStructuralBooleanConvergencePlan> {
    let [binding] = bindings else {
        return None;
    };
    if binding.statement_ordinal != 0 || binding.primitive_type != PrimitiveType::Bool {
        return None;
    }
    let expression = facts.values.scalar_expressions.expression_at(
        state,
        0,
        CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 },
    )?;
    let CheckedScalarExpression::Boolean(expression) = expression else {
        return None;
    };
    let runtime_inputs = shared_boolean_runtime_inputs(expression, scalar_parameter_count)?;
    let structural_fields = runtime_inputs
        .iter()
        .filter_map(|input| match input {
            SharedBooleanRuntimeInput::StructuralField {
                parameter_position,
                field,
            } => Some((*parameter_position, field)),
            SharedBooleanRuntimeInput::BooleanScalar(_)
            | SharedBooleanRuntimeInput::IntegerScalar(_) => None,
        })
        .collect::<Vec<_>>();
    let has_boolean_scalar_input = runtime_inputs
        .iter()
        .any(|input| matches!(input, SharedBooleanRuntimeInput::BooleanScalar(_)));
    let has_integer_scalar_input = runtime_inputs
        .iter()
        .any(|input| matches!(input, SharedBooleanRuntimeInput::IntegerScalar(_)));
    if structural_fields.len() > 1
        || (!structural_fields.is_empty() && !has_boolean_scalar_input)
        || (!structural_fields.is_empty() && has_integer_scalar_input)
        || structural_fields.first().is_some_and(|(position, _)| {
            !cleanup_actions.iter().any(|action| {
                matches!(
                    action,
                    CheckedStructuralScalarReturnCleanupAction::InvokeNominal(cleanup)
                        if cleanup.source_parameter_index == *position
                )
            })
        })
    {
        return None;
    }
    if !checked_boolean_contains_short_circuit(expression)
        || runtime_inputs.is_empty()
        || !matches!(
            return_expression,
            CheckedScalarExpression::Boolean(expression)
                if matches!(expression.as_ref(),
                    checked_trees::CheckedBooleanExpression::Local { position }
                        if *position == scalar_parameter_count)
        )
    {
        return None;
    }
    Some(checked_trees::CheckedStructuralBooleanConvergencePlan { binding_ordinal: 0 })
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum SharedBooleanRuntimeInput {
    BooleanScalar(usize),
    IntegerScalar(usize),
    StructuralField {
        parameter_position: u32,
        field: String,
    },
}

pub(super) fn shared_boolean_has_member_and_integer_inputs(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> bool {
    let CheckedScalarExpression::Boolean(expression) = expression else {
        return false;
    };
    shared_boolean_runtime_inputs(expression, scalar_parameter_count).is_some_and(|inputs| {
        inputs
            .iter()
            .any(|input| matches!(input, SharedBooleanRuntimeInput::StructuralField { .. }))
            && inputs
                .iter()
                .any(|input| matches!(input, SharedBooleanRuntimeInput::IntegerScalar(_)))
    })
}

fn shared_boolean_runtime_inputs(
    expression: &checked_trees::CheckedBooleanExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    match expression {
        checked_trees::CheckedBooleanExpression::StorageRead { .. } => None,
        checked_trees::CheckedBooleanExpression::Constant(_) => Some(BTreeSet::new()),
        checked_trees::CheckedBooleanExpression::Parameter { position }
            if *position < scalar_parameter_count =>
        {
            Some(BTreeSet::from([SharedBooleanRuntimeInput::BooleanScalar(
                *position,
            )]))
        }
        checked_trees::CheckedBooleanExpression::Not(operand) => {
            shared_boolean_runtime_inputs(operand, scalar_parameter_count)
        }
        checked_trees::CheckedBooleanExpression::Equal { left, right } => {
            match (left.as_ref(), right.as_ref()) {
                (checked_trees::CheckedBooleanExpression::Constant(_), expression)
                | (expression, checked_trees::CheckedBooleanExpression::Constant(_)) => {
                    shared_boolean_runtime_inputs(expression, scalar_parameter_count)
                }
                _ => None,
            }
        }
        checked_trees::CheckedBooleanExpression::And { left, right }
        | checked_trees::CheckedBooleanExpression::Or { left, right } => {
            let mut parameters = shared_boolean_runtime_inputs(left, scalar_parameter_count)?;
            parameters.extend(shared_boolean_runtime_inputs(
                right,
                scalar_parameter_count,
            )?);
            Some(parameters)
        }
        checked_trees::CheckedBooleanExpression::StructuralParameterField {
            parameter_position,
            path,
        } if matches!(
            path.as_slice(),
            [checked_trees::CheckedStructuralPredicatePathSegment::Field(
                _
            )]
        ) =>
        {
            let [checked_trees::CheckedStructuralPredicatePathSegment::Field(field)] =
                path.as_slice()
            else {
                unreachable!("guarded by one field segment")
            };
            Some(BTreeSet::from([
                SharedBooleanRuntimeInput::StructuralField {
                    parameter_position: *parameter_position,
                    field: field.clone(),
                },
            ]))
        }
        checked_trees::CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
            let mut inputs = shared_integer_runtime_inputs(left, scalar_parameter_count)?;
            inputs.extend(shared_integer_runtime_inputs(
                right,
                scalar_parameter_count,
            )?);
            Some(inputs)
        }
        checked_trees::CheckedBooleanExpression::IeeeFloatComparison { .. }
        | checked_trees::CheckedBooleanExpression::ByteSequenceEqual { .. }
        | checked_trees::CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | checked_trees::CheckedBooleanExpression::StructuralCaseMembership { .. } => None,
        checked_trees::CheckedBooleanExpression::Parameter { .. }
        | checked_trees::CheckedBooleanExpression::ErasedParameter { .. }
        | checked_trees::CheckedBooleanExpression::Local { .. }
        | checked_trees::CheckedBooleanExpression::StructuralParameterField { .. } => None,
    }
}

fn fixed_native_integer(primitive_type: PrimitiveType) -> bool {
    matches!(
        primitive_type,
        PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
    )
}

/// Collect roots, not sufficient arithmetic forms. Operand safety belongs to
/// the independent operation obligations, never to a final expression bound.
fn shared_integer_runtime_inputs(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    if !expression
        .primitive_type()
        .is_some_and(fixed_native_integer)
    {
        return None;
    }
    match expression {
        CheckedScalarExpression::IntegerLiteral { .. } => Some(BTreeSet::new()),
        CheckedScalarExpression::Parameter { position, .. }
            if *position < scalar_parameter_count =>
        {
            Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                *position,
            )]))
        }
        CheckedScalarExpression::IntegerBinary {
            kind:
                kind @ (CheckedIntegerBinaryKind::BitwiseAnd
                | CheckedIntegerBinaryKind::BitwiseOr
                | CheckedIntegerBinaryKind::BitwiseXor
                | CheckedIntegerBinaryKind::WrappingShiftLeft
                | CheckedIntegerBinaryKind::WrappingShiftRight
                | CheckedIntegerBinaryKind::WrappingAdd
                | CheckedIntegerBinaryKind::SaturatingAdd
                | CheckedIntegerBinaryKind::WrappingSubtract
                | CheckedIntegerBinaryKind::SaturatingSubtract
                | CheckedIntegerBinaryKind::WrappingMultiply
                | CheckedIntegerBinaryKind::SaturatingMultiply
                | CheckedIntegerBinaryKind::ExactAdd
                | CheckedIntegerBinaryKind::ExactSubtract
                | CheckedIntegerBinaryKind::ExactMultiply
                | CheckedIntegerBinaryKind::ExactDivide
                | CheckedIntegerBinaryKind::ExactRemainder
                | CheckedIntegerBinaryKind::ExactShiftLeft
                | CheckedIntegerBinaryKind::ExactShiftRight),
            left,
            right,
            primitive_type,
        } => {
            let is_shift = matches!(
                kind,
                CheckedIntegerBinaryKind::ExactShiftLeft
                    | CheckedIntegerBinaryKind::ExactShiftRight
                    | CheckedIntegerBinaryKind::WrappingShiftLeft
                    | CheckedIntegerBinaryKind::WrappingShiftRight
            );
            if left.primitive_type() != Some(*primitive_type)
                || (!is_shift && right.primitive_type() != Some(*primitive_type))
            {
                return None;
            }
            let mut inputs = shared_integer_runtime_inputs(left, scalar_parameter_count)?;
            inputs.extend(shared_integer_runtime_inputs(
                right,
                scalar_parameter_count,
            )?);
            Some(inputs)
        }
        CheckedScalarExpression::IntegerBitwiseNot {
            primitive_type,
            operand,
        } if operand.primitive_type() == Some(*primitive_type) => {
            shared_integer_runtime_inputs(operand, scalar_parameter_count)
        }
        CheckedScalarExpression::IntegerExactCast { operand, .. } => {
            // Every fixed-to-fixed cast has a canonical representability goal.
            // Identity/total casts are legal operations, not partial cast words.
            shared_integer_runtime_inputs(operand, scalar_parameter_count)
        }
        CheckedScalarExpression::IntegerWiden {
            primitive_type,
            operand,
        } if operand.primitive_type().is_some_and(|source_type| {
            validation::integer_widen_is_total(source_type, *primitive_type)
        }) =>
        {
            shared_integer_runtime_inputs(operand, scalar_parameter_count)
        }
        CheckedScalarExpression::StorageRead { .. }
        | CheckedScalarExpression::IeeeFloatLiteral { .. }
        | CheckedScalarExpression::StructuralParameterByteLength { .. }
        | CheckedScalarExpression::Parameter { .. }
        | CheckedScalarExpression::ErasedParameter { .. }
        | CheckedScalarExpression::Local { .. }
        | CheckedScalarExpression::IntegerBinary { .. }
        | CheckedScalarExpression::IntegerBitwiseNot { .. }
        | CheckedScalarExpression::IntegerWiden { .. }
        | CheckedScalarExpression::IntegerTrappingCast { .. }
        | CheckedScalarExpression::IntegerWrappingCast { .. }
        | CheckedScalarExpression::StructuralParameterIndexedRead { .. }
        | CheckedScalarExpression::StructuralParameterField { .. }
        | CheckedScalarExpression::Boolean(_) => None,
    }
}

#[cfg(test)]
pub(crate) fn shared_integer_runtime_parameter_positions_for_test(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
) -> Option<Vec<usize>> {
    shared_integer_runtime_inputs(expression, scalar_parameter_count).map(|inputs| {
        inputs
            .into_iter()
            .filter_map(|input| match input {
                SharedBooleanRuntimeInput::IntegerScalar(position) => Some(position),
                _ => None,
            })
            .collect()
    })
}
