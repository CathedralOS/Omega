/*
Package command admission, staging, review reconciliation, and publication (`install`, `update`, `--resume`, and `--discard-review`).

`PackageCommandTransaction` holds the project file transaction lock and review-file root
across application-driven Build execution and target review passes. `prepare` stages the
proposed `build.omg` edit into a private snapshot and retains any recovered proposal's
exact root build-input selection so `--resume` never silently widens back to the full
package inventory.
*/

/*
`PreparedPackageCommand::complete` verifies that every requested target returned a review
bound to this exact command admission before writing `proposal` or decision documents.
When any required policy change remains unresolved or rejected, accepted project files
stay untouched (`ReviewRequired`); once all changes are accepted, the build session closes
before `publish_reviewed_package_change` commits `build.omg` and `omega.lock`.
*/

mod change_decisions;
mod planning;
mod proposal;
mod source_review;
mod state;

use crate::build_session::{PackageBuildSession, PackageSourceBindings};
use crate::declarations::{BuildFileReplacement, PackageKey};
use crate::digest_encoder::content_identity;
use crate::lock::{PackageLock, PackageLockRecoveryLimits, PackageLockTarget};
use crate::operations::{
    LOCAL_PROJECT_CONTEXT, PackageChangeReview, PackageFileTransaction, PackagePublicationLimits,
    publish_reviewed_package_change, review_package_change_reusing, stage_build_dependency_edit,
};
use crate::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits, GitDependencyPins,
    GitResolutionOptions, PackageRootSourceRequest, ResolvedPackageSourceClosure,
};
use crate::resolution::source::{
    AcquiredBuildSource, BuildSourceAcquisitions, ResolvedBuildSources,
};
use crate::review::{CompileResolvedPackageReviewsError, PackageReviewExecutor};
use package_source::PrimaryGitChoices;
use package_source::git::resolution::GitExactRevisionAcquisition;
use package_source::local::staging::StagedLocalSnapshot;
use package_source::{ExternalSourceContext, LocalSourceLimits, SourceResolver};
use platform_custody::record_file::{RecordFileRoot, RootRecordRead};
use proposal::PendingPackageChange;
use state::{LIMITS, PROPOSAL, discard, file_failure, read, text, write_proposal};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use target::TargetProfile;

pub struct PackageCommandTransaction {
    transaction: PackageFileTransaction,
    files: RecordFileRoot,
    options: PackageCommandOptions,
}

impl PackageCommandTransaction {
    pub fn open(options: PackageCommandOptions) -> Result<Self, PackageCommandError> {
        let mut transaction = PackageFileTransaction::open(
            &options.project_root,
            PackagePublicationLimits::default(),
        )
        .map_err(failure)?;
        transaction.recover().map_err(failure)?;
        let files = transaction.command_state_files().map_err(failure)?;
        Ok(Self {
            transaction,
            files,
            options,
        })
    }

