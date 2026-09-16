//! Whole-place assignment transports live evidence on its contained fields.
//! Predicates, literal leaf values, folded scalar snapshots, and per-byte
//! predicate classes all describe the copied VALUE, so each stored fact below
//! the source place reappears at the destination under the same relative path:
//! `let copy = rows` carries `rows[i].bytes` evidence onto `copy[i].bytes`, and
//! `let r = rows[0]` carries `rows[0].bytes` onto `r.bytes`. This is the
//! below-source sibling of the exact-place lane in
//! `propagate_statement_transfers`, which transports the same payloads at the
//! source place itself. Runtime-indexed segments still stop the transport -- a
//! copy cannot promise which element supplied the evidence.
//!
//! The same rebase serves the builtin collection VIEWS: `recv.as_slice()` and
//! `recv.as_mut_slice()` lend the receiver's element storage to the bound
//! local element-for-element, so `view[i]` IS `recv[i]` and every fact below
//! `recv` re-anchors below `view`. The borrows checker keeps `recv` frozen
//! for the view's loan, and a write through a mutable view retires both the
//! view-named and the receiver-named facts through the usual alias-closing
//! invalidation, so transported evidence cannot go stale while it is
//! consultable.
use super::PlaceHandle;
use crate::flow::FlowBuildContext;
use arena::HandleSpan;
use checked_trees::FlowSemanticContextRef;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use facts::{Fact, FactOrigin, FactPayload, FactPlace, FactPlan, ProgramPoint};
use symbols::SymbolHandle;

#[allow(clippy::too_many_arguments)]
pub(super) fn append_copied_field_predicates(
    program: &typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    contexts: &FlowBuildContext,
    active: HandleSpan<FlowSemanticContextRef>,
    source: PlaceHandle,
    destination: PlaceHandle,
    point: ProgramPoint,
    references: &mut HandleSpan<facts::FactRef>,
) {
    let source_place = *semantic.places.get(source);
    let destination_place = *semantic.places.get(destination);
    // The source is the value read at this statement: storage, or the exact
    // call-expression occurrence whose checked ensures published result-field
    // facts (flow/calls.rs). Other expression roots carry no such evidence, so
    // a non-call expression source still stops here.
    let valid_source_root = match source_place.root {
        facts::PlaceRoot::Symbol(symbol) => symbol.is_valid(),
        facts::PlaceRoot::Expression(expression) => matches!(
            program.expression_table.expression(expression),
            checked_trees::expression::ExpressionNode::Call(_)
        ),
        _ => false,
    };
    if !valid_source_root
        || !matches!(destination_place.root, facts::PlaceRoot::Symbol(symbol) if symbol.is_valid())
    {
        return;
    }
    let source_segments = semantic
        .place_segments
        .span_or_empty(source_place.segments)
        .to_vec();
    let destination_segments = semantic
        .place_segments
        .span_or_empty(destination_place.segments)
        .to_vec();
    if !source_segments
        .iter()
        .chain(&destination_segments)
        .all(stable_segment)
    {
        return;
    }
    let facts: Vec<_> = contexts
        .contexts
        .semantic_context_refs
        .span_or_empty(active)
        .iter()
        .flat_map(|reference| {
            semantic
                .refs
                .span_or_empty(semantic.contexts.get(reference.context).facts)
        })
        .map(|reference| *semantic.facts.get(reference.fact))
        .collect();
    for fact in facts {
        let payload = match fact.payload {
            FactPayload::DomainMembership {
                domain,
                domain_symbol,
                semantic_domain,
                ..
            }
            | FactPayload::ContractDomainMembership {
                domain,
                domain_symbol,
                semantic_domain,
                ..
            } => {
                // A field predicate follows the copied value. Routed
                // qualifications require their own custody correspondence,
                // not this predicate rule.
                if !program.domain_definitions().iter().any(|definition| {
                    definition.symbol == domain_symbol
                        && definition.establishment_routes.is_empty()
                        && definition.alias.is_none()
                        && definition.predicate_body.is_present()
                }) {
                    continue;
                }
                FactPayload::DomainMembership {
                    value: ExpressionHandle::invalid(),
                    domain,
                    domain_symbol,
                    semantic_domain,
                }
            }
            // Value evidence below the source describes the copied contents
            // the same way a predicate does: the element literal a constructor
            // recorded at `rows[i].bytes`, the folded scalar at `rows[i].tag`,
            // or the per-byte class a checked element write preserved. Only
            // literal `AssignedValue` leaves qualify -- a call occurrence is
            // retained as provenance at its root place and can never name a
            // copied field's value.
            FactPayload::AssignedValue { value }
                if program.expression_table.expression_is_valid(value)
                    && matches!(
                        program.expression_table.expression(value),
                        ExpressionNode::Integer(_)
                            | ExpressionNode::Boolean(_)
                            | ExpressionNode::String(_)
                    ) =>
            {
                fact.payload
            }
            FactPayload::AssignedScalarValue { .. } | FactPayload::BytePredicate { .. } => {
                fact.payload
            }
            _ => continue,
        };
        let FactPlace::Place(place) = fact.place else {
            continue;
        };
        let place = semantic.places.get(place);
        let segments = semantic.place_segments.span_or_empty(place.segments);
        if place.root != source_place.root
            || segments.len() <= source_segments.len()
            || !segments.starts_with(&source_segments)
            || !segments.iter().all(stable_segment)
        {
            continue;
        }
        let suffix = segments[source_segments.len()..].to_vec();
        let copied_place = semantic.append_place(facts::Place {
            root: destination_place.root,
            segments: HandleSpan::empty(),
        });
        for segment in destination_segments.iter().chain(&suffix) {
            semantic.push_place_segment(copied_place, *segment);
        }
        let copied_fact = semantic.append_fact(Fact {
            place: FactPlace::Place(copied_place),
            point,
            origin: FactOrigin::StatementTransfer,
            evidence: fact.evidence,
            payload,
        });
        semantic.append_ref(references, copied_fact);
    }
}

