//! Load accepted state, stage the command, check its exact graph, and publish.

use super::model::{
    PackageCommand, PackageCommandError, PackageCommandOutcome, PackageCommandStatus, failure,
};
use super::proposal::PendingPackageChange;
use super::{planning, review, state};
use crate::declarations::BuildFileReplacement;
use crate::lock::{PackageLock, PackageLockRecoveryLimits};
use crate::operations::{
    PackageFileTransaction, publish_reviewed_package_change, review_package_change,
    stage_build_dependency_edit,
};
use crate::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits, GitDependencyPins,
    PackageSourceClosureLimits, resolve_staged_external_local_project_closure_with_git_pins,
    resolve_staged_external_local_project_closure_with_storage,
};
use package_source::git::resolution::GitExactRevisionAcquisition;
use package_source::{ExternalSourceContext, LocalSourceLimits, SourceResolverStorage};
use sha2::{Digest, Sha256};
use target::TargetProfile;

pub(super) fn execute(
    command: PackageCommand,
    requested_targets: Vec<TargetProfile>,
    transaction: &mut PackageFileTransaction,
    storage: &SourceResolverStorage,
) -> Result<PackageCommandOutcome, PackageCommandError> {
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
    let recovered = if let PackageCommand::Resume { kind } = &command {
        if !requested_targets.is_empty() {
            return Err(failure("--resume uses the pending proposal's targets"));
        }
        let file = pending_file.as_ref().ok_or_else(|| {
            failure("no pending package review; start an install or update first")
        })?;
        let proposal = PendingPackageChange::recover(state::text(file)?).map_err(failure)?;
        if proposal.kind != *kind {
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
        Some(proposal)
    } else {
        if pending_file.is_some() {
            return Err(failure(
                "a package review is pending; use --resume, or --discard-review to abandon that proposal before starting another command",
            ));
        }
        None
    };
    let (kind, replacement, updates, targets) = if let Some(proposal) = &recovered {
        (
            proposal.kind,
            BuildFileReplacement::from_sources(
                transaction.project_root().join("build.omg"),
                before,
                proposal.proposed_build.clone(),
            )
            .map_err(failure)?,
            Some(Vec::new()),
            proposal.targets.clone(),
        )
    } else {
        let plan = planning::plan(
            command,
            transaction.project_root(),
            before,
            accepted.as_ref(),
        )?;
        (
            plan.kind,
            plan.replacement,
            plan.updates,
            targets(requested_targets, accepted.as_ref())?,
        )
    };
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
    let context =
        ExternalSourceContext::derive(super::super::prepare_project::LOCAL_PROJECT_CONTEXT);
    let closure = if let (Some(subject), Some(updates)) = (preserved_subject, updates.as_deref()) {
        let pins =
            GitDependencyPins::new(subject, updates, GitExactRevisionAcquisition::AllowFetch)
                .map_err(failure)?;
        resolve_staged_external_local_project_closure_with_git_pins(
            &stage,
            context,
            storage,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
            pins,
        )
    } else {
        resolve_staged_external_local_project_closure_with_storage(
            &stage,
            context,
            storage,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
        )
    }
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
    let mut reviews = Vec::new();
    for target in &targets {
        let build_root = transaction
            .project_root()
            .join("build/package-manager")
            .join(format!("check-{}", target.target_name()));
        reviews.push(
            review_package_change(
                closure.clone(),
                *target,
                accepted.as_ref().and_then(|lock| lock.target(*target)),
                &build_root,
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
            }
            .encode()
            .map_err(failure)?,
        )
    } else {
        None
    };
    let mut choices = review::prepare(&files, transaction, &reviews, resume, accepted.is_none())?;
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
    let published =
        publish_reviewed_package_change(transaction, &replacement, &stage, &paired, accepted_text)
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
        requested.push(TargetProfile::host());
    }
    requested.sort_by_key(|target| target.target_name());
    Ok(requested)
}