    pub fn prepare<'workspace, 'storage>(
        &'workspace mut self,
        command: PackageCommand,
        storage: Option<&'storage SourceResolver>,
    ) -> Result<PackageCommandPreparation<'workspace, 'storage>, PackageCommandError> {
        let options = &self.options;
        if options.build_inputs.is_some()
            && matches!(
                command,
                PackageCommand::Resume { .. } | PackageCommand::DiscardReview
            )
        {
            return Err(failure(
                "--resume and --discard-review cannot override the pending input inventory",
            ));
        }
        let transaction = &mut self.transaction;
        if matches!(command, PackageCommand::DiscardReview) {
            return Err(failure("discard does not require a review command"));
        }
        let storage = match storage {
            Some(storage) => CommandStorage::Supplied(storage),
            None => CommandStorage::Opened(
                SourceResolver::for_current_user(PrimaryGitChoices {
                    excluded_controlled_roots: &[transaction.project_root().to_path_buf()],
                    ..PrimaryGitChoices::default()
                })
                .map_err(failure)?,
            ),
        };
        let PackageCommandOptions {
            targets: requested_targets,
            offline,
            build_inputs,
            ..
        } = options.clone();
        let files = &self.files;
        let pending_file = read(files, PROPOSAL)?;
        let (before_build, before_lock) = transaction.read_pair().map_err(failure)?;
        let before = std::str::from_utf8(&before_build)
            .map_err(|_| failure("build.omg is not UTF-8 Omega source"))?;
        let accepted_text = before_lock.as_deref().map(std::str::from_utf8).transpose()
        .map_err(|_| failure("omega.lock is not UTF-8; restore a supported lock or explicitly remove it for fresh graph review"))?;
        let accepted = accepted_text.map(|text| PackageLock::recover_review_baseline(text, PackageLockRecoveryLimits::default())).transpose()
        .map_err(|error| failure(format!("cannot read accepted omega.lock: {error}; restore a compatible lock or explicitly remove it and run omega packages update for fresh review; no revision was refreshed")))?;
        let resume = matches!(command, PackageCommand::Resume { .. });
        if !resume && pending_file.is_some() {
            return Err(failure(
                "a package review is pending; use --resume, or --discard-review to abandon that proposal before starting another command",
            ));
        }
        let (plan, targets, recovered) = match command {
            PackageCommand::Install {
                source,
                revision,
                alias,
                package,
            } => (
                planning::install(
                    source,
                    revision,
                    alias,
                    package,
                    transaction.project_root(),
                    before,
                )?,
                targets(requested_targets, accepted.as_ref())?,
                None,
            ),
            PackageCommand::Update { packages, revision } => (
                planning::update(
                    packages,
                    revision,
                    transaction.project_root(),
                    before,
                    accepted.as_ref(),
                )?,
                targets(requested_targets, accepted.as_ref())?,
                None,
            ),
            PackageCommand::Resume { kind } => {
                if !requested_targets.is_empty() {
                    return Err(failure("--resume uses the pending proposal's targets"));
                }
                let file = pending_file.as_ref().ok_or_else(|| {
                    failure("no pending package review; start an install or update first")
                })?;
                let proposal = PendingPackageChange::recover(text(file)?).map_err(failure)?;
                if proposal.kind != kind {
                    return Err(failure(
                        "pending proposal belongs to the other package command",
                    ));
                }
                if proposal.before_build != digest(&before_build)
                    || proposal.before_lock != before_lock.as_deref().map(digest)
                {
                    return Err(failure(
                        "build.omg or omega.lock changed since the proposal; use --discard-review and start a fresh command",
                    ));
                }
                (
                    planning::Plan {
                        kind,
                        replacement: BuildFileReplacement::from_sources(
                            transaction.project_root().join("build.omg"),
                            before,
                            proposal.proposed_build.clone(),
                        ),
                        updates: Some(Vec::new()),
                    },
                    proposal.targets.clone(),
                    Some(proposal),
                )
            }
            PackageCommand::DiscardReview => {
                return Err(failure("discard does not require a review command"));
            }
        };
        let planning::Plan {
            kind,
            replacement,
            updates,
        } = plan;
        let build_inputs = recovered
            .as_ref()
            .map_or(build_inputs, |proposal| proposal.build_inputs.clone());
        let build_snapshot = build_inputs
            .clone()
            .map(|inputs| build_settings::BuildSnapshotRequest::scoped(Vec::new(), inputs));
        let stage = stage_build_dependency_edit(
            &replacement,
            storage.resolver(),
            LocalSourceLimits::default(),
            build_inputs.as_ref(),
        )
        .map_err(failure)?;
        if recovered.as_ref().is_some_and(|proposal| {
            proposal.original_content != digest(stage.original().content_identity.as_bytes())
        }) {
            return Err(failure(
                "project source changed since the proposal; discard it and review a fresh candidate",
            ));
        }
        let acquisition = if offline {
            GitExactRevisionAcquisition::Offline
        } else {
            GitExactRevisionAcquisition::AllowFetch
        };
        Ok(PackageCommandPreparation {
            identity: Arc::new(CommandReviewIdentity),
            transaction,
            files,
            pending_file,
            storage,
            acquisition,
            before_lock,
            accepted,
            replacement,
            stage,
            targets,
            build_snapshot,
            resume,
            kind,
            before_build,
            build_inputs,
            recovered,
            updates,
            offline,
        })
    }
}

