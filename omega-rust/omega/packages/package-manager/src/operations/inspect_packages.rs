/*
Observational project package inspection (`omega packages`).

`PackageInspectionTransaction` holds the original project file transaction throughout
inspection without recovering pending publications or altering proposal files.
`PreparedPackageInspection` acquires the shared source graph once across all admitted
targets, compares each target's fresh compiler findings against its accepted baseline
in `omega.lock`, and verifies at `finish` that neither `build.omg` nor `omega.lock`
changed while inspection ran.
*/

use super::prepare_project::LOCAL_PROJECT_CONTEXT;
mod report;
mod sources;

use super::{PackageFileTransaction, PackagePublicationLimits};
use crate::build_session::{PackageBuildSession, PackageSourceBindings};
use crate::lock::{PackageLock, PackageLockRecoveryLimits, PackageLockTarget};
use crate::resolution::graph::{
    CanonicalRootSourceRequest, GitDependencyPins, GitResolutionOptions, PackageRootSourceRequest,
};
use crate::resolution::source::{
    AcquiredBuildSource, BuildSourceAcquisitions, ResolvedBuildSources,
};
use crate::review::CompileResolvedPackageReviewsError;
use package_source::PrimaryGitChoices;
use package_source::SourceResolver;
use package_source::git::resolution::GitExactRevisionAcquisition;
use package_source::{ExternalSourceContext, LocalSourceLimits};
pub use sources::PackageInspectionSources;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use target::TargetProfile;

#[derive(Debug, Clone)]
pub struct PackageInspectionOptions {
    pub project_root: PathBuf,
    pub targets: Vec<TargetProfile>, // empty selects every accepted target, or the host for an unlocked project
    pub details: bool, // include full compiler-owned normalized policy after the readable summary
    pub offline: bool, // restrict Git acquisition to cached exact pins, without selector refresh
    pub build_inputs: Option<package_compilation::BuildSourceCaptureRequest>, // caller-selected immutable root inputs relative to the project source root
}

#[derive(Debug)]
pub struct PackageInspectionOutcome {
    pub report: String,
    pub complete: bool, // false means at least one target has no fresh compiler findings
    pub requires_decision: bool, // inspection reports required changes but never resolves their decisions
}

#[derive(Debug)]
pub struct PackageInspectionError(String);

diagnostics::string_diagnostic!(PackageInspectionError);

impl From<String> for PackageInspectionError {
    fn from(message: String) -> Self {
        Self(message)
    }
}

fn failure(error: impl fmt::Display) -> PackageInspectionError {
    PackageInspectionError(error.to_string())
}

