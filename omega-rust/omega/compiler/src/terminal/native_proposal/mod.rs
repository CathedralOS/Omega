//! Project what a later native realization may consume from the retained
//! Terminal artifact: exact provider-plan joins, callback occurrences and
//! thunks, IEEE float custody, and the closed external binding rows.

use crate::checked::CheckedCompilation;
use diagnostics::Diagnostic;
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi_to_abstract_operations::{
    TerminalNativeBoundaryApplications, TerminalNativeOccurrences, TerminalNativeProviders,
    TerminalNativeRealizationProposal,
};

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
    let program_entry = checked.selected_program_entry().ok_or_else(|| {
        vec![Diagnostic::error(
            "Terminal native proposal requires one exact selected ProgramEntry",
        )]
    })?;
    if program_entry.source_signature().target_slot().owner != target_profile {
        return Err(vec![Diagnostic::error(
            "Terminal native proposal target, profile, and ProgramEntry disagree",
        )]);
    }
    // Close the build-owned entry selection into the existing independently
    // replayed native settlement. Retention carries this pipeline product,
    // never the Build evaluator's selection object or a second entry receipt.
    let calling_plans = program_entry.calling_plans().map(|plans| {
        (
            &plans.semantic_calling_application,
            &plans.physical_calling_application,
            &plans.storage_entry,
        )
    });
    let program_entry =
        terminal_psi_to_abstract_operations::validate_native_program_entry_settlement(
            artifact,
            &checked_program_entry,
            terminal_psi_to_abstract_operations::NativeProgramEntrySettlement::new(
                program_entry.source_signature(),
                calling_plans,
                program_entry.fused_service_establishments(),
            )
            .with_checked_entry(&checked_program_entry),
            native_target,
        )
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "Terminal native proposal ProgramEntry settlement failed: {error}"
            ))]
        })?;
    let terminal_module =
        terminal_codec::decode_module(artifact.semantic_bytes()).map_err(|error| {
            vec![Diagnostic::error(format!(
                "Terminal native proposal could not replay canonical semantics: {error}",
            ))]
        })?;
    let demanded_intrinsics =
        terminal_psi_to_abstract_operations::provider_admission::demanded_boundary_identities(
            &terminal_module,
        )?;
    let builtin_proposals =
        terminal_psi_to_abstract_operations::provider_admission::derive_selected_intrinsic_settlement_proposals(
            checked.selected_provider_plans().plans(),
            checked.selected_provider_provenance().iter().map(|retained| {
                terminal_psi_to_abstract_operations::provider_admission::CompilerIntrinsicPlanProvenance {
                    plan: &retained.plan,
                    row_executions: &retained.row_compiler_intrinsic_executions,
                }
            }),
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
        selections,
    )?;
    let ieee_float_comparison_occurrences =
        terminal_psi_to_abstract_operations::settle_ieee_float_comparison_occurrences(
            checked,
            &terminal_module,
            checked.selected_provider_plans(),
            checked.selected_provider_provenance(),
            selected_ieee_float_comparison_occurrences,
        )?;
    let integer_comparison_occurrences =
        terminal_psi_to_abstract_operations::settle_integer_comparison_occurrences(
            checked,
            &terminal_module,
            checked.selected_provider_plans(),
            checked.selected_provider_provenance(),
            selected_integer_comparison_occurrences,
        )?;
    let boundary_application_coverage =
        terminal_psi_to_abstract_operations::BoundaryApplicationSettlement {
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
        }
        .settle(artifact, &checked_boundary_operator_scope)?;
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
    TerminalNativeRealizationProposal::new(
        artifact,
        checked.project_native_application_metadata(),
        program_entry,
        selections.project_post_terminal(),
        TerminalNativeProviders {
            selection: checked.selected_provider_plans().clone(),
            external_bindings: external_binding_rows,
            compiler_builtins: builtin_proposals,
        },
        TerminalNativeOccurrences {
            callbacks: callback_occurrences,
            ieee_float_comparisons: ieee_float_comparison_occurrences,
            integer_comparisons: integer_comparison_occurrences,
        },
        TerminalNativeBoundaryApplications {
            scope: checked_boundary_operator_scope,
            coverage: boundary_application_coverage,
        },
        package_terminal_authority_permissions,
        effects::authored_behavior_exclusion_set_in(
            checked.behavior_exclusions(),
            &terminal_module,
            &|symbol| effects::boundary_trait_identity(checked, symbol),
        ),
    )
    .map_err(|message| vec![Diagnostic::error(message)])
}

/// Rejoin retained callback placements to their exact Terminal registrar
/// occurrences and produce each occurrence's thunk artifact.
fn project_callback_occurrences(
    checked: &CheckedCompilation,
    profile: &proof_admission::AdmissionProfile,
    callback_placements: &[backend_plan::BoundNominalCallbackPlacement],
    source_call_occurrences: &[lowered_psi::LoweredSourceCallOccurrence],
    selections: &optimization_core::OptimizationSelections,
) -> Result<Vec<crate::report::TerminalCallbackOccurrenceProposal>, Vec<Diagnostic>> {
    let callback_source_calls = backend_plan::CallbackRegistrarOccurrences::new(
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
                produce_callback_thunk_artifact(checked, profile, placement, selections)?;
            Ok(crate::report::TerminalCallbackOccurrenceProposal::new(
                placement_index,
                occurrence.terminal_operation,
                placement
                    .private_materialization
                    .as_ref()
                    .map(|materialization| materialization.destination_parameter_application().cloned())
                    .transpose()
                    .map_err(|error| vec![Diagnostic::error(error.0)])?,
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
