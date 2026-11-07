//! Synchronous executable closure of a canonical Terminal callable.
//!
//! Stop at unresolved provider boundaries. A receiving owner may supply exact
//! admitted boundary/body relations without changing the canonical module.
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
    boundary_calls: Vec<(BoundaryMachineId, MachineId)>,
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
            boundary_calls: Vec::new(),
        })
    }

    /// Add receiver-resolved calls without choosing or admitting a provider.
    /// The receiver owns the policy and its artifact/target joins. This neutral
    /// graph checks coordinates and keeps each boundary as a reached demand.
    pub fn with_boundary_calls(
        mut self,
        calls: impl IntoIterator<Item = (BoundaryMachineId, MachineId)>,
    ) -> Result<Self, String> {
        self.boundary_calls.extend(calls);
        self.boundary_calls
            .sort_unstable_by_key(|(boundary, _)| *boundary);
        for &(boundary, machine) in &self.boundary_calls {
            if self.module.boundary_machine(boundary).is_none()
                || !self.machines.contains_key(&machine)
            {
                return Err("synchronous boundary call names an absent boundary or body".into());
            }
        }
        if self
            .boundary_calls
            .windows(2)
            .any(|rows| rows[0].0 == rows[1].0)
        {
            return Err("synchronous boundary call repeats a resolved boundary".into());
        }
        Ok(self)
    }

    /// The exact called body at this boundary, or an opaque requirement.
    /// Per-invocation consumers use the same resolution as closure traversal.
    pub fn boundary_callee(
        &self,
        boundary: BoundaryMachineId,
    ) -> Result<Option<MachineId>, String> {
        self.boundary_call(boundary).map(|(machine, _)| machine)
    }

    fn boundary_call(
        &self,
        boundary: BoundaryMachineId,
    ) -> Result<(Option<MachineId>, bool), String> {
        if self.module.boundary_machine(boundary).is_none() {
            return Err(format!(
                "synchronous call names absent boundary {boundary:?}"
            ));
        }
        if let Some((_, entry)) = self.entry_bridge.filter(|(head, _)| *head == boundary) {
            return Ok((Some(entry), true));
        }
        match crate::admitted_boundary_member(self.module, self.target_members, boundary)? {
            Some(TargetMachineCallable::Machine(member)) => Ok((Some(member), false)),
            Some(TargetMachineCallable::Boundary(_)) | None => {
                let callee = self
                    .boundary_calls
                    .binary_search_by_key(&boundary, |(boundary, _)| *boundary)
                    .ok()
                    .map(|index| self.boundary_calls[index].1);
                Ok((callee, true))
            }
        }
    }

    pub fn from_root(&self, root: MachineId) -> Result<SynchronousCallClosure<'module>, String> {
        let mut pending = vec![TargetMachineCallable::Machine(root)];
        let mut visited = BTreeSet::new();
        let mut machines = Vec::new();
        let mut boundaries = BTreeSet::new();
        while let Some(callable) = pending.pop() {
            if !visited.insert(callable) {
                continue;
            }
            if let TargetMachineCallable::Boundary(boundary) = callable {
                let (callee, retain_boundary) = self.boundary_call(boundary)?;
                if retain_boundary {
                    boundaries.insert(boundary);
                }
                if let Some(callee) = callee {
                    pending.push(TargetMachineCallable::Machine(callee));
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
                            pending.extend(
                                self.dynamic_callees(
                                    *interface,
                                    *slot,
                                    !family_actuals.is_empty(),
                                )?
                                .into_iter()
                                .map(TargetMachineCallable::Machine),
                            );
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

    /// All callable rows of this exact dynamic slot/family, in catalog order.
    /// Address roots that are not invoked are deliberately absent.
    pub fn dynamic_callees(
        &self,
        interface_id: u32,
        slot: u32,
        finite_family: bool,
    ) -> Result<Vec<MachineId>, String> {
        let mut callees = Vec::new();
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
                    callees.push(callee);
                }
            }
        }
        Ok(callees)
    }
}
