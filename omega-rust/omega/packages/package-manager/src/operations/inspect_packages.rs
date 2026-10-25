//! Fresh project inspection without accepting or publishing package changes.

mod execution;
mod report;

use super::{PackageFileTransaction, PackagePublicationLimits};
use crate::build_session::PackageBuildSession;
use crate::review::CompileResolvedPackageReviewsError;
use package_source::PrimaryGitChoices;
use package_source::SourceResolver;
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

diagnostics::string_diagnostic!(PackageInspectionError);

fn failure(error: impl fmt::Display) -> PackageInspectionError {
    PackageInspectionError(error.to_string())
}

impl From<CompileResolvedPackageReviewsError> for PackageInspectionError {
    fn from(error: CompileResolvedPackageReviewsError) -> Self {
        failure(error)
    }
}

/// Inspect the packages of one project. Without `storage`, the current user's
/// resolver storage is opened once the project files have been read, with the
/// project root excluded from primary Git selection.
/// One sponsored session covers every requested target, including targets
/// examined after a sibling fails; returning disposes the complete invocation.
pub fn inspect_packages(
    options: PackageInspectionOptions,
    storage: Option<&SourceResolver>,
    execute: crate::review::PackageReviewExecutor,
) -> Result<PackageInspectionOutcome, PackageInspectionError> {
    let transaction =
        PackageFileTransaction::open(&options.project_root, PackagePublicationLimits::default())
            .map_err(failure)?;
    // Inspection needs the same source selection as checking, not an output
    // request or a publication grant. The shared review pipeline owns capture.
    let build_snapshot = options
        .build_inputs
        .map(|inputs| build_evaluation::BuildSnapshotRequest::scoped(Vec::new(), inputs));
    let mut session =
        PackageBuildSession::create(&transaction.project_root().join("build/package-manager"))
            .map_err(failure)?;
    let result = execution::inspect(
        &transaction,
        options.targets,
        options.details,
        options.offline,
        build_snapshot.as_ref(),
        &mut session,
        execute,
        |root| match storage {
            Some(storage) => Ok(InspectionStorage::Supplied(storage)),
            None => SourceResolver::for_current_user(PrimaryGitChoices {
                excluded_controlled_roots: &[root.to_path_buf()],
                ..PrimaryGitChoices::default()
            })
            .map(InspectionStorage::Opened)
            .map_err(failure),
        },
    );
    session.dispose(result)
}

/// The resolver storage of one inspection: supplied by the caller, or opened
/// for the current user after the project files were read.
enum InspectionStorage<'a> {
    Supplied(&'a SourceResolver),
    Opened(SourceResolver),
}

impl std::borrow::Borrow<SourceResolver> for InspectionStorage<'_> {
    fn borrow(&self) -> &SourceResolver {
        match self {
            Self::Supplied(storage) => storage,
            Self::Opened(storage) => storage,
        }
    }
}
