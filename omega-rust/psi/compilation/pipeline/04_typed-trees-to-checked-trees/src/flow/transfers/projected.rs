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
use crate::flow::origin_place;
use arena::HandleSpan;
use checked_trees::FlowSemanticContextRef;
use checked_trees::expression::ExpressionHandle;
use checked_trees::statement::StatementNode;
use facts::{Fact, FactOrigin, FactPayload, FactPlace, FactPlan, ProgramPoint};
use language_semantics::declaration_selection::CollectionViewOperation;
use symbols::SymbolHandle;

#[allow(clippy::too_many_arguments)]
pub(super) fn append_copied_field_predicates(
    program: &dyn ::validation::CheckingInput,
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
    // Storage and completed expression occurrences share the same evidence
    // namespace. Calls publish checked result guarantees; constructors publish
    // completed child snapshots. The active facts below establish what may be
    // transported, not the source expression's spelling. No initializer or
    // expression is evaluated again to recover a value here.
    let valid_source_root = match source_place.root {
        facts::PlaceRoot::Symbol(symbol) => symbol.is_valid(),
        facts::PlaceRoot::Expression(expression) => {
            program.expression_table.expression_is_valid(expression)
        }
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
    if !crate::flow::source_transfer_cases_selected(
        program,
        semantic,
        contexts,
        active,
        point,
        source_place.root,
        source_segments.clone(),
    ) {
        return;
    }
    if !source_segments
        .iter()
        .chain(&destination_segments)
        .all(crate::flow::place_segment_has_static_identity)
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
            facts::domain_membership_fields_pattern!(domain, domain_symbol, semantic_domain) => {
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
                if crate::values::is_assignment_literal(program, value) =>
            {
                fact.payload
            }
            FactPayload::AssignedCase { .. }
            | FactPayload::AssignedScalarValue { .. }
            | FactPayload::BytePredicate { .. } => fact.payload,
            _ => continue,
        };
        let Some(place) = fact.place.as_place() else {
            continue;
        };
        let place = semantic.places.get(place);
        let segments = semantic.place_segments.span_or_empty(place.segments);
        if place.root != source_place.root
            || segments.len() <= source_segments.len()
            || !segments.starts_with(&source_segments)
            || !segments
                .iter()
                .all(crate::flow::place_segment_has_static_identity)
        {
            continue;
        }
        let suffix = segments[source_segments.len()..].to_vec();
        let ProgramPoint::Statement {
            state_symbol,
            statement_index,
            ..
        } = point
        else {
            continue;
        };
        let source_value = validation::CanonicalPlace {
            root: place.root,
            segments: segments.to_vec(),
        };
        let target_value = validation::CanonicalPlace {
            root: destination_place.root,
            segments: destination_segments
                .iter()
                .chain(&suffix)
                .copied()
                .collect(),
        };
        if !super::completed_value_carriers_agree(
            program,
            state_symbol,
            statement_index,
            &source_value,
            &target_value,
        ) {
            continue;
        }
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

/// The place a builtin collection-view call lends to its result, or `None`
/// when `expression` is not a compiler-owned `as_slice`/`as_mut_slice` view on
/// a collection receiver. The call shape comes from
/// [`validation::collection_operations::collection_view_call`], which this lane and the
/// ownership lane's `append_builtin_collection_view` now share: a resolved
/// `target_symbol` means a declared machine or boundary operator answered
/// instead, and its returned view may be only a partial projection of the
/// input -- evidence below the argument cannot re-anchor 1:1, so that shape
/// stays unclaimed here. `as_view`/`bytes` are text-level views of a scalar
/// carrier rather than element views of a collection, so this lane claims only
/// the two element views. A non-collection receiver fails closed the same way.
pub(super) fn collection_view_source_place(
    program: &dyn ::validation::CheckingInput,
    semantic: &mut FactPlan,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<PlaceHandle> {
    let call = program.expression_table.call_node(expression)?;
    if !matches!(
        validation::collection_operations::collection_view_call(program, call),
        Some(CollectionViewOperation::SharedSlice | CollectionViewOperation::MutableSlice)
    ) {
        return None;
    }
    let mut reference = validation::expression_type_reference_in_state(
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

/// The caller storage a freshly bound local REFERENCE refers to, or `None`
/// when the statement does not bind a bare reference local or the caller
/// prefix cannot prove one unique referent. A returned reference has no
/// result storage of its own (flow/calls.rs), so its evidence stays below
/// the referent place this recovers -- the same local write origin the
/// mutation side already derives for `let room = room_mut(level)`, projected
/// back into a canonical place. Only an exactly resolved origin (no runtime
/// index truncation, no coarse-only path) may lend evidence to the binding.
#[allow(clippy::too_many_arguments)]
pub(super) fn bound_reference_referent_place(
    program: &dyn ::validation::CheckingInput,
    semantic: &mut FactPlan,
    contexts: &FlowBuildContext,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
    destination: PlaceHandle,
) -> Option<PlaceHandle> {
    let destination = *semantic.places.get(destination);
    if !destination.segments.is_empty() {
        return None;
    }
    let local_symbol = destination.root.as_symbol()?;
    if !local_symbol.is_valid() {
        return None;
    }
    let type_reference = match statement {
        StatementNode::LocalData(local) if local.symbol == local_symbol => local.type_reference,
        StatementNode::Assignment(_) => {
            declared_local_type_reference(program, state_symbol, statement_index, local_symbol)?
        }
        _ => return None,
    };
    let peeled = ::validation::peel_declared_constraints(program, type_reference);
    if !matches!(
        program.type_reference_table.type_reference(peeled),
        typed_trees::types::TypeReferenceNode::Reference { .. }
    ) {
        return None;
    }
    let machine = program.machine_by_symbol(machine_symbol)?;
    let state = program.state_by_symbol(state_symbol)?;
    // The binding's own alias is recorded once the next statement begins, so
    // the prefix observed there is the one carrying the new referent.
    let before = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index + 1)?;
    let mut owned = None;
    let resolver = crate::flow::shared_call_frames_or(contexts.call_frames, program, &mut owned)?;
    // The walk records the referent beside the local's own storage entry;
    // only an origin rooted in different storage can be the referent.
    let origin = crate::flow::sole_local_referent_origin(resolver, machine, before, local_symbol)?;
    let (place, precision) = origin_place(program, state, statement_index, &origin)?;
    if precision != crate::flow::OriginPrecision::Exact {
        return None;
    }
    Some(crate::semantic::places::append_place(
        semantic,
        place.root,
        &place.segments,
    ))
}

fn declared_local_type_reference(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    local_symbol: SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let state = program.state_by_symbol(state_symbol)?;
    program
        .state_local_declaration(state, local_symbol)
        .filter(|&(ordinal, _)| ordinal < statement_index)
        .map(|(_, local)| local.type_reference)
}
