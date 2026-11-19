/*

Console adapter for `omega [path]` and `omega --check [path]`.

We translate `CompileArguments` into an `orchestration::BuildRequest`, run
`orchestration::build`, and format the resulting `BuildReport`s on stdout and
stderr, in `--timings`, and in `--report-file`.

How target selection shapes the run:
- Zero `--target` flags select `BuildTargets::Host`; one selects
  `BuildTargets::Single`; two or more select `BuildTargets::Batch`.
- A single-target invocation (`present_single`) prints the check summary or
  published artifact lines directly. `--report-file` gets the full report on
  success and the same `target:`/`failed:` block a batch writes on refusal.
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

`CompilerRequestOutcome::InternalFailure` (exit code 3) outranks both: it means
the compiler caught a defect in itself, such as its own stage output failing an
independent validator. A script reading `$?` must learn that the compiler is at
fault, even when another target in the batch was rejected, because the batch's
verdicts no longer carry the compiler's authority.

*/

use std::path::{Path, PathBuf};
use std::time::Instant;

use compiler::CompilerRequestOutcome;
use diagnostics::Diagnostic;
use orchestration::{
    BuildError, BuildOutcome, BuildReport, BuildRequest, BuildTargets, NativeDestination,
    ProjectProduct, TargetBuildOutcome, build,
};

use package_compilation::build_execution::BuildSnapshotRequest;

use crate::arguments::CompileArguments;
use crate::diagnostic_output::DiagnosticOutput;
use crate::saved_program::SavedProgramError;

pub(crate) fn run(arguments: CompileArguments, output: DiagnosticOutput) {
    if arguments
        .root_path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("psi"))
    {
        match build_saved(arguments) {
            Ok(paths) => {
                for path in paths {
                    println!("executable {}", path.display());
                }
            }
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(i32::from(error.outcome().halt_value()));
            }
        }
        return;
    }
    let started = arguments.timings.then(Instant::now);
    let report_file = arguments.report_file;
    let installation_assumptions = arguments.installation_assumptions.clone();
    for row in &installation_assumptions {
        println!("{}", assumption_text(row));
    }
    // The enumerated set depends on this compiler's registry, so a report of
    // it names the compiler and the roster it enumerated.
    let roster = arguments.stage_validators.enumerates_every().then(|| {
        format!(
            "stage validator roster (omega {} {}): {}\n",
            env!("OMEGA_VERSION"),
            env!("OMEGA_COMMIT"),
            orchestration::StageValidator::ROSTER
                .iter()
                .map(|validator| validator.name())
                .collect::<Vec<_>>()
                .join(", ")
        )
    });
    let roster = roster.as_deref().unwrap_or_default();
    let target_names = arguments.target_names;
    let targets = match resolve_targets(&target_names) {
        Ok(RequestedTargets::Host) => BuildTargets::Host,
        Ok(RequestedTargets::Single(profile)) => BuildTargets::Single(profile),
        Ok(RequestedTargets::Batch(set)) => BuildTargets::Batch(set),
        Err(diagnostics) => exit_on_refusal(output, [&BuildError::Diagnostics(diagnostics)]),
    };
    let request = BuildRequest {
        stage_validators: arguments.stage_validators,
        build_directory: arguments.build_dir,
        root_path: arguments.root_path,
        targets,
        product: if arguments.check_only {
            ProjectProduct::Check
        } else if let Some(path) = arguments.emit_psi {
            ProjectProduct::PublishedPsi(path)
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
            .map(|capture| BuildSnapshotRequest::scoped(std::iter::empty(), capture)),
    };

    if target_names.len() > 1 {
        present_batch(
            request,
            started,
            report_file.as_deref(),
            &installation_assumptions,
            roster,
            output,
        );
    } else {
        present_single(
            request,
            target_names.first().map(String::as_str),
            started,
            report_file.as_deref(),
            &installation_assumptions,
            roster,
            output,
        );
    }
}

fn build_saved(arguments: CompileArguments) -> Result<Vec<PathBuf>, SavedProgramError> {
    let authorization = crate::saved_program::input::open_trusted_program(
        &arguments.root_path,
        arguments.trusted_artifact.as_deref(),
    )?;
    let targets = resolve_targets(&arguments.target_names)?;
    let profiles = targets.profiles()?;
    crate::saved_program::publish(
        authorization,
        &arguments.root_path,
        arguments.build_dir.as_deref(),
        &profiles,
        matches!(targets, RequestedTargets::Batch(_)),
    )
}

enum RequestedTargets {
    Host,
    Single(target::TargetProfile),
    Batch(target::ExplicitTargetSet),
}

impl RequestedTargets {
    fn profiles(&self) -> Result<Vec<target::TargetProfile>, Vec<Diagnostic>> {
        match self {
            Self::Host => target::TargetProfile::host_if_supported()
                .map(|profile| vec![profile])
                .ok_or_else(|| {
                    let names = target::TargetProfile::ALL.map(target::TargetProfile::target_name);
                    vec![Diagnostic::error(format!(
                        "no target was named and this host has no catalogued Omega deployment profile; name an exact target ({})",
                        names.join(", ")
                    ))]
                }),
            Self::Single(profile) => Ok(vec![*profile]),
            Self::Batch(set) => Ok(set.profiles().to_vec()),
        }
    }
}

fn resolve_targets(target_names: &[String]) -> Result<RequestedTargets, Vec<Diagnostic>> {
    match target_names {
        [] => Ok(RequestedTargets::Host),
        [name] => target::TargetProfile::from_omega_target_name(Some(name))
            .map(RequestedTargets::Single)
            .map_err(|diagnostic| vec![diagnostic]),
        names => target::ExplicitTargetSet::from_caller_names(names).map(RequestedTargets::Batch),
    }
}

