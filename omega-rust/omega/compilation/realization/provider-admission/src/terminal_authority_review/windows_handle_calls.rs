//! Exact executable callers for native handle argument substitution.
//!
//! Bodies come from the existing selected-family and installed-provider custody
//! joins. Runtime inputs remain borrowed from canonical invocations. Recursive
//! machine edges stay finite graph edges; they are not expanded into invented
//! activations or silently treated as a pure call.

use std::collections::BTreeMap;

use effects::TerminalMechanismIdentity;
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, IdTable, InvocationSite, MachineId, ValueId,
};
use terminal_psi::control_flow::BoundaryInvocationCompletion;
use terminal_psi::{
    ModuleInvocationInputs, OperationKind, StructuralArgument, TerminalAffineCleanupAction,
    TerminalMachine,
};

use super::context::ReviewContext;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Caller {
    pub machine: MachineId,
    pub block: BlockId,
    pub operation_index: usize,
    pub site: InvocationSite,
}

#[derive(Clone, Copy)]
pub(super) struct RuntimeArguments<'a> {
    pub scalar: &'a [ValueId],
    pub structural: &'a [StructuralArgument],
}

impl<'a> From<ModuleInvocationInputs<'a>> for RuntimeArguments<'a> {
    fn from(inputs: ModuleInvocationInputs<'a>) -> Self {
        Self {
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
    /// Actual operation result; cleanup and nonnormal edges have no such lane.
    pub receiving: Option<&'a terminal_psi::OperationResult>,
}

#[derive(Clone, Copy)]
pub(super) struct RuntimeStorageActual<'a> {
    pub place: semantic_vocabulary::PlaceId,
    pub path: &'a [terminal_psi::StructuralPathSegment],
    pub access: terminal_psi::StructuralAccess,
}

impl<'a> RuntimeCall<'a> {
    /// Reuse the admitted call's original positional arguments, including the
    /// owned subject slice of a cleanup invocation.
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
        let actual = self
            .arguments
            .structural
            .get(parameter)
            .ok_or("handle storage formal omits its actual argument")?;
        if self.arguments.structural.len() != callee.structural_parameters.len()
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
}

pub(super) struct HandleCallGraph<'a> {
    pub entry: MachineId,
    machines: IdTable<MachineId, Option<&'a TerminalMachine>>,
    pub calls: BTreeMap<Caller, Vec<RuntimeCall<'a>>>,
    pub callers: IdTable<MachineId, Vec<RuntimeCall<'a>>>,
}

