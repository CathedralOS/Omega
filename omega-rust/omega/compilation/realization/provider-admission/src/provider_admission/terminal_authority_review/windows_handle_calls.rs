//! Exact executable callers for native handle argument substitution.
//!
//! Bodies come from the existing selected-family and installed-provider custody
//! joins. Runtime inputs remain borrowed from canonical invocations. Recursive
//! machine edges stay finite graph edges; they are not expanded into invented
//! activations or silently treated as a pure call.

use std::collections::BTreeMap;

use effects::TerminalMechanismIdentity;
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, EdgeId, IdTable, InvocationSite, MachineId, ValueId,
};
use terminal_psi::control_flow::BoundaryInvocationCompletion;
use terminal_psi::{
    ModuleInvocationInputs, OperationKind, StructuralArgument, TargetMachineCallable,
    TerminalAffineCleanupAction, TerminalMachine,
};

use super::context::ReviewContext;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum CallSite {
    Invocation(InvocationSite),
    Cleanup { edge: EdgeId, ordinal: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Caller {
    pub machine: MachineId,
    pub block: BlockId,
    pub operation_index: usize,
    pub site: CallSite,
}

#[derive(Clone, Copy)]
pub(super) enum RuntimeArguments<'a> {
    Explicit {
        scalar: &'a [ValueId],
        structural: &'a [StructuralArgument],
    },
    Cleanup(&'a terminal_psi::NominalAffineCleanup),
}

impl<'a> From<ModuleInvocationInputs<'a>> for RuntimeArguments<'a> {
    fn from(inputs: ModuleInvocationInputs<'a>) -> Self {
        Self::Explicit {
            scalar: inputs.arguments,
            structural: inputs.structural_arguments,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum CallTarget {
    Machine(MachineId),
    Boundary(BoundaryMachineId),
    /// The exact selected closed compiler mechanism, not a source effect row.
    Intrinsic(effects::CompilerIntrinsicExecutionIdentity),
}

#[derive(Clone, Copy)]
pub(super) struct RuntimeCall<'a> {
    pub caller: Caller,
    /// Original boundary coordinate, retained when its selected execution is a
    /// module body or intrinsic. Terminator invocations have the same custody.
    pub boundary: Option<BoundaryMachineId>,
    pub target: CallTarget,
    pub arguments: RuntimeArguments<'a>,
    pub normal_return: bool,
}

#[derive(Clone, Copy)]
pub(super) struct RuntimeStorageActual<'a> {
    pub place: semantic_vocabulary::PlaceId,
    pub path: &'a [terminal_psi::StructuralPathSegment],
    pub access: terminal_psi::StructuralAccess,
}

impl<'a> RuntimeCall<'a> {
    /// Reuse the admitted call's own positional arguments. A nominal cleanup
    /// has only its canonically checked receiver, never an invented ordinal
    /// or a second expression which could select different storage.
    pub(super) fn storage_actual(
        self,
        callee: &TerminalMachine,
        parameter: usize,
    ) -> Result<RuntimeStorageActual<'a>, String> {
        if !matches!(self.target, CallTarget::Machine(actual) if actual == callee.id) {
            return Err("handle storage actual belongs to another selected callee".into());
        }
        let formal = callee
            .structural_parameters
            .get(parameter)
            .ok_or("handle storage actual has no original callee formal")?;
        match self.arguments {
            RuntimeArguments::Explicit { structural, .. } => {
                let actual = structural
                    .get(parameter)
                    .ok_or("handle storage formal omits its actual argument")?;
                if structural.len() != callee.structural_parameters.len()
                    || actual.access != formal.access
                {
                    return Err("handle storage call changes its formal count or access".into());
                }
                Ok(RuntimeStorageActual {
                    place: actual.place,
                    path: &actual.path,
                    access: actual.access,
                })
            }
            RuntimeArguments::Cleanup(cleanup) => {
                if !formal.is_cleanup_receiver(cleanup) || callee.structural_parameters.len() != 1 {
                    return Err("handle storage call changes its nominal cleanup receiver".into());
                }
                Ok(RuntimeStorageActual {
                    place: cleanup.place,
                    path: &cleanup.path,
                    access: formal.access,
                })
            }
        }
    }
}

pub(super) struct HandleCallGraph<'a> {
    pub entry: MachineId,
    machines: IdTable<MachineId, Option<&'a TerminalMachine>>,
    pub calls: BTreeMap<Caller, Vec<RuntimeCall<'a>>>,
    pub callers: IdTable<MachineId, Vec<RuntimeCall<'a>>>,
    callees: IdTable<MachineId, Vec<MachineId>>,
}