fn present_single(
    request: BuildRequest,
    target_name: Option<&str>,
    started: Option<Instant>,
    report_file: Option<&Path>,
    installation_assumptions: &[installation_conditions::InstallationEnvironmentAssumption],
    roster: &str,
    output: DiagnosticOutput,
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
        Err(error) => {
            // The report is written before the exit status is chosen, so a
            // refused compilation still leaves its rows, including failed
            // stage validator rows, in `--report-file`.
            if let Some(path) = report_file {
                write_report_or_exit(
                    path,
                    &format!(
                        "{roster}{}",
                        failed_report_text(target_name.unwrap_or("host default"), &error, output)
                    ),
                );
            }
            exit_on_refusal(output, [&error])
        }
    };
    if let Some(path) = report_file {
        write_report_or_exit(
            path,
            &format!(
                "{roster}{}",
                report_text(target_name, &outcome, installation_assumptions)
            ),
        );
    }
    print!("{roster}");
    print_outcome(&outcome);
}

fn present_batch(
    request: BuildRequest,
    started: Option<Instant>,
    report_file: Option<&Path>,
    installation_assumptions: &[installation_conditions::InstallationEnvironmentAssumption],
    roster: &str,
    output: DiagnosticOutput,
) {
    let outcomes = match build(request) {
        Ok(outcomes) => outcomes,
        Err(error) => {
            if let Some(started) = started {
                print_elapsed(started);
            }
            exit_on_refusal(output, [&error]);
        }
    };
    let mut report_contents = roster.to_owned();
    print!("{roster}");
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
                eprintln!("{}", output.build_error(&error));
                println!("target {name} failed");
                report_contents.push_str(&failed_report_text(name, &error, output));
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

fn exit_on_refusal<'a>(output: DiagnosticOutput, refusals: impl IntoIterator<Item = &'a BuildError>) -> ! {
    let list: Vec<&BuildError> = refusals.into_iter().collect();
    for error in &list {
        eprintln!("{}", output.build_error(error));
    }
    std::process::exit(refusal_exit_status(list));
}

fn refusal_exit_status<'a>(refusals: impl IntoIterator<Item = &'a BuildError>) -> i32 {
    i32::from(batch_outcome(refusals.into_iter().map(BuildError::outcome)).halt_value())
}

fn batch_outcome(
    outcomes: impl IntoIterator<Item = CompilerRequestOutcome>,
) -> CompilerRequestOutcome {
    let mut combined: Option<CompilerRequestOutcome> = None;
    for outcome in outcomes {
        combined = Some(match (combined, outcome) {
            (_, CompilerRequestOutcome::InternalFailure)
            | (Some(CompilerRequestOutcome::InternalFailure), _) => {
                CompilerRequestOutcome::InternalFailure
            }
            (
                None | Some(CompilerRequestOutcome::Incomplete(_)),
                CompilerRequestOutcome::Incomplete(_),
            ) => combined.unwrap_or(outcome),
            _ => CompilerRequestOutcome::Reject,
        });
    }
    combined.unwrap_or(CompilerRequestOutcome::Reject)
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
    if outcome.report.wrote_output()
        && let Some(artifact) = outcome.report.artifact()
    {
        println!("Psi manifest {}", artifact.manifest().identity());
    }
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
    print!("{}", format_stage_validations(outcome));
}

fn assumption_text(row: &installation_conditions::InstallationEnvironmentAssumption) -> String {
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
    installation_assumptions: &[installation_conditions::InstallationEnvironmentAssumption],
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
    contents.push_str(&format_stage_validations(outcome));
    contents
}

// A refused target's report block. A stage validator failure's text holds
// every row that target ran, passed and failed.
fn failed_report_text(target: &str, error: &BuildError, output: DiagnosticOutput) -> String {
    format!("target: {target}\nfailed: {}\n", output.build_error(error))
}

// One line per requested validator and target; empty for default compilation.
fn format_stage_validations(outcome: &BuildReport) -> String {
    outcome
        .stage_validations
        .iter()
        .map(|row| format!("{row}\n"))
        .collect()
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

#[cfg(test)]
mod tests {
    use super::{batch_outcome, refusal_exit_status};
    use compiler::CompilerRequestOutcome::{Incomplete, InternalFailure, Reject};
    use diagnostics::{Diagnostic, ExceededProvision};
    use orchestration::BuildError;

    const PROVISION: ExceededProvision = ExceededProvision {
        resource: "proof steps",
        limit: 1,
        requested: 2,
        coordinate: 0,
    };

    #[test]
    fn a_source_rejection_exits_1_and_a_compiler_defect_exits_3() {
        let rejected = BuildError::Diagnostics(vec![Diagnostic::error("bad source")]);
        assert_eq!(refusal_exit_status([&rejected]), 1);
        assert_eq!(batch_outcome([InternalFailure]).halt_value(), 3);
    }

    #[test]
    fn a_compiler_defect_outranks_every_other_refusal_in_a_batch() {
        assert_eq!(batch_outcome([Reject, InternalFailure]), InternalFailure);
        assert_eq!(
            batch_outcome([InternalFailure, Incomplete(PROVISION)]),
            InternalFailure
        );
        assert_eq!(batch_outcome([Incomplete(PROVISION), Reject]), Reject);
        assert_eq!(
            batch_outcome([Incomplete(PROVISION), Incomplete(PROVISION)]),
            Incomplete(PROVISION)
        );
    }
}
