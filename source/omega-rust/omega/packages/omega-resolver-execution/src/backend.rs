use crate::ResolverPreparedExecution;
use crate::confinement;
use crate::model::{
    ResolverExecutionBackendIdentity, ResolverExecutionGuarantee, ResolverExecutionGuaranteeRow,
    ResolverExecutionNetworkTransport, ResolverExecutionPhase, ResolverExecutionPolicyObservation,
};
use crate::network::{
    ResolverExecutionEndpointRoute, ResolverExecutionEndpointRoutePolicy,
    ResolverExecutionRequestedEndpoint, ResolverExecutionTransferBudget,
};
use crate::process::limits;
use crate::request::{
    RESOLVER_EXECUTION_ADDITIONAL_EXECUTABLE_LIMIT, require_absolute,
    require_canonical_bounded_path, require_regular_file,
};
use std::io;
use std::path::{Path, PathBuf};
#[cfg(any(test, not(target_os = "macos")))]
use std::process::Command;

#[derive(Debug)]
pub struct ResolverExecutionBackend {
    pub(crate) identity: ResolverExecutionBackendIdentity,
    #[cfg(target_os = "macos")]
    pub(crate) sandbox_metadata: confinement::macos::ExecutableMetadataIdentity,
}

struct ResolverExecutionPolicyInputs<'a> {
    phase: ResolverExecutionPhase,
    network_transport: Option<ResolverExecutionNetworkTransport>,
    endpoint_route: Option<&'a ResolverExecutionEndpointRoutePolicy>,
    generated_policy_sha256: Option<String>,
    executable: &'a Path,
    additional_executables: &'a [PathBuf],
    discovery_read_root: Option<&'a Path>,
    inspection_read_root: Option<&'a Path>,
    mutable_root: Option<&'a Path>,
}

#[derive(Clone, Copy)]
pub(crate) struct ResolverExecutionAuthorityRoots<'a> {
    pub(crate) discovery_read_root: Option<&'a Path>,
    pub(crate) inspection_read_root: Option<&'a Path>,
    pub(crate) mutable_root: Option<&'a Path>,
}

impl ResolverExecutionBackend {
    pub fn open() -> io::Result<Self> {
        #[cfg(target_os = "macos")]
        {
            let path = PathBuf::from(confinement::macos::MACOS_SANDBOX_EXECUTABLE);
            confinement::macos::verify_owned_native_executable(&path)?;
            let sandbox_metadata = confinement::macos::executable_metadata_identity(&path)?;
            let content_sha256 = confinement::macos::hash_executable(&path)?;
            if confinement::macos::executable_metadata_identity(&path)? != sandbox_metadata {
                return Err(io::Error::other(
                    "macOS resolver sandbox boundary changed while opening",
                ));
            }
            Ok(Self {
                identity: ResolverExecutionBackendIdentity::MacosSeatbelt {
                    executable: path,
                    content_sha256,
                },
                sandbox_metadata,
            })
        }
        #[cfg(target_os = "linux")]
        {
            let identity = if confinement::linux::backend_available() {
                ResolverExecutionBackendIdentity::LinuxLandlockV5
            } else {
                ResolverExecutionBackendIdentity::UnixResourceLimits
            };
            Ok(Self { identity })
        }
        #[cfg(all(unix, not(any(target_os = "macos", target_os = "linux"))))]
        {
            Ok(Self {
                identity: ResolverExecutionBackendIdentity::UnixResourceLimits,
            })
        }
        #[cfg(windows)]
        {
            Ok(Self {
                identity: ResolverExecutionBackendIdentity::WindowsJobObject,
            })
        }
        #[cfg(not(any(unix, windows)))]
        {
            Ok(Self {
                identity: ResolverExecutionBackendIdentity::PortableProcessContainer,
            })
        }
    }

    pub const fn identity(&self) -> &ResolverExecutionBackendIdentity {
        &self.identity
    }

    /// Reject a Linux package-source launch that would otherwise degrade to
    /// resource limits without a native filesystem boundary.
    pub fn require_package_resolution_floor(&self) -> io::Result<()> {
        #[cfg(target_os = "linux")]
        if !matches!(
            self.identity,
            ResolverExecutionBackendIdentity::LinuxLandlockV5
        ) {
            return Err(io::Error::other(
                "Linux package resolution requires fully available Landlock ABI v5",
            ));
        }
        Ok(())
    }

    /// Open a compiler-owned loopback broker for one already-validated remote
    /// destination. This does not establish transport trust or acceptance.
    pub fn open_endpoint_route(
        &self,
        requested_endpoint: ResolverExecutionRequestedEndpoint,
        transfer_budget: ResolverExecutionTransferBudget,
    ) -> io::Result<ResolverExecutionEndpointRoute> {
        self.verify()?;
        ResolverExecutionEndpointRoute::open(requested_endpoint, transfer_budget)
    }

