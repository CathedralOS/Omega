//! Validate buffered Terminal output against the same frozen native oracles.
//! This tool consumes the normal prepared native report's exact Terminal
//! artifact and admitted provisions. It neither interprets checked trees nor
//! reruns Build per input. Every input receives an isolated runtime instance.
use terminal_runtime::{TerminalDomainExitDelegation, TerminalRuntimeRequest};
use package_manager::operations::{LocalProjectPreparationOptions, prepare_local_project};
use omega_build::{PreparedLocalProjectNativeRequest, compile_prepared_local_project_for_native};
use std::{fs, io::Cursor, path::PathBuf};
use target::TargetProfile;
use terminal_interpreter::TerminalExecutionStatus;

fn main() {
    // Match the supported CLI's compiler stack provision.
    let result = std::thread::Builder::new()
        .name("sci04-terminal-validation".into())
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
        &fs::read(root.join("data/oracle.json")).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let cases = manifest["cases"]
        .as_array()
        .ok_or("missing frozen cases")?
        .iter()
        .map(|case| {
            let name = case["name"].as_str().ok_or("non-string case name")?;
            if name.is_empty()
                || name.len() > 64
                || !name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            {
                return Err("invalid case filename".to_string());
            }
            let exit = case["exit"].as_i64().ok_or("non-integer case exit")?;
            let exit = i32::try_from(exit).map_err(|_| "case exit outside i32")?;
            Ok((name.to_string(), exit))
        })
        .collect::<Result<Vec<(String, i32)>, String>>()?;
    let mut verified = 0;
    for (name, expected_exit) in &cases {
        let name = name.as_str();
        let expected_exit = *expected_exit;
        let input = manifest["cases"]
            .as_array()
            .ok_or("missing cases")?
            .iter()
            .find(|case| case["name"] == name)
            .ok_or("missing input")?["input"]
            .as_str()
            .ok_or("non-string input")?
            .as_bytes()
            .to_vec();
        let observation = terminal_runtime::execute(TerminalRuntimeRequest {
            native_provider: None,
            x86_flags: terminal_runtime::capture_host_x86_flags(target.target_name()).expect("host flags capture"),
            artifact,
            target: Some(target.target_name()),
            provisions: report.runtime_provisions(),
            provider_proposal: None,
            fuel_allowance: 100_000_000,
            domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: Some(Box::new(Cursor::new(input))),
            clock: None,
        })
        .map_err(|error| format!("{name}: {error}"))?;
        fs::write(
            output_directory.join(format!("{name}.stdout")),
            &observation.stdout,
        )
        .map_err(|error| error.to_string())?;
        fs::write(
            output_directory.join(format!("{name}.stderr")),
            &observation.stderr,
        )
        .map_err(|error| error.to_string())?;
        let actual_exit = match observation.status {
            TerminalExecutionStatus::ExternalCompletion(completion) => completion.return_code,
            other => return Err(format!("{name}: unexpected Terminal status {other:?}")),
        };
        fs::write(
            output_directory.join(format!("{name}.exit")),
            actual_exit.to_string(),
        )
        .map_err(|error| error.to_string())?;
        if actual_exit != expected_exit || !observation.stderr.is_empty() {
            return Err(format!(
                "{name}: Terminal exit/output differs from frozen oracle; exit {actual_exit}"
            ));
        }
        println!("terminal: {name} captured for independent numerical validation");
        verified += 1;
    }
    println!("{verified} Terminal cases captured; run validate.py --captured on this directory");
    Ok(())
}
