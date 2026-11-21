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
use provider_admission::{CompilerIntrinsicPlanProvenance, TerminalAuthorityPermissionPolicy};
use std::sync::Arc;

struct PublishedApplicationInputs<'a, Publisher> {
    execution: &'a PublishedSelection<Publisher, SelectedExecution>,
    bindings: &'a [provider_acceptance::AcceptedSemanticBinding],
    package_permissions: Option<&'a declaration_scope::ProductDeclarationScope>,
    application: target::NativeApplicationMetadata,
    optimizations: &'a optimization_core::OptimizationSelections,
    behavior_exclusions: &'a [behavior_exclusion_requests::AuthoredBehaviorExclusion],
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
    let calls = crate::execution::selected_boundary_implementations(module, declarations)?
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
        target_provider: provider_proposal,
        bind_physical_adapter: entry.physical_adapter().is_some(),
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
    let authorized = if Arc::ptr_eq(original.shared_artifact(), artifact) {
        original.clone()
    } else {
        // effective_artifact checked the folded program against this publication.
        let producer = psi_authorization::HostProducerTrust::accept("in-process target folding");
        psi_authorization::authorize_terminal_artifact(Arc::clone(artifact), Some(&producer))
            .map_err(|reason| refuse(reason.to_string()))?
    };
    let terminal = RetainedTerminalArtifact::project(authorized, &receipt, native)?;
    Ok((terminal, permissions))
}

/// Target application settings used after stage 00 has bound the issued demand.
pub struct NativeApplicationSettings<'a> {
    pub application: target::NativeApplicationMetadata,
    pub optimizations: &'a optimization_core::OptimizationSelections,
    pub x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
}

/// Settle one admitted program's entry, permissions and provider plans for its target.
pub fn bind_published_application<Publisher>(
    admitted: native_inputs::AdmittedProviderDemand<Publisher>,
    settings: NativeApplicationSettings<'_>,
    behavior_exclusions: &[behavior_exclusion_requests::AuthoredBehaviorExclusion],
) -> Result<(RetainedTerminalArtifact, TerminalAuthorityPermissionPolicy), Vec<Diagnostic>> {
    let refuse = |reason: String| vec![Diagnostic::error(reason)];
    let provider_demand::ProviderDemandInputs::Target { origins, .. } = admitted.inputs() else {
        return Err(refuse(
            "native application preparation requires a target-scoped provider demand".into(),
        ));
    };
    let package_permissions = origins.inputs().package_permissions.clone();
    let entry_bindings = Arc::clone(&origins.inputs().semantic_bindings);
    let mut execution = select_published_providers(admitted)?;
    let artifact = Arc::clone(execution.published_authorization().shared_artifact());
    execution
        .bind_folded_entry(&artifact, artifact.published_evidence().map(Arc::as_ref))
        .map_err(refuse)?;
    prepare_published_application(PublishedApplicationInputs {
        execution: &execution,
        bindings: &entry_bindings,
        package_permissions: package_permissions.as_ref(),
        application: settings.application,
        optimizations: settings.optimizations,
        behavior_exclusions,
        x86_scalar_fma_provider: settings.x86_scalar_fma_provider,
    })
}