    fn policy_observation(
        &self,
        inputs: ResolverExecutionPolicyInputs<'_>,
    ) -> io::Result<ResolverExecutionPolicyObservation> {
        self.verify()?;
        let guarantees =
            ResolverExecutionGuarantee::ALL.map(|guarantee| ResolverExecutionGuaranteeRow {
                guarantee,
                disposition: confinement::guarantee_disposition(
                    &self.identity,
                    inputs.phase,
                    inputs.network_transport,
                    inputs.endpoint_route.is_some(),
                    guarantee,
                ),
            });
        Ok(ResolverExecutionPolicyObservation {
            backend: self.identity.clone(),
            phase: inputs.phase,
            network_transport: inputs.network_transport,
            endpoint_route: inputs.endpoint_route.cloned(),
            generated_policy_sha256: inputs.generated_policy_sha256,
            resource_ceilings: limits::configured_resource_ceilings(),
            executable: inputs.executable.to_path_buf(),
            additional_executables: inputs.additional_executables.to_vec(),
            discovery_read_root: inputs.discovery_read_root.map(Path::to_path_buf),
            inspection_read_root: inputs.inspection_read_root.map(Path::to_path_buf),
            mutable_root: inputs.mutable_root.map(Path::to_path_buf),
            guarantees,
        })
    }

    pub fn verify(&self) -> io::Result<()> {
        #[cfg(target_os = "macos")]
        {
            let ResolverExecutionBackendIdentity::MacosSeatbelt {
                executable,
                content_sha256,
            } = &self.identity
            else {
                return Err(io::Error::other(
                    "macOS resolver selected a non-Seatbelt backend",
                ));
            };
            confinement::macos::verify_owned_native_executable(executable)?;
            if confinement::macos::executable_metadata_identity(executable)?
                != self.sandbox_metadata
                || confinement::macos::hash_executable(executable)? != *content_sha256
            {
                return Err(io::Error::other(
                    "macOS resolver sandbox executable changed",
                ));
            }
        }
        #[cfg(target_os = "linux")]
        if matches!(
            self.identity,
            ResolverExecutionBackendIdentity::LinuxLandlockV5
        ) && !confinement::linux::backend_available()
        {
            return Err(io::Error::other(
                "Linux resolver Landlock v5 boundary became unavailable",
            ));
        }
        Ok(())
    }

    /// Construct a command under the host's selected native enforcement.
    ///
    /// `additional_executables` is the closed set of transport helpers the
    /// already-verified primary executable may launch. `inspection_read_root`
    /// is required exactly for repository inspection. `mutable_root` is
    /// required exactly for the two mutating phases and rejected otherwise.
    #[cfg(test)]
    pub(crate) fn command(
        &self,
        executable: &Path,
        additional_executables: &[PathBuf],
        phase: ResolverExecutionPhase,
        mutable_root: Option<&Path>,
    ) -> io::Result<Command> {
        let network_transport = phase
            .permits_network()
            .then_some(ResolverExecutionNetworkTransport::Ssh);
        self.command_with_observation(
            executable,
            additional_executables,
            phase,
            network_transport,
            mutable_root,
        )
        .map(|(command, _observation)| command)
    }

    #[cfg(all(test, target_os = "macos"))]
    pub(crate) fn command_with_inspection_read_root(
        &self,
        executable: &Path,
        additional_executables: &[PathBuf],
        inspection_read_root: &Path,
    ) -> io::Result<Command> {
        let (mut command, _observation) = self.command_with_inspection_read_root_observation(
            executable,
            additional_executables,
            inspection_read_root,
        )?;
        command.current_dir(inspection_read_root);
        Ok(command)
    }

    /// Prepare one non-routed command with its policy retained inside the
    /// opaque execution value. Spawning consumes that value.
    pub fn prepare(
        &self,
        executable: &Path,
        additional_executables: &[PathBuf],
        phase: ResolverExecutionPhase,
        network_transport: Option<ResolverExecutionNetworkTransport>,
        mutable_root: Option<&Path>,
    ) -> io::Result<ResolverPreparedExecution> {
        self.prepare_with_endpoint_route(
            executable,
            additional_executables,
            phase,
            network_transport,
            None,
            mutable_root,
        )
    }

