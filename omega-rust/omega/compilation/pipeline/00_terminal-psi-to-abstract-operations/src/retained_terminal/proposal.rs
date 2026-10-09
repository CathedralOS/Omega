//! Build native realization evidence from completed Terminal products.
//!
//! This is the producing half of the retained-Terminal contract. It owns
//! canonical decode, entry settlement, provider demand, comparison custody,
//! callback placement and boundary coverage. The compiler supplies already
//! produced callback bodies; this operation never calls back into compilation.
//! Checked selections remain transitional inputs, not native execution authority.

use super::{
    TerminalNativeBoundaryApplications, TerminalNativeOccurrences, TerminalNativeProviders,
    TerminalNativeRealizationProposal,
};
use diagnostics::Diagnostic;

/// Borrow this target's source-side selections while closing them against one
/// canonical artifact. Completed callback bodies arrive in placement order;
/// neither their cardinality nor their identity is trusted.
pub struct TerminalNativeProposalRequest<'input> {
    pub target_profile: Option<target::TargetProfile>,
    pub boundaries: crate::BoundaryApplicationSettlement<'input>,
    pub native_target: Option<target::NativeTarget>,
    pub program_entry: Option<crate::NativeProgramEntrySettlement<'input>>,
    pub application: target::NativeApplicationMetadata,
    pub optimizations: optimization_core::PostTerminalOptimizationSelectionProjection,
    pub external_bindings: &'input [calling_conventions::ExternalBindingRow],
    pub behavior_exclusions: &'input [effects::AuthoredBehaviorExclusion],
    pub package_terminal_authority_permissions: Vec<effects::ServiceTerminalAuthorityPermission>,
    pub callback_placements: Vec<backend_plan::NativeCallbackPlacement>,
    pub callback_thunks: Vec<crate::TerminalCallbackThunkArtifact>,
}

/// Reconstruct the entry, provider and occurrence joins required by native
/// re-entry. No compilation, publication or execution happens in this operation.
pub fn project_terminal_native_realization_proposal(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    checked_program_entry: terminal_psi::CheckedProgramEntryTerminalReceipt,
    request: TerminalNativeProposalRequest<'_>,
) -> Result<TerminalNativeRealizationProposal, Vec<Diagnostic>> {
    let callback_placements = &request.callback_placements;
    let target_profile = request.target_profile.ok_or_else(|| {
        vec![Diagnostic::error(
            "Terminal native proposal requires one selected target profile",
        )]
    })?;
    let native_target = request.native_target.ok_or_else(|| {
        vec![Diagnostic::error(
            "Terminal native proposal requires one selected native target",
        )]
    })?;
    let program_entry = request.program_entry.ok_or_else(|| {
        vec![Diagnostic::error(
            "Terminal native proposal requires one exact selected ProgramEntry",
        )]
    })?;
    if program_entry.source().target_slot().owner != target_profile {
        return Err(vec![Diagnostic::error(
            "Terminal native proposal target, profile, and ProgramEntry disagree",
        )]);
    }
    // Close the build-owned entry selection into the existing independently
    // replayed native settlement. Retention carries this pipeline product,
    // never the Build evaluator's selection object or a second entry receipt.
    let program_entry = crate::validate_native_program_entry_settlement(
        artifact,
        &checked_program_entry,
        program_entry.with_checked_entry(&checked_program_entry),
        native_target,
    )
    .map_err(|error| {
        vec![Diagnostic::error(format!(
            "Terminal native proposal ProgramEntry settlement failed: {error}"
        ))]
    })?;
    let terminal_module = artifact.semantic_module();
    let demanded_intrinsics =
        crate::provider_admission::demanded_boundary_identities(terminal_module)?;
    let builtin_proposals =
        crate::provider_admission::derive_selected_intrinsic_settlement_proposals(
            request.boundaries.selected_provider_plans,
            request
                .boundaries
                .selected_provider_provenance
                .iter()
                .map(
                    |retained| crate::provider_admission::CompilerIntrinsicPlanProvenance {
                        plan: retained.plan,
                        row_executions: retained.row_executions,
                    },
                ),
            &demanded_intrinsics,
            &terminal_module.boundary_machines,
        )?;
    let callback_occurrences = project_callback_occurrences(
        callback_placements,
        artifact.view(),
        request.callback_thunks,
    )?;
    // Close source placements while their producer still owns them. Re-entry
    // borrows these native rows; it never retains the checked-source records.
    let callback_arguments = crate::provider_admission::project_native_callback_arguments(
        callback_placements,
        &callback_occurrences,
    )?;
    backend_plan::validate_callback_thunk_joins(artifact.view(), &callback_occurrences)
        .map_err(|message| vec![Diagnostic::error(message)])?;
    let boundary_application_coverage = request.boundaries.settle(artifact)?;
    let comparisons = crate::comparison_settlement::settle_comparison_occurrences(
        terminal_module,
        request.boundaries.selected_provider_plans,
        &boundary_application_coverage,
    )?;
    let external_binding_rows = crate::settle_callback_registrar_imports(
        terminal_module,
        callback_placements,
        callback_occurrences
            .iter()
            .map(|occurrence| crate::CallbackRegistrarOccurrence {
                placement_index: occurrence.placement_index(),
                terminal_operation: occurrence.terminal_operation(),
            }),
        &request.boundaries.calling_applications,
        request.external_bindings,
    )
    .map_err(|message| vec![Diagnostic::error(message)])?;
    Ok(TerminalNativeRealizationProposal::new(
        request.application,
        program_entry,
        request.optimizations,
        TerminalNativeProviders {
            selection: request.boundaries.selected_provider_plans.clone(),
            external_bindings: external_binding_rows,
            compiler_builtins: builtin_proposals,
        },
        TerminalNativeOccurrences {
            callbacks: callback_occurrences,
            callback_arguments,
            ieee_float_comparisons: comparisons.ieee_float,
            integer_comparisons: comparisons.integer,
        },
        TerminalNativeBoundaryApplications {
            coverage: boundary_application_coverage,
        },
        request.package_terminal_authority_permissions,
        effects::authored_behavior_exclusion_set_in(request.behavior_exclusions, terminal_module),
    ))
}

