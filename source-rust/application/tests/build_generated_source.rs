//! A Product imports the exact files its Build emitted. The producing Build retains its own inputs.
#[path = "support/project.rs"]
mod project;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::fs;
use std::path::Path;
use std::process::Output;

fn run(project: &Path, command: &str, inputs: &[&str]) -> Output {
    let mut arguments = vec![command];
    if command == "run" {
        arguments.extend(["--outputs", "observed"]);
    }
    for path in inputs {
        arguments.extend(["--build-input", *path]);
    }
    arguments.push("build.omg");
    if command == "build" {
        let mut build = std::process::Command::new(env!("CARGO_BIN_EXE_omega"));
        build.args(arguments).current_dir(project);
        let result = project::build_and_run_command(&mut build);
        if let Some(execution) = result.execution {
            assert!(execution.status.success(), "{execution:?}");
            assert!(
                execution.stdout.is_empty() && execution.stderr.is_empty(),
                "{execution:?}"
            );
        }
        result.compiler
    } else {
        project::omega(project, &arguments)
    }
}

fn report(result: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    )
}

fn assert_returned(root: &Path, command: &str) {
    if command == "build" {
        return;
    }
    let output = root.join("observed");
    let termination = fs::read_to_string(output.join("termination.tsv")).unwrap();
    assert_eq!(termination, "psi\treturn\n");
    assert!(fs::read(output.join("stdout")).unwrap().is_empty());
    assert!(fs::read(output.join("stderr")).unwrap().is_empty());
}

#[test]
fn handwritten_product_calls_a_generated_file() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let main = r#"use generated.omg;
use unused.omg;
public machine preparing() -> u8 { 1u8 }
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
            r#"use main.omg as shared;
machine build(plan: @mutable BuildPlan) {{
    _ = |shared|::preparing();
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
        for command in ["run", "build"] {
            let result = run(
                project.path(),
                command,
                &["build.omg", "main.omg", "next.template", "asset.txt"],
            );
            let report = report(&result);
            assert_eq!(result.status.success(), valid, "{command}: {report}");
            if valid {
                assert_returned(project.path(), command);
                assert!(
                    !report.contains("unused file import `generated`"),
                    "{report}"
                );
                assert_eq!(
                    report.matches("unused file import `unused`").count(),
                    1,
                    "{report}"
                );
            } else {
                assert!(
                    report.contains("no visible declaration"),
                    "{command}: {report}"
                );
                assert!(report.contains("generated.omg"), "{command}: {report}");
                assert!(report.contains("unused"), "{command}: {report}");
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
            _ = BuildSource::resolve(@plan.source, "../entered");
            let file: BuildPath = BuildOutput::resolve(@plan.output, "generated.omg");
            let writer: i32 = BuildOutput::create(@mutable plan.output, file, 0);
            _ = BuildOutput::write(@mutable plan.output, writer, "public machine read() -> u8 { 17u8 }");
            _ = BuildOutput::close(@mutable plan.output, writer);
            plan.product = BuildProduct::Package { name: "future" };
            transition plan.tests.disabled { true -> read_future(), false -> done() }
            machine read_future() { _ = |generated|::read(); }
            machine done() {}
        }
    "#,
        )],
    );
    let result = run(project.path(), "run", &["build.omg"]);
    assert!(!result.status.success());
    let missing = report(&result);
    assert!(
        missing.contains("no admitted logical file `generated.omg`"),
        "{missing}"
    );
    assert!(!missing.contains("Build activation failed"), "{missing}");

    // The first host operation gives a distinct, observable refusal once the
    // source checker admits the same Build with its import supplied.
    fs::write(
        project.path().join("generated.omg"),
        "public machine read() -> u8 { 17u8 }",
    )
    .unwrap();
    let entered = run(project.path(), "run", &["build.omg", "generated.omg"]);
    assert!(!entered.status.success());
    let message = report(&entered);
    assert!(
        message.contains("Build path is not a canonical relative path"),
        "{message}"
    );
}

#[test]
fn a_build_overload_requires_only_its_selected_body_imports() {
    let project = TemporaryDirectory::create("overloaded-build-imports", false).unwrap();
    fs::write(
        project.path().join("choices.omg"),
        r#"use future.omg;
public machine choose(value: u8) -> u32 { 7u32 }
public machine choose(value: bool) -> u32 { |future|::read() }"#,
    )
    .unwrap();
    for (argument, reason) in [
        ("true", "no admitted logical file `future.omg`"),
        ("1u8", "Build path is not a canonical relative path"),
    ] {
        fs::write(
            project.path().join("build.omg"),
            format!(
                r#"use choices.omg;
machine build(plan: @mutable BuildPlan) {{
    _ = BuildSource::resolve(@plan.source, "../entered");
    _ = |choices|::choose({argument});
    plan.product = BuildProduct::Package {{ name: "overloaded" }};
}}"#
            ),
        )
        .unwrap();
        let result = run(project.path(), "run", &["build.omg", "choices.omg"]);
        assert!(!result.status.success());
        let message = report(&result);
        assert!(message.contains(reason), "{argument}: {message}");
    }
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
    for command in ["run", "build"] {
        let result = run(root, command, &[]);
        assert!(result.status.success(), "{}", report(&result));
        assert_returned(root, command);
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
    let result = run(root, "run", &[]);
    assert!(!result.status.success());
    assert!(
        report(&result).contains("Build path is not a canonical relative path"),
        "{}",
        report(&result)
    );
}
