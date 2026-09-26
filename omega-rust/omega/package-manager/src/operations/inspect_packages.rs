//! Fresh project inspection without accepting or publishing package changes.

mod execution;
mod report;

use super::{PackageFileTransaction, PackagePublicationLimits};
use package_source::PrimaryGitChoices;
use package_source::SourceResolverStorage;
use std::fmt;
use std::path::PathBuf;
use target::TargetProfile;

#[derive(Debug, Clone)]
pub struct PackageInspectionOptions {
    pub project_root: PathBuf,
    /// Empty selects every accepted target, or the host for an unlocked
    /// project; a host with no catalogued profile declines with a diagnostic.
    pub targets: Vec<TargetProfile>,
    /// Include full compiler-owned normalized policy after the readable summary.
    pub details: bool,
    /// Restrict Git acquisition to cached exact pins, without selector refresh.
    pub offline: bool,
    /// Caller-selected immutable root inputs, relative to the project source
    /// root. Dependencies retain their own inventories. Absence preserves the
    /// ordinary package inventory; selection grants no restricted host access.
    pub build_inputs: Option<package_compilation::BuildSourceCaptureRequest>,
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

/// Inspect the packages of one project. Without `storage`, the current user's
/// resolver storage is opened once the project files have been read, with the
/// project root excluded from primary Git selection.
pub fn inspect_packages(
    options: PackageInspectionOptions,
    storage: Option<&SourceResolverStorage>,
) -> Result<PackageInspectionOutcome, PackageInspectionError> {
    let transaction =
        PackageFileTransaction::open(&options.project_root, PackagePublicationLimits::default())
            .map_err(failure)?;
    // Inspection needs the same source selection as checking, not an output
    // request or a publication grant. The shared review pipeline owns capture.
    let build_snapshot = options
        .build_inputs
        .map(|inputs| build_evaluation::BuildSnapshotRequest::scoped(Vec::new(), inputs));
    execution::inspect(
        &transaction,
        options.targets,
        options.details,
        options.offline,
        build_snapshot.as_ref(),
        |root| match storage {
            Some(storage) => Ok(InspectionStorage::Supplied(storage)),
            None => SourceResolverStorage::for_current_user(PrimaryGitChoices {
                excluded_controlled_roots: &[root.to_path_buf()],
                ..PrimaryGitChoices::default()
            })
            .map(InspectionStorage::Opened)
            .map_err(failure),
        },
    )
}

/// The resolver storage of one inspection: supplied by the caller, or opened
/// for the current user after the project files were read.
enum InspectionStorage<'a> {
    Supplied(&'a SourceResolverStorage),
    Opened(SourceResolverStorage),
}

impl std::borrow::Borrow<SourceResolverStorage> for InspectionStorage<'_> {
    fn borrow(&self) -> &SourceResolverStorage {
        match self {
            Self::Supplied(storage) => storage,
            Self::Opened(storage) => storage,
        }
    }
}
