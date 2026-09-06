//! Guard-derived flow facts for arithmetic validation.
//!
//! This module owns entry preconditions, branch/fall-through narrowing,
//! out-parameter postconditions, and incoming-edge fact joins. The value
//! environment and expression analyzer remain separate responsibilities.

use std::collections::BTreeMap;

use super::*;

mod arrivals;
mod parameter_bounds;
pub use arrivals::arrival_integer_expression_bounds;

#[cfg(test)]
mod tests;

/// S4: build a value environment pre-seeded with the integer bounds a machine's
/// `requires` clause places on its parameters (`requires amount <= 100`). Used to
/// seed the ENTRY state's env so param arithmetic with a declared bound stays
/// exact instead of being forced into a domain. Simple `param <OP> literal`
/// comparisons seed intervals; canonical joint addition and subtraction
/// relations additionally seed their existing proof carriers. Other shapes are
/// ignored (sound -- a missing bound just falls back to the type width).
pub(crate) fn requires_value_env(
    program: &TypedTrees,
    machine: &Machine,
    entry_state: &State,
) -> ValueEnv {
    let mut bounds: BTreeMap<String, (Option<i64>, Option<i64>)> = BTreeMap::new();
    for contract in program.machine_contracts(machine) {
        if contract.kind != SignatureContractKind::Requires {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let ProofFact::Expression(expression) = fact else {
                continue;
            };
            if let Some((name, low, high)) = comparison_bound(program, *expression) {
                let entry = bounds.entry(name).or_insert((None, None));
                // Intersect across facts: tightest lower (max) and upper (min).
                if let Some(low) = low {
                    entry.0 = Some(entry.0.map_or(low, |existing| existing.max(low)));
                }
                if let Some(high) = high {
                    entry.1 = Some(entry.1.map_or(high, |existing| existing.min(high)));
                }
            }
        }
    }
    let mut env = ValueEnv::new();
    for (name, (low, high)) in bounds {
        env.set(name, Interval { low, high });
    }
    for contract in program.machine_contracts(machine) {
        if contract.kind != SignatureContractKind::Requires {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let ProofFact::Expression(expression) = fact else {
                continue;
            };
            let ExpressionNode::Binary(comparison) =
                program.expression_table.expression(*expression)
            else {
                continue;
            };
            if let Some((left, right)) =
                joint_add_upper_guard(program, machine, Some(entry_state), &env, comparison)
            {
                env.mark_joint_add_upper_bound(left, right);
            }
            if let Some((left, right)) =
                joint_add_lower_guard(program, machine, Some(entry_state), &env, comparison)
            {
                env.mark_joint_add_lower_bound(left, right);
            }
            if let Some((left, right)) =
                joint_subtract_guard(program, machine, Some(entry_state), comparison)
            {
                env.mark_joint_subtract_bound(left, right);
            }
            if let Some((left, right)) = signed_joint_subtract_lower_guard(
                program,
                machine,
                Some(entry_state),
                &env,
                comparison,
            ) {
                env.mark_signed_joint_subtract_lower_bound(left, right);
            }
            if let Some((left, right)) = signed_joint_subtract_upper_guard(
                program,
                machine,
                Some(entry_state),
                &env,
                comparison,
            ) {
                env.mark_signed_joint_subtract_upper_bound(left, right);
            }
            if let Some((left, right)) =
                joint_multiply_guard(program, machine, Some(entry_state), &env, comparison)
            {
                env.mark_joint_multiply_bound(left, right);
            }
            if let Some((left, right)) = signed_joint_multiply_lower_guard(
                program,
                machine,
                Some(entry_state),
                &env,
                comparison,
            ) {
                env.mark_signed_joint_multiply_lower_bound(left, right);
            }
            if let Some((left, right)) = signed_joint_multiply_upper_guard(
                program,
                machine,
                Some(entry_state),
                &env,
                comparison,
            ) {
                env.mark_signed_joint_multiply_upper_bound(left, right);
            }
        }
    }
    env
}

