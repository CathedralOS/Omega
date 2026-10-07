//! Install, update, and inspect package sources and accepted project policy.

pub(crate) mod audit;
pub(crate) mod source;

use package_manager::{
    PackageCommand, PackageCommandKind, PackageCommandOptions, PackageCommandOutcome,
    PackageCommandStatus, execute_package_command,
};
use std::path::Path;

/// A resume regenerates findings, so a changed closure can surface fresh
/// pending decisions; bounded so a review that never settles cannot spin.
const ACCEPT_ALL_ROUNDS: usize = 4;

pub(crate) fn run(command: PackageCommand, mut options: PackageCommandOptions, accept_all: bool) {
    let kind = match &command {
        PackageCommand::Install { .. } => PackageCommandKind::Install,
        PackageCommand::Resume { kind } => *kind,
        _ => PackageCommandKind::Update,
    };
    let mut outcome = execute(command, options.clone());
    // The saved proposal owns its target and input inventory. Automatic
    // acceptance resumes that proposal, just like an explicit --resume;
    // forwarding new-proposal overrides would reject or replace its custody.
    options.targets.clear();
    options.build_inputs = None;
    let mut round = 0;
    while accept_all
        && outcome.status == PackageCommandStatus::ReviewRequired
        && round < ACCEPT_ALL_ROUNDS
    {
        round += 1;
        report(&outcome);
        let accepted = outcome
            .review_paths
            .iter()
            .map(|path| accept_pending_decisions(path))
            .sum::<usize>();
        println!("--dangerously-accept-all: accepted {accepted} pending decisions unread");
        outcome = execute(PackageCommand::Resume { kind }, options.clone());
    }
    report(&outcome);
    let status = exit_status(outcome.status);
    if status != 0 {
        std::process::exit(status);
    }
}

fn execute(command: PackageCommand, options: PackageCommandOptions) -> PackageCommandOutcome {
    execute_package_command(command, options, None).unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(1);
    })
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
fn accept_pending_decisions(path: &Path) -> usize {
    let text = std::fs::read_to_string(path).unwrap_or_else(|error| {
        eprintln!("failed to read review {}: {error}", path.display());
        std::process::exit(1);
    });
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
    if accepted != 0
        && let Err(error) = std::fs::write(path, rewritten)
    {
        eprintln!("failed to write review {}: {error}", path.display());
        std::process::exit(1);
    }
    accepted
}

fn exit_status(status: PackageCommandStatus) -> i32 {
    match status {
        PackageCommandStatus::Published | PackageCommandStatus::ReviewDiscarded => 0,
        PackageCommandStatus::ReviewRequired => 3,
    }
}
