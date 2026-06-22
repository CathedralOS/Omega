use super::*;

pub(crate) fn build_flow_facts(
    program: &omega_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    domains: &DomainFacts,
    effects: &omega_effects::EffectPlan,
) -> FlowFacts {
    let mut ctx = FlowBuildContext::new(borrow, proof, semantic);

    for machine in program.machines() {
        let machine_effects = effects_machine(effects, machine.symbol);

        for state in program.machine_states(machine) {
            build_state_flow_fact(
                program,
                borrow,
                proof,
                semantic,
                domains,
                effects,
                &mut ctx,
                machine,
                state,
                machine_effects,
            );
        }
    }

    ctx.finish()
}
