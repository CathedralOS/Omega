use crate::{
    ResolverExecutionBackend, ResolverExecutionGuarantee, ResolverExecutionGuaranteeDisposition,
    ResolverExecutionPhase, ResolverExecutionPolicyObservation,
};
use std::path::{Path, PathBuf};

fn inspection_root() -> PathBuf {
    std::env::temp_dir()
        .canonicalize()
        .expect("canonical temporary inspection root")
}

fn disposition(
    observation: &ResolverExecutionPolicyObservation,
    guarantee: ResolverExecutionGuarantee,
) -> ResolverExecutionGuaranteeDisposition {
    observation
        .guarantees()
        .iter()
        .find(|row| row.guarantee() == guarantee)
        .expect("complete resolver guarantee row")
        .disposition()
}

mod host_routed;
mod host_selected_git;
mod initialization;
mod inspection;
mod policy_observation;
