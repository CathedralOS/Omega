//! Fresh project inspection without accepting or publishing package changes.

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

/// Package-owned admission and report state. Application Build executes the
/// target roster; this owner compares findings and checks final file identity.
pub struct PreparedPackageInspection<'inspection> {
    identity: Arc<InspectionIdentity>,
    transaction: &'inspection PackageInspectionTransaction,
    before: (Vec<u8>, Option<Vec<u8>>),
    accepted: Option<PackageLock>,
    targets: Vec<TargetProfile>,
    storage: Result<InspectionStorage<'inspection>, PackageInspectionError>,
    build_snapshot: Option<build_evaluation::BuildSnapshotRequest>,
    completed_targets: usize,
    outcome: PackageInspectionOutcome,
}

struct InspectionIdentity;

// Full normalized findings include all authored target members. Bound aggregate
// reporting without truncating contracts or dropping inactive deployments.
const MAXIMUM_REPORT_BYTES: usize = 64 * 1024 * 1024;

impl PreparedPackageInspection<'_> {
    pub fn targets(&self) -> &[TargetProfile] {
        &self.targets
    }

    pub fn acquire_root(&self) -> Result<AcquiredBuildSource, PackageInspectionError> {
        AcquiredBuildSource::capture_local(
            self.transaction.project_root(),
            LocalSourceLimits::default(),
            ExternalSourceContext::derive(super::prepare_project::LOCAL_PROJECT_CONTEXT),
        )
        .map_err(failure)
    }

    pub fn build_snapshot(&self) -> Option<&build_evaluation::BuildSnapshotRequest> {
        self.build_snapshot.as_ref()
    }

    /// Every recovered target shares the same source graph. Acquire its exact
    /// Git pins once; Product review still checks each target's own baseline.
    pub fn source_acquisitions(
        &self,
    ) -> Result<BuildSourceAcquisitions<'_>, PackageInspectionError> {
        let storage = self.storage.as_ref().map_err(failure)?.as_resolver();
        let source = self
            .accepted
            .as_ref()
            .and_then(|lock| lock.targets().first())
            .map(PackageLockTarget::source);
        let context = ExternalSourceContext::derive(super::prepare_project::LOCAL_PROJECT_CONTEXT);
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
            source_context: ExternalSourceContext::derive(
                super::prepare_project::LOCAL_PROJECT_CONTEXT,
            ),
        }
    }

    /// Bind the shared executed closure to this observational transaction.
    /// This admits no publication and changes neither proposals nor decisions.
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

    /// Join one target's fresh findings or refusal to its accepted baseline.
    /// Compiler refusal is report data; report failure stops subsequent work.
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
