//! The preparing Build chooses which package implementation the next Build calls.
mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::fs;
use std::process::{Command, Output, Stdio};

const SERVICE: &str = r#"public boundary trait Meter {
    machine read() -> u32 reaches none crashes none;
}"#;

const BACKINGS: &str = r#"use main.omg as api;
public Seventeen satisfies |api|::Meter {
    machine read() -> u32 reaches none crashes none { 17u32 }
}
public TwentyThree satisfies |api|::Meter {
    machine read() -> u32 reaches none crashes none { 23u32 }
}"#;

const PRODUCT_ENTRIES: &str = r#"public machine return_normally() {}
public machine trap() crashes Trap { crash Trap; }"#;

#[derive(Clone, Copy)]
enum ProviderSource {
    Dependency,
    Local,
}

enum ProductEntries {
    Separate,
    SharedWithBuild,
}

fn project(
    target: &str,
    provider: Option<&str>,
    source: ProviderSource,
    before_read: &str,
    entries: ProductEntries,
) -> TemporaryDirectory {
    let sandbox = TemporaryDirectory::create("staged-build-providers", false).unwrap();
    let app = sandbox.path().join("app");
    fs::create_dir(&app).unwrap();
    let (meter, import_root, registration) = match source {
        ProviderSource::Local => (app.clone(), "", ""),
        ProviderSource::Dependency => {
            let meter = sandbox.path().join("meter");
            fs::create_dir(&meter).unwrap();
            fs::write(
                meter.join("build.omg"),
                r#"machine build(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: "meter" }; }"#,
            )
            .unwrap();
            (
                meter,
                "{meter}/",
                r#"let source: DependencyDescription = |dependency|::from_source(Source::Path { location: "../meter" });
    _ = Build::dependency(builder, source);"#,
            )
        }
    };
    fs::write(meter.join("main.omg"), SERVICE).unwrap();
    fs::write(meter.join("backings.omg"), BACKINGS).unwrap();

    let (provider_files, selection) = match provider {
        Some(name) => (
            format!("use {import_root}main.omg as api; use {import_root}backings.omg;"),
            format!(
                "Build::select_provider::<|api|::Meter, |backings|::{name}>(builder, interpreter);"
            ),
        ),
        None => (String::new(), String::new()),
    };
    fs::write(
        app.join("build.omg"),
        format!(
            r#"use {{omega}}/build/dependency.omg;
{provider_files}
machine build(builder: @mutable BuildPlan) {{
    {registration}
    builder.product = BuildProduct::InterpretedBuild {{ entry: "configure.omg" }};
    {selection}
}}"#
        ),
    )
    .unwrap();
    let (product_import, qualifier, shared_entries) = match entries {
        ProductEntries::Separate => {
            fs::write(app.join("product.omg"), PRODUCT_ENTRIES).unwrap();
            ("use product.omg as entries;", "|entries|::", "")
        }
        ProductEntries::SharedWithBuild => ("", "", PRODUCT_ENTRIES),
    };
    fs::write(
        app.join("configure.omg"),
        format!(
            r#"use {import_root}main.omg as api;
{product_import}
machine build(builder: @mutable BuildPlan) {{
    {before_read}
    let observed: u32 = |api|::Meter::read();
    builder.product = BuildProduct::Application {{ name: "staged_provider" }};
    transition observed {{
        17u32 -> select_return(builder),
        23u32 -> select_trap(builder),
        _ -> reject(builder)
    }}
    machine select_return(builder: @mutable BuildPlan) {{ Build::bind(builder, {target}::ProgramEntry, {qualifier}return_normally); }}
    machine select_trap(builder: @mutable BuildPlan) {{ Build::bind(builder, {target}::ProgramEntry, {qualifier}trap); }}
    machine reject(builder: @mutable BuildPlan) {{ builder.product = BuildProduct::Unselected; }}
}}
{shared_entries}"#
        ),
    )
    .unwrap();
    sandbox
}

