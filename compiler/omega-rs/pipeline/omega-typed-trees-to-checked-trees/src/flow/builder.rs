use super::*;

#[cfg(test)]
pub(crate) fn build_flow_facts(
    program: &omega_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    domains: &DomainFacts,
    effects: &omega_effects::EffectPlan,
) -> FlowFacts {
    let service_reaches = omega_effects::infer_service_reaches(program, effects);
    build_flow_facts_with_service_reaches(
        program,
        borrow,
        proof,
        semantic,
        domains,
        effects,
        &service_reaches,
    )
}

pub(crate) fn build_flow_facts_with_service_reaches(
    program: &omega_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    domains: &DomainFacts,
    effects: &omega_effects::EffectPlan,
    service_reaches: &omega_effects::ServiceReachInferencePlan,
) -> FlowFacts {
    let mut ctx = FlowBuildContext::new(borrow, proof, semantic);

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            build_state_flow_fact(
                program, borrow, proof, semantic, domains, &mut ctx, machine, state,
            );
        }
    }

    let mut flow = ctx.finish();
    attach_reach_summaries(&mut flow, service_reaches, effects);
    flow
}