/// Rejoin retained callback placements to their exact Terminal registrar
/// occurrences. The thunks were compiled from these placements in this
/// compile, so their native interfaces are not re-checked here.
fn project_callback_occurrences(
    callback_placements: &[backend_plan::NativeCallbackPlacement],
    caller: terminal_codec::TerminalArtifactView<'_>,
    thunks: Vec<crate::TerminalCallbackThunkArtifact>,
) -> Result<Vec<crate::TerminalCallbackOccurrenceProposal>, Vec<Diagnostic>> {
    let module = caller.module();
    if callback_placements.len() != thunks.len() {
        return Err(vec![Diagnostic::error(
            "callback thunk roster differs from checked placement roster",
        )]);
    }
    let occurrences = callback_placements
        .iter()
        .zip(thunks)
        .enumerate()
        .map(|(placement_index, (placement, callback_thunk_artifact))| {
            let application = module.callback_applications.binary_search_by_key(
                &placement.application_index, |row| row.application_index
            ).ok().map(|ordinal| &module.callback_applications[ordinal])
                .ok_or_else(|| vec![Diagnostic::error(format!(
                    "callback placement {placement_index} has no Terminal application",
                ))])?;
            if application.static_machine_ordinal != placement.static_machine_ordinal
                || application.requirement != placement.canonical_requirement_overload
                || application.body.contract != placement.contract_commitment
            {
                return Err(vec![Diagnostic::error("callback placement disagrees with its checked Terminal application")]);
            }
            let callback_thunk_identity =
                function_identity::MachineFunctionIdentity::callback_thunk(caller.identity(), placement_index)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(format!(
                            "callback placement {placement_index} cannot derive one valid callback-thunk identity",
                        ))]
                    })?;
            Ok(crate::TerminalCallbackOccurrenceProposal::new(
                placement_index,
                placement.application_index,
                application.registrar,
                placement
                    .private_materialization
                    .as_ref()
                    .map(|materialization| materialization.destination_parameter_application().cloned())
                    .transpose()
                    .map_err(|error| vec![Diagnostic::error(error.0)])?,
                callback_thunk_identity,
                callback_thunk_artifact,
                placement.boundary_entry_plan.clone(),
            ))
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    Ok(occurrences)
}
