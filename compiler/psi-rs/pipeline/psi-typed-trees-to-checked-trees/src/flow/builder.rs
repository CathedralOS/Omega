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
    )
}

pub(crate) fn build_flow_facts_with_service_reaches(
    program: &psi_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    domains: &DomainFacts,
    operational: &psi_effects::OperationalPlan,
    service_reaches: &psi_effects::ServiceReachInferencePlan,
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
    attach_reach_summaries(&mut flow, service_reaches, operational);
    flow
}
