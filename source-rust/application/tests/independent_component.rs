//! A root Build selects an entryless dependency as a separate native supplier.
#![cfg(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "aarch64"),
))]

#[path = "support/components.rs"]
mod components;
#[path = "support/project.rs"]
mod project;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::{fs, path::Path, process::Command};

fn entryless_supplier_sources(root: &Path) -> std::path::PathBuf {
    project::write(
        root,
        &[
            (
                "supplier/build.omg",
                r#"machine build(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: "numbers" }; }"#,
            ),
            (
                "supplier/main.omg",
                r#"
public boundary trait Reading {
    machine value(input: u64) -> u64 reaches none crashes none;
    machine unused() -> u64 reaches none crashes none;
}
public ConstantReading satisfies Reading {
    machine value(input: u64) -> u64 { input }
    machine unused() -> u64 { 11u64 }
}
"#,
            ),
            (
                "application/build.omg",
                r#"
use {omega}/build/dependency.omg;
use main.omg as app;
use {numbers}/main.omg as numbers;
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: "number_application" };
    let dependency: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Path { location: "../supplier" }));
    Build::select_provider::<|numbers|::Reading, |numbers|::ConstantReading>(builder, independent_mode());
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
    Build::bind(builder, windows_arm64::ProgramEntry, |app|::main);
}
machine independent_mode() -> CompositionMode { CompositionMode::Independent }
"#,
            ),
            (
                "application/main.omg",
                r#"
use {numbers}/main.omg as numbers;
machine main() reaches |numbers|::Reading crashes Trap {
    let number: u64 = |numbers|::Reading::value(73u64);
    transition number == 73u64 { true -> done(), false -> bad() }
    machine done() {}
    machine bad() { crash Trap; }
}
"#,
            ),
        ],
    );
    root.join("application")
}

#[cfg(target_os = "linux")]
#[test]
fn native_loads_the_separately_compiled_supplier() {
    let scratch = TemporaryDirectory::create("entryless-component", false).unwrap();
    let root = scratch.path();
    let consumer = entryless_supplier_sources(root);
    let output = root.join("out");
    let review = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args([
            "packages",
            "update",
            "--offline",
            "--target",
            "linux_x86_64",
            "--dangerously-accept-all",
            "--project",
        ])
        .arg(&consumer)
        .output()
        .unwrap();
    assert!(
        review.status.success() && consumer.join("omega.lock").is_file(),
        "the receiver must be able to review its selected supplier: {} {}",
        String::from_utf8_lossy(&review.stdout),
        String::from_utf8_lossy(&review.stderr)
    );
    require_native_suppliers(&consumer, &output, 1);
}

#[test]
fn a_windows_arm64_consumer_imports_the_slot_exported_by_its_component() {
    let scratch = TemporaryDirectory::create("windows-arm64-component", false).unwrap();
    let consumer = entryless_supplier_sources(scratch.path());
    let output = scratch.path().join("out");
    let build = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["--target", "windows_arm64", "--build-dir"])
        .arg(&output)
        .arg(consumer.join("build.omg"))
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let manifest = fs::read_to_string(output.join("omega-components.tsv")).unwrap();
    let mut lines = manifest.lines();
    assert_eq!(lines.next(), Some("omega-native-components-v1"));
    let row = lines.next().expect("one separately compiled supplier");
    assert!(lines.next().is_none());
    let fields: Vec<_> = row.split('\t').collect();
    let [_, library, export] = fields.as_slice() else {
        panic!("{row}");
    };
    assert!(library.ends_with(".dll") && export.starts_with("__omega_slot_"));
    for (name, is_dll) in [("omega-program.exe", false), (*library, true)] {
        let bytes = fs::read(output.join(name)).unwrap();
        let pe = u32::from_le_bytes(bytes[60..64].try_into().unwrap()) as usize;
        assert_eq!(&bytes[pe..pe + 4], b"PE\0\0");
        assert_eq!(
            u16::from_le_bytes(bytes[pe + 4..pe + 6].try_into().unwrap()),
            0xaa64
        );
        let characteristics = u16::from_le_bytes(bytes[pe + 22..pe + 24].try_into().unwrap());
        assert_eq!(characteristics & 0x2000 != 0, is_dll);
        assert!(
            bytes
                .windows(export.len())
                .any(|window| window == export.as_bytes())
        );
    }

    #[cfg(target_os = "windows")]
    {
        // The source traps if the imported supplier returns the wrong value.
        let native = Command::new(output.join("omega-program.exe"))
            .output()
            .unwrap();
        assert!(native.status.success(), "{native:?}");
        let psi = Command::new(env!("CARGO_BIN_EXE_omega"))
            .args(["run", "--target", "windows_arm64"])
            .arg(consumer.join("build.omg"))
            .output()
            .unwrap();
        assert!(psi.status.success(), "{psi:?}");
        assert_eq!(native.stdout, psi.stdout);
        assert_eq!(native.stderr, psi.stderr);
    }
}