impl<'a> HandleCallGraph<'a> {
    pub(super) fn new(context: &'a ReviewContext<'_>) -> Result<Self, String> {
        let entry = context.input.plan().entry;
        let mut graph = Self {
            entry,
            machines: IdTable::default(),
            calls: BTreeMap::new(),
            callers: IdTable::default(),
            callees: IdTable::default(),
        };
        let mut pending = vec![entry];
        let mut machine_namespace = context
            .input
            .module()
            .machines
            .iter()
            .map(|machine| (machine.id, Some(false)))
            .collect::<Vec<_>>();
        machine_namespace.sort_by_key(|(identity, _)| *identity);
        let mut reached = IdTable::from_rows_with_gaps(machine_namespace, 65_536)
            .map_err(|reason| format!("canonical machine namespace is invalid: {reason:?}"))?;
        let mut machines = Vec::new();
        while let Some(identity) = pending.pop() {
            let machine = context.graph.machine(identity).ok_or_else(|| {
                format!("native handle caller {identity:?} has no canonical body")
            })?;
            let visited = reached
                .get_mut(identity)
                .and_then(Option::as_mut)
                .ok_or("native handle caller is absent from its canonical namespace")?;
            if *visited {
                continue;
            }
            *visited = true;
            machines.push((identity, Some(machine)));
            for block in &machine.blocks {
                let mut operation_positions = block
                    .operations
                    .iter()
                    .enumerate()
                    .map(|(index, operation)| (operation.id, Some(index)))
                    .collect::<Vec<_>>();
                operation_positions.sort_by_key(|(id, _)| *id);
                let operation_positions = IdTable::from_rows_with_gaps(operation_positions, 65_536)
                    .map_err(|reason| {
                        format!(
                            "native handle block has an invalid operation inventory: {reason:?}"
                        )
                    })?;
                let position = |site: InvocationSite| match site.operation() {
                    Some(operation) => operation_positions
                        .get(operation)
                        .copied()
                        .flatten()
                        .ok_or_else(|| {
                            format!("native handle invocation {site:?} lost its operation position")
                        }),
                    None => Ok(block.operations.len()),
                };
                for invocation in block.module_invocations() {
                    let caller = Caller {
                        machine: identity,
                        block: block.id,
                        operation_index: position(invocation.site)?,
                        site: CallSite::Invocation(invocation.site),
                    };
                    graph.insert(
                        RuntimeCall {
                            caller,
                            boundary: None,
                            target: CallTarget::Machine(invocation.inputs.callee),
                            arguments: invocation.inputs.into(),
                            normal_return: !matches!(
                                invocation.site,
                                InvocationSite::InvokeNoNormal(_)
                            ),
                        },
                        &mut pending,
                    );
                }
                for invocation in block.boundary_invocations() {
                    let caller = Caller {
                        machine: identity,
                        block: invocation.block,
                        operation_index: position(invocation.site)?,
                        site: CallSite::Invocation(invocation.site),
                    };
                    graph.insert(
                        RuntimeCall {
                            caller,
                            boundary: Some(invocation.boundary),
                            target: boundary_target(context, invocation.boundary)?,
                            arguments: RuntimeArguments::Explicit {
                                scalar: invocation.arguments,
                                structural: invocation.structural_arguments,
                            },
                            normal_return: matches!(
                                invocation.completion,
                                BoundaryInvocationCompletion::Normal { .. }
                            ),
                        },
                        &mut pending,
                    );
                }
                for (index, operation) in block.operations.iter().enumerate() {
                    let OperationKind::CallDynamic {
                        interface,
                        slot,
                        arguments,
                        structural_arguments,
                        family_actuals,
                        ..
                    } = &operation.kind
                    else {
                        continue;
                    };
                    // Use the same complete finite target roster as provider
                    // reachability; an unknown selector cannot choose a subset.
                    for callee in context.graph.dynamic_callees(
                        *interface,
                        *slot,
                        !family_actuals.is_empty(),
                    )? {
                        graph.insert(
                            RuntimeCall {
                                caller: Caller {
                                    machine: identity,
                                    block: block.id,
                                    operation_index: index,
                                    site: CallSite::Invocation(InvocationSite::Operation(
                                        operation.id,
                                    )),
                                },
                                boundary: None,
                                target: CallTarget::Machine(callee),
                                arguments: RuntimeArguments::Explicit {
                                    scalar: arguments,
                                    structural: structural_arguments,
                                },
                                normal_return: true,
                            },
                            &mut pending,
                        );
                    }
                }
                for (edge, actions) in block.terminator.cleanup_rosters() {
                    for (ordinal, action) in actions.iter().enumerate() {
                        if let TerminalAffineCleanupAction::InvokeNominal(cleanup) = action {
                            terminal_semantics::nominal_cleanup::target(
                                context.input.module().structural_types.iter(),
                                context.input.module().machines.iter(),
                                cleanup,
                            ).map_err(|reason| format!("native handle cleanup call has no canonical receiver contract: {reason}"))?;
                            graph.insert(
                                RuntimeCall {
                                    caller: Caller {
                                        machine: identity,
                                        block: block.id,
                                        operation_index: block.operations.len(),
                                        site: CallSite::Cleanup { edge, ordinal },
                                    },
                                    boundary: None,
                                    target: CallTarget::Machine(cleanup.cleanup_machine),
                                    arguments: RuntimeArguments::Cleanup(cleanup),
                                    normal_return: true,
                                },
                                &mut pending,
                            );
                        }
                    }
                }
            }
        }
        machines.sort_by_key(|(identity, _)| *identity);
        graph.machines = IdTable::from_rows_with_gaps(machines, 65_536)
            .map_err(|reason| format!("native handle machine inventory is invalid: {reason:?}"))?;
        graph.callers = graph.machines.map(|_, _| Vec::new());
        graph.callees = graph.machines.map(|_, _| Vec::new());
        for alternatives in graph.calls.values() {
            for call in alternatives {
                if let CallTarget::Machine(callee) = call.target {
                    graph
                        .callers
                        .get_mut(callee)
                        .ok_or("native handle callee has no prepared machine coordinate")?
                        .push(*call);
                    graph
                        .callees
                        .get_mut(call.caller.machine)
                        .ok_or("native handle caller has no prepared machine coordinate")?
                        .push(callee);
                }
            }
        }
        let identities = graph.callees.keys().collect::<Vec<_>>();
        for identity in identities {
            let targets = graph
                .callees
                .get_mut(identity)
                .ok_or("native handle callee table lost its coordinate")?;
            targets.sort_unstable();
            targets.dedup();
        }
        Ok(graph)
    }

