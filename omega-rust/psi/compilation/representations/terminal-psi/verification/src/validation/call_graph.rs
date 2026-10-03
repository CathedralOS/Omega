//! The module's call graph over static, dynamic-realization and
//! nominal-cleanup and boundary-candidate callees must be acyclic: a recursive call slice is
//! rejected.

use super::{
    BTreeMap, BTreeSet, MachineId, ModuleError, OperationKind, TerminalMachine, TerminalModule,
};

/// Current executable machine dependencies, including calls owned by normal
/// cleanup edges. Proof/evidence retention does not create an execution edge.
/// Opaque boundary candidates are a separate universal dispatch obligation.
/// Module validation authenticates the complete dynamic-dispatch catalog before
/// invoking machine validation (including cleanup closure), this graph, or root
/// reach. Thus every dynamic operation below already has exactly one target.
pub(super) fn executable_callees(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> BTreeSet<MachineId> {
    let mut callees = BTreeSet::new();
    for block in &machine.blocks {
        callees.extend(
            super::affine_cleanup::nominal_cleanups(&block.terminator)
                .map(|cleanup| cleanup.cleanup_machine),
        );
        for operation in &block.operations {
            match &operation.kind {
                OperationKind::Call { callee, .. }
                | OperationKind::CallUnit { callee, .. }
                | OperationKind::CallStructuralScalar { callee, .. }
                | OperationKind::CallStructural { callee, .. }
                | OperationKind::CallStructuralWithScalarArguments { callee, .. } => {
                    callees.insert(*callee);
                }
                OperationKind::CallDynamicScalar { .. } | OperationKind::CallDynamicUnit { .. } => {
                    let realization = super::dynamic_dispatch::dynamic_call_realization(
                        module,
                        machine.id,
                        operation.id,
                    )
                    .expect("validated dynamic call has one dispatch row");
                    callees.insert(realization);
                }
                _ => {}
            }
        }
    }
    callees
}

/// Reconstruct the completion premise from executable calls. An opaque boundary
/// contributes its declared contract, never its selected provider or service
/// spelling. Installation separately authenticates that premise. This query
/// runs after dynamic dispatch validation, as `executable_callees` requires.
pub(super) fn has_only_normal_completion(module: &TerminalModule, root: MachineId) -> bool {
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !visited.insert(id) {
            continue;
        }
        let Some(machine) = module.machines.iter().find(|machine| machine.id == id) else {
            return false;
        };
        for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
            if let OperationKind::BoundaryCall { boundary, .. } = operation.kind
                && !module.boundary_machines.iter().any(|declaration| {
                    declaration.id == boundary
                        && declaration.completion == terminal_psi::BoundaryCompletion::NormalReturn
                })
            {
                return false;
            }
        }
        pending.extend(executable_callees(module, machine));
    }
    true
}

/// A cleanup must return in finite work. An unranked control-flow cycle is
/// legal ordinary computation, but cannot establish this obligation. Ranked
/// components undergo the ordinary exact-coverage and proof checks; opaque
/// calls contribute only an authenticated unconditional progress premise.
pub(super) fn has_finite_normal_completion(module: &TerminalModule, root: MachineId) -> bool {
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !visited.insert(id) {
            continue;
        }
        let Some(machine) = module.machines.iter().find(|machine| machine.id == id) else {
            return false;
        };
        if !crate::control_graph::cyclic_components(machine).is_empty()
            && machine.ranked_scc.is_none()
        {
            return false;
        }
        for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
            if let OperationKind::BoundaryCall { boundary, .. } = operation.kind
                && !module.boundary_machines.iter().any(|declaration| {
                    declaration.id == boundary
                        && declaration.completion == terminal_psi::BoundaryCompletion::NormalReturn
                        && declaration.unconditional_termination
                })
            {
                return false;
            }
        }
        pending.extend(executable_callees(module, machine));
    }
    true
}

pub(super) fn validate_call_graph(module: &TerminalModule) -> Result<(), ModuleError> {
    // A checked provider body must preserve the opaque completion premise.
    // External bodies need the future terminal-invocation representation; an
    // ordinary returning Unit body cannot prove that outcome today.
    for candidate in &module.provider_candidates {
        if !module.boundary_machines.iter().any(|boundary| {
            boundary.id == candidate.boundary
                && boundary.completion == terminal_psi::BoundaryCompletion::NormalReturn
        }) || !has_only_normal_completion(module, candidate.candidate)
            || module.boundary_machines.iter().any(|boundary| {
                boundary.id == candidate.boundary && boundary.unconditional_termination
            }) && !has_finite_normal_completion(module, candidate.candidate)
        {
            return Err(ModuleError::InvalidProviderCandidate {
                boundary: candidate.boundary,
                candidate: candidate.candidate,
            });
        }
    }
    let calls = module
        .machines
        .iter()
        .map(|machine| {
            let mut callees = executable_callees(module, machine);
            for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
                if let OperationKind::BoundaryCall { boundary, .. } = &operation.kind {
                    callees.extend(
                        module
                            .provider_candidates
                            .iter()
                            .filter(|candidate| candidate.boundary == *boundary)
                            .map(|candidate| candidate.candidate),
                    );
                }
            }
            (machine.id, callees)
        })
        .collect::<BTreeMap<_, _>>();

    let mut indegree = calls
        .keys()
        .copied()
        .map(|machine| (machine, 0_usize))
        .collect::<BTreeMap<_, _>>();
    for callees in calls.values() {
        for callee in callees {
            let count = indegree
                .get_mut(callee)
                .expect("validated call target is registered");
            *count += 1;
        }
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(machine, count)| (*count == 0).then_some(*machine))
        .collect::<BTreeSet<_>>();
    let mut completed = 0_usize;
    while let Some(machine) = ready.pop_first() {
        completed += 1;
        for callee in &calls[&machine] {
            let count = indegree
                .get_mut(callee)
                .expect("validated call target has an indegree");
            *count -= 1;
            if *count == 0 {
                ready.insert(*callee);
            }
        }
    }
    if completed != calls.len() {
        let machine = indegree
            .into_iter()
            .find_map(|(machine, count)| (count != 0).then_some(machine))
            .expect("incomplete topological order has a cyclic remainder");
        return Err(ModuleError::RecursiveCallSliceNotYetSupported(machine));
    }
    Ok(())
}
