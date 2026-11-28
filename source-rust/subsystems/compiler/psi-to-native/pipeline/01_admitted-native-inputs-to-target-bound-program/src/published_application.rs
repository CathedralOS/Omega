//! Prepare native execution from the host-admitted publication and its selected providers.
use provider_admission::terminal_authority_permissions::admit_package_terminal_authority_permissions;
use verified_optimization_input::settlement::boundary_application_settlement::BoundaryApplicationSettlement;
use verified_optimization_input::{
    NativeProgramEntrySettlement, RetainedTerminalArtifact, TerminalNativeProposalRequest,
};
mod program_entry;
use crate::{PublishedSelection, SelectedExecution, select_published_providers};
use diagnostics::Diagnostic;
use program_entry::PublishedNativeEntry;
use provider_admission::TerminalAuthorityPermissionPolicy;
use provider_correspondence::CompilerIntrinsicPlanProvenance;
use std::sync::Arc;

struct PublishedApplicationInputs<'a, Publisher> {
    execution: &'a PublishedSelection<Publisher, SelectedExecution>,
    package_permissions: Option<&'a declaration_scope::ProductDeclarationScope>,
    application: target::NativeApplicationMetadata,
    optimizations: &'a optimization_core::OptimizationSelections,
    behavior_exclusions: &'a [build_model::behavior_exclusions::AuthoredBehaviorExclusion],
    x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
}

fn prepare_published_application<Publisher>(
    inputs: PublishedApplicationInputs<'_, Publisher>,
) -> Result<(RetainedTerminalArtifact, TerminalAuthorityPermissionPolicy), Vec<Diagnostic>> {
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
    let selection = PublishedNativeEntry::for_program(artifact, execution)?;
    let requester = publication
        .entry_schema_custody(selection.schema_declaration())
        .map_err(refuse)?;
    let entry = selection.settle_calling_plans(requester, inputs.package_permissions)?;
    let calls = crate::execution::selected_boundary_implementations(module, realization)?
        .into_iter()
        .map(|body| (body.boundary, body.machine));
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
        .interfaces
        .callback_applications
        .iter()
        .any(|callback| active_operation(callback.registrar_machine, callback.registrar))
        || module
            .interfaces
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
                .scalars
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
    let plans = entry
        .calling_plans()
        .ok_or_else(|| refuse("executable entry lost its paired arrival plans".into()))?;
    let root = verified_optimization_input::NativeRootNomination::Program {
        receipt: &receipt,
        bind_physical_adapter: entry.physical_adapter().is_some(),
        entry: NativeProgramEntrySettlement::new(
            entry.source_signature(),
            Some((
                &plans.semantic_calling_application,
                &plans.physical_calling_application,
                &plans.storage_entry,
            )),
        ),
    };
    project_root(
        execution,
        artifact,
        root,
        roots,
        NativeApplicationSettings {
            application: inputs.application,
            optimizations: inputs.optimizations,
            x86_scalar_fma_provider: inputs.x86_scalar_fma_provider,
        },
        inputs.behavior_exclusions,
    )
}

fn add_component_calling(
    artifact: &Arc<terminal_codec::CanonicalTerminalArtifact>,
    profile: target::TargetProfile,
    boundary: semantic_vocabulary::BoundaryMachineId,
    calling: &mut Vec<calling_conventions::BoundaryCallingApplication>,
) -> Result<(), Vec<Diagnostic>> {
    let program = crate::calling_policy_plans::CallingProgram {
        artifact: Arc::clone(artifact),
    };
    let application = crate::calling_policy_plans::component_calling_application(
        &program, profile, boundary, calling,
    )
    .map_err(|reason| vec![Diagnostic::error(reason)])?;
    if !calling.contains(&application) {
        calling.push(application);
    }
    Ok(())
}

/// Bind a supplier root to its own publication, target providers and crossing ABI.
pub fn bind_component_export<Publisher>(
    execution: &PublishedSelection<Publisher, SelectedExecution>,
    selected: &psi_provider_selections::AdmittedComponentProvider,
    settings: NativeApplicationSettings<'_>,
    exclusions: &[build_model::behavior_exclusions::AuthoredBehaviorExclusion],
) -> Result<(RetainedTerminalArtifact, TerminalAuthorityPermissionPolicy), Vec<Diagnostic>> {
    let artifact = execution.published_authorization().shared_artifact();
    if selected.provider() != artifact.view().identity() {
        return Err(vec![Diagnostic::error(
            "component export belongs to another supplier publication",
        )]);
    }
    let entry = terminal_psi::TargetMachineCallable::Machine(selected.candidate());
    let root = artifact
        .semantic_module()
        .scalars
        .scalar_applications
        .proof_scope
        .callable_declaration(entry)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "component export lacks its checked declaration",
            )]
        })?;
    project_root(
        execution,
        artifact,
        verified_optimization_input::NativeRootNomination::Component(selected),
        vec![root],
        settings,
        exclusions,
    )
}

