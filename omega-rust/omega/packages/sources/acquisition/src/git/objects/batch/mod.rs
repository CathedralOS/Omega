//! Bounded `cat-file --batch` transfer.
//!
//! Follow the operation from [`transfer`], through exact response handling in
//! [`protocol`], to private request-file ownership in [`custody`].

mod custody;
mod protocol;
mod transfer;

pub(crate) use custody::PendingGitBatchRequest;
pub(crate) use protocol::{assign_git_batch_output, git_batch_output_limit};
pub(super) use transfer::read_git_blobs_batch;
#[cfg(test)]
pub(crate) use transfer::read_git_blobs_batch_from_path;
