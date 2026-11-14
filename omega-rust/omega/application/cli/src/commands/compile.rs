/*

Console adapter for `omega [path]` and `omega --check [path]`.

We translate `CompileArguments` into an `orchestration::BuildRequest`, run
`orchestration::build`, and format the resulting `BuildReport`s on stdout and
stderr, in `--timings`, and in `--report-file`.

How target selection shapes the run:
- Zero `--target` flags select `BuildTargets::Host`; one selects
  `BuildTargets::Single`; two or more select `BuildTargets::Batch`.
- A single-target invocation (`present_single`) prints the check summary or
  published artifact lines directly and writes `--report-file` only when
  compilation succeeds.
- A multi-target invocation (`present_batch`) compiles every requested target
  in one prepared session and continues past individual target failures so a
  cross-target sweep reports all broken targets in one pass. Each target block
  is headed by `target <name>:`, `--report-file` records both passing and
  failing targets in canonical order, and we exit nonzero at the end if any
  target refused.
- For macOS application bundles, publication prints the `.app` root
  (`checked_native_package_path`) while the inner Mach-O executable remains
  addressable through `checked_native_executable_path`.

*/

/*

Why `refusal_exit_status` requires unanimity before returning 2 (`Incomplete`):

The compiler distinguishes a definitive refutation (`CompilerRequestOutcome::Reject`,
exit code 1) from a search that exhausted a private resource bound without a
verdict (`CompilerRequestOutcome::Incomplete`, exit code 2). In a multi-target
batch, if one target hits an `Incomplete` bound while another target is
outright `Reject`ed, returning 2 for the batch would hide a proven source or
target violation behind a resource-bound status. We therefore return 2 only
when every refusal in the batch is `Incomplete`; any `Reject` wins and produces
exit code 1.

*/

use std::path::Path;
use std::time::Instant;

use compiler::CompilerRequestOutcome;
use orchestration::{
    BuildError, BuildOutcome, BuildReport, BuildRequest, BuildTargets, NativeDestination,
    ProjectProduct, TargetBuildOutcome, build,
};

use crate::arguments::CompileArguments;

