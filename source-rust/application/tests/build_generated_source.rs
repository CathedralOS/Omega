//! A Product imports the exact files its Build emitted. The producing Build retains its own inputs.
use platform_custody::temporary_directory::TemporaryDirectory;
use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn run(project: &Path, engine: &str, inputs: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_omega"));
    command.args(["run", "--engine", engine]);
    for path in inputs {
        command.args(["--build-input", path]);
    }
    command
        .arg("build.omg")
        .current_dir(project)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn report(result: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    )
}

#[test]
fn handwritten_product_calls_a_generated_file() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let project =
        TemporaryDirectory::create("handwritten-import-of-generated-file", false).unwrap();
    let main = r#"use generated.omg;
public machine launch() crashes Trap {
    transition |generated|::read() == 17u8 { true -> done(), false -> wrong() }
    machine done() {}
    machine wrong() { crash Trap; }
}"#;
    let entry = format!(
        r#"use main.omg as app;
machine build(plan: @mutable BuildPlan) {{
    plan.product = BuildProduct::Application {{ name: "frozen" }};
    Build::bind(plan, {}::ProgramEntry, |app|::launch);
}}"#,
        host.target_name()
    );
    fs::write(project.path().join("main.omg"), main).unwrap();
    fs::write(project.path().join("next.template"), &entry).unwrap();
    for (body, valid) in [
        ("public machine read() -> u8 { 17u8 }", true),
        (
            "public machine read() -> u8 { 17u8 } machine unused() { missing(); }",
            false,
        ),
    ] {
        fs::write(project.path().join("asset.txt"), body).unwrap();
        let source = format!(
            r#"machine build(plan: @mutable BuildPlan) {{
    copy::<{main_length}>(plan, "main.omg", "main.omg");
    copy::<{asset_length}>(plan, "asset.txt", "generated.omg");
    copy::<{entry_length}>(plan, "next.template", "next.omg");
    let directory: BuildPath = BuildOutput::resolve(@plan.output, "");
    plan.package_root = BuildPackageRoot::Directory {{ path: directory }};
    plan.product = BuildProduct::InterpretedBuild {{ entry: "next.omg" }};
}}

machine copy<const N: u64>(plan: @mutable BuildPlan, source: @[u8], destination: @[u8]) {{
    let path: BuildPath = BuildSource::resolve(@plan.source, source);
    let reader: i32 = BuildSource::open(@plan.source, path, 0);
    let mutable bytes: [u8; N];
    _ = BuildSource::read(@plan.source, reader, @mutable bytes[..], N);
    _ = BuildSource::close(@plan.source, reader);
    let file: BuildPath = BuildOutput::resolve(@plan.output, destination);
    let writer: i32 = BuildOutput::create(@mutable plan.output, file, 0);
    _ = BuildOutput::write(@mutable plan.output, writer, @bytes[..]);
    _ = BuildOutput::close(@mutable plan.output, writer);
}}"#,
            main_length = main.len(),
            asset_length = body.len(),
            entry_length = entry.len(),
        );
        fs::write(project.path().join("build.omg"), source).unwrap();
        for engine in ["psi", "native"] {
            let result = run(
                project.path(),
                engine,
                &["build.omg", "main.omg", "next.template", "asset.txt"],
            );
            let report = report(&result);
            assert_eq!(result.status.success(), valid, "{engine}: {report}");
            if !valid {
                assert!(
                    report.contains("no visible declaration"),
                    "{engine}: {report}"
                );
                assert!(report.contains("generated.omg"), "{engine}: {report}");
                assert!(report.contains("unused"), "{engine}: {report}");
            }
        }
    }
}

#[test]
fn a_build_cannot_import_its_own_future_product_file() {
    let project = TemporaryDirectory::create("build-import-of-future-product-file", false).unwrap();
    fs::write(
        project.path().join("build.omg"),
        r#"
        use generated.omg;
        machine build(plan: @mutable BuildPlan) {
            let file: BuildPath = BuildOutput::resolve(@plan.output, "generated.omg");
            let writer: i32 = BuildOutput::create(@mutable plan.output, file, 0);
            _ = BuildOutput::write(@mutable plan.output, writer, "public machine read() -> u8 { 17u8 }");
            _ = BuildOutput::close(@mutable plan.output, writer);
            let value: u8 = |generated|::read();
            plan.product = BuildProduct::Package { name: "future" };
        }
    "#,
    )
    .unwrap();
    let result = run(project.path(), "psi", &["build.omg"]);
    assert!(!result.status.success());
    assert!(report(&result).contains("no admitted logical file `generated.omg`"));
}