#[cfg(target_os = "linux")]
fn require_native_suppliers(consumer: &Path, output: &Path, expected: usize) {
    let build = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["--target", "linux_x86_64", "--build-dir"])
        .arg(output)
        .arg(consumer.join("build.omg"))
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let components: Vec<_> = fs::read_dir(output)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "so"))
        .collect();
    assert_eq!(
        components.len(),
        expected,
        "published suppliers: {components:?}"
    );
    let executable = output.join("omega-program");
    let run = || {
        Command::new(&executable)
            .env_remove("LD_LIBRARY_PATH")
            .output()
            .unwrap()
    };
    let first = run();
    assert!(
        first.status.success(),
        "consumer exited with {}: {}",
        first.status,
        String::from_utf8_lossy(&first.stderr)
    );
    for component in &components {
        let sidecar = component.with_extension("so.component");
        assert!(
            !sidecar.try_exists().unwrap(),
            "unexpected {}",
            sidecar.display()
        );
        let missing = component.with_extension("withheld");
        fs::rename(component, &missing).unwrap();
        let refused = run();
        assert!(
            !refused.status.success(),
            "the consumer ran with its required supplier missing"
        );
        assert!(
            String::from_utf8_lossy(&refused.stderr)
                .contains(component.file_name().unwrap().to_str().unwrap())
        );
        fs::rename(missing, component).unwrap();
    }
}

#[cfg(target_os = "linux")]
#[test]
fn a_fused_dependency_can_select_its_own_independent_supplier() {
    let scratch = TemporaryDirectory::create("nested-component", false).unwrap();
    let root = scratch.path();
    nested_sources(root, "Fused");
    require_native_suppliers(&root.join("application"), &root.join("out"), 1);
}

#[cfg(target_os = "linux")]
#[test]
fn independent_packages_can_call_an_independently_compiled_supplier() {
    let scratch = TemporaryDirectory::create("nested-independent-supplier", false).unwrap();
    let root = scratch.path();
    nested_sources(root, "Independent");
    let consumer = root.join("application");
    let build = consumer.join("build.omg");
    let review = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args([
            "packages",
            "update",
            "--offline",
            "--target",
            "linux_x86_64",
            "--dangerously-accept-all",
            "--project",
        ])
        .arg(&consumer)
        .output()
        .unwrap();
    assert!(
        review.status.success(),
        "{}",
        String::from_utf8_lossy(&review.stderr)
    );
    require_native_suppliers(&consumer, &root.join("out"), 2);
    let interpreted = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["run", "--target", "linux_x86_64"])
        .arg(build)
        .output()
        .unwrap();
    assert!(
        interpreted.status.success(),
        "{}",
        String::from_utf8_lossy(&interpreted.stderr)
    );
}

