//! An unchanged call through a local wrapper has the wait predicate of its
//! selected callee, expressed on the wrapper's own structural formals.

use super::facts::Facts;
use super::spin::{ForwardedWait, SpinWait, forwarded_wait, spin_wait};
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use terminal_psi::{TargetMachineCallable, TerminalModule};

#[derive(Default)]
pub(super) struct Summaries(Facts<MachineId, Option<SpinWait>>);

impl Summaries {
    pub fn get(
        &mut self,
        module: &TerminalModule,
        entry: MachineId,
        boundary: &impl Fn(BoundaryMachineId) -> Option<SpinWait>,
    ) -> Option<SpinWait> {
        let mut chain = Vec::<(MachineId, ForwardedWait)>::new();
        let mut visiting = Facts::default();
        let mut current = entry;
        let mut summary = loop {
            if let Some(known) = self.0.get(&current) {
                break known.clone();
            }
            if visiting.insert(current, ()).is_some() {
                break None;
            }

            let machine = module.machine(current)?;
            let leaf = spin_wait(module, machine);
            if leaf.is_some() {
                self.0.insert(current, leaf.clone());
                break leaf;
            }
            let Some(forward) = forwarded_wait(module, machine) else {
                self.0.insert(current, None);
                break None;
            };
            let callee = forward.callee;
            chain.push((current, forward));
            match callee {
                TargetMachineCallable::Machine(id) => current = id,
                TargetMachineCallable::Boundary(id) => break boundary(id),
            }
        };
        for (caller, forward) in chain.into_iter().rev() {
            summary = summary.and_then(|wait| forward.apply(wait));
            self.0.insert(caller, summary.clone());
        }
        summary
    }
}

/// Recover a predicate for a selected module machine, including checked
/// forwarding through local machines or independently admitted suppliers.
pub fn selected_spin_wait(
    module: &TerminalModule,
    entry: MachineId,
    boundary: impl Fn(BoundaryMachineId) -> Option<SpinWait>,
) -> Option<SpinWait> {
    Summaries::default().get(module, entry, &boundary)
}
