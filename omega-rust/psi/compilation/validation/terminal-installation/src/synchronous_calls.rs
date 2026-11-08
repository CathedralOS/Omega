//! Synchronous executable closure of a canonical Terminal callable.
//!
//! Stop at provider boundaries; installation joins those boundaries separately.
//! Code-retention roots (callbacks, dynamic table addresses, native layout
//! requests) are not calls. Ordinary helpers, invoked dynamic slots, selected
//! target members and cleanup hooks are calls, regardless of result shape.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{BoundaryMachineId, MachineId};
use terminal_psi::{
    OperationKind, TargetMachineCallable, TerminalAffineCleanupAction, TerminalModule, Terminator,
};

use crate::AdmittedTargetMachineSelections;

#[cfg(test)]
mod tests;

pub struct SynchronousCallClosure<'module> {
    pub machines: Vec<&'module terminal_psi::TerminalMachine>,
    pub boundaries: BTreeSet<BoundaryMachineId>,
}

/// Invocation-local index shared by the provider roots a receiver checks.
/// This view inspects admitted semantics; it grants no execution authority.
pub struct SynchronousCallGraph<'module> {
    module: &'module TerminalModule,
    machines: BTreeMap<MachineId, &'module terminal_psi::TerminalMachine>,
    target_members: Option<&'module AdmittedTargetMachineSelections>,
    entry_bridge: Option<(BoundaryMachineId, MachineId)>,
}

impl<'module> SynchronousCallGraph<'module> {
    /// An optional admitted entry bridge retains its boundary demand and
    /// continues into its selected body. Target selection is required only for
    /// reached heads. Preparing once avoids rescanning the machine catalog for
    /// every edge of every installed provider instance.
    pub fn new(
        module: &'module TerminalModule,
        target_members: Option<&'module AdmittedTargetMachineSelections>,
        entry_bridge: Option<(BoundaryMachineId, MachineId)>,
    ) -> Result<Self, String> {
        let mut machines = BTreeMap::new();
        for machine in &module.machines {
            if machines.insert(machine.id, machine).is_some() {
                return Err(format!(
                    "synchronous call catalog repeats machine {:?}",
                    machine.id
                ));
            }
        }
        Ok(Self {
            module,
            machines,
            target_members,
            entry_bridge,
        })
    }

    pub fn from_root(&self, root: MachineId) -> Result<SynchronousCallClosure<'module>, String> {
        let module = self.module;
        let target_members = self.target_members;
        let entry_bridge = self.entry_bridge;
        let mut pending = vec![TargetMachineCallable::Machine(root)];
        let mut visited = BTreeSet::new();
        let mut machines = Vec::new();
        let mut boundaries = BTreeSet::new();
        while let Some(callable) = pending.pop() {
            if !visited.insert(callable) {
                continue;
            }
            if let TargetMachineCallable::Boundary(boundary) = callable {
                if module.boundary_machine(boundary).is_none() {
                    return Err(format!(
                        "synchronous call names absent boundary {boundary:?}"
                    ));
                }
                if let Some((_, entry)) = entry_bridge.filter(|(head, _)| *head == boundary) {
                    boundaries.insert(boundary);
                    pending.push(TargetMachineCallable::Machine(entry));
                    continue;
                }
                match crate::admitted_boundary_member(module, target_members, boundary)? {
                    Some(TargetMachineCallable::Machine(member)) => {
                        pending.push(TargetMachineCallable::Machine(member))
                    }
                    // Opaque members retain the original head as their admitted
                    // requirement. Ordinary and legacy canonical members do too.
                    Some(TargetMachineCallable::Boundary(_)) | None => {
                        boundaries.insert(boundary);
                    }
                }
                continue;
            }
            let TargetMachineCallable::Machine(machine) = callable else {
                continue;
            };
            let body = self
                .machines
                .get(&machine)
                .copied()
                .ok_or_else(|| format!("synchronous call names absent machine {machine:?}"))?;
            machines.push(body);
            pending.extend(
                body.module_invocations()
                    .map(|call| TargetMachineCallable::Machine(call.inputs.callee)),
            );
            for block in &body.blocks {
                for (_, actions) in block.terminator.cleanup_rosters() {
                    for action in actions {
                        if let TerminalAffineCleanupAction::InvokeNominal(cleanup) = action {
                            pending.push(TargetMachineCallable::Machine(cleanup.cleanup_machine));
                        }
                    }
                }
                if let Terminator::ExternalCompletion { boundary, .. } = block.terminator {
                    pending.push(TargetMachineCallable::Boundary(boundary));
                }
                for operation in &block.operations {
                    match &operation.kind {
                        OperationKind::BoundaryCall { boundary, .. } => {
                            pending.push(TargetMachineCallable::Boundary(*boundary))
                        }
                        OperationKind::CallDynamic {
                            interface,
                            slot,
                            family_actuals,
                            ..
                        } => {
                            self.append_dynamic_calls(
                                *interface,
                                *slot,
                                !family_actuals.is_empty(),
                                &mut pending,
                            )?;
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(SynchronousCallClosure {
            machines,
            boundaries,
        })
    }

    fn append_dynamic_calls(
        &self,
        interface_id: u32,
        slot: u32,
        finite_family: bool,
        pending: &mut Vec<TargetMachineCallable>,
    ) -> Result<(), String> {
        let catalog = &self.module.dynamic_dispatch;
        let interface = catalog
            .interface(interface_id)
            .ok_or("synchronous dynamic call has no interface")?;
        let first = interface
            .slots
            .get(slot as usize)
            .filter(|row| row.slot == slot)
            .ok_or("synchronous dynamic call has no exact interface slot")?;
        let slots = interface
            .slots
            .iter()
            .skip(slot as usize)
            .take_while(|row| {
                if !finite_family {
                    row.slot == slot
                } else {
                    !row.family_tuple.is_empty()
                        && row.requirement_identity == first.requirement_identity
                }
            });
        for row in slots {
            for table in &catalog.tables {
                if table.interface != interface.id {
                    continue;
                }
                if let Some(callee) = table.rows.get(row.slot as usize).copied().flatten() {
                    pending.push(TargetMachineCallable::Machine(callee));
                }
            }
        }
        Ok(())
    }
}