fn run(project: &TemporaryDirectory, engine: &str) -> (Output, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .current_dir(project.path().join("app"))
        .args(["run", "--engine", engine, "--outputs"])
        .arg(project.path().join(engine))
        .args([
            "--build-input",
            "build.omg",
            "--build-input",
            "configure.omg",
            "--optional-build-input",
            "product.omg",
            "--optional-build-input",
            "main.omg",
            "--optional-build-input",
            "backings.omg",
            "build.omg",
        ])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output, report)
}

#[test]
fn changing_only_the_interpreter_selection_changes_the_final_product() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        return;
    };
    for source in [ProviderSource::Dependency, ProviderSource::Local] {
        for (provider, psi_termination, native_trap) in [
            ("Seventeen", "psi\treturn", false),
            ("TwentyThree", "psi\tcrash\tTrap", true),
        ] {
            let fixture = project(
                host.target_name(),
                Some(provider),
                source,
                "",
                ProductEntries::Separate,
            );
            for engine in ["native", "psi"] {
                let (output, report) = run(&fixture, engine);
                assert!(output.status.success(), "{provider} {engine}: {report}");
                let termination =
                    fs::read_to_string(fixture.path().join(engine).join("termination.tsv"))
                        .unwrap();
                if engine == "psi" {
                    assert_eq!(termination.trim_end(), psi_termination, "{provider}");
                } else {
                    let columns: Vec<_> = termination.trim_end().split('\t').collect();
                    let expected = if native_trap {
                        matches!(columns.as_slice(), ["native", _, _, "trap", _])
                    } else {
                        matches!(columns.as_slice(), ["native", "exit", "0", _, _])
                    };
                    assert!(expected, "{provider}: {termination}");
                }
            }
        }
    }
}

#[test]
fn a_successor_with_an_unmapped_service_cannot_begin_execution() {
    let poison = "let fabricated: PackageBinding = PackageBinding {}; Build::validate(builder, @fabricated);";
    let fixture = project(
        "linux_x86_64",
        None,
        ProviderSource::Dependency,
        poison,
        ProductEntries::Separate,
    );
    let (output, report) = run(&fixture, "psi");
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("before entry; no interpreter provider or host binding was supplied"),
        "{report}"
    );
    assert!(
        !report.contains("requires a PackageBinding this activation issued"),
        "{report}"
    );

    let admitted = project(
        "linux_x86_64",
        Some("Seventeen"),
        ProviderSource::Dependency,
        poison,
        ProductEntries::Separate,
    );
    let (output, report) = run(&admitted, "psi");
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("requires a PackageBinding this activation issued"),
        "{report}"
    );
}