    #[cfg(test)]
    pub fn command_with_observation(
        &self,
        executable: &Path,
        additional_executables: &[PathBuf],
        phase: ResolverExecutionPhase,
        network_transport: Option<ResolverExecutionNetworkTransport>,
        mutable_root: Option<&Path>,
    ) -> io::Result<(Command, ResolverExecutionPolicyObservation)> {
        self.prepare(
            executable,
            additional_executables,
            phase,
            network_transport,
            mutable_root,
        )
        .map(ResolverPreparedExecution::into_parts)
    }

    /// Construct one repository-inspection command bound to the exact retained
    /// repository whose file contents may be read.
    pub fn prepare_inspection(
        &self,
        executable: &Path,
        additional_executables: &[PathBuf],
        inspection_read_root: &Path,
    ) -> io::Result<ResolverPreparedExecution> {
        self.prepare_with_authority_roots(
            executable,
            additional_executables,
            ResolverExecutionPhase::RepositoryInspection,
            None,
            None,
            ResolverExecutionAuthorityRoots {
                discovery_read_root: None,
                inspection_read_root: Some(inspection_read_root),
                mutable_root: None,
            },
        )
    }

    #[cfg(test)]
    pub fn command_with_inspection_read_root_observation(
        &self,
        executable: &Path,
        additional_executables: &[PathBuf],
        inspection_read_root: &Path,
    ) -> io::Result<(Command, ResolverExecutionPolicyObservation)> {
        self.prepare_inspection(executable, additional_executables, inspection_read_root)
            .map(ResolverPreparedExecution::into_parts)
    }

    /// Construct one transport-discovery command bound to the exact working
    /// root whose file contents may be read by a narrowed transport policy.
    pub fn prepare_discovery(
        &self,
        executable: &Path,
        additional_executables: &[PathBuf],
        network_transport: ResolverExecutionNetworkTransport,
        endpoint_route: &ResolverExecutionEndpointRoute,
        discovery_read_root: &Path,
    ) -> io::Result<ResolverPreparedExecution> {
        self.prepare_with_authority_roots(
            executable,
            additional_executables,
            ResolverExecutionPhase::TransportDiscovery,
            Some(network_transport),
            Some(endpoint_route),
            ResolverExecutionAuthorityRoots {
                discovery_read_root: Some(discovery_read_root),
                inspection_read_root: None,
                mutable_root: None,
            },
        )
    }

    #[cfg(test)]
    pub fn command_with_discovery_route_observation(
        &self,
        executable: &Path,
        additional_executables: &[PathBuf],
        network_transport: ResolverExecutionNetworkTransport,
        endpoint_route: &ResolverExecutionEndpointRoute,
        discovery_read_root: &Path,
    ) -> io::Result<(Command, ResolverExecutionPolicyObservation)> {
        self.prepare_discovery(
            executable,
            additional_executables,
            network_transport,
            endpoint_route,
            discovery_read_root,
        )
        .map(ResolverPreparedExecution::into_parts)
    }

    /// Construct one command and bind its native policy to an endpoint route.
    /// Network phases require a route; nonnetwork phases reject one. Finish the
    /// route after execution to obtain its separate endpoint observation.
    pub fn prepare_with_endpoint_route(
        &self,
        executable: &Path,
        additional_executables: &[PathBuf],
        phase: ResolverExecutionPhase,
        network_transport: Option<ResolverExecutionNetworkTransport>,
        endpoint_route: Option<&ResolverExecutionEndpointRoute>,
        mutable_root: Option<&Path>,
    ) -> io::Result<ResolverPreparedExecution> {
        self.prepare_with_authority_roots(
            executable,
            additional_executables,
            phase,
            network_transport,
            endpoint_route,
            ResolverExecutionAuthorityRoots {
                discovery_read_root: None,
                inspection_read_root: None,
                mutable_root,
            },
        )
    }

    #[cfg(test)]
    pub fn command_with_endpoint_route_observation(
        &self,
        executable: &Path,
        additional_executables: &[PathBuf],
        phase: ResolverExecutionPhase,
        network_transport: Option<ResolverExecutionNetworkTransport>,
        endpoint_route: Option<&ResolverExecutionEndpointRoute>,
        mutable_root: Option<&Path>,
    ) -> io::Result<(Command, ResolverExecutionPolicyObservation)> {
        self.prepare_with_endpoint_route(
            executable,
            additional_executables,
            phase,
            network_transport,
            endpoint_route,
            mutable_root,
        )
        .map(ResolverPreparedExecution::into_parts)
    }

