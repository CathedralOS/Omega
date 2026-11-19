// V1 request conversion and report presentation during the project driver migration.
fn run_project(arguments: CompileArguments, output: DiagnosticOutput) {
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
            compiler::stage_validators::StageValidator::ROSTER
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

