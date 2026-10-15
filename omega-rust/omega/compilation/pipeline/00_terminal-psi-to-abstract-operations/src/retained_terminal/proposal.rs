//! Build native realization evidence from completed Terminal products.
//!
//! This is the producing half of the retained-Terminal contract. It owns
//! canonical decode, entry settlement, provider demand, comparison custody,
//! callback placement and boundary coverage. The compiler supplies retained
//! callback entries in one program; this never calls back into compilation.
//! Checked selections remain transitional inputs, not native execution authority.

use super::{
    TerminalNativeBodyProposal, TerminalNativeBoundaryApplications, TerminalNativeOccurrences,
    TerminalNativeProviders, TerminalNativeRealizationProposal,
};
use diagnostics::Diagnostic;

/// Borrow this target's source-side selections while closing them against one
/// canonical artifact. Completed callback bodies arrive in placement order;
/// neither their cardinality nor their identity is trusted.
pub struct TerminalNativeProposalRequest<'input> {
    pub provider_declarations:
        Vec<crate::provider_admission::NativeProviderPlanDeclaration<'input>>,
    pub target_profile: Option<target::TargetProfile>,
    pub family_realizations: Vec<SelectedFamilyRealizationSource>,
    pub boundaries: crate::BoundaryApplicationSettlement<'input>,
    pub native_target: Option<target::NativeTarget>,
    pub program_entry: Option<crate::NativeProgramEntrySettlement<'input>>,
    pub application: target::NativeApplicationMetadata,
    pub optimizations: optimization_core::PostTerminalOptimizationSelectionProjection,
    pub external_bindings: &'input [calling_conventions::ExternalBindingRow],
    pub behavior_exclusions: &'input [effects::AuthoredBehaviorExclusion],
    pub package_terminal_authority_permissions: Vec<effects::ServiceTerminalAuthorityPermission>,
    pub package_terminal_authority_permission_identities: terminal_psi::TerminalIdentityTable,
    pub callback_placements: Vec<backend_plan::NativeCallbackPlacement>,
    pub callback_thunks: Vec<crate::TerminalCallbackThunkArtifact>,
    pub x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
}

/// Source custody for one already selected provider row. Its physical member
/// identity is deliberately absent: only the emitted artifact owns that identity.
pub struct SelectedFamilyRealizationSource {
    pub head_declaration_identity: String,
    pub head_callable_identity: String,
    pub target: String,
    pub selected_requirement_identity: String,
    pub provider_plan_report_identity: u64,
    pub provider_plan_digest: effects::provider_plan::ProviderPlanDigest,
}

fn close_family_realizations(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    profile: target::TargetProfile,
    selected: &effects::SelectedProviderPlanFacts,
    sources: &[SelectedFamilyRealizationSource],
) -> Result<effects::SelectedProviderPlanFacts, Vec<Diagnostic>> {
    use terminal_psi::TargetMachineCallable;
    let error = |message: String| vec![Diagnostic::error(message)];
    let selections = terminal_installation::admit_target_machine_selections(
        artifact.view(),
        profile.target_name(),
    );
    let module = artifact.semantic_module();
    let mut rows = Vec::new();
    // The requirement text is provider-side and absent from the module table,
    // so the rows mint into a proposal-local table rather than `index_of`.
    let mut row_identities = terminal_psi::TerminalIdentityTableBuilder::default();
    for source in sources {
        let mut heads = module.boundary_machines.iter().filter(|boundary| {
            boundary.declaration.as_ref().is_some_and(|declaration| {
                module
                    .identities
                    .text(declaration.source.semantic_declaration)
                    == Some(source.head_declaration_identity.as_str())
                    && module.identities.text(declaration.source.callable)
                        == Some(source.head_callable_identity.as_str())
            })
        });
        let Some(head) = heads.next() else {
            // An unretained source family creates no execution demand.
            continue;
        };
        if heads.next().is_some() {
            return Err(error(
                "selected provider family has multiple retained heads".to_owned(),
            ));
        }
        if source.target != selections.target() {
            return Err(error(
                "selected provider family has a different target".to_owned(),
            ));
        }
        let selected_member = selections
            .member(TargetMachineCallable::Boundary(head.id))
            .map_err(|message| error(format!("selected provider family member: {message}")))?;
        let TargetMachineCallable::Boundary(member_id) = selected_member.callable() else {
            // Checked bodies execute through the ordinary family frame path.
            continue;
        };
        let member = terminal_psi::find_boundary_machine(&module.boundary_machines, member_id)
            .ok_or_else(|| {
                error("selected provider family member declaration is absent".to_owned())
            })?;
        rows.push(effects::SelectedTargetFamilyRealization {
            terminal: selections.terminal_psi(),
            target: source.target.clone(),
            head: head.id,
            member: member.id,
            head_identity: row_identities
                .intern(module.identities.text(head.identity).unwrap_or_default()),
            member_identity: row_identities
                .intern(module.identities.text(member.identity).unwrap_or_default()),
            selected_requirement_identity: row_identities
                .intern(&source.selected_requirement_identity),
            provider_plan_report_identity: source.provider_plan_report_identity,
            provider_plan_digest: source.provider_plan_digest,
        });
    }
    selected
        .clone()
        .with_family_realization_rows(rows, &row_identities.build())
        .map_err(error)
}

