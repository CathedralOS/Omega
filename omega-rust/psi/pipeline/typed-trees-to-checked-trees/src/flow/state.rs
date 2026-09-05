use super::*;

pub(super) fn build_state_flow_fact(
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    domains: &DomainFacts,
    ctx: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) {
    let Some((borrow_state_handle, borrow_state)) =
        borrow_state_fact(borrow, machine.symbol, state.symbol)
    else {
        return;
    };

    ctx.built_state_value_inputs.push(state.symbol);
    super::state_values::append_entry_context(program, semantic, ctx, machine, state);
    let mut state_contexts = arena::HandleSpan::empty();
    let mut state_constraints = arena::HandleSpan::empty();
    append_flow_contexts_for_points(
        semantic,
        &mut ctx.contexts.semantic_context_refs,
        &mut state_contexts,
        &[
            ProgramPoint::Global,
            ProgramPoint::Machine {
                machine_symbol: machine.symbol,
            },
            ProgramPoint::State {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
            },
        ],
    );
    append_semantic_constraints_for_points(
        semantic,
        &mut ctx.contexts.constraint_refs,
        &mut state_constraints,
        &[
            ProgramPoint::Global,
            ProgramPoint::Machine {
                machine_symbol: machine.symbol,
            },
            ProgramPoint::State {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
            },
        ],
    );
    let (rebased_contexts, _) = super::entry_origins::rebase_contexts(
        program,
        semantic,
        ctx,
        machine,
        state,
        state_contexts,
        true,
    );
    state_contexts = rebased_contexts;
    state_constraints = project_constraint_refs_to_active_contexts(
        &mut ctx.contexts.constraint_refs,
        state_constraints,
        state_contexts,
        &ctx.contexts.semantic_context_refs,
    );
    for context in ctx
        .contexts
        .semantic_context_refs
        .span_or_empty(state_contexts)
        .to_vec()
    {
        append_constraint_ref(
            &mut ctx.contexts.constraint_refs,
            &mut state_constraints,
            FlowConstraintKind::SemanticContext {
                context: context.context,
            },
        );
    }
    append_constraint_ref(
        &mut ctx.contexts.constraint_refs,
        &mut state_constraints,
        FlowConstraintKind::BorrowState {
            state: borrow_state_handle,
        },
    );
    append_contiguous_borrow_root_constraints(
        &mut ctx.contexts.constraint_refs,
        &mut state_constraints,
        borrow_state.writable_roots,
    );
    let mut active_contexts =
        clone_flow_contexts(&mut ctx.contexts.semantic_context_refs, state_contexts);
    let mut active_constraints =
        clone_constraint_refs(&mut ctx.contexts.constraint_refs, state_constraints);
    let state_invalidations_start = ctx.invalidations.events.len();
    let state_borrow_activations_start = ctx.borrow_lifetimes.activations.len();
    let state_borrow_weakenings_start = ctx.borrow_lifetimes.weakenings.len();
    let state_boundary_edges_start = ctx.boundaries.edges.len();
    let state_statements_start = ctx.control.statements.len();
    let state_exits_start = ctx.control.exits.len();
    let state_calls = append_state_statement_flow_facts(
        program,
        borrow,
        proof,
        semantic,
        domains,
        ctx,
        machine,
        state,
        &mut active_contexts,
        &mut active_constraints,
        borrow_state,
    );
    active_constraints = filter_expired_borrow_loans(
        &mut ctx.borrow_lifetimes.weakenings,
        &mut ctx.contexts.constraint_refs,
        active_constraints,
        borrow,
        program
            .statement_table
            .statements(state.statement_nodes)
            .len(),
        FlowBorrowWeakeningReason::StateExit,
    );
    append_state_exit_facts(
        program,
        proof,
        semantic,
        ctx,
        machine.symbol,
        state.symbol,
        Default::default(),
        active_contexts,
        active_constraints,
    );

    ctx.control.states.append(FlowStateFact {
        machine_symbol: machine.symbol,
        state_symbol: state.symbol,
        writable_roots: borrow_state.writable_roots,
        mutable_parameter_count: borrow_state.mutable_parameter_count,
        entry_semantic_contexts: state_contexts,
        entry_constraints: state_constraints,
        invalidations: appended_span_since(&ctx.invalidations.events, state_invalidations_start),
        borrow_activations: appended_span_since(
            &ctx.borrow_lifetimes.activations,
            state_borrow_activations_start,
        ),
        borrow_weakenings: appended_span_since(
            &ctx.borrow_lifetimes.weakenings,
            state_borrow_weakenings_start,
        ),
        boundary_edges: appended_span_since(&ctx.boundaries.edges, state_boundary_edges_start),
        statements: appended_span_since(&ctx.control.statements, state_statements_start),
        calls: state_calls,
        exits: appended_span_since(&ctx.control.exits, state_exits_start),
        service_reach: Default::default(),
        suspension: Default::default(),
        blocking: Default::default(),
    });
}
