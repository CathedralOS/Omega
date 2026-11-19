//! Compile a source project or reopen a saved Psi publication.
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::arguments::CompileArguments;
use crate::diagnostic_output::DiagnosticOutput;
use crate::saved_program::SavedProgramError;
use compiler::CompilerRequestOutcome;
use compiler::stage_validators::{NativeValidationError, StageValidationRow, StageValidator};
use compiler::{Artifact, CompileRequest};
use diagnostics::Diagnostic;
use package_compilation::build_execution::{BuildSnapshotRequest, RetainedBuildOutputs};
use product_publication::CompletedProduct;
use project_build::planning::PlannedProduct;
use std::fmt::Write as _;
use target::TargetProfile;

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
    let started = Instant::now();
    let outcome = compile_project(&arguments, output);
    if arguments.timings {
        print_elapsed(started);
    }
    if outcome != CompilerRequestOutcome::Complete {
        std::process::exit(i32::from(outcome.halt_value()));
    }
}

fn compile_project(
    arguments: &CompileArguments,
    output: DiagnosticOutput,
) -> CompilerRequestOutcome {
    let mut report = String::new();
    if arguments.stage_validators.enumerates_every() {
        let names: Vec<_> = StageValidator::ROSTER
            .iter()
            .map(|row| row.name())
            .collect();
        writeln!(
            report,
            "stage validator roster (omega {} {}): {}",
            env!("OMEGA_VERSION"),
            env!("OMEGA_COMMIT"),
            names.join(", ")
        )
        .unwrap();
        print!("{report}");
    }
    for row in &arguments.installation_assumptions {
        println!("{}", assumption_text(row));
    }

    let result = compile_targets(arguments, output, &mut report);
    let verdict = match result {
        Ok(refusals) if refusals.is_empty() => CompilerRequestOutcome::Complete,
        Ok(refusals) => batch_outcome(refusals),
        Err(diagnostics) => {
            let name = arguments
                .target_names
                .first()
                .map(String::as_str)
                .unwrap_or("host default");
            let message = output.diagnostics(&diagnostics);
            eprintln!("{message}");
            writeln!(report, "target: {name}\nfailed: {message}").unwrap();
            CompilerRequestOutcome::of_refusal(&diagnostics)
        }
    };
    if let Some(path) = &arguments.report_file {
        write_report_or_exit(path, &report);
    }
    verdict
}

fn compile_targets(
    arguments: &CompileArguments,
    output: DiagnosticOutput,
    report: &mut String,
) -> Result<Vec<CompilerRequestOutcome>, Vec<Diagnostic>> {
    let targets = resolve_targets(&arguments.target_names)?;
    let profiles = targets.profiles()?;
    let batch = matches!(targets, RequestedTargets::Batch(_));
    let snapshot = arguments
        .build_inputs
        .clone()
        .map(|capture| BuildSnapshotRequest::scoped(std::iter::empty(), capture));
    let planned = crate::project_sources::open_product(
        &arguments.root_path,
        &profiles,
        arguments.offline,
        arguments.verify,
        arguments.build_dir.as_deref(),
        snapshot,
    )?;
    if arguments.timings {
        for phase in planned.timings().phases() {
            eprintln!(
                "{:>10.3} ms  {}",
                phase.microseconds as f64 / 1_000.0,
                phase.phase
            );
        }
    }
    let mut stage = planned.product_stage(&profiles)?;
    let rollback = stage.apply_rollback(&arguments.optimization_rollback);
    let base =
        project_build::output_directory::base(&arguments.root_path, arguments.build_dir.as_deref());
    let mut failures = Vec::new();
    let mut failed_targets = Vec::new();
    // Emit one portable program for the entire set of deployments.
    let count = if arguments.emit_psi.is_some() {
        1
    } else {
        profiles.len()
    };
    for &profile in &profiles[..count] {
        let name = profile.target_name();
        if batch {
            println!("target {name}:");
        }
        let directory = if batch { base.join(name) } else { base.clone() };
        let report_name = if arguments.target_names.is_empty() {
            "host default"
        } else {
            name
        };
        match compile_target(
            arguments,
            &planned,
            &stage,
            profile,
            &directory,
            rollback.as_ref(),
        ) {
            Ok(outcome) => {
                writeln!(report, "product: {}\ntarget: {report_name}", outcome.kind).unwrap();
                for row in arguments
                    .installation_assumptions
                    .iter()
                    .filter(|row| row.profile() == profile)
                {
                    writeln!(report, "{}", assumption_text(row)).unwrap();
                }
                writeln!(report, "{}", outcome.summary).unwrap();
                if outcome.show_summary {
                    println!("{}", outcome.summary);
                }
                print!("{}", outcome.publications);
                report.push_str(&outcome.publications);
                for row in outcome.rows {
                    println!("{row}");
                    writeln!(report, "{row}").unwrap();
                }
            }
            Err(error) => {
                let message = match &error {
                    NativeValidationError::Compilation(rows) => {
                        output.diagnostics(rows).to_string()
                    }
                    NativeValidationError::Unavailable(row) => {
                        output.diagnostics(std::slice::from_ref(row)).to_string()
                    }
                    NativeValidationError::Failed(rows) => rows
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("\n"),
                };
                eprintln!("{message}");
                writeln!(report, "target: {report_name}\nfailed: {message}").unwrap();
                if batch {
                    println!("target {name} failed");
                }
                failures.push(CompilerRequestOutcome::of_stage_validation(&error));
                failed_targets.push(name);
            }
        }
    }
    if batch && !failed_targets.is_empty() {
        eprintln!(
            "{} of {count} targets failed: {}",
            failed_targets.len(),
            failed_targets.join(", ")
        );
    }
    Ok(failures)
}

