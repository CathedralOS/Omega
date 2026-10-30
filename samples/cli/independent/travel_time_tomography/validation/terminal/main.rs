//! Validate the actual produced Terminal program, retaining admitted provisions.
//! Compilation follows the same public package Build and native operation as the
//! CLI. Each fixture gets a fresh isolated runtime and finite input; captured
//! output is checked by the separate frozen rational oracle. No source replay,
//! synthetic receiver, expected-result callback, or package acceptance occurs.
use omega_build::{BuildRequest, BuildTargets, NativeDestination, ProjectProduct};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use terminal_interpreter::{TerminalExecutionResult, TerminalExecutionStatus};
use terminal_runtime::{TerminalDomainExitDelegation, TerminalRuntimeRequest};

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.len() != 2 {
        eprintln!("usage: new087-terminal-validation SAMPLE_DIRECTORY OUTPUT_DIRECTORY");
        std::process::exit(64);
    }
    let sample = PathBuf::from(&arguments[0]);
    let output = PathBuf::from(&arguments[1]);
    // Match the shipped compiler worker stack; this is its documented current
    // embedding requirement, not an interpreter recursion or guest fuel change.
    let result = std::thread::Builder::new()
        .name("new087-validation".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(move || validate(&sample, &output))
        .expect("compiler worker creation")
        .join()
        .expect("compiler worker panic");
    if let Err(error) = result {
        eprintln!("validation unavailable: {error}");
        std::process::exit(200);
    }
}

fn validate(sample: &Path, output: &Path) -> Result<(), String> {
    std::fs::create_dir_all(output).map_err(|error| error.to_string())?;
    let profile = target::TargetProfile::from_omega_target_name(Some("linux_x86_64"))
        .map_err(|diagnostic| format!("{diagnostic:?}"))?;
    let native_directory = output.join("native");
    let outcomes = omega_build::build(BuildRequest {
        root_path: sample.join("main.omg"),
        build_directory: Some(native_directory.clone()),
        targets: BuildTargets::Single(profile),
        product: ProjectProduct::Native {
            destination: NativeDestination::Retained,
            private_entry_ram: Vec::new(),
        },
        timings: false,
        offline: true,
        verify: false,
        // The managed sample package is the only project this driver admits.
        require_package_project: true,
        optimization_rollback: Default::default(),
        build_snapshot: None,
    })
    .map_err(|error| error.to_string())?;
    let [outcome] = outcomes
        .try_into()
        .map_err(|_| "one named target produces exactly one Build outcome")?;
    let report = outcome
        .result
        .map_err(|error| error.to_string())?
        .into_report()
        .map_err(|error| error.to_string())?
        .report;
    let artifact = report
        .retained_native_program()
        .map(|program| program.psi_artifact())
        .ok_or("native product omitted its retained Terminal program")?;
    let mut fixtures: Vec<PathBuf> = std::fs::read_dir(sample.join("data"))
        .map_err(|error| error.to_string())?
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|error| error.to_string())
        })
        .collect::<Result<_, _>>()?;
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
            entry: None,
            native_provider: None,
            x86_flags: terminal_runtime::capture_host_x86_flags("linux_x86_64")
                .expect("host flags capture"),
            memory_ordering:
                terminal_runtime::TerminalMemoryOrderingEnvironment::private_sequential(
                    "linux_x86_64",
                ),
            // Match the CLI: use the native report's retained installation.
            provider_proposal: None,
            artifact,
            target: Some("linux_x86_64"),
            provisions: report.runtime_provisions(),
            fuel_allowance: 10_000_000,
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
