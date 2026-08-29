//! Resolver-issued source and execution observations exposed to callers.

mod accounting;
mod execution;
mod resolution;
mod resolved;

pub use accounting::{GitCapturedOutputObservation, GitNetworkTransferObservation};
pub use execution::{
    GitCommandExecutionObservation, GitExecutableIdentity, GitTransportExecutableIdentity,
};
pub use resolution::GitSourceResolutionObservation;
pub use resolved::ResolvedGitSource;

pub(in crate::source::acquisition) use accounting::{
    git_captured_output_observation, git_network_transfer_observation,
    git_resolution_captured_output_ceiling, git_resolution_network_transfer_ceiling,
};
pub(in crate::source::acquisition) use resolution::issue_git_source_resolution_observation;
pub(in crate::source::acquisition) use resolved::PendingResolvedGitSource;
