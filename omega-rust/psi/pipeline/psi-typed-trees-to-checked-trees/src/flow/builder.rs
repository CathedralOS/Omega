use super::*;

#[cfg(test)]
pub(crate) fn build_flow_facts(
    program: &psi_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    domains: &DomainFacts,
    operational: &psi_effects::OperationalPlan,
) -> FlowFacts {
    let service_reaches = psi_effects::infer_service_reaches(program, operational);
    build_flow_facts_with_service_reaches(
        program,
        borrow,
        proof,
        semantic,
        domains,
        operational,
        &service_reaches,
        &Default::default(),
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_flow_facts_with_service_reaches(
    program: &psi_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    domains: &DomainFacts,
    operational: &psi_effects::OperationalPlan,
    service_reaches: &psi_effects::ServiceReachInferencePlan,
    scalar_expressions: &psi_checked_trees::CheckedScalarExpressionPlans,
) -> FlowFacts {
    // Reuse the ordinary effect/statement transfer once per input revision.
    // Each pass starts from declaration/proof facts, never a prior pass's
    // derived contexts. Inputs join immediately so source-ordered chains need
    // no pass per edge. A changed input can only weaken, never regain a value.
    let baseline = semantic.clone();
    let mut inputs = Vec::new();
    // Each state becomes reachable once; each formal can acquire a constant
    // then lose it to unknown once. Include one pass to observe convergence.
    let pass_limit = 1 + program
        .machines()
        .iter()
        .map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .map(|state| 1 + 2 * program.state_parameters(state).len())
                .sum::<usize>()
        })
        .sum::<usize>();
    for pass in 0..pass_limit {
        if pass != 0 {
            *semantic = baseline.clone();
        }
        let mut ctx = FlowBuildContext::new(borrow, proof, semantic, scalar_expressions);
        ctx.state_value_inputs = inputs;
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                build_state_flow_fact(
                    program, borrow, proof, semantic, domains, &mut ctx, machine, state,
                );
            }
        }
        // Inputs arriving before a state's entry was built are already in its
        // contexts. Only a change after that point requires rebuilding flow.
        if !ctx.state_value_inputs_changed_after_build {
            let mut flow = ctx.finish();
            attach_reach_summaries(&mut flow, service_reaches, operational);
            return flow;
        }
        inputs = std::mem::take(&mut ctx.state_value_inputs);
    }
    // No provisional input fact survives a nonconvergent graph.
    *semantic = baseline;
    let mut ctx = FlowBuildContext::new(borrow, proof, semantic, scalar_expressions);
    // Unknown is absorbing: immediate joins during fallback cannot establish
    // a new provisional constant in a state built later in this pass.
    ctx.state_value_inputs = super::state_values::unknown_inputs(program);
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            build_state_flow_fact(
                program, borrow, proof, semantic, domains, &mut ctx, machine, state,
            );
        }
    }
    let mut flow = ctx.finish();
    attach_reach_summaries(&mut flow, service_reaches, operational);
    flow
}