#[cfg(target_os = "linux")]
fn nested_sources(root: &Path, mode: &str) {
    let root_build = format!(
        r#"use {{omega}}/build/dependency.omg;
use main.omg as app;
use {{numbers}}/main.omg as numbers;
machine build(builder: @mutable BuildPlan) {{
    builder.product = BuildProduct::Application {{ name: "reader" }};
    let number: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Path {{ location: "../supplier" }}));
    Build::select_provider::<|numbers|::Reading, |numbers|::Sensor>(builder, linux_x86_64, CompositionMode::{mode});
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
}}"#
    );
    let sources = [
        (
            "leaf/build.omg",
            r#"machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Package { name: "measurements" };
}"#,
        ),
        (
            "leaf/main.omg",
            r#"public boundary trait Measure {
    machine pass(input: u64) -> u64 reaches none crashes none;
}
public Device satisfies Measure {
    machine pass(input: u64) -> u64 { input }
}"#,
        ),
        (
            "supplier/build.omg",
            r#"use {omega}/build/dependency.omg;
use {measurements}/main.omg as measurements;
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Package { name: "numbers" };
    let leaf: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Path { location: "../leaf" }));
    Build::select_provider::<|measurements|::Measure, |measurements|::Device>(builder, linux_x86_64, CompositionMode::Independent);
}"#,
        ),
        (
            "supplier/main.omg",
            r#"use {measurements}/main.omg as measurements;
public boundary trait Reading {
    machine value(input: u64) -> u64 reaches |measurements|::Measure crashes none;
}
public Sensor satisfies Reading {
    machine value(input: u64) -> u64 reaches |measurements|::Measure { |measurements|::Measure::pass(input) }
}"#,
        ),
        ("application/build.omg", root_build.as_str()),
        (
            "application/main.omg",
            r#"use {numbers}/main.omg as numbers;
machine main() crashes Trap {
    let measured: u64 = |numbers|::Reading::value(73u64);
    transition measured == 73u64 { true -> done(), false -> bad() }
    machine done() {} machine bad() { crash Trap; }
}"#,
        ),
    ];
    project::write(root, &sources);
}

#[cfg(target_os = "linux")]
#[test]
fn independent_wait_uses_the_selected_export_and_the_callers_private_cell() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/synchronization/run/independent_wait");
    let read = |name: &str| fs::read_to_string(fixture.join(name)).unwrap();
    let supplier_build = read("supplier/build.omg");
    let supplier = read("supplier/main.omg");
    let build = read("application/build.omg");
    let main = read("application/main.omg");
    for (provider, value, stuck) in [
        ("ObservedWait", "1", true),
        ("ObservedWait", "0", false),
        ("ImmediateWait", "1", false),
    ] {
        let scratch = project::stage(
            "independent-atomic-wait",
            &[
                ("supplier/build.omg", &supplier_build),
                ("supplier/main.omg", &supplier),
                (
                    "application/build.omg",
                    &build.replace("ObservedWait", provider),
                ),
                (
                    "application/main.omg",
                    &main.replace("store(0", &format!("store({value}")),
                ),
            ],
        );
        let application = scratch.path().join("application");
        let (built, diagnostic) = project::status(&project::omega(
            &application,
            &[
                "--target",
                "linux_x86_64",
                "--build-dir",
                "out",
                "build.omg",
            ],
        ));
        assert_eq!(built, !stuck, "{provider}, {value}: {diagnostic}");
        let (ran, psi_diagnostic) = project::status(&project::omega(
            &application,
            &[
                "run",
                "--target",
                "linux_x86_64",
                "--psi-work-budget",
                "10000",
                "build.omg",
            ],
        ));
        assert_eq!(ran, !stuck, "{provider}, {value}: {psi_diagnostic}");
        if stuck {
            for report in [&diagnostic, &psi_diagnostic] {
                assert!(
                    report.contains("synchronization deadlock")
                        && report.contains("unchanged private atomic"),
                    "{report}"
                );
            }
        } else {
            let suppliers =
                fs::read_to_string(application.join("out/omega-components.tsv")).unwrap();
            assert!(
                suppliers.contains(".so"),
                "expected an independent native supplier: {suppliers}"
            );
            project::assert_native_returns(
                &application.join("out/omega-program"),
                &format!("{provider}, {value}"),
            );
        }
    }
}

