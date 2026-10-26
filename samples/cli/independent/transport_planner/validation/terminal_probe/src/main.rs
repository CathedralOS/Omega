//! Validation-only consumer of the same accepted native product and retained
//! Terminal artifact as `omega run`. Captures BOTH output streams and checks
//! them against independently frozen oracles; never substitutes host solving.
use terminal_runtime::{TerminalDomainExitDelegation, TerminalRuntimeRequest};
use package_manager::operations::{LocalProjectPreparationOptions, prepare_local_project};
use omega_build::{PreparedLocalProjectNativeRequest, compile_prepared_local_project_for_native};
use std::{fs, io::Cursor, path::PathBuf};
use terminal_interpreter::{TerminalExecutionResult, TerminalExecutionStatus};

fn main() {
    let root = PathBuf::from(std::env::args().nth(1).expect("sample directory"))
        .canonicalize()
        .expect("sample exists");
    let target =
        target::TargetProfile::from_omega_target_name(Some("linux_x86_64")).expect("known target");
    let prepared = prepare_local_project(
        &root.join("main.omg"),
        LocalProjectPreparationOptions {
            target,
            offline: true,
        },
    )
    .expect("accepted project preparation")
    .expect("declared project");
    let report = compile_prepared_local_project_for_native(PreparedLocalProjectNativeRequest::new(
        prepared,
        root.join("build/terminal-probe"),
        target,
    ))
    .expect("complete native product");
    let artifact = report
        .retained_native_artifact()
        .expect("retained native artifact")
        .psi_artifact();
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("validation/oracles.json")).expect("frozen oracles"),
    )
    .expect("oracle JSON");
    assert_eq!(manifest["tolerance"], 0);
    let mut records = Vec::new();
    // Nontrivial variants plus representative tiny nonzero, zero and negative
    // cases. Native separately runs the entire exhaustive tiny grid.
    let cases = manifest["cases"].as_array().expect("cases");
    for case in cases
        .iter()
        .take(8)
        .chain(cases.iter().skip(8).step_by(137))
    {
        let values = case["values"].as_array().expect("values");
        let input = values
            .iter()
            .map(|value| value.as_i64().unwrap().to_string())
            .collect::<Vec<_>>()
            .join(" ")
            + "\n";
        let observed = terminal_runtime::execute(TerminalRuntimeRequest {
            native_provider: None,
            x86_flags: terminal_runtime::capture_host_x86_flags("linux_x86_64").expect("host flags capture"),
            artifact,
            provider_proposal: None,
            target: Some("linux_x86_64"),
            provisions: report.runtime_provisions(),
            fuel_allowance: 10_000_000,
            domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: Some(Box::new(Cursor::new(input.into_bytes()))),
            clock: None,
        })
        .expect("Terminal execution available");
        let exit = match observed.status {
            TerminalExecutionStatus::ExternalCompletion(completion) => completion.return_code,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => 0,
            status => panic!("Terminal did not complete: {status:?}"),
        };
        assert_eq!(
            exit as i64,
            case["exit"].as_i64().unwrap(),
            "{}",
            case["name"]
        );
        assert_eq!(
            observed.stdout,
            case["stdout"].as_str().unwrap().as_bytes(),
            "{}",
            case["name"]
        );
        assert!(
            observed.stderr.is_empty(),
            "Terminal stderr: {:?}",
            observed.stderr
        );
        records.push(serde_json::json!({"name": case["name"], "exit": exit,
                                      "stdout": String::from_utf8(observed.stdout).unwrap()}));
    }
    for (name, input) in manifest["invalid"].as_object().expect("invalid inputs") {
        let observed = terminal_runtime::execute(TerminalRuntimeRequest {
            native_provider: None,
            x86_flags: terminal_runtime::capture_host_x86_flags("linux_x86_64").expect("host flags capture"),
            artifact,
            provider_proposal: None,
            target: Some("linux_x86_64"),
            provisions: report.runtime_provisions(),
            fuel_allowance: 10_000_000,
            domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: Some(Box::new(Cursor::new(
                input.as_str().unwrap().as_bytes().to_vec(),
            ))),
            clock: None,
        })
        .expect("Terminal invalid input execution");
        let exit = match observed.status {
            TerminalExecutionStatus::ExternalCompletion(completion) => completion.return_code,
            status => panic!("Terminal invalid input did not exit: {status:?}"),
        };
        assert_eq!(exit, 2, "{name}");
        assert_eq!(
            observed.stdout,
            manifest["invalid_stdout"].as_str().unwrap().as_bytes(),
            "{name}"
        );
        assert!(observed.stderr.is_empty());
        records.push(
            serde_json::json!({"name": format!("invalid_{name}"), "exit": exit,
                                      "stdout": String::from_utf8(observed.stdout).unwrap()}),
        );
    }
    let count = records.len();
    fs::write(
        root.join("build/terminal-validation.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"engine": "Terminal", "count": count,
                                                    "tolerance": 0, "records": records}))
        .unwrap(),
    )
    .expect("write validation receipt");
    println!("PASS: {count} Terminal cases; exact exit/stdout/stderr oracles");
}
