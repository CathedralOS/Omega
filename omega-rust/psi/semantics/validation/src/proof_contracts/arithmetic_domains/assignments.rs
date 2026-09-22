//! Recording assignments and checking narrowing.

use crate::proof_contracts::arithmetic_domains::integer_ranges::{
    is_atomic_type, literal_u64, primitive_name, primitive_range,
    validate_anonymous_integer_primitive_range, validate_value_range,
};
use crate::proof_contracts::arithmetic_domains::interval::Interval;
use crate::proof_contracts::arithmetic_domains::place_paths::place_paths_overlap;
use crate::proof_contracts::arithmetic_domains::range_constraints::range_constraint_interval;
use crate::proof_contracts::arithmetic_domains::value_environment::ValueEnvironment;
use diagnostics::Diagnostic;
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

/// Record an assignment's proven interval into the environment (decision 17 S4).
/// A place whose path cannot be formed (a complex lvalue) just is not tracked.
/// The interval is INTERSECTED with the place's declared `[a..=b]` range before
/// recording: an environment entry SHADOWS the declared-range fallback in the operand
/// analysis, so recording an UNBOUNDED interval (an unresolvable initializer)
/// onto a range-declared place would WIDEN its effective range -- the same
/// landmine as the guard-seeding one (`let __hoist: i32 [0..=9] = cells[k].v`
/// recorded unbounded and `__hoist + 5` "may overflow").
pub(crate) fn record_assignment(
    environment: &mut ValueEnvironment,
    path: Option<String>,
    interval: Interval,
    declared_range: Option<Interval>,
) {
    if let Some(path) = path {
        // A new payload retires float range/non-NaN facts as well as integer
        // facts. Literal recording may establish fresh facts after the store
        // has passed its ordinary validation.
        environment
            .float_intervals
            .retain(|known, _| !place_paths_overlap(known, &path));
        environment
            .non_nan
            .retain(|known| !place_paths_overlap(known, &path));
        environment
            .ordered_values
            .retain(|relation| relation.survives(std::slice::from_ref(&path)));
        environment
            .known_u64_values
            .retain(|known, _| !place_paths_overlap(known, &path));
        environment.joint_add_upper_bounds.retain(|(left, right)| {
            !place_paths_overlap(left, &path) && !place_paths_overlap(right, &path)
        });
        environment.joint_add_lower_bounds.retain(|(left, right)| {
            !place_paths_overlap(left, &path) && !place_paths_overlap(right, &path)
        });
        environment.joint_subtract_bounds.retain(|(left, right)| {
            !place_paths_overlap(left, &path) && !place_paths_overlap(right, &path)
        });
        environment
            .signed_joint_subtract_lower_bounds
            .retain(|(left, right)| {
                !place_paths_overlap(left, &path) && !place_paths_overlap(right, &path)
            });
        environment
            .signed_joint_subtract_upper_bounds
            .retain(|(left, right)| {
                !place_paths_overlap(left, &path) && !place_paths_overlap(right, &path)
            });
        environment.joint_multiply_bounds.retain(|(left, right)| {
            !place_paths_overlap(left, &path) && !place_paths_overlap(right, &path)
        });
        environment
            .signed_joint_multiply_lower_bounds
            .retain(|(left, right)| {
                !place_paths_overlap(left, &path) && !place_paths_overlap(right, &path)
            });
        environment
            .signed_joint_multiply_upper_bounds
            .retain(|(left, right)| {
                !place_paths_overlap(left, &path) && !place_paths_overlap(right, &path)
            });
        environment
            .signed_joint_multiply_negation_bounds
            .retain(|value| !place_paths_overlap(value, &path));
        let interval = match declared_range {
            Some(declared) => interval.intersect(declared),
            None => interval,
        };
        environment.set(path, interval);
    }
}

/// A direct u64 literal binding retains its mathematical value beyond the
/// interval endpoint window. Call only after invalidating the old assignment;
/// the ordinary environment write/arrival rules retire this fact as well.
pub(crate) fn record_unsigned_literal_assignment(
    program: &TypedTrees,
    environment: &mut ValueEnvironment,
    path: Option<String>,
    primitive: Option<PrimitiveType>,
    value: ExpressionHandle,
) {
    if primitive == Some(PrimitiveType::U64)
        && let Some(path) = path
        && let Some(value) = literal_u64(program, value)
    {
        environment.mark_known_u64(path, value);
    }
}

