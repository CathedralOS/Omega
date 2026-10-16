mod bounds;

use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
};

use super::expressions::{expression_integer_value, expression_name};
use super::facts::RangeFacts;
use bounds::{
    seed_at_most_fact, seed_at_most_len_range_bound_fact, seed_equal_len_offset_fact,
    seed_index_at_most_integer_fact, seed_index_less_than_integer_fact, seed_length_at_least_fact,
    seed_length_equality_fact, seed_length_greater_than_fact, seed_length_not_zero_fact,
    seed_less_than_len_fact, seed_non_negative_fact, seed_successor_at_most_len_fact,
};

/// The decomposed-meaning check plus boolean-alias peel both guard seeders
/// share: a guard with no builtin decomposed meaning is the caller's to
/// skip, and a name-headed guard stands for its comparison, so re-seed from
/// the alias guard. Returns true when the guard is handled and the caller
/// must stop. False leaves an ordinary builtin expression for the caller to
/// seed; reversing that dispatch both loses valid bounds and admits facts
/// from operators without builtin meaning.
fn peel_boolean_alias_guard(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    facts: &mut RangeFacts<'_>,
    guard: ExpressionHandle,
    recurse: fn(
        &dyn ::validation::CheckingInput,
        &typed_trees::machine::Machine,
        &typed_trees::state::State,
        &mut RangeFacts<'_>,
        ExpressionHandle,
    ),
) -> bool {
    if !validation::has_builtin_decomposed_guard_meaning(program, machine, Some(state), guard) {
        return true;
    }
    // A boolean local's name stands for its comparison: `has_next ->
    // target(next_index)` under `has_next = next_index < count` mints
    // `next_index`'s bound from `count`'s declared range exactly as the
    // spelled comparison would.
    if let Some(path) = program.expression_table.name_path_node(guard) {
        let name = program
            .expression_table
            .name_path_members(path.members)
            .last()
            .map(|name| name.as_str());
        if let Some(alias_guard) = facts.boolean_guard_local(path.symbol, name)
            && alias_guard != guard
        {
            recurse(program, machine, state, facts, alias_guard);
        }
        return true;
    }
    false
}

/// A selected comparison transfers a proven endpoint onto its lower operand:
/// `i < k` with `k <= 8` proves `i < 8`; `<=` shifts by one and `>`/`>=`
/// mirror. Read live bounds, legacy store-enforced ranges, or an immutable
/// parameter's own declaration facts; mutable default-domain text is not a
/// premise during an open invariant window. Literal RHS comparisons stay
/// with the literal seeders. This needs machine/state context, so it runs
/// beside `seed_guard_facts` at the co-located transition arm.
pub(super) fn seed_value_vs_value_endpoints(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    facts: &mut RangeFacts<'_>,
    guard: ExpressionHandle,
) {
    if peel_boolean_alias_guard(
        program,
        machine,
        state,
        facts,
        guard,
        seed_value_vs_value_endpoints,
    ) {
        return;
    }
    let Some(binary) = program.expression_table.binary_node(guard) else {
        return;
    };
    match binary.operator {
        BinaryOperator::And => {
            seed_value_vs_value_endpoints(program, machine, state, facts, binary.left);
            seed_value_vs_value_endpoints(program, machine, state, facts, binary.right);
            return;
        }
        BinaryOperator::Equal
            if matches!(
                program.expression_table.expression(binary.right),
                ExpressionNode::Boolean(true)
            ) =>
        {
            seed_value_vs_value_endpoints(program, machine, state, facts, binary.left);
            return;
        }
        _ => {}
    }
    let (bounded, bound_source, inclusive_shift) = match binary.operator {
        BinaryOperator::Less => (binary.left, binary.right, 0),
        BinaryOperator::LessOrEqual => (binary.left, binary.right, 1),
        BinaryOperator::Greater => (binary.right, binary.left, 0),
        BinaryOperator::GreaterOrEqual => (binary.right, binary.left, 1),
        _ => return,
    };
    // Literal bounds are the literal seeders' job.
    if crate::checks::ranges::expressions::expression_integer_value(program, facts, bound_source)
        .is_some()
    {
        return;
    }
    let high = [
        facts
            .proven_index_upper_bound(&program.expression_table.display_name(bound_source))
            .and_then(|exclusive| exclusive.checked_sub(1)),
        crate::checks::ranges::types::expression_enforced_declared_range(
            program,
            machine,
            state,
            bound_source,
        )
        .map(|(_, high)| high),
        immutable_parameter_endpoint(program, machine, state, bound_source),
        // A `.len` over declared fixed storage reads a constant: `hi <=
        // main.arr.len` on a `[T; 5]` field bounds `hi` by the declared
        // extent, no carried premise needed.
        validation::collection_length_receiver(program, machine, Some(state), bound_source)
            .and_then(|receiver| {
                validation::declared_collection_extent(program, machine, Some(state), receiver)
            })
            .map(|extent| extent as i64),
    ]
    .into_iter()
    .flatten()
    .min();
    let Some(high) = high else {
        return;
    };
    let Some(exclusive) = high.checked_add(inclusive_shift) else {
        return;
    };
    facts.prove_index_upper_bound(program.expression_table.display_name(bounded), exclusive);
}

