//! Command proposal and editable findings, separate from publication intent.

use super::model::{PackageCommandError, PackageCommandOutcome, PackageCommandStatus, failure};
use crate::operations::{PackageFileTransaction, PackagePublicationError};
use platform_custody::record_file::{RecordFileLimits, RecordFileRoot, RootRecordRead};
use std::path::{Path, PathBuf};
use target::TargetProfile;

pub(super) const PROPOSAL: &str = "proposal";
pub(super) const LIMITS: RecordFileLimits = RecordFileLimits {
    maximum_bytes: 128 * 1024 * 1024,
};

pub(super) fn file_failure(
    error: platform_custody::record_file::RecordFileError,
) -> PackageCommandError {
    failure(PackagePublicationError::File(error))
}

pub(super) fn read<'a>(
    files: &'a RecordFileRoot,
    name: &str,
) -> Result<Option<RootRecordRead<'a>>, PackageCommandError> {
    files
        .read_optional(Path::new(name), LIMITS)
        .map_err(file_failure)
}

pub(super) fn text<'a>(read: &'a RootRecordRead<'_>) -> Result<&'a str, PackageCommandError> {
    std::str::from_utf8(read.bytes()).map_err(|_| failure("package command state is not UTF-8"))
}

pub(super) fn write(
    files: &RecordFileRoot,
    name: &str,
    text: &str,
) -> Result<(), PackageCommandError> {
    if let Some(before) = files
        .read_optional(Path::new(name), LIMITS)
        .map_err(file_failure)?
    {
        before
            .replace(text.as_bytes(), LIMITS)
            .map_err(file_failure)
    } else {
        files
            .write_new(Path::new(name), text.as_bytes(), LIMITS)
            .map_err(file_failure)
    }
}

pub(super) fn write_proposal(
    files: &RecordFileRoot,
    text: &str,
) -> Result<(), PackageCommandError> {
    files
        .write_new(Path::new(PROPOSAL), text.as_bytes(), LIMITS)
        .map_err(file_failure)
}

pub(super) fn review_name(target: TargetProfile) -> String {
    format!("review-{}.txt", target.target_name())
}

pub(super) fn review_path(transaction: &PackageFileTransaction, target: TargetProfile) -> PathBuf {
    transaction
        .project_root()
        .join("build/package-manager")
        .join(review_name(target))
}

pub(super) fn discard(
    transaction: &PackageFileTransaction,
) -> Result<PackageCommandOutcome, PackageCommandError> {
    let files = transaction.command_state_files().map_err(failure)?;
    let report = if let Some(proposal) = read(&files, PROPOSAL)? {
        proposal.remove(LIMITS).map_err(file_failure)?;
        "Discarded the pending package proposal. Existing review documents remain diagnostic files; accepted project files were not rolled back."
    } else {
        "No pending package review to discard."
    };
    Ok(PackageCommandOutcome {
        status: PackageCommandStatus::ReviewDiscarded,
        report: report.to_owned(),
        review_paths: Vec::new(),
    })
}
