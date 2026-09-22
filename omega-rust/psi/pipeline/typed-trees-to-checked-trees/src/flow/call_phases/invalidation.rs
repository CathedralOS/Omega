use crate::flow::CanonicalPlace;
use crate::flow::FlowBuildContext;
use crate::flow::appended_span_since;
use crate::flow::call_mutated_places;
use crate::flow::filter_contexts_after_place_mutations;
use crate::flow::mutation::close_storage_places_over_aliases_with_resolver;
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

pub(in crate::flow) fn call_storage_writes(
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    build: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    borrow_call: &BorrowCallFact,
) -> Option<Vec<CanonicalPlace>> {
    call_mutated_places(
        program,
        machine.symbol,
        state.symbol,
        borrow,
        borrow_call,
        build.state_mutation_summary_cache,
        build.call_frames,
    )
    .and_then(|places| {
        // When the resolver cannot enumerate this state's local origins (a
        // reference local of finite candidate origins is such a state),
        // borrow exclusivity already forbids a second live alias of the
        // written storage, so the places stand without the closure.
        close_storage_places_over_aliases_with_resolver(
            program,
            machine.symbol,
            state.symbol,
            borrow_call.statement_index,
            places.clone(),
            build.call_frames,
        )
        .or(Some(places))
    })
}

pub(in crate::flow) fn apply_call_invalidations(
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    semantic: &FactPlan,
    domains: &DomainFacts,
    build: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    active_contexts: HandleSpan<FlowSemanticContextRef>,
    active_constraints: HandleSpan<FlowConstraintRef>,
    borrow_call: &BorrowCallFact,
) -> CallInvalidationResult {
    let mutated_places = call_storage_writes(program, borrow, build, machine, state, borrow_call);
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
            &mutated_places,
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
