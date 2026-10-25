//! Console adapter for the project compilation operation.

use super::arguments::CompileArguments;
use compiler::CompilerRequestOutcome;
use omega_build::{
    BuildError, BuildRequest, BuildTargets, NativeDestination, ProjectProduct, TargetBuildOutcome,
    build,
};

pub(crate) fn compile_project_command(arguments: CompileArguments) {
    let started = arguments.timings.then(std::time::Instant::now);
    let report_file = arguments.report_file.clone();
    let target_names = arguments.target_names;
    let targets = match target_names.as_slice() {
        [] => Ok(BuildTargets::Host),
        [name] => target::TargetProfile::from_omega_target_name(Some(name))
            .map(BuildTargets::Single)
            .map_err(|error| vec![error]),
        names => target::ExplicitTargetSet::from_caller_names(names).map(BuildTargets::Batch),
    };
    let targets = match targets {
        Ok(targets) => targets,
        Err(diagnostics) => {
            let error = BuildError::Diagnostics(diagnostics);
            eprintln!("{error}");
            std::process::exit(refusal_exit_status([&error]));
        }
    };
    let request = BuildRequest {
        build_directory: arguments.build_dir,
        root_path: arguments.root_path,
        targets,
        product: if arguments.check_only {
            ProjectProduct::Check
        } else {
            ProjectProduct::Native {
                destination: NativeDestination::Published,
                private_entry_ram: Vec::new(),
            }
        },
        timings: arguments.timings,
        offline: arguments.offline,
        verify: arguments.verify,
        require_package_project: false,
        optimization_rollback: arguments.optimization_rollback,
        build_snapshot: arguments
            .build_inputs
            .map(|capture| compiler::BuildSnapshotRequest::scoped(std::iter::empty(), capture)),
    };
    if target_names.len() > 1 {
        compile_several_targets(request, started, report_file.as_deref());
        return;
    }
    let report_target_name = target_names.first();
    let result = build(request).and_then(|outcomes| match outcomes.into_iter().next() {
        Some(outcome) => outcome
            .result
            .and_then(omega_build::BuildOutcome::into_report),
        None => Err(BuildError::Diagnostics(vec![
            diagnostics::Diagnostic::error("build lost the requested target outcome"),
        ])),
    });
    if let Some(started) = started {
        if let Ok(outcome) = &result {
            print_timings(outcome);
        }
        print_elapsed(started);
    }
    let outcome = match result {
        Ok(outcome) => outcome,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(refusal_exit_status([&error]));
        }
    };
    if let Some(path) = &report_file {
        write_report_or_exit(
            path,
            &report_text(report_target_name.map(String::as_str), &outcome),
        );
    }
    print_outcome(&outcome);
}

/// Several targets: one block per target in canonical order. A failed target
/// is reported and the rest still run; the command fails if any target did.
fn compile_several_targets(
    request: BuildRequest,
    started: Option<std::time::Instant>,
    report_file: Option<&std::path::Path>,
) {
    let outcomes = match build(request) {
        Ok(outcomes) => outcomes,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let mut report_contents = String::new();
    let mut failed = Vec::new();
    let mut refusals = Vec::new();
    let target_count = outcomes.len();
    for TargetBuildOutcome { target, result } in outcomes {
        let name = target.target_name();
        println!("target {name}:");
        match result.and_then(omega_build::BuildOutcome::into_report) {
            Ok(outcome) => {
                if started.is_some() {
                    print_timings(&outcome);
                }
                print_outcome(&outcome);
                report_contents.push_str(&report_text(Some(name), &outcome));
            }
            Err(error) => {
                eprintln!("{error}");
                println!("target {name} failed");
                report_contents.push_str(&format!("target: {name}\nfailed: {error}\n"));
                failed.push(name);
                refusals.push(error);
            }
        }
    }
    if let Some(started) = started {
        print_elapsed(started);
    }
    if let Some(path) = report_file {
        write_report_or_exit(path, &report_contents);
    }
    if !failed.is_empty() {
        eprintln!(
            "{} of {} targets failed: {}",
            failed.len(),
            target_count,
            failed.join(", ")
        );
        std::process::exit(refusal_exit_status(refusals.iter()));
    }
}

/// The exit status of a compile command that refused: the compiler request's
/// halt value. Several refused targets aggregate by the same rule one
/// refusal's diagnostics follow — `Incomplete` (2) only when every refusal is
/// incomplete, else `Reject` (1) — so an established violation in any target
/// is never reported as a search that merely stopped early.
fn refusal_exit_status<'a>(refusals: impl IntoIterator<Item = &'a BuildError>) -> i32 {
    let mut outcomes = refusals.into_iter().map(BuildError::outcome);
    match outcomes.next() {
        Some(first @ CompilerRequestOutcome::Incomplete(_))
            if outcomes.all(|outcome| matches!(outcome, CompilerRequestOutcome::Incomplete(_))) =>
        {
            i32::from(first.halt_value())
        }
        _ => i32::from(CompilerRequestOutcome::Reject.halt_value()),
    }
}

