//! Guard-derived flow facts for arithmetic validation.
//!
//! This module owns entry preconditions, branch/fall-through narrowing,
//! out-parameter postconditions, and incoming-edge fact joins. The value
//! environment and expression analyzer remain separate responsibilities.

use super::{
    ArithmeticDomain, BinaryOperator, ExpressionHandle, ExpressionNode, Interval, Machine,
    PrimitiveType, SignatureContractKind, State, ValueEnvironment, declared_place_type_raw,
    literal_i64, ordered_values, place_path, range_constraint_interval,
};
use std::collections::BTreeMap;

mod arrivals;
mod boolean_values;
use crate::proof_contracts::arithmetic_domains::integer_ranges::{
    known_u64_value, literal_u64, primitive_range,
};
use crate::proof_contracts::bound_expression_meaning as meaning;
pub(super) use boolean_values::BooleanValue;
pub(crate) use boolean_values::{record_boolean_assignment, record_boolean_local};
mod case_facts;
mod parameter_bounds;
pub(super) mod projected_facts;
pub use arrivals::{
    CollectionBoundsObservations, arrival_integer_expression_bounds,
    occurrence_collection_footprint_fits, occurrence_integer_expression_bounds,
    occurrence_integer_expression_fits_range,
};
pub(crate) use arrivals::{
    OccurrenceValueEnvironments, inferred_normal_return_interval, occurrence_value_environment,
    occurrence_value_environments,
};
use projected_facts::ProjectedOperand;

