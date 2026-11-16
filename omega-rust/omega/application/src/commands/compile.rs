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

`CompilerRequestOutcome::InternalFailure` (exit code 3) outranks both: it means
the compiler caught a defect in itself, such as its own stage output failing an
independent validator. A script reading `$?` must learn that the compiler is at
fault, even when another target in the batch was rejected, because the batch's
verdicts no longer carry the compiler's authority.

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
        Ok(targets) => targets,
        Err(error) => exit_on_refusal([&error]),
    };
    let request = BuildRequest {
        stage_validators: arguments.stage_validators,
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
            roster,
        );
    } else {
        present_single(
            request,
            target_names.first().map(String::as_str),
            started,
            report_file.as_deref(),
            &installation_assumptions,
            roster,
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
    installation_assumptions: &[products::InstallationEnvironmentAssumption],
    roster: &str,
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
    installation_assumptions: &[products::InstallationEnvironmentAssumption],
    roster: &str,
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

fn assumption_text(row: &products::InstallationEnvironmentAssumption) -> String {
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
    installation_assumptions: &[products::InstallationEnvironmentAssumption],
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
