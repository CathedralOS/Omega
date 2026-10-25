//! Package command admission and publication around application-owned work.
//!
//! [`PackageCommandTransaction`] retains file custody while application Build
//! reviews the admitted target requests. Completing the command joins all
//! findings before writing decisions or publishing the accepted pair.

mod change_decisions;
mod planning;
mod proposal;
mod source_review;
mod state;

use crate::build_session::{PackageBuildSession, PackageSourceBindings};
use crate::declarations::BuildFileReplacement;
use crate::lock::{PackageLock, PackageLockRecoveryLimits};
use crate::operations::{
    PackageChangeReview, PackageFileTransaction, PackagePublicationLimits,
    publish_reviewed_package_change, stage_build_dependency_edit,
};
use crate::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits, GitDependencyPins,
    GitResolutionOptions, PackageSourceClosureLimits, ResolvedPackageSourceClosure,
    resolve_staged_external_local_project_closure,
};
use crate::review::CompileResolvedPackageReviewsError;
use package_source::PrimaryGitChoices;
use package_source::git::resolution::GitExactRevisionAcquisition;
use package_source::local::staging::StagedLocalSnapshot;
use package_source::{ExternalSourceContext, LocalSourceLimits, SourceResolver};
use platform_custody::record_file::{RecordFileRoot, RootRecordRead};
use proposal::PendingPackageChange;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use target::TargetProfile;

