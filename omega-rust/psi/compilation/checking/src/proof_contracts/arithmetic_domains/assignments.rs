//! Recording assignments and checking narrowing.

use crate::proof_contracts::arithmetic_domains::integer_ranges::{
    is_atomic_type, literal_u64, primitive_name, primitive_range,
    validate_anonymous_integer_primitive_range, validate_value_range,
};
use crate::proof_contracts::arithmetic_domains::interval::Interval;
use crate::proof_contracts::arithmetic_domains::range_constraints::range_constraint_interval;
use crate::proof_contracts::arithmetic_domains::value_environment::ValueEnvironment;
use diagnostics::Diagnostic;
use numerics::arithmetic::ArithmeticDomain;
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
        environment.invalidate_written_paths(std::slice::from_ref(&path));
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
    program: &dyn crate::CheckingInput,
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

pub(crate) struct CapturedFloatAssignment {
    pub(crate) bounds: Option<super::FloatSourceBounds>,
    source_path: Option<String>,
}

pub(super) fn float_source_place_path(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<String> {
    if let Some(path) = super::place_path(program, expression)
        .or_else(|| super::element_place_path(program, expression))
    {
        return Some(path);
    }
    if let ExpressionNode::Match(dispatch) = program.expression_table.expression(expression) {
        let arms = program.expression_table.match_arms(dispatch.arms);
        let first = arms.first()?;
        let candidate = float_source_place_path(program, first.value)?;
        if arms
            .iter()
            .skip(1)
            .all(|arm| float_source_place_path(program, arm.value).as_ref() == Some(&candidate))
        {
            return Some(candidate);
        }
    }
    None
}

/// Capture the completed IEEE value before destination invalidation. Literals,
/// live copies and selected public result guarantees share one enclosure query.
/// A receiving predicate never supplies bounds, and a later write cannot
/// reinterpret this detached value as the source's new contents.
pub(crate) fn capture_float_assignment(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    environment: &ValueEnvironment,
    destination: Option<PrimitiveType>,
    value: ExpressionHandle,
) -> Option<CapturedFloatAssignment> {
    let Some(destination @ (PrimitiveType::F32 | PrimitiveType::F64)) = destination else {
        return None;
    };
    let bounds = if let ExpressionNode::Float(literal) = program.expression_table.expression(value)
    {
        let format = match destination {
            PrimitiveType::F32 => numerics::literals::FloatFormat::F32,
            _ => numerics::literals::FloatFormat::F64,
        };
        if literal.landing().is_some_and(|retained| retained != format) {
            None
        } else {
            let value = literal.with_landing(format).landed_f64();
            (!value.is_nan()).then_some(super::FloatSourceBounds {
                primitive: destination,
                minimum: value,
                maximum: value,
            })
        }
    } else {
        super::float_source_bounds(program, environment, value, &|expression| {
            state
                .and_then(|state| {
                    crate::expression_result_type_reference(program, machine, state, expression)
                })
                .or_else(|| {
                    crate::value_custody::places::declared_place_type_raw(
                        program, machine, state, expression,
                    )
                })
        })
        .filter(|bounds| bounds.primitive == destination)
    };
    let source_path = float_source_place_path(program, value);
    (bounds.is_some() || source_path.is_some()).then_some(CapturedFloatAssignment {
        bounds,
        source_path,
    })
}

/// Publish an already captured value only after retiring old destination facts.
pub(crate) fn record_float_assignment(
    environment: &mut ValueEnvironment,
    path: Option<String>,
    captured: Option<CapturedFloatAssignment>,
) {
    let (Some(path), Some(captured)) = (path, captured) else {
        return;
    };
    if let Some(bounds) = captured.bounds {
        environment.narrow_float(
            path.clone(),
            super::value_environment::FloatInterval {
                low: Some(bounds.minimum),
                high: Some(bounds.maximum),
            },
        );
        environment.mark_non_nan(path.clone());
    }
    if let Some(source_path) = captured.source_path {
        environment.mark_float_alias(path, source_path);
    }
}

/// The declared `[a..=b]` range of a place's type, ONLY when that range is
/// store-enforced (EXACT arithmetic domain; atomics wrap by hardware): the
/// interval an environment entry may soundly be intersected with. A `in Wrapping`
/// place can legitimately hold out-of-range values (its declared range is
/// deliberately permissive at stores), so clamping its precise environment fact
/// against the range would fabricate an in-range claim -- return None there.
pub(crate) fn enforced_declared_range(
    program: &dyn crate::CheckingInput,
    handle: TypeReferenceHandle,
) -> Option<Interval> {
    if is_atomic_type(program, handle)
        || program.arithmetic_domain_for_type_reference(handle) != ArithmeticDomain::Exact
    {
        return None;
    }
    range_constraint_interval(program, handle)
}

/// Spell a carrier for diagnostics: the primitive name plus its non-Exact
/// arithmetic policy (`u32 in Wrapping`); a bare name spells Exact.
fn carrier_spelling(primitive: PrimitiveType, domain: Option<ArithmeticDomain>) -> String {
    match domain {
        Some(domain) if domain != ArithmeticDomain::Exact => {
            format!("{} in {}", primitive_name(primitive), domain.name())
        }
        _ => primitive_name(primitive).to_owned(),
    }
}

/// Supply a stored value's declared integer carrier when interval analysis
/// names none. A selected expression result remains typed even when the
/// interval walk cannot describe it; the destination cannot choose its type.
/// Anonymous values keep ordinary literal landing, and established carrier
/// and policy judgments are never replaced here. Call-result analysis owns
/// recovery of both the carrier and the declared policy from its signature,
/// independently of whether the callee supplies result bounds or `ensures`.
pub(crate) fn stored_value_carrier(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    value: ExpressionHandle,
    analyzed: Option<PrimitiveType>,
    analyzed_domain: Option<ArithmeticDomain>,
) -> (Option<PrimitiveType>, Option<ArithmeticDomain>) {
    if analyzed.is_some()
        || !value.is_valid()
        || crate::value_custody::literals::has_anonymous_numeric_results(program, value)
    {
        return (analyzed, analyzed_domain);
    }
    let Some(reference) = state.and_then(|state| {
        crate::value_custody::expression_types::expression_result_type_reference(
            program, machine, state, value,
        )
    }) else {
        return (analyzed, analyzed_domain);
    };
    let Some(primitive) = program
        .primitive_type_reference(reference)
        .filter(|primitive| primitive_range(*primitive).is_some())
    else {
        return (analyzed, analyzed_domain);
    };
    (Some(primitive), analyzed_domain)
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
///
/// A TYPED value also keeps its own carrier across the store: it does not
/// implicitly take the destination's integer type or arithmetic policy even
/// when its proven range fits (`self.output = self.state & 255` stores a `u32
/// in Wrapping` value into an `i32` slot). Typed values retain their types,
/// policy erasure is explicit, and checked lowering has no conversion row for
/// the mismatch -- so the destination difference is rejected here, naming the
/// `as` cast that spells it. An anonymous value (`source`/`source_domain`
/// `None`) carries no carrier of its own and keeps landing by range.
#[allow(clippy::too_many_arguments)]
pub(crate) fn check_narrowing_assignment(
    target: Option<PrimitiveType>,
    target_domain: ArithmeticDomain,
    value: Interval,
    source: Option<PrimitiveType>,
    source_domain: Option<ArithmeticDomain>,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(primitive) = target else {
        return;
    };
    if let Some(source_primitive) = source
        && (source_primitive != primitive
            || source_domain.is_some_and(|domain| domain != target_domain))
    {
        let target_spelling = carrier_spelling(primitive, Some(target_domain));
        // One carrier diagnostic owns integer policy removal as well. Retain
        // its domain-weakening classification without a second report for the
        // same conversion; semantic/provenance atoms remain separate checks.
        let weakening = if source_domain
            .is_some_and(|domain| domain != ArithmeticDomain::Exact && domain != target_domain)
        {
            " (implicit domain weakening)"
        } else {
            ""
        };
        diagnostics.push(Diagnostic::error(format!(
            "store in {owner} implicitly converts `{}` to `{target_spelling}`{weakening}; a typed \
             value keeps its type and arithmetic policy -- spell the conversion with \
             `as` (for example `expr as {target_spelling}`)",
            carrier_spelling(source_primitive, source_domain),
        )));
        return;
    }
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
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    value: ExpressionHandle,
    target: PrimitiveType,
    target_domain: ArithmeticDomain,
    environment: &ValueEnvironment,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(dispatch) = program.expression_table.match_node(value) {
        for arm in program.expression_table.match_arms(dispatch.arms) {
            check_value_narrowing(
                program,
                machine,
                state,
                arm.value,
                target,
                target_domain,
                environment,
                owner,
                diagnostics,
            );
        }
        return;
    }
    // Anonymous formation is exact at every scalar destination, including
    // aggregate leaves. An unbounded signed analysis endpoint cannot prove
    // that a large value fits, and the destination policy cannot repair it.
    if validate_anonymous_integer_primitive_range(program, target, value, owner, diagnostics)
        .is_some()
    {
        return;
    }
    let mut throwaway = Vec::new();
    let (interval, source, source_domain) = validate_value_range(
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
        let (source, source_domain) =
            stored_value_carrier(program, machine, state, value, source, source_domain);
        check_narrowing_assignment(
            Some(target),
            target_domain,
            interval,
            source,
            source_domain,
            owner,
            diagnostics,
        );
    }
}