#[test]
fn a_shared_product_entry_cannot_use_the_builds_meter_dependency() {
    let fixture = project(
        "linux_x86_64",
        Some("Seventeen"),
        ProviderSource::Dependency,
        "",
        ProductEntries::SharedWithBuild,
    );
    let (output, report) = run(&fixture, "psi");
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("dependency `meter` is unavailable in the product scope"),
        "{report}"
    );
    assert!(report.contains("configure.omg:1:5"), "{report}");
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn sandbox_standard_output_and_system_editor_guide() {
    use package_manager::operations::begin_local_project_preparation;
    use target::TargetProfile;

    let target = TargetProfile::LinuxX64;
    let root = support::repository_root();
    package_compilation::toolchain_library::configure(&root.join("source/library")).unwrap();
    let work = TemporaryDirectory::create("stock-successor-console", false).unwrap();
    let project = work.path().join("editor");
    let vendor = project.join("vendor/system");
    fs::create_dir_all(&vendor).unwrap();
    let system = begin_local_project_preparation(
        &root.join("source/library/system/build.omg"),
        &[target],
        true,
    )
    .unwrap()
    .acquire_root()
    .unwrap();
    let captured = system.source_capture().tree();
    for (path, _) in captured
        .entries()
        .filter(|(p, _)| p.extension().is_some_and(|e| e == "omg"))
    {
        let destination = vendor.join(path);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::write(
            destination,
            captured.read_to_string(path).unwrap().as_bytes(),
        )
        .unwrap();
    }
    let guide = fs::read_to_string(root.join("wiki/language_guide/packages.md")).unwrap();
    let (_, guide) = guide
        .split_once("## Using the same console in a Build and an application")
        .expect("the packages guide contains the staged editor");
    for filename in ["build.omg", "build_0.omg", "generated.omg", "main.omg"] {
        let opening = format!("```omega\n// {filename}");
        let (_, example) = guide
            .split_once(&opening)
            .expect("the guide names its source file");
        let (_, source) = example.split_once('\n').unwrap();
        let (source, _) = source.split_once("\n```").unwrap();
        fs::write(project.join(filename), source).unwrap();
    }

    let outputs = work.path().join("outputs");
    fs::create_dir(&outputs).unwrap();
    let product = package_manager::project::open_product(
        &project.join("build.omg"),
        target,
        true,
        &outputs,
        None,
    )
    .expect("run the staged guide under the package driver");
    assert_eq!(product.evaluated().preparations.len(), 1);
    let summary = product
        .evaluated()
        .configuration
        .observation_summary
        .as_ref()
        .unwrap();
    assert_eq!(summary.build_log(), b"configuring editor\n");

    let plan = product.product_stage(&[target]).unwrap();
    let program = plan.program(target, Vec::new()).unwrap();
    let compiler::Artifact::Native(output) = compiler::compile(
        compiler::CompileRequest::Native {
            program,
            entry_memory: &[],
        },
        Default::default(),
    )
    .expect("compile the editor using the stock native provider")
    .artifact
    else {
        panic!("expected a native application");
    };
    let file = work.path().join("editor.elf");
    let artifact = output.retained_native_artifact().unwrap();
    fs::write(&file, &artifact.image().output().bytes).unwrap();
    platform_custody::file_publication::make_executable(&file).unwrap();
    let execution = Command::new(file).output().unwrap();
    assert!(execution.status.success(), "{execution:?}");
    assert_eq!(execution.stdout, b"editor started\n");
}