/// Read a `requires` comparison as `(param_name, lower, upper)` -- one of the
/// bounds is `None` (open). `None` when the fact is not a simple
/// `name <OP> literal` / `literal <OP> name` integer comparison.
pub(super) fn comparison_bound(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<(String, Option<i64>, Option<i64>)> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
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
        match operator {
            BinaryOperator::Less => BinaryOperator::Greater,
            BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
            BinaryOperator::Greater => BinaryOperator::Less,
            BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
            other => other,
        }
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

/// S4 dominating-guard narrowing: a transition arm fires only when its guard
/// holds, so the arm's argument arithmetic can assume that bound. Returns `base`
/// refined by the arm's guard. The desugared arm guard is `<comparison> ==
/// true|false`; the comparison's bound (negated for the `false` arm) is
/// INTERSECTED with the guarded place's type range so a one-sided guard (`n >
/// 0`) keeps the type's other end (else `n - 1` loses its `u32` upper bound).
/// Only simple `place <OP> literal` comparisons narrow; anything else leaves the
/// env unchanged (sound -- the arm's arithmetic then has to prove on its own).
pub(crate) fn guard_narrowed_env(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    guard: &typed_trees::statement::TransitionGuardNode,
    base: &ValueEnv,
) -> ValueEnv {
    use typed_trees::statement::TransitionGuardNode;
    let mut env = base.clone();
    let TransitionGuardNode::When(guard_expr) = guard else {
        return env;
    };
    let ExpressionNode::Binary(equality) = program.expression_table.expression(*guard_expr) else {
        return env;
    };
    if equality.operator != BinaryOperator::Equal {
        return env;
    }
    let ExpressionNode::Boolean(arm_true) = program.expression_table.expression(equality.right)
    else {
        return env;
    };
    narrow_env_by_condition(program, machine, state, &mut env, equality.left, *arm_true);
    env
}

/// S4 fall-through complement (MR2 exact-domain unlock): a guarded
/// transition with a valid target and NO fall-through arm EXITS when its
/// guard holds, so every LATER statement in the state runs under the
/// guard's NEGATION (`transition n == 0 { true -> 7 }` then
/// `-> countdown(n - 1)` may assume n >= 1). Returns `base` refined by the
/// negated guard; same simple-comparison leaves as the arm narrowing.
pub(crate) fn fall_through_narrowed_env(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    guard: &typed_trees::statement::TransitionGuardNode,
    base: &ValueEnv,
) -> ValueEnv {
    use typed_trees::statement::TransitionGuardNode;
    let mut env = base.clone();
    let TransitionGuardNode::When(guard_expr) = guard else {
        return env;
    };
    // The multi-arm desugar wraps `(cmp) == true|false`; a single-arm guard
    // stores the comparison bare. Unwrap when present, else negate the whole
    // expression.
    if let ExpressionNode::Binary(equality) = program.expression_table.expression(*guard_expr)
        && equality.operator == BinaryOperator::Equal
        && let ExpressionNode::Boolean(arm_true) =
            program.expression_table.expression(equality.right)
    {
        narrow_env_by_condition(program, machine, state, &mut env, equality.left, !*arm_true);
        return env;
    }
    narrow_env_by_condition(program, machine, state, &mut env, *guard_expr, false);
    env
}

/// Narrow `env` by a guard condition holding with the given polarity,
/// recursing through the boolean structure: a POSITIVE `a && b` narrows by
/// both conjuncts (each may bound a DIFFERENT place -- `dir >= 0 && dir <= 1`
/// or multi-variable conjunctions both narrow); a NEGATIVE `a || b` narrows by
/// both negated disjuncts (De Morgan). A negative `&&` / positive `||` cannot
/// attribute which side holds, so it leaves the env unchanged (sound). Leaves
/// are the existing simple `place <OP> literal` comparisons.
pub(super) fn narrow_env_by_condition(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    env: &mut ValueEnv,
    condition: ExpressionHandle,
    positive: bool,
) {
    if let ExpressionNode::Unary(unary) = program.expression_table.expression(condition)
        && unary.operator == typed_trees::expression::UnaryOperator::LogicalNot
    {
        narrow_env_by_condition(program, machine, state, env, unary.operand, !positive);
        return;
    }
    let ExpressionNode::Binary(comparison) = program.expression_table.expression(condition) else {
        return;
    };
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
            let operand_positive =
                positive == (value == (comparison.operator == BinaryOperator::Equal));
            narrow_env_by_condition(program, machine, state, env, operand, operand_positive);
            return;
        }
    }
    match comparison.operator {
        BinaryOperator::And if positive => {
            let (left, right) = (comparison.left, comparison.right);
            narrow_env_by_condition(program, machine, state, env, left, true);
            narrow_env_by_condition(program, machine, state, env, right, true);
            return;
        }
        BinaryOperator::Or if !positive => {
            let (left, right) = (comparison.left, comparison.right);
            narrow_env_by_condition(program, machine, state, env, left, false);
            narrow_env_by_condition(program, machine, state, env, right, false);
            return;
        }
        BinaryOperator::And | BinaryOperator::Or => return,
        _ => {}
    }
    let comparison = *comparison;
    // Float facts are independent from the integer interval lattice. A
    // positive self-equality proves non-NaN; a positive ordered comparison
    // proves both non-NaN and its one-sided bound. Negated IEEE comparisons
    // do not yield the complementary bound because NaN makes both ordered
    // directions false.
    if positive {
        if let Some((left, right)) =
            joint_add_upper_guard(program, machine, state, env, &comparison)
        {
            env.mark_joint_add_upper_bound(left, right);
        }
        if let Some((left, right)) =
            joint_add_lower_guard(program, machine, state, env, &comparison)
        {
            env.mark_joint_add_lower_bound(left, right);
        }
        if let Some((left, right)) = joint_subtract_guard(program, machine, state, &comparison) {
            env.mark_joint_subtract_bound(left, right);
        }
        if let Some((left, right)) =
            signed_joint_subtract_lower_guard(program, machine, state, env, &comparison)
        {
            env.mark_signed_joint_subtract_lower_bound(left, right);
        }
        if let Some((left, right)) =
            signed_joint_subtract_upper_guard(program, machine, state, env, &comparison)
        {
            env.mark_signed_joint_subtract_upper_bound(left, right);
        }
        if let Some((left, right)) = joint_multiply_guard(program, machine, state, env, &comparison)
        {
            env.mark_joint_multiply_bound(left, right);
        }
        if let Some((left, right)) =
            signed_joint_multiply_lower_guard(program, machine, state, env, &comparison)
        {
            env.mark_signed_joint_multiply_lower_bound(left, right);
        }
        if let Some((left, right)) =
            signed_joint_multiply_upper_guard(program, machine, state, env, &comparison)
        {
            env.mark_signed_joint_multiply_upper_bound(left, right);
        }
        if let Some(value) =
            signed_joint_multiply_negation_guard(program, machine, state, &comparison)
        {
            env.mark_signed_joint_multiply_negation_bound(value);
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
                    env.mark_known_u64(path, value);
                }
            }
        }
        if comparison.operator == BinaryOperator::Equal
            && let (Some(left), Some(right)) = (
                place_path(program, comparison.left),
                place_path(program, comparison.right),
            )
            && left == right
            && declared_place_type_raw(program, machine, state, comparison.left).is_some_and(
                |handle| {
                    matches!(
                        program.primitive_type_reference(handle),
                        Some(PrimitiveType::F32 | PrimitiveType::F64)
                    )
                },
            )
        {
            env.mark_non_nan(left);
            return;
        }
        let float_sides = if let Some(literal) = float_literal_value(program, comparison.right) {
            Some((comparison.left, literal, true))
        } else {
            float_literal_value(program, comparison.left)
                .map(|literal| (comparison.right, literal, false))
        };
        if let Some((place_expr, literal, name_on_left)) = float_sides
            && literal.is_finite()
            && let Some(handle) = declared_place_type_raw(program, machine, state, place_expr)
            && matches!(
                program.primitive_type_reference(handle),
                Some(PrimitiveType::F32 | PrimitiveType::F64)
            )
            && let Some(name) = place_path(program, place_expr)
            && let Some(mut interval) = float_bound_from(comparison.operator, literal, name_on_left)
        {
            if let Some(declared) = float_range_constraint_interval(program, handle) {
                interval = interval.intersect(declared);
            }
            env.narrow_float(name.clone(), interval);
            env.mark_non_nan(name);
            return;
        }
    }
    parameter_bounds::narrow(program, machine, state, env, condition, positive);
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
    // Identify the (place, constant bound) sides.
    let (place_expr, literal, name_on_left) = if let Some(literal) = integer_bound(comparison.right)
    {
        (comparison.left, literal, true)
    } else if let Some(literal) = integer_bound(comparison.left) {
        (comparison.right, literal, false)
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
    let Some(name) = place_path(program, place_expr) else {
        return;
    };
    if negated_equality {
        // Start from the full line; the intersection below brings in the
        // type + declared ranges, then the point exclusion bumps an end.
        let mut interval = Interval {
            low: None,
            high: None,
        };
        if let Some(handle) = declared_place_type_raw(program, machine, state, place_expr) {
            if let Some(type_interval) = program
                .primitive_type_reference(handle)
                .and_then(primitive_range)
            {
                interval = interval.intersect(type_interval);
            }
            if let Some(declared_range) = range_constraint_interval(program, handle) {
                interval = interval.intersect(declared_range);
            }
        }
        if interval.low == Some(literal) {
            interval.low = literal.checked_add(1);
        }
        if interval.high == Some(literal) {
            interval.high = literal.checked_sub(1);
        }
        env.narrow(name, interval);
        return;
    }
    let (_, low, high) = bound_from(name.clone(), operator, literal, name_on_left);
    let mut interval = Interval { low, high };
    // Intersect with the place's type range AND its declared `[a..=b]` range
    // constraint to retain the bounds the guard leaves open. Skipping the
    // DECLARED range here was a live regression: a one-sided `i < 7` on
    // `i: i32 [0..=7]` seeded [i32::MIN, 6] into the env, which SHADOWS the
    // declared [0, 7] in the operand analysis (env wins over the constraint
    // there) -- `7 - i` then "may overflow" even though it provably cannot.
    if let Some(handle) = declared_place_type_raw(program, machine, state, place_expr) {
        if let Some(type_interval) = program
            .primitive_type_reference(handle)
            .and_then(primitive_range)
        {
            interval = interval.intersect(type_interval);
        }
        if let Some(declared_range) = range_constraint_interval(program, handle) {
            interval = interval.intersect(declared_range);
        }
    }
    // `narrow` intersects with anything already established, so a prior
    // conjunct on the SAME place composes: `dir >= 0 && dir <= 1` lands [0, 1].
    env.narrow(name, interval);
}

