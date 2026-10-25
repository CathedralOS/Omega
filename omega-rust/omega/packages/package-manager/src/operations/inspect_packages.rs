//! Fresh project inspection without accepting or publishing package changes.

mod report;
mod sources;

use super::{PackageFileTransaction, PackagePublicationLimits};
use crate::build_session::{PackageBuildSession, PackageSourceBindings};
use crate::lock::{PackageLock, PackageLockRecoveryLimits, PackageLockTarget};
use crate::review::CompileResolvedPackageReviewsError;
use package_source::PrimaryGitChoices;
use package_source::SourceResolver;
use std::fmt;
use std::path::{Path, PathBuf};
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

/// Holds the original project transaction throughout observational review.
/// Opening never recovers pending publication or changes proposal files.
pub struct PackageInspectionTransaction {
    files: PackageFileTransaction,
    options: PackageInspectionOptions,
}

impl PackageInspectionTransaction {
    pub fn open(options: PackageInspectionOptions) -> Result<Self, PackageInspectionError> {
        let files = PackageFileTransaction::open(
            &options.project_root,
            PackagePublicationLimits::default(),
        )
        .map_err(failure)?;
        Ok(Self { files, options })
    }

    pub fn project_root(&self) -> &Path {
        self.files.project_root()
    }

    /// Read accepted policy and select targets before opening resolver storage.
    /// Storage failure becomes unavailable findings per target, not an empty graph.
    pub fn prepare<'inspection>(
        &'inspection self,
        storage: Option<&'inspection SourceResolver>,
    ) -> Result<PreparedPackageInspection<'inspection>, PackageInspectionError> {
        let before = self.files.read_pair().map_err(failure)?;
        let accepted = before
            .1
            .as_deref()
            .map(|bytes| {
                let text = std::str::from_utf8(bytes).map_err(failure)?;
                PackageLock::recover_review_baseline(text, PackageLockRecoveryLimits::default())
                    .map_err(failure)
            })
            .transpose()?;
        let targets = select_targets(self.options.targets.clone(), accepted.as_ref())?;
        let storage = match storage {
            Some(storage) => Ok(InspectionStorage::Supplied(storage)),
            None => SourceResolver::for_current_user(PrimaryGitChoices {
                excluded_controlled_roots: &[self.project_root().to_path_buf()],
                ..PrimaryGitChoices::default()
            })
            .map(InspectionStorage::Opened)
            .map_err(failure),
        };
        let build_snapshot = self
            .options
            .build_inputs
            .clone()
            .map(|inputs| build_evaluation::BuildSnapshotRequest::scoped(Vec::new(), inputs));
        Ok(PreparedPackageInspection {
            transaction: self,
            before,
            accepted,
            targets,
            storage,
            build_snapshot,
            completed_targets: 0,
            outcome: PackageInspectionOutcome {
                report: String::new(),
                complete: true,
                requires_decision: false,
            },
        })
    }
}

/// Package-owned admission and report state. Application Build executes the
/// target roster; this owner compares findings and checks final file identity.
pub struct PreparedPackageInspection<'inspection> {
    transaction: &'inspection PackageInspectionTransaction,
    before: (Vec<u8>, Option<Vec<u8>>),
    accepted: Option<PackageLock>,
    targets: Vec<TargetProfile>,
    storage: Result<InspectionStorage<'inspection>, PackageInspectionError>,
    build_snapshot: Option<build_evaluation::BuildSnapshotRequest>,
    completed_targets: usize,
    outcome: PackageInspectionOutcome,
}

// Full normalized findings include all authored target members. Bound aggregate
// reporting without truncating contracts or dropping inactive deployments.
const MAXIMUM_REPORT_BYTES: usize = 64 * 1024 * 1024;

