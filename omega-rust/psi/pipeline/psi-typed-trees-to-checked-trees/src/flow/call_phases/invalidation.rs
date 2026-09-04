use super::*;

pub(in crate::flow) struct CallInvalidationResult {
    pub(in crate::flow) post_contexts: HandleSpan<FlowSemanticContextRef>,
    pub(in crate::flow) post_constraints: HandleSpan<FlowConstraintRef>,
    pub(in crate::flow) invalidations: HandleSpan<FlowInvalidationFact>,
}

pub(in crate::flow) fn apply_call_invalidations(
    program: &psi_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    semantic: &FactPlan,
    domains: &DomainFacts,
    ctx: &mut FlowBuildContext,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    active_contexts: HandleSpan<FlowSemanticContextRef>,
    active_constraints: HandleSpan<FlowConstraintRef>,
    borrow_call: &BorrowCallFact,
) -> CallInvalidationResult {
    let mutated_places = call_mutated_places(
        program,
        machine.symbol,
        state.symbol,
        borrow,
        borrow_call,
        &mut ctx.state_mutation_summary_cache,
    );
    let invalidations_start = ctx.invalidations.events.len();
    let post_contexts = match mutated_places {
        None => HandleSpan::empty(),
        Some(mutated_places) => filter_contexts_after_place_mutations(
            program,
            semantic,
            domains,
            &mut ctx.contexts.semantic_context_refs,
            &mut ctx.invalidations.segments,
            &mut ctx.invalidations.events,
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
        &mut ctx.contexts.constraint_refs,
        active_constraints,
        post_contexts,
        &ctx.contexts.semantic_context_refs,
    );
    let invalidations = appended_span_since(&ctx.invalidations.events, invalidations_start);

    CallInvalidationResult {
        post_contexts,
        post_constraints,
        invalidations,
    }
}
