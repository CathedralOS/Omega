//! Private acceptance driver over the existing complete project operation and
//! admitted Terminal runtime. It captures streams the CLI's --both exit-only
//! comparison does not expose. It never supplies a source interpreter fallback.
use terminal_runtime::{TerminalDomainExitDelegation, TerminalRuntimeRequest};
use package_manager::operations::{LocalProjectPreparationOptions, prepare_local_project};
use omega_build::{PreparedLocalProjectNativeRequest, compile_prepared_local_project_for_native};
use std::{fs, path::PathBuf};
use terminal_interpreter::{TerminalExecutionResult, TerminalExecutionStatus};
fn main() {
    std::thread::Builder::new().stack_size(64 * 1024 * 1024).spawn(run).unwrap().join().unwrap();
}
fn run() {
    let arguments: Vec<_> = std::env::args().collect();
    let root = PathBuf::from(&arguments[1]);
    let output = PathBuf::from(&arguments[2]);
    fs::create_dir_all(&output).unwrap();
    let target = target::TargetProfile::from_omega_target_name(Some("linux_x86_64")).unwrap();
    let prepared = prepare_local_project(&root.join("main.omg"), LocalProjectPreparationOptions { target, offline: false })
        .expect("ordinary accepted project preparation").expect("sample is a project");
    let report = compile_prepared_local_project_for_native(PreparedLocalProjectNativeRequest::new(prepared, output.join("native-build"), target))
        .unwrap_or_else(|error| panic!("ordinary native compilation: {error}"));
    let artifact = report.retained_native_artifact().expect("produced native artifact").psi_artifact();
    let mut fixtures: Vec<_> = fs::read_dir(root.join("data")).unwrap().map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "in")).collect();
    fixtures.sort();
    for fixture in fixtures {
        let name = fixture.file_stem().unwrap().to_str().unwrap();
        let result = terminal_runtime::execute(TerminalRuntimeRequest {
            x86_flags: terminal_runtime::capture_host_x86_flags("linux_x86_64").expect("host flags capture"),
            artifact, target: Some("linux_x86_64"), provisions: report.runtime_provisions(),
            provider_proposal: None,
            fuel_allowance: 10_000_000,
            domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: Some(Box::new(fs::File::open(&fixture).unwrap())), clock: None,
        });
        match result {
            Ok(observation) => {
                fs::write(output.join(format!("{name}.terminal.stdout")), observation.stdout).unwrap();
                fs::write(output.join(format!("{name}.terminal.stderr")), observation.stderr).unwrap();
                fs::write(output.join(format!("{name}.terminal.status")), format!("{:?}\n", observation.status)).unwrap();
                let code = match observation.status {
                    TerminalExecutionStatus::ExternalCompletion(completion) => completion.return_code,
                    TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => 0,
                    other => panic!("{name}: Terminal did not complete: {other:?}"),
                };
                fs::write(output.join(format!("{name}.terminal.exit")), format!("{code}\n")).unwrap();
                println!("Terminal {name}: {code}");
            }
            Err(error) => {
                fs::write(output.join(format!("{name}.terminal.blocker")), error.to_string()).unwrap();
                panic!("{name}: unsupported or failed Terminal route: {error}");
            }
        }
    }
    let (_, executable) = omega_build::publication::publish_native_artifact(report, &output.join("native-build")).unwrap();
    fs::write(output.join("executable.txt"), executable.to_string_lossy().as_bytes()).unwrap();
}
