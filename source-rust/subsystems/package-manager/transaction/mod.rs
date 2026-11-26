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
pub(crate) mod restricted_build;
mod source_review;
mod state;

use crate::build_session::{PackageBuildSession, PackageSourceBindings};
use crate::declarations::{BuildFileReplacement, PackageKey};
use crate::dependencies::{LOCAL_PROJECT_CONTEXT, stage_build_dependency_edit};
use crate::digest_encoder::content_identity;
use crate::lock::{PackageLock, PackageLockRecoveryLimits, PackageLockTarget};
use crate::lockfile::publication::{
    PackageFileTransaction, PackagePublicationLimits, ReviewedProposal,
    publish_reviewed_package_change,
};
use crate::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits, GitDependencyPins,
    GitResolutionOptions, PackageRootSourceRequest, ResolvedPackageSourceClosure,
};
use crate::resolution::source::{AcquiredBuildSource, BuildSourceAcquisitions};
use crate::review::CompileResolvedPackageReviewsError;
use crate::review::{PackageChangeReview, review_package_change_reusing};
use build_evaluation_result::CompileFailure;
use package_compilation::build_execution::BuildSnapshotRequest;
use platform_custody::record_file::{RecordFileRoot, RootRecordRead};
use proposal::PendingPackageChange;
use source_acquisition::PrimaryGitChoices;
use source_acquisition::git::resolution::GitExactRevisionAcquisition;
use source_acquisition::local::staging::StagedLocalSnapshot;
use source_acquisition::{ExternalSourceContext, LocalSourceLimits, SourceResolver};
use source_completion_accounting::SourceCompletionAccounting;
use state::{LIMITS, PROPOSAL, discard, file_failure, read, text, write_proposal};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use target::TargetProfile;

