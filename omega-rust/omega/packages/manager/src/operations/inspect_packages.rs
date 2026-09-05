//! Fresh project inspection without accepting or publishing package changes.

mod execution;
mod report;

use super::{PackageFileTransaction, PackagePublicationLimits};
use package_source::SourceResolverStorage;
use std::fmt;
use std::path::PathBuf;
use target::TargetProfile;

#[derive(Debug, Clone)]
pub struct PackageInspectionOptions {
    pub project_root: PathBuf,
    /// Empty selects every accepted target, or the host for an unlocked project.
    pub targets: Vec<TargetProfile>,
    /// Include full compiler-owned normalized policy after the readable summary.
    pub details: bool,
}

#[derive(Debug)]
pub struct PackageInspectionOutcome {
    pub report: String,
    /// False means at least one target has no fresh compiler findings.
    pub complete: bool,
    /// Inspection reports required changes but never resolves their decisions.
    pub requires_decision: bool,
}

#[derive(Debug)]
pub struct PackageInspectionError(String);

impl fmt::Display for PackageInspectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PackageInspectionError {}

fn failure(error: impl fmt::Display) -> PackageInspectionError {
    PackageInspectionError(error.to_string())
}

pub fn inspect_packages(
    options: PackageInspectionOptions,
) -> Result<PackageInspectionOutcome, PackageInspectionError> {
    let transaction =
        PackageFileTransaction::open(&options.project_root, PackagePublicationLimits::default())
            .map_err(failure)?;
    execution::inspect(&transaction, options.targets, options.details, |root| {
        SourceResolverStorage::for_current_user_excluding_primary_git_roots(&[root.to_path_buf()])
            .map_err(failure)
    })
}

pub fn inspect_packages_with_storage(
    options: PackageInspectionOptions,
    storage: &SourceResolverStorage,
) -> Result<PackageInspectionOutcome, PackageInspectionError> {
    let transaction =
        PackageFileTransaction::open(&options.project_root, PackagePublicationLimits::default())
            .map_err(failure)?;
    execution::inspect(&transaction, options.targets, options.details, |_| {
        Ok(storage)
    })
}
