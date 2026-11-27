//! Verify the terminal closure of each separately produced package under the receiving target.
use component_description::{
    AcceptedComponentAssumptions, ComponentVerificationRequest, VerifiedComponent,
};
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// One dependency's own Build and target-specific source publication.
pub struct ComponentSourcePublication {
    pub package: semantic_vocabulary::PackageKeyIdentity,
    pub authorization: psi_authorization::AuthorizedTerminalArtifact,
    pub providers: provider_demand::PublishedTargetProviders,
    pub exclusions: Vec<build_model::behavior_exclusions::AuthoredBehaviorExclusion>,
    pub package_permissions: Option<declaration_scope::ProductDeclarationScope>,
    pub optimizations: optimization_core::OptimizationSelections,
    pub x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
    pub semantic_bindings: Arc<[build_providers::acceptance::AcceptedSemanticBinding]>,
}

pub(crate) type VerifiedSuppliers = BTreeMap<PackageKeyIdentity, Supplier>;

pub(crate) struct Supplier {
    pub component: Arc<VerifiedComponent>,
    pub selection: provider_realization::PublishedSelection<
        terminal_codec::TerminalArtifactIdentity,
        provider_realization::SelectedExecution,
    >,
    pub exclusions: Vec<build_model::behavior_exclusions::AuthoredBehaviorExclusion>,
    pub optimizations: optimization_core::OptimizationSelections,
    pub x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
}

/// Verify each provider under the descriptions its own Build selected. A diamond
/// shares the finished package, and a back edge rejects the selection before native lowering.
pub(super) fn verify(
    roots: BTreeSet<PackageKeyIdentity>,
    profile: target::TargetProfile,
    mut publish: impl FnMut(
        PackageKeyIdentity,
        target::TargetProfile,
    ) -> Result<ComponentSourcePublication, Vec<Diagnostic>>,
) -> Result<VerifiedSuppliers, Vec<Diagnostic>> {
    enum Visit {
        Open(PackageKeyIdentity),
        Finish(
            Box<ComponentSourcePublication>,
            BTreeSet<PackageKeyIdentity>,
        ),
    }
    let mut pending: Vec<_> = roots.into_iter().map(Visit::Open).collect();
    let mut open = BTreeSet::new();
    let mut verified = VerifiedSuppliers::new();
    while let Some(visit) = pending.pop() {
        match visit {
            Visit::Open(package) if verified.contains_key(&package) => continue,
            Visit::Open(package) => {
                if !open.insert(package) {
                    return Err(vec![Diagnostic::error(
                        "cyclic independent supplier selection",
                    )]);
                }
                let component = publish(package, profile)?;
                if component.package != package {
                    return Err(vec![Diagnostic::error(
                        "independent supplier has a different package owner",
                    )]);
                }
                let dependencies = component
                    .providers
                    .independent_packages()
                    .map_err(|reason| vec![Diagnostic::error(reason)])?;
                let children: Vec<_> = dependencies.iter().copied().map(Visit::Open).collect();
                pending.push(Visit::Finish(Box::new(component), dependencies));
                pending.extend(children);
            }
            Visit::Finish(component, dependencies) => {
                let package = component.package;
                let descriptions = dependencies
                    .into_iter()
                    .map(|dependency| {
                        verified
                            .get(&dependency)
                            .map(|supplier| (dependency, Arc::clone(&supplier.component)))
                            .ok_or_else(|| {
                                vec![Diagnostic::error(
                                    "independent supplier dependency was not verified",
                                )]
                            })
                    })
                    .collect::<Result<_, _>>()?;
                verified.insert(
                    package,
                    verify_publication(*component, profile, descriptions)?,
                );
                open.remove(&package);
            }
        }
    }
    Ok(verified)
}

fn verify_publication(
    component: ComponentSourcePublication,
    profile: target::TargetProfile,
    descriptions: BTreeMap<PackageKeyIdentity, Arc<VerifiedComponent>>,
) -> Result<Supplier, Vec<Diagnostic>> {
    let refuse = |reason: String| vec![Diagnostic::error(reason)];
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
    let selection = provider_realization::select_published_providers(admitted, descriptions)?;
    let description = native_artifact::describe_portable_component(&artifact, selection.plans())
        .map_err(|why| refuse(why.to_string()))?;
    // Verify the supplier under its own authenticated subject. The receiving program must
    // compare every environmental premise with its own acceptance before release.
    let checked = component_description::verify_component(
        &description,
        &ComponentVerificationRequest {
            expected_subject: artifact.view().identity(),
            accepted_schemas: [component_description::COMPONENT_DESCRIPTION_SCHEMA_V3]
                .into_iter()
                .collect(),
            accepted_assumptions: AcceptedComponentAssumptions::PendingConsumerAcceptance,
        },
    )
    .and_then(|checked| checked.with_executable_artifact(&artifact))
    .map_err(|why| refuse(why.to_string()))?;
    Ok(Supplier {
        component: Arc::new(checked),
        selection,
        exclusions: component.exclusions,
        optimizations: component.optimizations,
        x86_scalar_fma_provider: component.x86_scalar_fma_provider,
    })
}
