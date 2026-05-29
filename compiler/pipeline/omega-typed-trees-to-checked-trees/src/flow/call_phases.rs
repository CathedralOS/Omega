use super::*;

pub(super) struct CallFlowContexts {
    pub(super) contexts: HandleSpan<FlowSemanticContextRef>,
    pub(super) constraints: HandleSpan<FlowConstraintRef>,
}

pub(super) struct CallInvalidationResult {
    pub(super) post_contexts: HandleSpan<FlowSemanticContextRef>,
    pub(super) post_constraints: HandleSpan<FlowConstraintRef>,
    pub(super) invalidations: HandleSpan<FlowInvalidationFact>,
}

pub(super) fn build_call_entry_contexts(
    borrow: &BorrowFacts,
    ctx: &mut FlowBuildContext,
    active_contexts: HandleSpan<FlowSemanticContextRef>,
    active_constraints: HandleSpan<FlowConstraintRef>,
    borrow_call: &BorrowCallFact,
) -> CallFlowContexts {
    let contexts = clone_flow_contexts(&mut ctx.semantic_context_refs, active_contexts);
    let mut constraints = clone_constraint_refs(&mut ctx.constraint_refs, active_constraints);
    if let Some((borrow_call_handle, _)) = borrow.calls.iter().find(|(_, call)| {
        call.statement_index == borrow_call.statement_index
            && call.call_ordinal == borrow_call.call_ordinal
            && call.target_symbol == borrow_call.target_symbol
            && call.receiver_symbol == borrow_call.receiver_symbol
    }) {
        append_constraint_ref(
            &mut ctx.constraint_refs,
            &mut constraints,
            FlowConstraintKind::BorrowCall {
                call: borrow_call_handle,
            },
        );
    }
    append_contiguous_borrow_access_constraints(
        &mut ctx.constraint_refs,
        &mut constraints,
        borrow_call.accesses,
    );

    CallFlowContexts {
        contexts,
        constraints,
    }
}

pub(super) fn build_call_requires_contexts(
    semantic: &FactPlan,
    ctx: &mut FlowBuildContext,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    borrow_call: &BorrowCallFact,
) -> CallFlowContexts {
    build_call_contract_contexts(
        semantic,
        ctx,
        ProgramPoint::CallRequires {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            statement_index: borrow_call.statement_index,
            call_ordinal: borrow_call.call_ordinal,
        },
    )
}

pub(super) fn apply_call_invalidations(
    program: &omega_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    semantic: &FactPlan,
    domains: &DomainFacts,
    ctx: &mut FlowBuildContext,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
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
    let invalidations_start = ctx.invalidations.len();
    let post_contexts = if call_may_mutate_contract_state(program, borrow, borrow_call) {
        if mutated_places.is_empty() {
            HandleSpan::empty()
        } else {
            filter_contexts_after_place_mutations(
                program,
                semantic,
                domains,
                &mut ctx.semantic_context_refs,
                &mut ctx.invalidation_segments,
                &mut ctx.invalidations,
                active_contexts,
                &mutated_places,
                FlowInvalidationSource::Call {
                    statement_index: borrow_call.statement_index,
                    call_ordinal: borrow_call.call_ordinal,
                    target_symbol: borrow_call.target_symbol,
                },
            )
        }
    } else {
        clone_flow_contexts(&mut ctx.semantic_context_refs, active_contexts)
    };
    let post_constraints = project_constraint_refs_to_active_contexts(
        &mut ctx.constraint_refs,
        active_constraints,
        post_contexts,
        &ctx.semantic_context_refs,
    );
    let invalidations = appended_span_since(&ctx.invalidations, invalidations_start);

    CallInvalidationResult {
        post_contexts,
        post_constraints,
        invalidations,
    }
}

pub(super) fn build_call_exit_contexts(
    semantic: &FactPlan,
    ctx: &mut FlowBuildContext,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    borrow_call: &BorrowCallFact,
    post_contexts: HandleSpan<FlowSemanticContextRef>,
    post_constraints: HandleSpan<FlowConstraintRef>,
) -> CallFlowContexts {
    let mut contexts = clone_flow_contexts(&mut ctx.semantic_context_refs, post_contexts);
    let mut constraints = clone_constraint_refs(&mut ctx.constraint_refs, post_constraints);
    append_call_contract_contexts(
        semantic,
        ctx,
        &mut contexts,
        &mut constraints,
        ProgramPoint::CallEnsures {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            statement_index: borrow_call.statement_index,
            call_ordinal: borrow_call.call_ordinal,
        },
    );

    CallFlowContexts {
        contexts,
        constraints,
    }
}

fn build_call_contract_contexts(
    semantic: &FactPlan,
    ctx: &mut FlowBuildContext,
    point: ProgramPoint,
) -> CallFlowContexts {
    let mut contexts = HandleSpan::empty();
    let mut constraints = HandleSpan::empty();
    append_call_contract_contexts(semantic, ctx, &mut contexts, &mut constraints, point);
    CallFlowContexts {
        contexts,
        constraints,
    }
}

fn append_call_contract_contexts(
    semantic: &FactPlan,
    ctx: &mut FlowBuildContext,
    contexts: &mut HandleSpan<FlowSemanticContextRef>,
    constraints: &mut HandleSpan<FlowConstraintRef>,
    point: ProgramPoint,
) {
    append_flow_contexts_for_points(semantic, &mut ctx.semantic_context_refs, contexts, &[point]);
    append_semantic_constraints_for_points(
        semantic,
        &mut ctx.constraint_refs,
        constraints,
        &[point],
    );
}