fn print_timings(outcome: &omega_build::BuildReport) {
    for timing in outcome.timings.phases() {
        eprintln!(
            "{:>10.3} ms  {}",
            timing.microseconds as f64 / 1_000.0,
            timing.phase
        );
    }
    for timing in outcome.report.timings() {
        eprintln!(
            "{:>10.3} ms  {}",
            timing.microseconds as f64 / 1_000.0,
            timing.phase
        );
    }
}

fn print_elapsed(started: std::time::Instant) {
    eprintln!(
        "{:>10.3} ms  total elapsed",
        started.elapsed().as_secs_f64() * 1_000.0
    );
    if phase_profile::enabled() {
        eprint!("{}", phase_profile::render());
    }
}

fn print_outcome(outcome: &omega_build::BuildReport) {
    if let Some(path) = &outcome.executable_path {
        if let Some(receipt) = outcome.report.optimization_rollback_receipt() {
            println!("optimizer rollback: {receipt}");
        }
        // A published macOS package reports its `.app` root; the inner
        // executable stays reachable through `checked_native_executable_path`.
        if let Some(package_root) = outcome.report.checked_native_package_path() {
            println!("published macOS package to {}", package_root.display());
        } else {
            println!("published native output to {}", path.display());
        }
    } else {
        println!("{}", outcome.report.summary());
    }
    if let Some(directory) = outcome
        .report
        .build_outputs()
        .and_then(|outputs| outputs.published_directory())
    {
        println!(
            "published completed build outputs to {}",
            directory.display()
        );
    }
}

/// Render the produced outcome as the plain-text observation `--report-file`
/// requests: the requested product, the named target, the report summary,
/// and every publication line the console reports.
fn report_text(target_name: Option<&str>, outcome: &omega_build::BuildReport) -> String {
    let mut contents = String::new();
    contents.push_str(match outcome.executable_path.is_some() {
        true => "product: native artifact\n",
        false => "product: check\n",
    });
    match target_name {
        Some(target) => contents.push_str(&format!("target: {target}\n")),
        None => contents.push_str("target: host default\n"),
    }
    contents.push_str(&outcome.report.summary());
    contents.push('\n');
    if let Some(receipt) = outcome.report.optimization_rollback_receipt() {
        contents.push_str(&format!("optimizer rollback: {receipt}\n"));
    }
    if let Some(path) = &outcome.executable_path {
        match outcome.report.checked_native_package_path() {
            Some(package_root) => {
                contents.push_str(&format!(
                    "published macOS package to {}\n",
                    package_root.display()
                ));
            }
            None => {
                contents.push_str(&format!("published native output to {}\n", path.display()));
            }
        }
    }
    if let Some(directory) = outcome
        .report
        .build_outputs()
        .and_then(|outputs| outputs.published_directory())
    {
        contents.push_str(&format!(
            "published completed build outputs to {}\n",
            directory.display()
        ));
    }
    contents
}

fn write_report_or_exit(path: &std::path::Path, contents: &str) {
    if let Err(error) = write_report_text(path, contents) {
        eprintln!(
            "cannot write compile report to `{}`: {error}",
            path.display()
        );
        std::process::exit(1);
    }
}

fn write_report_text(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)
}