/// Recognize the exact guard `left <= MAX - right` (including its `>=`
/// spelling) without pretending either operand has an independent tighter
/// interval. The subtraction is total for unsigned carriers and for signed
/// carriers when the current path already proves `right >= 0`; the comparison
/// is then exactly the upper no-overflow condition for `left + right`.
fn joint_add_upper_guard(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    env: &ValueEnv,
    comparison: &typed_trees::expression::TableBinaryExpression,
) -> Option<(String, String)> {
    let (left, bound) = match comparison.operator {
        BinaryOperator::LessOrEqual => (comparison.left, comparison.right),
        BinaryOperator::GreaterOrEqual => (comparison.right, comparison.left),
        _ => return None,
    };
    let ExpressionNode::Binary(subtract) = program.expression_table.expression(bound) else {
        return None;
    };
    if subtract.operator != BinaryOperator::Subtract {
        return None;
    }
    let left_type = declared_place_type_raw(program, machine, state, left)?;
    let right_type = declared_place_type_raw(program, machine, state, subtract.right)?;
    let left_primitive = program.primitive_type_reference(left_type)?;
    if program.primitive_type_reference(right_type) != Some(left_primitive)
        || program.arithmetic_domain_for_type_reference(left_type) != ArithmeticDomain::Exact
        || program.arithmetic_domain_for_type_reference(right_type) != ArithmeticDomain::Exact
    {
        return None;
    }
    let right_path = place_path(program, subtract.right)?;
    let maximum_matches = match left_primitive {
        PrimitiveType::U8 => literal_i64(program, subtract.left) == Some(u8::MAX as i64),
        PrimitiveType::U16 => literal_i64(program, subtract.left) == Some(u16::MAX as i64),
        PrimitiveType::U32 => literal_i64(program, subtract.left) == Some(u32::MAX as i64),
        PrimitiveType::U64 => known_u64_value(program, env, subtract.left) == Some(u64::MAX),
        PrimitiveType::I8 if env.get(&right_path)?.low? >= 0 => {
            literal_i64(program, subtract.left) == Some(i8::MAX as i64)
        }
        PrimitiveType::I16 if env.get(&right_path)?.low? >= 0 => {
            literal_i64(program, subtract.left) == Some(i16::MAX as i64)
        }
        PrimitiveType::I32 if env.get(&right_path)?.low? >= 0 => {
            literal_i64(program, subtract.left) == Some(i32::MAX as i64)
        }
        PrimitiveType::I64 if env.get(&right_path)?.low? >= 0 => {
            literal_i64(program, subtract.left) == Some(i64::MAX)
        }
        _ => false,
    };
    if !maximum_matches {
        return None;
    }
    Some((place_path(program, left)?, right_path))
}

