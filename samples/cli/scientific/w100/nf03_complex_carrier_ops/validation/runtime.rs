//! Observe both native and buffered Terminal execution of the authored projects.
//! Preparation consumes their reviewed package graph; no decisions are rewritten.
use terminal_runtime::{TerminalDomainExitDelegation, TerminalRuntimeRequest};
use package_manager::operations::{
    LocalProjectPreparationOptions, PreparedLocalProjectNativeRequest,
    compile_prepared_local_project_for_native, prepare_local_project,
};
use std::{
    fs,
    io::Cursor,
    path::Path,
    process::{Command, Stdio},
};
use target::TargetProfile;
use terminal_interpreter::TerminalExecutionStatus;

fn main() {
    let result = std::thread::Builder::new()
        .name("nf03-runtime-validation".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(validate)
        .expect("create validation thread")
        .join()
        .expect("validation thread panicked");
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn validate() -> Result<(), String> {
    let root = std::env::args_os()
        .nth(1)
        .ok_or("missing NF03 sample root")?;
    let root = Path::new(&root)
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let target = TargetProfile::host_if_supported().ok_or("unsupported validation host")?;
    for name in ["phasor_power", "quadrature_rotation"] {
        let project = root.join(name);
        let build = project.join("build/runtime-validation");
        fs::create_dir_all(&build).map_err(|error| error.to_string())?;
        let prepared = prepare_local_project(
            &project.join("main.omg"),
            LocalProjectPreparationOptions {
                target,
                offline: true,
            },
        )
        .map_err(|error| format!("{name}: {error}"))?
        .ok_or_else(|| format!("{name}: missing complete project"))?;
        let report = compile_prepared_local_project_for_native(
            PreparedLocalProjectNativeRequest::new(prepared, build.clone(), target),
        )
        .map_err(|error| format!("{name}: {error}"))?;
        let artifact = report
            .retained_native_artifact()
            .map(|artifact| artifact.psi_artifact())
            .or_else(|| {
                report
                    .retained_dynamic_elf_artifact()
                    .map(|artifact| artifact.psi_artifact())
            })
            .ok_or_else(|| format!("{name}: native report retained no Terminal artifact"))?;
        let terminal = terminal_runtime::execute(TerminalRuntimeRequest {
            x86_flags: terminal_runtime::capture_host_x86_flags(target.target_name()).expect("host flags capture"),
            artifact,
            target: Some(target.target_name()),
            provisions: report.runtime_provisions(),
            provider_proposal: None,
            fuel_allowance: 10_000_000,
            domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: Some(Box::new(Cursor::new(Vec::<u8>::new()))),
            clock: None,
        })
        .map_err(|error| format!("{name}: {error}"))?;
        fs::write(build.join("terminal.stdout"), &terminal.stdout)
            .map_err(|error| error.to_string())?;
        fs::write(build.join("terminal.stderr"), &terminal.stderr)
            .map_err(|error| error.to_string())?;
        let exit = match terminal.status {
            TerminalExecutionStatus::ExternalCompletion(completion) => completion.return_code,
            other => return Err(format!("{name}: unexpected Terminal status {other:?}")),
        };
        fs::write(build.join("terminal.exit"), format!("{exit}\n"))
            .map_err(|error| error.to_string())?;
        if exit != 0 || !terminal.stdout.is_empty() || !terminal.stderr.is_empty() {
            return Err(format!(
                "{name}: Terminal exit/streams differ from frozen expectations"
            ));
        }
        let report = compilation_products::publication::publish_compilation(report, &build)
            .map_err(|error| format!("{name}: {error}"))?;
        let executable = report
            .checked_native_executable_path()
            .ok_or_else(|| format!("{name}: no published native executable"))?;
        let native = Command::new(executable)
            .stdin(Stdio::null())
            .output()
            .map_err(|error| format!("{name}: {error}"))?;
        fs::write(build.join("native.stdout"), &native.stdout)
            .map_err(|error| error.to_string())?;
        fs::write(build.join("native.stderr"), &native.stderr)
            .map_err(|error| error.to_string())?;
        fs::write(
            build.join("native.exit"),
            format!("{:?}\n", native.status.code()),
        )
        .map_err(|error| error.to_string())?;
        if native.status.code() != Some(0) || !native.stdout.is_empty() || !native.stderr.is_empty()
        {
            return Err(format!(
                "{name}: native exit/streams differ from frozen expectations"
            ));
        }
        println!("{name}: native and captured Terminal exit 0, empty stdout/stderr verified");
    }
    Ok(())
}
