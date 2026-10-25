//! Place reads (`x`, `self.field`, `self.cells[i]`): the declared type gives the
//! domain and the integer primitive; the value range is the flow-tracked
//! interval (S4) if one is proven on this linear path, else the primitive's
//! full range.

use super::{Analysis, ExpressionWalk, NEUTRAL};
use crate::proof_contracts::arithmetic_domains::integer_ranges::{is_atomic_type, primitive_range};
use crate::proof_contracts::arithmetic_domains::{
    ArithmeticDomain, ExpressionHandle, Interval, place_path, range_constraint_interval,
};
use crate::value_custody::places;

/// An INDEXED read (`self.cells[rp]`) resolves through its collection's ELEMENT
/// type, so a range-refined element (`[i32 [0..=7]; N]`) feeds the overflow
/// proof exactly like a range-refined field (writes into elements are
/// range-enforced by the proof side, and ZII requires 0 in the element range --
/// the interval is a true invariant).
pub(super) fn analyze(walk: &ExpressionWalk, expression: ExpressionHandle) -> Analysis {
    let program = walk.program;
    // A literal-index element read (`self.cells[3]`) can carry a flow fact of
    // its own, such as a cited helper theorem's element bound.
    let element_path = walk.machine.and_then(|machine| {
        super::super::stable_element_place_path(program, machine, walk.state, expression)
    });
    let element_fact = element_path
        .as_deref()
        .and_then(|path| walk.environment.get(path));
    // The declared-place owner handles every index hop and preserves its
    // effective element type. A second indexed-only lookup would ask the same
    // projection again with different admission conditions.
    let Some(handle) = walk.declared_type(expression) else {
        return NEUTRAL;
    };
    // A scalar-reference place reads its referent's value. Losing a tracked
    // interval must fall back to that primitive's range, not make the
    // arithmetic untyped and therefore unchecked. Keep the raw handle below for
    // its domain and range contracts.
    let primitive = places::unwrapped_type_reference(program, handle)
        .and_then(|referent| program.primitive_type_reference(referent));
    let type_range = primitive
        .and_then(primitive_range)
        .unwrap_or(Interval::UNBOUNDED);
    // Narrowest sound interval: a value PROVEN on this path (flow environment)
    // wins; else a declared `[min..max]` range constraint (S4); else the full
    // type width. A proven interval is INTERSECTED with the type range, because
    // a typed value is ALWAYS within its type even when the proof only bounds
    // ONE end -- a one-sided `requires x < 100` gives the environment
    // `[None, 99]`, and `[None, 99] ∩ i32 = [i32::MIN, 99]` keeps the type's low
    // end so `x + 1` proves Exact (a Wrapping-spilled interval is likewise
    // clamped back to the type). The same intersect-with-source-type keystone
    // the narrowing store uses.
    let interval = place_path(program, expression)
        .and_then(|path| walk.environment.get(&path))
        .or(element_fact)
        .or_else(|| range_constraint_interval(program, handle))
        .map(|proven| proven.intersect(type_range))
        .unwrap_or(type_range);
    // R2 rung 3 slice 7 (READER HYPOTHESES): a domain-carrying place's standing
    // where facts refine the read -- sound because the write net is TOTAL and
    // gated reads are access-gated, so the facts hold at every legal
    // observation.
    let interval = walk
        .machine
        .and_then(|machine| {
            crate::proof_contracts::default_domains::where_fact_interval(
                program, machine, walk.state, expression,
            )
        })
        .map_or(interval, |facts| interval.intersect(facts));
    // Calls may clear snapshot facts without changing an immutable parameter.
    // Its checked arrival requirements remain valid at every read; the shared
    // standing-bounds query excludes mutable parameter arrival facts and only
    // recovers store-enforced bounds for mutable places.
    let interval = walk
        .machine
        .zip(walk.state)
        .and_then(|(machine, state)| {
            crate::proof_contracts::arithmetic_domains::standing_integer_interval(
                program, machine, state, expression,
            )
        })
        .map_or(interval, |standing| interval.intersect(standing));
    let interval = walk.machine.map_or(interval, |machine| {
        super::super::ordered_values::refine_place_interval(
            program,
            machine,
            walk.state,
            walk.environment,
            expression,
            interval,
        )
    });
    // Atomic integer types (AtomicU32, ...) have hardware wrap-around semantics,
    // so their arithmetic is Wrapping, not Exact -- a `fetch_add` never raises
    // an overflow proof obligation.
    let domain = if is_atomic_type(program, handle) {
        ArithmeticDomain::Wrapping
    } else {
        program.arithmetic_domain_for_type_reference(handle)
    };
    // The shared place projection already retains inherited numeric element
    // policy at every collection hop. Field projections keep their own type;
    // arithmetic does not reconstruct a policy from the source arrangement.
    let recipe = walk
        .environment
        .range_recipe(program, expression)
        .cloned()
        .or_else(|| {
            let typed_trees::expression::ExpressionNode::Name(path) =
                program.expression_table.expression(expression)
            else {
                return None;
            };
            let declaration = program.const_declaration_by_symbol(path.symbol)?;
            if declaration.evaluated_value.is_some()
                || !declaration.materialized_initializer.is_valid()
            {
                return None;
            }
            Some(checked_trees::IntegerRangeRecipe::observation(
                expression,
                walk.lexical_owner,
            ))
        });
    Analysis {
        recipe,
        domain: Some(domain),
        interval,
        primitive,
    }
}
