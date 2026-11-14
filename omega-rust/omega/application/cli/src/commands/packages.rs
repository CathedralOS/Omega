/*

Console adapter for `omega packages install`, `omega packages update`, and `omega packages audit`.

Package installation and update run inside a single retained `BuildSession` at
`<project_root>/build/package-manager`. That session owns the temporary work
directory and resource grant across every requested target and across any
automatic review rounds triggered by `--dangerously-accept-all`. A subsequent
CLI invocation creates its own session and resource grant.

When `execute_package_command` returns `PackageCommandStatus::ReviewRequired`,
the normal workflow prints the review file paths and exits with code 3 so the
user can inspect the findings, edit `decision ... pending` lines to `accept`,
and run `omega packages install --resume` or `omega packages update --resume`.

When `--dangerously-accept-all` is passed, `accept_pending_decisions` performs
that exact text edit in place (`decision <subject> pending` ->
`decision <subject> accept`) and resumes the command. Before resuming, we clear
`options.targets` and `options.build_inputs` because the saved review proposal
already owns its target roster and build-input inventory; automatic acceptance
only supplies review decisions. Because accepting one round of decisions can
expose a transitive dependency closure with fresh findings, we repeat up to
`ACCEPT_ALL_ROUNDS` (4) times before giving up, preventing an unsettled review
from spinning forever.

*/

use crate::arguments::USAGE_ERROR_EXIT;
use crate::arguments::audit::SourceArguments;
use orchestration::{BuildSession, execute_package_command, inspect_packages};
use package_manager::operations::PackageInspectionOptions;
use package_manager::{
    PackageCommand, PackageCommandError, PackageCommandKind, PackageCommandOptions,
    PackageCommandOutcome, PackageCommandStatus, discard_package_review,
};
use package_source::PrimaryGitChoices;
use std::path::Path;

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
            let mut session = match BuildSession::create_retained(
                &options.project_root.join("build/package-manager"),
            ) {
                Ok(session) => session,
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(1);
                }
            };
            let result = (|| -> Result<PackageCommandOutcome, PackageCommandError> {
                let mut outcome = execute_package_command(command, options.clone(), &mut session)?;
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

pub(crate) fn run_audit(options: PackageInspectionOptions) {
    match inspect_packages(options) {
        Ok(outcome) => {
            print!("{}", outcome.report);
            if !outcome.complete {
                std::process::exit(1);
            }
            if outcome.requires_decision {
                std::process::exit(3);
            }
        }
        Err(error) => {
            eprintln!("cannot inspect packages: {error}");
            std::process::exit(1);
        }
    }
}

pub(crate) fn run_source_audit(arguments: SourceArguments) {
    let adapter = match package_manager::operations::SourceAdapter::parse(&arguments.source_kind) {
        Ok(adapter) => adapter,
        Err(error) => {
            eprintln!("invalid source adapter: {error:?}");
            std::process::exit(i32::from(USAGE_ERROR_EXIT));
        }
    };
    let storage =
        match package_source::SourceResolver::for_current_user(PrimaryGitChoices::default()) {
            Ok(storage) => storage,
            Err(error) => {
                eprintln!("cannot open private source resolver storage: {error}");
                std::process::exit(1);
            }
        };
    match package_manager::operations::inspect_package_source_locator(
        adapter,
        arguments.locator,
        arguments.rev,
        &storage,
        package_source::LocalSourceLimits::default(),
    ) {
        Ok(report) => print!("{}", report.to_text()),
        Err(error) => {
            eprintln!("cannot inspect package source: {error:?}");
            std::process::exit(1);
        }
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
