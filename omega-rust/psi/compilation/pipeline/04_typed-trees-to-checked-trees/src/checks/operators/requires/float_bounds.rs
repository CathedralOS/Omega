//! IEEE bound implications for invocation preconditions, independent of the
//! called requirement. A positive ordered comparison excludes NaN and can
//! imply a weaker bound; its negation cannot. Only builtin or sealed IEEE
//! meanings supply these laws. Canonical places identify the subject, while
//! InvocationContexts selects its captured value's facts before this query:
//! current storage facts must never replace a detached operand's snapshot.

use super::{
    InvocationContexts, closed_exact_float, closed_operators_are_builtin, closed_scalar_value,
    operand_for_parameter,
};
use checked_trees::CheckedOperatorFacts;
use facts::{FactPayload, FactPlace, FactPlan, ProgramPoint};
use numerics::bignum::ExactFloat;
use numerics::literals::FloatFormat;
use std::cmp::Ordering;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
};
use typed_trees::signature::StateParameter;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

use crate::flow::{CanonicalPlace, canonical_place_from_expression};

struct RequiredComparison {
    place: CanonicalPlace,
    reference: TypeReferenceHandle,
    format: FloatFormat,
    operator: BinaryOperator,
    // Absent only for IEEE self-equality, which requires non-NaN evidence.
    bound: Option<ExactFloat>,
}

pub(super) fn contexts_prove_comparison(
    program: &TypedTrees,
    semantic: &FactPlan,
    operators: &CheckedOperatorFacts,
    contexts: &InvocationContexts<'_>,
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    expression: ExpressionHandle,
) -> bool {
    let Some(required) = required_comparison(program, parameters, operands, expression) else {
        return false;
    };
    contexts
        .for_expressions(program, parameters, [expression])
        .iter()
        .any(|context| {
            semantic
                .context_view(semantic.contexts.get(*context))
                .facts()
                .any(|fact| match fact.payload {
                    FactPayload::BooleanExpression(expression)
                    | FactPayload::BooleanValue {
                        expression,
                        value: true,
                    } => expression_proves_comparison(
                        program,
                        operators,
                        point_owner(fact.point),
                        expression,
                        &required,
                    ),
                    FactPayload::ContractBooleanExpression {
                        expression,
                        instantiated,
                        ..
                    } if !instantiated.is_valid() => expression_proves_comparison(
                        program,
                        operators,
                        point_owner(fact.point),
                        expression,
                        &required,
                    ),
                    FactPayload::AssignedValue { value }
                        if fact_has_place(program, semantic, fact.place, &required.place) =>
                    {
                        literal(program, value, required.format).is_some_and(|bound| {
                            bound_implies(&required, BinaryOperator::Equal, &bound)
                        })
                    }
                    _ => false,
                })
        })
}

fn required_comparison(
    program: &TypedTrees,
    parameters: &[StateParameter],
    operands: &[ExpressionHandle],
    expression: ExpressionHandle,
) -> Option<RequiredComparison> {
    let binary = comparison(program, expression)?;
    for (subject, other, operator) in [
        (binary.left, binary.right, binary.operator),
        (binary.right, binary.left, reverse(binary.operator)),
    ] {
        let ExpressionNode::Name(path) = program.expression_table.expression(subject) else {
            continue;
        };
        let Some(parameter) = parameters.iter().find(|parameter| {
            parameter.symbol == path.symbol || parameter.symbol == path.head_symbol
        }) else {
            continue;
        };
        let format = match program
            .type_reference_table
            .primitive_type(parameter.type_reference)
        {
            Some(PrimitiveType::F32) => FloatFormat::F32,
            Some(PrimitiveType::F64) => FloatFormat::F64,
            _ => continue,
        };
        let owner = program.symbols.get(parameter.symbol).parent;
        if !has_ieee_meaning(
            program,
            owner,
            expression,
            binary.operator,
            parameter.type_reference,
        ) {
            continue;
        }
        let operand = operand_for_parameter(program, parameters, operands, path)?;
        let place = canonical_place_from_expression(program, operand)?;
        let bound = if operator == BinaryOperator::Equal
            && canonical_place_from_expression(program, subject)
                == canonical_place_from_expression(program, other)
        {
            None
        } else {
            Some(literal(program, other, format)?)
        };
        return Some(RequiredComparison {
            place,
            reference: parameter.type_reference,
            format,
            operator,
            bound,
        });
    }
    None
}