    pub(super) fn machine(&self, identity: MachineId) -> Option<&'a TerminalMachine> {
        self.machines.get(identity).copied().flatten()
    }

    pub(super) fn machines(&self) -> impl Iterator<Item = (MachineId, &'a TerminalMachine)> + '_ {
        self.machines
            .iter()
            .filter_map(|(identity, machine)| machine.map(|machine| (identity, machine)))
    }

    fn insert(&mut self, call: RuntimeCall<'a>, pending: &mut Vec<MachineId>) {
        self.calls.entry(call.caller).or_default().push(call);
        if let CallTarget::Machine(machine) = call.target {
            pending.push(machine);
        }
    }

    /// Reachable machine cycles are explicit. A safety summary may establish
    /// a preserved formal invariant across them; it cannot assume that a
    /// recursive caller has no effects merely because expansion revisited it.
    pub(super) fn components(&self) -> Vec<Vec<MachineId>> {
        let mut completed = Vec::new();
        let mut visited = self.machines.map(|_, _| false);
        for (root, _) in self.machines() {
            let mut pending = vec![(root, false)];
            while let Some((machine, returning)) = pending.pop() {
                if returning {
                    completed.push(machine);
                } else if !std::mem::replace(
                    visited
                        .get_mut(machine)
                        .expect("reached machine coordinate"),
                    true,
                ) {
                    pending.push((machine, true));
                    pending.extend(
                        self.callees
                            .get(machine)
                            .into_iter()
                            .flatten()
                            .rev()
                            .map(|callee| (*callee, false)),
                    );
                }
            }
        }
        visited = self.machines.map(|_, _| false);
        let mut components = Vec::new();
        while let Some(root) = completed.pop() {
            if visited.get(root).copied().unwrap_or(false) {
                continue;
            }
            let mut component = Vec::new();
            let mut pending = vec![root];
            while let Some(machine) = pending.pop() {
                if !std::mem::replace(
                    visited
                        .get_mut(machine)
                        .expect("reached machine coordinate"),
                    true,
                ) {
                    component.push(machine);
                    pending.extend(
                        self.callers
                            .get(machine)
                            .into_iter()
                            .flatten()
                            .map(|call| call.caller.machine),
                    );
                }
            }
            component.sort();
            components.push(component);
        }
        components
    }
}

fn boundary_target(
    context: &ReviewContext<'_>,
    boundary: BoundaryMachineId,
) -> Result<CallTarget, String> {
    if context.component_authority(boundary)?.is_some() {
        return Ok(CallTarget::Boundary(boundary));
    }
    if let Some(entry) = context.input.selected_program_entry_call()
        && entry.boundary() == boundary
    {
        return Ok(CallTarget::Machine(entry.entry()));
    }
    if let Some(TargetMachineCallable::Machine(machine)) =
        context.input.target_family_member(boundary)?
    {
        return Ok(CallTarget::Machine(machine));
    }
    if let Some(TerminalMechanismIdentity::CompilerIntrinsic(intrinsic)) =
        context.mechanism(boundary)
    {
        return Ok(CallTarget::Intrinsic(*intrinsic));
    }
    let declaration = context
        .boundaries
        .get(&boundary)
        .ok_or_else(|| format!("native handle call names absent boundary {boundary:?}"))?;
    let requirement = context
        .input
        .plan()
        .identities
        .text(declaration.identity)
        .ok_or("native handle boundary lost its original requirement identity")?;
    if let Some(candidate) = context.checked_adapter(requirement)? {
        Ok(CallTarget::Machine(candidate.candidate))
    } else {
        Ok(CallTarget::Boundary(boundary))
    }
}
