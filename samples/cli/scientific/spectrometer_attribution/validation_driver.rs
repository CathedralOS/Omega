//! Private runtime validator, not an implementation of the spectrum algorithm.
//! Uses retained native and Psi products with explicit runtime oracles.
//! No production acceptance is persisted.
use compiler::CompileReport;
use orchestration::{BuildRequest, BuildTargets, NativeDestination, ProjectProduct};
use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use terminal_interpreter::{TerminalExecutionResult, TerminalExecutionStatus};
use terminal_runtime::{
    TerminalClockSnapshot, TerminalDomainExitDelegation, TerminalRuntimeRequest,
};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// Build one product of this sample through the public application Build.
fn build(root: &Path, build_dir: &Path, product: ProjectProduct) -> CompileReport {
    let profile = target::TargetProfile::from_omega_target_name(Some("linux_x86_64")).unwrap();
    let outcomes = orchestration::build(BuildRequest {
        root_path: root.to_path_buf(),
        build_directory: Some(build_dir.to_path_buf()),
        targets: BuildTargets::Single(profile),
        product,
        timings: false,
        offline: true,
        verify: false,
        require_package_project: true,
        optimization_rollback: Default::default(),
        build_snapshot: None,
    })
    .unwrap_or_else(|error| panic!("build: {error}"));
    let [outcome] = outcomes
        .try_into()
        .map_err(|_| "one named target produces exactly one Build outcome")
        .unwrap();
    outcome
        .result
        .unwrap_or_else(|error| panic!("build: {error}"))
        .into_report()
        .unwrap_or_else(|error| panic!("build report: {error}"))
        .report
}

fn observe(
    directory: &Path,
    name: &str,
    engine: &str,
    exit: i32,
    stdout: &[u8],
    stderr: &[u8],
    expected_exit: i32,
    expected_stdout: &[u8],
) {
    let path = directory.join(name);
    fs::create_dir_all(&path).unwrap();
    fs::write(path.join(format!("{engine}.stdout")), stdout).unwrap();
    fs::write(path.join(format!("{engine}.stderr")), stderr).unwrap();
    fs::write(path.join(format!("{engine}.exit")), format!("{exit}\n")).unwrap();
    assert_eq!(exit, expected_exit, "{name}: {engine} exit");
    assert_eq!(stdout, expected_stdout, "{name}: {engine} stdout");
    assert!(stderr.is_empty(), "{name}: {engine} stderr: {:?}", stderr);
    println!("{name}\t{engine}\texit:{exit}\tstdout:match\tstderr:match");
    std::io::stdout().flush().unwrap();
}

// Finalized selection occurrences belonged to the retired checker. Raw typed
// nominations cannot certify the inspection this mode promises.
fn verify_value_equality_selection(_root: &Path, _evidence: &Path) -> Result<(), &'static str> {
    Err("no product route yet: equality selection inspection requires route-owned finalized occurrences")
}

