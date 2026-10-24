//! Exact embedded scalar helper closure and source eligibility.

use super::super::CheckedTerminalSignatureEligibility;
use super::{CheckedTrees, LoweringError, unsupported};

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
    let mut structural_members = structural_roots.to_vec();
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
        );
        if selection.name.is_empty() || !supported_signature {
            return unsupported(
                "embedded scalar call closure has an unsupported terminal signature",
            );
        }
        let callee = if structural_members.contains(&machine) {
            crate::scalar_lowering::call_closure::callee::CheckedScalarCallee::find_for_unit_call(
                checked, machine,
            )?
        } else {
            crate::scalar_lowering::call_closure::callee::CheckedScalarCallee::find(
                checked, machine,
            )?
        };
        let mut computed = source_checked_computation_targets(checked, machine)?;
        let mut computed_structural =
            crate::expression_preparation::computation_graph::structural_call_targets(
                checked, machine,
            )?;
        for operation in callee.body().operations() {
            // An ordinary scalar call names a scalar callee.
            if let Some(call) = operation
                .call()
                .filter(|call| call.is_module() && call.scalar_result().is_some())
            {
                let target_machine = call.target.machine;
                if !computed.contains(&target_machine) {
                    computed.push(target_machine);
                }
                if (!call.structural_arguments.is_empty() || !call.claim_transfers().is_empty())
                    && !computed_structural.contains(&target_machine)
                {
                    computed_structural.push(target_machine);
                }
            }
        }
        for target in computed {
            // Root validation above rejoins each mixed signature independently;
            // another call reaching this target does not grant its borrow rows.
            if computed_structural.contains(&target) {
                crate::scalar_lowering::call_closure::callee::CheckedScalarCallee::find_for_unit_call(
                    checked, target,
                )?;
                if !structural_members.contains(&target) {
                    structural_members.push(target);
                }
            } else {
                crate::scalar_lowering::call_closure::callee::CheckedScalarCallee::find(
                    checked, target,
                )?;
            }
            if !closure.contains(&target) {
                closure.push(target);
            }
        }
    }
    Ok(closure)
}

fn source_checked_computation_targets(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    for root in plans.machine_roots(&[machine]) {
        if plans.root_at(root.state, root.statement_ordinal, root.role) != Some(root)
            || !plans.nodes.is_valid(root.root)
        {
            return unsupported("embedded computation has no unique live source root");
        }
    }
    crate::expression_preparation::computation_graph::call_targets(checked, machine)
}
