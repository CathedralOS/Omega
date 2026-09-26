//! Place reads (`x`, `self.field`, `self.cells[i]`): the declared type gives the
//! domain and the integer primitive; the value range is the flow-tracked
//! interval (S4) if one is proven on this linear path, else the primitive's
//! full range.

use super::{Analysis, ExpressionWalk, NEUTRAL};
use crate::proof_contracts::arithmetic_domains::integer_ranges::{is_atomic_type, primitive_range};
use crate::proof_contracts::arithmetic_domains::{
    ArithmeticDomain, ExpressionHandle, Interval, declared_place_type_raw, place_path,
    range_constraint_interval,
};
use crate::value_custody::places;

/// An INDEXED read (`self.cells[rp]`) resolves through its collection's ELEMENT
/// type, so a range-refined element (`[i32 [0..=7]; N]`) feeds the overflow
/// proof exactly like a range-refined field (writes into elements are
/// range-enforced by the proof side, and ZII requires 0 in the element range --
/// the interval is a true invariant).
pub(super) fn analyze(walk: &ExpressionWalk, expression: ExpressionHandle) -> Analysis {
    let program = walk.program;
    let Some(handle) = declared_place_type_raw(program, walk.machine, walk.state, expression)
        .or_else(|| {
            // Only a RANGE-refined element feeds the analysis. An unranged
            // element stays NEUTRAL (unchecked) as before -- resolving it would
            // stamp `primitive + Exact` onto reads whose enclosing arithmetic
            // was historically domain-neutral (`[i32; 3] in Wrapping` carries
            // the domain on the ARRAY, so its bare-`i32` element read as Exact
            // broke Wrapping-array canaries with spurious overflow +
            // domain-mixing rejects).
            places::declared_indexed_projection_type_raw(
                program,
                walk.machine,
                walk.state,
                expression,
            )
            .filter(|handle| range_constraint_interval(program, *handle).is_some())
        })
    else {
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
        .or_else(|| range_constraint_interval(program, handle))
        .map(|proven| proven.intersect(type_range))
        .unwrap_or(type_range);
    // R2 rung 3 slice 7 (READER HYPOTHESES): a domain-carrying place's standing
    // where facts refine the read -- sound because the write net is TOTAL and
    // gated reads are access-gated, so the facts hold at every legal
    // observation.
    let interval = match crate::proof_contracts::default_domains::where_fact_interval(
        program,
        walk.machine,
        walk.state,
        expression,
    ) {
        Some(facts) => interval.intersect(facts),
        None => interval,
    };
    // Atomic integer types (AtomicU32, ...) have hardware wrap-around semantics,
    // so their arithmetic is Wrapping, not Exact -- a `fetch_add` never raises
    // an overflow proof obligation.
    let domain = if is_atomic_type(program, handle) {
        ArithmeticDomain::Wrapping
    } else {
        program.arithmetic_domain_for_type_reference(handle)
    };
    // A domain constraint on a COLLECTION declaration (`[i32; 5] in Wrapping`)
    // is the element's arithmetic policy: the element type itself is bare
    // `i32`, so the resolved handle reads Exact while the array declares how
    // element arithmetic behaves. An element read whose own type carries no
    // domain inherits the collection's, keeping its carrier across stores
    // (`let h: i32 in Wrapping = self.coeffs[i]` is a same-carrier move, and
    // `h as i32` spells the erasure the strict store rule requires). A member
    // projection (`cells[k].v`) resolves the field's own declared type, so the
    // collection domain applies only to a direct `Indexed` leaf.
    let domain = match program.expression_table.expression(expression) {
        typed_trees::expression::ExpressionNode::Indexed(indexed)
            if domain == ArithmeticDomain::Exact =>
        {
            declared_place_type_raw(program, walk.machine, walk.state, indexed.collection)
                .map(|collection| program.arithmetic_domain_for_type_reference(collection))
                .filter(|collection_domain| *collection_domain != ArithmeticDomain::Exact)
                .unwrap_or(domain)
        }
        _ => domain,
    };
    Analysis {
        domain: Some(domain),
        interval,
        primitive,
    }
}
