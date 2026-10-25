//! Validation sponsor over the unchanged accepted project and its exact retained
//! Terminal artifact. No reconstructed grants, provider policy, or source oracle.
use terminal_runtime::{
    TerminalClockSnapshot, TerminalDomainExitDelegation, TerminalRuntimeRequest,
};
use package_manager::operations::{
    LocalProjectPreparationOptions, PreparedLocalProjectNativeRequest,
    compile_prepared_local_project_for_native, prepare_local_project,
};
use std::{fs, io::Cursor, path::PathBuf, time::Instant};
use terminal_interpreter::{TerminalExecutionResult, TerminalExecutionStatus};

fn main() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(run)
        .unwrap()
        .join()
        .unwrap();
}
fn run() {
    let args: Vec<_> = std::env::args_os().collect();
    assert_eq!(args.len(), 4, "PROJECT CASE_NAMES OUTPUT");
    let project = PathBuf::from(&args[1]);
    let names = fs::read_to_string(&args[2]).unwrap();
    assert_eq!(names.lines().count(), 44);
    assert_eq!(
        names
            .lines()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        44,
        "case roster must not substitute repeated cases"
    );
    let output = PathBuf::from(&args[3]);
    fs::create_dir_all(&output).unwrap();
    let target = target::TargetProfile::LinuxX64;
    let started = Instant::now();
    let prepared = prepare_local_project(
        &project.join("main.omg"),
        LocalProjectPreparationOptions {
            target,
            offline: true,
        },
    )
    .unwrap_or_else(|error| panic!("accepted preparation failed: {error}"))
    .expect("original project has its accepted package graph");
    let report = compile_prepared_local_project_for_native(PreparedLocalProjectNativeRequest::new(
        prepared,
        output.join("compile"),
        target,
    ))
    .unwrap_or_else(|error| panic!("accepted native product failed: {error:?}"));
    let artifact = report
        .retained_native_artifact()
        .map(|native| native.psi_artifact())
        .or_else(|| {
            report
                .retained_dynamic_elf_artifact()
                .map(|native| native.psi_artifact())
        })
        .expect("native product retains exact produced Terminal artifact");
    let mut records = format!("compile_seconds\t{:.3}\n", started.elapsed().as_secs_f64());
    let mut passed = true;
    let mut count = 0;
    for name in names.lines() {
        assert!(!name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'));
        let fixture = project.join("data").join(name);
        let input = fs::read(fixture.join("input.txt")).unwrap();
        let expected_exit: i32 = fs::read_to_string(fixture.join("expected_exit.txt"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let started = Instant::now();
        let observation = terminal_runtime::execute(TerminalRuntimeRequest {
            artifact,
            target: Some("linux_x86_64"),
            provisions: report.runtime_provisions(),
            provider_proposal: None,
            fuel_allowance: 100_000_000,
            domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: Some(Box::new(Cursor::new(input))),
            clock: Some(TerminalClockSnapshot::capture_host_epoch().unwrap()),
        });
        match observation {
            Ok(observation) => {
                let exit = match &observation.status {
                    TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => Some(0),
                    TerminalExecutionStatus::ExternalCompletion(completion) => {
                        Some(completion.return_code)
                    }
                    _ => None,
                };
                fs::write(output.join(format!("{name}.stdout")), &observation.stdout).unwrap();
                fs::write(output.join(format!("{name}.stderr")), &observation.stderr).unwrap();
                let ok = exit == Some(expected_exit) && observation.stderr.is_empty();
                passed &= ok;
                records.push_str(&format!(
                    "{name}\t{}\t{exit:?}\t{:.3}\t{:?}\n",
                    if ok { "PASS" } else { "FAIL" },
                    started.elapsed().as_secs_f64(),
                    observation.status
                ));
                println!("{name} {} {exit:?}", if ok { "PASS" } else { "FAIL" });
            }
            Err(error) => {
                passed = false;
                records.push_str(&format!("{name}\tFAIL\t{error:?}\n"));
                println!("{name} FAIL {error:?}");
            }
        }
        count += 1;
        fs::write(output.join("records.tsv"), &records).unwrap();
        if !passed {
            break;
        }
    }
    assert_eq!(count, 44, "must observe all frozen cases");
    assert!(passed, "every exact Terminal output and exit must match");
}
