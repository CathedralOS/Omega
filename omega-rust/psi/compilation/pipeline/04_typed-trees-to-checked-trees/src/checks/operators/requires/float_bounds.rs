//! IEEE bound implications for invocation preconditions, independent of the
//! called requirement. A positive ordered comparison excludes NaN and can
//! imply a weaker bound; its negation cannot. Only builtin or sealed IEEE
//! meanings supply these laws. Canonical places identify the subject, while
//! InvocationContexts selects its captured value's facts before this query:
//! current storage facts must never replace a detached operand's snapshot.

use super::{InvocationContexts, closed_operators_are_builtin, operand_for_parameter};
use checked_trees::CheckedOperatorFacts;
use facts::{FactPayload, FactPlace, FactPlan, FloatPredicate, ProgramPoint};
use numerics::literals::FloatFormat;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

use crate::flow::{CanonicalPlace, canonical_place_from_expression};
use crate::values::float_predicates::{comparison, has_ieee_meaning, literal, reverse};

struct RequiredComparison {
    place: CanonicalPlace,
    reference: TypeReferenceHandle,
    predicate: FloatPredicate,
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
                    FactPayload::FloatPredicate { predicate }
                        if fact_has_place(program, semantic, fact.place, &required.place) =>
                    {
                        predicate.implies(required.predicate)
                    }
                    FactPayload::AssignedValue { value }
                        if fact_has_place(program, semantic, fact.place, &required.place) =>
                    {
                        literal(program, value, required.predicate.format())
                            .and_then(|bound| {
                                FloatPredicate::comparison(
                                    required.predicate.format(),
                                    BinaryOperator::Equal,
                                    &bound,
                                )
                            })
                            .is_some_and(|observed| observed.implies(required.predicate))
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
        let predicate = if operator == BinaryOperator::Equal
            && canonical_place_from_expression(program, subject)
                == canonical_place_from_expression(program, other)
        {
            FloatPredicate::non_nan(format)
        } else {
            FloatPredicate::comparison(format, operator, &literal(program, other, format)?)?
        };
        return Some(RequiredComparison {
            place,
            reference: parameter.type_reference,
            predicate,
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
        return binary.operator == BinaryOperator::Equal
            && FloatPredicate::non_nan(required.predicate.format()).implies(required.predicate);
    }
    let (operator, bound) = if left_is_subject {
        (binary.operator, binary.right)
    } else if right_is_subject {
        (reverse(binary.operator), binary.left)
    } else {
        return false;
    };
    literal(program, bound, required.predicate.format())
        .and_then(|bound| FloatPredicate::comparison(required.predicate.format(), operator, &bound))
        .is_some_and(|observed| observed.implies(required.predicate))
}

fn expression_has_place(
    program: &TypedTrees,
    expression: ExpressionHandle,
    expected: &CanonicalPlace,
) -> bool {
    canonical_place_from_expression(program, expression)
        .is_some_and(|place| crate::flow::canonical_places_agree(program, &place, expected))
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
        .is_some_and(|place| crate::flow::canonical_places_agree(program, &place, expected))
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
