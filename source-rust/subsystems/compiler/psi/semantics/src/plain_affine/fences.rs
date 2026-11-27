//! Which calls may run while an owned value is out of borrowed storage.
//!
//! ownership.md, "Boundary calls during restoration": a call may consume a
//! value moved out of mutably borrowed storage when "the requirement's
//! admitted contract and the arguments at this call site ... establish that
//! the call cannot observe or expose the incomplete owner, and that every
//! path that hands the storage back first restores it. Unknown access or
//! outcomes require refusal. The decision must hold for every implementation
//! allowed by the requirement." It rejects the call "if a carried reference,
//! callback or provider authority could expose the incomplete connection",
//! and "`blocks` alone permits waiting on the worker". dependent_values.md,
//! "Invariant windows", applies the same test to ordinary calls and counts
//! "argument evaluation, callbacks and authorized capability state".
//!
//! The ownership walk judges each part where it can see it. Its use check
//! refuses an argument that moves or lends the vacant place or an ancestor
//! of it, such as `@holder` while `holder.inner` is out. This table judges what the
//! callee can do: an ordinary callee by everything its body can run, a
//! boundary requirement by its declared contract alone, so the answer holds
//! for every provider Build may select. A normal return continues in the
//! caller, and the walk checks every later handback there; a crash route or
//! process exit abandons the window under its own contract.

use semantic_vocabulary::{BoundaryMachineId, MachineId};
use std::sync::OnceLock;
use terminal_psi::{
    BoundaryMachineDeclaration, ExternalReach, OperationKind, TerminalAffineCleanupAction,
    TerminalMachine, TerminalModule, Terminator,
};

/// Why a call may not run while an owned value is moved out of borrowed
/// storage: the part of the callee's contract that could let it observe or
/// expose the incomplete owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallFence {
    /// The callee may enter a callback: an `invokes` entry or a callable
    /// parameter it declares it may call. That program code is not bounded
    /// by this call's arguments.
    Callback,
    /// The callee may reach a service, or touch machine state outside the
    /// program's storage, whose authority is not named by an argument.
    ServiceReach,
    /// The module does not state what the callee can access: a computed or
    /// dynamic call, or a machine or requirement without a checked
    /// declaration in this module.
    UnknownAccess,
    /// The callee may suspend, which keeps the suspension rule.
    Suspension,
}

impl CallFence {
    pub const fn message(self) -> &'static str {
        match self {
            Self::Callback => {
                "cannot make a call that may enter a callback while an owned value is moved \
                 out of borrowed storage: the callback could observe the incomplete owner; \
                 store a replacement into the vacant place before this call"
            }
            Self::ServiceReach => {
                "cannot make a call that may reach a service while an owned value is moved \
                 out of borrowed storage: authority the call holds outside its arguments \
                 could expose the incomplete owner; store a replacement into the vacant \
                 place before this call"
            }
            Self::UnknownAccess => {
                "cannot make a call whose access is unknown while an owned value is moved \
                 out of borrowed storage: nothing shows it cannot observe the incomplete \
                 owner; store a replacement into the vacant place before this call"
            }
            Self::Suspension => {
                "cannot make a call that may suspend while an owned value is moved out of \
                 borrowed storage: store a replacement into the vacant place before this call"
            }
        }
    }
}

/// What running one operation or terminator can reach.
pub(super) enum Reach {
    /// No call and no service instruction.
    Plain,
    /// An ordinary call into this machine of the module.
    Calls(MachineId),
    /// A call to this boundary requirement.
    Boundary(BoundaryMachineId),
    /// A service instruction or a computed call.
    Fenced(CallFence),
}

pub(super) fn operation(kind: &OperationKind) -> Reach {
    reach(kind.external_reach())
}

pub(super) fn terminator(terminator: &Terminator) -> Reach {
    reach(terminator.external_reach().map(|(_, reach)| reach))
}

/// A register access that names no service still touches machine state
/// outside the program's storage.
fn reach(reach: Option<ExternalReach>) -> Reach {
    match reach {
        None => Reach::Plain,
        Some(ExternalReach::Machine(callee)) => Reach::Calls(callee),
        Some(ExternalReach::Boundary(boundary)) => Reach::Boundary(boundary),
        Some(ExternalReach::InstructionService(_)) => Reach::Fenced(CallFence::ServiceReach),
        Some(ExternalReach::ComputedCall) => Reach::Fenced(CallFence::UnknownAccess),
    }
}

/// The fences of one module's machines and boundary requirements. The tables
/// are built on the first call made inside an open window, then shared by
/// every walk over that module.
pub(crate) struct CallFences<'m> {
    module: &'m TerminalModule,
    tables: OnceLock<Tables>,
}

struct Tables {
    /// By machine slot; `None` is a machine that may run in a window.
    machines: Vec<Option<CallFence>>,
    /// Sorted by requirement.
    boundaries: Vec<(BoundaryMachineId, Option<CallFence>)>,
}

impl<'m> CallFences<'m> {
    pub(crate) const fn new(module: &'m TerminalModule) -> Self {
        Self {
            module,
            tables: OnceLock::new(),
        }
    }

    pub(super) const fn module(&self) -> &'m TerminalModule {
        self.module
    }

    fn tables(&self) -> &Tables {
        self.tables.get_or_init(|| {
            let boundaries = boundary_fences(self.module);
            let machines = machine_fences(&self.module.machines, &boundaries);
            Tables {
                machines,
                boundaries,
            }
        })
    }

    /// What fences a call to `machine`. A machine outside the module's dense
    /// catalog has unknown access.
    pub(super) fn machine(&self, machine: MachineId) -> Option<CallFence> {
        usize::try_from(machine.get())
            .ok()
            .and_then(|raw| raw.checked_sub(1))
            .and_then(|slot| self.tables().machines.get(slot))
            .copied()
            .unwrap_or(Some(CallFence::UnknownAccess))
    }

    /// What fences a call to `boundary`.
    pub(super) fn boundary(&self, boundary: BoundaryMachineId) -> Option<CallFence> {
        boundary_fence(&self.tables().boundaries, boundary)
    }
}

