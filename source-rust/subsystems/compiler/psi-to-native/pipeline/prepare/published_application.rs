//! Prepare native execution from the host-admitted publication and its selected providers.
use verified_optimization_input::{NativeProgramEntrySettlement, RetainedTerminalArtifact, TerminalNativeProposalRequest, TerminalNativeRealizationProposal};
use verified_optimization_input::settlement::boundary_application_settlement::BoundaryApplicationSettlement;
use verified_optimization_input::native_proposal::project_terminal_native_realization_proposal;
use provider_admission::terminal_authority_permissions::admit_package_terminal_authority_permissions;
use super::program_entry::PublishedNativeEntry;
use diagnostics::Diagnostic;
use provider_admission::{
    CompilerIntrinsicPlanProvenance, TerminalAuthorityPermissionPolicy,
};
use provider_realization::{
    PublishedSelection, SelectedExecution, select_published_providers,
};
use std::sync::Arc;

struct PublishedApplicationInputs<'a, Publisher> {
    execution: &'a PublishedSelection<Publisher, SelectedExecution>,
    bindings: &'a [provider_acceptance::AcceptedSemanticBinding],
    package_permissions:
        Option<&'a declaration_scope::ProductDeclarationScope>,
    application: target::NativeApplicationMetadata,
    optimizations: &'a optimization_core::OptimizationSelections,
    behavior_exclusions: &'a [behavior_exclusion_requests::AuthoredBehaviorExclusion],
    x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
}

fn prepare_published_application<Publisher>(
    inputs: PublishedApplicationInputs<'_, Publisher>,
) -> Result<
    (
        RetainedTerminalArtifact,
        TerminalAuthorityPermissionPolicy,
    ),
    Vec<Diagnostic>,
