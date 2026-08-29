use super::observation::ResolverExecutionPolicyInputs;
use super::request::validate_launch_request;
use super::{ResolverExecutionAuthorityRoots, ResolverExecutionBackend};
use crate::confinement;
#[cfg(test)]
use crate::model::ResolverExecutionPolicyObservation;
use crate::model::{ResolverExecutionNetworkTransport, ResolverExecutionPhase};
use crate::network::ResolverExecutionEndpointRoute;
use crate::process::limits;
use crate::ResolverPreparedExecution;
use std::io;
use std::path::{Path, PathBuf};
#[cfg(any(test, not(target_os = "macos")))]
use std::process::Command;

impl ResolverExecutionBackend {
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

    pub(super) fn prepare_with_authority_roots(
        &self,
        executable: &Path,
        additional_executables: &[PathBuf],
        phase: ResolverExecutionPhase,
        network_transport: Option<ResolverExecutionNetworkTransport>,
        endpoint_route: Option<&ResolverExecutionEndpointRoute>,
        roots: ResolverExecutionAuthorityRoots<'_>,
    ) -> io::Result<ResolverPreparedExecution> {
        self.verify()?;
        let additional_executables = validate_launch_request(
            executable,
            additional_executables,
            phase,
            network_transport,
            endpoint_route,
            roots,
        )?;

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
    pub(super) fn command_with_authority_roots_observation(
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