fn immutable_parameter_endpoint(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
) -> Option<i64> {
    let place = validation::canonical_place_from_expression(program, expression)?;
    let root = place.root.as_symbol()?;
    let parameter = program.state_parameter_by_symbol(state, root)?;
    // The general declaration reader also accepts mutable self storage. That
    // is not invocation-fixed: its where clause may be temporarily broken.
    // Only an immutable non-receiver input can contribute declaration bounds
    // here without separate statement-local establishment evidence.
    if (parameter.is_mutable) || parameter.is_const {
        return None;
    }
    validation::immutable_integer_expression_bounds(program, machine, state, expression)
        .map(|(_, high)| high)
}

pub(super) fn seed_guard_facts(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    facts: &mut RangeFacts<'_>,
    guard: ExpressionHandle,
) {
    if peel_boolean_alias_guard(program, machine, state, facts, guard, seed_guard_facts) {
        return;
    }

    // `!guard` holding is `guard` refuted.
    if let Some(operand) = negated_operand(program, guard) {
        seed_negated_guard_facts(program, machine, state, facts, operand);
        return;
    }

    let Some(binary) = program.expression_table.binary_node(guard) else {
        return;
    };

    seed_binary_guard_facts(program, machine, state, facts, binary, binary.operator);
}

/// The operand of a logical negation, the one unary guard spelling whose
/// truth and refutation swap.
fn negated_operand(
    program: &dyn ::validation::CheckingInput,
    guard: ExpressionHandle,
) -> Option<ExpressionHandle> {
    match program.expression_table.expression(guard) {
        ExpressionNode::Unary(unary)
            if unary.operator == typed_trees::expression::UnaryOperator::LogicalNot =>
        {
            Some(unary.operand)
        }
        _ => None,
    }
}

/// Preserve the exact lower bound, not merely nonnegativity, of an immutable
/// const binder. Requires and selected positive/refuted guards share this path;
/// callers have already established builtin decomposed comparison meaning.
fn seed_const_binder_floor(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    facts: &mut RangeFacts<'_>,
    binary: &TableBinaryExpression,
    operator: BinaryOperator,
) {
    for (subject, bound, left) in [
        (binary.left, binary.right, true),
        (binary.right, binary.left, false),
    ] {
        let Some(path) = program.expression_table.name_path_node(subject) else {
            continue;
        };
        if path.head_symbol != path.symbol
            || program
                .expression_table
                .name_path_members(path.members)
                .len()
                != 1
            || !super::types::is_const_binder(program, machine, path.symbol)
        {
            continue;
        }
        let Some(bound) = expression_integer_value(program, facts, bound) else {
            continue;
        };
        let floor = match (operator, left) {
            (BinaryOperator::GreaterOrEqual, true)
            | (BinaryOperator::LessOrEqual, false)
            | (BinaryOperator::Equal, _) => Some(bound),
            (BinaryOperator::Greater, true) | (BinaryOperator::Less, false) => bound.checked_add(1),
            _ => None,
        };
        if let Some(floor) = floor {
            facts.prove_const_binder_floor(path.symbol, floor);
        }
    }
}

