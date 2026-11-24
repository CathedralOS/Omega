//! Receive source and saved Psi publications for the ordinary native driver.
use diagnostics::Diagnostic;
use provider_realization::published_application::{
    NativeApplicationSettings, bind_published_application,
};
use std::sync::Arc;

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

pub struct SourceNativeTarget<'a> {
    pub profile: target::TargetProfile,
    pub settings: NativeApplicationSettings<'a>,
    /// Receiving execution in an isolated domain supplies only these physical classes
    /// and has no ambient foreign-call adapter.
    pub isolated_execution_grant: Option<semantic_vocabulary::TerminalAuthorityDisposition>,
    pub package_permissions: Option<declaration_scope::ProductDeclarationScope>,
    pub semantic_bindings: Arc<[provider_acceptance::AcceptedSemanticBinding]>,
    pub installation_assumptions: Vec<installation_conditions::InstallationEnvironmentAssumption>,
}

/// Receive the ordinary source publisher's target-bound program and Build choices.
pub fn prepare_source_program(
    authorization: psi_authorization::AuthorizedTerminalArtifact,
    publication: provider_demand::PublishedTargetProviders,
    exclusions: &[behavior_exclusion_requests::AuthoredBehaviorExclusion],
    publish: impl FnMut(
        semantic_vocabulary::PackageKeyIdentity,
        target::TargetProfile,
    ) -> Result<super::components::ComponentSourcePublication, Vec<Diagnostic>>,
    target: SourceNativeTarget<'_>,
    receive: impl FnOnce(&crate::NativeProgram) -> Result<(), Vec<Diagnostic>>,
) -> Result<crate::NativeProgram, Vec<Diagnostic>> {
    let packages = publication
        .independent_packages()
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let issued = source_demand(
        &authorization,
        publication,
        target.profile,
        target.package_permissions,
        target.semantic_bindings,
    )?;
    let supplied = super::components::verify(packages, target.profile, publish)?;
    let program = prepare_issued_program(
        authorization,
        issued,
        supplied,
        target.settings,
        exclusions,
        target.installation_assumptions,
        target.isolated_execution_grant,
    )?;
    receive(&program)?;
    Ok(program)
}

/// Determine the checked target supply for a package, including an entryless library.
/// Selecting providers does not require a process-entry ABI.
pub fn review_source_providers(
    authorization: psi_authorization::AuthorizedTerminalArtifact,
    publication: provider_demand::PublishedTargetProviders,
    target: SourceNativeTarget<'_>,
    components: std::collections::BTreeMap<
        semantic_vocabulary::PackageKeyIdentity,
        Arc<component_description::VerifiedComponent>,
    >,
    package: semantic_vocabulary::PackageKeyIdentity,
) -> Result<
    (
        provider_realization::SelectedProviderResult,
        Option<Vec<u8>>,
    ),
    Vec<Diagnostic>,
> {
    let issued = source_demand(
        &authorization,
        publication,
        target.profile,
        target.package_permissions,
        target.semantic_bindings,
    )?;
    let admitted = native_inputs::admit(authorization, issued)
        .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
    let selected = provider_realization::select_published_providers(admitted, components)?;
    let artifact = selected.published_authorization().shared_artifact();
    let has_exports = artifact
        .semantic_module()
        .interfaces
        .provider_candidates
        .iter()
        .any(|candidate| candidate.source_candidate_package_identity == Some(package));
    // Publishing an independent component is optional until a receiving Product selects it.
    let description = if has_exports {
        native_artifact::describe_portable_component(artifact, selected.plans()).ok()
    } else {
        None
    };
    Ok((selected.result().clone(), description))
}

fn source_demand(
    authorization: &psi_authorization::AuthorizedTerminalArtifact,
    publication: provider_demand::PublishedTargetProviders,
    profile: target::TargetProfile,
    permissions: Option<declaration_scope::ProductDeclarationScope>,
    bindings: Arc<[provider_acceptance::AcceptedSemanticBinding]>,
) -> Result<
    provider_demand::IssuedProviderDemand<terminal_codec::TerminalArtifactIdentity>,
    Vec<Diagnostic>,
> {
    let artifact = authorization.shared_artifact();
    let demand = publication.receive(artifact, profile.target_name())?;
    provider_demand::IssuedProviderDemand::from_publication(
        artifact.manifest().identity(),
        Arc::clone(artifact),
        Some(profile),
        demand,
        permissions,
        bindings,
    )
}

pub fn prepare_saved_program(
    authorization: psi_authorization::AuthorizedTerminalArtifact,
    target: SavedProgramTarget<'_>,
) -> Result<crate::NativeProgram, Vec<Diagnostic>> {
    let issued = provider_demand::IssuedProviderDemand::from_saved_program(
        authorization.shared_artifact(),
        target.profile,
        target.requested_boundaries,
        target.installation,
    )?;
    prepare_issued_program(
        authorization,
        issued,
        Default::default(),
        NativeApplicationSettings {
            application: target.application,
            optimizations: target.optimizations,
            x86_scalar_fma_provider: target.x86_scalar_fma_provider,
        },
        target.behavior_exclusions,
        Vec::new(),
        None,
    )
}

/// Admit an issued target demand against its authorized Psi, then use its selected
/// providers and application settings to prepare one native invocation.
fn prepare_issued_program<Publisher>(
    authorization: psi_authorization::AuthorizedTerminalArtifact,
    issued: provider_demand::IssuedProviderDemand<Publisher>,
    components: super::components::VerifiedSuppliers,
    settings: NativeApplicationSettings<'_>,
    behavior_exclusions: &[behavior_exclusion_requests::AuthoredBehaviorExclusion],
    installation_assumptions: Vec<installation_conditions::InstallationEnvironmentAssumption>,
    isolated_execution_grant: Option<semantic_vocabulary::TerminalAuthorityDisposition>,
) -> Result<crate::NativeProgram, Vec<Diagnostic>> {
    let admitted = native_inputs::admit(authorization, issued)
        .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
    let descriptions = components
        .iter()
        .map(|(&package, supplier)| (package, Arc::clone(&supplier.component)))
        .collect();
    let (terminal, permissions) =
        bind_published_application(admitted, settings, descriptions, behavior_exclusions)?;
    Ok(crate::NativeProgram {
        terminal,
        suppliers: components,
        permissions,
        isolated_execution_grant,
        installation_assumptions,
    })
}
