//! Validate the actual produced Terminal program, retaining admitted provisions.
//! Compilation follows the same public package preparation/native operation as
//! the CLI. Each fixture gets a fresh isolated runtime and finite input; captured
//! output is checked by the separate frozen token-flow expectations. No source replay,
//! synthetic receiver, expected-result callback, or package acceptance occurs.
use terminal_runtime::{TerminalDomainExitDelegation, TerminalRuntimeRequest};
use package_manager::operations::{
    LocalProjectPreparationOptions, PreparedLocalProjectNativeRequest,
    compile_prepared_local_project_for_native, prepare_local_project,
};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use terminal_interpreter::{TerminalExecutionResult, TerminalExecutionStatus};

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if !(2..=3).contains(&arguments.len()) {
        eprintln!(
            "usage: linear-provider-terminal-validation SAMPLE_DIRECTORY OUTPUT_DIRECTORY [resume]"
        );
        std::process::exit(64);
    }
    let resume = arguments
        .get(2)
        .is_some_and(|argument| argument == "resume");
    let sample = PathBuf::from(&arguments[0]);
    let output = PathBuf::from(&arguments[1]);
    // Match the shipped compiler worker stack; this is its documented current
    // embedding requirement, not an interpreter recursion or guest fuel change.
    let result = std::thread::Builder::new()
        .name("linear-provider-validation".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(move || validate(&sample, &output, resume))
        .expect("compiler worker creation")
        .join()
        .expect("compiler worker panic");
    if let Err(error) = result {
        eprintln!("validation unavailable: {error}");
        std::process::exit(200);
    }
}