struct TargetReport {
    kind: &'static str,
    summary: String,
    show_summary: bool,
    publications: String,
    rows: Vec<StageValidationRow>,
}

fn output_error(message: impl ToString) -> NativeValidationError {
    NativeValidationError::Compilation(vec![Diagnostic::error(message.to_string())])
}

fn compile_target(
    arguments: &CompileArguments,
    planned: &PlannedProduct,
    stage: &project_build::planning::ProductStage<'_>,
    profile: TargetProfile,
    directory: &Path,
    rollback: Option<&optimization_requests::OptimizationRollbackReceipt>,
) -> Result<TargetReport, NativeValidationError> {
    let configuration = &planned.evaluated().configuration;
    let mut completed = configuration
        .observation_summary
        .as_ref()
        .map(RetainedBuildOutputs::from_observation)
        .transpose()
        .map_err(output_error)?
        .flatten();
    let mut publications = String::new();
    let mut advisories = Vec::new();
    let mut test_roster = Vec::new();
    let mut wrote_output = false;
    let mut show_summary = true;
    let artifact_only = stage.artifact_only();
    let kind = if artifact_only {
        "build outputs"
    } else if arguments.check_only {
        "check"
    } else if arguments.emit_psi.is_some() {
        "Psi"
    } else {
        "native artifact"
    };
    let rows;
    if artifact_only {
        if completed.is_none() || arguments.emit_psi.is_some() {
            return Err(output_error(
                "an artifact-only Build requires completed outputs and cannot publish executable Psi",
            ));
        }
        rows = arguments.stage_validators.stages_not_produced(
            &[profile.target_name()],
            |_| "the Build produces only completed files",
        )?;
    } else if arguments.check_only || arguments.emit_psi.is_some() {
        let names = if arguments.emit_psi.is_some() {
            stage.source().selections.calling_targets.to_vec()
        } else {
            vec![profile.target_name()]
        };
        rows = arguments
            .stage_validators
            .stages_not_produced(&names, |_| {
                if arguments.check_only {
                    "--check produces no native target operations"
                } else {
                    "a portable Psi publication produces no native target operations"
                }
            })?;
        let (Artifact::Psi(artifact), _) = compiler::compile_with_observations(
            CompileRequest::Psi(stage.source()),
            &Default::default(),
            Some(&mut advisories),
        )?
        else {
            return Err(output_error(
                "source compilation returned an unexpected product",
            ));
        };
        if let Some(path) = &arguments.emit_psi {
            let identity = artifact.manifest().identity();
            CompletedProduct::from_published_psi(artifact.clone(), rollback.cloned())
                .map_err(output_error)?
                .save_published_psi(path)
                .map_err(output_error)?;
            writeln!(publications, "Psi manifest {identity}").unwrap();
            wrote_output = true;
        }
    } else {
        let program = stage.program(
            profile,
            arguments
                .installation_assumptions
                .iter()
                .filter(|row| row.profile() == profile)
                .cloned()
                .collect(),
        )?;
        let (artifact, validations) = compiler::compile_with_observations(
            CompileRequest::Native {
                program,
                entry_memory: &[],
            },
            &arguments.stage_validators,
            Some(&mut advisories),
        )?;
        rows = validations;
        let Artifact::Native(product) = artifact else {
            return Err(output_error(
                "native compilation returned an unexpected product",
            ));
        };
        test_roster =
            stage.execute_registered_tests(profile, &arguments.installation_assumptions)?;
        let lease = project_build::output_directory::acquire(directory, &arguments.root_path)
            .map_err(output_error)?;
        let published = product
            .publish(&arguments.root_path, lease.directory())
            .map_err(output_error)?;
        if let Some(artifact) = published.artifact() {
            writeln!(
                publications,
                "Psi manifest {}",
                artifact.manifest().identity()
            )
            .unwrap();
        }
        if let Some(receipt) = rollback {
            writeln!(publications, "optimizer rollback: {receipt}").unwrap();
        }
        if let Some(path) = published.checked_native_package_path() {
            writeln!(
                publications,
                "published macOS package to {}",
                path.display()
            )
            .unwrap();
        } else {
            let path = published
                .checked_native_executable_path()
                .ok_or_else(|| output_error("native publication produced no executable"))?;
            writeln!(
                publications,
                "published native output to {}",
                path.display()
            )
            .unwrap();
        }
        publish_outputs(completed.take(), lease.directory(), &mut publications)?;
        wrote_output = true;
        show_summary = false;
    }
    if let Some(outputs) = completed {
        if !arguments.check_only && arguments.emit_psi.is_none() {
            let lease = project_build::output_directory::acquire(directory, &arguments.root_path)
                .map_err(output_error)?;
            publish_outputs(Some(outputs), lease.directory(), &mut publications)?;
            wrote_output = true;
        }
    }
    if arguments.timings && !rows.is_empty() {
        eprintln!("{} validator rows", rows.len());
    }
    for test in test_roster {
        eprintln!("{test}");
    }
    if !advisories.is_empty() {
        let mut text = String::new();
        diagnostics::write_diagnostics(&mut text, &advisories).unwrap();
        eprintln!("{text}");
    }
    let sources = &planned.source().sources;
    let file_count = sources
        .files()
        .filter(|file| sources.source_is_selected(file.source_id))
        .count();
    let summary = format!(
        "compiled {file_count} source file(s) from {}; wrote_output={wrote_output}",
        arguments.root_path.display()
    );
    Ok(TargetReport {
        kind,
        summary,
        show_summary,
        publications,
        rows,
    })
}