fn project_root<Publisher>(
    execution: &PublishedSelection<Publisher, SelectedExecution>,
    artifact: &Arc<terminal_codec::CanonicalTerminalArtifact>,
    root: verified_optimization_input::NativeRootNomination<'_>,
    roots: Vec<terminal_psi::ProofDeclarationId>,
    settings: NativeApplicationSettings<'_>,
    exclusions: &[build_model::behavior_exclusions::AuthoredBehaviorExclusion],
) -> Result<(RetainedTerminalArtifact, TerminalAuthorityPermissionPolicy), Vec<Diagnostic>> {
    let refuse = |reason: String| vec![Diagnostic::error(reason)];
    let realization = execution.realization();
    let declarations = realization.provider_declarations();
    let original = execution.published_authorization();
    let profile = execution
        .target_profile()
        .ok_or_else(|| refuse("native root has no deployment target".into()))?;
    // A supplier can export one boundary and call others from its body.
    // Resolve both sides against this package's own artifact.
    let imports: Vec<_> = realization
        .component_imports()
        .map(|(_, import)| import.clone())
        .collect();
    let mut calling = realization.calling_applications().to_vec();
    if let verified_optimization_input::NativeRootNomination::Component(export) = &root {
        add_component_calling(artifact, profile, export.provider_boundary(), &mut calling)?;
    }
    for import in &imports {
        for selected in import.calls() {
            add_component_calling(artifact, profile, selected.receiving_boundary(), &mut calling)?;
        }
    }
    let mut provider_proposal = execution
        .provider_proposal(artifact.view(), settings.optimizations.identity())
        .ok_or_else(|| refuse("native root lost its provider proposal".into()))?;
    provider_proposal.application_roots = Some(roots);
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
    // A plain requirement is supplied by its executed Build locator. Present
    // that checked receipt directly to native admission exactly once.
    let module = artifact.semantic_module();
    let mut native_bindings = Vec::new();
    let mut direct_boundary_leaves = Vec::new();
    for leaf in realization.direct_boundary_leaves() {
        let Some(boundary) = leaf.receiving_declaration(module).map_err(refuse)? else {
            continue;
        };
        if boundary.kind == semantic_vocabulary::BoundaryRequirementKind::Trait
            && matches!(
                &leaf.execution,
                direct_boundary_bindings::DirectBoundaryExecution::Foreign(_)
            )
        {
            native_bindings.push(
                leaf.validate_plain_execution(
                    module,
                    realization.evaluated_native_bindings(),
                    realization.evaluated_binding_identities(),
                )
                .map_err(refuse)?,
            );
        } else {
            direct_boundary_leaves.push(leaf);
        }
    }
    let native = TerminalNativeProposalRequest {
        evaluated_binding_identities: realization.evaluated_binding_identities(),
        native_bindings: &native_bindings,
        direct_boundary_leaves,
        target_profile: profile,
        target_provider: provider_proposal,
        boundaries: BoundaryApplicationSettlement {
            selected_provider_plans: realization.plans(),
            selected_provider_provenance: provenance,
            calling_applications: calling,
        },
        root,
        imports,
        application: settings.application,
        optimizations: settings.optimizations.project_post_terminal(),
        external_bindings: realization.external_bindings(),
        behavior_exclusions: exclusions,
        package_terminal_authority_permissions: realization
            .terminal_authority_permissions()
            .to_vec(),
        package_terminal_authority_permission_identities: realization
            .terminal_authority_permission_identities()
            .clone(),
        x86_scalar_fma_provider: settings.x86_scalar_fma_provider,
    };
    let authorized = if Arc::ptr_eq(original.shared_artifact(), artifact) {
        original.clone()
    } else if original.is_contained() {
        // Folding a contained program keeps it contained: the folded copy is
        // derived from Psi the host does not trust.
        psi_authorization::admit_contained_interpretation(Arc::clone(artifact))
            .map_err(|reason| refuse(reason.to_string()))?
            .into()
    } else {
        // effective_artifact checked the folded program against this publication.
        let producer = psi_authorization::HostProducerTrust::accept("in-process target folding");
        psi_authorization::authorize_terminal_artifact(Arc::clone(artifact), Some(&producer))
            .map_err(|reason| refuse(reason.to_string()))?
            .into()
    };
    let terminal = RetainedTerminalArtifact::project(authorized, native)?;
    Ok((terminal, permissions))
}

/// Target application settings used after stage 00 has bound the issued demand.
#[derive(Clone)]
pub struct NativeApplicationSettings<'a> {
    pub application: target::NativeApplicationMetadata,
    pub optimizations: &'a optimization_core::OptimizationSelections,
    pub x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
}

/// Settle one admitted program's entry, permissions and provider plans for its target.
pub fn bind_published_application<Publisher>(
    admitted: native_inputs::AdmittedProviderDemand<Publisher>,
    settings: NativeApplicationSettings<'_>,
    components: std::collections::BTreeMap<
        semantic_vocabulary::PackageKeyIdentity,
        Arc<component_description::VerifiedComponent>,
    >,
    behavior_exclusions: &[build_model::behavior_exclusions::AuthoredBehaviorExclusion],
) -> Result<(RetainedTerminalArtifact, TerminalAuthorityPermissionPolicy), Vec<Diagnostic>> {
    let refuse = |reason: String| vec![Diagnostic::error(reason)];
    let provider_demand::ProviderDemandInputs::Target { origins, .. } = admitted.inputs() else {
        return Err(refuse(
            "native application preparation requires a target-scoped provider demand".into(),
        ));
    };
    let package_permissions = origins.inputs().package_permissions.clone();
    let mut execution = select_published_providers(admitted, components)?;
    let artifact = Arc::clone(execution.published_authorization().shared_artifact());
    execution
        .bind_folded_entry(&artifact, artifact.published_evidence().map(Arc::as_ref))
        .map_err(refuse)?;
    prepare_published_application(PublishedApplicationInputs {
        execution: &execution,
        package_permissions: package_permissions.as_ref(),
        application: settings.application,
        optimizations: settings.optimizations,
        behavior_exclusions,
        x86_scalar_fma_provider: settings.x86_scalar_fma_provider,
    })
}
