//! A canonical safe boundary contract retains a real provider-owned cell.
//! Exercise ordinary produced provisions, rather than a symbol-specific model.

#![cfg(target_os = "linux")]

use orchestration::{BuildRequest, BuildTargets, NativeDestination, ProjectProduct};
use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Stdio};
use terminal_runtime::{
    TerminalDomainExitDelegation, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalRuntimeRequest,
};

#[test]
fn internal_native_provider_worker() {
    terminal_runtime::run_native_provider_worker_if_requested();
}

fn compile(command: &mut Command) {
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "{command:?}: {:?}\n{}",
        result.status,
        String::from_utf8_lossy(&result.stderr)
    );
}

fn execute_provider_fixture(
    forward: bool,
) -> Result<terminal_runtime::TerminalRuntimeObservation, terminal_runtime::TerminalRuntimeError> {
    let directory = TemporaryDirectory::create("provider-reference", false).unwrap();
    let root = directory.path().join("source");
    std::fs::create_dir(&root).unwrap();
    let library = directory.path().join("libomega_reference.so");
    let provider = directory.path().join("provider.c");
    let version = directory.path().join("provider.map");
    std::fs::write(
        &provider,
        if forward {
            "#include <stdint.h>\nstatic int32_t cell = 37;\nconst int32_t *omega_reference_cell(void) { return &cell; }\nint32_t omega_reference_read(const int32_t *value) { return *value; }\n"
        } else {
            "#include <stdint.h>\nstatic int32_t cell = 37;\nconst int32_t *omega_reference_cell(void) { return &cell; }\n"
        },
    )
    .unwrap();
    std::fs::write(
        &version,
        if forward {
            "REFERENCE_1.0 { global: omega_reference_cell; omega_reference_read; local: *; };\n"
        } else {
            "REFERENCE_1.0 { global: omega_reference_cell; local: *; };\n"
        },
    )
    .unwrap();
    compile(
        Command::new(std::env::var_os("CC").unwrap_or_else(|| "cc".into()))
            .args([
                "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-shared", "-fPIC",
            ])
            .arg(format!("-Wl,--version-script={}", version.display()))
            .arg(&provider)
            .arg("-o")
            .arg(&library),
    );
    std::fs::write(
        root.join("main.omg"),
        if forward {
            r#"
boundary trait Source { machine cell() -> &i32 reaches none crashes none; }
boundary trait Sink { machine read(cell: &i32) -> i32 reaches none crashes none; }
machine main() reaches Source + Sink invokes Source invokes Sink crashes Trap {
    let observed: i32 = Sink::read(Source::cell());
    transition observed == 37i32 { true -> done() _ -> bad() }
    machine done() {}
    machine bad() { crash Trap; }
}
"#
        } else {
            r#"
boundary trait Source { machine cell() -> &i32 reaches none crashes none; }
machine read(cell: &i32) -> i32 { cell }
machine main() reaches Source invokes Source crashes Trap {
    let observed: i32 = read(Source::cell());
    transition observed == 37i32 { true -> done() _ -> bad() }
    machine done() {}
    machine bad() { crash Trap; }
}
"#
        },
    )
    .unwrap();
    let profile = target::TargetProfile::host();
    let forwarding_binding = if forward {
        format!(
            r#"    Build::bind<Sink::read>(builder, {target}, ForeignBinding::DllImport {{
        import: DllImport::ElfVersioned {{ object: "{library}", symbol: "omega_reference_read", version: "REFERENCE_1.0" }}
    }});