pub(crate) fn run(arguments: CompileArguments) {
    let started = arguments.timings.then(Instant::now);
    let report_file = arguments.report_file;
    let installation_assumptions = arguments.installation_assumptions.clone();
    for row in &installation_assumptions {
        println!("{}", assumption_text(row));
    }
    let target_names = arguments.target_names;
    let targets = match resolve_targets(&target_names) {
        Ok(targets) => targets,
        Err(error) => exit_on_refusal([&error]),
    };
    let request = BuildRequest {
        build_directory: arguments.build_dir,
        root_path: arguments.root_path,
        targets,
        product: if arguments.check_only {
            ProjectProduct::Check
        } else {
            ProjectProduct::Native {
                installation_assumptions: arguments.installation_assumptions,
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
        present_batch(
            request,
            started,
            report_file.as_deref(),
            &installation_assumptions,
        );
    } else {
        present_single(
            request,
            target_names.first().map(String::as_str),
            started,
            report_file.as_deref(),
            &installation_assumptions,
        );
    }
}

fn resolve_targets(target_names: &[String]) -> Result<BuildTargets, BuildError> {
    match target_names {
        [] => Ok(BuildTargets::Host),
        [name] => target::TargetProfile::from_omega_target_name(Some(name))
            .map(BuildTargets::Single)
            .map_err(|error| BuildError::Diagnostics(vec![error])),
        names => target::ExplicitTargetSet::from_caller_names(names)
            .map(BuildTargets::Batch)
            .map_err(BuildError::Diagnostics),
    }
}

fn present_single(
    request: BuildRequest,
    target_name: Option<&str>,
    started: Option<Instant>,
    report_file: Option<&Path>,
    installation_assumptions: &[compilation_products::InstallationEnvironmentAssumption],
) {
    let result = build(request).and_then(|outcomes| match outcomes.into_iter().next() {
        Some(outcome) => outcome.result.and_then(BuildOutcome::into_report),
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
        Err(error) => exit_on_refusal([&error]),
    };
    if let Some(path) = report_file {
        write_report_or_exit(
            path,
            &report_text(target_name, &outcome, installation_assumptions),
        );
    }
    print_outcome(&outcome);
}

fn present_batch(
    request: BuildRequest,
    started: Option<Instant>,
    report_file: Option<&Path>,
    installation_assumptions: &[compilation_products::InstallationEnvironmentAssumption],
) {
    let outcomes = match build(request) {
        Ok(outcomes) => outcomes,
        Err(error) => {
            if let Some(started) = started {
                print_elapsed(started);
            }
            exit_on_refusal([&error]);
        }
    };
    let mut report_contents = String::new();
    let mut failed = Vec::new();
    let mut refusals = Vec::new();
    let target_count = outcomes.len();

    for TargetBuildOutcome { target, result } in outcomes {
        let name = target.target_name();
        println!("target {name}:");
        match result.and_then(BuildOutcome::into_report) {
            Ok(outcome) => {
                if started.is_some() {
                    print_timings(&outcome);
                }
                print_outcome(&outcome);
                report_contents.push_str(&report_text(
                    Some(name),
                    &outcome,
                    installation_assumptions,
                ));
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

fn exit_on_refusal<'a>(refusals: impl IntoIterator<Item = &'a BuildError>) -> ! {
    let list: Vec<&BuildError> = refusals.into_iter().collect();
    for error in &list {
        eprintln!("{error}");
    }
    std::process::exit(refusal_exit_status(list));
}

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

fn print_timings(outcome: &BuildReport) {
    for timing in outcome
        .timings
        .phases()
        .iter()
        .chain(outcome.report.timings())
    {
        eprintln!(
            "{:>10.3} ms  {}",
            timing.microseconds as f64 / 1_000.0,
            timing.phase
        );
    }
}

fn print_elapsed(started: Instant) {
    eprintln!(
        "{:>10.3} ms  total elapsed",
        started.elapsed().as_secs_f64() * 1_000.0
    );
}

fn print_outcome(outcome: &BuildReport) {
    // The executed test roster goes to stderr so the publication lines on
    // stdout keep their format.
    for entry in outcome.report.test_roster() {
        eprintln!("{entry}");
    }
    // Warnings about an accepted program are advisory: they go to stderr
    // and change neither the summary nor the exit status.
    let advisories = outcome.report.advisories();
    if !advisories.is_empty() {
        let mut text = String::new();
        if diagnostics::write_diagnostics(&mut text, advisories).is_ok() {
            eprintln!("{text}");
        }
    }
    if outcome.executable_path.is_none() {
        println!("{}", outcome.report.summary());
    }
    print!("{}", format_publication_lines(outcome));
}

fn assumption_text(row: &compilation_products::InstallationEnvironmentAssumption) -> String {
    format!(
        "Caller assumption (not proof or observation): {}; entry {:?}; installation {:?}; target {}; Psi {}; environment SHA-256 {}; source {}; caller {}",
        row.description(),
        row.entry(),
        row.installation(),
        row.profile().target_name(),
        row.psi().program_fingerprint,
        row.environment_description_sha256()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        row.source_provenance(),
        row.caller_provenance()
    )
}

fn report_text(
    target_name: Option<&str>,
    outcome: &BuildReport,
    installation_assumptions: &[compilation_products::InstallationEnvironmentAssumption],
) -> String {
    let product = if outcome.executable_path.is_some() {
        "native artifact"
    } else {
        "check"
    };
    let target = target_name.unwrap_or("host default");
    let mut contents = format!("product: {product}\ntarget: {target}\n");
    for row in installation_assumptions
        .iter()
        .filter(|row| target_name.is_none_or(|target| row.profile().target_name() == target))
    {
        contents.push_str(&assumption_text(row));
        contents.push('\n');
    }
    contents.push_str(&outcome.report.summary());
    contents.push('\n');
    contents.push_str(&format_publication_lines(outcome));
    contents
}

fn format_publication_lines(outcome: &BuildReport) -> String {
    let mut lines = String::new();
    if let Some(path) = &outcome.executable_path {
        if let Some(receipt) = outcome.report.optimization_rollback_receipt() {
            lines.push_str(&format!("optimizer rollback: {receipt}\n"));
        }
        if let Some(package_root) = outcome.report.checked_native_package_path() {
            lines.push_str(&format!(
                "published macOS package to {}\n",
                package_root.display()
            ));
        } else {
            lines.push_str(&format!("published native output to {}\n", path.display()));
        }
    }
    if let Some(directory) = outcome
        .report
        .build_outputs()
        .and_then(|outputs| outputs.published_directory())
    {
        lines.push_str(&format!(
            "published completed build outputs to {}\n",
            directory.display()
        ));
    }
    lines
}

fn write_report_or_exit(path: &Path, contents: &str) {
    let result = (|| -> std::io::Result<()> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, contents)
    })();
    if let Err(error) = result {
        eprintln!(
            "cannot write compile report to `{}`: {error}",
            path.display()
        );
        std::process::exit(1);
    }
}
