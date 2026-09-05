use super::*;

pub(super) fn validate_call_graph(module: &TerminalModule) -> Result<(), ModuleError> {
    let calls = module
        .machines
        .iter()
        .map(|machine| {
            let mut callees = BTreeSet::new();
            for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
                match &operation.kind {
                    OperationKind::Call { callee, .. }
                    | OperationKind::CallUnit { callee, .. }
                    | OperationKind::CallStructuralScalar { callee, .. }
                    | OperationKind::CallStructural { callee, .. }
                    | OperationKind::CallStructuralWithScalarArguments { callee, .. } => {
                        callees.insert(*callee);
                    }
                    OperationKind::CallDynamicScalar { .. } => {
                        let realization = module
                            .dynamic_dispatch
                            .indirect_dispatches
                            .iter()
                            .find(|dispatch| {
                                dispatch.owner == machine.id && dispatch.operation == operation.id
                            })
                            .map(|dispatch| dispatch.realization)
                            .or_else(|| {
                                module.dynamic_dispatch.stored_dispatches.iter().find_map(
                                    |dispatch| {
                                        (dispatch.owner == machine.id
                                            && dispatch.operation == operation.id)
                                            .then_some(dispatch.realization)
                                    },
                                )
                            })
                            .expect("validated dynamic call has one dispatch row");
                        callees.insert(realization);
                    }
                    OperationKind::BoundaryCall { boundary, .. } => {
                        callees.extend(
                            module
                                .provider_candidates
                                .iter()
                                .filter(|candidate| candidate.boundary == *boundary)
                                .map(|candidate| candidate.candidate),
                        );
                    }
                    _ => {}
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
