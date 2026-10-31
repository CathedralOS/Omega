//! Conservative public cause envelopes over the prepared crash dependency graph.
//! Invocation proofs may narrow private call routes, but cannot erase a cause
//! from a generated family contract. Authored member guards stay on the head;
//! only inferred member causes need an additional unconditional permission.

use super::private_summaries::{SummaryNode, needs_inferred_summary};
use checked_trees::CrashCause;
use symbols::SymbolHandle;

struct CauseEquation {
    machine: SymbolHandle,
    direct: Vec<CrashCause>,
    dependencies: Vec<SymbolHandle>,
}

pub(super) fn infer(
    program: &dyn validation::CheckingInput,
    nodes: &[SummaryNode],
    capsules: &[checked_trees::CrashContractCapsule],
    plans: &[checked_trees::MachineContractPlan],
) -> Vec<(SymbolHandle, Vec<CrashCause>)> {
    let mut plan_by_machine =
        symbols::SymbolKeyMap::<SymbolHandle, &checked_trees::MachineContractPlan>::default();
    for plan in plans {
        plan_by_machine.entry(plan.machine).or_insert(plan);
    }
    let mut capsule_by_target = symbols::SymbolKeyMap::<
        (SymbolHandle, SymbolHandle),
        &checked_trees::CrashContractCapsule,
    >::default();
    for capsule in capsules {
        capsule_by_target
            .entry((capsule.target_machine(), capsule.target_state()))
            .or_insert(capsule);
    }
    let mut equations = Vec::with_capacity(nodes.len());
    let mut node_index_by_machine = symbols::SymbolKeyMap::<SymbolHandle, usize>::default();
    for (index, node) in nodes.iter().enumerate() {
        node_index_by_machine.entry(node.machine).or_insert(index);
        let plan = plan_by_machine
            .get(&node.machine)
            .copied()
            .expect("a viable summary node has its local contract plan");
        let machine = program
            .machine_by_symbol(node.machine)
            .expect("a viable summary node has its typed declaration");
        let mut direct = Vec::new();
        let mut dependencies = Vec::new();
        if machine.target_family_interface {
            direct.extend(plan.crash.published().iter().map(|bucket| bucket.cause()));
        } else {
            direct.extend(plan.crash.checked_sites().iter().map(|site| site.cause()));
            if !plan.crash.trapping_sites().is_empty() {
                direct.push(CrashCause::Trap);
            }
            for operator in plan.crash.checked_operators() {
                direct.extend(operator.published.iter().map(|bucket| bucket.cause()));
            }
        }
        for member in &node.family_members {
            let member_plan = plan_by_machine
                .get(member)
                .copied()
                .expect("a viable family member has its local contract plan");
            let member_machine = program
                .machine_by_symbol(*member)
                .expect("a viable family member has its typed declaration");
            if needs_inferred_summary(member_machine, member_plan) {
                dependencies.push(*member);
            }
            // Pinned member routes already occur in the authored head's
            // fresh telescope; membership does not widen their guards.
        }
        for invocation in &node.invocations {
            if let Some(&target_plan) = plan_by_machine.get(&invocation.target_machine) {
                let target_machine = program
                    .machine_by_symbol(invocation.target_machine)
                    .expect("a viable local invocation has its typed declaration");
                if needs_inferred_summary(target_machine, target_plan) {
                    dependencies.push(invocation.target_machine);
                } else {
                    direct.extend(
                        target_plan
                            .crash
                            .published()
                            .iter()
                            .map(|bucket| bucket.cause()),
                    );
                }
            } else {
                let capsule = capsule_by_target
                    .get(&(invocation.target_machine, invocation.target_state))
                    .copied()
                    .expect("a viable abstract invocation has its pinned crash capsule");
                direct.extend(
                    capsule
                        .published_buckets()
                        .iter()
                        .map(|bucket| bucket.cause()),
                );
            }
        }
        direct.sort_unstable();
        direct.dedup();
        equations.push(CauseEquation {
            machine: node.machine,
            direct,
            dependencies,
        });
    }
    let mut resolved = equations
        .iter()
        .map(|equation| (equation.machine, equation.direct.clone()))
        .collect::<Vec<_>>();
    loop {
        let mut changed = false;
        for (equation_index, equation) in equations.iter().enumerate() {
            let mut causes = equation.direct.clone();
            for dependency in &equation.dependencies {
                let &target_index = node_index_by_machine
                    .get(dependency)
                    .expect("every summary dependency belongs to the viable fixed point");
                causes.extend_from_slice(&resolved[target_index].1);
            }
            causes.sort_unstable();
            causes.dedup();
            let current = &mut resolved[equation_index].1;
            if *current != causes {
                *current = causes;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| {
            program
                .machine_by_symbol(node.machine)
                .is_some_and(|machine| machine.target_family_interface)
        })
        .map(|(equation_index, node)| {
            let equation = &equations[equation_index];
            let mut inferred = Vec::new();
            for dependency in &equation.dependencies {
                let &target_index = node_index_by_machine
                    .get(dependency)
                    .expect("every family dependency has a resolved cause summary");
                inferred.extend_from_slice(&resolved[target_index].1);
            }
            inferred.sort_unstable();
            inferred.dedup();
            (node.machine, inferred)
        })
        .collect()
}
