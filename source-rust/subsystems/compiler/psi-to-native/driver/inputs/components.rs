//! Verify the terminal closure of each separately produced package under the receiving target.
use component_description::{
    AcceptedComponentAssumptions, ComponentVerificationRequest, VerifiedComponent,
};
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use std::collections::BTreeMap;
use std::sync::Arc;

/// One dependency's own Build and target-specific source publication.
pub struct ComponentSourcePublication {
    pub package: semantic_vocabulary::PackageKeyIdentity,
    pub authorization: psi_authorization::AuthorizedTerminalArtifact,
    pub providers: provider_demand::PublishedTargetProviders,
    pub exclusions: Vec<behavior_exclusion_requests::AuthoredBehaviorExclusion>,
    pub package_permissions: Option<declaration_scope::ProductDeclarationScope>,
    pub optimizations: optimization_core::OptimizationSelections,
    pub x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
    pub semantic_bindings: Arc<[provider_acceptance::AcceptedSemanticBinding]>,
}

pub(crate) type VerifiedSuppliers = BTreeMap<PackageKeyIdentity, Supplier>;

pub(crate) struct Supplier {
    pub component: Arc<VerifiedComponent>,
    pub selection: provider_realization::PublishedSelection<
        terminal_codec::TerminalArtifactIdentity,
        provider_realization::SelectedExecution,
    >,
    pub exclusions: Vec<behavior_exclusion_requests::AuthoredBehaviorExclusion>,
    pub optimizations: optimization_core::OptimizationSelections,
    pub x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
}

pub(super) fn verify(
    components: Vec<ComponentSourcePublication>,
    profile: target::TargetProfile,
) -> Result<VerifiedSuppliers, Vec<Diagnostic>> {
    let refuse = |reason: String| vec![Diagnostic::error(reason)];
    let mut verified = BTreeMap::new();
    for component in components {
        let artifact = Arc::clone(component.authorization.shared_artifact());
        let publication = component
            .providers
            .receive(&artifact, profile.target_name())?;
        let issued = provider_demand::IssuedProviderDemand::from_publication(
            artifact.manifest().identity(),
            Arc::clone(&artifact),
            Some(profile),
            publication,
            component.package_permissions,
            component.semantic_bindings,
        )?;
        let admitted = native_inputs::admit(component.authorization, issued)
            .map_err(|why| refuse(why.to_string()))?;
        let selection =
            provider_realization::select_published_providers(admitted, Default::default())?;
        let description =
            native_artifact::describe_portable_component(&artifact, selection.plans())
                .map_err(|why| refuse(why.to_string()))?;
        // A fresh source compilation authenticates the subject. Unreviewed environmental
        // assumptions cannot be accepted merely because its publisher names them.
        let checked = component_description::verify_component(
            &description,
            &ComponentVerificationRequest {
                expected_subject: artifact.view().identity(),
                accepted_schemas: [component_description::COMPONENT_DESCRIPTION_SCHEMA_V3]
                    .into_iter()
                    .collect(),
                accepted_assumptions: AcceptedComponentAssumptions::Exact(Default::default()),
            },
        )
        .map_err(|why| refuse(why.to_string()))?;
        if verified
            .insert(
                component.package,
                Supplier {
                    component: Arc::new(checked),
                    selection,
                    exclusions: component.exclusions,
                    optimizations: component.optimizations,
                    x86_scalar_fma_provider: component.x86_scalar_fma_provider,
                },
            )
            .is_some()
        {
            return Err(refuse(
                "duplicate independently compiled supplier package".into(),
            ));
        }
    }
    Ok(verified)
}
