//! Bounded path alternatives for private crash guard checking. A merge retains
//! each incoming conjunction until the crash consumer proves every alternative;
//! ordinary operation obligations still use the independent intersection walk.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{BlockId, MachineId, Proposition};
use terminal_psi::{TerminalMachine, TerminalModule};

use crate::ModuleError;

use super::{
    ReconstructedCrashSiteFacts, crash_field_origins,
    machine_context::MachineReconstructionContext, operation_facts, terminator_facts,
};

const MAXIMUM_BLOCK_VISITS: usize = 4096;
const MAXIMUM_FACT_COPIES: usize = 4096;

#[derive(Default)]
struct Budget {
    block_visits: usize,
    fact_copies: usize,
    join_snapshot_facts: usize,
}

impl Budget {
    fn visit(&mut self, machine: MachineId) -> Result<(), ModuleError> {
        self.block_visits = self
            .block_visits
            .checked_add(1)
            .filter(|count| *count <= MAXIMUM_BLOCK_VISITS)
            .ok_or(ModuleError::CrashSiteReconstructionLimitExceeded(machine))?;
        Ok(())
    }

    fn retain(&mut self, facts: usize, machine: MachineId) -> Result<(), ModuleError> {
        self.fact_copies = self
            .fact_copies
            .checked_add(facts)
            .filter(|count| *count <= MAXIMUM_FACT_COPIES)
            .ok_or(ModuleError::CrashSiteReconstructionLimitExceeded(machine))?;
        Ok(())
    }

    fn snapshot(&mut self, facts: usize, machine: MachineId) -> Result<(), ModuleError> {
        self.join_snapshot_facts = self
            .join_snapshot_facts
            .checked_add(facts)
            .filter(|count| *count <= MAXIMUM_FACT_COPIES)
            .ok_or(ModuleError::CrashSiteReconstructionLimitExceeded(machine))?;
        Ok(())
    }
}

/// Only reconvergence can repeat work in an acyclic graph. Keep the exact
/// ordered premises: neither set equality nor a hash establishes identity.
fn join_histories(machine: &TerminalMachine) -> BTreeMap<BlockId, BTreeSet<Vec<Proposition>>> {
    // Repeated loop states are not an invariant proof. Preserve the existing
    // exhaustive, fail-closed walk for cyclic guarded-crash machines.
    if !crate::control_graph::feedback_edges(machine).is_empty() {
        return BTreeMap::new();
    }
    let mut arrivals = BTreeSet::new();
    let mut joins = BTreeMap::new();
    for successors in crate::control_graph::successors(machine).values() {
        for (_, target) in successors {
            if !arrivals.insert(*target) {
                joins.entry(*target).or_default();
            }
        }
    }
    joins
}

pub(super) fn reconstruct(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
) -> Result<Vec<ReconstructedCrashSiteFacts>, ModuleError> {
    debug_assert!(machine.ranked_scc.is_none());
    let context = MachineReconstructionContext::new(module, machine, true);
    let mut primitive_snapshots =
        super::primitive_snapshots::PrimitiveSnapshots::new(machine, &context.blocks);
    let mut pending = vec![(machine.entry, Vec::<Proposition>::new())];
    let mut sites = Vec::new();
    let mut budget = Budget::default();
    let mut histories = join_histories(machine);
    let ignored_backedges = BTreeSet::new();
    while let Some((current, mut axioms)) = pending.pop() {
        let block = context
            .blocks
            .get(&current)
            .expect("validated successor names an exact block");
        axioms
            .retain(|proposition| crash_field_origins::retains_entry_meaning(proposition, machine));
        if let Some(history) = histories.get_mut(&current) {
            if history.contains(&axioms) {
                // The same block and ordered premises deterministically
                // produce the same successors and crash questions. The first
                // DFS visit covers them; no distinct alternative is omitted.
                continue;
            }
            // Each snapshot is a subset of one already charged incoming path.
            // Its separate storage cap therefore cannot tighten the transport
            // budget, but bounds the additional retained copies to 4,096 facts.
            budget.snapshot(axioms.len(), machine.id)?;
            history.insert(axioms.clone());
        }
        budget.visit(machine.id)?;
        // These obligations are not accepted or exported here. Final ordinary
        // proof reconstruction independently checks every operation and call.
        let mut operation_obligations = Vec::new();
        for (position, operation) in block.operations.iter().enumerate() {
            let prior_facts = axioms.len();
            primitive_snapshots.append_read_equation(
                current,
                position,
                operation,
                &context.value_types,
                &mut axioms,
            )?;
            operation_facts::append_operation(
                module,
                machine,
                operation,
                machines,
                &context.value_types,
                operation_facts::OperationFactPurpose::PrivateCrashPredicates,
                &mut axioms,
                &mut operation_obligations,
            )?;
            budget.retain(axioms.len().saturating_sub(prior_facts), machine.id)?;
            axioms.retain(|proposition| {
                crash_field_origins::retains_entry_meaning(proposition, machine)
            });
            operation_obligations.clear();
        }
        let mut successors = BTreeMap::new();
        let mut exits = Vec::new();
        let mut outcome_exits = BTreeMap::new();
        let prior_sites = sites.len();
        terminator_facts::append_terminator(
            module,
            &block.terminator,
            current,
            machine,
            &context.blocks,
            machines,
            &|id| context.value_term(id),
            context.reconstruct_path_facts,
            true,
            axioms,
            &mut successors,
            &mut exits,
            None,
            &mut outcome_exits,
            &mut operation_obligations,
            &mut sites,
            &ignored_backedges,
        );
        for site in &sites[prior_sites..] {
            budget.retain(site.semantic_axioms.len(), machine.id)?;
        }
        // Do not intersect or infer reachability from a claimed guard. Even
        // contradictory path facts are retained for the consumer's kernel
        // proof of infeasibility; missing rows never imply a justified crash.
        for (target, paths) in successors.into_iter().rev() {
            for path in paths.into_iter().rev() {
                budget.retain(path.len(), machine.id)?;
                pending.push((target, path));
            }
        }
    }
    Ok(sites)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_visits_and_retained_facts_have_independent_hard_limits() {
        let machine = MachineId::new(1).unwrap();
        let mut budget = Budget::default();
        for _ in 0..MAXIMUM_BLOCK_VISITS {
            budget.visit(machine).unwrap();
        }
        assert!(matches!(budget.visit(machine),
            Err(ModuleError::CrashSiteReconstructionLimitExceeded(owner)) if owner == machine));
        let mut budget = Budget::default();
        budget.retain(MAXIMUM_FACT_COPIES, machine).unwrap();
        assert!(matches!(budget.retain(1, machine),
            Err(ModuleError::CrashSiteReconstructionLimitExceeded(owner)) if owner == machine));
        let mut budget = Budget::default();
        assert!(budget.retain(usize::MAX, machine).is_err());
        let mut budget = Budget::default();
        budget.retain(MAXIMUM_FACT_COPIES, machine).unwrap();
        budget.snapshot(MAXIMUM_FACT_COPIES, machine).unwrap();
        assert_eq!(budget.fact_copies, MAXIMUM_FACT_COPIES);
        assert!(budget.snapshot(1, machine).is_err());
        let mut budget = Budget::default();
        assert!(budget.snapshot(usize::MAX, machine).is_err());
    }
}