/// Recognize the signed guard `MIN - right <= left` (including its `>=`
/// spelling) once the current path proves `right <= 0`. The sign fact makes
/// the bound subtraction total, and the comparison is exactly the lower
/// no-underflow condition for `left + right`.
fn joint_add_lower_guard(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    env: &ValueEnv,
    comparison: &typed_trees::expression::TableBinaryExpression,
) -> Option<(String, String)> {
    let (bound, left) = match comparison.operator {
        BinaryOperator::LessOrEqual => (comparison.left, comparison.right),
        BinaryOperator::GreaterOrEqual => (comparison.right, comparison.left),
        _ => return None,
    };
    let ExpressionNode::Binary(subtract) = program.expression_table.expression(bound) else {
        return None;
    };
    if subtract.operator != BinaryOperator::Subtract {
        return None;
    }
    let left_type = declared_place_type_raw(program, machine, state, left)?;
    let right_type = declared_place_type_raw(program, machine, state, subtract.right)?;
    let left_primitive = program.primitive_type_reference(left_type)?;
    if program.primitive_type_reference(right_type) != Some(left_primitive)
        || program.arithmetic_domain_for_type_reference(left_type) != ArithmeticDomain::Exact
        || program.arithmetic_domain_for_type_reference(right_type) != ArithmeticDomain::Exact
    {
        return None;
    }
    let right_path = place_path(program, subtract.right)?;
    let minimum = match left_primitive {
        PrimitiveType::I8 if env.get(&right_path)?.high? <= 0 => i8::MIN as i64,
        PrimitiveType::I16 if env.get(&right_path)?.high? <= 0 => i16::MIN as i64,
        PrimitiveType::I32 if env.get(&right_path)?.high? <= 0 => i32::MIN as i64,
        PrimitiveType::I64 if env.get(&right_path)?.high? <= 0 => i64::MIN,
        _ => return None,
    };
    if literal_i64(program, subtract.left) != Some(minimum) {
        return None;
    }
    Some((place_path(program, left)?, right_path))
}

