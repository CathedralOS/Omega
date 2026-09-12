//! Exact embedded scalar helper closure and source eligibility.

use super::*;

/// Discover scalar bodies selected by exact calls in an external caller.
/// Embedded roots retain their caller attachment; every
/// transitive attached callee needs an exact source-validated static computation
/// edge. Other transitive scalar callees pass the generic signature fence.
/// Operation-body calls use the shared assembler's exact authored call custody;
/// discovery only retains their targets and cannot authorize their execution.
pub(crate) fn checked_scalar_call_closure(
    checked: &CheckedTrees,
    roots: &[symbols::SymbolHandle],
) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
    let closure = checked_scalar_call_closure_with_structural_roots(checked, roots, &[])?;
    // The legacy standalone helper emitter has no service namespace. Shared
    // assembly uses the discovery entry below and retains those contracts.
    for machine in &closure {
        let mut reaches = checked
            .facts
            .service_reaches
            .machines()
            .iter()
            .filter(|reach| reach.machine == *machine);
        let reach = reaches.next().ok_or(LoweringError::Unsupported(
            "scalar helper lost its checked service contract",
        ))?;
        if reaches.next().is_some() {
            return unsupported("scalar helper has ambiguous checked service contracts");
        }
        if !checked
            .facts
            .service_reaches
            .rows
            .services(reach.effective)
            .is_empty()
            || !reach.unresolved_installation_reaches.is_empty()
        {
            return unsupported("service-bearing scalar helpers require the shared catalog");
        }
    }
    Ok(closure)
}

pub(crate) fn checked_scalar_call_closure_with_structural_roots(
    checked: &CheckedTrees,
    roots: &[symbols::SymbolHandle],
    structural_roots: &[symbols::SymbolHandle],
) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
    let mut closure = Vec::new();
    for root in roots {
        if !closure.contains(root) {
            closure.push(*root);
        }
    }
    let embedded_roots = closure.clone();
    let mut computation_targets = Vec::new();
    let mut structural_members = structural_roots.to_vec();
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
                | CheckedTerminalSignatureEligibility::FreeUnitEffect
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
        let callee = if structural_members.contains(&machine) {
            crate::scalar_call_closure::callee::CheckedScalarCallee::find_for_unit_call(
                checked, machine,
            )?
        } else {
            crate::scalar_call_closure::callee::CheckedScalarCallee::find(checked, machine)?
        };
        let direct_targets = match callee {
            crate::scalar_call_closure::callee::CheckedScalarCallee::Graph(graph) => graph
                .states
                .iter()
                .flat_map(|state| state.bindings.iter())
                .filter_map(|binding| match &binding.value {
                    CheckedScalarBindingValue::DirectCall { target_machine, .. } => {
                        Some(*target_machine)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>(),
            crate::scalar_call_closure::callee::CheckedScalarCallee::Boundary(_) => Vec::new(),
            crate::scalar_call_closure::callee::CheckedScalarCallee::Structural(_) => Vec::new(),
            crate::scalar_call_closure::callee::CheckedScalarCallee::Operations(_) => Vec::new(),
        };
        let mut computed = source_checked_computation_targets(checked, machine)?;
        let mut computed_structural =
            crate::scalar_computations::structural_call_targets(checked, machine)?;
        if let crate::scalar_call_closure::callee::CheckedScalarCallee::Operations(plan) = callee {
            for operation in &plan.operations {
                if let CheckedUnitEffectOperationPlan::ScalarCall {
                    target_machine,
                    structural_arguments,
                    claim_transfers,
                    ..
                } = operation
                {
                    if !computed.contains(target_machine) {
                        computed.push(*target_machine);
                    }
                    if (!structural_arguments.is_empty() || !claim_transfers.is_empty())
                        && !computed_structural.contains(target_machine)
                    {
                        computed_structural.push(*target_machine);
                    }
                }
            }
        }
        for target in &computed {
            if !computation_targets.contains(target) {
                computation_targets.push(*target);
            }
        }
        for target in direct_targets {
            crate::scalar_call_closure::callee::CheckedScalarCallee::find(checked, target)?;
            if !closure.contains(&target) {
                closure.push(target);
            }
        }
        for target in computed {
            // Root validation above rejoins each mixed signature independently;
            // another call reaching this target does not grant its borrow rows.
            if computed_structural.contains(&target) {
                crate::scalar_call_closure::callee::CheckedScalarCallee::find_for_unit_call(
                    checked, target,
                )?;
                if !structural_members.contains(&target) {
                    structural_members.push(target);
                }
            } else {
                crate::scalar_call_closure::callee::CheckedScalarCallee::find(checked, target)?;
            }
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
