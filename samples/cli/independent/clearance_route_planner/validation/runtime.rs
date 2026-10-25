//! Observe original frozen requests through one normally admitted product.
//! Native and public Terminal buffers are captured before oracle comparison.
use terminal_runtime::{TerminalDomainExitDelegation, TerminalRuntimeRequest};
use package_manager::operations::{
    LocalProjectPreparationOptions, PreparedLocalProjectNativeRequest,
    compile_prepared_local_project_for_native, prepare_local_project,
};
use std::{
    fs,
    io::{Cursor, Write},
    path::Path,
    process::{Command, Stdio},
};
use terminal_interpreter::TerminalExecutionStatus;

fn main() {
    let result = std::thread::Builder::new()
        .name("clearance-roadmap-validation".into())
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
    let root = std::env::args_os().nth(1).ok_or("missing sample root")?;
    let root = Path::new(&root)
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let target = target::TargetProfile::host_if_supported().ok_or("unsupported host")?;
    let output = root.join("build/runtime-validation");
    fs::create_dir_all(&output).map_err(|error| error.to_string())?;
    let prepared = prepare_local_project(
        &root.join("main.omg"),
        LocalProjectPreparationOptions {
            target,
            offline: true,
        },
    )
    .map_err(|error| error.to_string())?
    .ok_or("missing complete project")?;
    let report = compile_prepared_local_project_for_native(PreparedLocalProjectNativeRequest::new(
        prepared,
        output.join("native-build"),
        target,
    ))
    .map_err(|error| error.to_string())?;
    let artifact = report
        .retained_native_artifact()
        .map(|native| native.psi_artifact())
        .ok_or("missing retained Terminal artifact")?;
    fs::write(output.join("application.terminal"), artifact.to_bytes())
        .map_err(|error| error.to_string())?;
    fs::write(
        output.join("compiler.identity"),
        compiler::compiler_identity(),
    )
    .map_err(|error| error.to_string())?;
    let frozen: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("data/expected.json")).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let cases = frozen["cases"].as_array().ok_or("missing frozen cases")?;
    if cases.len() != 22 {
        return Err("expected exactly 22 original cases".into());
    }
    let mut terminal_exits = Vec::with_capacity(cases.len());
    for case in cases {
        let name = case["name"].as_str().ok_or("missing case name")?;
        let input = case["input"]
            .as_str()
            .ok_or("missing case input")?
            .as_bytes();
        let expected = case["stdout"]
            .as_str()
            .ok_or("missing expected stdout")?
            .as_bytes();
        let expected_exit = case["exit"].as_i64().ok_or("missing expected exit")?;
        let terminal = terminal_runtime::execute(TerminalRuntimeRequest {
            x86_flags: terminal_runtime::capture_host_x86_flags(target.target_name()).expect("host flags capture"),
            artifact,
            target: Some(target.target_name()),
            provisions: report.runtime_provisions(),
            provider_proposal: None,
            fuel_allowance: 10_000_000,
            domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: Some(Box::new(Cursor::new(input.to_vec()))),
            clock: None,
        })
        .map_err(|error| format!("{name}: {error}"))?;
        fs::write(
            output.join(format!("{name}.terminal.stdout")),
            &terminal.stdout,
        )
        .map_err(|error| error.to_string())?;
        fs::write(
            output.join(format!("{name}.terminal.stderr")),
            &terminal.stderr,
        )
        .map_err(|error| error.to_string())?;
        let TerminalExecutionStatus::ExternalCompletion(completion) = terminal.status else {
            return Err(format!(
                "{name}: unexpected Terminal status {:?}",
                terminal.status
            ));
        };
        fs::write(
            output.join(format!("{name}.terminal.exit")),
            format!("{}\n", completion.return_code),
        )
        .map_err(|error| error.to_string())?;
        if i64::from(completion.return_code) != expected_exit
            || terminal.stdout != expected
            || !terminal.stderr.is_empty()
        {
            return Err(format!("{name}: Terminal stdout, stderr or exit differs"));
        }
        terminal_exits.push(completion.return_code);
    }
    // Compilation retains the checked product. Its ordinary publication
    // transition installs the executable and consumes pre-publication custody.
    let report = compilation_products::publication::publish_compilation(
        report,
        &output.join("native-build"),
    )
    .map_err(|error| error.to_string())?;
    let executable = report
        .checked_native_executable_path()
        .ok_or("missing published native executable")?;
    let mut records = String::from("case\tnative_exit\tterminal_exit\n");
    for (case, terminal_exit) in cases.iter().zip(terminal_exits) {
        let name = case["name"].as_str().ok_or("missing case name")?;
        let input = case["input"]
            .as_str()
            .ok_or("missing case input")?
            .as_bytes();
        let expected = case["stdout"]
            .as_str()
            .ok_or("missing expected stdout")?
            .as_bytes();
        let expected_exit = case["exit"].as_i64().ok_or("missing expected exit")?;
        let mut child = Command::new(executable)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| error.to_string())?;
        child
            .stdin
            .take()
            .ok_or("missing native stdin")?
            .write_all(input)
            .map_err(|error| error.to_string())?;
        let native = child
            .wait_with_output()
            .map_err(|error| error.to_string())?;
        fs::write(output.join(format!("{name}.native.stdout")), &native.stdout)
            .map_err(|error| error.to_string())?;
        fs::write(output.join(format!("{name}.native.stderr")), &native.stderr)
            .map_err(|error| error.to_string())?;
        let native_exit = native
            .status
            .code()
            .ok_or_else(|| format!("{name}: native signal"))?;
        records.push_str(&format!("{name}\t{native_exit}\t{terminal_exit}\n"));
        fs::write(output.join("records.tsv"), &records).map_err(|error| error.to_string())?;
        if i64::from(native_exit) != expected_exit
            || native.stdout != expected
            || !native.stderr.is_empty()
        {
            return Err(format!("{name}: native stdout, stderr or exit differs"));
        }
        println!("{name}: native and captured Terminal stdout/stderr/exit verified");
    }
    Ok(())
}
