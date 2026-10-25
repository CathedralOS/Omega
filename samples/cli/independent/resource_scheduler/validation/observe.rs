//! Observation-only validation consumer of existing public product/runtime APIs.
//! Scheduling remains in Omega. One compile retains the exact Terminal artifact;
//! each finite fixture executes separately under a fresh private runtime domain.
//! Both streams and exact statuses are exported before native publication.
use terminal_runtime::{TerminalDomainExitDelegation, TerminalRuntimeRequest};
use package_manager::operations::{
    LocalProjectPreparationOptions, PreparedLocalProjectNativeRequest,
    compile_prepared_local_project_for_native, prepare_local_project,
};
use std::{fs, path::PathBuf, process::Command};
use terminal_interpreter::{TerminalExecutionResult, TerminalExecutionStatus};

fn observe() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let arguments: Vec<_> = std::env::args().collect();
    if arguments.len() != 4 {
        return Err("usage: observe MAIN.OMG DATA_DIR OUTPUT_DIR".into());
    }
    let root = PathBuf::from(&arguments[1]);
    let data = PathBuf::from(&arguments[2]);
    let output = PathBuf::from(&arguments[3]);
    fs::create_dir_all(output.join("terminal"))?;
    fs::create_dir_all(output.join("native"))?;
    let target = target::TargetProfile::from_omega_target_name(Some("linux_x86_64"))
        .map_err(|error| error.to_string())?;
    let prepared = prepare_local_project(&root, LocalProjectPreparationOptions {
        target, offline: true,
    }).map_err(|error| error.to_string())?.ok_or("missing package project")?;
    let report = compile_prepared_local_project_for_native(
        PreparedLocalProjectNativeRequest::new(prepared, output.join("product"), target),
    ).map_err(|error| error.to_string())?;
    let artifact = report.retained_native_artifact().map(|value| value.psi_artifact())
        .or_else(|| report.retained_dynamic_elf_artifact().map(|value| value.psi_artifact()))
        .ok_or("native report has no Terminal artifact")?;
    let mut fixtures: Vec<_> = fs::read_dir(&data)?.filter_map(Result::ok)
        .map(|entry| entry.path()).filter(|path| path.extension().is_some_and(|e| e == "input"))
        .collect();
    fixtures.sort();
    for fixture in &fixtures {
        let name = fixture.file_stem().unwrap().to_str().unwrap();
        let result = terminal_runtime::execute(TerminalRuntimeRequest {
            artifact, target: Some("linux_x86_64"), provisions: report.runtime_provisions(),
            fuel_allowance: 10_000_000,
            domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: Some(Box::new(fs::File::open(fixture)?)),
            clock: Some(terminal_runtime::TerminalClockSnapshot::capture_host_epoch()?),
        })?;
        let exit = match result.status {
            TerminalExecutionStatus::ExternalCompletion(value) => value.return_code,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => 0,
            other => return Err(format!("{name}: Terminal did not complete: {other:?}").into()),
        };
        fs::write(output.join("terminal").join(format!("{name}.stdout")), result.stdout)?;
        fs::write(output.join("terminal").join(format!("{name}.stderr")), result.stderr)?;
        fs::write(output.join("terminal").join(format!("{name}.exit")), format!("{exit}\n"))?;
        println!("terminal {name} exit={exit}");
    }
    let (_published, executable) = omega_build::publication::publish_native_artifact(
        report, &output.join("product"),
    )?;
    for fixture in &fixtures {
        let name = fixture.file_stem().unwrap().to_str().unwrap();
        let result = Command::new(&executable).stdin(fs::File::open(fixture)?).output()?;
        let exit = result.status.code().ok_or("native process died by signal")?;
        fs::write(output.join("native").join(format!("{name}.stdout")), result.stdout)?;
        fs::write(output.join("native").join(format!("{name}.stderr")), result.stderr)?;
        fs::write(output.join("native").join(format!("{name}.exit")), format!("{exit}\n"))?;
        println!("native {name} exit={exit}");
    }
    fs::write(output.join("executable.txt"), format!("{}\n", executable.display()))?;
    Ok(())
}
fn main() {
    let result = std::thread::Builder::new().stack_size(256 * 1024 * 1024)
        .spawn(observe).expect("create compiler stack").join().expect("validation panicked");
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
