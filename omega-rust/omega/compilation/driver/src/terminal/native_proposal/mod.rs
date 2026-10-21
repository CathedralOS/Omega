//! Join native callback selections to the program Psi already produced.
//!
//! Every private ABI adapter names an exact callable in the shared artifact.
//! Neither another lowering run nor another Terminal identity is needed.

use crate::checked::CheckedCompilation;
use diagnostics::Diagnostic;
use std::sync::Arc;
use terminal_psi_to_abstract_operations::TerminalNativeRealizationProposal;

pub(crate) fn project_terminal_native_realization_proposal(
    checked: &CheckedCompilation,
    artifact: &Arc<terminal_codec::CanonicalTerminalArtifact>,
    checked_program_entry: terminal_psi::CheckedProgramEntryTerminalReceipt,
    selections: &optimization_core::OptimizationSelections,
    exported_provider_plans: &[&effects::provider_plan::ProviderPlan],
) -> Result<TerminalNativeRealizationProposal, Vec<Diagnostic>> {
    let callback_placements = checked.execution().callback_placements.as_slice();
    let callback_thunks = callback_placements
        .iter()
        .map(|placement| {
            let signature = psi_compiler::retained_callable_signature(
                &checked.execution().program,
                placement.selected_machine,
                checked.sources().exact_toolchain_sources(),
                artifact,
            )?;
            compilation_products::TerminalCallbackThunkArtifact::new(
                Arc::clone(artifact),
                signature.machine,
                &signature.scalar_source_positions,
                &signature.structural_source_positions,
            )
            .map_err(|message| vec![Diagnostic::error(message)])
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut request = checked.terminal_native_proposal_request(
        callback_placements,
        callback_thunks,
        selections,
    )?;
    let component_plans = exported_provider_plans;
    if !component_plans.is_empty() {
        let native_target = checked
            .execution()
            .selected_native_target()
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "component native publication requires an exact selected target",
                )]
            })?;
        let applications = provider_planning::calling_policy_plans::component_calling_applications(
            &checked.execution().program.typed,
            native_target,
            &component_plans,
            &checked.providers().boundary_calling_plan_realizations,
            &checked.build().config.opaque_representation_selections,
        )?;
        for application in applications {
            if let Some(existing) =
                request
                    .boundaries
                    .calling_applications
                    .iter()
                    .find(|existing| {
                        existing.requirement_identity == application.requirement_identity
                            && existing.target == application.target
                    })
            {
                if existing != &application {
                    return Err(vec![Diagnostic::error(
                        "component ABI differs from the exact already materialized boundary policy",
                    )]);
                }
            } else {
                request.boundaries.calling_applications.push(application);
            }
        }
    }
    let mut proposal =
        terminal_psi_to_abstract_operations::project_terminal_native_realization_proposal(
            artifact,
            checked_program_entry,
            request,
        )?;
    if let Some(adapter) = checked
        .execution()
        .selected_program_entry
        .as_ref()
        .and_then(|entry| entry.physical_adapter())
    {
        let signature = psi_compiler::retained_callable_signature(
            &checked.execution().program,
            adapter,
            checked.sources().exact_toolchain_sources(),
            artifact,
        )?;
        proposal
            .bind_physical_adapter(artifact, signature.machine)
            .map_err(|message| vec![Diagnostic::error(message)])?;
    }
    Ok(proposal)
}
