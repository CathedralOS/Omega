//! The preparing Build chooses which package implementation the next Build calls.
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

fn project(target: &str, provider: Option<&str>, before_read: &str) -> TemporaryDirectory {
    let sandbox = TemporaryDirectory::create("staged-build-providers", false).unwrap();
    let app = sandbox.path().join("app");
    let meter = sandbox.path().join("meter");
    fs::create_dir(&app).unwrap();
    fs::create_dir(&meter).unwrap();
    fs::write(
        meter.join("build.omg"),
        r#"machine build(builder: @mutable Build) { Build::package(builder, "meter"); }"#,
    )
    .unwrap();
    fs::write(meter.join("main.omg"), SERVICE).unwrap();
    fs::write(meter.join("backings.omg"), BACKINGS).unwrap();

    let (provider_files, selection) = match provider {
        Some(name) => (
            "use {meter}/main.omg as api; use {meter}/backings.omg;",
            format!(
                "Build::select_provider::<|api|::Meter, |backings|::{name}>(builder, interpreter);"
            ),
        ),
        None => ("", String::new()),
    };
    fs::write(
        app.join("build.omg"),
        format!(
            r#"use {{omega}}/build/dependency.omg;
{provider_files}
machine build(builder: @mutable Build) {{
    let source: DependencyDescription = |dependency|::from_source(Source::Path {{ location: "../meter" }});
    _ = Build::dependency(builder, source);
    Build::build_entry(builder, "configure.omg");
    {selection}
}}"#
        ),
    )
    .unwrap();
    fs::write(
        app.join("configure.omg"),
        format!(
            r#"use {{meter}}/main.omg as api;
machine build(builder: @mutable Build) {{
    {before_read}
    let observed: u32 = |api|::Meter::read();
    Build::application(builder, "staged_provider");
    transition observed {{
        17u32 -> select_return(builder),
        23u32 -> select_trap(builder),
        _ -> reject(builder)
    }}
    machine select_return(builder: @mutable Build) {{ Build::bind(builder, {target}::ProgramEntry, return_normally); }}
    machine select_trap(builder: @mutable Build) {{ Build::bind(builder, {target}::ProgramEntry, trap); }}
    machine reject(builder: @mutable Build) {{ Build::application(builder, "unknown_reading"); }}
}}
machine return_normally() {{}}
machine trap() crashes Trap {{ crash Trap; }}"#
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
    for (provider, psi_termination, native_trap) in [
        ("Seventeen", "psi\treturn", false),
        ("TwentyThree", "psi\tcrash\tTrap", true),
    ] {
        let fixture = project(host.target_name(), Some(provider), "");
        for engine in ["native", "psi"] {
            let (output, report) = run(&fixture, engine);
            assert!(output.status.success(), "{provider} {engine}: {report}");
            let termination =
                fs::read_to_string(fixture.path().join(engine).join("termination.tsv")).unwrap();
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

#[test]
fn a_successor_with_an_unmapped_service_cannot_begin_execution() {
    let poison = r#"Build::build_entry(builder, "../successor-started.omg");"#;
    let fixture = project("linux_x86_64", None, poison);
    let (output, report) = run(&fixture, "psi");
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("before entry; no interpreter provider or host binding was supplied"),
        "{report}"
    );
    assert!(
        !report.contains("requires a canonical package-relative .omg filename"),
        "{report}"
    );
}