fn stable_segment(segment: &facts::PlaceSegment) -> bool {
    match segment {
        facts::PlaceSegment::Field { symbol } => symbol.is_valid(),
        facts::PlaceSegment::Case { variant } => variant.is_valid(),
        facts::PlaceSegment::FixedIndex { .. } => true,
        _ => false,
    }
}

/// The place a builtin collection-view call lends to its result, or `None`
/// when `expression` is not a targetless `as_slice`/`as_mut_slice` on a
/// collection receiver. This mirrors the ownership lane's
/// `append_builtin_collection_view` gate exactly: a resolved `target_symbol`
/// means a declared machine or boundary operator answered instead, and its
/// returned view may be only a partial projection of the input -- evidence
/// below the argument cannot re-anchor 1:1, so that shape stays unclaimed
/// here. `as_view`/`bytes` are text-level views of a scalar carrier rather
/// than element views of a collection, and stay unclaimed for the same reason.
/// Extra operands or a non-collection receiver fail closed the same way.
pub(super) fn collection_view_source_place(
    program: &typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<PlaceHandle> {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return None;
    };
    if !matches!(call.target.as_str(), "as_slice" | "as_mut_slice")
        || call.target_symbol.is_valid()
        || !call.receiver.is_valid()
        || !call.arguments.is_empty()
        || !call.evidence_arguments.is_empty()
        || !call.machine_arguments.is_empty()
        || call.static_requirement_dispatch.is_some()
        || call.quotient_operation.is_some()
        || call.private_layout_operation.is_some()
    {
        return None;
    }
    let mut reference = crate::flow::expression_type_reference_in_state(
        program,
        state_symbol,
        statement_index,
        call.receiver,
    )?;
    for _ in 0..program.type_reference_table.type_reference_count() {
        if !program
            .type_reference_table
            .contains_type_reference(reference)
        {
            return None;
        }
        match program.type_reference_table.type_reference(reference) {
            typed_trees::types::TypeReferenceNode::Reference { referee, .. }
            | typed_trees::types::TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => reference = *referee,
            typed_trees::types::TypeReferenceNode::FixedArray { .. }
            | typed_trees::types::TypeReferenceNode::Slice { .. } => {
                return super::contextual_expression_place(
                    program,
                    semantic,
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    call.receiver,
                );
            }
            _ => return None,
        }
    }
    None
}
