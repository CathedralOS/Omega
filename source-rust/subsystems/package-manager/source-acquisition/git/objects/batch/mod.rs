// Bounded `git cat-file --batch` blob transfer and `--batch-check` exact-object
// availability probes, backed by private capability-verified request files.

mod availability;
mod custody;
mod protocol;
mod transfer;

pub(in crate::git) use availability::{
    ExactGitObjectAvailability, ExactGitObjectKind, probe_exact_git_object,
};
pub(crate) use custody::PendingGitBatchRequest;
pub(crate) use protocol::{assign_git_batch_output, git_batch_output_limit};
pub(super) use transfer::read_git_blobs_batch;
#[cfg(test)]
pub(crate) use transfer::read_git_blobs_batch_from_path;
