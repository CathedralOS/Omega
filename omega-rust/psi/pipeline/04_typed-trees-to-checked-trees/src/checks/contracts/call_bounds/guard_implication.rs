//! A strict selected-arm comparison implies its non-strict counterpart only
//! over the same substituted values. This is an identity judgment, not interval
//! evaluation; the caller separately checks intervening operand writes.
use checked_trees::CheckFacts;
use language_core::OperatorSpelling;
use typed_trees::{
    TypedTrees,
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode},
    machine::Machine,
    signature::StateParameter,
    state::State,
    types::{PrimitiveType, TypeReferenceNode},
};

#[derive(PartialEq, Eq)]
struct Operand {
    place: crate::flow::CanonicalPlace,
    meaning: Meaning,
}

#[derive(PartialEq, Eq)]
enum Meaning {
    CollectionLength,
    Integer(PrimitiveType),
}

#[allow(clippy::too_many_arguments)]
pub(super) fn proves(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller: &Machine,
    caller_state: &State,
    callee: &Machine,
    callee_state: &State,
    parameters: &[StateParameter],
    arguments: &[ExpressionHandle],
    goal: ExpressionHandle,
    guard: ExpressionHandle,
) -> bool {
    let ExpressionNode::Binary(required) = program.expression_table.expression(goal) else {
        return false;
    };
    let (left, right, spelling) = match required.operator {
        BinaryOperator::LessOrEqual => (required.left, required.right, OperatorSpelling::LessEqual),
        BinaryOperator::GreaterOrEqual => (
            required.right,
            required.left,
            OperatorSpelling::GreaterEqual,
        ),
        _ => return false,
    };
    if !builtin(program, facts, callee, callee_state, goal, spelling) {
        return false;
    }
    let goal_operand = |expression| {
        let (expression, meaning) = observation(program, callee, callee_state, expression)?;
        let (actual, suffix) = crate::semantic::places::call_contract_argument_projection(
            program, parameters, arguments, expression,
        )?;
        // This proof preserves values, not merely storage identities. Reject
        // projected formal coercions and policy-bearing scalar shells here.
        if !suffix.is_empty()
            || !super::super::prover::has_builtin_operators(program, &facts.operators, actual)
        {
            return None;
        }
        let actual_meaning = match meaning {
            Meaning::CollectionLength => {
                let reference = validation::declared_place_type_raw(
                    program,
                    caller,
                    Some(caller_state),
                    actual,
                )?;
                let reference = validation::unwrapped_type_reference(program, reference)?;
                if !matches!(
                    program.type_reference_table.type_reference(reference),
                    TypeReferenceNode::Slice { .. } | TypeReferenceNode::FixedArray { .. }
                ) {
                    return None;
                }
                Meaning::CollectionLength
            }
            Meaning::Integer(_) => observation(program, caller, caller_state, actual)?.1,
        };
        if actual_meaning != meaning {
            return None;
        }
        let mut place = crate::flow::canonical_place_from_expression(program, actual)?;
        place.segments.extend(suffix);
        exact_place(place).map(|place| Operand { place, meaning })
    };
    let (Some(left), Some(right)) = (goal_operand(left), goal_operand(right)) else {
        return false;
    };
    let guard_operand = |expression| {
        let (expression, meaning) = observation(program, caller, caller_state, expression)?;
        let place = crate::flow::canonical_place_from_expression(program, expression)?;
        exact_place(place).map(|place| Operand { place, meaning })
    };
    let mut pending = vec![guard];
    while let Some(expression) = pending.pop() {
        let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
            continue;
        };
        match binary.operator {
            BinaryOperator::And
                if super::super::prover::has_builtin_operators(
                    program,
                    &facts.operators,
                    expression,
                ) =>
            {
                pending.extend([binary.right, binary.left]);
            }
            BinaryOperator::Equal
                if builtin(
                    program,
                    facts,
                    caller,
                    caller_state,
                    expression,
                    OperatorSpelling::Equal,
                ) =>
            {
                if matches!(
                    program.expression_table.expression(binary.right),
                    ExpressionNode::Boolean(true)
                ) {
                    pending.push(binary.left);
                } else if matches!(
                    program.expression_table.expression(binary.left),
                    ExpressionNode::Boolean(true)
                ) {
                    pending.push(binary.right);
                }
            }
            BinaryOperator::Less | BinaryOperator::Greater => {
                let (first, second, spelling) = if binary.operator == BinaryOperator::Less {
                    (binary.left, binary.right, OperatorSpelling::Less)
                } else {
                    (binary.right, binary.left, OperatorSpelling::Greater)
                };
                if builtin(program, facts, caller, caller_state, expression, spelling)
                    && guard_operand(first).as_ref() == Some(&left)
                    && guard_operand(second).as_ref() == Some(&right)
                {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

fn builtin(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    spelling: OperatorSpelling,
) -> bool {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return false;
    };
    let operands = [binary.left, binary.right]
        .map(|operand| validation::declared_place_type_raw(program, machine, Some(state), operand));
    super::super::prover::has_builtin_operators(program, &facts.operators, expression)
        && typed_trees::operator::has_builtin_spelled_expression_meaning(
            program,
            machine.symbol,
            expression,
            spelling,
            &operands,
        )
}

fn observation(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<(ExpressionHandle, Meaning)> {
    if let Some(receiver) =
        validation::collection_length_receiver(program, machine, Some(state), expression)
    {
        return Some((receiver, Meaning::CollectionLength));
    }
    let reference = validation::declared_place_type_raw(program, machine, Some(state), expression)?;
    // Plain primitive carriers have the same exact arithmetic policy on
    // both sides of substitution. Constrained/ref carriers need a separate
    // value-preserving coercion judgment and are deliberately not inferred.
    if !matches!(
        program.type_reference_table.type_reference(reference),
        TypeReferenceNode::Named { .. }
    ) {
        return None;
    }
    let primitive = program.primitive_type_reference(reference)?;
    matches!(
        primitive,
        PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
    )
    .then_some((expression, Meaning::Integer(primitive)))
}

fn exact_place(place: crate::flow::CanonicalPlace) -> Option<crate::flow::CanonicalPlace> {
    (matches!(place.root, facts::PlaceRoot::Symbol(symbol) if symbol.is_valid())
        && place.segments.iter().all(
            |segment| matches!(segment, facts::PlaceSegment::Field { symbol } if symbol.is_valid()),
        ))
    .then_some(place)
}
