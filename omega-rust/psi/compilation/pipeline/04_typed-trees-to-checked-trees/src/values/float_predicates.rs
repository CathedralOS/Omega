//! Normalize positive IEEE comparisons into observations of one stable place.
//! Selection and sealed-source checks happen before assigning any order law.
//! The resulting predicate contains landed bits, not a source expression that
//! could read another value after a mutation or a state-parameter transfer.

use facts::FloatPredicate;
use numerics::bignum::{BigRational, ExactFloat, IeeeRounding};
use numerics::literals::FloatFormat;
use symbols::SymbolHandle;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

/// Enforced result ranges describe the completed value, including a computed
/// result with no storage identity. Publish them after evaluation; callers can
/// then use the ordinary operand snapshot rather than reconstructing a callee.
pub(crate) fn declared_type_predicates(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
    predicates: &mut Vec<FloatPredicate>,
) {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            declared_type_predicates(program, *referee, predicates);
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            declared_type_predicates(program, *base_type, predicates);
            let Some(primitive @ (PrimitiveType::F32 | PrimitiveType::F64)) =
                program.primitive_type_reference(reference)
            else {
                return;
            };
            let format = if primitive == PrimitiveType::F32 {
                FloatFormat::F32
            } else {
                FloatFormat::F64
            };
            let endpoint = |expression| match validation::closed_float_range_endpoint(
                program, expression, primitive,
            )? {
                semantic_vocabulary::IeeeFloatValue::Binary32(bits) => {
                    Some(ExactFloat::from_f32(f32::from_bits(bits)))
                }
                semantic_vocabulary::IeeeFloatValue::Binary64(bits) => {
                    Some(ExactFloat::from_f64(f64::from_bits(bits)))
                }
            };
            for constraint in program.type_reference_table.constraints(*constraints) {
                let TypeConstraintNode::Range {
                    minimum,
                    maximum,
                    end_inclusive,
                } = constraint
                else {
                    continue;
                };
                for (operator, value) in [
                    (BinaryOperator::GreaterOrEqual, *minimum),
                    (
                        if *end_inclusive {
                            BinaryOperator::LessOrEqual
                        } else {
                            BinaryOperator::Less
                        },
                        *maximum,
                    ),
                ] {
                    if let Some(predicate) = endpoint(value)
                        .and_then(|bound| FloatPredicate::comparison(format, operator, &bound))
                    {
                        predicates.push(predicate);
                    }
                }
            }
        }
        _ => {}
    }
}

pub(crate) fn collect_true(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    observations: &mut Vec<(ExpressionHandle, FloatPredicate)>,
) {
    if let Some(borrow) = program.expression_table.borrow_expression(expression) {
        collect_true(
            program,
            machine,
            state,
            statement_index,
            borrow.target,
            observations,
        );
        return;
    }
    let Some(binary) = comparison(program, expression) else {
        return;
    };
    if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Equal)
        && validation::has_builtin_decomposed_guard_meaning(
            program,
            machine,
            Some(state),
            expression,
        )
    {
        if binary.operator == BinaryOperator::And {
            collect_true(
                program,
                machine,
                state,
                statement_index,
                binary.left,
                observations,
            );
            collect_true(
                program,
                machine,
                state,
                statement_index,
                binary.right,
                observations,
            );
            return;
        }
        for (operand, wrapper) in [(binary.left, binary.right), (binary.right, binary.left)] {
            if matches!(
                program.expression_table.expression(wrapper),
                ExpressionNode::Boolean(true)
            ) {
                collect_true(
                    program,
                    machine,
                    state,
                    statement_index,
                    operand,
                    observations,
                );
                return;
            }
        }
    }
    for (subject, bound, operator) in [
        (binary.left, binary.right, binary.operator),
        (binary.right, binary.left, reverse(binary.operator)),
    ] {
        let Some(reference) = validation::expression_type_reference_in_state(
            program,
            state.symbol,
            statement_index,
            subject,
        ) else {
            continue;
        };
        let format = match program.type_reference_table.primitive_type(reference) {
            Some(PrimitiveType::F32) => FloatFormat::F32,
            Some(PrimitiveType::F64) => FloatFormat::F64,
            _ => continue,
        };
        if !has_ieee_meaning(
            program,
            machine.symbol,
            expression,
            binary.operator,
            reference,
        ) {
            continue;
        }
        let Some(place) = validation::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            statement_index,
            subject,
        ) else {
            continue;
        };
        // A live selector is a separate read dependency. Until its captured
        // identity is represented, a source-free predicate cannot follow a
        // later spelling of that selector after it changes. Keep fields and
        // fixed selections under the same static-identity policy as transport.
        if !place
            .segments
            .iter()
            .all(crate::flow::place_segment_has_static_identity)
        {
            continue;
        }
        let predicate = if operator == BinaryOperator::Equal
            && validation::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                statement_index,
                bound,
            )
            .is_some_and(|other| other == place)
        {
            Some(FloatPredicate::non_nan(format))
        } else {
            literal(program, bound, format)
                .and_then(|bound| FloatPredicate::comparison(format, operator, &bound))
        };
        if let Some(predicate) = predicate {
            observations.push((subject, predicate));
            return;
        }
    }
}

pub(crate) fn literal(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    format: FloatFormat,
) -> Option<ExactFloat> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Float(value) => Some(match format {
            FloatFormat::F32 => ExactFloat::from_f32(value.value_f32()),
            FloatFormat::F64 => ExactFloat::from_f64(value.value_f64()),
        }),
        ExpressionNode::Integer(value) => {
            let rational = BigRational::from_integer(value.value_bignum()?);
            Some(match format {
                FloatFormat::F32 => ExactFloat::from_f32(
                    rational.to_f32_with_rounding(IeeeRounding::NearestTiesToEven),
                ),
                FloatFormat::F64 => ExactFloat::from_f64(
                    rational.to_f64_with_rounding(IeeeRounding::NearestTiesToEven),
                ),
            })
        }
        ExpressionNode::Borrow(borrow) => literal(program, borrow.target, format),
        _ => None,
    }
}

pub(crate) fn has_ieee_meaning(
    program: &dyn ::validation::CheckingInput,
    owner: SymbolHandle,
    expression: ExpressionHandle,
    operator: BinaryOperator,
    reference: TypeReferenceHandle,
) -> bool {
    owner.is_valid()
        && validation::has_builtin_or_sealed_float_comparison_meaning(
            program,
            owner,
            expression,
            operator,
            &[Some(reference), Some(reference)],
        )
}

/// Reading a token-lowered call's operation does not authorize its laws.
/// Consumers must check the exact selection with `has_ieee_meaning`.
pub(crate) fn comparison(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<TableBinaryExpression> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => Some(*binary),
        ExpressionNode::Call(call)
            if !call.receiver.is_valid()
                && !call.static_machine_parameter.is_valid()
                && call.static_requirement_dispatch.is_none()
                && call.machine_arguments.is_empty()
                && call.evidence_arguments.is_empty() =>
        {
            let (machine, _) =
                validation::find_machine_by_entry_state(program, call.target_symbol)?;
            let binding = program.machine_token_binding_by_symbol(machine.symbol)?;
            let (operator, _) =
                typed_trees::operator::primitive_float_binary_semantics(program, binding)?;
            let [left, right] = program.expression_table.expression_handles(call.arguments) else {
                return None;
            };
            Some(TableBinaryExpression {
                operator,
                left: *left,
                right: *right,
            })
        }
        _ => None,
    }
}

pub(crate) fn reverse(operator: BinaryOperator) -> BinaryOperator {
    operator.mirrored()
}