pub struct PackageCommandTransaction {
    recovered: Option<PackageCommandKind>,
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
        let recovered = transaction.recover().map_err(failure)?;
        let files = transaction.command_state_files().map_err(failure)?;
        Ok(Self {
            recovered,
            transaction,
            files,
            options,
        })
    }

    pub fn prepare<'workspace, 'storage>(
        &'workspace mut self,
        command: PackageCommand,
        storage: Option<&'storage SourceResolver>,
    ) -> Result<PackageCommandAdmission<'workspace, 'storage>, PackageCommandError> {
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
        let recovered = self.recovered.take();
        if let PackageCommand::Resume { kind } = command {
            if !options.targets.is_empty() {
                return Err(failure("--resume uses the pending proposal's targets"));
            }
            if let Some(recovered) = recovered {
                if kind != recovered {
                    return Err(failure(
                        "the recovered publication belongs to the other package command",
                    ));
                }
                return Ok(PackageCommandAdmission::Published(PackageCommandOutcome {
                    status: PackageCommandStatus::Published,
                    report: "Recovered the reviewed build.omg and omega.lock.".into(),
                    review_paths: Vec::new(),
                }));
            }
        }
        let transaction = &mut self.transaction;
        if matches!(command, PackageCommand::DiscardReview) {
            return Err(failure("discard does not require a review command"));
        }
        let storage = CommandStorage::open(storage, transaction.project_root()).map_err(failure)?;
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
        let accepted = accepted_text.map(|text| PackageLock::recover_text(text, PackageLockRecoveryLimits::default())).transpose()
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
                        updates: proposal
                            .resume_updates(accepted.as_ref())
                            .map_err(failure)?,
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
            .map(|inputs| BuildSnapshotRequest::scoped(Vec::new(), inputs));
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
        Ok(PackageCommandAdmission::Review(Box::new(
            PackageCommandPreparation {
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
            },
        )))
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

    pub fn build_snapshot(&self) -> Option<&BuildSnapshotRequest> {
        self.build_snapshot.as_ref()
    }

    pub fn source_acquisitions(&self) -> Result<BuildSourceAcquisitions<'_>, PackageCommandError> {
        let preserved = self
            .recovered
            .as_ref()
            .and_then(|proposal| proposal.source.as_ref())
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
        let mut acquisitions = BuildSourceAcquisitions::new(
            self.storage.resolver(),
            GitResolutionOptions {
                pins,
                offline: self.offline,
            },
            self.root_request(),
        );
        acquisitions.select_reviewed_root(self.accepted.as_ref(), &self.targets)?;
        Ok(acquisitions)
    }

    fn root_request(&self) -> PackageRootSourceRequest {
        PackageRootSourceRequest::ExternalLocal {
            requested_root: self.stage.requested_root().to_path_buf(),
            source_context: ExternalSourceContext::derive(LOCAL_PROJECT_CONTEXT),
        }
    }

    fn previous_build_requests(&self) -> restricted_build::Requests {
        self.recovered
            .as_ref()
            .map(|p| p.build_requests.clone())
            .unwrap_or_default()
    }

    fn proposal(
        &self,
        source: Option<CanonicalSourceClosureSubject>,
        build_requests: restricted_build::Requests,
    ) -> PendingPackageChange {
        PendingPackageChange {
            kind: self.kind,
            before_build: digest(&self.before_build),
            before_lock: self.before_lock.as_deref().map(digest),
            original_content: digest(self.stage.original().content_identity.as_bytes()),
            proposed_build: self.replacement.replacement_source().to_owned(),
            // A resolved graph has pinned its revisions. A pre-action pause
            // still needs the update selection the user originally requested.
            refresh: if source.is_some() {
                Some(Default::default())
            } else {
                self.updates
                    .as_ref()
                    .map(|keys| keys.iter().map(|key| key.identity().digest()).collect())
            },
            source,
            build_requests,
            targets: self.targets.clone(),
            build_inputs: self.build_inputs.clone(),
        }
    }

    /// Resolve the candidate under the transaction. Restricted Build actions
    /// may need a decision before the candidate can return its package graph.
    pub fn resolve_sources(
        self,
        session: &mut crate::PackageBuildSession,
        programs: package_compilation::build_execution::BuildProgramInvocation,
    ) -> Result<CommandSourceReview<'workspace, 'storage>, PackageCommandError> {
        let mut rows = self.previous_build_requests();
        let reviewed = if rows.is_empty() {
            Default::default()
        } else if let Some(mut document) = read(self.files, restricted_build::DOCUMENT)? {
            let decisions = restricted_build::decisions(&rows, text(&document)?)?;
            document.verify_current(LIMITS).map_err(file_failure)?;
            decisions
        } else {
            // The process can stop after writing the proposal and before
            // writing its review. Recover every finding with a pending vote.
            let decisions = Default::default();
            let document = restricted_build::render(&rows, &decisions)?;
            state::write(self.files, restricted_build::DOCUMENT, &document)?;
            decisions
        };
        let approved = rows
            .iter()
            .filter(|(key, _)| reviewed.contains(*key))
            .map(|(key, text)| (*key, text.clone()))
            .collect();
        let phase = restricted_build::BuildPhaseGrants::new(
            self.stage.requested_root(),
            self.stage.normalized().content_identity.as_bytes(),
            &self.targets,
            approved,
        )?;
        let acquired = self.acquire_root()?;
        let inputs = crate::project::RootBuildInputs {
            snapshot: self.build_snapshot().cloned(),
            programs,
        };
        let result = {
            let mut acquisitions = self.source_acquisitions()?;
            acquisitions.phase_grants = Some(phase.clone());
            crate::project::resolve_build_sources(session, acquired, &mut acquisitions, inputs)
        };
        let error = match result {
            Ok(sources) => {
                return self
                    .complete_source_resolution(sources)
                    .map(|prepared| CommandSourceReview::Resolved(Box::new(prepared)));
            }
            Err(error) => error,
        };
        let Some(CompileFailure::BuildConsentRequired { site, requests, .. }) =
            error.compilation_failure()
        else {
            return Err(error);
        };
        if site.subject.is_none() {
            return Err(error);
        }
        self.stage.verify_live_source_unchanged().map_err(failure)?;
        let original = rows.len();
        rows.extend(phase.findings(site, requests)?);
        if rows.len() > restricted_build::MAX_REQUESTS {
            return Err(failure("too many restricted Build requests"));
        }
        if rows.len() != original {
            let document = restricted_build::render(&rows, &reviewed)?;
            let proposal = self.proposal(None, rows).encode().map_err(failure)?;
            // Commit the findings before exposing a document that can vote on them.
            replace_proposal(self.files, self.pending_file, &proposal)?;
            state::write(self.files, restricted_build::DOCUMENT, &document)?;
        }
        Ok(CommandSourceReview::Pending(PackageCommandOutcome {
            status: PackageCommandStatus::ReviewRequired,
            report: "Restricted Build actions await review. Generated-source and Product review are incomplete. Edit the Build decisions and resume the same package command.".into(),
            review_paths: vec![self.transaction.project_root().join("build/package-manager").join(restricted_build::DOCUMENT)],
        }))
    }

    pub fn complete_source_resolution(
        self,
        resolved: crate::project::ResolvedPackageProject,
    ) -> Result<PreparedPackageCommand<'workspace, 'storage>, PackageCommandError> {
        let closure = resolved.sources.into_source_closure();
        let root = closure
            .custody(closure.graph().root())
            .ok_or_else(|| failure("resolved command has no root source custody"))?;
        if closure.source_requests().root().request() != &self.root_request()
            || root.snapshot_root() != self.stage.snapshot_root()
            || root.resolution()
                != &source_acquisition::ImmutableSourceResolution::external_local(
                    source_acquisition::SourceContentDigest::derive(
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
            .and_then(|proposal| proposal.source.as_ref())
            .is_some_and(|previous| previous != &source)
        {
            return Err(failure(
                "candidate sources or dependency graph changed since the proposal; discard it and review a fresh candidate",
            ));
        }
        let proposal = if self
            .recovered
            .as_ref()
            .is_none_or(|pending| pending.source.is_none())
        {
            Some(self.proposal(Some(source), self.previous_build_requests()))
        } else {
            None
        };
        Ok(PreparedPackageCommand {
            builds: resolved.builds,
            preparation: self,
            closure,
            proposal,
        })
    }
}

/// The caller's source storage, or the current user's own storage opened for
/// one package command or inspection with the project kept out of Git control.
pub(crate) enum CommandStorage<'storage> {
    Supplied(&'storage SourceResolver),
    Opened(SourceResolver),
}

impl<'storage> CommandStorage<'storage> {
    pub(crate) fn open(
        supplied: Option<&'storage SourceResolver>,
        project_root: &Path,
    ) -> Result<Self, source_acquisition::SourceResolveError> {
        Ok(match supplied {
            Some(storage) => Self::Supplied(storage),
            None => Self::Opened(SourceResolver::for_current_user(PrimaryGitChoices {
                excluded_controlled_roots: &[project_root.to_path_buf()],
                ..PrimaryGitChoices::default()
            })?),
        })
    }

    pub(crate) fn resolver(&self) -> &SourceResolver {
        match self {
            Self::Supplied(storage) => storage,
            Self::Opened(storage) => storage,
        }
    }
}

pub enum PackageCommandAdmission<'workspace, 'storage> {
    Review(Box<PackageCommandPreparation<'workspace, 'storage>>),
    Published(PackageCommandOutcome),
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

pub enum CommandSourceReview<'workspace, 'storage> {
    Resolved(Box<PreparedPackageCommand<'workspace, 'storage>>),
    Pending(PackageCommandOutcome),
}

pub struct PreparedPackageCommand<'workspace, 'storage> {
    builds: crate::project::PackageReviewBuilds,
    preparation: PackageCommandPreparation<'workspace, 'storage>,
    closure: ResolvedPackageSourceClosure,
    proposal: Option<PendingPackageChange>,
}

pub struct PackageCommandReviewRequest<'request> {
    builds: &'request crate::project::PackageReviewBuilds,
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
    ) -> Result<PackageCommandReview, PackageCommandError> {
        let review = review_package_change_reusing(
            self.source_closure.clone(),
            self.target,
            self.accepted,
            session,
            self.build_snapshot,
            source_bindings,
            self.builds,
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
                builds: &self.builds,
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
            builds: _,
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
            kind,
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
        let graph_resume = resume && proposal_text.is_none();
        let mut choices = change_decisions::prepare(
            files,
            transaction,
            &reviews,
            graph_resume,
            accepted.is_none(),
        )?;
        choices.report.push_str(&source_review::prepare(
            files,
            transaction,
            accepted.as_ref(),
            &closure,
            storage.resolver(),
            acquisition,
        )?);
        if let Some(text) = proposal_text {
            replace_proposal(files, pending_file.take(), &text)?;
            pending_file = read(files, PROPOSAL)?;
            if pending_file.as_ref().map(|file| file.bytes()) != Some(text.as_bytes()) {
                return Err(failure("the package proposal changed before publication"));
            }
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
        let proposal = pending_file
            .ok_or_else(|| failure("reviewed package proposal disappeared before publication"))?;
        let published = publish_reviewed_package_change(
            transaction,
            ReviewedProposal {
                kind,
                digest: digest(proposal.bytes()),
            },
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

fn replace_proposal(
    files: &RecordFileRoot,
    existing: Option<RootRecordRead<'_>>,
    content: &str,
) -> Result<(), PackageCommandError> {
    match existing {
        Some(file) => file
            .replace(content.as_bytes(), LIMITS)
            .map_err(file_failure),
        None => write_proposal(files, content),
    }
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
    pub programs: package_compilation::build_execution::BuildProgramInvocation,
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
    accounting: SourceCompletionAccounting,
}

#[derive(Debug)]
enum PackageCommandCause {
    Message(String),
    Review(Box<CompileResolvedPackageReviewsError>),
    Change(Box<crate::review::PackageChangeError>),
    Compilation(CompileFailure),
    Preparation(Box<crate::dependencies::PrepareLocalProjectError>),
}

impl PackageCommandError {
    pub fn source_completion_accounting(&self) -> &SourceCompletionAccounting {
        &self.accounting
    }

    pub fn with_source_completion_accounting(
        mut self,
        accounting: SourceCompletionAccounting,
    ) -> Self {
        self.accounting.append(accounting);
        self
    }

    pub fn compilation_failure(&self) -> Option<&CompileFailure> {
        match &self.cause {
            PackageCommandCause::Compilation(failure) => Some(failure),
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
    fn from_package_change(error: crate::review::PackageChangeError) -> Self {
        let accounting = match &error {
            crate::review::PackageChangeError::Compilation(error) => error
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

impl From<CompileFailure> for PackageCommandError {
    fn from(error: CompileFailure) -> Self {
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

impl From<crate::dependencies::PrepareLocalProjectError> for PackageCommandError {
    fn from(error: crate::dependencies::PrepareLocalProjectError) -> Self {
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
        self.fmt_with_diagnostics(formatter, &diagnostics::write_bounded_diagnostics)
    }
}

impl PackageCommandError {
    pub fn fmt_with_diagnostics(
        &self,
        formatter: &mut fmt::Formatter<'_>,
        render: &diagnostics::WriteDiagnosticList<'_>,
    ) -> fmt::Result {
        match &self.cause {
            PackageCommandCause::Message(message) => fmt::Display::fmt(message, formatter),
            PackageCommandCause::Review(error) => error.fmt_with_diagnostics(formatter, render),
            PackageCommandCause::Change(error) => error.fmt_with_diagnostics(formatter, render),
            PackageCommandCause::Preparation(error) => fmt::Display::fmt(error, formatter),
            PackageCommandCause::Compilation(failure) => {
                render(formatter, &failure.clone().into_diagnostics())
            }
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
