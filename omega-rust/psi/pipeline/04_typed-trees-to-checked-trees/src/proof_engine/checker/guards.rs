//! Range narrowing by transition guards, assignment guards and literal
//! comparisons.

use crate::proof_engine::checker::integer_ranges::{
    apply_handle_condition_complement, integer_literal_handle,
};
use crate::proof_engine::obligations::{
    IntegerRange, ProofPlan, dehoisted_condition, dehoisted_operand,
};
use numerics::bignum::BigInt;
use symbol_resolved_trees_to_typed_trees::typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
};
use symbol_resolved_trees_to_typed_trees::typed_trees::statement::TransitionGuardNode;

pub(crate) fn apply_handle_guard(
    proof_plan: &ProofPlan,
    range: IntegerRange,
    argument: ExpressionHandle,
    guard: &TransitionGuardNode,
) -> IntegerRange {
    match guard {
        TransitionGuardNode::Always => range,
        TransitionGuardNode::When(condition) => {
            apply_handle_condition(proof_plan, range, argument, *condition)
        }
    }
}

pub(crate) fn apply_assignment_guard(
    proof_plan: &ProofPlan,
    range: IntegerRange,
    value: ExpressionHandle,
    guard: &TransitionGuardNode,
) -> IntegerRange {
    let range = apply_handle_guard(proof_plan, range, value, guard);

    let ExpressionNode::Binary(value_binary) =
        proof_plan.program.expression_table.expression(value)
    else {
        return range;
    };
    if value_binary.operator != BinaryOperator::Subtract {
        return range;
    }

    let TransitionGuardNode::When(condition) = guard else {
        return range;
    };
    let condition = unwrap_true_guard_condition(proof_plan, *condition);
    let ExpressionNode::Binary(condition_binary) =
        proof_plan.program.expression_table.expression(condition)
    else {
        return range;
    };

    let lower_bound =
        if expressions_equivalent_for_proof(proof_plan, value_binary.left, condition_binary.left)
            && expressions_equivalent_for_proof(
                proof_plan,
                value_binary.right,
                condition_binary.right,
            )
        {
            match condition_binary.operator {
                BinaryOperator::Greater => Some(1),
                BinaryOperator::GreaterOrEqual => Some(0),
                _ => None,
            }
        } else if expressions_equivalent_for_proof(
            proof_plan,
            value_binary.left,
            condition_binary.right,
        ) && expressions_equivalent_for_proof(
            proof_plan,
            value_binary.right,
            condition_binary.left,
        ) {
            match condition_binary.operator {
                BinaryOperator::Less => Some(1),
                BinaryOperator::LessOrEqual => Some(0),
                _ => None,
            }
        } else {
            None
        };

    let Some(lower_bound) = lower_bound else {
        return range;
    };

    IntegerRange {
        minimum: range.minimum.max(BigInt::from_i64(lower_bound)),
        maximum: range.maximum,
    }
}

pub(crate) fn unwrap_true_guard_condition(
    proof_plan: &ProofPlan,
    condition: ExpressionHandle,
) -> ExpressionHandle {
    let ExpressionNode::Binary(binary) = proof_plan.program.expression_table.expression(condition)
    else {
        return condition;
    };

    if binary.operator == BinaryOperator::Equal {
        if matches!(
            proof_plan.program.expression_table.expression(binary.right),
            ExpressionNode::Boolean(true)
        ) {
            return binary.left;
        }

        if matches!(
            proof_plan.program.expression_table.expression(binary.left),
            ExpressionNode::Boolean(true)
        ) {
            return binary.right;
        }
    }

    condition
}

pub(crate) fn apply_handle_condition(
    proof_plan: &ProofPlan,
    range: IntegerRange,
    argument: ExpressionHandle,
    condition: ExpressionHandle,
) -> IntegerRange {
    if let ExpressionNode::Unary(unary) = proof_plan.program.expression_table.expression(condition)
        && unary.operator == UnaryOperator::LogicalNot
    {
        return apply_handle_condition_complement(proof_plan, range, argument, unary.operand);
    }
    let ExpressionNode::Binary(binary) = proof_plan.program.expression_table.expression(condition)
    else {
        return range;
    };

    if binary.operator == BinaryOperator::Equal {
        if matches!(
            proof_plan.program.expression_table.expression(binary.right),
            ExpressionNode::Boolean(true)
        ) {
            return apply_handle_condition(proof_plan, range, argument, binary.left);
        }

        if matches!(
            proof_plan.program.expression_table.expression(binary.left),
            ExpressionNode::Boolean(true)
        ) {
            return apply_handle_condition(proof_plan, range, argument, binary.right);
        }
    }

    if binary.operator == BinaryOperator::And {
        let range = apply_handle_condition(proof_plan, range, argument, binary.left);
        return apply_handle_condition(proof_plan, range, argument, binary.right);
    }

    if expressions_equivalent_for_proof(proof_plan, binary.left, argument) {
        return apply_right_literal_guard(proof_plan, range, binary.operator, binary.right);
    }

    if expressions_equivalent_for_proof(proof_plan, binary.right, argument) {
        return apply_left_literal_guard(proof_plan, range, binary.left, binary.operator);
    }

    range
}

