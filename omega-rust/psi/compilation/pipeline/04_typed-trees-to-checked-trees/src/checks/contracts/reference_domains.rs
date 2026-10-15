//! Exact live aliases and owned captures transport established qualifications.

use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use facts::{Fact, FactContextHandle};

pub(super) fn proves(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    entry_contexts: &[FactContextHandle],
    required: &Fact,
    captured_referents: Option<&[validation::CanonicalPlace]>,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let Some((_, _, domain_symbol, _)) = required.payload.domain_membership() else {
        return false;
    };
    let Some(place) = required.place.as_place() else {
        return false;
    };
    let place = facts.semantic.places.get(place);
    let place = validation::CanonicalPlace {
        root: place.root,
        segments: facts
            .semantic
            .place_segments
            .span_or_empty(place.segments)
            .to_vec(),
    };
    let Some(machine) = program.machine_by_symbol(state.machine_symbol) else {
        return false;
    };
    let mut owned_frames = None;
    let Some(frames) = crate::flow::shared_call_frames_or(call_frames, program, &mut owned_frames)
    else {
        return false;
    };
    // Match facts at this call, not facts that held when the alias was made.
    // Exact identity transports an existing qualification; it cannot create one.
    // A helper's guarantee may name a different live alias to this same storage.
    // A reference-result local with no single origin names one of finitely
    // many candidates; the required row must then hold on every candidate,
    // since the callee receives whichever storage the loan names.
    let sources = if let Some(referents) = captured_referents {
        referents.to_vec()
    } else {
        match crate::flow::local_reference_storage_at_call(
            program,
            frames,
            machine,
            &facts.flow,
            state,
            call,
            place.clone(),
        ) {
            Some(source) => vec![source],
            None => crate::flow::local_reference_candidate_storages_at_call(
                program,
                frames,
                &facts.borrow,
                machine,
                &facts.flow,
                state,
                call,
                place.clone(),
            )
            .unwrap_or_default(),
        }
    };
    if !sources.is_empty()
        && sources.iter().all(|source| {
            if captured_referents.is_some()
                && !crate::flow::place_cases_are_selected(
                    program,
                    &facts.semantic,
                    entry_contexts,
                    state.machine_symbol,
                    state.state_symbol,
                    call.statement_index,
                    source,
                )
            {
                return false;
            }
            contexts_prove_domain(
                program,
                facts,
                entry_contexts.iter().copied(),
                source,
                domain_symbol,
                |candidate| {
                    let candidate = crate::flow::local_reference_storage_at_call(
                        program,
                        frames,
                        machine,
                        &facts.flow,
                        state,
                        call,
                        candidate,
                    )?;
                    // A frozen unresolved selector is not its expression's
                    // current value. Only whole-extent evidence covers it;
                    // equal selector syntax cannot authenticate that capture.
                    if captured_referents.is_some()
                        && candidate.segments.iter().zip(&source.segments).any(
                            |(candidate, source)| {
                                matches!(source, facts::PlaceSegment::Index { .. })
                                    && !matches!(
                                        candidate,
                                        facts::PlaceSegment::FixedRange {
                                            start: 0,
                                            end: usize::MAX
                                        }
                                    )
                            },
                        )
                    {
                        return None;
                    }
                    Some(candidate)
                },
            )
        })
    {
        return true;
    }
    // An owned copy freezes its source value. Replay the existing exact value
    // custody walk before consulting the qualification at that state's entry;
    // a current fact for an overwritten source slot is not capture evidence.
    if captured_referents.is_some() || !field_place(&place) {
        return false;
    }
    let Some(source) = crate::flow::value_origin_at_call(
        program,
        &facts.flow,
        machine,
        state,
        call,
        place,
        Some(frames),
    ) else {
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

fn field_place(place: &validation::CanonicalPlace) -> bool {
    matches!(place.root, facts::PlaceRoot::Symbol(symbol) if symbol.is_valid())
        && place.segments.iter().all(|segment| {
            matches!(
                segment,
                facts::PlaceSegment::Field { .. } | facts::PlaceSegment::Case { .. }
            )
        })
}

fn contexts_prove_domain(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    contexts: impl Iterator<Item = FactContextHandle>,
    source: &validation::CanonicalPlace,
    domain_symbol: symbols::SymbolHandle,
    mut resolve_candidate: impl FnMut(validation::CanonicalPlace) -> Option<validation::CanonicalPlace>,
) -> bool {
    let mut contexts = contexts;
    contexts.any(|context| {
        facts
            .semantic
            .context_view(facts.semantic.contexts.get(context))
            .facts()
            .any(|candidate| {
                let candidate_domain = match candidate.payload {
                    facts::domain_membership_symbol_pattern!(domain_symbol) => domain_symbol,
                    _ => return false,
                };
                let Some(candidate) = candidate.place.as_place() else {
                    return false;
                };
                let candidate = facts.semantic.places.get(candidate);
                if !validation::field_domains::domain_membership_implies(
                    program,
                    candidate_domain,
                    domain_symbol,
                ) {
                    return false;
                }
                let candidate = validation::CanonicalPlace {
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
                (candidate.root) == (source.root)
                    && super::prover::segments_cover_subject(
                        program,
                        &candidate.segments,
                        &source.segments,
                    )
            })
    })
}
