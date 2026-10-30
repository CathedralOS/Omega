//! Propagate missing declaration custody through the prepared direct-call roster.
//!
//! Scope membership is source provenance, not target applicability. Bodyless
//! requirements remain provider dependencies; this walk grants no execution.

use language_semantics::direct_calls::DirectCallDeclarations;
use symbols::SymbolHandle;

pub(crate) struct DirectDeclarationAbsences<'declarations> {
    declarations: &'declarations DirectCallDeclarations<SymbolHandle>,
    absent: Vec<bool>,
}

impl<'declarations> DirectDeclarationAbsences<'declarations> {
    pub(crate) fn derive(
        declarations: &'declarations DirectCallDeclarations<SymbolHandle>,
    ) -> Self {
        let mut absent = vec![false; declarations.machines.len()];
        let mut callers = vec![Vec::new(); declarations.machines.len()];
        for (position, machine) in declarations.machines.iter().enumerate() {
            if !machine.in_scope {
                absent[position] = true;
                continue;
            }
            for called in machine.states.iter().flat_map(|state| &state.calls) {
                // A requirement without an authored machine remains a provider
                // dependency. Exact source-scope absence propagates to callers.
                let Some(callee) = called.machine else {
                    continue;
                };
                if !declarations.machines[callee].in_scope {
                    absent[position] = true;
                }
                callers[callee].push(position);
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
        Self {
            declarations,
            absent,
        }
    }

    pub(crate) fn contains(&self, machine: SymbolHandle) -> bool {
        self.declarations
            .called_machine_position(&machine)
            .is_some_and(|position| self.absent[position])
    }
}