/// `apply_handle_condition` with GUARD-SIDE de-hoisting: the guard's compared
/// sides may spell a hoisted local from the SOURCE state's scope (an indexed
/// guard subject is frontend-hoisted to `let __hoist_N = tallies[self.k]`), so
/// each side is resolved through that state's call-free place initializers
/// before the structural match. Only used where the obligation carries the
/// guard's source state; co-located guards keep the plain applier.
pub(crate) fn apply_source_condition(
    proof_plan: &ProofPlan,
    range: IntegerRange,
    argument: ExpressionHandle,
    condition: ExpressionHandle,
    machine_symbol: symbols::SymbolHandle,
    source_state: symbols::SymbolHandle,
) -> IntegerRange {
    // A hoisted GUARD SUBJECT is a bare name whose initializer is the actual
    // comparison (`transition __hoist_N { .. }`): resolve it in the SOURCE
    // state's scope before matching.
    let condition = proof_plan
        .program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .and_then(|machine| {
            proof_plan
                .program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == source_state)
                .map(|state| dehoisted_condition(proof_plan.program, state, condition))
        })
        .unwrap_or(condition);
    let ExpressionNode::Binary(binary) = proof_plan.program.expression_table.expression(condition)
    else {
        return range;
    };

    if binary.operator == BinaryOperator::Equal {
        if matches!(
            proof_plan.program.expression_table.expression(binary.right),
            ExpressionNode::Boolean(true)
        ) {
            return apply_source_condition(
                proof_plan,
                range,
                argument,
                binary.left,
                machine_symbol,
                source_state,
            );
        }
        if matches!(
            proof_plan.program.expression_table.expression(binary.left),
            ExpressionNode::Boolean(true)
        ) {
            return apply_source_condition(
                proof_plan,
                range,
                argument,
                binary.right,
                machine_symbol,
                source_state,
            );
        }
    }

    if binary.operator == BinaryOperator::And {
        let range = apply_source_condition(
            proof_plan,
            range,
            argument,
            binary.left,
            machine_symbol,
            source_state,
        );
        return apply_source_condition(
            proof_plan,
            range,
            argument,
            binary.right,
            machine_symbol,
            source_state,
        );
    }

    let dehoist = |handle: ExpressionHandle| {
        let program = proof_plan.program;
        program
            .machines()
            .iter()
            .find(|machine| machine.symbol == machine_symbol)
            .and_then(|machine| {
                program
                    .machine_states(machine)
                    .iter()
                    .find(|state| state.symbol == source_state)
                    .map(|state| dehoisted_operand(program, state, handle))
            })
            .unwrap_or(handle)
    };

    let left = dehoist(binary.left);
    let right = dehoist(binary.right);
    if expressions_equivalent_for_proof(proof_plan, left, argument) {
        return apply_right_literal_guard(proof_plan, range, binary.operator, right);
    }
    if expressions_equivalent_for_proof(proof_plan, right, argument) {
        return apply_left_literal_guard(proof_plan, range, left, binary.operator);
    }

    range
}

