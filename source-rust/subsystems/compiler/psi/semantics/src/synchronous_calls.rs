//! Follow synchronous calls in published Psi, including explicitly supplied boundary implementations.
//! Consumers choose the entry and provider bodies. This reader follows those edges and reports
//! the reached machines, retained boundaries and service requirements.

mod scalar_conditions;
mod scalar_roots;
mod service_reach;
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::sync::OnceLock;

use semantic_vocabulary::{BoundaryMachineId, MachineId};
use terminal_psi::{OperationKind, TargetMachineCallable, TerminalModule};

use crate::callable_values::{
    BoundCallableValues, CallableBoundaryImplementation, CallableValueProgram, CallableValueRows,
    CallableValueView,
};
use crate::module_analyses::{MachineSlots, ModuleAnalyses};

/// The shared analyses a call graph reads, refused with the call graph's
/// own message when the module repeats a machine identity.
pub fn call_analyses(module: &TerminalModule) -> Result<ModuleAnalyses<'_>, String> {
    ModuleAnalyses::new(module).map_err(repeated_machine)
}

fn repeated_machine(error: crate::descriptor_results::DescriptorGraphError) -> String {
    format!(
        "synchronous call catalog repeats machine {:?}",
        error.machine()
    )
}

pub struct SynchronousCallClosure<'module> {
    pub machines: Vec<&'module terminal_psi::TerminalMachine>,
    pub boundaries: BTreeSet<BoundaryMachineId>,
}

pub struct SynchronousCallGraph<'module> {
    module: &'module TerminalModule,
    machine_slots: Cow<'module, MachineSlots>,
    entry_bridge: Option<(BoundaryMachineId, MachineId)>,
    boundary_calls: Vec<(BoundaryMachineId, MachineId)>,
    callable_values: CallableValueSource<'module>,
}

/// Reads shared callable values from a `ModuleAnalyses` bundle without naming
/// the bundle's invariant module lifetime.
trait SharedCallableValues: Sync {
    fn callable_values(
        &self,
    ) -> Result<CallableValueView<'_>, &crate::callable_values::CallableValuePreparationError>;
}

impl SharedCallableValues for ModuleAnalyses<'_> {
    fn callable_values(
        &self,
    ) -> Result<CallableValueView<'_>, &crate::callable_values::CallableValuePreparationError> {
        ModuleAnalyses::callable_values(self)
    }
}

/// A module's callable values, shared by every closure of a call graph.
///
/// A computed call resolves its code value from the closure's entries over
/// the whole module, so the first computed call pays for the program and
/// every later closure of the same graph reuses it. The stored rows carry no
/// module borrow, so callers can still shorten the graph's lifetime.
enum CallableValueSource<'module> {
    /// Built from the module on first use, with this graph's boundary bodies
    /// applied.
    Built(OnceLock<Result<CallableValueRows, String>>),
    /// The shared analyses' program, read through this graph's boundary
    /// bodies once they are checked against it.
    Shared {
        analyses: &'module dyn SharedCallableValues,
        bindings: OnceLock<Result<Vec<CallableBoundaryImplementation>, String>>,
    },
}

impl CallableValueSource<'_> {
    /// The same source before any program was built for this graph's
    /// boundary bodies.
    fn unbound(&self) -> Self {
        match self {
            Self::Built(_) => Self::Built(OnceLock::new()),
            Self::Shared { analyses, .. } => Self::Shared {
                analyses: *analyses,
                bindings: OnceLock::new(),
            },
        }
    }
}

