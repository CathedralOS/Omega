//! The portable result of resolving source coordinates and package permissions.
//! Native planning receives the canonical artifact and closed logical choices.
use semantic_vocabulary::{
    BoundaryMachineId, MachineId, compiler_evaluation::ProductDeclarationScope,
};
use std::sync::Arc;
use terminal_semantics::{
    callable_values::CallableBoundaryImplementation, logical_providers::ClosedLogicalProviders,
};

pub type NativeBindingMaterializationResult =
    Result<evaluation_receipts::NativeBindingMaterialization, String>;

pub struct ProviderExecutionOrigins {
    artifact: Arc<terminal_codec::CanonicalTerminalArtifact>,
    installation: TargetProviderInstallation,
}

/// Completed Build bindings name their full callable, including static arguments.
/// Package grants are supplied independently by the receiving host.
#[derive(Clone, Default)]
pub struct ProviderInstallationInputs {
    pub bindings: Vec<NativeBindingMaterializationResult>,
    pub package_permissions: Option<ProductDeclarationScope>,
    pub semantic_bindings: Arc<[effects::semantic_bindings::AcceptedSemanticBinding]>,
}

/// A receiving host can use its reviewed packages for several programs. The
/// package installer owns these bindings and grants before any program arrives.
#[derive(Clone)]
pub struct InstalledPackageProviders {
    profile: target::TargetProfile,
    bindings: Vec<evaluation_receipts::NativeBindingMaterialization>,
    package_permissions: ProductDeclarationScope,
    semantic_bindings: Arc<[effects::semantic_bindings::AcceptedSemanticBinding]>,
}

impl InstalledPackageProviders {
    pub fn new(
        profile: target::TargetProfile,
        bindings: Vec<evaluation_receipts::NativeBindingMaterialization>,
        package_permissions: ProductDeclarationScope,
        semantic_bindings: Arc<[effects::semantic_bindings::AcceptedSemanticBinding]>,
    ) -> Self {
        Self {
            profile,
            bindings,
            package_permissions,
            semantic_bindings,
        }
    }

    pub fn for_program(
        self,
        authorization: &terminal_installation::AuthorizedTerminalArtifact,
    ) -> TargetProviderInstallation {
        TargetProviderInstallation::new(
            authorization.identity(),
            self.profile,
            ProviderInstallationInputs {
                bindings: self.bindings.into_iter().map(Ok).collect(),
                package_permissions: Some(self.package_permissions),
                semantic_bindings: self.semantic_bindings,
            },
        )
    }
}

/// This installation may outlive the producing compiler and its in-memory
/// artifact. A receiver checks the manifest and target before using its grants.
#[derive(Clone)]
pub struct TargetProviderInstallation {
    artifact: terminal_codec::TerminalArtifactIdentity,
    profile: target::TargetProfile,
    inputs: ProviderInstallationInputs,
}

impl TargetProviderInstallation {
    pub fn new(
        artifact: terminal_codec::TerminalArtifactIdentity,
        profile: target::TargetProfile,
        inputs: ProviderInstallationInputs,
    ) -> Self {
        Self {
            artifact,
            profile,
            inputs,
        }
    }

    pub fn attach(
        self,
        artifact: Arc<terminal_codec::CanonicalTerminalArtifact>,
        profile: target::TargetProfile,
    ) -> Result<ProviderExecutionOrigins, &'static str> {
        if self.artifact != artifact.manifest().identity() {
            return Err("provider installation belongs to a different Psi artifact");
        }
        if self.profile != profile {
            return Err("provider installation belongs to a different target");
        }
        Ok(ProviderExecutionOrigins {
            artifact,
            installation: self,
        })
    }
}

impl ProviderExecutionOrigins {
    pub fn from_inputs(
        artifact: Arc<terminal_codec::CanonicalTerminalArtifact>,
        profile: target::TargetProfile,
        inputs: ProviderInstallationInputs,
    ) -> Self {
        let installation =
            TargetProviderInstallation::new(artifact.manifest().identity(), profile, inputs);
        Self {
            artifact,
            installation,
        }
    }

    pub fn artifact(&self) -> &Arc<terminal_codec::CanonicalTerminalArtifact> {
        &self.artifact
    }

    pub fn installation(&self) -> &TargetProviderInstallation {
        &self.installation
    }

    pub fn inputs(&self) -> &ProviderInstallationInputs {
        &self.installation.inputs
    }

    pub fn into_inputs(
        self,
        profile: target::TargetProfile,
    ) -> Result<
        (
            Arc<terminal_codec::CanonicalTerminalArtifact>,
            ProviderInstallationInputs,
        ),
        &'static str,
    > {
        if self.installation.profile != profile {
            return Err("provider installation belongs to a different target");
        }
        Ok((self.artifact, self.installation.inputs))
    }
}

pub enum ProviderDemandInputs {
    Unscoped {
        artifact: Arc<terminal_codec::CanonicalTerminalArtifact>,
        roots: Vec<MachineId>,
    },
    Target {
        profile: target::TargetProfile,
        logical: ClosedLogicalProviders,
        folded_entry: Result<crate::FoldedEntryReach, String>,
        bodies: Vec<CallableBoundaryImplementation>,
        labels: Vec<(BoundaryMachineId, String)>,
        origins: ProviderExecutionOrigins,
        review: Option<crate::package_review::PackageReviewRequirements>,
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

/// The receiving host has authorized the very program whose providers are being selected.
pub struct AdmittedProviderDemand<Publisher> {
    demand: IssuedProviderDemand<Publisher>,
    authorization: terminal_installation::AuthorizedTerminalArtifact,
}

impl<Publisher> AdmittedProviderDemand<Publisher> {
    pub fn new(
        demand: IssuedProviderDemand<Publisher>,
        authorization: terminal_installation::AuthorizedTerminalArtifact,
    ) -> Result<Self, terminal_installation::ArtifactTrustError> {
        let artifact = match demand.inputs() {
            ProviderDemandInputs::Unscoped { artifact, .. } => artifact,
            ProviderDemandInputs::Target { origins, .. } => &origins.artifact,
        };
        authorization.require_artifact(artifact)?;
        Ok(Self {
            demand,
            authorization,
        })
    }

    pub fn into_parts(
        self,
    ) -> (
        IssuedProviderDemand<Publisher>,
        terminal_installation::AuthorizedTerminalArtifact,
    ) {
        (self.demand, self.authorization)
    }
}