    fn prepare_with_authority_roots(
        &self,
        executable: &Path,
        additional_executables: &[PathBuf],
        phase: ResolverExecutionPhase,
        network_transport: Option<ResolverExecutionNetworkTransport>,
        endpoint_route: Option<&ResolverExecutionEndpointRoute>,
        roots: ResolverExecutionAuthorityRoots<'_>,
    ) -> io::Result<ResolverPreparedExecution> {
        self.verify()?;
        require_absolute(executable, "resolver executable")?;
        require_regular_file(executable, "resolver executable")?;
        if additional_executables.len() > RESOLVER_EXECUTION_ADDITIONAL_EXECUTABLE_LIMIT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "resolver helper executable set exceeds its fixed limit",
            ));
        }
        require_canonical_bounded_path(executable, "resolver executable")?;
        for helper in additional_executables {
            require_absolute(helper, "resolver helper executable")?;
            require_canonical_bounded_path(helper, "resolver helper executable")?;
            require_regular_file(helper, "resolver helper executable")?;
        }
        let mut additional_executables = additional_executables.to_vec();
        additional_executables.retain(|helper| helper != executable);
        additional_executables.sort();
        additional_executables.dedup();
        match (phase.permits_network(), network_transport, endpoint_route) {
            (true, Some(_), Some(_)) | (false, None, None) => {}
            (true, None, _) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "networked resolver phase has no closed transport authority",
                ));
            }
            (true, Some(_), None) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "networked resolver phase has no endpoint route",
                ));
            }
            (false, Some(_), _) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "nonnetwork resolver phase received transport authority",
                ));
            }
            (false, None, Some(_)) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "nonnetwork resolver phase received an endpoint route",
                ));
            }
        }
        match (phase.requires_mutable_root(), roots.mutable_root) {
            (true, Some(root)) => {
                require_absolute(root, "resolver mutable root")?;
                require_canonical_bounded_path(root, "resolver mutable root")?;
            }
            (true, None) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "mutating resolver phase has no mutable root",
                ));
            }
            (false, Some(_)) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "read-only resolver phase received a mutable root",
                ));
            }
            (false, None) => {}
        }
        match (phase, roots.inspection_read_root) {
            (ResolverExecutionPhase::RepositoryInspection, Some(root)) => {
                require_absolute(root, "resolver inspection read root")?;
                require_canonical_bounded_path(root, "resolver inspection read root")?;
            }
            (ResolverExecutionPhase::RepositoryInspection, None) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "repository inspection has no read root",
                ));
            }
            (_, Some(_)) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "noninspection resolver phase received an inspection read root",
                ));
            }
            (_, None) => {}
        }
        match (phase, roots.discovery_read_root) {
            (ResolverExecutionPhase::TransportDiscovery, Some(root)) => {
                require_absolute(root, "resolver discovery read root")?;
                require_canonical_bounded_path(root, "resolver discovery read root")?;
            }
            (ResolverExecutionPhase::TransportDiscovery, None) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "transport discovery has no read root",
                ));
            }
            (_, Some(_)) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "nondiscovery resolver phase received a discovery read root",
                ));
            }
            (_, None) => {}
        }

        #[cfg(target_os = "macos")]
        let (mut command, generated_policy_sha256) = confinement::macos::command(
            self,
            executable,
            &additional_executables,
            phase,
            network_transport,
            endpoint_route.map(ResolverExecutionEndpointRoute::policy),
            roots,
        )?;
        #[cfg(not(target_os = "macos"))]
        let mut command = Command::new(executable);
        #[cfg(not(target_os = "macos"))]
        let generated_policy_sha256 = None;

        limits::configure_child_resource_limits(&mut command)?;
        let observation = self.policy_observation(ResolverExecutionPolicyInputs {
            phase,
            network_transport,
            endpoint_route: endpoint_route.map(ResolverExecutionEndpointRoute::policy),
            generated_policy_sha256,
            executable,
            additional_executables: &additional_executables,
            discovery_read_root: roots.discovery_read_root,
            inspection_read_root: roots.inspection_read_root,
            mutable_root: roots.mutable_root,
        })?;
        Ok(ResolverPreparedExecution::new(command, observation))
    }

    #[cfg(test)]
    fn command_with_authority_roots_observation(
        &self,
        executable: &Path,
        additional_executables: &[PathBuf],
        phase: ResolverExecutionPhase,
        network_transport: Option<ResolverExecutionNetworkTransport>,
        endpoint_route: Option<&ResolverExecutionEndpointRoute>,
        roots: ResolverExecutionAuthorityRoots<'_>,
    ) -> io::Result<(Command, ResolverExecutionPolicyObservation)> {
        self.prepare_with_authority_roots(
            executable,
            additional_executables,
            phase,
            network_transport,
            endpoint_route,
            roots,
        )
        .map(ResolverPreparedExecution::into_parts)
    }
}

#[cfg(test)]
mod tests;
