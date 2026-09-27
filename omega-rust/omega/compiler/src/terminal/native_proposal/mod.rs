//! Project what a later native realization may consume from the retained
//! Terminal artifact: exact provider-plan joins, callback occurrences and
//! thunks, IEEE float custody, and the closed external binding rows.

use crate::checked::CheckedCompilation;
use crate::terminal::application_coverage::project_terminal_boundary_application_coverage;
use crate::terminal::terminal_artifact::verification::verify_terminal_artifact;
use crate::terminal::{float_comparisons, integer_comparisons};
use diagnostics::Diagnostic;
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};

mod callback_registrars;

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
) -> Result<crate::report::TerminalNativeRealizationProposal, Vec<Diagnostic>> {
    let target_profile = checked.selected_target_profile().ok_or_else(|| {
        vec![Diagnostic::error(
            "Terminal native proposal requires one selected target profile",
        )]
    })?;
    let native_target = checked.selected_native_target().ok_or_else(|| {
        vec![Diagnostic::error(
            "Terminal native proposal requires one selected native target",
        )]
    })?;
    let program_entry = checked.selected_program_entry().cloned().ok_or_else(|| {
        vec![Diagnostic::error(
            "Terminal native proposal requires one exact selected ProgramEntry",
        )]
    })?;
    let terminal_module =
        terminal_codec::decode_module(artifact.semantic_bytes()).map_err(|error| {
            vec![Diagnostic::error(format!(
                "Terminal native proposal could not replay canonical semantics: {error}",
            ))]
        })?;
    let demanded_intrinsics =
        provider_planning::compiler_intrinsics::demanded_boundary_identities(&terminal_module)?;
    let builtin_proposals =
        provider_planning::compiler_intrinsics::derive_selected_intrinsic_settlement_proposals(
            checked.selected_provider_plans().plans(),
            checked.selected_provider_provenance(),
            &demanded_intrinsics,
        )?
        .into_iter()
        .map(|proposal| {
            crate::report::TerminalCompilerBuiltinProposal::new(
                proposal.requirement_identity,
                proposal.plan_index,
                proposal.execution,
            )
            .map_err(|message| vec![Diagnostic::error(message)])
        })
        .collect::<Result<Vec<_>, _>>()?;
    let callback_occurrences = project_callback_occurrences(
        checked,
        profile,
        callback_placements,
        source_call_occurrences,
    )?;
    let ieee_float_comparison_occurrences = float_comparisons::associate(
        checked,
        &terminal_module,
        checked.selected_provider_plans(),
        checked.selected_provider_provenance(),
        selected_ieee_float_comparison_occurrences,
    )?;
    let integer_comparison_occurrences = integer_comparisons::associate(
        checked,
        &terminal_module,
        checked.selected_provider_plans(),
        checked.selected_provider_provenance(),
        selected_integer_comparison_occurrences,
    )?;
    let boundary_application_coverage = project_terminal_boundary_application_coverage(
        checked,
        artifact,
        &checked_boundary_operator_scope,
    )?;
    let (
        boundary_application_demands,
        boundary_application_realizations,
        boundary_opaque_applications,
    ) = boundary_application_coverage.into_parts();
    let external_binding_rows =
        terminal_psi_to_abstract_operations::settle_callback_registrar_imports(
            &terminal_module,
            callback_placements,
            callback_occurrences.iter().map(|occurrence| {
                terminal_psi_to_abstract_operations::CallbackRegistrarOccurrence {
                    placement_index: occurrence.placement_index(),
                    terminal_operation: occurrence.terminal_operation(),
                }
            }),
            checked.boundary_calling_plan_realizations(),
            checked.external_binding_rows(),
        )
        .map_err(|message| vec![Diagnostic::error(message)])?;
    let package_terminal_authority_permissions = checked
        .resolved_semantic_bindings()
        .flat_map(|binding| binding.terminal_authority_permissions())
        .cloned()
        .collect();
    crate::report::TerminalNativeRealizationProposal::new(
        artifact,
        crate::report::TerminalNativeRealizationInputs {
            target_profile,
            native_target,
            subsystem: checked.subsystem(),
            application_intent: checked.application_intent(),
            application_identifier: checked.application_identifier().cloned(),
            application_name: checked
                .application_name()
                .map(|name| name.as_str().to_owned()),
            post_terminal_optimizations: selections.project_post_terminal(),
            program_entry,
            checked_program_entry,
            selected_provider_plans: checked.selected_provider_plans().clone(),
            external_binding_rows,
            // The proposal retains the exact selected-application commitment
            // at each by-value opaque edge while the checked signature custody
            // still exists; artifacts compared across an exchange must carry
            // the same set.
            boundary_opaque_applications,
            package_terminal_authority_permissions,
            compiler_builtins: builtin_proposals,
            callback_occurrences,
            ieee_float_comparison_occurrences,
            integer_comparison_occurrences,
            boundary_application_demands,
            boundary_application_realizations,
            checked_boundary_operator_scope,
            behavior_exclusions: build_evaluation::authored_behavior_exclusion_set_in(
                checked.behavior_exclusions(),
                &terminal_module,
                &crate::terminal::terminal_artifact::behavior_exclusions::boundary_trait_identity(
                    checked,
                ),
            ),
        },
    )
    .map_err(|message| vec![Diagnostic::error(message)])
}