fn validate(sample: &Path, output: &Path, resume: bool) -> Result<(), String> {
    std::fs::create_dir_all(output).map_err(|error| error.to_string())?;
    let target = target::TargetProfile::from_omega_target_name(Some("linux_x86_64"))
        .map_err(|diagnostic| format!("{diagnostic:?}"))?;
    let prepared = prepare_local_project(
        &sample.join("main.omg"),
        LocalProjectPreparationOptions {
            target,
            offline: true,
        },
    )
    .map_err(|error| error.to_string())?
    .ok_or("sample is not a package project")?;
    let native_directory = output.join("native");
    let report = compile_prepared_local_project_for_native(PreparedLocalProjectNativeRequest::new(
        prepared,
        &native_directory,
        target,
    ))
    .map_err(|error| error.to_string())?;
    let artifact = report
        .retained_native_artifact()
        .map(|artifact| artifact.psi_artifact())
        .or_else(|| {
            report
                .retained_dynamic_elf_artifact()
                .map(|artifact| artifact.psi_artifact())
        })
        .ok_or("native product omitted its retained Terminal artifact")?;
    // Cross the actual canonical codec admission boundary with malformed
    // returned custody. These are refusal probes, never executable inputs.
    let module = artifact.semantic_module();
    let candidate = module
        .provider_candidates
        .iter()
        .find_map(|candidate| {
            module.machine(candidate.candidate).filter(|machine| {
                machine.result.structural().is_some_and(|result| {
                    result.multiplicity == terminal_psi::StructuralMultiplicity::Linear
                })
            })
        })
        .ok_or("no checked linear provider witness")?;
    for kind in ["missing", "duplicate", "wrong-path", "missing-projected"] {
        let mut changed = module.clone();
        let selected = if kind == "missing-projected" {
            module
                .provider_candidates
                .iter()
                .find_map(|row| {
                    module.machine(row.candidate).filter(|machine| {
                        machine
                            .result
                            .structural()
                            .is_some_and(|result| !result.projected_qualifications.is_empty())
                    })
                })
                .map(|machine| machine.id)
        } else {
            Some(candidate.id)
        };
        if selected.is_none() {
            continue;
        } // The pure witness carries no projected domains.
        let machine = changed
            .machines
            .iter_mut()
            .find(|machine| Some(machine.id) == selected)
            .unwrap();
        let claims = machine
            .blocks
            .iter_mut()
            .find_map(|block| match &mut block.terminator {
                terminal_psi::Terminator::ReturnStructural {
                    returned_claims, ..
                } if !returned_claims.is_empty() => Some(returned_claims),
                _ => None,
            })
            .ok_or("provider has no returned claims")?;
        match kind {
            "missing-projected" => {
                let terminal_psi::TerminalMachineResult::Structural(result) = &mut machine.result
                else {
                    unreachable!()
                };
                result.projected_qualifications.clear();
            }
            "missing" => claims.clear(),
            "duplicate" => claims.push(claims[0].clone()),
            "wrong-path" => {
                claims[0].path = vec![terminal_psi::StructuralPathSegment::FixedIndex(0)]
            }
            _ => unreachable!(),
        }
        let rejected = terminal_codec::CanonicalTerminalArtifact::from_parts(
            changed,
            artifact.optimization().clone(),
            None,
        )
        .err()
        .ok_or_else(|| format!("{kind}: malformed custody was admitted"))?;
        std::fs::write(
            output.join(format!("mutation-{kind}.txt")),
            format!("{rejected:?}\n"),
        )
        .map_err(|error| error.to_string())?;
        println!("canonical custody refusal: {kind}");
    }
    if resume {
        // This authored pure entry has no arrival parameters or host imports.
        // Use the report's exact installation and target selections; provide
        // no synthetic values, binding roster, grants or effect handler.
        use terminal_fuel::TerminalFuelMeter;
        use terminal_interpreter::{
            TerminalProgram, TerminalRuntimeSupplyRequest, TerminalStructuralInputs,
        };
        let program = TerminalProgram::borrow(artifact.view());
        let provisions = report
            .runtime_provisions()
            .ok_or("no retained provisions")?;
        let machines =
            terminal_interpreter::admit_target_machine_selections(artifact.view(), "linux_x86_64");
        let layouts = terminal_interpreter::admit_target_layouts(artifact.view(), &machines)
            .map_err(|error| format!("{error:?}"))?;
        let installation = program
            .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
                entry: Some(terminal_psi::TargetMachineCallable::Machine(
                    provisions.entry(),
                )),
                installation: provisions.installation(),
                host_imports: &[],
                target_machines: Some(&machines),
                target_layouts: Some(&layouts),
            })
            .map_err(|error| format!("{error:?}"))?;
        let entry = program
            .entry_installed(&installation)
            .map_err(|error| format!("{error:?}"))?;
        assert!(entry.scalar_parameters.is_empty() && entry.structural_parameters.is_empty());
        let requirements = program
            .execution_requirements(Some(&installation))
            .map_err(|error| format!("{error:?}"))?;
        assert!(requirements.imports.is_empty() && requirements.external_completions.is_empty());
        let mut paused = 0;
        let mut direct = 0;
        let allowances = (0..=256).chain([512, 1024, 2048, 4096, 8192, 16384, 32768, 65536]);
        for allowance in allowances {
            let mut instance = program
                .instantiate_installed(&[], TerminalStructuralInputs::default(), &installation)
                .map_err(|error| format!("{error:?}"))?;
            let first = instance
                .invoke(&mut TerminalFuelMeter::with_allowance(allowance))
                .map_err(|error| format!("{error:?}"))?;
            match first {
                TerminalExecutionStatus::SponsorExhausted(_) => paused += 1,
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => direct += 1,
                other => return Err(format!("fuel {allowance}: {other:?}")),
            }
            if !instance.is_finished() {
                let final_status = instance
                    .invoke(&mut TerminalFuelMeter::with_allowance(100_000_000))
                    .map_err(|error| format!("fuel {allowance} resume: {error:?}"))?;
                assert_eq!(
                    final_status,
                    TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
                );
            }
            instance.close().map_err(|error| format!("{error:?}"))?;
        }
        assert!(paused > 0 && direct > 0);
        std::fs::write(
            output.join("fuel-resume.txt"),
            format!(
                "paused={paused} direct={direct} total={}\n",
                paused + direct
            ),
        )
        .map_err(|error| error.to_string())?;
        println!("fuel-resume preserved actual custody: {paused} paused, {direct} direct");
    }
    let mut fixtures: Vec<PathBuf> = if resume {
        Vec::new()
    } else {
        std::fs::read_dir(sample.join("data"))
            .map_err(|error| error.to_string())?
            .map(|entry| {
                entry
                    .map(|entry| entry.path())
                    .map_err(|error| error.to_string())
            })
            .collect::<Result<_, _>>()?
    };
    fixtures.retain(|fixture| {
        fixture
            .extension()
            .is_some_and(|extension| extension == "txt")
    });
    fixtures.sort();
    for fixture in fixtures {
        let name = fixture
            .file_stem()
            .and_then(|name| name.to_str())
            .ok_or("non-UTF8 fixture name")?;
        let input = std::fs::read(&fixture).map_err(|error| error.to_string())?;
        let observation = terminal_runtime::execute(TerminalRuntimeRequest {
            // Match the CLI: use the native report's retained installation.
            provider_proposal: None,
            artifact,
            target: Some("linux_x86_64"),
            provisions: report.runtime_provisions(),
            fuel_allowance: 100_000_000,
            domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: Some(Box::new(Cursor::new(input))),
            clock: None, // This sample demands no clock import.
        })
        .map_err(|error| format!("{name}: {error}"))?;
        std::fs::write(output.join(format!("{name}.stdout")), &observation.stdout)
            .map_err(|error| error.to_string())?;
        std::fs::write(output.join(format!("{name}.stderr")), &observation.stderr)
            .map_err(|error| error.to_string())?;
        let exit = match observation.status {
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => 0,
            TerminalExecutionStatus::ExternalCompletion(completion) => completion.return_code,
            status => return Err(format!("{name}: Terminal did not complete: {status:?}")),
        };
        std::fs::write(output.join(format!("{name}.exit")), format!("{exit}\n"))
            .map_err(|error| error.to_string())?;
        println!("Terminal captured: {name}, exit {exit}");
    }
    // Publish only after interpretation, exactly as `omega run --both` does.
    // The public publisher retains executable custody; it supplies no acceptance.
    let (_, executable) =
        omega_build::publication::publish_native_artifact(report, &native_directory)?;
    std::fs::write(
        output.join("native-path.txt"),
        format!("{}\n", executable.display()),
    )
    .map_err(|error| error.to_string())?;
    println!("native executable: {}", executable.display());
    Ok(())
}
