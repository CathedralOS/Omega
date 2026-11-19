//! Prepare native execution from the host-admitted publication and its selected providers.
use super::application::{TerminalApplication, admit_package_permissions};
use super::program_entry::PublishedNativeEntry;
use artifact_lowering::{
    BoundaryApplicationSettlement, NativeProgramEntrySettlement, TerminalNativeProposalRequest,
};
use diagnostics::Diagnostic;
use provider_admission::provider_admission::{
    CompilerIntrinsicPlanProvenance, TerminalAuthorityPermissionPolicy,
};
use provider_realization::{
    PublishedSelection, SelectedProviderResult, select_published_providers,
};
use std::sync::Arc;

struct PublishedApplicationInputs<'a, Publisher> {
    artifact: &'a Arc<terminal_codec::CanonicalTerminalArtifact>,
    execution: &'a PublishedSelection<Publisher, SelectedProviderResult>,
    proposal: Option<backend_plan::TerminalTargetProviderProposal>,
    bindings: &'a [effects::semantic_bindings::AcceptedSemanticBinding],
    package_permissions:
        Option<&'a declaration_scope::ProductDeclarationScope>,
    application: target::NativeApplicationMetadata,
    optimizations: &'a optimization_core::OptimizationSelections,
    behavior_exclusions: &'a [effects::AuthoredBehaviorExclusion],
    x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
}

fn prepare_published_application<Publisher>(
    inputs: PublishedApplicationInputs<'_, Publisher>,
) -> Result<
    (
        artifact_lowering::RetainedTerminalArtifact,
        TerminalAuthorityPermissionPolicy,
    ),
    Vec<Diagnostic>,
> {
    let refuse = |reason: String| vec![Diagnostic::error(reason)];
    let artifact = inputs.artifact;
    let publication = artifact.publication_metadata().ok_or_else(|| {
        refuse("saved Psi lacks entry declaration owners; recompile it to publish them".into())
    })?;
    let module = artifact.semantic_module();
    let execution = inputs.execution;
    let realization = execution.realization();
    let profile = execution
        .target_profile()
        .unwrap_or_else(target::TargetProfile::host);
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
    let closure = terminal_installation::SynchronousCallGraph::new(module, None)
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
    let custody = terminal_installation::application::TerminalApplicationCustody::prepare(
        entry.nominal_root(),
        Some(entry.source_signature().identity().bytes()),
        Some(
            terminal_installation::application::TerminalCompositionRoots {
                callbacks: None,
                physical_adapter: entry.physical_adapter(),
            },
        ),
    );
    let application = custody.attach(
        artifact.view().into(),
        Ok(artifact),
        publication.machine_provenance(),
        &mut terminal_installation::application::UnmeasuredEntryReceipt,
    )?;
    let receipt = application
        .receipt?
        .ok_or_else(|| refuse("executable application has no checked entry receipt".into()))?;
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
    let mut provider_proposal = inputs.proposal.ok_or_else(|| {
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
    let permissions = admit_package_permissions(
        realization.terminal_authority_permissions().to_vec(),
        realization
            .terminal_authority_permission_identities()
            .clone(),
    )?;
    let native = TerminalNativeProposalRequest {
        evaluated_binding_identities: realization.evaluated_binding_identities(),
        native_bindings: &[],
        direct_boundary_leaves: realization.direct_boundary_leaves().iter().collect(),
        target_profile: Some(profile),
        boundaries: BoundaryApplicationSettlement {
            selected_provider_plans: realization.plans(),
            selected_provider_provenance: provenance,
            calling_applications: realization.calling_applications().to_vec(),
        },
        native_target: Some(profile.native_target()),
        program_entry: Some(
            NativeProgramEntrySettlement::new(
                entry.source_signature(),
                Some((
                    &plans.semantic_calling_application,
                    &plans.physical_calling_application,
                    &plans.storage_entry,
                )),
            )
            .with_checked_entry(&receipt),
        ),
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
    let proposals = vec![provider_proposal];
    let physical_entry_adapter = entry.physical_adapter().is_some();
    let original = execution.published_authorization();
    let authorized = if Arc::ptr_eq(original.shared_artifact(), artifact) {
        original.clone()
    } else {
        // Target folding produces a new artifact. Only its actual result may receive
        // the host's authorization for this locally run transformation.
        let folded = execution.folded_program().ok_or_else(|| {
            refuse("native execution received an artifact outside its admitted selection".into())
        })?;
        if !Arc::ptr_eq(folded.artifact(), artifact) {
            return Err(refuse(
                "native execution received a different folded artifact".into(),
            ));
        }
        let producer =
            terminal_installation::HostProducerTrust::accept("in-process target folding");
        terminal_installation::authorize_terminal_artifact(Arc::clone(artifact), Some(&producer))
            .map_err(|reason| refuse(reason.to_string()))?
    };
    let terminal = TerminalApplication {
        artifact: authorized,
        receipt: receipt.clone(),
        provider_proposals: proposals,
        physical_entry_adapter,
        evidence: artifact.published_evidence().cloned(),
    }
    .prepare(native)?;
    Ok((terminal, permissions))
}

fn checked_provider_calls(
    module: &terminal_psi::TerminalModule,
    declarations: &[effects::provider_plan::ProviderPlan],
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
                effects::provider_plan::ProviderBinding::CheckedAdapter { .. }
            )
        }) {
            continue;
        }
        for row in
            backend_plan::selected_provider_candidate_rows(declaration, module).map_err(refuse)?
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
    pub installation: backend_plan::TargetProviderInstallation,
    pub application: target::NativeApplicationMetadata,
    pub optimizations: &'a optimization_core::OptimizationSelections,
    pub behavior_exclusions: &'a [effects::AuthoredBehaviorExclusion],
    pub x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
}