impl<'a> HandleCallGraph<'a> {
    pub(super) fn new(context: &'a ReviewContext<'_>) -> Result<Self, String> {
        let entry = context.input.plan().entry;
        let mut graph = Self {
            entry,
            machines: IdTable::default(),
            calls: BTreeMap::new(),
            callers: IdTable::default(),
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
                graph.insert_module_invocations(identity, block, position, &mut pending)?;
                for invocation in block.boundary_invocations() {
                    let caller = Caller {
                        machine: identity,
                        block: invocation.block,
                        operation_index: position(invocation.site)?,
                        site: invocation.site,
                    };
                    graph.insert(
                        RuntimeCall {
                            caller,
                            boundary: Some(invocation.boundary),
                            target: boundary_target(context, invocation.boundary)?,
                            arguments: RuntimeArguments {
                                scalar: invocation.arguments,
                                structural: invocation.structural_arguments,
                            },
                            normal_return: matches!(
                                invocation.completion,
                                BoundaryInvocationCompletion::Normal { .. }
                            ),
                            receiving: match invocation.completion {
                                BoundaryInvocationCompletion::Normal { result, .. } => Some(result),
                                BoundaryInvocationCompletion::External(_) => None,
                            },
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
                                    site: InvocationSite::Operation(operation.id),
                                },
                                boundary: None,
                                target: CallTarget::Machine(callee),
                                arguments: RuntimeArguments {
                                    scalar: arguments,
                                    structural: structural_arguments,
                                },
                                normal_return: true,
                                receiving: Some(&operation.result),
                            },
                            &mut pending,
                        );
                    }
                }
                // Cleanup invocations already appear in module_invocations with
                // their real edge/action owner and ordinary owned arguments.
                // Validate their target contract without staging a second call.
                for (_, actions) in block.terminator.cleanup_rosters() {
                    for action in actions {
                        if let TerminalAffineCleanupAction::InvokeOwned(cleanup) = action {
                            terminal_semantics::nominal_cleanup::owned_target(
                                context.input.module().structural_types.iter(),
                                context.input.module().machines.iter(),
                                cleanup,
                            ).map_err(|reason| format!("native handle cleanup call has no canonical owned contract: {reason}"))?;
                        }
                    }
                }
            }
        }
        machines.sort_by_key(|(identity, _)| *identity);
        graph.machines = IdTable::from_rows_with_gaps(machines, 65_536)
            .map_err(|reason| format!("native handle machine inventory is invalid: {reason:?}"))?;
        graph.callers = graph.machines.map(|_, _| Vec::new());
        for alternatives in graph.calls.values() {
            for call in alternatives {
                if let CallTarget::Machine(callee) = call.target {
                    graph
                        .callers
                        .get_mut(callee)
                        .ok_or("native handle callee has no prepared machine coordinate")?
                        .push(*call);
                }
            }
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

    fn insert_module_invocations(
        &mut self,
        identity: MachineId,
        block: &'a terminal_psi::Block,
        position: impl Fn(InvocationSite) -> Result<usize, String>,
        pending: &mut Vec<MachineId>,
    ) -> Result<(), String> {
        for invocation in block.module_invocations() {
            let caller = Caller {
                machine: identity,
                block: block.id,
                operation_index: position(invocation.site)?,
                site: invocation.site,
            };
            self.insert(
                RuntimeCall {
                    caller,
                    boundary: None,
                    target: CallTarget::Machine(invocation.inputs.callee),
                    arguments: invocation.inputs.into(),
                    normal_return: !matches!(invocation.site, InvocationSite::InvokeNoNormal(_)),
                    receiving: invocation
                        .site
                        .operation()
                        .map(|_| &block.operations[caller.operation_index].result),
                },
                pending,
            );
        }
        Ok(())
    }

    fn insert(&mut self, call: RuntimeCall<'a>, pending: &mut Vec<MachineId>) {
        self.calls.entry(call.caller).or_default().push(call);
        if let CallTarget::Machine(machine) = call.target {
            pending.push(machine);
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{EdgeId, PlaceId, StructuralTypeId};
    use terminal_psi::{Block, OwnedCleanupInvocation, StructuralAccess, Terminator};

    #[test]
    fn owned_cleanup_has_one_call_per_original_edge_and_action() {
        let machine = MachineId::new(1).unwrap();
        let callee = MachineId::new(2).unwrap();
        let place = PlaceId::new(1).unwrap();
        let action = || {
            TerminalAffineCleanupAction::InvokeOwned(OwnedCleanupInvocation {
                subject: StructuralArgument {
                    place,
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                },
                structural_type: StructuralTypeId::new(1).unwrap(),
                cleanup_machine: callee,
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
            })
        };
        let blocks = [1, 2].map(|index| {
            Block::plain(
                BlockId::new(index).unwrap(),
                Vec::new(),
                Terminator::ReturnUnit {
                    edge: EdgeId::new(index).unwrap(),
                    cleanup_actions: vec![
                        TerminalAffineCleanupAction::RetireHome(place),
                        action(),
                        action(),
                    ],
                },
            )
        });
        let mut graph = HandleCallGraph {
            entry: machine,
            machines: IdTable::default(),
            calls: BTreeMap::new(),
            callers: IdTable::default(),
        };
        let mut pending = Vec::new();
        for block in &blocks {
            graph
                .insert_module_invocations(machine, block, |_| Ok(0), &mut pending)
                .unwrap();
        }
        assert_eq!(graph.calls.len(), 4);
        for (caller, alternatives) in &graph.calls {
            assert_eq!(alternatives.len(), 1);
            let call = alternatives[0];
            let InvocationSite::Cleanup {
                edge,
                action_ordinal,
            } = caller.site
            else {
                panic!("cleanup lost its action owner")
            };
            assert_eq!(edge.get(), caller.block.get());
            assert!(matches!(action_ordinal, 1 | 2));
            assert_eq!(caller.operation_index, 0);
            assert!(call.arguments.scalar.is_empty());
            assert_eq!(call.arguments.structural.len(), 1);
            assert_eq!(call.arguments.structural[0].access, StructuralAccess::Owned);
            let block = &blocks[caller.block.get() as usize - 1];
            let (_, actions) = block.terminator.cleanup_rosters().next().unwrap();
            let TerminalAffineCleanupAction::InvokeOwned(original) =
                &actions[action_ordinal as usize]
            else {
                panic!("call substitutes a disposal action")
            };
            assert!(std::ptr::eq(
                call.arguments.structural,
                std::slice::from_ref(&original.subject)
            ));
            assert!(call.normal_return);
            assert!(call.receiving.is_none());
        }
    }
}