#[test]
fn selected_component_wait_needs_progress_for_the_receivers_actual_input() {
    use psi_provider_selections::{
        ProgressScope, invocation_progress_failure,
    };
    use terminal_semantics::synchronization::ProgressFailure;

    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/synchronization/run/independent_wait");
    let read = |file: &str| fs::read_to_string(fixture.join(file)).unwrap();
    let supplier_build = read("supplier/build.omg");
    let mut supplier = read("supplier/main.omg");
    supplier.push_str(
        "
public TwoWaits satisfies Wait {
    machine until_clear(signal: @Signal) { wait_for_clear(signal); wait_for_clear(signal); }
}",
    );
    let build = read("application/build.omg");
    let application = read("application/main.omg");
    for (requires, lend_before_wait, use_descriptor) in [
        ("", false, false),
        ("requires input != 1", false, false),
        ("requires input != 1", true, false),
        ("", false, true),
        ("requires input != 1", false, true),
        ("requires input != 1", true, true),
    ] {
        let mut work = application
            .replace("machine main()", "public machine work(input: u32)")
            .replace("crashes none {", &format!("crashes none {requires} {{"))
            .replace("store(0, Publish)", "store(input, Publish)");
        work.push_str("machine main() reaches |signals|::Wait crashes none { work(2); }");
        if lend_before_wait {
            work = work
                .replace("reaches |signals|::Wait", "reaches |signals|::Wait + Gate")
                .replace(
                    "|signals|::Wait::until_clear(@signal);",
                    "Gate::receive(@signal); |signals|::Wait::until_clear(@signal);",
                );
            work.push_str("public boundary trait Gate { machine receive(signal: @|signals|::Signal) reaches none crashes none; }");
        }
        if use_descriptor {
            work = work.replace(
                "|signals|::Wait::until_clear(@signal);",
                "let run: Action = dispatch; run(@signal);",
            );
            work.push_str("
data Action = machine(signal: @|signals|::Signal) reaches |signals|::Wait invokes |signals|::Wait crashes none;
machine dispatch(signal: @|signals|::Signal) reaches |signals|::Wait invokes |signals|::Wait crashes none {
    |signals|::Wait::until_clear(signal);
}");
        }
        let scratch = project::stage(
            "selected-component-wait-progress",
            &[
                ("supplier/build.omg", &supplier_build),
                ("supplier/main.omg", &supplier),
                ("application/build.omg", &build),
                ("application/main.omg", &work),
            ],
        );
        let receiver = components::inspect(&scratch.path().join("application"), "work");
        let supplier = components::inspect(&scratch.path().join("supplier"), "wait_for_clear");
        let module = supplier.semantic_module();
        let root = receiver.semantic_module();
        let Some(terminal_psi::TargetMachineCallable::Machine(entry)) = root.entry else {
            panic!("expected the selected work entry");
        };
        assert_eq!(module.interfaces.provider_candidates.len(), 3);
        for candidate in &module.interfaces.provider_candidates {
            let provider = module.identities.text(candidate.provider_identity).unwrap();
            let admitted = components::admit(&receiver, &supplier, candidate);
            if provider.ends_with("::ObservedWait") {
                assert_eq!(
                    invocation_progress_failure(
                        supplier.view(),
                        candidate.candidate,
                        ProgressScope::OpenInterface,
                        [],
                        |_| None
                    ),
                    None
                );
                let standalone = invocation_progress_failure(
                    supplier.view(),
                    candidate.candidate,
                    ProgressScope::Executable,
                    [],
                    |_| None,
                )
                .expect("a standalone provider has no incoming progress premise");
                assert_eq!(standalone.cause, ProgressFailure::UnprovedExternal);
            }
            let failure = invocation_progress_failure(
                receiver.view(),
                entry,
                ProgressScope::Executable,
                [&admitted],
                |_| None,
            );
            if provider.ends_with("::TwoWaits") {
                let failure =
                    failure.expect("an unprojected or unvisited input obligation stays open");
                assert_eq!(failure.cause, ProgressFailure::UnprovedImport);
                if !use_descriptor {
                    assert_eq!(failure.machine, entry);
                }
            } else if provider.ends_with("::ObservedWait")
                && (requires.is_empty() || lend_before_wait)
            {
                let failure =
                    failure.expect("the installed provider may never see an enabled value");
                assert_eq!(failure.cause, ProgressFailure::Unproved);
                if !use_descriptor {
                    assert_eq!(failure.machine, entry);
                }
                let call = root
                    .machine(failure.machine)
                    .unwrap()
                    .operations()
                    .find(|op| op.id == failure.operation)
                    .unwrap();
                if use_descriptor {
                    assert!(call.kind.callable_invocation().is_some(), "{call:?}");
                } else {
                    assert_eq!(
                        call.declared_invocation().unwrap().callee,
                        terminal_psi::TargetMachineCallable::Boundary(admitted.receiving_boundary())
                    );
                }
            } else {
                assert_eq!(failure, None, "{provider} with {requires}");
            }
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn a_fused_wait_uses_the_body_selected_by_the_actual_target() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/synchronization/run/independent_wait");
    let read = |file: &str| fs::read_to_string(fixture.join(file)).unwrap();
    let supplier_build = read("supplier/build.omg");
    let supplier = read("supplier/main.omg");
    let build = read("application/build.omg")
        .replace("CompositionMode::Independent", "CompositionMode::Fused");
    let main = read("application/main.omg").replace("store(0", "store(1");
    for (provider, blocked) in [("ObservedWait", true), ("ImmediateWait", false)] {
        let scratch = project::stage(
            "fused-selected-wait",
            &[
                ("supplier/build.omg", &supplier_build),
                ("supplier/main.omg", &supplier),
                (
                    "application/build.omg",
                    &build.replace("ObservedWait", provider),
                ),
                ("application/main.omg", &main),
            ],
        );
        let project_dir = scratch.path().join("application");
        for command in [
            &[
                "--target",
                "linux_x86_64",
                "--build-dir",
                "out",
                "build.omg",
            ][..],
            &[
                "run",
                "--target",
                "linux_x86_64",
                "--psi-work-budget",
                "10000",
                "build.omg",
            ][..],
        ] {
            let (ok, diagnostic) = project::status(&project::omega(&project_dir, command));
            assert_eq!(ok, !blocked, "{provider} {command:?}: {diagnostic}");
            if blocked {
                assert!(
                    diagnostic.contains("synchronization deadlock"),
                    "{diagnostic}"
                );
            }
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn a_separately_compiled_publisher_lends_the_same_atomic_cell_to_both_arguments() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/synchronization/run/independent_atomic_update");
    let scratch = TemporaryDirectory::create("independent-atomic-publisher", false).unwrap();
    for package in ["application", "supplier"] {
        let target = scratch.path().join(package);
        fs::create_dir(&target).unwrap();
        for file in ["main.omg", "build.omg"] {
            fs::copy(source.join(package).join(file), target.join(file)).unwrap();
        }
    }
    let application = scratch.path().join("application");
    require_native_suppliers(&application, &scratch.path().join("out"), 1);
    let (ran, diagnostic) = project::status(&project::omega(
        &application,
        &["run", "--target", "linux_x86_64", "build.omg"],
    ));
    assert!(ran, "{diagnostic}");
}

#[cfg(target_os = "linux")]
#[test]
fn two_independent_providers_forward_the_callers_atomic_cell() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/synchronization/run/independent_atomic_update");
    let scratch = TemporaryDirectory::create("forwarded-atomic-publisher", false).unwrap();
    for directory in ["supplier", "relay", "forwarding"] {
        let target = scratch.path().join(directory);
        fs::create_dir(&target).unwrap();
        for file in ["build.omg", "main.omg"] {
            fs::copy(fixture.join(directory).join(file), target.join(file)).unwrap();
        }
    }
    let root = scratch.path().join("forwarding");
    fs::copy(
        fixture.join("forwarding/configure.omg"),
        root.join("configure.omg"),
    )
    .unwrap();
    let update = project::omega(
        &root,
        &[
            "packages",
            "update",
            "--offline",
            "--target",
            "linux_x86_64",
            "--dangerously-accept-all",
            "--project",
            ".",
        ],
    );
    let (reviewed, report) = project::status(&update);
    assert!(reviewed, "{report}");
    require_native_suppliers(&root, &scratch.path().join("out"), 2);

    let run = [
        "run",
        "--target",
        "linux_x86_64",
        "--psi-work-budget",
        "10000",
        "build.omg",
    ];
    let (ran, report) = project::status(&project::omega(&root, &run));
    assert!(ran, "{report}");

    let main = root.join("main.omg");
    let source = fs::read_to_string(&main).unwrap();
    fs::write(main, source.replace("store(3,", "store(1,")).unwrap();
    for arguments in [
        &[
            "--target",
            "linux_x86_64",
            "--build-dir",
            "held",
            "build.omg",
        ][..],
        &run,
    ] {
        let (accepted, report) = project::status(&project::omega(&root, arguments));
        assert!(
            !accepted
                && report.contains("synchronization deadlock")
                && report.contains("unchanged private atomic"),
            "{report}"
        );
    }
}