/// A strict comparison against a const binder is the symbolic extent
/// obligation: `index < Count` and `index <= Count - k` (k >= 1) prove
/// `index` is below every `[T; Count]` in this machine. The binder is
/// immutable, so the row survives until the index label is written.
fn seed_strictly_below_const_binder(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    facts: &mut RangeFacts<'_>,
    lower: ExpressionHandle,
    upper: ExpressionHandle,
    strict: bool,
) {
    let Some(binder) = const_binder_strict_upper(program, machine, state, facts, upper, strict)
    else {
        return;
    };
    facts.prove_below_const_binder(program.expression_table.display_name(lower), binder);
}

/// The const binder a comparison's upper side strictly bounds, if any.
/// A bare binder needs a strict comparison. `binder - k` is strict for any
/// k >= 0 under `<`, and for k >= 1 under `<=`, matching the len offset rule.
fn const_binder_strict_upper(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    facts: &RangeFacts<'_>,
    upper: ExpressionHandle,
    strict: bool,
) -> Option<symbols::SymbolHandle> {
    if strict && let Some(binder) = bare_const_binder(program, machine, upper) {
        return Some(binder);
    }
    // A wrapping subtraction is not `binder - k`. Only the builtin bound
    // meaning can turn that spelling into a strict upper bound.
    if !validation::has_builtin_bound_expression_meaning(program, machine, Some(state), upper) {
        return None;
    }
    let binary = program.expression_table.binary_node(upper)?;
    if binary.operator != BinaryOperator::Subtract {
        return None;
    }
    let offset = expression_integer_value(program, facts, binary.right)?;
    if offset < if strict { 0 } else { 1 } {
        return None;
    }
    bare_const_binder(program, machine, binary.left)
}

fn bare_const_binder(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    expression: ExpressionHandle,
) -> Option<symbols::SymbolHandle> {
    let path = program.expression_table.name_path_node(expression)?;
    if path.head_symbol != path.symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
        || !super::types::is_const_binder(program, machine, path.symbol)
    {
        return None;
    }
    Some(path.symbol)
}

