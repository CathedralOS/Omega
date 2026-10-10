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
    let mut equations = Vec::with_capacity(nodes.len());
    for node in nodes {
        let plan = checked_trees::contract_plan_by_machine(plans, node.machine)
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
            let member_plan = checked_trees::contract_plan_by_machine(plans, *member)
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
            if let Some(target_plan) =
                checked_trees::contract_plan_by_machine(plans, invocation.target_machine)
            {
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
                let capsule = capsules
                    .iter()
                    .find(|capsule| {
                        capsule.target_machine() == invocation.target_machine
                            && capsule.target_state() == invocation.target_state
                    })
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
        for equation in &equations {
            let mut causes = equation.direct.clone();
            for dependency in &equation.dependencies {
                let (_, target) = resolved
                    .iter()
                    .find(|(machine, _)| machine == dependency)
                    .expect("every summary dependency belongs to the viable fixed point");
                causes.extend_from_slice(target);
            }
            causes.sort_unstable();
            causes.dedup();
            let (_, current) = resolved
                .iter_mut()
                .find(|(machine, _)| *machine == equation.machine)
                .expect("every viable node has an initial cause summary");
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
        .filter(|node| {
            program
                .machine_by_symbol(node.machine)
                .is_some_and(|machine| machine.target_family_interface)
        })
        .map(|node| {
            let equation = equations
                .iter()
                .find(|equation| equation.machine == node.machine)
                .expect("a prepared family has its cause equation");
            let mut inferred = Vec::new();
            for dependency in &equation.dependencies {
                let (_, causes) = resolved
                    .iter()
                    .find(|(machine, _)| machine == dependency)
                    .expect("every family dependency has a resolved cause summary");
                inferred.extend_from_slice(causes);
            }
            inferred.sort_unstable();
            inferred.dedup();
            (node.machine, inferred)
        })
        .collect()
}