fn main() {
    let root = PathBuf::from(env::args_os().nth(1).expect("application directory"));
    if env::args_os().any(|argument| argument == "--selection-identity") {
        if let Err(error) =
            verify_value_equality_selection(&root, &root.join("build/selection_identity"))
        {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    let terminal_only = env::args_os().any(|argument| argument == "--terminal-only");
    let fixture = env::args_os()
        .nth(2)
        .is_some_and(|argument| argument == "--fixture");
    let evidence = root.join(if terminal_only {
        "build/terminal_matrix"
    } else {
        "build/runtime_matrix"
    });
    fs::create_dir_all(&evidence).unwrap();
    let root_source = root.join("main.omg");
    let check = build(&root_source, &evidence.join("check"), ProjectProduct::Check);
    drop(check);
    fs::write(evidence.join("check.txt"), "Check product succeeded\n").unwrap();
    println!("check\tpassed");
    let report = build(
        &root_source,
        &evidence.join("native"),
        if terminal_only {
            ProjectProduct::Terminal
        } else {
            ProjectProduct::Native {
                installation_assumptions: Vec::new(),
                destination: NativeDestination::Retained,
                private_entry_ram: Vec::new(),
            }
        },
    );
    let terminal = if terminal_only {
        report.artifact().expect("retained Terminal artifact")
    } else {
        if let Some(artifact) = report.retained_native_artifact() {
            image_emission::verify_retained_target_operations(artifact.object()).unwrap();
        }
        report
            .retained_native_program()
            .map(|program| program.psi_artifact())
            .expect("retained native program")
    };
    fs::write(evidence.join("program.terminal"), terminal.to_bytes()).unwrap();
    let manifest = if fixture {
        format!(
            "fixture\t{}",
            fs::read_to_string(root.join("expected_native_exit.txt"))
                .unwrap()
                .trim()
        )
    } else {
        fs::read_to_string(root.join("build/runtime-cases.tsv")).unwrap()
    };
    let cases = manifest
        .lines()
        .map(|line| {
            let (name, exit) = line.split_once('\t').expect("name and expected exit");
            assert!(
                name.bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            );
            (name.to_owned(), exit.parse::<i32>().unwrap())
        })
        .collect::<Vec<_>>();
    assert_eq!(
        cases.len(),
        if fixture { 1 } else { 32 },
        "complete requested runtime matrix"
    );
    let input_path = |name: &str| {
        if fixture {
            root.join("input.txt")
        } else {
            root.join(format!("data/{name}.input"))
        }
    };
    let expected_path = |name: &str| {
        if fixture {
            root.join("expected_stdout.txt")
        } else {
            root.join(format!("data/{name}.expected"))
        }
    };
    for (name, expected_exit) in &cases {
        let input = fs::read(input_path(name)).unwrap_or_else(|error| {
            if fixture && error.kind() == std::io::ErrorKind::NotFound {
                Vec::new()
            } else {
                panic!("input: {error}")
            }
        });
        let expected_stdout = fs::read(expected_path(name)).unwrap_or_else(|error| {
            if fixture && error.kind() == std::io::ErrorKind::NotFound {
                Vec::new()
            } else {
                panic!("stdout oracle: {error}")
            }
        });
        let result = terminal_runtime::execute(TerminalRuntimeRequest {
            native_image: None,
            entry: None,
            native_provider: None,
            native_services: Default::default(),
            filesystem: Default::default(),
            x86_flags: terminal_runtime::capture_host_x86_flags("linux_x86_64")
                .expect("host flags capture"),
            memory_ordering:
                terminal_runtime::TerminalMemoryOrderingEnvironment::private_sequential(
                    "linux_x86_64",
                ),
            artifact: terminal,
            provisions: report.runtime_provisions(),
            provider_proposal: None,
            target: Some("linux_x86_64"),
            work_budget: 1_000_000,
            domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: Some(Box::new(std::io::Cursor::new(input))),
            clock: Some(TerminalClockSnapshot::capture_host_epoch().unwrap().into()),
        })
        .unwrap_or_else(|error| panic!("{name}: Terminal admission: {error:?}"));
        let exit = match result.status {
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => 0,
            TerminalExecutionStatus::ExternalCompletion(completion) => completion.return_code,
            other => panic!("{name}: Terminal outcome: {other:?}"),
        };
        observe(
            &evidence,
            name,
            "Terminal",
            exit,
            &result.stdout,
            &result.stderr,
            *expected_exit,
            &expected_stdout,
        );
    }
    if terminal_only {
        println!(
            "matrix\tpassed\t{} inputs\t{} Terminal executions",
            cases.len(),
            cases.len()
        );
        return;
    }
    let (_, executable) =
        orchestration::publication::publish_native_artifact(report, &evidence.join("native"))
            .unwrap();
    fs::copy(&executable, evidence.join("spectrometer.native")).unwrap();
    for (name, expected_exit) in &cases {
        let input = match fs::File::open(input_path(name)) {
            Ok(file) => Stdio::from(file),
            Err(error) if fixture && error.kind() == std::io::ErrorKind::NotFound => Stdio::null(),
            Err(error) => panic!("input: {error}"),
        };
        let expected_stdout = fs::read(expected_path(name)).unwrap_or_else(|error| {
            if fixture && error.kind() == std::io::ErrorKind::NotFound {
                Vec::new()
            } else {
                panic!("stdout oracle: {error}")
            }
        });
        let output = Command::new(&executable)
            .current_dir(&evidence)
            .stdin(input)
            .output()
            .unwrap();
        let exit = output
            .status
            .code()
            .expect("normal native process termination");
        observe(
            &evidence,
            name,
            "native",
            exit,
            &output.stdout,
            &output.stderr,
            *expected_exit,
            &expected_stdout,
        );
    }
    println!(
        "matrix\tpassed\t{} inputs\t{} executions",
        cases.len(),
        cases.len() * 2
    );
}
