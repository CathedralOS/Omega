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
    fs::write(
        project.path().join("main.omg"),
        r#"
        use generated.omg;
        public machine launch() crashes Trap {
            transition |generated|::read() == 17u8 { true -> success(), _ -> failure() }
            machine success() {}
            machine failure() { crash Trap; }
        }
    "#,
    )
    .unwrap();
    for (body, valid) in [
        ("public machine read() -> u8 { 17u8 }", true),
        ("public machine read() -> u8 { missing() }", false),
    ] {
        fs::write(project.path().join("asset.txt"), body).unwrap();
        let length = body.len();
        let source = format!(
            r#"use main.omg as app;
            machine build(builder: @mutable Build) {{
                Build::application(builder, "frozen");
                let asset: BuildPath = BuildSource::resolve(@builder.source, "asset.txt");
                let reader: i32 = BuildSource::open(@builder.source, asset, 0);
                let mutable bytes: [u8; {length}];
                _ = BuildSource::read(@builder.source, reader, @mutable bytes[..], {length});
                _ = BuildSource::close(@builder.source, reader);
                Build::include_source(builder, "generated.omg", @bytes[..]);
                Build::bind(builder, {}::ProgramEntry, |app|::launch);
            }}"#,
            host.target_name()
        );
        fs::write(project.path().join("build.omg"), source).unwrap();
        for engine in ["psi", "native"] {
            let result = run(
                project.path(),
                engine,
                &["build.omg", "main.omg", "asset.txt"],
            );
            let report = report(&result);
            assert_eq!(result.status.success(), valid, "{engine}: {report}");
            if !valid {
                assert!(report.contains("missing"), "{engine}: {report}");
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
        machine build(builder: @mutable Build) {
            let value: u8 = |generated|::read();
            Build::application(builder, "future");
            Build::include_source(builder, "generated.omg", "public machine read() -> u8 { 17u8 }");
        }
    "#,
    )
    .unwrap();
    let result = run(project.path(), "psi", &["build.omg"]);
    assert!(!result.status.success());
    assert!(report(&result).contains("no admitted logical file `generated.omg`"));
}