/// Rejoin retained callback placements to their exact Terminal registrar
/// occurrences and produce each occurrence's thunk artifact. Both native
/// routes — retained product and direct program-entry — admit the same rows.
pub(crate) fn project_callback_occurrences(
    checked: &CheckedCompilation,
    profile: &proof_admission::AdmissionProfile,
    callback_placements: &[backend_plan::BoundNominalCallbackPlacement],
    source_call_occurrences: &[lowered_psi::LoweredSourceCallOccurrence],
) -> Result<Vec<crate::report::TerminalCallbackOccurrenceProposal>, Vec<Diagnostic>> {
    let callback_source_calls = callback_registrars::CallbackSourceCalls::new(
        callback_placements
            .iter()
            .map(|placement| (placement.site, placement.registration_operation)),
        source_call_occurrences,
    );
    callback_placements
        .iter()
        .enumerate()
        .map(|(placement_index, placement)| {
            let occurrence = callback_source_calls
                .find(placement.site, placement.registration_operation)
                .map_err(|count| vec![Diagnostic::error(format!(
                    "callback placement {placement_index} resolves to {count} Terminal registrar occurrences; exactly one is required",
                ))])?;
            let callback_thunk_identity =
                backend_plan::canonical_callback_thunk_identity(placement_index, placement)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(format!(
                            "callback placement {placement_index} cannot derive one valid callback-thunk identity",
                        ))]
                    })?;
            let callback_thunk_artifact =
                produce_callback_thunk_artifact(checked, profile, placement)?;
            Ok(crate::report::TerminalCallbackOccurrenceProposal::new(
                placement_index,
                occurrence.terminal_operation,
                placement
                    .private_materialization
                    .as_ref()
                    .and_then(|materialization| {
                        materialization
                            .direct_registrar_parameter_application
                            .clone()
                            .or_else(|| materialization.root_parameter_application.clone())
                    }),
                callback_thunk_identity,
                callback_thunk_artifact,
            ))
        })
        .collect()
}

fn produce_callback_thunk_artifact(
    checked: &CheckedCompilation,
    profile: &proof_admission::AdmissionProfile,
    placement: &backend_plan::BoundNominalCallbackPlacement,
) -> Result<crate::report::TerminalCallbackThunkArtifact, Vec<Diagnostic>> {
    // The thunk body leaves through the same production body as the program
    // entry: one lowering, the selected Psi optimization, canonical
    // publication. It carries no entry identity, so no `ProgramEntry`
    // receipt is minted; the callback placement binds its own evidence to
    // the lowering receipt instead.
    let psi_optimizations = checked.optimization_selections().project_psi();
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
    crate::terminal::terminal_artifact::behavior_exclusions::verify_entry_behavior_exclusions(
        checked,
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
    verify_terminal_artifact(&artifact, profile)?;
    terminal_psi_to_abstract_operations::validate_callback_thunk_settlement(
        &artifact,
        placement,
        checked.boundary_calling_plan_realizations(),
    )
    .map_err(|message| vec![Diagnostic::error(message)])?;
    crate::report::TerminalCallbackThunkArtifact::new(
        backend_plan::canonical_callback_private_symbol(placement),
        artifact,
        receipt,
    )
    .map_err(|message| vec![Diagnostic::error(message)])
}