/// The declared `[a..=b]` range of a place's type, ONLY when that range is
/// store-enforced (EXACT arithmetic domain; atomics wrap by hardware): the
/// interval an environment entry may soundly be intersected with. A `in Wrapping`
/// place can legitimately hold out-of-range values (its declared range is
/// deliberately permissive at stores), so clamping its precise environment fact
/// against the range would fabricate an in-range claim -- return None there.
pub(crate) fn enforced_declared_range(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<Interval> {
    if is_atomic_type(program, handle)
        || program.arithmetic_domain_for_type_reference(handle) != ArithmeticDomain::Exact
    {
        return None;
    }
    range_constraint_interval(program, handle)
}

/// Decision 17 at a VALUE-BINDING boundary (`self.f = v`, `let x: T = v`): a
/// value whose proven range does not fit the destination integer type is a
/// SILENT NARROWING (truncation). Storing a wider value into a narrower slot is
/// the same proof obligation as an overflowing `+` -- prove it fits or opt into
/// an explicit `as` cast. Flagged ONLY when the source interval is fully bounded
/// AND provably escapes the target range: an unbounded end (a call result, a
/// param, a `u64` high) stays permissive, exactly as exact arithmetic leaves
/// unbounded unknowns unchecked. A `Cast` re-ranges to the target (its interval
/// is the target's own range), so `v as i32` always fits -- the escape hatch.
/// Signedness falls out for free: `i32 -> u32` is caught on the negative half,
/// `u32 -> i32` on the upper half. `target` is `None` (non-primitive or
/// non-integer destination) => nothing to prove.
pub(crate) fn check_narrowing_assignment(
    target: Option<PrimitiveType>,
    value: Interval,
    source: Option<PrimitiveType>,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(primitive) = target else {
        return;
    };
    let Some(range) = primitive_range(primitive) else {
        return;
    };
    // The value produced by a typed source is always WITHIN that type's range
    // (even a `Wrapping`/`Saturating` result), so intersect the mathematical
    // interval with the source type's range: a flow-proven `[7, 7]` survives
    // (tighter than the type), while a domain-wrapped over-approximation is
    // clamped back to the source type -- which, if it fits the target, is no
    // narrowing at all.
    let effective = match source.and_then(primitive_range) {
        Some(source_range) => value.intersect(source_range),
        None => value,
    };
    // Only a fully-bounded source can be PROVEN out of range; leave an unbounded
    // (unknown) end permissive rather than turn "unknown" into a spurious error.
    if effective.low().is_some() && effective.high().is_some() && !range.contains(effective) {
        diagnostics.push(Diagnostic::error(format!(
            "narrowing store in {owner} may not fit `{}`: the value is not provably \
             in range (decision 17 -- a narrowing store is a proof obligation, like exact \
             arithmetic). Truncate explicitly with an `as` cast, or constrain the source's \
             range.",
            primitive_name(primitive),
        )));
    }
}

/// Report the decision-17 narrowing-store obligation for a `value` flowing into a
/// `target` scalar slot: analyze the value's interval (honoring the flow facts in
/// `environment`) and flag it if it may not fit the target's width. The value's OWN
/// arithmetic obligations are reported by the normal statement walk, so they go to
/// a THROWAWAY buffer here -- only the narrowing check contributes to `diagnostics`.
/// SINGLE SOURCE OF TRUTH for the "does this value fit its typed scalar slot?"
/// obligation, shared by every store position: call/transition arguments,
/// struct-literal field construction, and array-literal elements. Pass the
/// statement `value_environment` for flow-sensitive positions, or `&ValueEnvironment::new()` where
/// no per-statement environment is threaded (construction).
pub(crate) fn check_value_narrowing(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    value: ExpressionHandle,
    target: PrimitiveType,
    environment: &ValueEnvironment,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let ExpressionNode::Match(dispatch) = program.expression_table.expression(value) {
        for arm in program.expression_table.match_arms(dispatch.arms) {
            // Narrowing discards arithmetic diagnostics normally produced by
            // statement validation; dispatch children need their own landing.
            if !matches!(
                program.expression_table.expression(arm.value),
                ExpressionNode::Match(_)
            ) {
                validate_anonymous_integer_primitive_range(
                    program,
                    target,
                    arm.value,
                    owner,
                    diagnostics,
                );
            }
            check_value_narrowing(
                program,
                machine,
                state,
                arm.value,
                target,
                environment,
                owner,
                diagnostics,
            );
        }
        return;
    }
    let mut throwaway = Vec::new();
    let (interval, source) = validate_value_range(
        program,
        machine,
        state,
        value,
        environment,
        Some(target),
        ArithmeticDomain::Exact,
        owner,
        &mut throwaway,
    );
    if throwaway.is_empty() {
        check_narrowing_assignment(Some(target), interval, source, owner, diagnostics);
    }
}