fn boundary_fence(
    boundaries: &[(BoundaryMachineId, Option<CallFence>)],
    boundary: BoundaryMachineId,
) -> Option<CallFence> {
    boundaries
        .binary_search_by_key(&boundary, |(id, _)| *id)
        .map_or(Some(CallFence::UnknownAccess), |position| {
            boundaries[position].1
        })
}

/// Every boundary requirement of the module, judged once. A requirement
/// declared twice has no single contract.
fn boundary_fences(module: &TerminalModule) -> Vec<(BoundaryMachineId, Option<CallFence>)> {
    let declarations = &module.interfaces.boundary_machines;
    let mut fences = declarations
        .iter()
        .map(|declaration| (declaration.id, requirement(module, declaration)))
        .collect::<Vec<_>>();
    fences.sort_unstable_by_key(|(id, _)| *id);
    for pair in 1..fences.len() {
        if fences[pair - 1].0 == fences[pair].0 {
            fences[pair - 1].1 = Some(CallFence::UnknownAccess);
            fences[pair].1 = Some(CallFence::UnknownAccess);
        }
    }
    fences.dedup_by_key(|(id, _)| *id);
    fences
}

/// The requirement's operational contract, read alone so that the answer
/// holds for every implementation it admits. Its envelope keeps every
/// written `invokes` row, including one that names a callable parameter,
/// which the published entries leave out. "`reaches none` alone bounds
/// service reach and leaves those access questions open": an empty reach is
/// necessary, and the callback row and the call's arguments settle the rest.
/// `blocks` fences nothing.
fn requirement(
    module: &TerminalModule,
    declaration: &BoundaryMachineDeclaration,
) -> Option<CallFence> {
    let (Some(checked), Some(envelope)) = (
        &declaration.declaration,
        module.execution.callables.boundary_envelope(declaration.id),
    ) else {
        return Some(CallFence::UnknownAccess);
    };
    if !envelope.invocations.is_empty() || !declaration.published_synchronous_entries.is_empty() {
        return Some(CallFence::Callback);
    }
    if !envelope.service_reach.is_empty()
        || !declaration.fixed_service_reach.is_empty()
        || !declaration.published_service_ceiling.is_empty()
    {
        return Some(CallFence::ServiceReach);
    }
    if envelope.suspends || checked.source.published_may_suspend {
        return Some(CallFence::Suspension);
    }
    None
}

fn slot(machines: &[TerminalMachine], machine: MachineId) -> Option<usize> {
    usize::try_from(machine.get())
        .ok()
        .and_then(|raw| raw.checked_sub(1))
        .filter(|slot| machines.get(*slot).is_some_and(|row| row.id == machine))
}

/// A machine may run in a window when its own contract and body reach
/// nothing fenced and every machine it calls, including owned cleanup, may
/// too. A fence spreads from each fenced machine to its callers over one
/// sorted edge list; each caller keeps the first fence that reaches it.
fn machine_fences(
    machines: &[TerminalMachine],
    boundaries: &[(BoundaryMachineId, Option<CallFence>)],
) -> Vec<Option<CallFence>> {
    let mut fences = vec![None; machines.len()];
    // (callee slot, caller slot)
    let mut calls = Vec::new();
    let mut pending = Vec::new();
    for (caller, machine) in machines.iter().enumerate() {
        let contract = &machine.contract;
        let mut fence = if machine.blocks.is_empty() || slot(machines, machine.id) != Some(caller) {
            Some(CallFence::UnknownAccess)
        } else if contract
            .declared_synchronous_entries
            .as_ref()
            .is_some_and(|entries| !entries.is_empty())
            || !contract.published_synchronous_entries.is_empty()
        {
            Some(CallFence::Callback)
        } else if contract
            .declared_service_reach
            .as_ref()
            .is_some_and(|reach| !reach.is_empty())
            || !contract.published_service_ceiling.is_empty()
        {
            Some(CallFence::ServiceReach)
        } else {
            None
        };
        let mut call =
            |callee: MachineId, fence: &mut Option<CallFence>| match slot(machines, callee) {
                Some(callee) => calls.push((callee, caller)),
                None => {
                    fence.get_or_insert(CallFence::UnknownAccess);
                }
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
                    Reach::Calls(callee) => call(callee, &mut fence),
                    Reach::Boundary(boundary) => {
                        if let Some(found) = boundary_fence(boundaries, boundary) {
                            fence.get_or_insert(found);
                        }
                    }
                    Reach::Fenced(found) => {
                        fence.get_or_insert(found);
                    }
                }
            }
            for (_, actions) in block.terminator.cleanup_rosters() {
                for action in actions {
                    if let TerminalAffineCleanupAction::InvokeOwned(cleanup) = action {
                        call(cleanup.cleanup_machine, &mut fence);
                    }
                }
            }
        }
        if fence.is_some() {
            fences[caller] = fence;
            pending.push(caller);
        }
    }
    calls.sort_unstable();
    calls.dedup();
    while let Some(callee) = pending.pop() {
        let fence = fences[callee];
        let first = calls.partition_point(|(row, _)| *row < callee);
        for &(_, caller) in calls[first..].iter().take_while(|(row, _)| *row == callee) {
            if fences[caller].is_none() {
                fences[caller] = fence;
                pending.push(caller);
            }
        }
    }
    fences
}