// Sibling Requires readers must recover the comparison from the same exact
// expression whose declaration occurrence is checked by the meaning owner.
pub(super) fn has_builtin_ordering(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> bool {
    let Some(comparison) = program.expression_table.binary_node(expression) else {
        return false;
    };
    meaning::builtin_ordering(program, machine, state, expression, comparison)
        && meaning::folded_constant_is_builtin(program, machine, state, comparison.left)
        && meaning::folded_constant_is_builtin(program, machine, state, comparison.right)
}

pub(super) fn has_builtin_equality(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> bool {
    let Some(comparison) = program.expression_table.binary_node(expression) else {
        return false;
    };
    meaning::builtin_boolean_equality(program, machine, state, expression, comparison)
        && meaning::folded_constant_is_builtin(program, machine, state, comparison.left)
        && meaning::folded_constant_is_builtin(program, machine, state, comparison.right)
}

pub(super) fn has_builtin_bound_arithmetic(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> bool {
    meaning::builtin_arithmetic(program, machine, state, expression)
}

/// S4: build a value environment pre-seeded with the integer bounds a machine's
/// `requires` clause places on its parameters (`requires amount <= 100`). Used to
/// seed the ENTRY state's environment so param arithmetic with a declared bound stays
/// exact instead of being forced into a domain. Simple `param <OP> literal`
/// comparisons seed intervals; canonical joint addition and subtraction
/// relations additionally seed their existing proof carriers. Other shapes are
/// ignored (sound -- a missing bound just falls back to the type width).
pub(crate) fn requires_value_environment(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    entry_state: &State,
) -> ValueEnvironment {
    // Both interval and joint-relation projection must consume the selected
    // comparison, not its token. Other fact kinds retain their existing owners.
    let comparison_meaning_is_available = |expression| {
        let Some(comparison) = program.expression_table.binary_node(expression) else {
            return true;
        };
        match comparison.operator {
            BinaryOperator::Equal | BinaryOperator::NotEqual => {
                has_builtin_equality(program, machine, Some(entry_state), expression)
            }
            BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual => {
                has_builtin_ordering(program, machine, Some(entry_state), expression)
            }
            _ => true,
        }
    };
    let mut bounds: BTreeMap<String, (Option<i64>, Option<i64>)> = BTreeMap::new();
    // Both passes walk the same fact set: Requires-contract expressions whose
    // comparison meaning is available.
    let each_available_comparison = |body: &mut dyn FnMut(ExpressionHandle)| {
        for expression in program.contract_expressions(
            program.machine_contracts(machine).iter(),
            SignatureContractKind::Requires,
        ) {
            if !comparison_meaning_is_available(expression) {
                continue;
            }
            body(expression);
        }
    };
    each_available_comparison(&mut |expression| {
        if let Some((name, low, high)) = comparison_bound(program, expression) {
            let entry = bounds.entry(name).or_insert((None, None));
            // Intersect across facts: tightest lower (max) and upper (min).
            if let Some(low) = low {
                entry.0 = Some(entry.0.map_or(low, |existing| existing.max(low)));
            }
            if let Some(high) = high {
                entry.1 = Some(entry.1.map_or(high, |existing| existing.min(high)));
            }
        }
    });
    let mut environment = ValueEnvironment::new();
    for (name, (low, high)) in bounds {
        environment.set(name, Interval { low, high });
    }
    each_available_comparison(&mut |expression| {
        let Some(comparison) = program.expression_table.binary_node(expression) else {
            return;
        };
        if let Some((left, right)) =
            joint_subtract_guard(program, machine, Some(entry_state), comparison)
        {
            environment.mark_joint_subtract_bound(left, right);
        }
        if let Some((left, right)) = signed_joint_subtract_guard(
            program,
            machine,
            Some(entry_state),
            &environment,
            comparison,
            true,
        ) {
            environment.mark_signed_joint_subtract_lower_bound(left, right);
        }
        if let Some((left, right)) = signed_joint_subtract_guard(
            program,
            machine,
            Some(entry_state),
            &environment,
            comparison,
            false,
        ) {
            environment.mark_signed_joint_subtract_upper_bound(left, right);
        }
        if let Some((left, right)) = joint_multiply_guard(
            program,
            machine,
            Some(entry_state),
            &environment,
            comparison,
        ) {
            environment.mark_joint_multiply_bound(left, right);
        }
        if let Some((left, right)) = signed_joint_multiply_lower_guard(
            program,
            machine,
            Some(entry_state),
            &environment,
            comparison,
        ) {
            environment.mark_signed_joint_multiply_lower_bound(left, right);
        }
        if let Some((left, right)) = signed_joint_multiply_upper_guard(
            program,
            machine,
            Some(entry_state),
            &environment,
            comparison,
        ) {
            environment.mark_signed_joint_multiply_upper_bound(left, right);
        }
    });
    for parameter in program.state_parameters(entry_state) {
        super::record_declared_float_fields(
            program,
            &mut environment,
            parameter.name.as_str(),
            parameter.type_reference,
        );
    }
    arrivals::seed_state_requirements(program, machine, entry_state, None, &mut environment);
    environment
}

/// Read a `requires` comparison as `(param_name, lower, upper)` -- one of the
/// bounds is `None` (open). `None` when the fact is not a simple
/// `name <OP> literal` / `literal <OP> name` integer comparison.
pub(super) fn comparison_bound(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<(String, Option<i64>, Option<i64>)> {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return None;
    };
    if let (Some(name), Some(literal)) = (
        place_path(program, binary.left),
        literal_i64(program, binary.right),
    ) {
        return Some(bound_from(name, binary.operator, literal, true));
    }
    if let (Some(name), Some(literal)) = (
        place_path(program, binary.right),
        literal_i64(program, binary.left),
    ) {
        return Some(bound_from(name, binary.operator, literal, false));
    }
    None
}

/// Convert a single `name <OP> literal` (`name_on_left`) or `literal <OP> name`
/// comparison into a one-sided (or, for `==`, two-sided) bound.
fn bound_from(
    name: String,
    operator: BinaryOperator,
    literal: i64,
    name_on_left: bool,
) -> (String, Option<i64>, Option<i64>) {
    let interval = comparison_interval(
        operator,
        Interval {
            low: Some(literal),
            high: Some(literal),
        },
        name_on_left,
    );
    (name, interval.low, interval.high)
}

/// Project a builtin comparison onto its subject using the other operand's
/// interval. Shared by local guard and modular normal-return range consumers.
pub(super) fn comparison_interval(
    operator: BinaryOperator,
    operand: Interval,
    subject_on_left: bool,
) -> Interval {
    // Normalise to `name <OP> literal` by flipping the operator when the name is
    // on the right.
    let operator = if subject_on_left {
        operator
    } else {
        operator.mirrored()
    };
    let (low, high) = match operator {
        BinaryOperator::LessOrEqual => (None, operand.high),
        BinaryOperator::Less => (None, operand.high.map(|value| value.saturating_sub(1))),
        BinaryOperator::GreaterOrEqual => (operand.low, None),
        BinaryOperator::Greater => (operand.low.map(|value| value.saturating_add(1)), None),
        BinaryOperator::Equal => (operand.low, operand.high),
        _ => (None, None),
    };
    Interval { low, high }
}

/// The comparison operator whose truth is the LOGICAL NEGATION of `operator`
/// (`>` ⟺ `<=`, ...). `None` for `==`/`!=` (their negation has no single-interval
/// bound). Used to narrow a transition's FALSE arm by the negated guard.
fn negate_comparison(operator: BinaryOperator) -> Option<BinaryOperator> {
    Some(match operator {
        BinaryOperator::Less => BinaryOperator::GreaterOrEqual,
        BinaryOperator::LessOrEqual => BinaryOperator::Greater,
        BinaryOperator::Greater => BinaryOperator::LessOrEqual,
        BinaryOperator::GreaterOrEqual => BinaryOperator::Less,
        BinaryOperator::NotEqual => BinaryOperator::Equal,
        _ => return None,
    })
}

/// Facts available while evaluating the right operand of a builtin logical
/// expression. The operator's selected meaning, not its spelling, establishes
/// short-circuit execution. The returned environment never escapes that operand.
pub(crate) fn short_circuit_operand_environment(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    environment: &ValueEnvironment,
) -> Option<ValueEnvironment> {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return None;
    };
    if !matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
        || !meaning::has_builtin_binary_expression_meaning(program, machine, state, expression)
    {
        return None;
    }
    Some(evaluated_condition_environment(
        program,
        machine,
        state,
        binary.left,
        binary.operator == BinaryOperator::And,
        environment,
    ))
}

/// Facts after evaluating a condition, rather than replaying its syntax after
/// all its effects. The expressions contract orders selective operands; the
/// live-fact contract expires only observations preceding overlapping writes.
/// A comparison within one effectful leaf stays conservative about snapshots.
/// A later call-free comparison can establish new facts on the changed storage.
pub(super) fn evaluated_condition_environment(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    condition: ExpressionHandle,
    positive: bool,
    environment: &ValueEnvironment,
) -> ValueEnvironment {
    // Call-free observations need no global call-frame index. Constructing
    // one for every library guard made ordinary package checks repeat the
    // whole program's symbol setup without changing any write answer.
    let frames = crate::machine_calls::calls::expression_has_calls(program, condition)
        .then(|| crate::CallFrameResolver::new(program))
        .flatten();
    evaluated_condition_with_frames(
        program,
        machine,
        state,
        condition,
        positive,
        environment,
        frames.as_ref(),
    )
}

fn evaluated_condition_with_frames(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    condition: ExpressionHandle,
    positive: bool,
    environment: &ValueEnvironment,
    frames: Option<&crate::CallFrameResolver<'_>>,
) -> ValueEnvironment {
    let evaluate = |operand, truth, before: &ValueEnvironment| {
        evaluated_condition_with_frames(program, machine, state, operand, truth, before, frames)
    };
    if let ExpressionNode::Unary(unary) = program.expression_table.expression(condition)
        && unary.operator == typed_trees::expression::UnaryOperator::LogicalNot
    {
        return evaluate(unary.operand, !positive, environment);
    }
    if let Some(binary) = program.expression_table.binary_node(condition) {
        if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
            && meaning::has_builtin_binary_expression_meaning(program, machine, state, condition)
        {
            let continuing_truth = binary.operator == BinaryOperator::And;
            let continuing = evaluate(binary.left, continuing_truth, environment);
            let completed = evaluate(binary.right, positive, &continuing);
            if positive == continuing_truth {
                return completed;
            }
            // False conjunction / true disjunction has two reachable paths.
            // A selected RHS cannot donate its facts to the skipped sibling.
            let skipped = evaluate(binary.left, !continuing_truth, environment);
            return skipped.join(&completed);
        }
        if matches!(
            binary.operator,
            BinaryOperator::Equal | BinaryOperator::NotEqual
        ) && meaning::builtin_boolean_equality(program, machine, state, condition, binary)
        {
            let wrapped = match (
                program.expression_table.expression(binary.left),
                program.expression_table.expression(binary.right),
            ) {
                (_, ExpressionNode::Boolean(value)) => Some((binary.left, *value)),
                (ExpressionNode::Boolean(value), _) => Some((binary.right, *value)),
                _ => None,
            };
            if let Some((operand, value)) = wrapped {
                return evaluate(
                    operand,
                    positive == (value == (binary.operator == BinaryOperator::Equal)),
                    environment,
                );
            }
        }
    }
    let writes = if crate::machine_calls::calls::expression_has_calls(program, condition) {
        frames.and_then(|frames| frames.expression_may_write_paths(machine, condition))
    } else {
        Some(Vec::new())
    };
    let cross_writes = |values: &mut ValueEnvironment| match &writes {
        Some(writes) => values.invalidate_written_paths(writes),
        None => values.invalidate_unknown_writes(program, state),
    };
    let mut selected = environment.clone();
    cross_writes(&mut selected);
    narrow_environment_by_condition(program, machine, state, &mut selected, condition, positive);
    cross_writes(&mut selected);
    selected
}

/// A selected transition arm establishes its whole guard, whether authored as
/// a bare dispatch condition or a Boolean-wrapped subject comparison. The shared
/// condition analysis retains polarity, selected operator meaning, and the
/// intersection with existing bounds. Unknown conditions contribute no facts.
pub(crate) fn guard_narrowed_environment(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    guard: &typed_trees::statement::TransitionGuardNode,
    base: &ValueEnvironment,
) -> ValueEnvironment {
    use typed_trees::statement::TransitionGuardNode;
    let environment = base.clone();
    let TransitionGuardNode::When(condition) = guard else {
        return environment;
    };
    evaluated_condition_environment(program, machine, state, *condition, true, &environment)
}

/// S4 fall-through complement (MR2 exact-domain unlock): a guarded
/// transition with a valid target and NO fall-through arm EXITS when its
/// guard holds, so every LATER statement in the state runs under the
/// guard's NEGATION (`transition n == 0 { true -> 7 }` then
/// `-> countdown(n - 1)` may assume n >= 1). Returns `base` refined by the
/// negated guard; same simple-comparison leaves as the arm narrowing.
pub(crate) fn fall_through_narrowed_environment(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    guard: &typed_trees::statement::TransitionGuardNode,
    base: &ValueEnvironment,
) -> ValueEnvironment {
    use typed_trees::statement::TransitionGuardNode;
    let environment = base.clone();
    let TransitionGuardNode::When(guard_expr) = guard else {
        return environment;
    };
    evaluated_condition_environment(program, machine, state, *guard_expr, false, &environment)
}

/// Narrow `environment` by a guard condition holding with the given polarity,
/// recursing through the boolean structure: a POSITIVE `a && b` narrows by
/// both conjuncts (each may bound a DIFFERENT place -- `dir >= 0 && dir <= 1`
/// or multi-variable conjunctions both narrow); a NEGATIVE `a || b` narrows by
/// both negated disjuncts (De Morgan). A negative `&&` / positive `||` cannot
/// attribute which side holds, so it leaves the environment unchanged (sound). Leaves
/// are the existing simple `place <OP> literal` comparisons.
pub(super) fn narrow_environment_by_condition(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    environment: &mut ValueEnvironment,
    condition: ExpressionHandle,
    positive: bool,
) {
    if boolean_values::select(program, machine, state, environment, condition, positive) {
        return;
    }
    if let ExpressionNode::Unary(unary) = program.expression_table.expression(condition)
        && unary.operator == typed_trees::expression::UnaryOperator::LogicalNot
    {
        narrow_environment_by_condition(
            program,
            machine,
            state,
            environment,
            unary.operand,
            !positive,
        );
        return;
    }
    let Some(comparison) =
        meaning::guard_comparison(program, machine.symbol, condition, &|operand| {
            crate::proof_contracts::bound_expression_meaning::operand_type(
                program, machine, state, operand,
            )
        })
    else {
        return;
    };
    narrow_environment_by_comparison(
        program,
        machine,
        state,
        environment,
        condition,
        &comparison,
        positive,
    );
}

/// Authored guards and implicit value-pattern equality contribute the same
/// facts. `condition` supplies occurrence custody; `comparison` supplies the
/// actual operation and operands, including equality against a saved subject.
/// Callers still invalidate facts across writes after that subject was saved.
pub(super) fn narrow_environment_by_comparison(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    environment: &mut ValueEnvironment,
    condition: ExpressionHandle,
    comparison: &typed_trees::expression::TableBinaryExpression,
    positive: bool,
) {
    if matches!(
        comparison.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) {
        let wrapped = match (
            program.expression_table.expression(comparison.left),
            program.expression_table.expression(comparison.right),
        ) {
            (_, ExpressionNode::Boolean(value)) => Some((comparison.left, *value)),
            (ExpressionNode::Boolean(value), _) => Some((comparison.right, *value)),
            _ => None,
        };
        if let Some((operand, value)) = wrapped {
            if !meaning::builtin_boolean_equality(program, machine, state, condition, comparison) {
                return;
            }
            let operand_positive =
                positive == (value == (comparison.operator == BinaryOperator::Equal));
            narrow_environment_by_condition(
                program,
                machine,
                state,
                environment,
                operand,
                operand_positive,
            );
            return;
        }
    }
    match comparison.operator {
        BinaryOperator::And if positive => {
            let (left, right) = (comparison.left, comparison.right);
            narrow_environment_by_condition(program, machine, state, environment, left, true);
            narrow_environment_by_condition(program, machine, state, environment, right, true);
            return;
        }
        BinaryOperator::Or if !positive => {
            let (left, right) = (comparison.left, comparison.right);
            narrow_environment_by_condition(program, machine, state, environment, left, false);
            narrow_environment_by_condition(program, machine, state, environment, right, false);
            return;
        }
        BinaryOperator::And | BinaryOperator::Or => return,
        _ => {}
    }
    // CASE-CONSTRAINTS: a selected arm whose guard proves `subject in
    // Type::Case` establishes that case's `where` facts on the subject for
    // everything the arm evaluates -- dispatch proved the case, so its facts
    // are premises at this point. Both spellings reach here: the generated
    // tag test (`CaseMembership`) and the membership/`Type::Case` arm
    // desugar, which lowers to `Equal` against a case classifier. The
    // membership's own legality is checked elsewhere; a negated membership
    // contributes nothing (case exclusion is coverage work, not a fact
    // premise), and a classifierless equality keeps its ordinary path.
    let membership_subject = match comparison.operator {
        BinaryOperator::CaseMembership => Some(comparison.left),
        BinaryOperator::Equal
            if meaning::exact_case_reference_owner(program, comparison.right).is_some() =>
        {
            Some(comparison.left)
        }
        _ => None,
    };
    if positive && let Some(subject) = membership_subject {
        case_facts::contribute_case_where_facts(
            program,
            machine,
            state,
            environment,
            subject,
            comparison.right,
        );
        return;
    }
    // An authored comparison does not imply the primitive comparison
    // relation, even when one operand happens to be a literal or singleton.
    // Gate every fact consumer below, not only immutable-parameter projection.
    let folded_operands_are_builtin = || {
        meaning::folded_constant_is_builtin(program, machine, state, comparison.left)
            && meaning::folded_constant_is_builtin(program, machine, state, comparison.right)
    };
    if matches!(
        comparison.operator,
        BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
    ) && !(meaning::builtin_ordering(program, machine, state, condition, comparison)
        && folded_operands_are_builtin())
        && !meaning::sealed_float_comparison(program, machine, state, condition, comparison)
    {
        return;
    }
    if matches!(
        comparison.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) && !(meaning::builtin_boolean_equality(program, machine, state, condition, comparison)
        && folded_operands_are_builtin())
        && !meaning::sealed_float_comparison(program, machine, state, condition, comparison)
    {
        return;
    }
    let comparison = *comparison;
    if let Some(state) = state {
        ordered_values::record(program, machine, state, environment, &comparison, positive);
    }
    // Float facts are independent from the integer interval lattice. A
    // positive self-equality proves non-NaN; a positive ordered comparison
    // proves both non-NaN and its one-sided bound. Negated IEEE comparisons
    // do not yield the complementary bound because NaN makes both ordered
    // directions false.
    if positive {
        if let Some((left, right)) = joint_subtract_guard(program, machine, state, &comparison) {
            environment.mark_joint_subtract_bound(left, right);
        }
        if let Some((left, right)) =
            signed_joint_subtract_guard(program, machine, state, environment, &comparison, true)
        {
            environment.mark_signed_joint_subtract_lower_bound(left, right);
        }
        if let Some((left, right)) =
            signed_joint_subtract_guard(program, machine, state, environment, &comparison, false)
        {
            environment.mark_signed_joint_subtract_upper_bound(left, right);
        }
        if let Some((left, right)) =
            joint_multiply_guard(program, machine, state, environment, &comparison)
        {
            environment.mark_joint_multiply_bound(left, right);
        }
        if let Some((left, right)) =
            signed_joint_multiply_lower_guard(program, machine, state, environment, &comparison)
        {
            environment.mark_signed_joint_multiply_lower_bound(left, right);
        }
        if let Some((left, right)) =
            signed_joint_multiply_upper_guard(program, machine, state, environment, &comparison)
        {
            environment.mark_signed_joint_multiply_upper_bound(left, right);
        }
        if let Some(value) =
            signed_joint_multiply_negation_guard(program, machine, state, &comparison)
        {
            environment.mark_signed_joint_multiply_negation_bound(value);
        }
        if comparison.operator == BinaryOperator::Equal {
            for (place, literal) in [
                (comparison.left, comparison.right),
                (comparison.right, comparison.left),
            ] {
                let Some(path) = place_path(program, place) else {
                    continue;
                };
                let Some(value) = literal_u64(program, literal) else {
                    continue;
                };
                if declared_place_type_raw(program, machine, state, place).is_some_and(|handle| {
                    program.primitive_type_reference(handle) == Some(PrimitiveType::U64)
                }) {
                    environment.mark_known_u64(path, value);
                }
            }
        }
        if super::float_arithmetic::record_float_comparison_bounds(
            program,
            environment,
            &comparison,
            &|place| declared_place_type_raw(program, machine, state, place),
        ) {
            return;
        }
    }
    parameter_bounds::narrow(program, machine, state, environment, condition, positive);
    // An immutable singleton parameter is the same integer at every
    // evaluation, so it supplies the literal-equivalent bound without reading
    // an initializer or borrowing facts from another state's same-spelled name.
    let integer_bound = |expression| {
        literal_i64(program, expression).or_else(|| {
            let (low, high) =
                super::immutable_integer_expression_bounds(program, machine, state?, expression)?;
            (low == high).then_some(low)
        })
    };
    // Identify the (place, bound) sides: a constant bound, or else another
    // place whose standing interval holds at every read (`y <= self.max_y`
    // with `where max_y <= 2` bounds `y` by 2). An integer const or value
    // binder is one of those places: its `requires` ceiling stands at every
    // read, so `index < Bound` narrows `index` through that ceiling.
    let standing_bound = |bound: ExpressionHandle, place: ExpressionHandle| {
        place_path(program, place)?;
        super::standing_integer_interval(program, machine, state?, bound)
    };
    let (place_expr, bound, name_on_left) = if let Some(literal) = integer_bound(comparison.right) {
        (comparison.left, Interval::constant(literal), true)
    } else if let Some(literal) = integer_bound(comparison.left) {
        (comparison.right, Interval::constant(literal), false)
    } else if let Some(bound) = standing_bound(comparison.right, comparison.left) {
        (comparison.left, bound, true)
    } else if let Some(bound) = standing_bound(comparison.left, comparison.right) {
        (comparison.right, bound, false)
    } else {
        return;
    };
    // A negative arm narrows by the NEGATED comparison. A negated EQUALITY
    // is a point exclusion: it tightens an interval only when an END sits
    // exactly on the excluded literal (`n == 0` refuted with `n: u64` gives
    // n >= 1), handled below after the type/declared intersection.
    let negated_equality = !positive && comparison.operator == BinaryOperator::Equal;
    let operator = if positive || negated_equality {
        comparison.operator
    } else {
        let Some(negated) = negate_comparison(comparison.operator) else {
            return;
        };
        negated
    };
    // A stable element read narrows under its element key, so a later read
    // of the same element sees the guard's facts.
    let Some(name) = place_path(program, place_expr)
        .or_else(|| super::stable_element_place_path(program, machine, state, place_expr))
    else {
        return;
    };
    if negated_equality {
        // A point exclusion needs a single excluded value.
        let Some(literal) = bound.low.filter(|low| bound.high == Some(*low)) else {
            return;
        };
        // Start from the full line; the intersection below brings in the
        // type + declared ranges, then the point exclusion bumps an end.
        let mut interval = Interval {
            low: None,
            high: None,
        };
        intersect_declared_place_ranges(program, machine, state, place_expr, &mut interval);
        if interval.low == Some(literal) {
            interval.low = literal.checked_add(1);
        }
        if interval.high == Some(literal) {
            interval.high = literal.checked_sub(1);
        }
        environment.narrow(name, interval);
        return;
    }
    let mut interval = comparison_interval(operator, bound, name_on_left);
    // Intersect with the place's type range AND its declared `[a..=b]` range
    // constraint to retain the bounds the guard leaves open. Skipping the
    // DECLARED range here was a live regression: a one-sided `i < 7` on
    // `i: i32 [0..=7]` seeded [i32::MIN, 6] into the environment, which SHADOWS the
    // declared [0, 7] in the operand analysis (environment wins over the constraint
    // there) -- `7 - i` then "may overflow" even though it provably cannot.
    intersect_declared_place_ranges(program, machine, state, place_expr, &mut interval);
    // `narrow` intersects with anything already established, so a prior
    // conjunct on the SAME place composes: `dir >= 0 && dir <= 1` lands [0, 1].
    environment.narrow(name, interval);
}

/// Intersect `interval` with the place's primitive type range and its
/// declared `[a..=b]` range constraint, retaining the bounds the guard
/// leaves open.
fn intersect_declared_place_ranges(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    place_expr: ExpressionHandle,
    interval: &mut Interval,
) {
    let Some(handle) = declared_place_type_raw(program, machine, state, place_expr) else {
        return;
    };
    if let Some(type_interval) = program
        .primitive_type_reference(handle)
        .and_then(primitive_range)
    {
        *interval = interval.intersect(type_interval);
    }
    if let Some(declared_range) = range_constraint_interval(program, handle) {
        *interval = interval.intersect(declared_range);
    }
}

/// Both declared operand types of a guard pair must name the same primitive
/// with Exact arithmetic; yields that primitive.
fn shared_exact_primitive(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> Option<PrimitiveType> {
    let left_type = declared_place_type_raw(program, machine, state, left)?;
    let right_type = declared_place_type_raw(program, machine, state, right)?;
    let left_primitive = program.primitive_type_reference(left_type)?;
    if program.primitive_type_reference(right_type) != Some(left_primitive)
        || program.arithmetic_domain_for_type_reference(left_type) != ArithmeticDomain::Exact
        || program.arithmetic_domain_for_type_reference(right_type) != ArithmeticDomain::Exact
    {
        return None;
    }
    Some(left_primitive)
}

/// Peel `bound` to a built-in exact-domain binary of the given operator whose
/// right operand shares `left`'s declared primitive; yields the binary node
/// and that primitive.
fn exact_bound_binary<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    left: ExpressionHandle,
    bound: ExpressionHandle,
    operator: BinaryOperator,
) -> Option<(
    &'program typed_trees::expression::TableBinaryExpression,
    PrimitiveType,
)> {
    let Some(binary) = program.expression_table.binary_node(bound) else {
        return None;
    };
    if binary.operator != operator || !meaning::builtin_arithmetic(program, machine, state, bound) {
        return None;
    }
    let left_primitive = shared_exact_primitive(program, machine, state, left, binary.right)?;
    Some((binary, left_primitive))
}

/// `<=` and `>=` orderings reduced to one directed operand pair: the tuple
/// reads (directed-left, directed-right) as if the comparison were always `<=`.
fn directed_comparison_operands(
    comparison: &typed_trees::expression::TableBinaryExpression,
) -> Option<(ExpressionHandle, ExpressionHandle)> {
    match comparison.operator {
        BinaryOperator::LessOrEqual => Some((comparison.left, comparison.right)),
        BinaryOperator::GreaterOrEqual => Some((comparison.right, comparison.left)),
        _ => None,
    }
}

/// Recognize the unsigned guard `right <= left` (including its `>=` spelling).
/// This ordered relation is exactly the no-underflow condition for
/// `left - right`; unlike addition bounds, its operand order is significant.
fn joint_subtract_guard(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    comparison: &typed_trees::expression::TableBinaryExpression,
) -> Option<(String, String)> {
    let (right, left) = match comparison.operator {
        BinaryOperator::LessOrEqual => (comparison.left, comparison.right),
        BinaryOperator::GreaterOrEqual => (comparison.right, comparison.left),
        _ => return None,
    };
    let left_primitive = shared_exact_primitive(program, machine, state, left, right)?;
    if !matches!(
        left_primitive,
        typed_trees::unsigned_integer_primitive_pattern!()
    ) {
        return None;
    }
    Some((place_path(program, left)?, place_path(program, right)?))
}

/// Recognize the signed guard `MIN + right <= left` (including its `>=`
/// spelling) once the current path proves `right >= 0`. The sign fact makes
/// the bound addition total, and the comparison is exactly the lower
/// Recognize the signed guards `MIN + right <= left` and
/// `left <= MAX + right` (including their `>=` spellings): the lower form
/// needs the current path to prove `right >= 0`, the upper `right <= 0`.
/// The sign fact makes the bound addition total, and the comparison is
/// exactly that side's representability condition for the subtraction.
fn signed_joint_subtract_guard(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    environment: &ValueEnvironment,
    comparison: &typed_trees::expression::TableBinaryExpression,
    lower: bool,
) -> Option<(String, String)> {
    let (bound, left) = match (comparison.operator, lower) {
        (BinaryOperator::LessOrEqual, true) => (comparison.left, comparison.right),
        (BinaryOperator::GreaterOrEqual, true) => (comparison.right, comparison.left),
        (BinaryOperator::LessOrEqual, false) => (comparison.right, comparison.left),
        (BinaryOperator::GreaterOrEqual, false) => (comparison.left, comparison.right),
        _ => return None,
    };
    let Some(add) = program.expression_table.binary_node(bound) else {
        return None;
    };
    if add.operator != BinaryOperator::Add
        || !meaning::builtin_arithmetic(program, machine, state, bound)
    {
        return None;
    }
    let (extreme_literal, right) = if literal_i64(program, add.left).is_some() {
        (add.left, add.right)
    } else {
        (add.right, add.left)
    };
    let left_primitive = shared_exact_primitive(program, machine, state, left, right)?;
    let right_path = place_path(program, right)?;
    let interval = environment.get(&right_path)?;
    if if lower {
        !(interval.low? >= 0)
    } else {
        !(interval.high? <= 0)
    } {
        return None;
    }
    let Some(extreme) = signed_primitive_bound(left_primitive, lower) else {
        return None;
    };
    if literal_i64(program, extreme_literal) != Some(extreme) {
        return None;
    }
    Some((place_path(program, left)?, right_path))
}

/// Recognize the unsigned guard `left <= MAX / right` (including its `>=`
/// spelling) once the current path proves `right >= 1`. The positive-factor
/// fact makes the bound division defined, and the comparison is exactly the
/// no-overflow condition for `left * right`.
fn joint_multiply_guard(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    environment: &ValueEnvironment,
    comparison: &typed_trees::expression::TableBinaryExpression,
) -> Option<(String, String)> {
    let (left, bound) = directed_comparison_operands(comparison)?;
    let (divide, left_primitive) =
        exact_bound_binary(program, machine, state, left, bound, BinaryOperator::Divide)?;
    if !matches!(
        left_primitive,
        typed_trees::unsigned_integer_primitive_pattern!()
    ) {
        return None;
    }
    let right_path = place_path(program, divide.right)?;
    if environment.get(&right_path)?.low? < 1 {
        return None;
    }
    let maximum_matches = match left_primitive {
        PrimitiveType::U8 => literal_i64(program, divide.left) == Some(u8::MAX as i64),
        PrimitiveType::U16 => literal_i64(program, divide.left) == Some(u16::MAX as i64),
        PrimitiveType::U32 => literal_i64(program, divide.left) == Some(u32::MAX as i64),
        PrimitiveType::U64 => known_u64_value(program, environment, divide.left) == Some(u64::MAX),
        _ => false,
    };
    if !maximum_matches {
        return None;
    }
    Some((place_path(program, left)?, right_path))
}

/// Recognize the carrier-tight lower quotient bound for signed multiplication.
/// A positive factor uses `MIN / right <= left`; a factor at most `-2` uses
/// `MAX / right <= left` because dividing reverses the product inequalities.
fn signed_joint_multiply_lower_guard(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    environment: &ValueEnvironment,
    comparison: &typed_trees::expression::TableBinaryExpression,
) -> Option<(String, String)> {
    let (bound, left) = directed_comparison_operands(comparison)?;
    signed_joint_multiply_quotient_guard(program, machine, state, environment, left, bound, true)
}

/// Recognize the carrier-tight upper quotient bound for signed multiplication.
/// A positive factor uses `left <= MAX / right`; a factor at most `-2` uses
/// `left <= MIN / right`.
fn signed_joint_multiply_upper_guard(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    environment: &ValueEnvironment,
    comparison: &typed_trees::expression::TableBinaryExpression,
) -> Option<(String, String)> {
    let (left, bound) = directed_comparison_operands(comparison)?;
    signed_joint_multiply_quotient_guard(program, machine, state, environment, left, bound, false)
}

fn signed_joint_multiply_quotient_guard(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    environment: &ValueEnvironment,
    left: ExpressionHandle,
    bound: ExpressionHandle,
    lower_bound: bool,
) -> Option<(String, String)> {
    let (divide, left_primitive) =
        exact_bound_binary(program, machine, state, left, bound, BinaryOperator::Divide)?;
    let right_path = place_path(program, divide.right)?;
    let interval = environment.get(&right_path)?;
    let positive = interval.low.is_some_and(|low| low >= 1);
    let negative = interval.high.is_some_and(|high| high <= -2);
    if !positive && !negative {
        return None;
    }
    let use_minimum = if positive { lower_bound } else { !lower_bound };
    let Some(boundary) = signed_primitive_bound(left_primitive, use_minimum) else {
        return None;
    };
    if literal_i64(program, divide.left) != Some(boundary) {
        return None;
    }
    Some((place_path(program, left)?, right_path))
}

/// Recognize `MIN + 1 <= value` (including its `>=` spelling). Together with
/// another operand proved equal to `-1`, this is the exact representability
/// condition for signed negation through multiplication.
/// The i64 extreme of a signed primitive — `minimum` selects MIN, else MAX.
fn signed_primitive_bound(primitive: PrimitiveType, minimum: bool) -> Option<i64> {
    Some(match (primitive, minimum) {
        (PrimitiveType::I8, true) => i8::MIN as i64,
        (PrimitiveType::I16, true) => i16::MIN as i64,
        (PrimitiveType::I32, true) => i32::MIN as i64,
        (PrimitiveType::I64, true) => i64::MIN,
        (PrimitiveType::I8, false) => i8::MAX as i64,
        (PrimitiveType::I16, false) => i16::MAX as i64,
        (PrimitiveType::I32, false) => i32::MAX as i64,
        (PrimitiveType::I64, false) => i64::MAX,
        _ => return None,
    })
}

fn signed_joint_multiply_negation_guard(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    comparison: &typed_trees::expression::TableBinaryExpression,
) -> Option<String> {
    let (boundary, value) = match comparison.operator {
        BinaryOperator::LessOrEqual => (comparison.left, comparison.right),
        BinaryOperator::GreaterOrEqual => (comparison.right, comparison.left),
        _ => return None,
    };
    let value_type = declared_place_type_raw(program, machine, state, value)?;
    let primitive = program.primitive_type_reference(value_type)?;
    if program.arithmetic_domain_for_type_reference(value_type) != ArithmeticDomain::Exact {
        return None;
    }
    let minimum_plus_one = match primitive {
        PrimitiveType::I8 => i8::MIN as i64 + 1,
        PrimitiveType::I16 => i16::MIN as i64 + 1,
        PrimitiveType::I32 => i32::MIN as i64 + 1,
        PrimitiveType::I64 => i64::MIN + 1,
        _ => return None,
    };
    if literal_i64(program, boundary) != Some(minimum_plus_one) {
        return None;
    }
    place_path(program, value)
}

/// Boundary out-parameter witnesses: a BOUNDARY callee's `ensures` facts
/// about its `&mut` formals hold on the caller's OUT-ARGUMENT places the
/// moment the call returns -- the boundary model's citable fact (design
/// brief: a boundary machine MINTS facts; ensures are the trusted tier the
/// way requires are the checked tier). Called after the call clears the
/// environment. Conjunctions split; each comparison whose sides are
/// reference formals or literals is projected onto the argument places and
/// contributed like a guard, so `size == cap && cap <= 8` relates the two
/// argument places and bounds the second. Anything else is skipped (sound --
/// fewer facts).
pub(crate) fn seed_out_param_ensures(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    call: &typed_trees::statement::TableCall,
    signature: &typed_trees::signature::StateSignature,
    environment: &mut ValueEnvironment,
) {
    use typed_trees::signature::SignatureContractKind;
    let arguments = program.statement_table.expression_handles(call.arguments);
    let parameters: Vec<&typed_trees::signature::StateParameter> = program
        .state_signature_parameters(signature)
        .iter()
        .collect();
    let projection = EnsuresProjection {
        program,
        machine,
        state,
        parameters: &parameters,
        arguments,
    };
    for contract in program
        .signature_contracts
        .span_or_empty(signature.contracts)
    {
        if !matches!(contract.kind, SignatureContractKind::Ensures) {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let Some(expression) = fact.as_expression() else {
                continue;
            };
            projection.seed_conjunct(*expression, environment);
        }
    }
}

/// A boundary call's formals mapped onto its caller's arguments.
struct EnsuresProjection<'a> {
    program: &'a dyn crate::CheckingInput,
    machine: &'a Machine,
    state: Option<&'a State>,
    parameters: &'a [&'a typed_trees::signature::StateParameter],
    arguments: &'a [ExpressionHandle],
}