fn expression_proves_comparison(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    owner: SymbolHandle,
    expression: ExpressionHandle,
    required: &RequiredComparison,
) -> bool {
    if let ExpressionNode::Borrow(borrow) = program.expression_table.expression(expression) {
        return expression_proves_comparison(program, operators, owner, borrow.target, required);
    }
    let Some(binary) = comparison(program, expression) else {
        return false;
    };
    if binary.operator == BinaryOperator::And
        && closed_operators_are_builtin(program, operators, expression)
    {
        return expression_proves_comparison(program, operators, owner, binary.left, required)
            || expression_proves_comparison(program, operators, owner, binary.right, required);
    }
    if binary.operator == BinaryOperator::Equal
        && matches!(
            program.expression_table.expression(binary.right),
            ExpressionNode::Boolean(true)
        )
        && closed_operators_are_builtin(program, operators, expression)
    {
        return expression_proves_comparison(program, operators, owner, binary.left, required);
    }
    if !has_ieee_meaning(
        program,
        owner,
        expression,
        binary.operator,
        required.reference,
    ) {
        return false;
    }
    let left_is_subject = expression_has_place(program, binary.left, &required.place);
    let right_is_subject = expression_has_place(program, binary.right, &required.place);
    if left_is_subject && right_is_subject {
        return required.bound.is_none() && binary.operator == BinaryOperator::Equal;
    }
    let (operator, bound) = if left_is_subject {
        (binary.operator, binary.right)
    } else if right_is_subject {
        (reverse(binary.operator), binary.left)
    } else {
        return false;
    };
    literal(program, bound, required.format)
        .is_some_and(|bound| bound_implies(required, operator, &bound))
}

/// Ordered positive facts exclude NaN; `!=` does not. Strictness belongs to
/// the relation, not a host-float epsilon (which would lose f32 landing).
fn bound_implies(
    required: &RequiredComparison,
    observed: BinaryOperator,
    bound: &ExactFloat,
) -> bool {
    use BinaryOperator::{Equal, Greater, GreaterOrEqual, Less, LessOrEqual};
    if !matches!(
        observed,
        Equal | Greater | GreaterOrEqual | Less | LessOrEqual
    ) || bound.partial_cmp_value(bound).is_none()
    {
        return false;
    }
    let Some(required_bound) = &required.bound else {
        return true;
    };
    let Some(ordering) = bound.partial_cmp_value(required_bound) else {
        return false;
    };
    match required.operator {
        Equal => observed == Equal && ordering == Ordering::Equal,
        Greater | GreaterOrEqual if matches!(observed, Equal | Greater | GreaterOrEqual) => {
            ordering == Ordering::Greater
                || (ordering == Ordering::Equal
                    && (required.operator == GreaterOrEqual || observed == Greater))
        }
        Less | LessOrEqual if matches!(observed, Equal | Less | LessOrEqual) => {
            ordering == Ordering::Less
                || (ordering == Ordering::Equal
                    && (required.operator == LessOrEqual || observed == Less))
        }
        _ => false,
    }
}

fn literal(
    program: &TypedTrees,
    expression: ExpressionHandle,
    format: FloatFormat,
) -> Option<ExactFloat> {
    closed_exact_float(&closed_scalar_value(program, &[], &[], expression)?, format)
}

fn has_ieee_meaning(
    program: &TypedTrees,
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

/// Token lowering may represent a comparison as the selected declaration's
/// entry call. Reading its operation does not authorize a law: every consumer
/// still checks exact sealed source and selection with `has_ieee_meaning`.
fn comparison(program: &TypedTrees, expression: ExpressionHandle) -> Option<TableBinaryExpression> {
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
                crate::semantic::calls::find_machine_by_entry_state(program, call.target_symbol)?;
            let binding = program
                .machine_token_bindings()
                .iter()
                .find(|binding| binding.symbol == machine.symbol)?;
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

fn reverse(operator: BinaryOperator) -> BinaryOperator {
    match operator {
        BinaryOperator::Less => BinaryOperator::Greater,
        BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
        BinaryOperator::Greater => BinaryOperator::Less,
        BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
        other => other,
    }
}

fn expression_has_place(
    program: &TypedTrees,
    expression: ExpressionHandle,
    expected: &CanonicalPlace,
) -> bool {
    canonical_place_from_expression(program, expression)
        .is_some_and(|place| places_equal(program, &place, expected))
}

fn fact_has_place(
    program: &TypedTrees,
    semantic: &FactPlan,
    place: FactPlace,
    expected: &CanonicalPlace,
) -> bool {
    let FactPlace::Place(place) = place else {
        return false;
    };
    crate::flow::canonical_place_from_semantic_place(program, semantic, semantic.places.get(place))
        .is_some_and(|place| places_equal(program, &place, expected))
}

fn places_equal(program: &TypedTrees, left: &CanonicalPlace, right: &CanonicalPlace) -> bool {
    crate::flow::normalized_event_place_root(program, left.root)
        == crate::flow::normalized_event_place_root(program, right.root)
        && left.segments.len() == right.segments.len()
        && left
            .segments
            .iter()
            .zip(&right.segments)
            .all(|(left, right)| crate::flow::canonical_place_segments_equal(*left, *right))
}

fn point_owner(point: ProgramPoint) -> SymbolHandle {
    match point {
        ProgramPoint::Global => SymbolHandle::default(),
        ProgramPoint::Definition { symbol } => symbol,
        ProgramPoint::Machine { machine_symbol }
        | ProgramPoint::State { machine_symbol, .. }
        | ProgramPoint::Statement { machine_symbol, .. }
        | ProgramPoint::Call { machine_symbol, .. }
        | ProgramPoint::CallRequires { machine_symbol, .. }
        | ProgramPoint::CallEnsures { machine_symbol, .. }
        | ProgramPoint::TransitionArm { machine_symbol, .. }
        | ProgramPoint::Exit { machine_symbol, .. } => machine_symbol,
    }
}
