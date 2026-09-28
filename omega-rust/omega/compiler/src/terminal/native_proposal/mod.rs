//! Produce callback Terminal bodies, then hand complete products to Omega
//! stage 00 for native proposal construction and source-to-artifact joins.

use crate::checked::CheckedCompilation;
use diagnostics::Diagnostic;
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi_to_abstract_operations::TerminalNativeRealizationProposal;

pub(crate) fn project_terminal_native_realization_proposal(
    checked: &CheckedCompilation,
    profile: &proof_admission::AdmissionProfile,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    checked_program_entry: terminal_psi::CheckedProgramEntryTerminalReceipt,
    checked_boundary_operator_scope: lowered_psi_to_terminal_psi::CheckedBoundaryOperatorApplicationScope,
    callback_placements: &[backend_plan::BoundNominalCallbackPlacement],
    source_call_occurrences: &[lowered_psi::LoweredSourceCallOccurrence],
    selected_ieee_float_comparison_occurrences: &[lowered_psi::LoweredSelectedIeeeFloatComparisonOccurrence],
    selected_integer_comparison_occurrences: &[lowered_psi::LoweredSelectedIntegerComparisonOccurrence],
    selections: &optimization_core::OptimizationSelections,
) -> Result<TerminalNativeRealizationProposal, Vec<Diagnostic>> {
    let callback_thunks = callback_placements
        .iter()
        .map(|placement| produce_callback_thunk_artifact(checked, profile, placement, selections))
        .collect::<Result<Vec<_>, _>>()?;
    terminal_psi_to_abstract_operations::project_terminal_native_realization_proposal(
        artifact,
        checked_program_entry,
        checked_boundary_operator_scope,
        terminal_psi_to_abstract_operations::TerminalNativeProposalRequest {
            boundaries: terminal_psi_to_abstract_operations::BoundaryApplicationSettlement {
                checked,
                selected_provider_plans: checked.selected_provider_plans(),
                selected_provider_provenance: checked.selected_provider_provenance(),
                exact_toolchain_sources: checked.exact_toolchain_sources(),
                target_profile: checked.selected_target_profile(),
                console_exit_binding: checked.resolved_semantic_binding(
                    package_compilation::AcceptedSemanticBindingRole::ConsoleExitProcessI32,
                ),
                process_exit_binding: checked.resolved_semantic_binding(
                    package_compilation::AcceptedSemanticBindingRole::ProcessExitExitProcessI32,
                ),
                calling_plan_realizations: checked.boundary_calling_plan_realizations(),
            },
            native_target: checked.selected_native_target(),
            program_entry: checked.selected_program_entry(),
            application: checked.project_native_application_metadata(),
            optimizations: selections.project_post_terminal(),
            external_bindings: checked.external_binding_rows(),
            behavior_exclusions: checked.behavior_exclusions(),
            package_terminal_authority_permissions: checked
                .package_terminal_authority_permissions(),
            callback_placements,
            callback_thunks,
            source_call_occurrences,
            selected_ieee_float_comparison_occurrences,
            selected_integer_comparison_occurrences,
        },
    )
}

fn produce_callback_thunk_artifact(
    checked: &CheckedCompilation,
    profile: &proof_admission::AdmissionProfile,
    placement: &backend_plan::BoundNominalCallbackPlacement,
    selections: &optimization_core::OptimizationSelections,
) -> Result<crate::report::TerminalCallbackThunkArtifact, Vec<Diagnostic>> {
    // The thunk body leaves through the same production body as the program
    // entry: one lowering, the selected Psi optimization, canonical
    // publication. It carries no entry identity, so no `ProgramEntry`
    // receipt is minted; the callback placement binds its own evidence to
    // the lowering receipt instead.
    let psi_optimizations = selections.project_psi();
    let mut production_timings = TerminalProductionTimings::default();
    let produced = terminal_production::TerminalProductionRequest {
        checked: checked.terminal_production_trees(),
        machine: terminal_production::TerminalMachineSelection::Symbol(placement.selected_machine),
        optimization_selections: psi_optimizations.selections().clone(),
    }
    .produce(TerminalProductionCustody {
        entry_identity: None,
        callback_custody: (),
        // The authored exclusions and the lowering receipt are judged on the
        // unoptimized composition the thunk was published from.
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
        checked.behavior_exclusions(),
        checked.selected_provider_plans(),
        produced.unoptimized(),
        placement.selected_machine,
    )?;
    let unoptimized = produced.unoptimized().ok_or_else(|| {
        vec![Diagnostic::error(
            "callback thunk production retained no unoptimized Terminal lowering to join the checked coordinate onto",
        )]
    })?;
    let receipt = checked_trees_to_lowered_psi::callback_lowering_receipt(
        checked,
        unoptimized,
        placement.selected_machine,
        placement.selected_entry,
    )
    .map_err(|error| {
        vec![Diagnostic::error(format!(
            "callback thunk Terminal lowering failed: {error}",
        ))]
    })?;
    let artifact = produced.into_artifact();
    terminal_codec::verify_terminal_artifact_proof(&artifact, profile).map_err(|error| {
        vec![Diagnostic::error(format!(
            "terminal-artifact verification failed for {}: {}",
            error.subject, error.reason,
        ))]
    })?;
    crate::report::TerminalCallbackThunkArtifact::new(
        backend_plan::canonical_callback_private_symbol(placement),
        artifact,
        receipt,
    )
    .map_err(|message| vec![Diagnostic::error(message)])
}
