//! Known missing target bodies in direct call closures.
//!
//! A portable adapter can call a family that only hosted targets implement.
//! Its source remains checked on a freestanding target, but the adapter cannot
//! make a complete provider candidate there. Compute this negative fact before
//! selection so an otherwise unique map cannot force an unavailable service
//! into a library review. Candidates remain whole maps, never combined rows.
//!
//! This is not a realizability certificate. Bodyless requirements still await
//! provider selection; implicit operators, cleanup and dynamic dispatch still
//! need their existing realization checks. Only an exact direct call into a
//! missing target body propagates absence. Reverse propagation reaches the
//! fixed point through recursive closures without caching provisional success.

use crate::ProviderPlanningContext;
use checked_trees::typed_trees::TypedTrees;
use checked_trees::typed_trees::typed_trees::inspection::callees::ExplicitStateCallees;
use symbols::SymbolHandle;

pub(crate) struct DirectTargetBodyAbsences {
    /// Positions belong to the immutable typed machine roster of this query.
    absent: Vec<bool>,
}

impl DirectTargetBodyAbsences {
    pub(crate) fn derive(context: ProviderPlanningContext<'_>, target: Option<&str>) -> Self {
        let typed = context.typed;
        let machines = typed.machines();
        let mut absent = vec![false; machines.len()];
        let mut callers = vec![Vec::new(); machines.len()];
        let mut callees = ExplicitStateCallees::default();
        for (position, machine) in machines.iter().enumerate() {
            if !super::realizes_machine(context, machine, target) {
                absent[position] = true;
                continue;
            }
            for state in typed.machine_states(machine) {
                for called in callees.collect(typed, state) {
                    // A requirement without an authored machine is a provider
                    // dependency, not a family with a missing target member.
                    let Some(declaration) = super::called_machine(typed, *called) else {
                        continue;
                    };
                    let Some(realization) =
                        super::realized_row_machine(context, declaration.symbol, target)
                    else {
                        absent[position] = true;
                        continue;
                    };
                    if let Some(callee) = typed.machine_position_by_symbol(realization.symbol) {
                        callers[callee].push(position);
                    }
                }
            }
        }
        let mut frontier: Vec<usize> = absent
            .iter()
            .enumerate()
            .filter_map(|(position, absent)| absent.then_some(position))
            .collect();
        while let Some(callee) = frontier.pop() {
            for &caller in &callers[callee] {
                if !std::mem::replace(&mut absent[caller], true) {
                    frontier.push(caller);
                }
            }
        }
        Self { absent }
    }

    pub(crate) fn contains(&self, typed: &TypedTrees, machine: SymbolHandle) -> bool {
        typed
            .machine_position_by_symbol(machine)
            .is_some_and(|position| self.absent[position])
    }
}
