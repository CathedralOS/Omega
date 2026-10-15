use validation::CanonicalPlace;

use crate::flow::FlowBuildContext;
use crate::flow::appended_span_since;
use crate::flow::filter_contexts_after_place_mutations;
use crate::flow::mutation::close_storage_places_over_aliases;
use crate::flow::mutation::{CallMutationWrite, call_mutation_writes};
use crate::flow::project_constraint_refs_to_active_contexts;
use arena::HandleSpan;
use checked_trees::{
    BorrowCallFact, BorrowFacts, DomainFacts, FlowConstraintRef, FlowInvalidationFact,
    FlowInvalidationSource, FlowSemanticContextRef,
};
use facts::FactPlan;

pub(in crate::flow) struct CallInvalidationResult {
    pub(in crate::flow) post_contexts: HandleSpan<FlowSemanticContextRef>,
    pub(in crate::flow) post_constraints: HandleSpan<FlowConstraintRef>,
    pub(in crate::flow) invalidations: HandleSpan<FlowInvalidationFact>,
}

/// One immutable invocation footprint supplies both overlap invalidation and
/// attributed handback. Retaining them together avoids recomputing the same
/// alias closure while preserving which parameter contributed each write.
pub(in crate::flow) struct CallStorageWrites {
    pub(in crate::flow) places: Vec<CanonicalPlace>,
    pub(in crate::flow) contributions: Vec<CallMutationWrite>,
}

pub(in crate::flow) fn call_storage_writes(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    build: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    borrow_call: &BorrowCallFact,
) -> Option<std::rc::Rc<CallStorageWrites>> {
    let occurrence = (
        machine.symbol,
        state.symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    );
    if let Some(writes) = build
        .call_sites
        .get(&occurrence)
        .and_then(|prepared| prepared.storage_writes.as_ref())
    {
        return writes.clone();
    }
    // The footprint is source-derived, not a judgment about the current
    // incoming values. Retain it beside the existing occurrence lookup across
    // sweeps; each consumer still filters its own live contexts below.
    let writes = call_storage_mutations(program, borrow, build, machine, state, borrow_call).map(
        |contributions| {
            std::rc::Rc::new(CallStorageWrites {
                places: contributions
                    .iter()
                    .map(|write| write.place.clone())
                    .collect(),
                contributions,
            })
        },
    );
    build
        .call_sites
        .entry(occurrence)
        .or_default()
        .storage_writes = Some(writes.clone());
    writes
}

fn call_storage_mutations(
    program: &dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    build: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    borrow_call: &BorrowCallFact,
) -> Option<Vec<CallMutationWrite>> {
    let writes = call_mutation_writes(
        program,
        machine.symbol,
        state.symbol,
        borrow,
        borrow_call,
        build.state_mutation_summary_cache,
        build.call_frames,
    )?;
    let mut result = Vec::new();
    for write in writes {
        // When the resolver cannot enumerate this state's local origins (a
        // reference local of finite candidate origins is such a state),
        // borrow exclusivity already forbids a second live alias of the
        // written storage, so the places stand without the closure.
        let places = close_storage_places_over_aliases(
            program,
            machine.symbol,
            state.symbol,
            borrow_call.statement_index,
            vec![write.place.clone()],
            build.call_frames,
        )
        .unwrap_or_else(|| vec![write.place]);
        for place in places {
            let contribution = CallMutationWrite {
                place,
                source: write.source.clone(),
            };
            if !result.contains(&contribution) {
                result.push(contribution);
            }
        }
    }
    Some(result)
}

pub(in crate::flow) fn apply_call_invalidations(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    domains: &DomainFacts,
    build: &mut FlowBuildContext,
    active_contexts: HandleSpan<FlowSemanticContextRef>,
    active_constraints: HandleSpan<FlowConstraintRef>,
    borrow_call: &BorrowCallFact,
    mutated_places: Option<&[CanonicalPlace]>,
) -> CallInvalidationResult {
    // Flow construction keeps every semantic constraint in the active context
    // selection. With a known empty footprint neither selection nor its facts
    // changes, so there is nothing to project or invalidate. Unknown writes
    // remain distinct: they still withdraw all semantic constraints below.
    if mutated_places.is_some_and(<[CanonicalPlace]>::is_empty) {
        return CallInvalidationResult {
            post_contexts: active_contexts,
            post_constraints: active_constraints,
            invalidations: HandleSpan::empty(),
        };
    }
    let invalidations_start = build.invalidations.events.len();
    let post_contexts = match mutated_places {
        None => HandleSpan::empty(),
        Some(mutated_places) => filter_contexts_after_place_mutations(
            program,
            semantic,
            domains,
            &mut build.contexts.semantic_context_refs,
            &mut build.invalidations.segments,
            &mut build.invalidations.events,
            active_contexts,
            mutated_places,
            FlowInvalidationSource::Call {
                statement_index: borrow_call.statement_index,
                call_ordinal: borrow_call.call_ordinal,
                target_symbol: borrow_call.target_symbol,
            },
        ),
    };
    let post_constraints = project_constraint_refs_to_active_contexts(
        &mut build.contexts.constraint_refs,
        active_constraints,
        post_contexts,
        &build.contexts.semantic_context_refs,
    );
    let invalidations = appended_span_since(&build.invalidations.events, invalidations_start);

    CallInvalidationResult {
        post_contexts,
        post_constraints,
        invalidations,
    }
}
