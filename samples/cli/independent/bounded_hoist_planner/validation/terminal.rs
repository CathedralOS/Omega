//! Validate buffered Terminal output against the same frozen native oracles.
//! This tool consumes the normal prepared native report's exact Terminal
//! artifact and admitted provisions. It neither interprets checked trees nor
//! reruns Build per input. Every input receives an isolated runtime instance.
use compiler::runtime::{TerminalDomainExitDelegation, TerminalRuntimeRequest};
use package_manager::operations::{
    LocalProjectPreparationOptions, PreparedLocalProjectNativeRequest,
    compile_prepared_local_project_for_native, prepare_local_project,
};
use std::{fs, io::Cursor, path::PathBuf};
use target::TargetProfile;
use terminal_interpreter::TerminalExecutionStatus;

fn main() {
    // Match the supported CLI's compiler stack provision.
    let result = std::thread::Builder::new()
        .name("add11-terminal-validation".into())
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
    let root = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("missing sample directory")?,
    )
    .canonicalize()
    .map_err(|error| error.to_string())?;
    let target = TargetProfile::host_if_supported().ok_or("unsupported validation host")?;
    let build_directory = root.join("build/terminal-compilation");
    fs::create_dir_all(&build_directory).map_err(|error| error.to_string())?;
    let prepared = prepare_local_project(
        &root.join("main.omg"),
        LocalProjectPreparationOptions {
            target,
            offline: false,
        },
    )
    .map_err(|error| error.to_string())?
    .ok_or("missing complete package build")?;
    let report = compile_prepared_local_project_for_native(PreparedLocalProjectNativeRequest::new(
        prepared,
        build_directory,
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
        .ok_or("normal native report did not retain its Terminal artifact")?;
    let output_directory = root.join("build/validation/terminal");
    fs::create_dir_all(&output_directory).map_err(|error| error.to_string())?;
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("data/frozen_oracles.json")).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let cases = manifest["cases"]
        .as_array()
        .ok_or("missing frozen cases")?
        .iter()
        .map(|case| {
            (
                case["name"].as_str().unwrap_or_default().to_string(),
                case["exit"].as_i64().unwrap_or_default() as i32,
            )
        })
        .collect::<Vec<(String, i32)>>();
    let mut verified = 0;
    for (name, expected_exit) in &cases {
        let name = name.as_str();
        let expected_exit = *expected_exit;
        let input = fs::read(root.join("data").join(format!("{name}.stdin")))
            .map_err(|error| error.to_string())?;
        let expected = fs::read(root.join("data").join(format!("{name}.stdout")))
            .map_err(|error| error.to_string())?;
        let observation = compiler::runtime::execute(TerminalRuntimeRequest {
            artifact,
            target: Some(target.target_name()),
            provisions: report.runtime_provisions(),
            provider_proposal: None,
            fuel_allowance: 10_000_000,
            domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: Some(Box::new(Cursor::new(input))),
            clock: None,
        })
        .map_err(|error| format!("{name}: {error}"))?;
        fs::write(
            output_directory.join(format!("{name}.stdout.txt")),
            &observation.stdout,
        )
        .map_err(|error| error.to_string())?;
        fs::write(
            output_directory.join(format!("{name}.stderr.txt")),
            &observation.stderr,
        )
        .map_err(|error| error.to_string())?;
        let actual_exit = match observation.status {
            TerminalExecutionStatus::ExternalCompletion(completion) => completion.return_code,
            other => return Err(format!("{name}: unexpected Terminal status {other:?}")),
        };
        fs::write(
            output_directory.join(format!("{name}.exit.txt")),
            actual_exit.to_string(),
        )
        .map_err(|error| error.to_string())?;
        if actual_exit != expected_exit
            || observation.stdout != expected
            || !observation.stderr.is_empty()
        {
            return Err(format!(
                "{name}: Terminal exit/output differs from frozen oracle; exit {actual_exit}"
            ));
        }
        println!("terminal: {name} exact exit/stdout/stderr verified");
        verified += 1;
    }
    println!("{verified} Terminal cases verified");
    Ok(())
}
