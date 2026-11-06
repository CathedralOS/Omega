//! A checking-only product still owes the lender duties used by its callers.
//! Resolve each requested call through the same admitted provider and target
//! choices as execution. Missing or opaque supplies cannot prove preservation.

use crate::{
    AdmittedProviderSelections, AdmittedTargetMachineSelections, admitted_boundary_member,
};
use terminal_codec::TerminalArtifactView;
use terminal_psi::{OperationKind, TargetMachineCallable};

/// Check only explicitly requested duties. The requests select original
/// subjects at their own call cuts; neither a declaration row nor a selected
/// body's input qualification supplies their normal-return proof.
pub fn require_selected_lender_preservation(
    artifact: TerminalArtifactView<'_>,
    providers: &AdmittedProviderSelections,
    targets: &AdmittedTargetMachineSelections,
) -> Result<(), String> {
    if providers.terminal_psi() != artifact.identity()
        || targets.terminal_psi() != artifact.identity()
    {
        return Err("lender preservation choices belong to another artifact".into());
    }
    let module = artifact.module();
    let mut boundaries = std::collections::BTreeSet::new();
    for (machine, operation) in module
        .boundary_lender_preservation
        .iter()
        .map(|demand| (demand.machine, demand.operation))
        .chain(
            module
                .boundary_reference_forwarding
                .iter()
                .map(|demand| (demand.machine, demand.operation)),
        )
    {
        let caller = module
            .machine(machine)
            .ok_or("a lender request has no caller")?;
        let mut operations = caller
            .operations()
            .filter(|candidate| candidate.id == operation);
        let operation = operations
            .next()
            .ok_or("a lender request has no operation")?;
        if operations.next().is_some() {
            return Err("a lender request has ambiguous operations".into());
        }
        let OperationKind::BoundaryCall { boundary, .. } = operation.kind else {
            return Err("a lender request does not name a boundary call".into());
        };
        boundaries.insert(boundary);
    }
    for boundary in boundaries {
        let assumption = providers
            .assumed_bindings()
            .iter()
            .find(|binding| binding.required == boundary);
        let installed = |required| {
            providers
                .providers()
                .find_map(|(found, body)| (found == required).then_some(body))
        };
        let installed = assumption
            .map(|binding| binding.implementation)
            .or_else(|| installed(boundary));
        let member = admitted_boundary_member(module, Some(targets), boundary)?;
        // Final native call selection refuses competing installed and family
        // supplies. Retain that refusal here instead of proving one body's
        // writes and relying on a different dispatch order at execution.
        if installed.is_some() && member.is_some() {
            return Err(
                "lender preservation has competing installed and target-family supplies".into(),
            );
        }
        let implementation = match member {
            Some(TargetMachineCallable::Machine(body)) => Some(body),
            _ => installed,
        };
        terminal_semantics::descriptor_results::require_boundary_lender_preservation(
            module,
            boundary,
            implementation,
        )
        .map_err(|reason| format!("lender preservation at {boundary:?}: {reason}"))?;
    }
    Ok(())
}