impl EnsuresProjection<'_> {
    fn seed_conjunct(&self, conjunct: ExpressionHandle, environment: &mut ValueEnvironment) {
        let Some(comparison) = self.program.expression_table.binary_node(conjunct) else {
            return;
        };
        if comparison.operator == BinaryOperator::And {
            self.seed_conjunct(comparison.left, environment);
            self.seed_conjunct(comparison.right, environment);
            return;
        }
        let (Some(left), Some(right)) = (self.side(comparison.left), self.side(comparison.right))
        else {
            return;
        };
        projected_facts::contribute_projected_comparison(
            self.program,
            environment,
            comparison.operator,
            (&left, comparison.left),
            (&right, comparison.right),
        );
    }

    /// A literal stays a literal; a reference formal names its out-argument
    /// place. A value formal denotes the argument snapshot, not the caller's
    /// live storage after other arguments or the callee may have changed it,
    /// so only a reference formal can publish a postcondition about that
    /// storage.
    fn side(&self, expression: ExpressionHandle) -> Option<ProjectedOperand> {
        let program = self.program;
        if literal_i64(program, expression).is_some() {
            return Some(match program.expression_table.expression(expression) {
                ExpressionNode::Integer(literal) => ProjectedOperand::literal(literal),
                _ => ProjectedOperand {
                    operand: None,
                    spelled: None,
                    declared_type: None,
                },
            });
        }
        let path = program.expression_table.name_path_node(expression)?;
        let [parameter_name] = program.expression_table.name_path_members(path.members) else {
            return None;
        };
        let position = self
            .parameters
            .iter()
            .position(|parameter| parameter.name.as_str() == parameter_name.as_str())?;
        crate::reference_shell(program, self.parameters[position].type_reference)?;
        let argument = self.arguments.get(position).copied()?;
        // The out-argument spells `&mut <place>`; unwrap to the place.
        let place = match program.expression_table.expression(argument) {
            ExpressionNode::Borrow(inner) => inner.target,
            _ => argument,
        };
        let spelled = place_path(program, place)?;
        let declared_type = declared_place_type_raw(program, self.machine, self.state, place);
        let operand = self
            .state
            .filter(|_| {
                declared_type.is_some_and(|declared| {
                    projected_facts::has_fixed_integer_meaning(program, declared)
                })
            })
            .and_then(|state| ordered_values::operand(program, self.machine, state, place, None));
        Some(ProjectedOperand {
            operand,
            spelled: Some(spelled),
            declared_type,
        })
    }
}

