use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn build_call_flow_fact(
    program: &omega_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &FactPlan,
    domains: &DomainFacts,
    effects: &omega_effects::EffectPlan,
    ctx: &mut FlowBuildContext,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    state_effects: Option<&omega_effects::StateEffects>,
    active_contexts: &mut omega_core::arena::HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut omega_core::arena::HandleSpan<FlowConstraintRef>,
    borrow_call: &BorrowCallFact,
) -> FlowCallFact {
    let effect_call = effects_call(effects, state_effects, borrow_call);
    let contract_call = proof_contract_call(
        proof,
        machine.symbol,
        state.symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    );
    let entry = build_call_entry_contexts(
        borrow,
        ctx,
        *active_contexts,
        *active_constraints,
        borrow_call,
    );
    let requires = build_call_requires_contexts(semantic, ctx, machine, state, borrow_call);
    let invalidation = apply_call_invalidations(
        program,
        borrow,
        semantic,
        domains,
        ctx,
        machine,
        state,
        *active_contexts,
        *active_constraints,
        borrow_call,
    );
    let exit = build_call_exit_contexts(
        semantic,
        ctx,
        machine,
        state,
        borrow_call,
        invalidation.post_contexts,
        invalidation.post_constraints,
    );
    append_call_ownership_events(program, ctx, machine, state, borrow_call);
    let boundary_edges = append_call_boundary_edges(program, ctx, borrow_call);
    *active_contexts = clone_flow_contexts(&mut ctx.semantic_context_refs, exit.contexts);
    *active_constraints = clone_constraint_refs(&mut ctx.constraint_refs, exit.constraints);

    FlowCallFact {
        statement_index: borrow_call.statement_index,
        call_ordinal: borrow_call.call_ordinal,
        receiver_symbol: borrow_call.receiver_symbol,
        target_symbol: borrow_call.target_symbol,
        has_receiver: borrow_call.has_receiver,
        accesses: borrow_call.accesses,
        entry_semantic_contexts: entry.contexts,
        entry_constraints: entry.constraints,
        requires_contexts: requires.contexts,
        requires_constraints: requires.constraints,
        exit_semantic_contexts: exit.contexts,
        exit_constraints: exit.constraints,
        invalidations: invalidation.invalidations,
        boundary_edges,
        requires: contract_call
            .map(|call| call.requires)
            .unwrap_or_else(HandleSpan::empty),
        ensures: contract_call
            .map(|call| call.ensures)
            .unwrap_or_else(HandleSpan::empty),
        direct_effects: effect_call
            .map(|call| call.direct)
            .unwrap_or_else(omega_effects::EffectSet::empty),
        transitive_effects: effect_call
            .map(|call| call.transitive)
            .unwrap_or_else(omega_effects::EffectSet::empty),
    }
}
