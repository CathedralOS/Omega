//! Package command lifecycle: recover, select an edit, resolve, review, publish.
//!
//! Start at [`execute_package_command`]. Edit planning, review documents and
//! publication custody stay with their subordinate owners; CLI parsing and
//! printing stay in the binary. Compiler preparation is separate in [`crate::operations`].

mod model;
mod planning;
mod proposal;
mod review;
mod source_review;
mod state;

use crate::declarations::BuildFileReplacement;
use crate::lock::{PackageLock, PackageLockRecoveryLimits};
use crate::operations::{
    PackageFileTransaction, PackagePublicationLimits, publish_reviewed_package_change,
    review_package_change_reusing, stage_build_dependency_edit,
};
use crate::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits, GitDependencyPins,
    GitResolutionOptions, PackageSourceClosureLimits,
    resolve_staged_external_local_project_closure,
};
use crate::review::CandidateSourcePreparation;
use model::failure;
pub use model::{
    PackageCommand, PackageCommandError, PackageCommandKind, PackageCommandOptions,
    PackageCommandOutcome, PackageCommandStatus,
};
use package_source::PrimaryGitChoices;
use package_source::git::resolution::GitExactRevisionAcquisition;
use package_source::{ExternalSourceContext, LocalSourceLimits, SourceResolverStorage};
use proposal::PendingPackageChange;
use sha2::{Digest, Sha256};
use target::TargetProfile;

/// Execute one package command under the project's publication authority.
/// An absent storage input selects the operator's ordinary resolver storage;
/// embedded callers may instead borrow their own. Discard requires neither.
pub fn execute_package_command(
    command: PackageCommand,
    options: PackageCommandOptions,
    storage: Option<&SourceResolverStorage>,
) -> Result<PackageCommandOutcome, PackageCommandError> {
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
    let mut transaction =
        PackageFileTransaction::open(&options.project_root, PackagePublicationLimits::default())
            .map_err(failure)?;
    transaction.recover().map_err(failure)?;
    if matches!(command, PackageCommand::DiscardReview) {
        return state::discard(&transaction);
    }
    let ordinary_storage;
    let storage = match storage {
        Some(storage) => storage,
        None => {
            ordinary_storage = SourceResolverStorage::for_current_user(PrimaryGitChoices {
                excluded_controlled_roots: &[transaction.project_root().to_path_buf()],
                ..PrimaryGitChoices::default()
            })
            .map_err(failure)?;
            &ordinary_storage
        }
    };
    let PackageCommandOptions {
        targets: requested_targets,
        offline,
        build_inputs,
        ..
    } = options;
    let files = transaction.command_state_files().map_err(failure)?;
    let mut pending_file = state::read(&files, state::PROPOSAL)?;
    let (before_build, before_lock) = transaction.read_pair().map_err(failure)?;
    let before = std::str::from_utf8(&before_build)
        .map_err(|_| failure("build.omg is not UTF-8 Omega source"))?;
    let accepted_text = before_lock.as_deref().map(std::str::from_utf8).transpose()
        .map_err(|_| failure("omega.lock is not UTF-8; restore a supported lock or explicitly remove it for fresh graph review"))?;
    let accepted = accepted_text.map(|text| PackageLock::recover_text(text, PackageLockRecoveryLimits::default())).transpose()
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
            let proposal = PendingPackageChange::recover(state::text(file)?).map_err(failure)?;
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
        // Discard normally returns before source/storage acquisition above.
        PackageCommand::DiscardReview => return state::discard(&transaction),
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
    let stage = stage_build_dependency_edit(&replacement, storage, LocalSourceLimits::default())
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
        storage,
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
    // One closure serves every requested target's review; binding-independent
    // source preparation happens once and each target still checks fresh.
    let mut preparation = CandidateSourcePreparation::for_closure(&closure);
    let mut reviews = Vec::new();
    for target in &targets {
        let build_root = transaction
            .project_root()
            .join("build/package-manager")
            .join(format!("check-{}", target.target_name()));
        reviews.push(
            review_package_change_reusing(
                closure.clone(),
                *target,
                accepted.as_ref().and_then(|lock| lock.target(*target)),
                &build_root,
                build_snapshot.as_ref(),
                &mut preparation,
            )
            .map_err(failure)?,
        );
    }
    stage.verify_live_source_unchanged().map_err(failure)?;
    let proposal_text = if !resume {
        Some(
            PendingPackageChange {
                kind,
                before_build: digest(&before_build),
                before_lock: before_lock.as_deref().map(digest),
                original_content: digest(stage.original().content_identity.as_bytes()),
                proposed_build: replacement.replacement_source().to_owned(),
                source,
                targets,
                build_inputs,
            }
            .encode()
            .map_err(failure)?,
        )
    } else {
        None
    };
    let mut choices = review::prepare(&files, &transaction, &reviews, resume, accepted.is_none())?;
    choices.report.push_str(&source_review::prepare(
        &files,
        &transaction,
        accepted.as_ref(),
        &closure,
        storage,
        acquisition,
    )?);
    if let Some(text) = proposal_text {
        // Publish the proposal only after every target's findings are present.
        // It is review state, never the accepted pair's commit intent.
        state::write_proposal(&files, &text)?;
        pending_file = state::read(&files, state::PROPOSAL)?;
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
    let published = publish_reviewed_package_change(
        &mut transaction,
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

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
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