impl<'workspace, 'storage> PackageCommandPreparation<'workspace, 'storage> {
    pub fn acquire_root(&self) -> Result<AcquiredBuildSource, PackageCommandError> {
        AcquiredBuildSource::capture_staged(
            &self.stage,
            ExternalSourceContext::derive(LOCAL_PROJECT_CONTEXT),
        )
        .map_err(failure)
    }

    pub fn build_snapshot(&self) -> Option<&build_settings::BuildSnapshotRequest> {
        self.build_snapshot.as_ref()
    }

    pub fn source_acquisitions(&self) -> Result<BuildSourceAcquisitions<'_>, PackageCommandError> {
        let preserved = self
            .recovered
            .as_ref()
            .map(|proposal| &proposal.source)
            .or_else(|| {
                self.accepted
                    .as_ref()
                    .map(|lock| lock.targets()[0].source())
            });
        if let Some(source) = preserved.filter(|_| self.updates.is_some()) {
            let matches_root = matches!(source.root().request(),
                crate::resolution::graph::CanonicalRootSourceRequest::ExternalLocal { requested_root, source_context }
                if requested_root == self.stage.requested_root().as_os_str().as_encoded_bytes()
                    && source_context == &ExternalSourceContext::derive(LOCAL_PROJECT_CONTEXT));
            if !matches_root {
                return Err(failure("accepted Git pins belong to another root request"));
            }
        }
        let pins = preserved
            .zip(self.updates.as_deref())
            .map(|(subject, updates)| GitDependencyPins::new(subject, updates, self.acquisition))
            .transpose()
            .map_err(failure)?;
        Ok(BuildSourceAcquisitions::new(
            self.storage.resolver(),
            GitResolutionOptions {
                pins,
                offline: self.offline,
            },
            self.root_request(),
        ))
    }

    fn root_request(&self) -> PackageRootSourceRequest {
        PackageRootSourceRequest::ExternalLocal {
            requested_root: self.stage.requested_root().to_path_buf(),
            source_context: ExternalSourceContext::derive(LOCAL_PROJECT_CONTEXT),
        }
    }

    pub fn complete_source_resolution(
        self,
        resolved: ResolvedBuildSources,
    ) -> Result<PreparedPackageCommand<'workspace, 'storage>, PackageCommandError> {
        let closure = resolved.into_source_closure();
        let root = closure
            .custody(closure.graph().root())
            .ok_or_else(|| failure("resolved command has no root source custody"))?;
        if closure.source_requests().root().request() != &self.root_request()
            || root.snapshot_root() != self.stage.snapshot_root()
            || root.resolution()
                != &package_source::ImmutableSourceResolution::external_local(
                    package_source::SourceContentDigest::derive(
                        self.stage.normalized().content_identity.as_bytes(),
                    ),
                )
        {
            return Err(failure(
                "resolved command sources do not belong to this staged proposal",
            ));
        }
        let source = CanonicalSourceClosureSubject::from_resolved(
            &closure.for_exact_target(self.targets[0]),
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .map_err(failure)?;
        if self
            .recovered
            .as_ref()
            .is_some_and(|proposal| proposal.source != source)
        {
            return Err(failure(
                "candidate sources or dependency graph changed since the proposal; discard it and review a fresh candidate",
            ));
        }
        let proposal = if !self.resume {
            Some(PendingPackageChange {
                kind: self.kind,
                before_build: digest(&self.before_build),
                before_lock: self.before_lock.as_deref().map(digest),
                original_content: digest(self.stage.original().content_identity.as_bytes()),
                proposed_build: self.replacement.replacement_source().to_owned(),
                source,
                targets: self.targets.clone(),
                build_inputs: self.build_inputs.clone(),
            })
        } else {
            None
        };
        Ok(PreparedPackageCommand {
            preparation: self,
            closure,
            proposal,
        })
    }
}

enum CommandStorage<'storage> {
    Supplied(&'storage SourceResolver),
    Opened(SourceResolver),
}

impl CommandStorage<'_> {
    fn resolver(&self) -> &SourceResolver {
        match self {
            Self::Supplied(storage) => storage,
            Self::Opened(storage) => storage,
        }
    }
}