/// Recognize the unsigned guard `right <= left` (including its `>=` spelling).
/// This ordered relation is exactly the no-underflow condition for
/// `left - right`; unlike addition bounds, its operand order is significant.
fn joint_subtract_guard(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    comparison: &typed_trees::expression::TableBinaryExpression,
) -> Option<(String, String)> {
    let (right, left) = match comparison.operator {
        BinaryOperator::LessOrEqual => (comparison.left, comparison.right),
        BinaryOperator::GreaterOrEqual => (comparison.right, comparison.left),
        _ => return None,
    };
    let left_type = declared_place_type_raw(program, machine, state, left)?;
    let right_type = declared_place_type_raw(program, machine, state, right)?;
    let left_primitive = program.primitive_type_reference(left_type)?;
    if program.primitive_type_reference(right_type) != Some(left_primitive)
        || !matches!(
            left_primitive,
            PrimitiveType::U8 | PrimitiveType::U16 | PrimitiveType::U32 | PrimitiveType::U64
        )
        || program.arithmetic_domain_for_type_reference(left_type) != ArithmeticDomain::Exact
        || program.arithmetic_domain_for_type_reference(right_type) != ArithmeticDomain::Exact
    {
        return None;
    }
    Some((place_path(program, left)?, place_path(program, right)?))
}

