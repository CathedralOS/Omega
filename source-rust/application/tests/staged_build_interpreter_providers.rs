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

fn run(target: &str, provider: Option<&str>, before_read: &str) -> (Output, String) {
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
    Build::dependency(builder, source);
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
    machine select_return(builder: @mutable Build) {{ Build::bind(builder, {target}\ProgramEntry, return_normally); }}
    machine select_trap(builder: @mutable Build) {{ Build::bind(builder, {target}\ProgramEntry, trap); }}
    machine reject(builder: @mutable Build) {{ Build::application(builder, "unknown_reading"); }}
}}
machine return_normally() {{}}
machine trap() crashes Trap {{ crash Trap; }}"#
        ),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .current_dir(&app)
        .args([
            "run",
            "--both",
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
    let (returning, report) = run(host.target_name(), Some("Seventeen"), "");
    assert!(returning.status.success(), "{report}");
    for outcome in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == outcome), "{report}");
    }
    let (trapping, report) = run(host.target_name(), Some("TwentyThree"), "");
    assert!(!trapping.status.success(), "{report}");
    assert!(report.contains("interp crash: Trap"), "{report}");
    assert!(!report.contains("native exit 0"), "{report}");
}

#[test]
fn a_successor_with_an_unmapped_service_cannot_begin_execution() {
    let poison = r#"Build::build_entry(builder, "../successor-started.omg");"#;
    let (output, report) = run("linux_x86_64", None, poison);
    assert!(!output.status.success(), "{report}");
    assert!(report.contains("no selected plan supplies"), "{report}");
    assert!(
        !report.contains("requires a canonical package-relative .omg filename"),
        "{report}"
    );
}
