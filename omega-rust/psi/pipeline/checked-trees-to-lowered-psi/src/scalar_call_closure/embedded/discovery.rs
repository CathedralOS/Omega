//! Exact embedded scalar helper closure and source eligibility.

use super::*;

/// Discover the scalar graphs selected by exact calls in an external caller.
/// Embedded roots retain their caller attachment; every
/// transitive attached callee needs an exact source-validated static computation
/// edge. Other transitive scalar callees pass the generic signature fence.
pub(crate) fn checked_scalar_call_closure(
    checked: &CheckedTrees,
    roots: &[symbols::SymbolHandle],
) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
    let mut closure = Vec::new();
    for root in roots {
        if !closure.contains(root) {
            closure.push(*root);
        }
    }
    let embedded_roots = closure.clone();
    let mut computation_targets = Vec::new();
    let mut attached_members = Vec::new();
    let mut next = 0_usize;
    while let Some(machine) = closure.get(next).copied() {
        next += 1;
        let selections = checked
            .facts
            .flow
            .terminal_machines
            .machines
            .iter()
            .filter(|selection| selection.machine == machine)
            .collect::<Vec<_>>();
        let [selection] = selections.as_slice() else {
            return unsupported("embedded scalar call has no unique checked terminal selection");
        };
        let supported_signature = matches!(
            selection.signature,
            CheckedTerminalSignatureEligibility::Eligible
                | CheckedTerminalSignatureEligibility::Attached
        );
        if selection.name.is_empty() || !supported_signature {
            return unsupported(
                "embedded scalar call closure has an unsupported terminal signature",
            );
        }
        if selection.signature == CheckedTerminalSignatureEligibility::Attached {
            attached_members.push(machine);
        }
        let graph = checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(machine)
            .ok_or(LoweringError::Unsupported(
                "embedded scalar call has no checked scalar graph",
            ))?;
        let computed = source_checked_computation_targets(checked, machine)?;
        for target in &computed {
            if !computation_targets.contains(target) {
                computation_targets.push(*target);
            }
        }
        for target in graph
            .states
            .iter()
            .flat_map(|state| {
                state.bindings.iter().filter_map(|binding| {
                    let CheckedScalarBindingValue::DirectCall { target_machine, .. } =
                        &binding.value
                    else {
                        return None;
                    };
                    Some(*target_machine)
                })
            })
            .chain(computed)
        {
            if !closure.contains(&target) {
                closure.push(target);
            }
        }
    }
    // Check after discovery so two callers reaching the same callee do not make
    // static eligibility depend on the closure's worklist order.
    if attached_members
        .iter()
        .any(|machine| !embedded_roots.contains(machine) && !computation_targets.contains(machine))
    {
        return unsupported("embedded scalar call closure has an unsupported terminal signature");
    }
    Ok(closure)
}

fn source_checked_computation_targets(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    for (_, root) in plans
        .roots
        .iter()
        .filter(|(_, root)| root.machine == machine)
    {
        if plans.root_at(root.state, root.statement_ordinal, root.role) != Some(root)
            || !plans.nodes.is_valid(root.root)
        {
            return unsupported("embedded computation has no unique live source root");
        }
        let source = crate::scalar_source_custody::locate(
            checked,
            root.state,
            root.statement_ordinal,
            root.role,
        )?;
        let node = plans.nodes.get(root.root);
        if source.machine != machine
            || source.expression != node.authored_root
            || source.primitive_type != node.primitive_type
        {
            return unsupported("embedded computation disagrees with its authored source root");
        }
        crate::scalar_source_custody::validate_computation_calls(
            checked,
            machine,
            root.state,
            root.statement_ordinal,
            root.root,
            source.expression,
        )?;
    }
    crate::scalar_computations::call_targets(checked, machine)
}
