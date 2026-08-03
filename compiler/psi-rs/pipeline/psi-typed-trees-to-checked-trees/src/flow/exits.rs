use super::*;

pub(super) fn append_state_exit_facts(
    proof: &ProofFacts,
    semantic: &FactPlan,
    ctx: &mut FlowBuildContext,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    active_contexts: psi_arena::HandleSpan<FlowSemanticContextRef>,
    active_constraints: psi_arena::HandleSpan<FlowConstraintRef>,
) -> psi_arena::HandleSpan<FlowExitFact> {
    let mut state_exits = psi_arena::HandleSpan::empty();

    for contract_exit in proof.contract_exits.iter().filter_map(|(_, exit)| {
        (exit.machine_symbol == machine_symbol && exit.state_symbol == state_symbol).then_some(exit)
    }) {
        let entry_exit_contexts =
            clone_flow_contexts(&mut ctx.contexts.semantic_context_refs, active_contexts);
        let entry_constraints =
            clone_constraint_refs(&mut ctx.contexts.constraint_refs, active_constraints);
        let mut ensures_contexts = psi_arena::HandleSpan::empty();
        let mut ensures_constraints = psi_arena::HandleSpan::empty();
        append_flow_contexts_for_points(
            semantic,
            &mut ctx.contexts.semantic_context_refs,
            &mut ensures_contexts,
            &[ProgramPoint::Exit {
                machine_symbol,
                state_symbol,
                statement_index: contract_exit.statement_index,
            }],
        );
        append_semantic_constraints_for_points(
            semantic,
            &mut ctx.contexts.constraint_refs,
            &mut ensures_constraints,
            &[ProgramPoint::Exit {
                machine_symbol,
                state_symbol,
                statement_index: contract_exit.statement_index,
            }],
        );

        ctx.control.exits.append_to_span(
            &mut state_exits,
            FlowExitFact {
                machine_symbol,
                state_symbol,
                statement_index: contract_exit.statement_index,
                entry_semantic_contexts: entry_exit_contexts,
                entry_constraints,
                ensures_contexts,
                ensures_constraints,
                ensures: contract_exit.ensures,
            },
        );
    }

    state_exits
}