fn seed_binary_guard_facts(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    facts: &mut RangeFacts<'_>,
    binary: &TableBinaryExpression,
    operator: BinaryOperator,
) {
    facts.retain_closed_integer_arrival(program, binary, operator);
    seed_const_binder_floor(program, machine, facts, binary, operator);
    facts.record_expression_dependencies(program, machine, state, binary.left);
    facts.record_expression_dependencies(program, machine, state, binary.right);
    match operator {
        BinaryOperator::Less => {
            seed_length_greater_than_fact(program, facts, binary.right, binary.left);
            seed_less_than_len_fact(program, machine, state, facts, binary.left, binary.right);
            seed_index_less_than_integer_fact(program, facts, binary.left, binary.right);
            seed_strictly_below_const_binder(
                program,
                machine,
                state,
                facts,
                binary.left,
                binary.right,
                true,
            );
            seed_at_most_fact(program, facts, binary.left, binary.right, true);
            // `K < right` (left a constant) floors `right` at `K + 1`.
            seed_non_negative_fact(program, facts, binary.right, binary.left, false);
        }
        BinaryOperator::Greater => {
            seed_length_greater_than_fact(program, facts, binary.left, binary.right);
            // `left > right` is `right < left` — the mirrored strict
            // ordering, including a len or `len - k` upper bound.
            seed_less_than_len_fact(program, machine, state, facts, binary.right, binary.left);
            seed_strictly_below_const_binder(
                program,
                machine,
                state,
                facts,
                binary.right,
                binary.left,
                true,
            );
            seed_at_most_fact(program, facts, binary.right, binary.left, true);
            // `left > K` (right a constant) floors `left` at `K + 1`.
            seed_non_negative_fact(program, facts, binary.left, binary.right, false);
        }
        BinaryOperator::LessOrEqual => {
            seed_length_at_least_fact(program, facts, binary.right, binary.left);
            seed_successor_at_most_len_fact(
                program,
                machine,
                state,
                facts,
                binary.left,
                binary.right,
            );
            seed_at_most_len_range_bound_fact(
                program,
                machine,
                state,
                facts,
                binary.left,
                binary.right,
            );
            seed_index_at_most_integer_fact(program, facts, binary.left, binary.right);
            seed_strictly_below_const_binder(
                program,
                machine,
                state,
                facts,
                binary.left,
                binary.right,
                false,
            );
            seed_at_most_fact(program, facts, binary.left, binary.right, false);
            // `K <= right` (left a constant) floors `right` at `K`.
            seed_non_negative_fact(program, facts, binary.right, binary.left, true);
        }
        BinaryOperator::GreaterOrEqual => {
            seed_length_at_least_fact(program, facts, binary.left, binary.right);
            seed_successor_at_most_len_fact(
                program,
                machine,
                state,
                facts,
                binary.right,
                binary.left,
            );
            seed_at_most_len_range_bound_fact(
                program,
                machine,
                state,
                facts,
                binary.right,
                binary.left,
            );
            // `left >= right` is `right <= left` — the mirrored ordering.
            seed_strictly_below_const_binder(
                program,
                machine,
                state,
                facts,
                binary.right,
                binary.left,
                false,
            );
            seed_at_most_fact(program, facts, binary.right, binary.left, false);
            // `left >= K` (right a constant) floors `left` at `K`.
            seed_non_negative_fact(program, facts, binary.left, binary.right, true);
        }
        BinaryOperator::And => {
            seed_guard_facts(program, machine, state, facts, binary.left);
            seed_guard_facts(program, machine, state, facts, binary.right);
        }
        BinaryOperator::Equal => {
            seed_length_equality_fact(program, facts, binary.left, binary.right);
            // `place == K` makes `place` the folded value under the guard —
            // a later `place - 1` index initializer closes through
            // `expression_integer_value` exactly as an assignment fold does.
            seed_equal_integer_fact(program, facts, binary.left, binary.right);
            seed_equal_integer_fact(program, facts, binary.right, binary.left);
            // Equality is `<=` in both directions: `pos == max` gives
            // `pos <= max` for the ordering chain and `max <= pos`, and
            // `pos == K` unfolds to the `pos <= K` integer bound plus the
            // `pos >= K` non-negativity floor.
            seed_at_most_fact(program, facts, binary.left, binary.right, false);
            seed_at_most_fact(program, facts, binary.right, binary.left, false);
            seed_index_at_most_integer_fact(program, facts, binary.left, binary.right);
            seed_index_at_most_integer_fact(program, facts, binary.right, binary.left);
            seed_equal_len_offset_fact(program, machine, state, facts, binary.left, binary.right);
            seed_equal_len_offset_fact(program, machine, state, facts, binary.right, binary.left);
            seed_non_negative_fact(program, facts, binary.left, binary.right, true);
            seed_non_negative_fact(program, facts, binary.right, binary.left, true);
            seed_boolean_equality_guard_facts(
                program,
                machine,
                state,
                facts,
                binary.left,
                binary.right,
            );
            seed_boolean_equality_guard_facts(
                program,
                machine,
                state,
                facts,
                binary.right,
                binary.left,
            );
        }
        BinaryOperator::NotEqual => {
            seed_length_not_zero_fact(program, facts, binary.left, binary.right);
            seed_length_not_zero_fact(program, facts, binary.right, binary.left);
            seed_boolean_inequality_guard_facts(
                program,
                machine,
                state,
                facts,
                binary.left,
                binary.right,
            );
            seed_boolean_inequality_guard_facts(
                program,
                machine,
                state,
                facts,
                binary.right,
                binary.left,
            );
        }
        BinaryOperator::Add
        | BinaryOperator::BitwiseAnd
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor
        | BinaryOperator::Divide
        | BinaryOperator::Modulo
        | BinaryOperator::Multiply
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight
        | BinaryOperator::Subtract
        | BinaryOperator::CaseMembership => {}
    }
}

