//! Produce callback Terminal bodies, then hand complete products to Omega
//! stage 00 for native proposal construction and source-to-artifact joins.

use crate::checked::CheckedCompilation;
use diagnostics::Diagnostic;
use terminal_production::TerminalProductionCustody;
use terminal_psi_to_abstract_operations::TerminalNativeRealizationProposal;

pub(crate) fn project_terminal_native_realization_proposal(
    checked: &CheckedCompilation,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    checked_program_entry: terminal_psi::CheckedProgramEntryTerminalReceipt,
    selections: &optimization_core::OptimizationSelections,
) -> Result<TerminalNativeRealizationProposal, Vec<Diagnostic>> {
    let callback_placements = checked.execution().callback_placements.as_slice();
    let callback_thunks = callback_placements
        .iter()
        .map(|placement| produce_callback_thunk_artifact(checked, placement, selections))
        .collect::<Result<Vec<_>, _>>()?;
    terminal_psi_to_abstract_operations::project_terminal_native_realization_proposal(
        artifact,
        checked_program_entry,
        checked.terminal_native_proposal_request(callback_placements, callback_thunks, selections),
    )
}

fn produce_callback_thunk_artifact(
    checked: &CheckedCompilation,
    placement: &provider_planning::calling_policy_plans::SelectedCallbackPlacement,
    selections: &optimization_core::OptimizationSelections,
) -> Result<compilation_products::TerminalCallbackThunkArtifact, Vec<Diagnostic>> {
    // The thunk body leaves through the same production body as the program
    // entry: one lowering, the selected Psi optimization, canonical
    // publication. It carries no entry identity, so no `ProgramEntry`
    // receipt is minted. Psi retains the selected machine's declaration in
    // the artifact itself, and native preparation joins the callback demand.
    let psi_optimizations = selections.project_psi();
    let mut production_timings = artifacts::compile_timings::CompileTimings::default();
    let produced = terminal_production::TerminalProductionRequest {
        checked: &checked.execution().program,
        exact_toolchain_sources: checked.sources().exact_toolchain_sources(),
        machine: terminal_production::TerminalMachineSelection::Symbol(placement.selected_machine),
        optimization_selections: psi_optimizations.selections().clone(),
    }
    .produce(TerminalProductionCustody {
        entry_identity: None,
        // Authored exclusions are judged on the unoptimized composition.
        retain_unoptimized: true,
        timings: &mut production_timings,
    })
    .map_err(|error| {
        vec![Diagnostic::error(format!(
            "callback thunk Terminal production failed: {error}",
        ))]
    })?;
    // The callback body is part of the admitted composition; the authored
    // exclusions apply to its unoptimized module exactly as to the program
    // entry's.
    trust_model::verify_entry_behavior_exclusions(
        checked,
        checked.build().config.behavior_exclusions.as_slice(),
        &checked.execution().selected_provider_plan_facts,
        produced.unoptimized(),
        placement.selected_machine,
    )?;
    compilation_products::TerminalCallbackThunkArtifact::new(produced.into_artifact())
        .map_err(|message| vec![Diagnostic::error(message)])
}