/// Use the same provider, entry and native application stages as a source build.
pub(crate) fn prepare_saved_program(
    authorization: terminal_installation::AuthorizedTerminalArtifact,
    mut target: SavedProgramTarget<'_>,
) -> Result<
    (
        artifact_lowering::RetainedTerminalArtifact,
        TerminalAuthorityPermissionPolicy,
    ),
    Vec<Diagnostic>,
> {
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
    let (logical, bodies) = terminal_installation::logical_reach::close_providers(
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
    let folded_entry = backend_plan::FoldedEntryReach::capture(
        artifact.view(),
        name,
        &bodies,
        metadata.has_bound_applications(),
    );
    let package_permissions = origins.inputs().package_permissions.clone();
    let entry_bindings = Arc::clone(&origins.inputs().semantic_bindings);
    let facts = backend_plan::ProviderDemandInputs::Target {
        profile: target.profile,
        logical,
        bodies,
        folded_entry,
        labels,
        origins,
        review: None,
    };
    let issued = backend_plan::IssuedProviderDemand::issue((), facts);
    let admitted = backend_plan::AdmittedProviderDemand::new(issued, authorization)
        .map_err(|error| refuse(error.to_string()))?;
    let mut execution = select_published_providers(admitted)?;
    execution
        .bind_folded_entry(&artifact, artifact.published_evidence().map(Arc::as_ref))
        .map_err(refuse)?;
    let effective = Arc::clone(execution.effective_artifact(&artifact).map_err(refuse)?);
    let proposal = execution.provider_proposal(effective.view(), target.optimizations.identity());
    let execution = execution.into_result();
    prepare_published_application(PublishedApplicationInputs {
        artifact: &effective,
        execution: &execution,
        proposal,
        bindings: &entry_bindings,
        package_permissions: package_permissions.as_ref(),
        application: target.application,
        optimizations: target.optimizations,
        behavior_exclusions: target.behavior_exclusions,
        x86_scalar_fma_provider: target.x86_scalar_fma_provider,
    })
}
