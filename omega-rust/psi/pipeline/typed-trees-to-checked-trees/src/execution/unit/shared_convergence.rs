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
    if (!structural_fields.is_empty() && !has_boolean_scalar_input)
        || (!structural_fields.is_empty() && has_integer_scalar_input)
        || structural_fields.iter().any(|(position, _)| {
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
        | checked_trees::CheckedBooleanExpression::ScalarIeeeFloatComparison { .. }
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
        | CheckedScalarExpression::IntegerSaturatingCast { .. }
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

#[cfg(test)]
mod tests {
    use super::*;
    use checked_trees::{
        CheckedBooleanExpression as Boolean, CheckedBooleanExpression,
        CheckedIntegerComparisonKind, CheckedLocatedScalarExpression,
        CheckedScalarBindingDestination, CheckedScalarBindingValue,
        CheckedStructuralPredicatePathSegment as Segment,
        CheckedUnitNominalAffineCleanupPlan,
    };

    fn direct_field(parameter_position: u32, field: &str) -> CheckedBooleanExpression {
        Boolean::StructuralParameterField {
            parameter_position,
            path: vec![Segment::Field(field.to_owned())],
        }
    }

    fn convergence(
        binding_expression: CheckedBooleanExpression,
        scalar_parameter_count: usize,
        cleanup_positions: &[u32],
    ) -> Option<checked_trees::CheckedStructuralBooleanConvergencePlan> {
        let mut facts = CheckFacts::default();
        let state = SymbolHandle::from_parts(1, 1);
        facts.values.scalar_expressions.expressions.push(
            CheckedLocatedScalarExpression {
                state,
                statement_ordinal: 0,
                role: CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 },
                expression: CheckedScalarExpression::Boolean(Box::new(binding_expression)),
            },
        );
        let bindings = [CheckedScalarBinding {
            statement_ordinal: 0,
            destination: CheckedScalarBindingDestination::Immutable,
            primitive_type: PrimitiveType::Bool,
            value: CheckedScalarBindingValue::Expression,
        }];
        let cleanup_actions = cleanup_positions
            .iter()
            .map(|position| {
                CheckedStructuralScalarReturnCleanupAction::InvokeNominal(
                    CheckedUnitNominalAffineCleanupPlan {
                        source_parameter_index: *position,
                        type_identity: String::new(),
                        cleanup_machine: SymbolHandle::invalid(),
                        cleanup_state: SymbolHandle::invalid(),
                        cleanup_contract_report_fingerprint: 0,
                        requirements: Vec::new(),
                    },
                )
            })
            .collect::<Vec<_>>();
        let return_expression = CheckedScalarExpression::Boolean(Box::new(Boolean::Local {
            position: scalar_parameter_count,
        }));
        checked_shared_boolean_convergence(
            &facts,
            state,
            &bindings,
            &return_expression,
            scalar_parameter_count,
            &cleanup_actions,
        )
    }

    #[test]
    fn second_member_fields_converge_when_each_source_carries_cleanup() {
        // `flag && (token.observed || token.other)`
        let same_source = Boolean::And {
            left: Box::new(Boolean::Parameter { position: 0 }),
            right: Box::new(Boolean::Or {
                left: Box::new(direct_field(0, "observed")),
                right: Box::new(direct_field(0, "other")),
            }),
        };
        assert!(convergence(same_source, 1, &[0]).is_some());
        // `flag && (a.ready || b.ready)` reads fields of two parameters.
        let two_sources = Boolean::And {
            left: Box::new(Boolean::Parameter { position: 0 }),
            right: Box::new(Boolean::Or {
                left: Box::new(direct_field(0, "ready")),
                right: Box::new(direct_field(1, "ready")),
            }),
        };
        assert!(convergence(two_sources.clone(), 1, &[0, 1]).is_some());
        // Each member field's source parameter must carry nominal cleanup.
        assert!(convergence(two_sources, 1, &[0]).is_none());
    }

    #[test]
    fn member_only_and_nested_fields_stay_outside_convergence() {
        // `token.observed || token.other` still lacks a Boolean scalar input.
        let field_only = Boolean::Or {
            left: Box::new(direct_field(0, "observed")),
            right: Box::new(direct_field(0, "other")),
        };
        assert!(convergence(field_only, 0, &[0]).is_none());
        // `flag && wrap.inner` where the member path is two segments deep.
        let nested = Boolean::And {
            left: Box::new(Boolean::Parameter { position: 0 }),
            right: Box::new(Boolean::StructuralParameterField {
                parameter_position: 0,
                path: vec![Segment::Field("wrap".to_owned()), Segment::Field("flag".to_owned())],
            }),
        };
        assert!(convergence(nested, 1, &[0]).is_none());
        // `flag && (token.observed || left == right)` keeps the member/integer
        // mixture out of the shared tail.
        let with_integer = Boolean::And {
            left: Box::new(Boolean::Parameter { position: 0 }),
            right: Box::new(Boolean::Or {
                left: Box::new(direct_field(0, "observed")),
                right: Box::new(Boolean::IntegerComparison {
                    kind: CheckedIntegerComparisonKind::Equal,
                    left: Box::new(CheckedScalarExpression::Parameter {
                        position: 1,
                        primitive_type: PrimitiveType::U8,
                    }),
                    right: Box::new(CheckedScalarExpression::Parameter {
                        position: 2,
                        primitive_type: PrimitiveType::U8,
                    }),
                }),
            }),
        };
        assert!(convergence(with_integer, 3, &[0]).is_none());
    }
}