/// Reconstruct the entry, provider and occurrence joins required by native
/// re-entry. No compilation, publication or execution happens in this operation.
pub fn project_terminal_native_realization_proposal(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    checked_program_entry: terminal_psi::CheckedProgramEntryTerminalReceipt,
    request: TerminalNativeProposalRequest<'_>,
) -> Result<TerminalNativeRealizationProposal, Vec<Diagnostic>> {
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
    let body = project_terminal_native_body(artifact, target_profile, &request)?;
    Ok(TerminalNativeRealizationProposal::new(
        request.application,
        program_entry,
        request.optimizations,
        body.providers,
        body.occurrences,
        body.boundary_applications,
        request.package_terminal_authority_permissions,
        request.package_terminal_authority_permission_identities,
        body.behavior_exclusions,
    ))
}

/// One provider projection for the complete canonical program. Main and
/// callback entries subsequently borrow these same identity-qualified rows.
fn project_terminal_native_body(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    target_profile: target::TargetProfile,
    request: &TerminalNativeProposalRequest<'_>,
) -> Result<TerminalNativeBodyProposal, Vec<Diagnostic>> {
    let terminal_module = artifact.semantic_module();
    let selected = close_family_realizations(
        artifact,
        target_profile,
        request.boundaries.selected_provider_plans,
        &request.family_realizations,
    )?;
    let checked_adapters = crate::provider_admission::retain_selected_provider_adapters(
        &selected,
        &request.provider_declarations,
    )
    .map_err(|message| vec![Diagnostic::error(message)])?;
    let demanded_intrinsics =
        crate::provider_admission::demanded_boundary_identities(terminal_module)?;
    let builtin_proposals =
        crate::provider_admission::derive_selected_intrinsic_settlement_proposals(
            &selected,
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
            &terminal_module.identities,
        )?;
    let callback_occurrences = project_callback_occurrences(
        &request.callback_placements,
        artifact.view(),
        &request.callback_thunks,
    )?;
    // Close source placements while their producer still owns them. Re-entry
    // borrows these native rows; it never retains the checked-source records.
    let callback_arguments = crate::provider_admission::project_native_callback_arguments(
        &request.callback_placements,
        &callback_occurrences,
    )?;
    backend_plan::validate_callback_thunk_joins(artifact.view(), &callback_occurrences)
        .map_err(|message| vec![Diagnostic::error(message)])?;
    let boundary_application_coverage = request.boundaries.settle(artifact)?;
    let comparisons = crate::comparison_settlement::settle_comparison_occurrences(
        terminal_module,
        &selected,
        &boundary_application_coverage,
    )?;
    let external_binding_rows = crate::settle_callback_registrar_imports(
        terminal_module,
        &request.callback_placements,
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
    Ok(TerminalNativeBodyProposal {
        providers: TerminalNativeProviders {
            selection: selected,
            checked_adapters,
            external_bindings: external_binding_rows,
            compiler_builtins: builtin_proposals,
            x86_scalar_fma: request.x86_scalar_fma_provider,
        },
        occurrences: TerminalNativeOccurrences {
            callbacks: callback_occurrences,
            callback_arguments,
            ieee_float_comparisons: comparisons.ieee_float,
            integer_comparisons: comparisons.integer,
        },
        boundary_applications: TerminalNativeBoundaryApplications {
            coverage: boundary_application_coverage,
        },
        behavior_exclusions: effects::authored_behavior_exclusion_set_in(
            request.behavior_exclusions,
            terminal_module,
        ),
    })
}

/// Rejoin retained callback placements to their exact Terminal registrar
/// occurrences. The thunks were compiled from these placements in this
/// compile, so their native interfaces are not re-checked here.
fn project_callback_occurrences(
    callback_placements: &[backend_plan::NativeCallbackPlacement],
    caller: terminal_codec::TerminalArtifactView<'_>,
    thunks: &[crate::TerminalCallbackThunkArtifact],
) -> Result<Vec<crate::TerminalCallbackOccurrenceProposal>, Vec<Diagnostic>> {
    let module = caller.module();
    if callback_placements.len() != thunks.len() {
        return Err(vec![Diagnostic::error(
            "callback thunk roster differs from checked placement roster",
        )]);
    }
    // The source roster may include uses in other canonical bodies, but every
    // retained application in this artifact must still have exactly one use.
    // Filtering out foreign namespaces cannot silently drop a local demand.
    let mut application_indices = callback_placements
        .iter()
        .map(|placement| placement.application_index)
        .collect::<Vec<_>>();
    application_indices.sort_unstable();
    for application in &module.callback_applications {
        let first = application_indices.partition_point(|application_index| {
            *application_index < application.application_index
        });
        let last = application_indices.partition_point(|application_index| {
            *application_index <= application.application_index
        });
        if last - first != 1 {
            return Err(vec![Diagnostic::error(
                "Terminal callback application has no unique checked placement",
            )]);
        }
    }
    let occurrences = callback_placements
        .iter()
        .zip(thunks)
        .enumerate()
        .filter(|(_, (placement, _))| module.callback_applications.binary_search_by_key(
            &placement.application_index, |row| row.application_index).is_ok())
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
                callback_thunk_artifact.clone(),
                placement.boundary_entry_plan.clone(),
                placement.entry_parameters.clone(),
            ))
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    Ok(occurrences)
}