/// Join the evaluated arguments and stable facts of every incoming path.
/// Every state's incoming guard environment. `frames` is the caller's
/// program-wide resolver: building one costs a symbol table per machine, so a
/// resolver per queried machine made validation quadratic in machine count.
/// Frozen statement validation and individual expression queries share this
/// completed batch. Neither consumer mutates it: each takes its own state
/// environment before applying statement-local effects.
pub(crate) fn incoming_guard_environments(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    frames: Option<&crate::CallFrameResolver>,
) -> std::sync::Arc<Vec<(symbols::SymbolHandle, ValueEnvironment)>> {
    // Resolver-free queries remain conservative and cannot populate a batch
    // computed under the program's own complete-or-opaque call-frame law.
    let memos = frames.is_some().then(|| program.checking_memos()).flatten();
    if let Some(cached) = memos.and_then(|memos| {
        memos
            .incoming_guard_environments
            .lock()
            .ok()
            .and_then(|environments| environments.get(&machine.symbol).cloned())
    }) {
        return cached;
    }
    let mut environments = arrivals::incoming_environments(program, machine, frames, None);
    for (state, (_, environment)) in program
        .machine_states(machine)
        .iter()
        .zip(&mut environments)
    {
        arrivals::seed_state_requirements(program, machine, state, frames, environment);
    }
    let environments = std::sync::Arc::new(environments);
    // Publish only after all arrivals and requirements are complete. The
    // mutex is never held while recursive source queries compute the batch.
    if let Some(memos) = memos
        && let Ok(mut cached) = memos.incoming_guard_environments.lock()
    {
        cached.insert(machine.symbol, environments.clone());
    }
    environments
}

/// Query one state without retaining a whole-machine validation batch.
pub(crate) fn incoming_guard_environment(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    frames: Option<&crate::CallFrameResolver>,
) -> ValueEnvironment {
    // A frozen input shares one seeded batch per machine; every state query
    // otherwise rebuilt all of the machine's environments to keep one. The
    // batch is keyed to the program's own resolver, so a query without one
    // computes directly.
    if frames.is_some() && program.checking_memos().is_some() {
        let environments = incoming_guard_environments(program, machine, frames);
        if let Some((_, environment)) = environments
            .iter()
            .find(|(symbol, _)| *symbol == state.symbol)
        {
            return environment.clone();
        }
    }
    let mut environment = arrivals::incoming_environments(program, machine, frames, None)
        .into_iter()
        .find_map(|(symbol, environment)| (symbol == state.symbol).then_some(environment))
        .unwrap_or_default();
    arrivals::seed_state_requirements(program, machine, state, frames, &mut environment);
    environment
}
