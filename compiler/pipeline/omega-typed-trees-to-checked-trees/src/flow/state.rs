use super::*;

pub(super) fn build_state_flow_fact(
    program: &omega_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    domains: &DomainFacts,
    effects: &omega_effects::EffectPlan,
    ctx: &mut FlowBuildContext,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    machine_effects: Option<&omega_effects::MachineEffects>,
) {
    let Some((borrow_state_handle, borrow_state)) =
        borrow_state_fact(borrow, machine.symbol, state.symbol)
    else {
        return;
    };

    let state_effects = effects_state(effects, machine_effects, state.symbol);
    let mut state_contexts = omega_core::arena::HandleSpan::empty();
    let mut state_constraints = omega_core::arena::HandleSpan::empty();
    append_flow_contexts_for_points(
        semantic,
        &mut ctx.semantic_context_refs,
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
        &mut ctx.constraint_refs,
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
    append_constraint_ref(
        &mut ctx.constraint_refs,
        &mut state_constraints,
        FlowConstraintKind::BorrowState {
            state: borrow_state_handle,
        },
    );
    append_contiguous_borrow_root_constraints(
        &mut ctx.constraint_refs,
        &mut state_constraints,
        borrow_state.writable_roots,
    );
    let mut active_contexts = clone_flow_contexts(&mut ctx.semantic_context_refs, state_contexts);
    let mut active_constraints = clone_constraint_refs(&mut ctx.constraint_refs, state_constraints);
    let state_invalidations_start = ctx.invalidations.len();
    let state_borrow_activations_start = ctx.borrow_activations.len();
    let state_borrow_weakenings_start = ctx.borrow_weakenings.len();
    let state_moves_start = ctx.moves.len();
    let state_drops_start = ctx.drops.len();
    let state_statements_start = ctx.statements.len();
    let state_calls = append_state_statement_flow_facts(
        program,
        borrow,
        proof,
        semantic,
        domains,
        effects,
        ctx,
        machine,
        state,
        state_effects,
        &mut active_contexts,
        &mut active_constraints,
        borrow_state,
    );
    active_constraints = filter_expired_borrow_loans(
        &mut ctx.borrow_weakenings,
        &mut ctx.constraint_refs,
        active_constraints,
        borrow,
        program
            .statement_table
            .statements(state.statement_nodes)
            .len(),
        FlowBorrowWeakeningReason::StateExit,
    );
    let state_exits = append_state_exit_facts(
        proof,
        semantic,
        ctx,
        machine.symbol,
        state.symbol,
        active_contexts,
        active_constraints,
    );

    ctx.states.append(FlowStateFact {
        machine_symbol: machine.symbol,
        state_symbol: state.symbol,
        writable_roots: borrow_state.writable_roots,
        mutable_parameter_count: borrow_state.mutable_parameter_count,
        entry_semantic_contexts: state_contexts,
        entry_constraints: state_constraints,
        invalidations: appended_span_since(&ctx.invalidations, state_invalidations_start),
        borrow_activations: appended_span_since(
            &ctx.borrow_activations,
            state_borrow_activations_start,
        ),
        borrow_weakenings: appended_span_since(
            &ctx.borrow_weakenings,
            state_borrow_weakenings_start,
        ),
        moves: appended_span_since(&ctx.moves, state_moves_start),
        drops: appended_span_since(&ctx.drops, state_drops_start),
        statements: appended_span_since(&ctx.statements, state_statements_start),
        calls: state_calls,
        exits: state_exits,
        direct_effects: state_effects
            .map(|state_effects| state_effects.direct)
            .unwrap_or_else(omega_effects::EffectSet::empty),
        transitive_effects: state_effects
            .map(|state_effects| state_effects.transitive)
            .unwrap_or_else(omega_effects::EffectSet::empty),
    });
}
