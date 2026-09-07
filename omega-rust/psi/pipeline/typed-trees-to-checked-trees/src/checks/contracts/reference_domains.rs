//! Exact live aliases and owned captures transport established qualifications.

use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use facts::{Fact, FactContextHandle, FactPayload, FactPlace};
use typed_trees::TypedTrees;

pub(super) fn proves(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    entry_contexts: &[FactContextHandle],
    required: &Fact,
) -> bool {
    let FactPayload::ContractDomainMembership { domain_symbol, .. } = required.payload else {
        return false;
    };
    let FactPlace::Place(place) = required.place else {
        return false;
    };
    let place = facts.semantic.places.get(place);
    let place = crate::flow::CanonicalPlace {
        root: place.root,
        segments: facts
            .semantic
            .place_segments
            .span_or_empty(place.segments)
            .to_vec(),
    };
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == state.machine_symbol)
    else {
        return false;
    };
    let Some(frames) = validation::CallFrameResolver::new(program) else {
        return false;
    };
    let Some(source) = crate::flow::local_reference_storage_at_call(
        program,
        &frames,
        machine,
        &facts.flow,
        state,
        call,
        place.clone(),
    ) else {
        return false;
    };
    // Match facts at this call, not facts that held when the alias was made.
    // Exact identity transports an existing qualification; it cannot create one.
    // A helper's guarantee may name a different live alias to this same storage.
    if contexts_prove_domain(
        program,
        facts,
        entry_contexts.iter().copied(),
        &source,
        domain_symbol,
        |candidate| {
            crate::flow::local_reference_storage_at_call(
                program,
                &frames,
                machine,
                &facts.flow,
                state,
                call,
                candidate,
            )
        },
    ) {
        return true;
    }
    // An owned copy freezes its source value. Replay the existing exact value
    // custody walk before consulting the qualification at that state's entry;
    // a current fact for an overwritten source slot is not capture evidence.
    if !field_place(&place) {
        return false;
    }
    let Some(source) =
        crate::flow::value_origin_at_call(program, &facts.flow, machine, state, call, place)
    else {
        return false;
    };
    if !field_place(&source) {
        return false;
    }
    contexts_prove_domain(
        program,
        facts,
        facts
            .flow
            .semantic_constraint_contexts(state.entry_constraints),
        &source,
        domain_symbol,
        Some,
    )
}

fn field_place(place: &crate::flow::CanonicalPlace) -> bool {
    matches!(place.root, facts::PlaceRoot::Symbol(symbol) if symbol.is_valid())
        && place.segments.iter().all(|segment| {
            matches!(
                segment,
                facts::PlaceSegment::Field { .. } | facts::PlaceSegment::Case { .. }
            )
        })
}

fn contexts_prove_domain(
    program: &TypedTrees,
    facts: &CheckFacts,
    contexts: impl Iterator<Item = FactContextHandle>,
    source: &crate::flow::CanonicalPlace,
    domain_symbol: symbols::SymbolHandle,
    mut resolve_candidate: impl FnMut(
        crate::flow::CanonicalPlace,
    ) -> Option<crate::flow::CanonicalPlace>,
) -> bool {
    let mut contexts = contexts;
    contexts.any(|context| {
        facts
            .semantic
            .context_view(facts.semantic.contexts.get(context))
            .facts()
            .any(|candidate| {
                let candidate_domain = match candidate.payload {
                    FactPayload::DomainMembership { domain_symbol, .. }
                    | FactPayload::ContractDomainMembership { domain_symbol, .. } => domain_symbol,
                    _ => return false,
                };
                let FactPlace::Place(candidate) = candidate.place else {
                    return false;
                };
                let candidate = facts.semantic.places.get(candidate);
                if !crate::field_domain::domain_membership_implies(
                    program,
                    candidate_domain,
                    domain_symbol,
                ) {
                    return false;
                }
                let candidate = crate::flow::CanonicalPlace {
                    root: candidate.root,
                    segments: facts
                        .semantic
                        .place_segments
                        .span_or_empty(candidate.segments)
                        .to_vec(),
                };
                let Some(candidate) = resolve_candidate(candidate) else {
                    return false;
                };
                crate::flow::normalized_event_place_root(program, candidate.root)
                    == crate::flow::normalized_event_place_root(program, source.root)
                    && candidate.segments == source.segments
            })
    })
}