pub(crate) fn expressions_equivalent_for_proof(
    proof_plan: &ProofPlan,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    if left == right {
        return true;
    }

    // The SAME place is spelled two ways across statements: a flat Name path
    // (`Name(["self", "k"])`) or a Member chain (`Member(Name(["self"]), "k")`).
    // Flatten both to segment lists and compare -- without this, a guard's
    // `self.k` never matched a hoisted initializer's `self.k` and the fact
    // silently failed to apply. `flat_place_segments` returns None for
    // anything indexed/called, which falls through to the structural arms.
    if let (Some(left_path), Some(right_path)) = (
        flat_place_segments(proof_plan, left),
        flat_place_segments(proof_plan, right),
    ) {
        return left_path == right_path;
    }

    match (
        proof_plan.program.expression_table.expression(left),
        proof_plan.program.expression_table.expression(right),
    ) {
        (ExpressionNode::Borrow(left), _) => {
            expressions_equivalent_for_proof(proof_plan, left.target, right)
        }
        (_, ExpressionNode::Borrow(right)) => {
            expressions_equivalent_for_proof(proof_plan, left, right.target)
        }
        (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
            proof_plan
                .program
                .expression_table
                .name_path_members(left.members)
                == proof_plan
                    .program
                    .expression_table
                    .name_path_members(right.members)
        }
        (ExpressionNode::Call(left), ExpressionNode::Call(right)) => {
            left.target == right.target
                && left.target_symbol == right.target_symbol
                && left.arguments.count() == right.arguments.count()
                && match (left.receiver.is_valid(), right.receiver.is_valid()) {
                    (true, true) => {
                        expressions_equivalent_for_proof(proof_plan, left.receiver, right.receiver)
                    }
                    (false, false) => true,
                    _ => false,
                }
                && proof_plan
                    .program
                    .expression_table
                    .expression_handles(left.arguments)
                    .iter()
                    .zip(
                        proof_plan
                            .program
                            .expression_table
                            .expression_handles(right.arguments),
                    )
                    .all(|(left_argument, right_argument)| {
                        expressions_equivalent_for_proof(
                            proof_plan,
                            *left_argument,
                            *right_argument,
                        )
                    })
        }
        (ExpressionNode::Member(left), ExpressionNode::Member(right)) => {
            left.member == right.member
                && left.member_symbol == right.member_symbol
                && expressions_equivalent_for_proof(proof_plan, left.receiver, right.receiver)
        }
        // An INDEXED place (`self.tallies[1]`, `self.tallies[self.k]`): same
        // collection, same index. Lets a guard fact on an element (`tallies[1]
        // < 16`) refine the element's read in the guarded state -- the
        // accumulate-into-array keystone (`tallies[1] = tallies[1] + 1`).
        // Sound under the stability gate: the guard's read paths include the
        // COLLECTION (any indexed write into it aliases) and the INDEX
        // variable (a write to `k` drops the fact).
        (ExpressionNode::Indexed(left), ExpressionNode::Indexed(right)) => {
            let (left, right) = (*left, *right);
            expressions_equivalent_for_proof(proof_plan, left.collection, right.collection)
                && expressions_equivalent_for_proof(proof_plan, left.index, right.index)
        }
        // LEAVES: identical guards/places from different statements hold
        // distinct handles, so literal sub-terms (a `[1]` index, a compared
        // constant) must compare by VALUE (same fix as the precondition twin).
        (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => left == right,
        (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) => left == right,
        (ExpressionNode::Float(left), ExpressionNode::Float(right)) => left == right,
        (ExpressionNode::String(left), ExpressionNode::String(right)) => left == right,
        _ => false,
    }
}

/// A pure member place flattened to its name segments (`self.k` ->
/// ["self", "k"]), through `Mutable`. `None` for anything indexed, called, or
/// non-place -- those compare structurally.
fn flat_place_segments(
    proof_plan: &ProofPlan,
    expression: ExpressionHandle,
) -> Option<Vec<String>> {
    match proof_plan.program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => flat_place_segments(proof_plan, inner.target),
        ExpressionNode::Name(path) => Some(
            proof_plan
                .program
                .expression_table
                .name_path_members(path.members)
                .iter()
                .map(|member| member.as_str().to_owned())
                .collect(),
        ),
        ExpressionNode::Member(member) => {
            let mut segments = flat_place_segments(proof_plan, member.receiver)?;
            segments.push(member.member.as_str().to_owned());
            Some(segments)
        }
        _ => None,
    }
}

pub(crate) fn apply_right_literal_guard(
    proof_plan: &ProofPlan,
    mut range: IntegerRange,
    operator: BinaryOperator,
    right: ExpressionHandle,
) -> IntegerRange {
    let Some(value) = integer_literal_handle(proof_plan, right) else {
        return range;
    };

    let value = BigInt::from_i64(value);
    let one = BigInt::from_i64(1);
    match operator {
        BinaryOperator::Equal => {
            range.minimum = range.minimum.max(value.clone());
            range.maximum = range.maximum.min(value);
        }
        BinaryOperator::Greater => range.minimum = range.minimum.max(value.add(&one)),
        BinaryOperator::GreaterOrEqual => range.minimum = range.minimum.max(value),
        BinaryOperator::Less => range.maximum = range.maximum.min(value.sub(&one)),
        BinaryOperator::LessOrEqual => range.maximum = range.maximum.min(value),
        BinaryOperator::Add
        | BinaryOperator::And
        | BinaryOperator::BitwiseAnd
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor
        | BinaryOperator::Divide
        | BinaryOperator::Modulo
        | BinaryOperator::Multiply
        | BinaryOperator::NotEqual
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight
        | BinaryOperator::Subtract
        | BinaryOperator::CaseMembership => {}
    }

    range
}

pub(crate) fn apply_left_literal_guard(
    proof_plan: &ProofPlan,
    mut range: IntegerRange,
    left: ExpressionHandle,
    operator: BinaryOperator,
) -> IntegerRange {
    let Some(value) = integer_literal_handle(proof_plan, left) else {
        return range;
    };

    let value = BigInt::from_i64(value);
    let one = BigInt::from_i64(1);
    match operator {
        BinaryOperator::Equal => {
            range.minimum = range.minimum.max(value.clone());
            range.maximum = range.maximum.min(value);
        }
        BinaryOperator::Greater => range.maximum = range.maximum.min(value.sub(&one)),
        BinaryOperator::GreaterOrEqual => range.maximum = range.maximum.min(value),
        BinaryOperator::Less => range.minimum = range.minimum.max(value.add(&one)),
        BinaryOperator::LessOrEqual => range.minimum = range.minimum.max(value),
        BinaryOperator::Add
        | BinaryOperator::And
        | BinaryOperator::BitwiseAnd
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor
        | BinaryOperator::Divide
        | BinaryOperator::Modulo
        | BinaryOperator::Multiply
        | BinaryOperator::NotEqual
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight
        | BinaryOperator::Subtract
        | BinaryOperator::CaseMembership => {}
    }

    range
}
