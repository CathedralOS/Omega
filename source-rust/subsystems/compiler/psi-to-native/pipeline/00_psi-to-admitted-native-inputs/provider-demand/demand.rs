//! Closed logical choices a publisher issues for one portable program.
#![forbid(unsafe_code)]

use diagnostics::Diagnostic;
use host_provider_inputs::ProviderExecutionOrigins;
use source_provider_publication::SourceProviderDemand;
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use std::sync::Arc;
use terminal_semantics::{
    callable_values::CallableBoundaryImplementation, logical_providers::ClosedLogicalProviders,
};

pub use source_provider_publication::{
    PublishedTargetProviders, SourcePackageReviewRequirements as PackageReviewRequirements,
};

pub enum ProviderDemandInputs {
    Unscoped {
        artifact: Arc<terminal_codec::CanonicalTerminalArtifact>,
        roots: Vec<MachineId>,
    },
    Target {
        profile: target::TargetProfile,
        logical: ClosedLogicalProviders,
        has_bound_applications: bool,
        bodies: Vec<CallableBoundaryImplementation>,
        labels: Vec<(BoundaryMachineId, String)>,
        origins: ProviderExecutionOrigins,
        review: Option<PackageReviewRequirements>,
    },
}

/// An issuer hands over a complete request. Taking its facts consumes the
/// request and never yields the issuer needed to authorize another set.
#[derive(Debug, PartialEq, Eq)]
pub struct ProviderDemandAuthority<Publisher>(Publisher);

impl<Publisher> ProviderDemandAuthority<Publisher> {
    pub fn publisher(&self) -> &Publisher {
        &self.0
    }
}

pub struct IssuedProviderDemand<Publisher> {
    inputs: ProviderDemandInputs,
    publisher: Arc<ProviderDemandAuthority<Publisher>>,
}

impl<Publisher> IssuedProviderDemand<Publisher> {
    pub fn issue(publisher: Publisher, inputs: ProviderDemandInputs) -> Self {
        Self {
            inputs,
            publisher: Arc::new(ProviderDemandAuthority(publisher)),
        }
    }

    /// Project the publisher's checked choices onto the corresponding portable artifact.
    pub fn from_publication(
        publisher: Publisher,
        artifact: Arc<terminal_codec::CanonicalTerminalArtifact>,
        profile: Option<target::TargetProfile>,
        demand: SourceProviderDemand,
        package_permissions: Option<declaration_scope::ProductDeclarationScope>,
        semantic_bindings: Arc<[host_provider_inputs::AcceptedSemanticBinding]>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let inputs = match (profile, demand) {
            (None, SourceProviderDemand::Unscoped { roots }) => {
                ProviderDemandInputs::Unscoped { artifact, roots }
            }
            (
                Some(profile),
                SourceProviderDemand::Target {
                    closed,
                    labels,
                    bindings,
                    review,
                },
            ) => {
                let (logical, bodies) = closed.into_parts();
                let has_bound_applications = artifact
                    .publication_metadata()
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "checked source lost its publication metadata",
                        )]
                    })?
                    .has_bound_applications();
                let origins = ProviderExecutionOrigins::from_inputs(
                    artifact,
                    profile,
                    host_provider_inputs::ProviderInstallationInputs {
                        bindings,
                        package_permissions,
                        semantic_bindings,
                    },
                );
                ProviderDemandInputs::Target {
                    profile,
                    logical,
                    bodies,
                    has_bound_applications,
                    labels,
                    origins,
                    review,
                }
            }
            _ => {
                return Err(vec![Diagnostic::error(
                    "closed provider demand has a different deployment scope",
                )]);
            }
        };
        Ok(Self::issue(publisher, inputs))
    }

    pub fn inputs(&self) -> &ProviderDemandInputs {
        &self.inputs
    }

    pub fn into_parts(
        self,
    ) -> (
        ProviderDemandInputs,
        Arc<ProviderDemandAuthority<Publisher>>,
    ) {
        (self.inputs, self.publisher)
    }
}
