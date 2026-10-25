//! Install, update, and inspect package sources and accepted project policy.

pub(crate) mod audit;
pub(crate) mod source;

use omega_build::execute_package_command;
use package_manager::{
    PackageBuildSession, PackageCommand, PackageCommandError, PackageCommandKind,
    PackageCommandOptions, PackageCommandOutcome, PackageCommandStatus, discard_package_review,
};
use std::path::Path;

/// A resume regenerates findings, so a changed closure can surface fresh
/// pending decisions; bounded so a review that never settles cannot spin.
const ACCEPT_ALL_ROUNDS: usize = 4;

pub(crate) fn run(command: PackageCommand, mut options: PackageCommandOptions, accept_all: bool) {
    let result = match command {
        PackageCommand::DiscardReview => discard_package_review(&options.project_root),
        command => {
            let kind = match &command {
                PackageCommand::Install { .. } => PackageCommandKind::Install,
                PackageCommand::Resume { kind } => *kind,
                _ => PackageCommandKind::Update,
            };
            let mut session = match PackageBuildSession::create(
                &options.project_root.join("build/package-manager"),
            ) {
                Ok(session) => session,
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(1);
                }
            };
            // This owner spans all targets and automatic review rounds. A
            // later user-issued command creates its own resource grant.
            let result = (|| -> Result<PackageCommandOutcome, PackageCommandError> {
                let mut outcome = execute_package_command(command, options.clone(), &mut session)?;
                // The saved proposal owns its target and input inventory.
                // Automatic acceptance changes decisions, not resource grants.
                options.targets.clear();
                options.build_inputs = None;
                let mut round = 0;
                while accept_all
                    && outcome.status == PackageCommandStatus::ReviewRequired
                    && round < ACCEPT_ALL_ROUNDS
                {
                    round += 1;
                    report(&outcome);
                    let mut accepted = 0;
                    for path in &outcome.review_paths {
                        accepted += accept_pending_decisions(path)?;
                    }
                    println!(
                        "--dangerously-accept-all: accepted {accepted} pending decisions unread"
                    );
                    outcome = execute_package_command(
                        PackageCommand::Resume { kind },
                        options.clone(),
                        &mut session,
                    )?;
                }
                Ok(outcome)
            })();
            session.dispose(result)
        }
    };
    let outcome = match result {
        Ok(outcome) => outcome,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    report(&outcome);
    let status = exit_status(outcome.status);
    if status != 0 {
        std::process::exit(status);
    }
}

fn report(outcome: &PackageCommandOutcome) {
    if !outcome.report.is_empty() {
        print!("{}", outcome.report);
        if !outcome.report.ends_with('\n') {
            println!();
        }
    }
    for path in &outcome.review_paths {
        println!("review: {}", path.display());
    }
}

/// Rewrite every `decision ... pending` line of one review document to
/// `accept`, the same edit the document asks a reviewer to make by hand.
fn accept_pending_decisions(path: &Path) -> Result<usize, PackageCommandError> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read review {}: {error}", path.display()))?;
    let mut accepted = 0;
    let mut rewritten = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches('\n');
        match body.strip_suffix(" pending") {
            Some(subject) if subject.starts_with("decision ") => {
                accepted += 1;
                rewritten.push_str(subject);
                rewritten.push_str(" accept");
                rewritten.push_str(&line[body.len()..]);
            }
            _ => rewritten.push_str(line),
        }
    }
    if accepted != 0 {
        std::fs::write(path, rewritten)
            .map_err(|error| format!("failed to write review {}: {error}", path.display()))?;
    }
    Ok(accepted)
}

fn exit_status(status: PackageCommandStatus) -> i32 {
    match status {
        PackageCommandStatus::Published | PackageCommandStatus::ReviewDiscarded => 0,
        PackageCommandStatus::ReviewRequired => 3,
    }
}
