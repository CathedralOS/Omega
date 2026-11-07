//! Exact executable callers for native handle argument substitution.
//!
//! Bodies come from the existing selected-family and installed-provider custody
//! joins. Runtime inputs remain borrowed from canonical invocations. Recursive
//! machine edges stay finite graph edges; they are not expanded into invented
//! activations or silently treated as a pure call.

use std::collections::{BTreeMap, BTreeSet};

use effects::TerminalMechanismIdentity;
use semantic_vocabulary::{BlockId, BoundaryMachineId, EdgeId, InvocationSite, MachineId, ValueId};
use terminal_psi::{
    BoundaryInvocationCompletion, ModuleInvocationInputs, OperationKind, StructuralArgument,
    TargetMachineCallable, TerminalAffineCleanupAction, TerminalMachine,
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
    pub target: CallTarget,
    pub arguments: RuntimeArguments<'a>,
    pub normal_return: bool,
}

pub(super) struct HandleCallGraph<'a> {
    pub entry: MachineId,
    pub machines: BTreeMap<MachineId, &'a TerminalMachine>,
    pub calls: BTreeMap<Caller, Vec<RuntimeCall<'a>>>,
    pub callers: BTreeMap<MachineId, Vec<RuntimeCall<'a>>>,
    callees: BTreeMap<MachineId, BTreeSet<MachineId>>,
}

impl<'a> HandleCallGraph<'a> {
    pub(super) fn new(context: &'a ReviewContext<'_>) -> Result<Self, String> {
        let entry = context.input.plan().entry;
        let mut graph = Self {
            entry,
            machines: BTreeMap::new(),
            calls: BTreeMap::new(),
            callers: BTreeMap::new(),
            callees: BTreeMap::new(),
        };
        let mut pending = vec![entry];
        while let Some(identity) = pending.pop() {
            if graph.machines.contains_key(&identity) {
                continue;
            }
            let machine = context.graph.machine(identity).ok_or_else(|| {
                format!("native handle caller {identity:?} has no canonical body")
            })?;
            graph.machines.insert(identity, machine);
            for block in &machine.blocks {
                let operation_positions = block
                    .operations
                    .iter()
                    .enumerate()
                    .map(|(index, operation)| (operation.id, index))
                    .collect::<BTreeMap<_, _>>();
                let position = |site: InvocationSite| match site.operation() {
                    Some(operation) => {
                        operation_positions.get(&operation).copied().ok_or_else(|| {
                            format!("native handle invocation {site:?} lost its operation position")
                        })
                    }
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
                            graph.insert(
                                RuntimeCall {
                                    caller: Caller {
                                        machine: identity,
                                        block: block.id,
                                        operation_index: block.operations.len(),
                                        site: CallSite::Cleanup { edge, ordinal },
                                    },
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
        Ok(graph)
    }

    fn insert(&mut self, call: RuntimeCall<'a>, pending: &mut Vec<MachineId>) {
        self.calls.entry(call.caller).or_default().push(call);
        if let CallTarget::Machine(machine) = call.target {
            self.callees
                .entry(call.caller.machine)
                .or_default()
                .insert(machine);
            self.callers.entry(machine).or_default().push(call);
            pending.push(machine);
        }
    }

    /// Reachable machine cycles are explicit. A safety summary may establish
    /// a preserved formal invariant across them; it cannot assume that a
    /// recursive caller has no effects merely because expansion revisited it.
    pub(super) fn components(&self) -> Vec<Vec<MachineId>> {
        let mut completed = Vec::new();
        let mut visited = BTreeSet::new();
        for root in self.machines.keys().copied() {
            let mut pending = vec![(root, false)];
            while let Some((machine, returning)) = pending.pop() {
                if returning {
                    completed.push(machine);
                } else if visited.insert(machine) {
                    pending.push((machine, true));
                    pending.extend(
                        self.callees
                            .get(&machine)
                            .into_iter()
                            .flatten()
                            .rev()
                            .map(|callee| (*callee, false)),
                    );
                }
            }
        }
        visited.clear();
        let mut components = Vec::new();
        while let Some(root) = completed.pop() {
            if visited.contains(&root) {
                continue;
            }
            let mut component = Vec::new();
            let mut pending = vec![root];
            while let Some(machine) = pending.pop() {
                if visited.insert(machine) {
                    component.push(machine);
                    pending.extend(
                        self.callers
                            .get(&machine)
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
        context.mechanisms.get(&boundary)
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
