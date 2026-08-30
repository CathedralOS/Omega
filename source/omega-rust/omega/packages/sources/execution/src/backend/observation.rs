use super::ResolverExecutionBackend;
use crate::confinement;
use crate::model::{
    ResolverExecutionGuarantee, ResolverExecutionGuaranteeRow, ResolverExecutionNetworkTransport,
    ResolverExecutionPhase, ResolverExecutionPolicyObservation,
};
use crate::network::ResolverExecutionEndpointRoutePolicy;
use crate::process::limits;
use std::io;
use std::path::{Path, PathBuf};

pub(super) struct ResolverExecutionPolicyInputs<'a> {
    pub(super) phase: ResolverExecutionPhase,
    pub(super) network_transport: Option<ResolverExecutionNetworkTransport>,
    pub(super) endpoint_route: Option<&'a ResolverExecutionEndpointRoutePolicy>,
    pub(super) generated_policy_sha256: Option<String>,
    pub(super) executable: &'a Path,
    pub(super) additional_executables: &'a [PathBuf],
    pub(super) discovery_read_root: Option<&'a Path>,
    pub(super) inspection_read_root: Option<&'a Path>,
    pub(super) mutable_root: Option<&'a Path>,
}

impl ResolverExecutionBackend {
    pub(super) fn policy_observation(
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
}