/// Recognize the signed guard `MIN + right <= left` (including its `>=`
/// spelling) once the current path proves `right >= 0`. The sign fact makes
/// the bound addition total, and the comparison is exactly the lower
/// representability condition for `left - right`.
fn signed_joint_subtract_lower_guard(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    env: &ValueEnv,
    comparison: &typed_trees::expression::TableBinaryExpression,
) -> Option<(String, String)> {
    let (bound, left) = match comparison.operator {
        BinaryOperator::LessOrEqual => (comparison.left, comparison.right),
        BinaryOperator::GreaterOrEqual => (comparison.right, comparison.left),
        _ => return None,
    };
    let ExpressionNode::Binary(add) = program.expression_table.expression(bound) else {
        return None;
    };
    if add.operator != BinaryOperator::Add {
        return None;
    }
    let (minimum, right) = if literal_i64(program, add.left).is_some() {
        (add.left, add.right)
    } else {
        (add.right, add.left)
    };
    let left_type = declared_place_type_raw(program, machine, state, left)?;
    let right_type = declared_place_type_raw(program, machine, state, right)?;
    let left_primitive = program.primitive_type_reference(left_type)?;
    if program.primitive_type_reference(right_type) != Some(left_primitive)
        || program.arithmetic_domain_for_type_reference(left_type) != ArithmeticDomain::Exact
        || program.arithmetic_domain_for_type_reference(right_type) != ArithmeticDomain::Exact
    {
        return None;
    }
    let right_path = place_path(program, right)?;
    let minimum_value = match left_primitive {
        PrimitiveType::I8 if env.get(&right_path)?.low? >= 0 => i8::MIN as i64,
        PrimitiveType::I16 if env.get(&right_path)?.low? >= 0 => i16::MIN as i64,
        PrimitiveType::I32 if env.get(&right_path)?.low? >= 0 => i32::MIN as i64,
        PrimitiveType::I64 if env.get(&right_path)?.low? >= 0 => i64::MIN,
        _ => return None,
    };
    if literal_i64(program, minimum) != Some(minimum_value) {
        return None;
    }
    Some((place_path(program, left)?, right_path))
}

/// Recognize the signed guard `left <= MAX + right` (including its `>=`
/// spelling) once the current path proves `right <= 0`. The sign fact makes
/// the bound addition total, and the comparison is exactly the upper
/// representability condition for `left - right`.
fn signed_joint_subtract_upper_guard(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    env: &ValueEnv,
    comparison: &typed_trees::expression::TableBinaryExpression,
) -> Option<(String, String)> {
    let (left, bound) = match comparison.operator {
        BinaryOperator::LessOrEqual => (comparison.left, comparison.right),
        BinaryOperator::GreaterOrEqual => (comparison.right, comparison.left),
        _ => return None,
    };
    let ExpressionNode::Binary(add) = program.expression_table.expression(bound) else {
        return None;
    };
    if add.operator != BinaryOperator::Add {
        return None;
    }
    let (maximum, right) = if literal_i64(program, add.left).is_some() {
        (add.left, add.right)
    } else {
        (add.right, add.left)
    };
    let left_type = declared_place_type_raw(program, machine, state, left)?;
    let right_type = declared_place_type_raw(program, machine, state, right)?;
    let left_primitive = program.primitive_type_reference(left_type)?;
    if program.primitive_type_reference(right_type) != Some(left_primitive)
        || program.arithmetic_domain_for_type_reference(left_type) != ArithmeticDomain::Exact
        || program.arithmetic_domain_for_type_reference(right_type) != ArithmeticDomain::Exact
    {
        return None;
    }
    let right_path = place_path(program, right)?;
    let maximum_value = match left_primitive {
        PrimitiveType::I8 if env.get(&right_path)?.high? <= 0 => i8::MAX as i64,
        PrimitiveType::I16 if env.get(&right_path)?.high? <= 0 => i16::MAX as i64,
        PrimitiveType::I32 if env.get(&right_path)?.high? <= 0 => i32::MAX as i64,
        PrimitiveType::I64 if env.get(&right_path)?.high? <= 0 => i64::MAX,
        _ => return None,
    };
    if literal_i64(program, maximum) != Some(maximum_value) {
        return None;
    }
    Some((place_path(program, left)?, right_path))
}