/// `place == value` under a true guard leaves `place` known-equal to the
/// folded integer: push a shadowing entry the newest-entry lookups see so an
/// expression over the place (a hoisted `place - 1` index initializer)
/// folds like an assignment value would. Only a place operand carries the
/// fact; two folded constants need no entry at all.
fn seed_equal_integer_fact(
    program: &dyn ::validation::CheckingInput,
    facts: &mut RangeFacts<'_>,
    place: ExpressionHandle,
    value: ExpressionHandle,
) {
    let Some(value) = expression_integer_value(program, facts, value) else {
        return;
    };
    match program.expression_table.expression(place) {
        ExpressionNode::Member(member) => {
            facts.define_field_integer(member.member_symbol, member.member.as_str(), value);
        }
        ExpressionNode::Name(_) => {
            if let Some((symbol, name)) = expression_name(program, place) {
                facts.define_local(
                    symbol,
                    name.unwrap_or_default().to_owned(),
                    None,
                    Some(value),
                );
            }
        }
        _ => {}
    }
}

fn seed_boolean_equality_guard_facts(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    facts: &mut RangeFacts<'_>,
    possible_boolean: ExpressionHandle,
    guard: ExpressionHandle,
) {
    match program.expression_table.expression(possible_boolean) {
        ExpressionNode::Boolean(true) => seed_guard_facts(program, machine, state, facts, guard),
        ExpressionNode::Boolean(false) => {
            seed_negated_guard_facts(program, machine, state, facts, guard)
        }
        _ => {}
    }
}

fn seed_boolean_inequality_guard_facts(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    facts: &mut RangeFacts<'_>,
    possible_boolean: ExpressionHandle,
    guard: ExpressionHandle,
) {
    match program.expression_table.expression(possible_boolean) {
        ExpressionNode::Boolean(true) => {
            seed_negated_guard_facts(program, machine, state, facts, guard)
        }
        ExpressionNode::Boolean(false) => seed_guard_facts(program, machine, state, facts, guard),
        _ => {}
    }
}

pub(super) fn seed_negated_guard_facts(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    facts: &mut RangeFacts<'_>,
    guard: ExpressionHandle,
) {
    // Test the actual selected operation, not the complement used below to
    // represent its false branch.
    if !validation::has_builtin_decomposed_guard_meaning(program, machine, Some(state), guard) {
        return;
    }
    // Refuting `!guard` establishes `guard`.
    if let Some(operand) = negated_operand(program, guard) {
        seed_guard_facts(program, machine, state, facts, operand);
        return;
    }
    let Some(binary) = program.expression_table.binary_node(guard) else {
        return;
    };

    let complement = match binary.operator {
        BinaryOperator::Equal => BinaryOperator::NotEqual,
        BinaryOperator::NotEqual => BinaryOperator::Equal,
        BinaryOperator::Less => BinaryOperator::GreaterOrEqual,
        BinaryOperator::LessOrEqual => BinaryOperator::Greater,
        BinaryOperator::Greater => BinaryOperator::LessOrEqual,
        BinaryOperator::GreaterOrEqual => BinaryOperator::Less,
        // Refuting a disjunction refutes both operands. Refuting a conjunction
        // does not identify a failed operand and cannot seed either complement.
        BinaryOperator::Or => {
            seed_negated_guard_facts(program, machine, state, facts, binary.left);
            seed_negated_guard_facts(program, machine, state, facts, binary.right);
            return;
        }
        _ => return,
    };
    seed_binary_guard_facts(program, machine, state, facts, binary, complement);
}
