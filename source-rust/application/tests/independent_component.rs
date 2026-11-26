//! A root Build selects an entryless dependency as a separate native supplier.
#![cfg(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "aarch64"),
))]

use platform_custody::temporary_directory::TemporaryDirectory;
use std::{fs, path::Path, process::Command};

fn entryless_supplier_sources(root: &Path) -> std::path::PathBuf {
    let package = root.join("supplier");
    let consumer = root.join("application");
    fs::create_dir_all(&package).unwrap();
    fs::create_dir_all(&consumer).unwrap();
    fs::write(
        package.join("build.omg"),
        r#"machine build(builder: @mutable Build) { Build::package(builder, "numbers"); }"#,
    )
    .unwrap();
    fs::write(
        package.join("main.omg"),
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
    )
    .unwrap();
    fs::write(consumer.join("build.omg"), r#"
use {omega}/build/dependency.omg;
use main.omg as app;
use {numbers}/main.omg as numbers;
machine build(builder: @mutable Build) {
    Build::application(builder, "number_application");
    let dependency: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Path { location: "../supplier" }));
    Build::select_provider::<|numbers|::Reading, |numbers|::ConstantReading>(builder, independent_mode());
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
    Build::bind(builder, windows_arm64::ProgramEntry, |app|::main);
}
machine independent_mode() -> CompositionMode { CompositionMode::Independent }
"#).unwrap();
    fs::write(
        consumer.join("main.omg"),
        r#"
use {numbers}/main.omg as numbers;
machine main() reaches |numbers|::Reading crashes Trap {
    let number: u64 = |numbers|::Reading::value(73u64);
    transition number == 73u64 { true -> done(), false -> bad() }
    machine done() {}
    machine bad() { crash Trap; }
}
"#,
    )
    .unwrap();
    consumer
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
            .args(["run", "--engine", "psi", "--target", "windows_arm64"])
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
        assert!(!sidecar.try_exists().unwrap(), "unexpected {}", sidecar.display());
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
        .args(["run", "--engine", "psi", "--target", "linux_x86_64"])
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
    for package in ["leaf", "supplier", "application"] {
        fs::create_dir(root.join(package)).unwrap();
    }
    let root_build = format!(
        r#"use {{omega}}/build/dependency.omg;
use main.omg as app;
use {{numbers}}/main.omg as numbers;
machine build(builder: @mutable Build) {{
    Build::application(builder, "reader");
    let number: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Path {{ location: "../supplier" }}));
    Build::select_provider::<|numbers|::Reading, |numbers|::Sensor>(builder, linux_x86_64, CompositionMode::{mode});
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
}}"#
    );
    let sources = [
        (
            "leaf/build.omg",
            r#"machine build(builder: @mutable Build) {
    Build::package(builder, "measurements");
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
machine build(builder: @mutable Build) {
    Build::package(builder, "numbers");
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
    for (path, source) in sources {
        fs::write(root.join(path), source).unwrap();
    }
}
