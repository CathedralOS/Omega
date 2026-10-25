//! Qualify many finite inputs against one normally produced native/Terminal product.
//!
//! `mbx run -p omega --example qualify_inputs -- SOURCE EVIDENCE INPUT...`
//! prepares the accepted local project and compiles it once. Each input owns a
//! fresh native process and an isolated Terminal invocation through the same
//! runtime facade as `omega run --both`. The artifact's retained entry/provider
//! provisions are reused; custody, actual host admission and completion checks
//! still run for every invocation. No package review decision is made here.
//! Streams and exact semantic statuses are retained for an independent oracle
//! validator. This is repository qualification tooling, not shipped CLI coverage.

use package_manager::operations::{
    LocalProjectPreparationOptions, PreparedLocalProjectNativeRequest,
    compile_prepared_local_project_for_native, prepare_local_project,
};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use terminal_runtime::{TerminalDomainExitDelegation, TerminalRuntimeRequest};

fn main() -> Result<(), String> {
    std::thread::Builder::new()
        .name("qualify-inputs".to_owned())
        .stack_size(256 * 1024 * 1024)
        .spawn(|| qualify().map_err(|error| error.to_string()))
        .map_err(|error| error.to_string())?
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
}

fn qualify() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let source = PathBuf::from(arguments.next().ok_or("missing SOURCE")?);
    let evidence = PathBuf::from(arguments.next().ok_or("missing EVIDENCE")?);
    std::fs::create_dir_all(&evidence)?;
    let evidence = evidence.canonicalize()?;
    let mut inputs: Vec<PathBuf> = Vec::new();
    for input in arguments.map(PathBuf::from) {
        let input = input.canonicalize()?;
        let directory = input.parent().ok_or("input has no evidence directory")?;
        if !directory.starts_with(&evidence) {
            return Err("input capture must be inside the requested evidence directory".into());
        }
        if inputs
            .iter()
            .any(|previous| previous.parent() == Some(directory))
        {
            return Err("each INPUT needs its own capture directory".into());
        }
        inputs.push(input);
    }
    if inputs.is_empty() {
        return Err("expected at least one finite INPUT file".into());
    }
    // The target is this executor's actual host. A foreign product cannot be
    // launched merely because its source declared another entry binding.
    let target = target::TargetProfile::from_omega_target_name(None)
        .map_err(|diagnostic| format!("{diagnostic:?}"))?;
    let prepared = prepare_local_project(
        &source,
        LocalProjectPreparationOptions {
            target,
            offline: false,
        },
    )?
    .ok_or("qualification requires a project with build.omg")?;
    let started = Instant::now();
    let report = compile_prepared_local_project_for_native(
        PreparedLocalProjectNativeRequest::new(prepared, evidence.join("product"), target),
    )?;
    let artifact = report
        .retained_native_artifact()
        .ok_or("publication lost retained Terminal artifact")?
        .psi_artifact();
    std::fs::write(
        evidence.join("production.seconds"),
        started.elapsed().as_secs_f64().to_string(),
    )?;
    for input in &inputs {
        let input = input.canonicalize()?;
        let directory = input.parent().ok_or("input has no evidence directory")?;
        if !directory.starts_with(&evidence) {
            return Err("input capture must be inside the requested evidence directory".into());
        }
        let started = Instant::now();
        let terminal = terminal_runtime::execute(TerminalRuntimeRequest {
            x86_flags: None,
            provider_proposal: None,
            artifact,
            target: Some(target.target_name()),
            provisions: report.runtime_provisions(),
            fuel_allowance: 10_000_000,
            domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: Some(Box::new(std::fs::File::open(&input)?)),
            clock: Some(terminal_runtime::TerminalClockSnapshot::capture_host_epoch()?),
        })?;
        std::fs::write(directory.join("terminal.stdout"), terminal.stdout)?;
        std::fs::write(directory.join("terminal.stderr"), terminal.stderr)?;
        std::fs::write(
            directory.join("terminal.status"),
            format!("{:?}", terminal.status),
        )?;
        std::fs::write(
            directory.join("terminal.seconds"),
            started.elapsed().as_secs_f64().to_string(),
        )?;
        println!("{}", directory.display());
    }
    // Publication transfers the retained artifact into a receipt. Finish every
    // reference invocation before that transfer, then launch only the executable
    // path whose custody the receipt establishes.
    let report =
        compilation_products::publication::publish_compilation(report, &evidence.join("product"))?;
    let executable = report
        .checked_native_executable_path()
        .ok_or("publication lost executable custody")?;
    for input in &inputs {
        let input = input.canonicalize()?;
        let directory = input.parent().ok_or("input has no evidence directory")?;
        let started = Instant::now();
        let stdout = std::fs::File::create(directory.join("native.stdout"))?;
        let stderr = std::fs::File::create(directory.join("native.stderr"))?;
        let mut child = Command::new(executable)
            .stdin(Stdio::from(std::fs::File::open(&input)?))
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()?;
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            // Sponsor bounds are harness availability limits, not a guest
            // completion or divergence verdict. Reap the actual child before
            // reporting exhaustion so a failed qualification cannot orphan it.
            let output_bytes = std::fs::metadata(directory.join("native.stdout"))?.len()
                + std::fs::metadata(directory.join("native.stderr"))?.len();
            if started.elapsed() >= Duration::from_secs(30) || output_bytes > 16 * 1024 * 1024 {
                child.kill()?;
                child.wait()?;
                return Err(
                    "native qualification exceeded its 30-second/16-MiB sponsor budget".into(),
                );
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        // A fast child may finish between polls; its completed captures still
        // owe the same sponsor output bound as a running child.
        let output_bytes = std::fs::metadata(directory.join("native.stdout"))?.len()
            + std::fs::metadata(directory.join("native.stderr"))?.len();
        if output_bytes > 16 * 1024 * 1024 {
            return Err("native qualification exceeded its 16-MiB output budget".into());
        }
        std::fs::write(
            directory.join("native.status"),
            status
                .code()
                .ok_or("native invocation ended by signal")?
                .to_string(),
        )?;
        std::fs::write(
            directory.join("native.seconds"),
            started.elapsed().as_secs_f64().to_string(),
        )?;
    }
    Ok(())
}