fn publish_outputs(
    outputs: Option<RetainedBuildOutputs>,
    directory: &Path,
    lines: &mut String,
) -> Result<(), NativeValidationError> {
    if let Some(mut outputs) = outputs {
        outputs.publish(directory).map_err(output_error)?;
        if let Some(path) = outputs.published_directory() {
            writeln!(
                lines,
                "published completed build outputs to {}",
                path.display()
            )
            .unwrap();
        }
    }
    Ok(())
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

fn print_elapsed(started: Instant) {
    eprintln!(
        "{:>10.3} ms  total elapsed",
        started.elapsed().as_secs_f64() * 1_000.0
    );
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
    use super::batch_outcome;
    use compiler::CompilerRequestOutcome;
    use compiler::CompilerRequestOutcome::{Incomplete, InternalFailure, Reject};
    use diagnostics::{Diagnostic, ExceededProvision};

    const PROVISION: ExceededProvision = ExceededProvision {
        resource: "proof steps",
        limit: 1,
        requested: 2,
        coordinate: 0,
    };

    #[test]
    fn a_source_rejection_exits_1_and_a_compiler_defect_exits_3() {
        let rejected = CompilerRequestOutcome::of_refusal(&[Diagnostic::error("bad source")]);
        assert_eq!(batch_outcome([rejected]).halt_value(), 1);
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