pub struct PackageCommandPreparation<'workspace, 'storage> {
    identity: Arc<CommandReviewIdentity>,
    transaction: &'workspace mut PackageFileTransaction,
    files: &'workspace RecordFileRoot,
    pending_file: Option<RootRecordRead<'workspace>>,
    storage: CommandStorage<'storage>,
    acquisition: GitExactRevisionAcquisition,
    before_lock: Option<Vec<u8>>,
    accepted: Option<PackageLock>,
    replacement: BuildFileReplacement,
    stage: StagedLocalSnapshot,
    targets: Vec<TargetProfile>,
    build_snapshot: Option<package_compilation::BuildSnapshotRequest>,
    resume: bool,
    kind: PackageCommandKind,
    before_build: Vec<u8>,
    build_inputs: Option<package_compilation::BuildSourceCaptureRequest>,
    recovered: Option<PendingPackageChange>,
    updates: Option<Vec<PackageKey>>,
    offline: bool,
}

pub struct PreparedPackageCommand<'workspace, 'storage> {
    preparation: PackageCommandPreparation<'workspace, 'storage>,
    closure: ResolvedPackageSourceClosure,
    proposal: Option<PendingPackageChange>,
}

pub struct PackageCommandReviewRequest<'request> {
    identity: &'request Arc<CommandReviewIdentity>,
    source_closure: &'request ResolvedPackageSourceClosure,
    target: TargetProfile,
    accepted: Option<&'request PackageLockTarget>,
    build_snapshot: Option<&'request package_compilation::BuildSnapshotRequest>,
}

pub struct PackageCommandReview {
    identity: Arc<CommandReviewIdentity>,
    review: PackageChangeReview,
}

struct CommandReviewIdentity;

impl PackageCommandReviewRequest<'_> {
    pub fn review(
        self,
        session: &mut PackageBuildSession,
        source_bindings: &mut PackageSourceBindings,
        execute: &mut PackageReviewExecutor<'_>,
    ) -> Result<PackageCommandReview, PackageCommandError> {
        let review = review_package_change_reusing(
            self.source_closure.clone(),
            self.target,
            self.accepted,
            session,
            self.build_snapshot,
            source_bindings,
            execute,
        )
        .map_err(PackageCommandError::from_package_change)?;
        Ok(PackageCommandReview {
            identity: Arc::clone(self.identity),
            review,
        })
    }
}

impl PreparedPackageCommand<'_, '_> {
    pub fn source_bindings(&self) -> PackageSourceBindings {
        PackageSourceBindings::for_closure(&self.closure)
    }

