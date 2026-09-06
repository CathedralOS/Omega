//! A live reference may use an existing domain fact for its exact referent.

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
    if source == place {
        return false;
    }
    // Match facts at this call, not facts that held when the alias was made.
    // Exact identity transports an existing qualification; it cannot create one.
    entry_contexts.iter().any(|context| {
        facts
            .semantic
            .context_view(facts.semantic.contexts.get(*context))
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
                crate::field_domain::domain_membership_implies(
                    program,
                    candidate_domain,
                    domain_symbol,
                ) && crate::flow::normalized_event_place_root(program, candidate.root)
                    == crate::flow::normalized_event_place_root(program, source.root)
                    && facts
                        .semantic
                        .place_segments
                        .span_or_empty(candidate.segments)
                        == source.segments
            })
    })
}