impl PreparedPackageInspection<'_> {
    pub fn targets(&self) -> &[TargetProfile] {
        &self.targets
    }

    /// Join one target's fresh findings or refusal to its accepted baseline.
    /// Compiler refusal is report data; report failure stops subsequent work.
    pub fn review_target(
        &mut self,
        target: TargetProfile,
        session: &mut PackageBuildSession,
        source_bindings: &mut PackageSourceBindings,
        execute: crate::review::PackageReviewExecutor,
    ) -> Result<(), PackageInspectionError> {
        if self.targets.get(self.completed_targets) != Some(&target) {
            return Err(failure(
                "inspection target does not match the next admitted target",
            ));
        }
        let baseline = self.accepted.as_ref().and_then(|lock| lock.target(target));
        let fresh = match &self.storage {
            Ok(storage) => sources::review(
                self.transaction.project_root(),
                target,
                baseline,
                storage.as_resolver(),
                self.transaction.options.offline,
                self.build_snapshot.as_ref(),
                session,
                source_bindings,
                execute,
            ),
            Err(error) => Err(failure(error)),
        };
        let available = match &fresh {
            Ok((source, reviews, changes)) => {
                self.outcome.requires_decision |= changes.requires_decision();
                Some((source, reviews, changes))
            }
            Err(_) => {
                self.outcome.complete = false;
                None
            }
        };
        let unavailable = fresh.as_ref().err().map(ToString::to_string);
        let section = report::render(
            target,
            baseline,
            available,
            unavailable.as_deref(),
            MAXIMUM_REPORT_BYTES.saturating_sub(self.outcome.report.len()),
            self.transaction.options.details,
        )
        .map_err(failure)?;
        self.outcome
            .report
            .try_reserve(section.len())
            .map_err(failure)?;
        self.outcome.report.push_str(&section);
        self.completed_targets += 1;
        Ok(())
    }

    /// A report cannot claim completion for unvisited targets or changed files.
    pub fn finish(self) -> Result<PackageInspectionOutcome, PackageInspectionError> {
        if self.completed_targets != self.targets.len() {
            return Err(failure(
                "inspection requires findings for every admitted target",
            ));
        }
        let after = self.transaction.files.read_pair().map_err(failure)?;
        if after != self.before {
            return Err(failure(
                "project build.omg or omega.lock changed during inspection; rerun the command",
            ));
        }
        Ok(self.outcome)
    }
}

fn select_targets(
    mut requested: Vec<TargetProfile>,
    accepted: Option<&PackageLock>,
) -> Result<Vec<TargetProfile>, PackageInspectionError> {
    if requested.is_empty() {
        return match accepted {
            Some(lock) => Ok(lock
                .targets()
                .iter()
                .map(PackageLockTarget::target)
                .collect()),
            // An unlocked project defaults to the host profile. A host with no
            // catalogued deployment profile (macOS x86-64) earns the same
            // ordinary diagnostic the other targetless commands report rather
            // than the panic `TargetProfile::host` retains for callers that
            // intrinsically require it.
            None => match TargetProfile::host_if_supported() {
                Some(host) => Ok(vec![host]),
                None => Err(failure(
                    "no inspection target was requested and this host has no catalogued \
                     Omega deployment profile; name an exact target with --target",
                )),
            },
        };
    }
    requested.sort_by_key(|target| target.identity().as_str());
    if requested.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(failure("duplicate inspection target"));
    }
    // An unaccepted target must be reviewed explicitly through update. Do not
    // silently resolve a new source graph when a lock is present.
    for target in &requested {
        if accepted.is_some_and(|lock| lock.target(*target).is_none()) {
            return Err(failure(format!(
                "omega.lock has no accepted target {}; use omega update --target {} for fresh review; no selector was refreshed",
                target.target_name(),
                target.target_name()
            )));
        }
    }
    Ok(requested)
}

/// Resolver storage belongs to this inspection, or is borrowed from its caller.
enum InspectionStorage<'storage> {
    Supplied(&'storage SourceResolver),
    Opened(SourceResolver),
}

impl InspectionStorage<'_> {
    fn as_resolver(&self) -> &SourceResolver {
        match self {
            Self::Supplied(storage) => storage,
            Self::Opened(storage) => storage,
        }
    }
}