impl<'module> SynchronousCallGraph<'module> {
    pub fn machine(&self, identity: MachineId) -> Option<&'module terminal_psi::TerminalMachine> {
        let module = self.module;
        module.machines.get(self.machine_slots.slot(identity)?)
    }

    pub fn new(
        module: &'module TerminalModule,
        entry_bridge: Option<(BoundaryMachineId, MachineId)>,
    ) -> Result<Self, String> {
        let machine_slots = MachineSlots::new(module).map_err(repeated_machine)?;
        Ok(Self {
            module,
            machine_slots: Cow::Owned(machine_slots),
            entry_bridge,
            boundary_calls: Vec::new(),
            callable_values: CallableValueSource::Built(OnceLock::new()),
        })
    }

    /// A graph over the analyses' module that borrows their machine lookup
    /// and callable values instead of building its own.
    pub(crate) fn shared(analyses: &'module ModuleAnalyses<'_>) -> Self {
        Self {
            module: analyses.module(),
            machine_slots: Cow::Borrowed(analyses.machine_slots()),
            entry_bridge: None,
            boundary_calls: Vec::new(),
            callable_values: CallableValueSource::Shared {
                analyses,
                bindings: OnceLock::new(),
            },
        }
    }

    pub fn with_boundary_calls(
        mut self,
        calls: impl IntoIterator<Item = (BoundaryMachineId, MachineId)>,
    ) -> Result<Self, String> {
        self.boundary_calls.extend(calls);
        self.boundary_calls
            .sort_unstable_by_key(|(boundary, _)| *boundary);
        // A program built for the earlier boundary bodies no longer applies.
        self.callable_values = self.callable_values.unbound();
        for &(boundary, machine) in &self.boundary_calls {
            if self.module.boundary_machine(boundary).is_none() || self.machine(machine).is_none() {
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

    pub fn boundary_callee(
        &self,
        boundary: BoundaryMachineId,
    ) -> Result<Option<MachineId>, String> {
        if self.module.boundary_machine(boundary).is_none() {
            return Err(format!(
                "synchronous call names absent boundary {boundary:?}"
            ));
        }
        if let Some((_, entry)) = self.entry_bridge.filter(|(head, _)| *head == boundary) {
            return Ok(Some(entry));
        }
        let callee = self
            .boundary_calls
            .binary_search_by_key(&boundary, |(boundary, _)| *boundary)
            .ok()
            .map(|index| self.boundary_calls[index].1);
        Ok(callee)
    }

    pub fn from_root(&self, root: MachineId) -> Result<SynchronousCallClosure<'module>, String> {
        self.from_callables([TargetMachineCallable::Machine(root)])
    }

    pub fn from_callables(
        &self,
        roots: impl IntoIterator<Item = TargetMachineCallable>,
    ) -> Result<SynchronousCallClosure<'module>, String> {
        self.visit_callables(roots, &mut BTreeSet::new())
    }

    fn visit_callables(
        &self,
        roots: impl IntoIterator<Item = TargetMachineCallable>,
        visited: &mut BTreeSet<TargetMachineCallable>,
    ) -> Result<SynchronousCallClosure<'module>, String> {
        self.visit_callables_stopping_at(roots, visited, &[])
    }

    fn visit_callables_stopping_at(
        &self,
        roots: impl IntoIterator<Item = TargetMachineCallable>,
        visited: &mut BTreeSet<TargetMachineCallable>,
        installation_requirements: &[BoundaryMachineId],
    ) -> Result<SynchronousCallClosure<'module>, String> {
        let mut pending: Vec<_> = roots.into_iter().collect();
        let roots = pending.clone();
        let mut machines = Vec::new();
        let mut boundaries = BTreeSet::new();
        let mut callable_reach = None;
        while let Some(callable) = pending.pop() {
            if !visited.insert(callable) {
                continue;
            }
            if let TargetMachineCallable::Boundary(boundary) = callable {
                let callee = self.boundary_callee(boundary)?;
                boundaries.insert(boundary);
                if let Some(callee) = callee
                    && installation_requirements.binary_search(&boundary).is_err()
                {
                    pending.push(TargetMachineCallable::Machine(callee));
                }
                continue;
            }
            let TargetMachineCallable::Machine(machine) = callable else {
                continue;
            };
            let body = self
                .machine(machine)
                .ok_or_else(|| format!("synchronous call names absent machine {machine:?}"))?;
            machines.push(body);
            pending.extend(
                body.module_invocations()
                    .map(|call| TargetMachineCallable::Machine(call.inputs.callee)),
            );
            pending.extend(
                body.boundary_invocations()
                    .map(|call| TargetMachineCallable::Boundary(call.boundary)),
            );
            for block in &body.blocks {
                for operation in &block.operations {
                    if matches!(operation.kind, OperationKind::CallMachine { .. }) {
                        // Resolve code values from the original entries, so
                        // a helper's formal keeps its caller's selection.
                        // Starting each helper as a root would lose it.
                        if callable_reach.is_none() {
                            let values = self.callable_values()?;
                            let mut entries = Vec::with_capacity(roots.len());
                            for root in &roots {
                                match *root {
                                    TargetMachineCallable::Machine(machine) => {
                                        entries.push(machine)
                                    }
                                    TargetMachineCallable::Boundary(boundary) => {
                                        if installation_requirements
                                            .binary_search(&boundary)
                                            .is_err()
                                            && let Some(machine) = self.boundary_callee(boundary)?
                                        {
                                            entries.push(machine);
                                        }
                                    }
                                }
                            }
                            callable_reach = Some(
                                crate::callable_values::reachable_callable_invocations(
                                    values, entries,
                                )
                                .map_err(str::to_owned)?,
                            );
                        }
                        let reached = callable_reach
                            .as_ref()
                            .ok_or("synchronous computed call lost its entry flow")?;
                        let site = semantic_vocabulary::CallableInvocationSite::Invocation(
                            semantic_vocabulary::InvocationSite::Operation(operation.id),
                        );
                        if let Some(invocation) = reached.site(machine, site) {
                            if let crate::callable_values::ReachedCallableTarget::Callable(values) =
                                &invocation.target
                            {
                                for ceiling in &values.unresolved_ceilings {
                                    let contract =
                                        self.module.execution.callables.contract(*ceiling).ok_or(
                                            "synchronous computed call has no exact contract",
                                        )?;
                                    if !contract.envelope.invocations.is_empty() {
                                        return Err("synchronous computed call has unresolved advertised arrivals".into());
                                    }
                                }
                            }
                            pending.extend(
                                invocation
                                    .selected_machines
                                    .iter()
                                    .copied()
                                    .map(TargetMachineCallable::Machine),
                            );
                        } else if !reached.reached_machines.contains(&machine) {
                            return Err(
                                "synchronous computed call has no exact incoming actual flow"
                                    .into(),
                            );
                        }
                    }
                    if let OperationKind::CallDynamic {
                        interface,
                        slot,
                        family_actuals,
                        ..
                    } = &operation.kind
                    {
                        pending.extend(
                            self.dynamic_callees(*interface, *slot, !family_actuals.is_empty())?
                                .into_iter()
                                .map(TargetMachineCallable::Machine),
                        );
                    }
                }
            }
        }
        Ok(SynchronousCallClosure {
            machines,
            boundaries,
        })
    }

    /// The module's callable values with this graph's boundary bodies
    /// applied, built on the first request.
    fn callable_values(&self) -> Result<CallableValueView<'_>, String> {
        let implementations = || {
            let mut implementations = Vec::new();
            for boundary in &self.module.interfaces.boundary_machines {
                if let Some(machine) = self.boundary_callee(boundary.id)? {
                    implementations.push(CallableBoundaryImplementation {
                        boundary: boundary.id,
                        machine,
                    });
                }
            }
            Ok::<_, String>(implementations)
        };
        match &self.callable_values {
            CallableValueSource::Built(rows) => rows
                .get_or_init(|| {
                    crate::callable_values::terminal_callable_values_with_boundary_implementations(
                        self.module,
                        &implementations()?,
                    )
                    .map(CallableValueProgram::into_rows)
                })
                .as_ref()
                .map(|rows| rows.view(self.module))
                .map_err(Clone::clone),
            CallableValueSource::Shared { analyses, bindings } => {
                let bindings = bindings
                    .get_or_init(|| {
                        crate::callable_values::bind_declared_implementations(
                            self.module,
                            analyses.callable_values(),
                            &implementations()?,
                        )
                        .map(BoundCallableValues::into_bindings)
                    })
                    .as_ref()
                    .map_err(Clone::clone)?;
                let values = analyses
                    .callable_values()
                    .map_err(|error| error.clone().in_module(self.module))?;
                Ok(values.with_bindings(bindings))
            }
        }
    }

    pub fn dynamic_callees(
        &self,
        interface_id: u32,
        slot: u32,
        finite_family: bool,
    ) -> Result<Vec<MachineId>, String> {
        let mut callees = Vec::new();
        let catalog = &self.module.execution.dynamic_dispatch;
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
#[cfg(test)]
mod tests {
    use super::*;
    use terminal_psi::{
        BoundaryMachineResult, TerminalDynamicInterface, TerminalDynamicSlot, TerminalDynamicTable,
        TerminalIdentityTableBuilder,
    };

    #[test]
    fn closure_refuses_absent_machines_boundaries_and_selected_bodies() {
        let module = TerminalModule::declarations();
        let graph = SynchronousCallGraph::new(&module, None).unwrap();
        let empty = graph.from_callables([]).unwrap();
        assert!(empty.machines.is_empty());
        assert!(empty.boundaries.is_empty());
        let machine = MachineId::new(1).unwrap();
        let boundary = BoundaryMachineId::new(1).unwrap();
        assert!(graph.from_root(machine).is_err());
        assert!(
            graph
                .from_callables([TargetMachineCallable::Boundary(boundary)])
                .is_err()
        );
        assert!(graph.with_boundary_calls([(boundary, machine)]).is_err());
    }

    #[test]
    fn dynamic_call_follows_the_invoked_slot_or_complete_finite_roster_only() {
        let mut identities = TerminalIdentityTableBuilder::default();
        let interface_identity = identities.intern("Shape");
        let first_requirement = identities.intern("Shape::first");
        let unrelated = identities.intern("Shape::other");
        let mut module = TerminalModule::declarations();
        let mut slots = Vec::new();
        for slot in 0..3 {
            slots.push(TerminalDynamicSlot {
                slot,
                declaring_trait_identity: interface_identity,
                requirement_identity: if slot < 2 {
                    first_requirement
                } else {
                    unrelated
                },
                family_tuple: vec![terminal_psi::FiniteScalarValue::Integer(i128::from(slot))],
                scalar_parameters: Vec::new(),
                structural_parameters: Vec::new(),
                entry_claim_paths: Vec::new(),
                result: BoundaryMachineResult::Unit,
                result_claim_paths: Vec::new(),
                result_reference_regions: Vec::new(),
            });
        }
        module
            .execution.dynamic_dispatch
            .interfaces
            .push(TerminalDynamicInterface {
                id: 0,
                identity: interface_identity,
                slots,
            });
        module.execution.dynamic_dispatch.tables.push(TerminalDynamicTable {
            id: 0,
            interface: 0,
            implementation_identity: identities.intern("Selected"),
            rows: (1..=3).map(MachineId::new).collect(),
        });
        module.identities = identities.build();
        let calls = SynchronousCallGraph::new(&module, None).unwrap();
        let pending = calls.dynamic_callees(0, 0, false).unwrap();
        assert_eq!(pending, vec![MachineId::new(1).unwrap()]);
        let pending = calls.dynamic_callees(0, 0, true).unwrap();
        assert_eq!(
            pending,
            vec![MachineId::new(1).unwrap(), MachineId::new(2).unwrap(),]
        );
        assert!(calls.dynamic_callees(0, 3, false).is_err());
        assert!(calls.dynamic_callees(1, 0, false).is_err());
    }
}
