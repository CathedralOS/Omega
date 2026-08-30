use super::ResolverExecutionBackend;
use crate::confinement;
use crate::model::{
    ResolverExecutionGuarantee, ResolverExecutionGuaranteeRow, ResolverExecutionPhase,
    ResolverExecutionPolicyObservation,
};
use crate::process::limits;
use std::io;
use std::path::Path;

pub(super) struct ResolverExecutionPolicyInputs<'a> {
    pub(super) phase: ResolverExecutionPhase,
    pub(super) generated_policy_sha256: Option<String>,
    pub(super) executable: &'a Path,
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
                    guarantee,
                    inputs.generated_policy_sha256.is_some(),
                ),
            });
        Ok(ResolverExecutionPolicyObservation {
            backend: self.identity.clone(),
            phase: inputs.phase,
            generated_policy_sha256: inputs.generated_policy_sha256,
            resource_ceilings: limits::configured_resource_ceilings(),
            executable: inputs.executable.to_path_buf(),
            discovery_read_root: inputs.discovery_read_root.map(Path::to_path_buf),
            inspection_read_root: inputs.inspection_read_root.map(Path::to_path_buf),
            mutable_root: inputs.mutable_root.map(Path::to_path_buf),
            guarantees,
        })
    }
}
