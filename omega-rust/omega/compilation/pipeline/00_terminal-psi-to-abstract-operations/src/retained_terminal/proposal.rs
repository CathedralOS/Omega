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
    pub boundaries: crate::BoundaryApplicationSettlement<'input>,
    pub native_target: Option<target::NativeTarget>,
    pub program_entry:
        Option<&'input provider_planning::program_entry::SelectedCompilerProgramEntry>,
    pub application: target::NativeApplicationMetadata,
    pub optimizations: optimization_core::PostTerminalOptimizationSelectionProjection,
    pub external_bindings: &'input [calling_conventions::ExternalBindingRow],
    pub behavior_exclusions: &'input [effects::AuthoredBehaviorExclusion],
    pub package_terminal_authority_permissions: Vec<effects::ServiceTerminalAuthorityPermission>,
    pub callback_placements: &'input [backend_plan::BoundNominalCallbackPlacement],
    pub callback_thunks: Vec<crate::TerminalCallbackThunkArtifact>,
    pub source_call_occurrences: &'input [lowered_psi::LoweredSourceCallOccurrence],
    pub selected_ieee_float_comparison_occurrences:
        &'input [lowered_psi::LoweredSelectedIeeeFloatComparisonOccurrence],
    pub selected_integer_comparison_occurrences:
        &'input [lowered_psi::LoweredSelectedIntegerComparisonOccurrence],
}

/// Reconstruct the entry, provider and occurrence joins required by native
/// re-entry. No compilation, publication or execution happens in this operation.
pub fn project_terminal_native_realization_proposal(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    checked_program_entry: terminal_psi::CheckedProgramEntryTerminalReceipt,
    checked_boundary_operator_scope: lowered_psi_to_terminal_psi::CheckedBoundaryOperatorApplicationScope,
    request: TerminalNativeProposalRequest<'_>,
) -> Result<TerminalNativeRealizationProposal, Vec<Diagnostic>> {
    let callback_placements = request.callback_placements;
    let source_call_occurrences = request.source_call_occurrences;
    let selected_ieee_float_comparison_occurrences =
        request.selected_ieee_float_comparison_occurrences;
    let selected_integer_comparison_occurrences = request.selected_integer_comparison_occurrences;
    let target_profile = request.boundaries.target_profile.ok_or_else(|| {
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
    let program_entry = crate::validate_native_program_entry_settlement(
        artifact,
        &checked_program_entry,
        crate::NativeProgramEntrySettlement::new(
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
        crate::provider_admission::demanded_boundary_identities(&terminal_module)?;
    let builtin_proposals =
        crate::provider_admission::derive_selected_intrinsic_settlement_proposals(
            request.boundaries.selected_provider_plans.plans(),
            request
                .boundaries
                .selected_provider_provenance
                .iter()
                .map(
                    |retained| crate::provider_admission::CompilerIntrinsicPlanProvenance {
                        plan: &retained.plan,
                        row_executions: &retained.row_compiler_intrinsic_executions,
                    },
                ),
            &demanded_intrinsics,
            &crate::provider_admission::boundary_float_formats(&terminal_module.boundary_machines),
        )?
        .into_iter()
        .map(|proposal| {
            crate::TerminalCompilerBuiltinProposal::new(
                proposal.requirement_identity,
                proposal.plan_index,
                proposal.execution,
            )
            .map_err(|message| vec![Diagnostic::error(message)])
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    let callback_occurrences = project_callback_occurrences(
        callback_placements,
        source_call_occurrences,
        request.boundaries.calling_plan_realizations,
        request.callback_thunks,
    )?;
    let ieee_float_comparison_occurrences = crate::settle_ieee_float_comparison_occurrences(
        request.boundaries.checked,
        &terminal_module,
        request.boundaries.selected_provider_plans,
        request.boundaries.selected_provider_provenance,
        selected_ieee_float_comparison_occurrences,
    )?;
    let integer_comparison_occurrences = crate::settle_integer_comparison_occurrences(
        request.boundaries.checked,
        &terminal_module,
        request.boundaries.selected_provider_plans,
        request.boundaries.selected_provider_provenance,
        selected_integer_comparison_occurrences,
    )?;
    let boundary_application_coverage = request
        .boundaries
        .settle(artifact, &checked_boundary_operator_scope)?;
    let external_binding_rows = crate::settle_callback_registrar_imports(
        &terminal_module,
        callback_placements,
        callback_occurrences
            .iter()
            .map(|occurrence| crate::CallbackRegistrarOccurrence {
                placement_index: occurrence.placement_index(),
                terminal_operation: occurrence.terminal_operation(),
            }),
        request.boundaries.calling_plan_realizations,
        request.external_bindings,
    )
    .map_err(|message| vec![Diagnostic::error(message)])?;
    TerminalNativeRealizationProposal::new(
        artifact,
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
            ieee_float_comparisons: ieee_float_comparison_occurrences,
            integer_comparisons: integer_comparison_occurrences,
        },
        TerminalNativeBoundaryApplications {
            scope: checked_boundary_operator_scope,
            coverage: boundary_application_coverage,
        },
        request.package_terminal_authority_permissions,
        effects::authored_behavior_exclusion_set_in(
            request.behavior_exclusions,
            &terminal_module,
            &|symbol| effects::boundary_trait_identity(request.boundaries.checked, symbol),
        ),
    )
    .map_err(|message| vec![Diagnostic::error(message)])
}

/// Rejoin retained callback placements to their exact Terminal registrar
/// occurrences and independently check each supplied thunk's native interface.
fn project_callback_occurrences(
    callback_placements: &[backend_plan::BoundNominalCallbackPlacement],
    source_call_occurrences: &[lowered_psi::LoweredSourceCallOccurrence],
    calling_plans: &[provider_planning::calling_policy_plans::BoundaryCallingPlanRealization],
    thunks: Vec<crate::TerminalCallbackThunkArtifact>,
) -> Result<Vec<crate::TerminalCallbackOccurrenceProposal>, Vec<Diagnostic>> {
    if callback_placements.len() != thunks.len() {
        return Err(vec![Diagnostic::error(
            "callback thunk roster differs from checked placement roster",
        )]);
    }
    let callback_source_calls = backend_plan::CallbackRegistrarOccurrences::new(
        callback_placements
            .iter()
            .map(|placement| (placement.site, placement.registration_operation)),
        source_call_occurrences,
    );
    let occurrences = callback_placements
        .iter()
        .zip(thunks)
        .enumerate()
        .map(|(placement_index, (placement, callback_thunk_artifact))| {
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
            crate::validate_callback_thunk_settlement(
                callback_thunk_artifact.artifact(), placement, calling_plans,
            ).map_err(|message| vec![Diagnostic::error(message)])?;
            Ok(crate::TerminalCallbackOccurrenceProposal::new(
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
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    backend_plan::validate_callback_occurrence_placements(callback_placements, &occurrences)
        .map_err(|message| vec![Diagnostic::error(message)])?;
    Ok(occurrences)
}