> {
    let refuse = |reason: String| vec![Diagnostic::error(reason)];
    let execution = inputs.execution;
    let original = execution.published_authorization();
    let artifact = execution
        .effective_artifact(original.shared_artifact())
        .map_err(refuse)?;
    let publication = artifact.publication_metadata().ok_or_else(|| {
        refuse("saved Psi lacks entry declaration owners; recompile it to publish them".into())
    })?;
    let module = artifact.semantic_module();
    let realization = execution.realization();
    let profile = execution
        .target_profile()
        .ok_or_else(|| refuse("saved Psi native selection has no deployment target".into()))?;
    let declarations = realization.provider_declarations();
    let selection = PublishedNativeEntry::for_program(artifact, execution)?;
    let requester = publication
        .entry_schema_custody(selection.schema_declaration())
        .map_err(refuse)?;
    let binding = selection.binding_role().and_then(|role| {
        inputs
            .bindings
            .iter()
            .find(|binding| binding.role() == role)
    });
    let entry = selection.settle_calling_plans(requester, binding, inputs.package_permissions)?;
    let calls = checked_provider_calls(module, declarations)?;
    let root = module
        .program_entry_contract(profile.target_name())
        .map_err(refuse)?
        .entry;
    let closure = terminal_semantics::synchronous_calls::SynchronousCallGraph::new(module, None)
        .and_then(|graph| graph.with_boundary_calls(calls))
        .and_then(|graph| graph.from_root(root))
        .map_err(refuse)?;
    let active_operation = |owner, identity| {
        closure
            .machines
            .iter()
            .filter(|machine| machine.id == owner)
            .any(|machine| {
                machine
                    .operations()
                    .any(|operation| operation.id == identity)
            })
    };
    if module
        .callback_applications
        .iter()
        .any(|callback| active_operation(callback.registrar_machine, callback.registrar))
        || module
            .installed_interrupt_registrations
            .iter()
            .any(|registration| active_operation(registration.machine, registration.operation))
    {
        return Err(refuse(
            "executable callback and interrupt entries need their canonical composition roots before application preparation".into(),
        ));
    }
    let custody = terminal_codec::entry_receipt::application::TerminalApplicationCustody::prepare(
        entry.nominal_root(),
        Some(entry.source_signature().identity().bytes()),
        Some(
            terminal_codec::entry_receipt::application::TerminalCompositionRoots {
                callbacks: None,
                physical_adapter: entry.physical_adapter(),
            },
        ),
    );
    let application = custody.attach(
        artifact.view().into(),
        Ok(artifact),
        publication.machine_provenance(),
    )?;
    let receipt = application
        .entry_receipt
        .ok_or_else(|| refuse("executable application has no checked entry receipt".into()))?
        .check()?;
    let mut roots = Vec::with_capacity(application.roots.len());
    for root in application.roots {
        let root = root?;
        roots.push(
            module
                .scalar_applications
                .proof_scope
                .callable_declaration(terminal_psi::TargetMachineCallable::Machine(root.machine))
                .ok_or_else(|| {
                    refuse("executable root has no actual checked declaration".into())
                })?,
        );
    }
    roots.sort_unstable();
    roots.dedup();
    let mut provider_proposal = execution
        .provider_proposal(artifact.view(), inputs.optimizations.identity())
        .ok_or_else(|| {
            refuse("executable application requires an explicit deployment target".into())
        })?;
    provider_proposal.application_roots = Some(roots);
    let plans = entry
        .calling_plans()
        .ok_or_else(|| refuse("executable entry lost its paired arrival plans".into()))?;
    let provenance = declarations
        .iter()
        .zip(realization.row_executions())
        .map(|(declaration, executions)| {
            let schema = realization
                .direct_boundary_leaves()
                .iter()
                .find(|leaf| leaf.plan == *declaration)
                .map_or(&declaration.schema, |leaf| &leaf.satisfied_schema);
            CompilerIntrinsicPlanProvenance {
                plan: declaration,
                semantic_schema: schema,
                row_executions: executions,
            }
        })
        .collect();
    let permissions = admit_package_terminal_authority_permissions(
        realization.terminal_authority_permissions().to_vec(),
        realization
            .terminal_authority_permission_identities()
            .clone(),
        "checked package terminal-authority permissions are invalid",
    )?;
    let native = TerminalNativeProposalRequest {
        evaluated_binding_identities: realization.evaluated_binding_identities(),
        native_bindings: &[],
        direct_boundary_leaves: realization.direct_boundary_leaves().iter().collect(),
        target_profile: profile,
        boundaries: BoundaryApplicationSettlement {
            selected_provider_plans: realization.plans(),
            selected_provider_provenance: provenance,
            calling_applications: realization.calling_applications().to_vec(),
        },
        program_entry: NativeProgramEntrySettlement::new(
            entry.source_signature(),
            Some((
                &plans.semantic_calling_application,
                &plans.physical_calling_application,
                &plans.storage_entry,
            )),
        )
        .with_checked_entry(&receipt),
        application: inputs.application,
        optimizations: inputs.optimizations.project_post_terminal(),
        external_bindings: realization.external_bindings(),
        behavior_exclusions: inputs.behavior_exclusions,
        package_terminal_authority_permissions: realization
            .terminal_authority_permissions()
            .to_vec(),
        package_terminal_authority_permission_identities: realization
            .terminal_authority_permission_identities()
            .clone(),
        x86_scalar_fma_provider: inputs.x86_scalar_fma_provider,
    };
    let authorized = if Arc::ptr_eq(original.shared_artifact(), artifact) {
        original.clone()
    } else {
        // effective_artifact checked the folded program against this publication.
        let producer =
            psi_authorization::HostProducerTrust::accept("in-process target folding");
        psi_authorization::authorize_terminal_artifact(Arc::clone(artifact), Some(&producer))
            .map_err(|reason| refuse(reason.to_string()))?
    };
    let mut proposal = project_terminal_native_realization_proposal(
        authorized.shared_artifact(),
        receipt.clone(),
        native,
    )?;
    if entry.physical_adapter().is_some() {
        proposal
            .bind_physical_adapter(authorized.artifact())
            .map_err(|message| vec![Diagnostic::error(message)])?;
    }
    let terminal = RetainedTerminalArtifact::new_with_native_realization_proposal(
        authorized,
        proposal.with_target_provider_proposals(vec![provider_proposal]),
    )
    .with_evidence(artifact.published_evidence().cloned())?;
    Ok((terminal, permissions))
}

fn checked_provider_calls(
    module: &terminal_psi::TerminalModule,
    declarations: &[provider_plans::ProviderPlan],
) -> Result<
    Vec<(
        semantic_vocabulary::BoundaryMachineId,
        semantic_vocabulary::MachineId,
    )>,
    Vec<Diagnostic>,
> {
    let refuse = |reason: String| vec![Diagnostic::error(reason)];
    let mut calls = Vec::new();
    for declaration in declarations {
        if !declaration.rows.iter().any(|row| {
            matches!(
                row.binding,
                provider_plans::ProviderBinding::CheckedAdapter { .. }
            )
        }) {
            continue;
        }
        for row in
            target_provider_admission::selected_provider_candidate_rows(declaration, module).map_err(refuse)?
        {
            for position in row.candidate_positions {
                let candidate = &module.provider_candidates[position];
                calls.push((candidate.boundary, candidate.candidate));
            }
        }
    }
    calls.sort_unstable();
    calls.dedup();
    Ok(calls)
}

/// Target, permissions and installations explicitly supplied by the receiving host.
pub struct SavedProgramTarget<'a> {
    pub profile: target::TargetProfile,
    pub requested_boundaries: Vec<semantic_vocabulary::BoundaryMachineId>,
    pub installation: host_provider_inputs::TargetProviderInstallation,
    pub application: target::NativeApplicationMetadata,
    pub optimizations: &'a optimization_core::OptimizationSelections,
    pub behavior_exclusions: &'a [behavior_exclusion_requests::AuthoredBehaviorExclusion],
    pub x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
}

