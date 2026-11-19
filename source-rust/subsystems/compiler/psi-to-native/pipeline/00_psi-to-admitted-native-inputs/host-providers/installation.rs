//! Receiving-host provider inputs for one authorized portable artifact and target.
#![forbid(unsafe_code)]
mod permissions;
mod semantic_bindings;

pub use permissions::{
    PortableFilesystemAuthorityFacet, ServiceTerminalAuthorityPermission,
    TerminalAuthorityDisposition,
};
pub use semantic_bindings::{
    AcceptedSemanticBinding, AcceptedSemanticBindingRole, program_entry_semantic_binding_role,
};
pub use semantic_vocabulary::TerminalAuthorityClass;

use declaration_scope::ProductDeclarationScope;
use std::sync::Arc;

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
    pub semantic_bindings: Arc<[AcceptedSemanticBinding]>,
}

/// A receiving host can use its reviewed packages for several programs. The
/// package installer owns these bindings and grants before any program arrives.
#[derive(Clone)]
pub struct InstalledPackageProviders {
    profile: target::TargetProfile,
    bindings: Vec<evaluation_receipts::NativeBindingMaterialization>,
    package_permissions: ProductDeclarationScope,
    semantic_bindings: Arc<[AcceptedSemanticBinding]>,
}

impl InstalledPackageProviders {
    pub fn new(
        profile: target::TargetProfile,
        bindings: Vec<evaluation_receipts::NativeBindingMaterialization>,
        package_permissions: ProductDeclarationScope,
        semantic_bindings: Arc<[AcceptedSemanticBinding]>,
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
        authorization: &psi_authorization::AuthorizedTerminalArtifact,
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

#[cfg(test)]
mod tests {
    use super::*;
    use psi_authorization::{HostProducerTrust, authorize_terminal_artifact};
    use terminal_codec::{
        CanonicalTerminalArtifact, TerminalDebugMap, build_identity_optimization_execution_record,
    };

    #[test]
    fn installation_accepts_reopened_bytes_and_requires_the_manifest_and_target() {
        let module = terminal_psi::TerminalModule::declarations();
        let optimization = build_identity_optimization_execution_record(&module).unwrap();
        let program = Arc::new(
            CanonicalTerminalArtifact::from_parts(module.clone(), optimization.clone(), None)
                .unwrap(),
        );
        let host = HostProducerTrust::accept("host installation test");
        let authorization = authorize_terminal_artifact(Arc::clone(&program), Some(&host)).unwrap();
        let profile = target::TargetProfile::from_omega_target_name(Some("linux_x86_64")).unwrap();
        let other_profile =
            target::TargetProfile::from_omega_target_name(Some("linux_arm64")).unwrap();
        let packages = InstalledPackageProviders::new(
            profile,
            Vec::new(),
            ProductDeclarationScope::from_direct_dependencies(&[], []),
            Arc::default(),
        );
        let installed = packages.for_program(&authorization);
        let reopened =
            Arc::new(CanonicalTerminalArtifact::from_bytes(&program.to_bytes()).unwrap());
        let origins = installed.clone().attach(reopened, profile).unwrap();
        assert!(origins.inputs().package_permissions.is_some());
        assert!(matches!(
            origins.into_inputs(other_profile),
            Err("provider installation belongs to a different target")
        ));
        assert!(matches!(
            installed
                .clone()
                .attach(Arc::clone(&program), other_profile),
            Err("provider installation belongs to a different target")
        ));
        let debug = TerminalDebugMap {
            semantic: program.view().identity(),
            files: Vec::new(),
            sites: Vec::new(),
        };
        let changed =
            CanonicalTerminalArtifact::from_parts(module, optimization, Some(&debug)).unwrap();
        assert_eq!(program.view().identity(), changed.view().identity());
        assert!(matches!(
            installed.attach(Arc::new(changed), profile),
            Err("provider installation belongs to a different Psi artifact")
        ));
    }
}
