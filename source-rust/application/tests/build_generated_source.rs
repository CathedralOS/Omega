//! A Product imports the exact files its Build emitted. The producing Build retains its own inputs.
#[path = "support/project.rs"]
mod project;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::fs;
use std::path::Path;
use std::process::Output;

fn run(project: &Path, engine: &str, inputs: &[&str]) -> Output {
    let mut arguments = vec!["run", "--engine", engine, "--outputs", engine];
    for path in inputs {
        arguments.extend(["--build-input", *path]);
    }
    arguments.push("build.omg");
    project::omega(project, &arguments)
}

fn report(result: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    )
}

fn assert_returned(root: &Path, engine: &str) {
    let output = root.join(engine);
    let termination = fs::read_to_string(output.join("termination.tsv")).unwrap();
    match engine {
        "psi" => assert_eq!(termination, "psi\treturn\n"),
        _ => assert!(
            termination.starts_with("native\texit\t0\t"),
            "{termination}"
        ),
    }
    assert!(fs::read(output.join("stdout")).unwrap().is_empty());
    assert!(fs::read(output.join("stderr")).unwrap().is_empty());
}

#[test]
fn handwritten_product_calls_a_generated_file() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        return;
    };
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
    let project = project::stage(
        "handwritten-import-of-generated-file",
        &[("main.omg", main), ("next.template", &entry)],
    );
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
            if valid {
                assert_returned(project.path(), engine);
            } else {
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
    let project = project::stage(
        "build-import-of-future-product-file",
        &[(
            "build.omg",
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
        )],
    );
    let result = run(project.path(), "psi", &["build.omg"]);
    assert!(!result.status.success());
    assert!(report(&result).contains("no admitted logical file `generated.omg`"));
}

#[test]
fn a_consumer_runs_its_nested_dependency_from_the_returned_directory() {
    if target::TargetProfile::host_if_supported().is_none() {
        return;
    }
    let project = TemporaryDirectory::create("returned-nested-dependency", false).unwrap();
    let sample =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/cli/probes/build_retry_reseal");
    let root = project.path();
    fs::create_dir(root.join("dependency")).unwrap();
    for name in [
        "build.omg",
        "main.omg",
        "dependency/build.omg",
        "dependency/main.omg",
        "dependency/seed.omg",
        "dependency/template.txt",
    ] {
        fs::copy(sample.join(name), root.join(name)).unwrap();
    }
    for engine in ["psi", "native"] {
        let result = run(root, engine, &[]);
        assert!(result.status.success(), "{}", report(&result));
        assert_returned(root, engine);
    }

    let generator = root.join("dependency/build.omg");
    let source = fs::read_to_string(&generator).unwrap();
    let selected = "BuildOutput::resolve(@builder.output, \"generated.omg\")";
    assert!(source.contains(selected));
    fs::write(
        generator,
        source.replace(
            selected,
            "BuildOutput::resolve(@builder.output, \"../outside.omg\")",
        ),
    )
    .unwrap();
    let result = run(root, "psi", &[]);
    assert!(!result.status.success());
    assert!(
        report(&result).contains("Build path is not a canonical relative path"),
        "{}",
        report(&result)
    );
}