/// Keep the publication lock and review-file root alive across compiler work.
/// No compiler work or resource grant is created by opening this transaction.
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

    /// Resolve one install/update/resume without invoking the compiler. The
    /// returned command borrows this transaction and its original proposal
    /// file handle until all target findings have been reconciled.
    pub fn prepare<'workspace, 'storage>(
        &'workspace mut self,
        command: PackageCommand,
        storage: Option<&'storage SourceResolver>,
    ) -> Result<PreparedPackageCommand<'workspace, 'storage>, PackageCommandError> {
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
        let pending_file = state::read(files, state::PROPOSAL)?;
        let (before_build, before_lock) = transaction.read_pair().map_err(failure)?;
        let before = std::str::from_utf8(&before_build)
            .map_err(|_| failure("build.omg is not UTF-8 Omega source"))?;
        let accepted_text = before_lock.as_deref().map(std::str::from_utf8).transpose()
        .map_err(|_| failure("omega.lock is not UTF-8; restore a supported lock or explicitly remove it for fresh graph review"))?;
        let accepted = accepted_text.map(|text| PackageLock::recover_review_baseline(text, PackageLockRecoveryLimits::default())).transpose()
        .map_err(|error| failure(format!("cannot read accepted omega.lock: {error}; restore a compatible lock or explicitly remove it and run omega update for fresh review; no revision was refreshed")))?;
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
                let proposal =
                    PendingPackageChange::recover(state::text(file)?).map_err(failure)?;
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
                        )
                        .map_err(failure)?,
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
        // Resuming repeats fresh capture and checking, but it must not silently
        // fall back to the complete package inventory. Proposal data retains the
        // caller's selection, not a completed build or an acceptance grant.
        let build_inputs = recovered
            .as_ref()
            .map_or(build_inputs, |proposal| proposal.build_inputs.clone());
        let build_snapshot = build_inputs
            .clone()
            .map(|inputs| build_evaluation::BuildSnapshotRequest::scoped(Vec::new(), inputs));
        let stage = stage_build_dependency_edit(
            &replacement,
            storage.resolver(),
            LocalSourceLimits::default(),
        )
        .map_err(failure)?;
        if recovered.as_ref().is_some_and(|proposal| {
            proposal.original_content != digest(stage.original().content_identity.as_bytes())
        }) {
            return Err(failure(
                "project source changed since the proposal; discard it and review a fresh candidate",
            ));
        }
        let preserved_subject = recovered
            .as_ref()
            .map(|proposal| &proposal.source)
            .or_else(|| accepted.as_ref().map(|lock| lock.targets()[0].source()));
        let context = ExternalSourceContext::derive(crate::operations::LOCAL_PROJECT_CONTEXT);
        let acquisition = if offline {
            GitExactRevisionAcquisition::Offline
        } else {
            GitExactRevisionAcquisition::AllowFetch
        };
        let pins = preserved_subject
            .zip(updates.as_deref())
            .map(|(subject, updates)| GitDependencyPins::new(subject, updates, acquisition))
            .transpose()
            .map_err(failure)?;
        let closure = resolve_staged_external_local_project_closure(
            &stage,
            context,
            storage.resolver(),
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
            GitResolutionOptions { pins, offline },
        )
        .map_err(failure)?;
        let source = CanonicalSourceClosureSubject::from_resolved(
            &closure.for_exact_target(targets[0]),
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .map_err(failure)?;
        if recovered
            .as_ref()
            .is_some_and(|proposal| proposal.source != source)
        {
            return Err(failure(
                "candidate sources or dependency graph changed since the proposal; discard it and review a fresh candidate",
            ));
        }
        let proposal = if !resume {
            Some(PendingPackageChange {
                kind,
                before_build: digest(&before_build),
                before_lock: before_lock.as_deref().map(digest),
                original_content: digest(stage.original().content_identity.as_bytes()),
                proposed_build: replacement.replacement_source().to_owned(),
                source,
                targets: targets.clone(),
                build_inputs,
            })
        } else {
            None
        };
        Ok(PreparedPackageCommand {
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
            closure,
            targets,
            build_snapshot,
            proposal,
            resume,
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

/// An admitted command with source and file custody retained through review.
/// Private fields prevent callers from replacing its accepted baseline or
/// proposal while compiler work is in flight.
pub struct PreparedPackageCommand<'workspace, 'storage> {
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
    closure: ResolvedPackageSourceClosure,
    targets: Vec<TargetProfile>,
    build_snapshot: Option<package_compilation::BuildSnapshotRequest>,
    proposal: Option<PendingPackageChange>,
    resume: bool,
}

/// One target's exact review inputs. This is work to perform, not a policy
/// decision or permission to publish a lock.
pub struct PackageCommandReviewRequest<'request> {
    identity: &'request Arc<CommandReviewIdentity>,
    source_closure: &'request ResolvedPackageSourceClosure,
    target: TargetProfile,
    accepted: Option<&'request crate::lock::PackageLockTarget>,
    build_snapshot: Option<&'request package_compilation::BuildSnapshotRequest>,
}

/// Compiler findings joined to the admitted candidate's policy baseline.
pub struct PackageCommandReview {
    identity: Arc<CommandReviewIdentity>,
    review: PackageChangeReview,
}

/// In-process identity of this exact admission, including its baseline and
/// selected inputs. Review receipts share this owner, not a reconstructed key
/// that could accidentally omit part of the command's policy context.
struct CommandReviewIdentity;

impl PackageCommandReviewRequest<'_> {
    /// Run the existing review policy passes for this admitted target. The
    /// application's work loop owns when to request each target; discovery,
    /// comparison and the lower-level occurrence loop remain package-owned.
    pub fn review(
        self,
        session: &mut PackageBuildSession,
        source_bindings: &mut PackageSourceBindings,
        execute: crate::review::PackageReviewExecutor,
    ) -> Result<PackageCommandReview, PackageCommandError> {
        let review = crate::operations::review_package_change_reusing(
            self.source_closure.clone(),
            self.target,
            self.accepted,
            session,
            self.build_snapshot,
            source_bindings,
            execute,
        )
        .map_err(failure)?;
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
        self.targets
            .iter()
            .map(|&target| PackageCommandReviewRequest {
                identity: &self.identity,
                source_closure: &self.closure,
                target,
                accepted: self.accepted.as_ref().and_then(|lock| lock.target(target)),
                build_snapshot: self.build_snapshot.as_ref(),
            })
    }

    /// Reconcile the complete requested roster before touching decision files.
    /// Missing, reordered, duplicate or another command's findings cannot publish.
    /// Pending review leaves the invocation sponsor open; acceptance closes
    /// staging before the accepted project files change.
    pub fn complete(
        self,
        reviews: Vec<PackageCommandReview>,
        session: &mut PackageBuildSession,
    ) -> Result<PackageCommandOutcome, PackageCommandError> {
        if reviews.len() != self.targets.len() {
            return Err(failure(
                "package command did not receive every requested target review",
            ));
        }
        for (result, &target) in reviews.iter().zip(&self.targets) {
            if !Arc::ptr_eq(&self.identity, &result.identity) {
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
            transaction,
            files,
            mut pending_file,
            storage,
            acquisition,
            before_lock,
            accepted,
            replacement,
            stage,
            closure,
            proposal,
            resume,
            ..
        } = self;
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
            // Publish the proposal only after every target's findings are present.
            // It is review state, never the accepted pair's commit intent.
            state::write_proposal(files, &text)?;
            pending_file = state::read(files, state::PROPOSAL)?;
        }
        if let Some(file) = &mut pending_file {
            file.verify_current(state::LIMITS)
                .map_err(state::file_failure)?;
        }
        for file in &mut choices.reads {
            file.verify_current(state::LIMITS)
                .map_err(state::file_failure)?;
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
        // No authored execution follows this point. Refuse cleanup failure before
        // accepted files change, rather than discovering it after publication.
        session.close().map_err(failure)?;
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
            && let Err(error) = file.remove(state::LIMITS)
        {
            choices.report.push_str(&format!("\nAccepted files were published, but proposal cleanup failed: {}. Use --discard-review before starting another change.", state::file_failure(error)));
        }
        Ok(PackageCommandOutcome {
            status: PackageCommandStatus::Published,
            report: choices.report,
            review_paths: choices.paths,
        })
    }
}

/// Recover publication state and discard a pending proposal without acquiring
/// resolver storage or a Build session.
pub fn discard_package_review(
    project_root: &Path,
) -> Result<PackageCommandOutcome, PackageCommandError> {
    let mut transaction =
        PackageFileTransaction::open(project_root, PackagePublicationLimits::default())
            .map_err(failure)?;
    transaction.recover().map_err(failure)?;
    state::discard(&transaction)
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    crate::digest_encoder::content_identity(&bytes).into()
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
        /// Declared Git workspace package name; absence selects the root.
        package: Option<String>,
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
    /// Explicit targets augment every previously accepted target. Empty uses
    /// existing targets, or the compiler's default target on first admission.
    pub targets: Vec<TargetProfile>,
    /// Use cached recorded Git pins only; never fetch or refresh a selector.
    pub offline: bool,
    /// Root-only immutable input selection for a new install/update. Resume
    /// uses the pending proposal's selection; it cannot override it.
    pub build_inputs: Option<package_compilation::BuildSourceCaptureRequest>,
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
pub struct PackageCommandError(String);

impl From<CompileResolvedPackageReviewsError> for PackageCommandError {
    fn from(error: CompileResolvedPackageReviewsError) -> Self {
        failure(error)
    }
}

impl From<String> for PackageCommandError {
    fn from(error: String) -> Self {
        Self(error)
    }
}

impl fmt::Display for PackageCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}
impl std::error::Error for PackageCommandError {}

fn failure(message: impl fmt::Display) -> PackageCommandError {
    PackageCommandError(message.to_string())
}