#[test]
fn a_returned_dependency_reads_only_its_own_captured_package() {
    let sandbox = TemporaryDirectory::create("board-filesystem", false).unwrap();
    let app = sandbox.path().join("app");
    let board = sandbox.path().join("board");
    fs::create_dir(&app).unwrap();
    fs::create_dir(&board).unwrap();
    fs::write(app.join("build.omg"), r#"use {omega}/build/dependency.omg;
machine build(plan: @mutable BuildPlan) {
    let board: DependencyDescription = |dependency|::from_source(Source::Path { location: "../board" });
    _ = |dependency|::add(plan, board);
    plan.product = BuildProduct::Package { name: "firmware" };
}"#).unwrap();
    fs::write(board.join("configure.omg"), r#"use {system}/filesystem.omg as files;
use {system}/filesystem_host.omg as paths;
machine build(plan: @mutable BuildPlan) blocks {
    let mutable bytes: [u8; 1] = [0u8];
    let loaded: |files|::IoResult = block |files|::Filesystem::read_all("board.cfg" as + |paths|::Path, @mutable bytes[..], 1u64);
    transition loaded {
        |files|::IoResult::Ok { count } -> choose(plan, bytes[0], count),
        |files|::IoResult::Error { kind } -> refused(plan, kind)
    }
    machine choose(plan: @mutable BuildPlan, byte: u8, count: u64) {
        transition count == 1u64 && byte == 82u8 {
            true -> rover(plan),
            false -> wrong(plan)
        }
    }
    machine rover(plan: @mutable BuildPlan) {
        plan.product = BuildProduct::Package { name: "board" };
    }
    machine refused(plan: @mutable BuildPlan, error: |files|::ErrorKind) {
        transition error {
            |files|::ErrorKind::NotFound -> missing(plan),
            _ -> wrong(plan)
        }
    }
    machine missing(plan: @mutable BuildPlan) {
        plan.product = BuildProduct::InterpretedBuild { entry: "board-input-was-absent.omg" };
    }
    machine wrong(plan: @mutable BuildPlan) {
        plan.product = BuildProduct::InterpretedBuild { entry: "unexpected-board-read.omg" };
    }
}"#).unwrap();
    let library = std::env::var_os("OMEGA_LIBRARY_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| support::repository_root().join("source/library"));
    let system = library.join("system");
    let check = || {
        let output = Command::new(env!("CARGO_BIN_EXE_omega"))
            .env("OMEGA_LIBRARY_ROOT", &library)
            .args([
                "--check",
                "--build-input",
                "build.omg",
                "--build-input",
                "board.cfg",
                "build.omg",
            ])
            .current_dir(&app)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let report = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        (output.status, report)
    };
    fs::write(app.join("board.cfg"), b"P").unwrap();
    fs::write(board.join("board.cfg"), b"R").unwrap();
    prepare_stock_filesystem(&board, &system);
    let (status, report) = check();
    assert!(status.success(), "{report}");

    fs::remove_file(board.join("board.cfg")).unwrap();
    let (status, report) = check();
    assert!(!status.success(), "{report}");
    assert!(report.contains("board-input-was-absent.omg"), "{report}");

    let copy = sandbox.path().join("another-system");
    fs::create_dir(&copy).unwrap();
    for name in ["filesystem.omg", "filesystem_host.omg"] {
        fs::copy(system.join(name), copy.join(name)).unwrap();
    }
    fs::write(
        copy.join("build.omg"),
        r#"machine build(plan: @mutable BuildPlan) {
        plan.product = BuildProduct::Package { name: "system" };
    }"#,
    )
    .unwrap();
    prepare_stock_filesystem(&board, &copy);
    let (status, report) = check();
    assert!(!status.success(), "{report}");
    assert!(
        report.contains("no interpreter provider or host binding was supplied"),
        "{report}"
    );
}

fn prepare_stock_filesystem(board: &std::path::Path, library: &std::path::Path) {
    let source = r#"use {omega}/build/dependency.omg;
use {omega}/language/core/filesystem_authority.omg as authority;
machine build(plan: @mutable BuildPlan) {
    let allowed: [BoundaryServiceReference; 4] = [
        Build::service::<|authority|::FilesystemContentRead>(plan),
        Build::service::<|authority|::FilesystemContentWrite>(plan),
        Build::service::<|authority|::FilesystemMetadataMutation>(plan),
        Build::service::<|authority|::FilesystemNamespaceMutation>(plan)
    ];
    let system: DependencyDescription = |dependency|::from_source(Source::Path { location: "SYSTEM_PATH" });
    _ = |dependency|::add(plan, system, @allowed[..]);
    plan.product = BuildProduct::InterpretedBuild { entry: "configure.omg" };
}"#.replace("SYSTEM_PATH", &library.to_string_lossy().replace('\\', "/"));
    fs::write(board.join("build.omg"), source).unwrap();
}

#[test]
fn a_generator_writes_an_ordinary_file_in_its_own_private_staging() {
    let sandbox = TemporaryDirectory::create("generator-filesystem", false).unwrap();
    let root = sandbox.path();
    let library = std::env::var_os("OMEGA_LIBRARY_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| support::repository_root().join("source/library"));
    prepare_stock_filesystem(root, &library.join("system"));
    fs::write(root.join("configure.omg"), r#"use {system}/filesystem.omg as files;
use {system}/filesystem_host.omg as paths;
machine build(plan: @mutable BuildPlan) blocks {
    let written: |files|::UnitResult = block |files|::Filesystem::write_all(
        "generated.omg" as + |paths|::Path, "public machine answer() -> u32 { 17u32 }");
    let protected: |files|::UnitResult = block |files|::Filesystem::write_all(
        "configure.omg" as + |paths|::Path, "replaced");
    let mutable first: [u8; 1] = [0u8];
    let loaded: |files|::IoResult = block |files|::Filesystem::read_all(
        "generated.omg" as + |paths|::Path, @mutable first[..], 1u64);
    transition written {
        |files|::UnitResult::Ok -> check_protection(plan, protected, loaded, first[0]),
        |files|::UnitResult::Error { kind } -> failed(plan)
    }
    machine check_protection(plan: @mutable BuildPlan, protected: |files|::UnitResult, loaded: |files|::IoResult, first: u8) {
        transition protected {
            |files|::UnitResult::Error { kind } -> check_error(plan, kind, loaded, first),
            |files|::UnitResult::Ok -> failed(plan)
        }
    }
    machine check_error(plan: @mutable BuildPlan, kind: |files|::ErrorKind, loaded: |files|::IoResult, first: u8) {
        transition kind {
            |files|::ErrorKind::PermissionDenied -> check_read(plan, loaded, first),
            _ -> failed(plan)
        }
    }
    machine check_read(plan: @mutable BuildPlan, loaded: |files|::IoResult, first: u8) {
        transition loaded {
            |files|::IoResult::Ok { count } -> check_byte(plan, count, first),
            |files|::IoResult::Error { kind } -> failed(plan)
        }
    }
    machine check_byte(plan: @mutable BuildPlan, count: u64, first: u8) {
        transition count == 1u64 && first == 112u8 {
            true -> publish(plan),
            false -> failed(plan)
        }
    }
    machine publish(plan: @mutable BuildPlan) {
        let need: RequiredOutput = BuildOutput::require(@mutable plan.output, "generated.omg");
        let generated: BuildPath = BuildOutput::resolve(@plan.output, "generated.omg");
        let completed: OutputCompletion = BuildOutput::complete(@mutable plan.output, need, generated);
        transition completed {
            OutputCompletion::Sealed { receipt } -> done(plan),
            OutputCompletion::Retry { obligation, file } -> refuse(plan, obligation)
        }
    }
    machine done(plan: @mutable BuildPlan) { plan.product = BuildProduct::Package { name: "generator" }; }
    machine refuse(plan: @mutable BuildPlan, need: RequiredOutput) {
        BuildOutput::fail(@mutable plan.output, need, "generated file was not sealed");
    }
    machine failed(plan: @mutable BuildPlan) {
        plan.product = BuildProduct::InterpretedBuild { entry: "generator-write-failed.omg" };
    }
}"#).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .env("OMEGA_LIBRARY_ROOT", library)
        .current_dir(root)
        .args(["build", "build.omg"])
        .output()
        .unwrap();
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.status.success(), "{report}");
    let published = report
        .lines()
        .find_map(|line| line.strip_prefix("published completed build outputs to "))
        .unwrap_or_else(|| panic!("missing generator output: {report}"));
    assert_eq!(
        fs::read(std::path::Path::new(published).join("files/generated.omg")).unwrap(),
        b"public machine answer() -> u32 { 17u32 }"
    );
    assert!(!root.join("generated.omg").exists());
}

#[test]
fn a_returned_directory_supplies_the_successor_entry_import_and_runtime_file() {
    let sandbox = TemporaryDirectory::create("generated-successor-package", false).unwrap();
    let root = sandbox.path();
    let library = std::env::var_os("OMEGA_LIBRARY_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| support::repository_root().join("source/library"));
    prepare_stock_filesystem(root, &library.join("system"));

    let entry = r#"use policy.omg;
machine build(plan: @mutable BuildPlan) {
    let asset: BuildPath = BuildSource::resolve(@plan.source, "settings.dat");
    let file: i32 = BuildSource::open(@plan.source, asset, 0);
    let mutable bytes: [u8; 1] = [0u8];
    let count: i64 = BuildSource::read(@plan.source, file, @mutable bytes[..], 1u64);
    _ = BuildSource::close(@plan.source, file);
    transition count == 1i64 && |policy|::valid(bytes[0]) {
        true -> { plan.product = BuildProduct::Package { name: "generated_board" }; },
        false -> { plan.product = BuildProduct::InterpretedBuild { entry: "wrong-settings.omg" }; }
    }
}"#;
    let generator = r#"use {system}/filesystem.omg as files;
use {system}/filesystem_host.omg as paths;
machine build(plan: @mutable BuildPlan) blocks {
    _ = block |files|::Filesystem::create_dir("generated" as + |paths|::Path);
    _ = block |files|::Filesystem::write_all("sibling.omg" as + |paths|::Path,
        "public machine valid(value: u8) -> bool { true }");
    _ = block |files|::Filesystem::write_all("generated/next.omg" as + |paths|::Path,
        "machine build(plan: @mutable BuildPlan) { plan.product = BuildProduct::InterpretedBuild { entry: \"final.omg\" }; }");
    _ = block |files|::Filesystem::write_all("generated/final.omg" as + |paths|::Path, "ENTRY");
    _ = block |files|::Filesystem::write_all("generated/policy.omg" as + |paths|::Path,
        "public machine valid(value: u8) -> bool { value == 82u8 }");
    _ = block |files|::Filesystem::write_all("generated/settings.dat" as + |paths|::Path, "R");
    SELECT
    plan.product = BuildProduct::InterpretedBuild { entry: "next.omg" };
}"#
    .replace("ENTRY", &entry.escape_default().to_string());
    fs::write(
        root.join("parent_only.omg"),
        "public machine valid(value: u8) -> bool { false }",
    )
    .unwrap();
    let run = |generator: &str, select: &str| {
        fs::write(
            root.join("configure.omg"),
            generator.replace("SELECT", select),
        )
        .unwrap();
        let result = Command::new(env!("CARGO_BIN_EXE_omega"))
            .env("OMEGA_LIBRARY_ROOT", &library)
            .current_dir(root)
            .args(["build", "build.omg"])
            .output()
            .unwrap();
        (
            result.status.success(),
            format!(
                "{}{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            ),
        )
    };
    let select = r#"let generated: BuildPath = BuildOutput::resolve(@plan.output, "generated");
    plan.package_root = BuildPackageRoot::Directory { path: generated };"#;
    let (ok, report) = run(&generator, select);
    assert!(ok, "{report}");
    for name in ["generated", "sibling.omg"] {
        assert!(!root.join(name).exists(), "{name} escaped private staging");
    }

    let (ok, report) = run(&generator, "");
    assert!(!ok && report.contains("next.omg"), "{report}");
    let (ok, report) = run(
        &generator.replace("use policy.omg;", "use parent_only.omg as policy;"),
        select,
    );
    assert!(!ok && report.contains("parent_only.omg"), "{report}");
    let (ok, report) = run(
        &generator.replace("use policy.omg;", "use sibling.omg as policy;"),
        select,
    );
    assert!(!ok && report.contains("sibling.omg"), "{report}");
    let (ok, report) = run(
        &generator,
        "plan.package_root = BuildPackageRoot::Directory { path: BuildPath {} };",
    );
    assert!(!ok && report.contains("issued"), "{report}");
}

#[test]
fn successor_runtime_directories_are_narrowed_and_end_at_the_next_build() {
    let sandbox = TemporaryDirectory::create("successor-runtime-directories", false).unwrap();
    let root = sandbox.path();
    let sdk = std::env::var_os("OMEGA_LIBRARY_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| support::repository_root().join("source/library"));
    let package = root.join("package");
    fs::create_dir_all(package.join("sdk/headers")).unwrap();
    fs::write(package.join("sdk/headers/input"), b"R").unwrap();
    fs::write(package.join("sdk/secret"), b"S").unwrap();
    prepare_stock_filesystem(&package, &sdk.join("system"));
    let preparing = fs::read_to_string(package.join("build.omg")).unwrap();
    let choose = r#"let headers: BuildPath = BuildSource::resolve(@plan.source, "sdk/headers");
    let out: BuildPath = BuildOutput::resolve(@plan.output, "");
    plan.directories = Build::map_readonly(plan.directories, headers, "/sdk");
    plan.directories = Build::map_readwrite(plan.directories, out, "/out");"#;
    fs::write(
        package.join("build.omg"),
        preparing.replace("    plan.product", &format!("{choose}\n    plan.product")),
    )
    .unwrap();

    let imports =
        "use {system}/filesystem.omg as files;\nuse {system}/filesystem_host.omg as paths;\n";
    let helpers = r#"
machine failed() { crash Trap; }
machine passed() {}
machine expect(value: bool) { transition value { true -> passed(), false -> failed() } }
machine expect_byte(result: |files|::IoResult, actual: u8, expected: u8) {
    transition result { |files|::IoResult::Ok { count } -> expect(count == 1u64 && actual == expected), _ -> failed() }
}
machine expect_denied(kind: |files|::ErrorKind) {
    transition kind { |files|::ErrorKind::PermissionDenied -> passed(), _ -> failed() }
}
machine expect_write_denied(result: |files|::UnitResult) {
    transition result { |files|::UnitResult::Error { kind } -> expect_denied(kind), _ -> failed() }
}
machine expect_write(result: |files|::UnitResult) {
    transition result { |files|::UnitResult::Ok -> passed(), _ -> failed() }
}
machine expect_missing(result: |files|::IoResult) {
    transition result { |files|::IoResult::Error { kind } -> expect_missing_kind(kind), _ -> failed() }
}
machine expect_missing_kind(kind: |files|::ErrorKind) {
    transition kind { |files|::ErrorKind::NotFound -> passed(), _ -> failed() }
}
"#;
    let checks = r#"
    let mutable byte: [u8; 1] = [0u8];
    let read: |files|::IoResult = block |files|::Filesystem::read_all("/sdk/input" as + |paths|::Path, @mutable byte[..], 1u64);
    expect_byte(read, byte[0], 82u8);
    let sibling: |files|::IoResult = block |files|::Filesystem::read_all("/sdk/secret" as + |paths|::Path, @mutable byte[..], 1u64);
    expect_missing(sibling);
    let refused: |files|::UnitResult = block |files|::Filesystem::write_all("/sdk/input" as + |paths|::Path, "X");
    expect_write_denied(refused);
    let written: |files|::UnitResult = block |files|::Filesystem::write_all("/out/test" as + |paths|::Path, "W");
    expect_write(written);
    let copied: |files|::IoResult = block |files|::Filesystem::read_all("/out/test" as + |paths|::Path, @mutable byte[..], 1u64);
    expect_byte(copied, byte[0], 87u8);
"#;
    let next = preparing
        .replace(
            "machine build(plan: @mutable BuildPlan) {",
            "machine build(plan: @mutable BuildPlan) blocks crashes Trap {",
        )
        .replace("    plan.product", &format!("{checks}\n    plan.product"))
        .replace("configure.omg", "last.omg");
    fs::write(
        package.join("configure.omg"),
        format!("{imports}{next}{helpers}"),
    )
    .unwrap();
    fs::write(package.join("last.omg"), format!("{imports}{}", r#"machine build(plan: @mutable BuildPlan) blocks crashes Trap {
    let mutable bytes: [u8; 1] = [0u8];
    let missing: |files|::IoResult = block |files|::Filesystem::read_all("/out/test" as + |paths|::Path, @mutable bytes[..], 1u64);
    transition missing { |files|::IoResult::Error { kind } -> check_kind(plan, kind), _ -> failed() }
}
machine check_kind(plan: @mutable BuildPlan, kind: |files|::ErrorKind) {
    transition kind { |files|::ErrorKind::PermissionDenied -> { plan.product = BuildProduct::Package { name: "checked_directories" }; }, _ -> failed() }
}
machine failed() { crash Trap; }"#)).unwrap();
    let check = || {
        Command::new(env!("CARGO_BIN_EXE_omega"))
            .args([
                "--check",
                "--build-input",
                "build.omg",
                "--build-input",
                "configure.omg",
                "--build-input",
                "last.omg",
                "--build-input",
                "sdk",
                "build.omg",
            ])
            .current_dir(&package)
            .output()
            .unwrap()
    };
    let result = check();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(fs::read(package.join("sdk/headers/input")).unwrap(), b"R");
    assert!(!package.join("test").exists());

    for (selection, expected) in [
        (
            choose.replace("map_readonly", "map_readwrite"),
            "cannot be delegated read-write",
        ),
        (
            choose.replace("\"/out\"", "\"/sdk\""),
            "duplicate guest directory",
        ),
        (
            choose.replace("sdk/headers", "sdk/headers/input"),
            "directory",
        ),
    ] {
        fs::write(
            package.join("build.omg"),
            preparing.replace(
                "    plan.product",
                &format!("{selection}\n    plan.product"),
            ),
        )
        .unwrap();
        let result = check();
        assert!(!result.status.success());
        let error = String::from_utf8_lossy(&result.stderr);
        assert!(error.contains(expected), "{error}");
    }
}

#[test]
fn explicitly_forwarding_a_runtime_subtree_keeps_its_shadowing_readonly_mount() {
    let sandbox = TemporaryDirectory::create("forward-runtime-directories", false).unwrap();
    let root = sandbox.path();
    let project = root.join("package");
    let parent = root.join("incoming");
    let private = root.join("private");
    fs::create_dir(&project).unwrap();
    fs::create_dir_all(parent.join("team/private")).unwrap();
    fs::create_dir(&private).unwrap();
    fs::write(parent.join("team/private/key"), b"H").unwrap();
    fs::write(private.join("key"), b"R").unwrap();

    let sdk = std::env::var_os("OMEGA_LIBRARY_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| support::repository_root().join("source/library"));
    prepare_stock_filesystem(&project, &sdk.join("system"));
    let preparing = fs::read_to_string(project.join("build.omg")).unwrap();
    let select = |source: &str| {
        fs::write(
        project.join("build.omg"),
        preparing.replace("    plan.product", &format!(r#"    plan.directories = Build::forward_readwrite(plan.directories, "{source}", "/team");
    plan.product"#)),
    ).unwrap()
    };
    fs::write(project.join("configure.omg"), r#"use {system}/filesystem.omg as files;
use {system}/filesystem_host.omg as paths;
machine build(plan: @mutable BuildPlan) blocks crashes Trap {
    let mutable byte: [u8; 1] = [0u8];
    let read: |files|::IoResult = block |files|::Filesystem::read_all("/team/private/key" as + |paths|::Path, @mutable byte[..], 1u64);
    transition read { |files|::IoResult::Ok { count } -> block check_value(plan, count, byte[0]), _ -> failed() }
}
machine check_value(plan: @mutable BuildPlan, count: u64, value: u8) blocks {
    transition count == 1u64 && value == 82u8 { true -> block try_shadow(plan), false -> failed() }
}
machine try_shadow(plan: @mutable BuildPlan) blocks {
    let result: |files|::UnitResult = block |files|::Filesystem::write_all("/team/private/key" as + |paths|::Path, "X");
    transition result { |files|::UnitResult::Error { kind } -> block check_denied(plan, kind), _ -> failed() }
}
machine check_denied(plan: @mutable BuildPlan, kind: |files|::ErrorKind) blocks {
    transition kind { |files|::ErrorKind::PermissionDenied -> block try_writable(plan), _ -> failed() }
}
machine try_writable(plan: @mutable BuildPlan) blocks {
    let result: |files|::UnitResult = block |files|::Filesystem::write_all("/team/result" as + |paths|::Path, "W");
    transition result { |files|::UnitResult::Ok -> { plan.product = BuildProduct::Package { name: "forwarded" }; }, _ -> failed() }
}
machine failed() { crash Trap; }
"#).unwrap();
    let check = || {
        Command::new(env!("CARGO_BIN_EXE_omega"))
            .args(["--check", "--build-directories-readwrite"])
            .arg(format!("{}::/incoming", parent.display()))
            .arg("--build-directories-readonly")
            .arg(format!("{}::/incoming/team/private", private.display()))
            .arg("build.omg")
            .current_dir(&project)
            .output()
            .unwrap()
    };
    select("/incoming/team");
    let result = check();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(fs::read(parent.join("team/result")).unwrap(), b"W");
    assert_eq!(fs::read(parent.join("team/private/key")).unwrap(), b"H");
    assert_eq!(fs::read(private.join("key")).unwrap(), b"R");
    assert!(!parent.join("result").exists());
    for path in ["/incoming/team/private", "/not-granted"] {
        select(path);
        let result = check();
        assert!(!result.status.success());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains("cannot forward directory"),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