    pub fn review_requests(&self) -> impl Iterator<Item = PackageCommandReviewRequest<'_>> {
        self.preparation
            .targets
            .iter()
            .map(|&target| PackageCommandReviewRequest {
                identity: &self.preparation.identity,
                source_closure: &self.closure,
                target,
                accepted: self
                    .preparation
                    .accepted
                    .as_ref()
                    .and_then(|lock| lock.target(target)),
                build_snapshot: self.preparation.build_snapshot.as_ref(),
            })
    }

    pub fn complete(
        self,
        reviews: Vec<PackageCommandReview>,
        session: &mut PackageBuildSession,
    ) -> Result<PackageCommandOutcome, PackageCommandError> {
        if reviews.len() != self.preparation.targets.len() {
            return Err(failure(
                "package command did not receive every requested target review",
            ));
        }
        for (result, &target) in reviews.iter().zip(&self.preparation.targets) {
            if !Arc::ptr_eq(&self.preparation.identity, &result.identity) {
                return Err(failure(
                    "package command received a review from another command",
                ));
            }
            let review = &result.review;
            if review.target() != target {
                return Err(failure(
                    "package command received a review for the wrong target",
                ));
            }
        }
        let Self {
            preparation,
            closure,
            proposal,
        } = self;
        let PackageCommandPreparation {
            transaction,
            files,
            mut pending_file,
            storage,
            acquisition,
            before_lock,
            accepted,
            replacement,
            stage,
            resume,
            ..
        } = preparation;
        let reviews = reviews
            .into_iter()
            .map(|result| result.review)
            .collect::<Vec<_>>();
        let accepted_text = before_lock
            .as_deref()
            .map(std::str::from_utf8)
            .transpose()
            .map_err(|_| failure("accepted lock is not UTF-8"))?;
        stage.verify_live_source_unchanged().map_err(failure)?;
        let proposal_text = proposal
            .map(|proposal| proposal.encode())
            .transpose()
            .map_err(failure)?;
        let mut choices =
            change_decisions::prepare(files, transaction, &reviews, resume, accepted.is_none())?;
        choices.report.push_str(&source_review::prepare(
            files,
            transaction,
            accepted.as_ref(),
            &closure,
            storage.resolver(),
            acquisition,
        )?);
        if let Some(text) = proposal_text {
            write_proposal(files, &text)?;
            pending_file = read(files, PROPOSAL)?;
        }
        if let Some(file) = &mut pending_file {
            file.verify_current(LIMITS).map_err(file_failure)?;
        }
        for file in &mut choices.reads {
            file.verify_current(LIMITS).map_err(file_failure)?;
        }
        if choices.blocked {
            choices.report.push_str("\nAccepted project files are unchanged. Edit each pending decision to accept or reject, then rerun this command with --resume. Use --discard-review to abandon the proposal.");
            return Ok(PackageCommandOutcome {
                status: PackageCommandStatus::ReviewRequired,
                report: choices.report,
                review_paths: choices.paths,
            });
        }
        let paired = reviews
            .iter()
            .zip(choices.resolutions.iter())
            .collect::<Vec<_>>();
        session.close().map_err(PackageCommandError::from)?;
        let published = publish_reviewed_package_change(
            transaction,
            &replacement,
            &stage,
            &paired,
            accepted_text,
        )
        .map_err(failure)?;
        choices.report.push_str(&format!(
            "\nPublished build.omg and omega.lock for {} packages across {} targets.",
            published.targets()[0].source().packages().len(),
            published.targets().len()
        ));
        if let Some(file) = pending_file
            && let Err(error) = file.remove(LIMITS)
        {
            choices.report.push_str(&format!("\nAccepted files were published, but proposal cleanup failed: {}. Use --discard-review before starting another change.", file_failure(error)));
        }
        Ok(PackageCommandOutcome {
            status: PackageCommandStatus::Published,
            report: choices.report,
            review_paths: choices.paths,
        })
    }
}

pub fn discard_package_review(
    project_root: &Path,
) -> Result<PackageCommandOutcome, PackageCommandError> {
    let mut transaction =
        PackageFileTransaction::open(project_root, PackagePublicationLimits::default())
            .map_err(failure)?;
    transaction.recover().map_err(failure)?;
    discard(&transaction)
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    content_identity(bytes)
}