/// Recognize the unsigned guard `left <= MAX / right` (including its `>=`
/// spelling) once the current path proves `right >= 1`. The positive-factor
/// fact makes the bound division defined, and the comparison is exactly the
/// no-overflow condition for `left * right`.
fn joint_multiply_guard(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    env: &ValueEnv,
    comparison: &typed_trees::expression::TableBinaryExpression,
) -> Option<(String, String)> {
    let (left, bound) = match comparison.operator {
        BinaryOperator::LessOrEqual => (comparison.left, comparison.right),
        BinaryOperator::GreaterOrEqual => (comparison.right, comparison.left),
        _ => return None,
    };
    let ExpressionNode::Binary(divide) = program.expression_table.expression(bound) else {
        return None;
    };
    if divide.operator != BinaryOperator::Divide {
        return None;
    }
    let left_type = declared_place_type_raw(program, machine, state, left)?;
    let right_type = declared_place_type_raw(program, machine, state, divide.right)?;
    let left_primitive = program.primitive_type_reference(left_type)?;
    if program.primitive_type_reference(right_type) != Some(left_primitive)
        || !matches!(
            left_primitive,
            PrimitiveType::U8 | PrimitiveType::U16 | PrimitiveType::U32 | PrimitiveType::U64
        )
        || program.arithmetic_domain_for_type_reference(left_type) != ArithmeticDomain::Exact
        || program.arithmetic_domain_for_type_reference(right_type) != ArithmeticDomain::Exact
    {
        return None;
    }
    let right_path = place_path(program, divide.right)?;
    if env.get(&right_path)?.low? < 1 {
        return None;
    }
    let maximum_matches = match left_primitive {
        PrimitiveType::U8 => literal_i64(program, divide.left) == Some(u8::MAX as i64),
        PrimitiveType::U16 => literal_i64(program, divide.left) == Some(u16::MAX as i64),
        PrimitiveType::U32 => literal_i64(program, divide.left) == Some(u32::MAX as i64),
        PrimitiveType::U64 => known_u64_value(program, env, divide.left) == Some(u64::MAX),
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
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    env: &ValueEnv,
    comparison: &typed_trees::expression::TableBinaryExpression,
) -> Option<(String, String)> {
    let (bound, left) = match comparison.operator {
        BinaryOperator::LessOrEqual => (comparison.left, comparison.right),
        BinaryOperator::GreaterOrEqual => (comparison.right, comparison.left),
        _ => return None,
    };
    signed_joint_multiply_quotient_guard(program, machine, state, env, left, bound, true)
}

/// Recognize the carrier-tight upper quotient bound for signed multiplication.
/// A positive factor uses `left <= MAX / right`; a factor at most `-2` uses
/// `left <= MIN / right`.
fn signed_joint_multiply_upper_guard(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    env: &ValueEnv,
    comparison: &typed_trees::expression::TableBinaryExpression,
) -> Option<(String, String)> {
    let (left, bound) = match comparison.operator {
        BinaryOperator::LessOrEqual => (comparison.left, comparison.right),
        BinaryOperator::GreaterOrEqual => (comparison.right, comparison.left),
        _ => return None,
    };
    signed_joint_multiply_quotient_guard(program, machine, state, env, left, bound, false)
}

fn signed_joint_multiply_quotient_guard(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    env: &ValueEnv,
    left: ExpressionHandle,
    bound: ExpressionHandle,
    lower_bound: bool,
) -> Option<(String, String)> {
    let ExpressionNode::Binary(divide) = program.expression_table.expression(bound) else {
        return None;
    };
    if divide.operator != BinaryOperator::Divide {
        return None;
    }
    let left_type = declared_place_type_raw(program, machine, state, left)?;
    let right_type = declared_place_type_raw(program, machine, state, divide.right)?;
    let left_primitive = program.primitive_type_reference(left_type)?;
    if program.primitive_type_reference(right_type) != Some(left_primitive)
        || program.arithmetic_domain_for_type_reference(left_type) != ArithmeticDomain::Exact
        || program.arithmetic_domain_for_type_reference(right_type) != ArithmeticDomain::Exact
    {
        return None;
    }
    let right_path = place_path(program, divide.right)?;
    let interval = env.get(&right_path)?;
    let positive = interval.low.is_some_and(|low| low >= 1);
    let negative = interval.high.is_some_and(|high| high <= -2);
    if !positive && !negative {
        return None;
    }
    let use_minimum = if positive { lower_bound } else { !lower_bound };
    let boundary = match (left_primitive, use_minimum) {
        (PrimitiveType::I8, true) => i8::MIN as i64,
        (PrimitiveType::I16, true) => i16::MIN as i64,
        (PrimitiveType::I32, true) => i32::MIN as i64,
        (PrimitiveType::I64, true) => i64::MIN,
        (PrimitiveType::I8, false) => i8::MAX as i64,
        (PrimitiveType::I16, false) => i16::MAX as i64,
        (PrimitiveType::I32, false) => i32::MAX as i64,
        (PrimitiveType::I64, false) => i64::MAX,
        _ => return None,
    };
    if literal_i64(program, divide.left) != Some(boundary) {
        return None;
    }
    Some((place_path(program, left)?, right_path))
}

/// Recognize `MIN + 1 <= value` (including its `>=` spelling). Together with
/// another operand proved equal to `-1`, this is the exact representability
/// condition for signed negation through multiplication.
fn signed_joint_multiply_negation_guard(
    program: &TypedTrees,
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

/// R4 witness mint (out-params as witnesses): a BOUNDARY callee's
/// `ensures <param> <OP> <literal>` bounds the `&mut` OUT-ARGUMENT's place
/// the moment the call returns -- the boundary model's citable fact
/// (design brief: a boundary machine MINTS facts; ensures are the trusted
/// tier the way requires are the checked tier). Called after the call
/// clears the env; each conjunct that names a signature parameter bound by
/// a literal seeds the matching argument place, intersected with the
/// place's type + declared ranges. Conjunctions split; anything else is
/// skipped (sound -- fewer facts).
pub(crate) fn seed_out_param_ensures(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    call: &typed_trees::statement::TableCall,
    signature: &typed_trees::signature::StateSignature,
    env: &mut ValueEnv,
) {
    use typed_trees::domain::ProofFact;
    use typed_trees::signature::SignatureContractKind;
    let arguments = program.statement_table.expression_handles(call.arguments);
    let parameters: Vec<&typed_trees::signature::StateParameter> = program
        .state_signature_parameters(signature)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect();
    for contract in program
        .signature_contracts
        .span_or_empty(signature.contracts)
    {
        if !matches!(contract.kind, SignatureContractKind::Ensures) {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let ProofFact::Expression(expression) = fact else {
                continue;
            };
            seed_ensures_conjunct(
                program,
                machine,
                state,
                &parameters,
                arguments,
                *expression,
                env,
            );
        }
    }
}

fn seed_ensures_conjunct(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    parameters: &[&typed_trees::signature::StateParameter],
    arguments: &[ExpressionHandle],
    conjunct: ExpressionHandle,
    env: &mut ValueEnv,
) {
    let ExpressionNode::Binary(comparison) = program.expression_table.expression(conjunct) else {
        return;
    };
    if comparison.operator == BinaryOperator::And {
        let (left, right) = (comparison.left, comparison.right);
        seed_ensures_conjunct(program, machine, state, parameters, arguments, left, env);
        seed_ensures_conjunct(program, machine, state, parameters, arguments, right, env);
        return;
    }
    // `param <OP> literal` (param on either side).
    let (param_expr, literal, name_on_left) =
        if let Some(literal) = literal_i64(program, comparison.right) {
            (comparison.left, literal, true)
        } else if let Some(literal) = literal_i64(program, comparison.left) {
            (comparison.right, literal, false)
        } else {
            return;
        };
    let ExpressionNode::Name(path) = program.expression_table.expression(param_expr) else {
        return;
    };
    let [param_name] = program.expression_table.name_path_members(path.members) else {
        return;
    };
    let Some(position) = parameters
        .iter()
        .position(|parameter| parameter.name.as_str() == param_name.as_str())
    else {
        return;
    };
    let Some(argument) = arguments.get(position).copied() else {
        return;
    };
    // The out-argument spells `&mut <place>`; unwrap to the place.
    let place_expr = match program.expression_table.expression(argument) {
        ExpressionNode::Borrow(inner) => inner.target,
        _ => argument,
    };
    let Some(place) = place_path(program, place_expr) else {
        return;
    };
    let (_, low, high) = bound_from(place.clone(), comparison.operator, literal, name_on_left);
    let mut interval = Interval { low, high };
    if let Some(handle) = declared_place_type_raw(program, machine, state, place_expr) {
        if let Some(type_interval) = program
            .primitive_type_reference(handle)
            .and_then(primitive_range)
        {
            interval = interval.intersect(type_interval);
        }
        if let Some(declared_range) = range_constraint_interval(program, handle) {
            interval = interval.intersect(declared_range);
        }
    }
    env.narrow(place, interval);
}

/// Join the evaluated arguments and stable facts of every incoming path.
pub(crate) fn incoming_guard_env(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
) -> ValueEnv {
    let mut environment = arrivals::incoming_environments(program, machine)
        .into_iter()
        .find_map(|(symbol, environment)| (symbol == state.symbol).then_some(environment))
        .unwrap_or_default();
    arrivals::seed_state_requirements(program, machine, state, &mut environment);
    environment
}
