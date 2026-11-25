//! Which calls may run while an owned value is out of borrowed storage.
//!
//! ownership.md, "Borrowed-storage invariant windows": calls "may consume the
//! detached value or use disjoint storage only when their complete access and
//! outcome contracts cannot expose the incomplete owner. Unknown access is not
//! evidence of non-observation. Boundary/capability calls and other
//! invariant-window fences remain in force." dependent_values.md, "Invariant
//! windows", fences "Boundary/capability-carrying calls that may observe or
//! expose the affected storage, including through authority not named as an
//! argument", and counts "callbacks and authorized capability state".
//!
//! So the ownership walk lets a call run inside an open window only when the
//! callee is quiet: nothing it can run makes a boundary call, a service
//! instruction or a computed call, reaches a machine this module holds no
//! body for, or declares service reach or synchronous entries. Anything else
//! is refused, including a boundary declared `reaches none`, because nothing
//! in the spec yet says such a declaration is a complete access contract
//! (`OWNER_QUESTIONS.md#vacant-storage-boundary-calls`).

use semantic_vocabulary::MachineId;
use std::sync::OnceLock;
use terminal_psi::{
    OperationKind, TerminalAffineCleanupAction, TerminalMachine, TerminalModule, Terminator,
};

/// What running one operation or terminator can reach.
pub(super) enum Reach {
    /// No call and no service instruction.
    Plain,
    /// An ordinary call into this machine of the module.
    Calls(MachineId),
    /// A boundary call, service instruction or computed call: code whose
    /// access the module does not state.
    Fenced,
}

pub(super) fn operation(kind: &OperationKind) -> Reach {
    if let Some(callee) = kind.direct_callee() {
        return Reach::Calls(callee);
    }
    match kind {
        // The instruction services are the ones
        // `UnoptimizedMachineBehavior::capture` classifies; every register
        // access touches machine state outside the program's storage.
        OperationKind::BoundaryCall { .. }
        | OperationKind::CallMachine { .. }
        | OperationKind::CallDynamic { .. }
        | OperationKind::ActivateTaskRegister { .. }
        | OperationKind::ReadTaskRegister { .. }
        | OperationKind::ReloadSegmentDescriptors { .. }
        | OperationKind::ReadSegmentSelector { .. }
        | OperationKind::GlobalDescriptorTable { .. }
        | OperationKind::PortWrite { .. }
        | OperationKind::RegisterAccess { .. } => Reach::Fenced,
        _ => Reach::Plain,
    }
}

pub(super) fn terminator(terminator: &Terminator) -> Reach {
    match terminator {
        Terminator::InvokeNoNormal { callee, .. } => Reach::Calls(*callee),
        Terminator::ExternalCompletion { .. } => Reach::Fenced,
        _ => Reach::Plain,
    }
}

/// The quiet machines of one module. The table is built on the first call
/// made inside an open window, then shared by every walk over that module.
pub(super) struct CallFences<'m> {
    module: &'m TerminalModule,
    quiet: OnceLock<Vec<bool>>,
}

impl<'m> CallFences<'m> {
    pub(super) const fn new(module: &'m TerminalModule) -> Self {
        Self {
            module,
            quiet: OnceLock::new(),
        }
    }

    pub(super) const fn module(&self) -> &'m TerminalModule {
        self.module
    }

    /// A machine outside the module's dense catalog is never quiet.
    pub(super) fn quiet(&self, machine: MachineId) -> bool {
        let quiet = self
            .quiet
            .get_or_init(|| quiet_machines(&self.module.machines));
        usize::try_from(machine.get())
            .ok()
            .and_then(|raw| raw.checked_sub(1))
            .and_then(|slot| quiet.get(slot))
            .copied()
            .unwrap_or(false)
    }
}

fn slot(machines: &[TerminalMachine], machine: MachineId) -> Option<usize> {
    usize::try_from(machine.get())
        .ok()
        .and_then(|raw| raw.checked_sub(1))
        .filter(|slot| machines.get(*slot).is_some_and(|row| row.id == machine))
}

/// A machine is quiet when its own contract and body reach nothing fenced and
/// every machine it calls, including owned cleanup, is quiet. Noise spreads
/// from each noisy machine to its callers over one sorted edge list.
fn quiet_machines(machines: &[TerminalMachine]) -> Vec<bool> {
    let mut quiet = vec![true; machines.len()];
    // (callee slot, caller slot)
    let mut calls = Vec::new();
    let mut pending = Vec::new();
    for (caller, machine) in machines.iter().enumerate() {
        let contract = &machine.contract;
        let mut noisy = machine.blocks.is_empty()
            || slot(machines, machine.id) != Some(caller)
            || contract
                .declared_service_reach
                .as_ref()
                .is_some_and(|reach| !reach.is_empty())
            || !contract.published_service_ceiling.is_empty()
            || contract
                .declared_synchronous_entries
                .as_ref()
                .is_some_and(|entries| !entries.is_empty())
            || !contract.published_synchronous_entries.is_empty();
        let mut call = |callee: MachineId, noisy: &mut bool| match slot(machines, callee) {
            Some(callee) => calls.push((callee, caller)),
            None => *noisy = true,
        };
        for block in &machine.blocks {
            let reached = block
                .operations
                .iter()
                .map(|operation| self::operation(&operation.kind))
                .chain([terminator(&block.terminator)]);
            for reach in reached {
                match reach {
                    Reach::Plain => {}
                    Reach::Calls(callee) => call(callee, &mut noisy),
                    Reach::Fenced => noisy = true,
                }
            }
            for (_, actions) in block.terminator.cleanup_rosters() {
                for action in actions {
                    if let TerminalAffineCleanupAction::InvokeOwned(cleanup) = action {
                        call(cleanup.cleanup_machine, &mut noisy);
                    }
                }
            }
        }
        if noisy {
            quiet[caller] = false;
            pending.push(caller);
        }
    }
    calls.sort_unstable();
    calls.dedup();
    while let Some(callee) = pending.pop() {
        let first = calls.partition_point(|(row, _)| *row < callee);
        for &(_, caller) in calls[first..].iter().take_while(|(row, _)| *row == callee) {
            if quiet[caller] {
                quiet[caller] = false;
                pending.push(caller);
            }
        }
    }
    quiet
}
