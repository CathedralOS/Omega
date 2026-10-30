//! Propagate missing declaration custody through direct provider call closures.
//!
//! Scope membership is source provenance, not target applicability. Bodyless
//! requirements remain provider dependencies; this walk grants no execution.

use crate::ProviderPlanningContext;
use checked_trees::typed_trees::TypedTrees;
use checked_trees::typed_trees::typed_trees::inspection::callees::ExplicitStateCallees;
use symbols::SymbolHandle;

pub(crate) struct DirectDeclarationAbsences {
    /// Positions belong to the immutable typed machine roster of this query.
    absent: Vec<bool>,
}

impl DirectDeclarationAbsences {
    pub(crate) fn derive(context: ProviderPlanningContext<'_>) -> Self {
        let typed = context.typed;
        let machines = typed.machines();
        let mut absent = vec![false; machines.len()];
        let mut callers = vec![Vec::new(); machines.len()];
        let mut callees = ExplicitStateCallees::default();
        for (position, machine) in machines.iter().enumerate() {
            if !context.contains(machine.symbol) {
                absent[position] = true;
                continue;
            }
            for state in typed.machine_states(machine) {
                for called in callees.collect(typed, state) {
                    // A requirement without an authored machine is a provider
                    // dependency, not a missing authored declaration.
                    let Some(declaration) = super::called_machine(typed, *called) else {
                        continue;
                    };
                    let Some(realization) = super::scoped_row_machine(context, declaration.symbol)
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