fn targets(
    mut requested: Vec<TargetProfile>,
    accepted: Option<&PackageLock>,
) -> Result<Vec<TargetProfile>, PackageCommandError> {
    if requested.len() > PackageLockRecoveryLimits::default().maximum_targets {
        return Err(failure("too many requested package targets"));
    }
    requested.sort_by_key(|target| target.target_name());
    if requested.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(failure("duplicate requested package target"));
    }
    if let Some(accepted) = accepted {
        for target in accepted.targets() {
            if !requested.contains(&target.target()) {
                requested.push(target.target());
            }
        }
    }
    if requested.is_empty() {
        match TargetProfile::host_if_supported() {
            Some(host) => requested.push(host),
            None => {
                return Err(failure(
                    "no package target was requested and this host has no catalogued \
                     Omega deployment profile; name an exact target with --target",
                ));
            }
        }
    }
    requested.sort_by_key(|target| target.target_name());
    Ok(requested)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageCommandKind {
    Install,
    Update,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageCommand {
    Install {
        source: String,
        revision: Option<String>,
        alias: Option<String>,
        package: Option<String>, // declared Git workspace package name; absence selects the root
    },
    Update {
        packages: Vec<String>,
        revision: Option<String>,
    },
    Resume {
        kind: PackageCommandKind,
    },
    DiscardReview,
}

#[derive(Debug, Clone)]
pub struct PackageCommandOptions {
    pub project_root: PathBuf,
    pub targets: Vec<TargetProfile>, // explicit targets augment every previously accepted target
    pub offline: bool, // use cached recorded Git pins only; never fetch or refresh a selector
    pub build_inputs: Option<package_compilation::BuildSourceCaptureRequest>, // root-only immutable input selection for a new install/update
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageCommandStatus {
    Published,
    ReviewRequired,
    ReviewDiscarded,
}

#[derive(Debug)]
pub struct PackageCommandOutcome {
    pub status: PackageCommandStatus,
    pub report: String,
    pub review_paths: Vec<PathBuf>,
}

#[derive(Debug)]
pub struct PackageCommandError {
    cause: PackageCommandCause,
    accounting: compilation_products::SourceCompletionAccounting,
}

#[derive(Debug)]
enum PackageCommandCause {
    Message(String),
    Review(Box<CompileResolvedPackageReviewsError>),
    Change(Box<crate::operations::PackageChangeError>),
    Compilation(compilation_products::CompileFailure),
    Preparation(Box<crate::operations::PrepareLocalProjectError>),
}

impl PackageCommandError {
    pub fn source_completion_accounting(
        &self,
    ) -> &compilation_products::SourceCompletionAccounting {
        &self.accounting
    }

    pub fn with_source_completion_accounting(
        mut self,
        accounting: compilation_products::SourceCompletionAccounting,
    ) -> Self {
        self.accounting.append(accounting);
        self
    }

    pub fn compilation_failure(&self) -> Option<&compilation_products::CompileFailure> {
        match &self.cause {
            PackageCommandCause::Compilation(failure) => Some(failure),
            _ => None,
        }
    }

    pub fn review_failure(&self) -> Option<&CompileResolvedPackageReviewsError> {
        match &self.cause {
            PackageCommandCause::Review(failure) => Some(failure),
            PackageCommandCause::Change(failure) => match failure.as_ref() {
                crate::operations::PackageChangeError::Compilation(error) => Some(error),
                _ => None,
            },
            _ => None,
        }
    }
}

impl From<CompileResolvedPackageReviewsError> for PackageCommandError {
    fn from(error: CompileResolvedPackageReviewsError) -> Self {
        let accounting = error
            .source_completion_accounting()
            .cloned()
            .unwrap_or_default();
        Self {
            cause: PackageCommandCause::Review(Box::new(error)),
            accounting,
        }
    }
}

impl PackageCommandError {
    fn from_package_change(error: crate::operations::PackageChangeError) -> Self {
        let accounting = match &error {
            crate::operations::PackageChangeError::Compilation(error) => error
                .source_completion_accounting()
                .cloned()
                .unwrap_or_default(),
            _ => Default::default(),
        };
        Self {
            cause: PackageCommandCause::Change(Box::new(error)),
            accounting,
        }
    }
}

impl From<compilation_products::CompileFailure> for PackageCommandError {
    fn from(error: compilation_products::CompileFailure) -> Self {
        let accounting = error
            .source_completion_accounting()
            .cloned()
            .unwrap_or_default();
        Self {
            cause: PackageCommandCause::Compilation(error),
            accounting,
        }
    }
}

impl From<crate::operations::PrepareLocalProjectError> for PackageCommandError {
    fn from(error: crate::operations::PrepareLocalProjectError) -> Self {
        Self {
            cause: PackageCommandCause::Preparation(Box::new(error)),
            accounting: Default::default(),
        }
    }
}

impl From<String> for PackageCommandError {
    fn from(error: String) -> Self {
        Self {
            cause: PackageCommandCause::Message(error),
            accounting: Default::default(),
        }
    }
}

impl fmt::Display for PackageCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.cause {
            PackageCommandCause::Message(message) => message.fmt(formatter),
            PackageCommandCause::Review(error) => error.fmt(formatter),
            PackageCommandCause::Change(error) => error.fmt(formatter),
            PackageCommandCause::Preparation(error) => error.fmt(formatter),
            PackageCommandCause::Compilation(failure) => diagnostics::write_bounded_diagnostics(
                formatter,
                &failure.clone().into_diagnostics(),
            ),
        }
    }
}
impl std::error::Error for PackageCommandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.cause {
            PackageCommandCause::Review(error) => Some(error.as_ref()),
            PackageCommandCause::Change(error) => Some(error.as_ref()),
            PackageCommandCause::Preparation(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

fn failure(message: impl fmt::Display) -> PackageCommandError {
    PackageCommandError::from(message.to_string())
}