/// Target application settings used after stage 00 has bound the issued demand.
pub struct NativeApplicationSettings<'a> {
    pub application: target::NativeApplicationMetadata,
    pub optimizations: &'a optimization_core::OptimizationSelections,
    pub behavior_exclusions: &'a [behavior_exclusion_requests::AuthoredBehaviorExclusion],
    pub x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
}

pub(crate) fn prepare_saved_program(
    authorization: psi_authorization::AuthorizedTerminalArtifact,
    mut target: SavedProgramTarget<'_>,
) -> Result<crate::NativeProgram, Vec<Diagnostic>> {
    let refuse = |reason: String| vec![Diagnostic::error(reason)];
    let artifact = Arc::clone(authorization.shared_artifact());
    let origins = target
        .installation
        .attach(Arc::clone(&artifact), target.profile)
        .map_err(|reason| refuse(reason.into()))?;
    let metadata = artifact.publication_metadata().ok_or_else(|| {
        refuse("saved Psi lacks publication metadata; recompile it to publish entry and calling declarations".into())
    })?;
    let module = artifact.semantic_module();
    let name = target.profile.target_name();
    let entry = module.program_entry_contract(name).map_err(refuse)?.entry;
    target.requested_boundaries.sort_unstable();
    target.requested_boundaries.dedup();
    let (logical, bodies) = psi_provider_selections::logical_reach::close_providers(
        module,
        name,
        Some(entry),
        &target.requested_boundaries,
    )?
    .into_parts();
    let labels = logical
        .requirements()
        .iter()
        .filter_map(|id| {
            let boundary = module.boundary_machine(*id)?;
            Some((*id, module.identities.text(boundary.identity)?.to_owned()))
        })
        .collect();
    let facts = provider_demand::ProviderDemandInputs::Target {
        profile: target.profile,
        logical,
        bodies,
        has_bound_applications: metadata.has_bound_applications(),
        labels,
        origins,
        review: None,
    };
    let issued = provider_demand::IssuedProviderDemand::issue((), facts);
    prepare_issued_program(
        authorization,
        issued,
        NativeApplicationSettings {
            application: target.application,
            optimizations: target.optimizations,
            behavior_exclusions: target.behavior_exclusions,
            x86_scalar_fma_provider: target.x86_scalar_fma_provider,
        },
        Vec::new(),
    )
}

/// Admit an issued target demand against its authorized Psi, then use its selected
/// providers and application settings to prepare one native invocation.
pub fn prepare_issued_program<Publisher>(
    authorization: psi_authorization::AuthorizedTerminalArtifact,
    issued: provider_demand::IssuedProviderDemand<Publisher>,
    settings: NativeApplicationSettings<'_>,
    installation_assumptions: Vec<installation_conditions::InstallationEnvironmentAssumption>,
) -> Result<crate::NativeProgram, Vec<Diagnostic>> {
    let refuse = |reason: String| vec![Diagnostic::error(reason)];
    let provider_demand::ProviderDemandInputs::Target {
        profile, origins, ..
    } = issued.inputs()
    else {
        return Err(refuse(
            "native application preparation requires a target-scoped provider demand".into(),
        ));
    };
    let profile = *profile;
    let package_permissions = origins.inputs().package_permissions.clone();
    let entry_bindings = Arc::clone(&origins.inputs().semantic_bindings);
    let artifact = Arc::clone(authorization.shared_artifact());
    let admitted =
        native_inputs::admit(authorization, issued).map_err(|error| refuse(error.to_string()))?;
    let mut execution = select_published_providers(admitted)?;
    execution
        .bind_folded_entry(&artifact, artifact.published_evidence().map(Arc::as_ref))
        .map_err(refuse)?;
    let subsystem = settings.application.subsystem;
    let (terminal, permissions) = prepare_published_application(PublishedApplicationInputs {
        execution: &execution,
        bindings: &entry_bindings,
        package_permissions: package_permissions.as_ref(),
        application: settings.application,
        optimizations: settings.optimizations,
        behavior_exclusions: settings.behavior_exclusions,
        x86_scalar_fma_provider: settings.x86_scalar_fma_provider,
    })?;
    Ok(crate::NativeProgram {
        terminal,
        permissions,
        subsystem,
        installation_assumptions,
        profile,
    })
}

/// Complete exactly the scalar applications required by the authorized native entry.
pub fn complete_entry_scalars(
    authorization: &psi_authorization::AuthorizedTerminalArtifact,
    proposal: &TerminalNativeRealizationProposal,
    accounting: &mut impl Extend<native_scalar_inputs::EvaluationReceipt>,
) -> Result<Option<psi_scalar_applications::AdmittedScalarApplications>, Vec<Diagnostic>> {
    let Some(demand) = proposal.entry_scalar_demand(authorization)? else {
        return Ok(None);
    };
    native_scalar_inputs::complete(
        authorization.shared_artifact(),
        proposal.target_profile().target_name(),
        &demand,
        proposal.target_provider_proposals(),
        accounting,
    )
    .map(Some)
    .map_err(|reason| vec![Diagnostic::error(format!("constant checking: {reason}"))])
}