impl From<CompileResolvedPackageReviewsError> for PackageInspectionError {
    fn from(error: CompileResolvedPackageReviewsError) -> Self {
        failure(error)
    }
}

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
            .map(|inputs| build_settings::BuildSnapshotRequest::scoped(Vec::new(), inputs));
        Ok(PreparedPackageInspection {
            identity: Arc::new(InspectionIdentity),
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

pub struct PreparedPackageInspection<'inspection> {
    identity: Arc<InspectionIdentity>,
    transaction: &'inspection PackageInspectionTransaction,
    before: (Vec<u8>, Option<Vec<u8>>),
    accepted: Option<PackageLock>,
    targets: Vec<TargetProfile>,
    storage: Result<InspectionStorage<'inspection>, PackageInspectionError>,
    build_snapshot: Option<build_settings::BuildSnapshotRequest>,
    completed_targets: usize,
    outcome: PackageInspectionOutcome,
}

struct InspectionIdentity;

const MAXIMUM_REPORT_BYTES: usize = 64 * 1024 * 1024;

impl PreparedPackageInspection<'_> {
    pub fn targets(&self) -> &[TargetProfile] {
        &self.targets
    }

    pub fn acquire_root(&self) -> Result<AcquiredBuildSource, PackageInspectionError> {
        let entire = package_source::SourceCaptureRequest::entire_tree();
        let selected = match self
            .build_snapshot
            .as_ref()
            .map(|snapshot| snapshot.capture())
        {
            Some(build_settings::BuildSnapshotCapture::Scoped(selected)) => selected,
            Some(build_settings::BuildSnapshotCapture::PackageInventory) | None => &entire,
        };
        let acquired = AcquiredBuildSource::capture_selected_local(
            self.transaction.project_root(),
            LocalSourceLimits::default(),
            ExternalSourceContext::derive(LOCAL_PROJECT_CONTEXT),
            selected,
        )
        .map_err(failure)?;
        acquired
            .require_compilation_source(Path::new("build.omg"))
            .map_err(failure)?;
        Ok(acquired)
    }

    pub fn build_snapshot(&self) -> Option<&build_settings::BuildSnapshotRequest> {
        self.build_snapshot.as_ref()
    }

    pub fn source_acquisitions(
        &self,
    ) -> Result<BuildSourceAcquisitions<'_>, PackageInspectionError> {
        let storage = self.storage.as_ref().map_err(failure)?.as_resolver();
        let source = self
            .accepted
            .as_ref()
            .and_then(|lock| lock.targets().first())
            .map(PackageLockTarget::source);
        let context = ExternalSourceContext::derive(LOCAL_PROJECT_CONTEXT);
        if let Some(source) = source
            && !matches!(source.root().request(), CanonicalRootSourceRequest::ExternalLocal { source_context, .. } if source_context == &context)
        {
            return Err(failure(
                "accepted inspection pins belong to another source context",
            ));
        }
        let acquisition = if self.transaction.options.offline {
            GitExactRevisionAcquisition::Offline
        } else {
            GitExactRevisionAcquisition::AllowFetch
        };
        let pins = source
            .map(|source| GitDependencyPins::new(source, &[], acquisition))
            .transpose()
            .map_err(failure)?;
        let mut acquisitions = BuildSourceAcquisitions::new(
            storage,
            GitResolutionOptions {
                pins,
                offline: self.transaction.options.offline,
            },
            self.root_request(),
        );
        if let Some(source) = source {
            acquisitions.require_locked_graph(source);
        }
        Ok(acquisitions)
    }

    fn root_request(&self) -> PackageRootSourceRequest {
        PackageRootSourceRequest::ExternalLocal {
            requested_root: self.transaction.project_root().to_path_buf(),
            source_context: ExternalSourceContext::derive(LOCAL_PROJECT_CONTEXT),
        }
    }

    pub fn complete_source_resolution(
        &self,
        resolved: ResolvedBuildSources,
    ) -> Result<PackageInspectionSources, PackageInspectionError> {
        let closure = resolved.into_source_closure();
        if closure.source_requests().root().request() != &self.root_request() {
            return Err(failure(
                "executed inspection sources belong to another project",
            ));
        }
        Ok(PackageInspectionSources {
            closure,
            identity: Arc::clone(&self.identity),
        })
    }

    pub fn review_target(
        &mut self,
        target: TargetProfile,
        sources: Result<&PackageInspectionSources, &PackageInspectionError>,
        session: &mut PackageBuildSession,
        source_bindings: &mut PackageSourceBindings,
        execute: &mut crate::review::PackageReviewExecutor<'_>,
    ) -> Result<(), PackageInspectionError> {
        if self.targets.get(self.completed_targets) != Some(&target) {
            return Err(failure(
                "inspection target does not match the next admitted target",
            ));
        }
        if let Ok(sources) = sources
            && !Arc::ptr_eq(&sources.identity, &self.identity)
        {
            return Err(failure("resolved sources belong to another inspection"));
        }
        let baseline = self.accepted.as_ref().and_then(|lock| lock.target(target));
        let fresh = match sources {
            Ok(sources) => sources::review(
                sources,
                target,
                baseline,
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
    for target in &requested {
        if accepted.is_some_and(|lock| lock.target(*target).is_none()) {
            return Err(failure(format!(
                "omega.lock has no accepted target {}; use omega packages update --target {} for fresh review; no selector was refreshed",
                target.target_name(),
                target.target_name()
            )));
        }
    }
    Ok(requested)
}

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