"#,
            target = profile.target_name(),
            library = library.display()
        )
    } else {
        String::new()
    };
    std::fs::write(
        root.join("build.omg"),
        format!(
            r#"
use omega::language::core::DllImport;
use omega::language::core::ForeignBinding;
machine build(builder: &mut Build) {{
    Build::application(builder, "provider_reference");
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
    Build::bind(builder, linux_arm64::ProgramEntry, main);
    Build::bind(builder, macos_arm64::ProgramEntry, main);
    Build::bind(builder, windows_x86_64::ProgramEntry, main);
    Build::bind<Source::cell>(builder, {target}, ForeignBinding::DllImport {{
        import: DllImport::ElfVersioned {{ object: "{library}", symbol: "omega_reference_cell", version: "REFERENCE_1.0" }}
    }});
{forwarding_binding}}}
"#,
            target = profile.target_name(),
            library = library.display(),
        ),
    )
    .unwrap();
    let outcomes = orchestration::build(BuildRequest {
        stage_validators: Default::default(),
        root_path: root.join("main.omg"),
        build_directory: Some(directory.path().join("build")),
        targets: BuildTargets::Single(profile),
        product: ProjectProduct::Native {
            installation_assumptions: Vec::new(),
            destination: NativeDestination::Retained,
            private_entry_ram: Vec::new(),
        },
        timings: false,
        offline: true,
        verify: false,
        require_package_project: true,
        optimization_rollback: Default::default(),
        build_snapshot: None,
    })
    .unwrap();
    assert_eq!(outcomes.len(), 1);
    let report = outcomes
        .into_iter()
        .next()
        .unwrap()
        .result
        .unwrap()
        .into_report()
        .unwrap()
        .report;
    let artifact = report.retained_native_program().unwrap().psi_artifact();
    let program = terminal_interpreter::TerminalProgram::borrow(artifact.view());
    let boundary = program
        .module()
        .boundary_machines
        .iter()
        .find(|boundary| {
            matches!(
                boundary.result,
                terminal_psi::BoundaryMachineResult::Structural(_)
            )
        })
        .expect("the produced program retains its reference boundary");
    assert!(boundary.result_reference_regions.is_empty());
    let groups = boundary.result_lifetimes.as_ref().unwrap();
    assert_eq!(groups.len(), 1);
    assert!(groups[0].inputs.is_empty());
    assert_eq!(groups[0].results, vec![Vec::new()]);
    let mut worker = Command::new(std::env::current_exe().unwrap());
    worker.args(["--exact", "internal_native_provider_worker", "--nocapture"]);
    let observed = terminal_runtime::execute(TerminalRuntimeRequest {
        native_image: None,
        entry: None,
        filesystem: Default::default(),
        native_services: Default::default(),
        native_provider: Some(worker),
        artifact,
        target: Some(profile.target_name()),
        provisions: report.runtime_provisions(),
        provider_proposal: None,
        x86_flags: terminal_runtime::capture_host_x86_flags(profile.target_name()).unwrap(),
        memory_ordering: None,
        work_budget: 100_000,
        domain_exit: TerminalDomainExitDelegation::Unprovided,
        standard_input: None,
        clock: None,
    });
    // Publish and execute this same retained product before requiring the Psi
    // completion, so a runtime refusal does not hide native descriptor custody.
    let (_published, executable) = orchestration::publication::publish_compilation(
        report,
        &directory.path().join("native-output"),
    )
    .expect("publish the same checked native product");
    let native = Command::new(executable.expect("checked native executable publication"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(native.status.code(), Some(0), "{native:?}");
    assert!(native.stdout.is_empty());
    assert!(native.stderr.is_empty());
    observed
}

#[test]
fn canonical_provider_reference_reads_its_live_cell() {
    let observed = execute_provider_fixture(false)
        .expect("the exact admitted provider reference should execute through its live backing");
    assert_eq!(
        observed.status,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert!(observed.stdout.is_empty());
    assert!(observed.stderr.is_empty());
}

#[test]
fn provider_reference_forwarding_requires_actual_pointer_authority() {
    let refusal = match execute_provider_fixture(true) {
        Err(refusal) => format!("{refusal:?}"),
        Ok(observed) => panic!(
            "unknown resident forwarding must refuse: {:?}",
            observed.status
        ),
    };
    assert!(
        refusal.contains("resident scalar belongs to a different original allocation"),
        "{refusal}"
    );
}
