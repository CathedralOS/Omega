/*

End-to-end CLI check that a Build's test groups run
(wiki/spec/build/testing.md).

`builder.tests.group<Tests::arithmetic>()` registers an ordinary trait
requirement as a test group. After the application compiles, every machine
that satisfies the requirement is a test: it is checked again as its own root,
run on the Psi interpreter and natively, and passes only by returning. A test
that crashes refuses the build. These twins differ only in one test's expected
sum.

A duplicate registration fails before any tests run. The retired
`tests.service` spelling follows ordinary name resolution even when a visible
group and service match its arguments.

*/

use std::process::Command;

const TESTS: &str = "trait Tests {
    machine arithmetic()
        crashes Trap;
}

machine addition_is_correct()
satisfies Tests::arithmetic
crashes Trap
{
    transition 2u32 + 3u32 != EXPECTED {
        true -> failed(),
        _ -> passed()
    }

    machine failed() crashes Trap {
        crash Trap;
    }

    machine passed() { }
}

machine subtraction_is_correct()
satisfies Tests::arithmetic
crashes Trap
{
    transition 5u32 - 3u32 != 2u32 {
        true -> failed(),
        _ -> passed()
    }

    machine failed() crashes Trap {
        crash Trap;
    }

    machine passed() { }
}

public boundary trait Filesystem {
    machine sync() reaches none crashes none;
}

// Executing the application in place of a registered test would fail every passing case.
machine main() crashes Trap { crash Trap; }
";

// Compile a project whose Build registers tests with `registration`, and
// return whether it compiled and its combined output.
fn build(name: &str, expected_sum: &str, registration: &str) -> (bool, String) {
    build_with(name, expected_sum, registration, "linux_x86_64", &[])
}

fn build_with(
    name: &str,
    expected_sum: &str,
    registration: &str,
    target: &str,
    flags: &[&str],
) -> (bool, String) {
    let source = format!(
        "use main.omg as app;
machine build(builder: @mutable Build) {{
    Build::application(builder, \"{name}\");
    Build::bind(builder, {target}::ProgramEntry, |app|::main);
{registration}
}}"
    );
    build_project(&source, expected_sum, target, flags)
}

fn build_project(source: &str, expected_sum: &str, target: &str, flags: &[&str]) -> (bool, String) {
    let scratch = platform_custody::temporary_directory::TemporaryDirectory::create(
        "build-test-group",
        false,
    )
    .expect("private staging directory");
    let root = scratch.path();
    std::fs::write(
        root.join("main.omg"),
        TESTS.replace("EXPECTED", expected_sum),
    )
    .expect("staged program");
    std::fs::write(root.join("build.omg"), source).expect("staged Build");
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .arg("build")
        .args(flags)
        .args(["--target", target, "build.omg"])
        .current_dir(root)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("omega runs");
    if flags.contains(&"--report-file") {
        let report = std::fs::read_to_string(root.join("report.txt")).expect("build report");
        let stderr = String::from_utf8_lossy(&output.stderr);
        for test in stderr
            .lines()
            .filter(|line| line.starts_with("test plan for `") || line.starts_with("test `"))
        {
            assert!(
                report.lines().any(|row| row == test),
                "{test} missing from {report}"
            );
        }
    }
    (
        output.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    )
}

const GROUP: &str = "    _ = builder.tests.group::<|app|::Tests::arithmetic>();";

#[test]
fn a_registered_group_runs_each_satisfier_as_a_test() {
    let (compiled, output) = build("passing", "5u32", GROUP);
    assert!(compiled, "{output}");
    for test in ["addition_is_correct", "subtraction_is_correct"] {
        assert!(
            output.contains(&format!(
                "test `{test}` in group `Tests::arithmetic` (target `linux_x86_64`): passed on Psi interpreter"
            )),
            "{output}"
        );
    }
}

#[test]
fn a_test_that_crashes_refuses_the_build() {
    let (compiled, output) = build("failing", "6u32", GROUP);
    assert!(!compiled, "{output}");
    assert!(
        output.contains(
            "test `addition_is_correct` in group `Tests::arithmetic` failed on the Psi interpreter engine: crashed with Trap"
        ),
        "{output}"
    );
    assert!(
        output.contains("test `subtraction_is_correct` in group `Tests::arithmetic` (target `linux_x86_64`): passed on Psi interpreter"),
        "{output}"
    );
    let verdict = output.find("failed on the Psi interpreter engine").unwrap();
    for name in ["addition_is_correct", "subtraction_is_correct"] {
        let planned = format!("test plan for `{name}` in group `Tests::arithmetic`");
        let start = output.find(&planned).unwrap_or_else(|| panic!("{output}"));
        assert!(start < verdict, "{output}");
        let row = output[start..].lines().next().unwrap();
        assert!(
            row.contains("Psi interpreter scheduled") && row.contains("bindings"),
            "{row}"
        );
    }
}

#[test]
fn the_build_refuses_malformed_group_requests_before_any_test_runs() {
    for (name, registration, refusal) in [
        (
            "twice",
            "    _ = builder.tests.group::<|app|::Tests::arithmetic>();\n    _ = builder.tests.group::<|app|::Tests::arithmetic>();",
            "test group registration repeats the same requirement",
        ),
        (
            "wrong_adapter",
            "    _ = builder.tests.group::<|app|::Tests::arithmetic, |app|::Tests>();",
            "test group operand `|app|::Tests` must name one visible adapter machine with a body",
        ),
        (
            "extra_adapter",
            "    _ = builder.tests.group::<|app|::Tests::arithmetic, |app|::main, |app|::main>();",
            "one exact trait machine requirement and an optional adapter machine",
        ),
        (
            "retired_service",
            "    _ = builder.tests.group::<|app|::Tests::arithmetic>();\n    _ = builder.tests.service::<|app|::Tests::arithmetic, |app|::Filesystem>();",
            "this call has no visible declaration in its source scope",
        ),
    ] {
        let (compiled, output) = build(name, "5u32", registration);
        assert!(!compiled, "{name}: {output}");
        assert!(output.contains(refusal), "{name}: {output}");
        assert!(!output.contains("passed on"), "{name}: {output}");
    }
}

#[test]
fn comparing_registered_tests_reports_each_pair_and_keeps_failed_expectations() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let (ok, output) = build_with(
        "comparison",
        "5u32",
        GROUP,
        host.target_name(),
        &["--compare-tests", "--report-file", "report.txt"],
    );
    assert!(ok, "{output}");
    assert_eq!(
        output.matches("native/Psi comparison agrees").count(),
        2,
        "{output}"
    );

    let (ok, output) = build_with(
        "failed_expectation",
        "6u32",
        GROUP,
        host.target_name(),
        &["--compare-tests", "--report-file", "report.txt"],
    );
    assert!(!ok, "{output}");
    let failed = output
        .lines()
        .find(|row| row.contains("test `addition_is_correct`"))
        .unwrap_or_else(|| panic!("missing failed test: {output}"));
    assert!(failed.contains("crashed with Trap"), "{output}");
    assert!(failed.contains("native/Psi comparison agrees"), "{output}");
}

#[test]
fn a_build_with_no_groups_has_zero_comparisons() {
    let (ok, output) = build_with(
        "no_comparisons",
        "5u32",
        "",
        "linux_x86_64",
        &["--compare-tests"],
    );
    assert!(ok, "{output}");
    assert!(output.contains("0 registered test comparisons"), "{output}");
}

#[test]
fn an_artifact_only_build_runs_its_groups_before_publishing_files() {
    use platform_custody::temporary_directory::TemporaryDirectory;
    const BUILD: &str = include_str!("fixtures/build/run/artifact_only_group/build.omg");
    for (sum, should_publish) in [("5u32", true), ("6u32", false)] {
        let scratch = TemporaryDirectory::create("artifact-group", false).unwrap();
        let root = scratch.path();
        std::fs::write(root.join("build.omg"), BUILD).unwrap();
        std::fs::write(root.join("main.omg"), TESTS.replace("EXPECTED", sum)).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_omega"))
            .args(["build", "--target", "linux_arm64", "build.omg"])
            .current_dir(root)
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        let report = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.success(), should_publish, "{report}");
        let completed = root.join("build/completed");
        if should_publish {
            assert!(report.contains("passed on Psi interpreter"), "{report}");
            let publication = std::fs::read_dir(completed)
                .unwrap()
                .next()
                .unwrap()
                .unwrap();
            assert_eq!(
                std::fs::read(publication.path().join("files/blob.bin")).unwrap(),
                b"blobdata\n"
            );
        } else {
            assert!(
                report.contains("test `addition_is_correct`")
                    && report.contains("crashed with Trap"),
                "{report}"
            );
            assert!(!completed.exists(), "a failing test published its artifact");
        }
    }
}

fn rejected_roster(source: &str, requirement: &str) -> [String; 2] {
    let scratch = platform_custody::temporary_directory::TemporaryDirectory::create(
        "test-entry-location",
        false,
    )
    .unwrap();
    let root = scratch.path();
    std::fs::write(root.join("main.omg"), source).unwrap();
    std::fs::write(
        root.join("build.omg"),
        format!(
            r#"use main.omg as app;
machine build(builder: @mutable Build) {{
        Build::application(builder, "rejected_roster");
        Build::bind(builder, linux_arm64::ProgramEntry, |app|::main);
        _ = builder.tests.group::<|app|::{requirement}>();
    }}"#
        ),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args([
            "build",
            "--target",
            "linux_arm64",
            "--report-file",
            "report.txt",
            "build.omg",
        ])
        .current_dir(root)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let report = std::fs::read_to_string(root.join("report.txt")).unwrap();
    [String::from_utf8(output.stderr).unwrap(), report]
}

#[test]
fn a_rejected_test_entry_keeps_the_location_of_its_declaration() {
    let source = "trait Checks { machine answer() -> u8; }\nmachine emits_byte() -> u8 satisfies Checks::answer { 7u8 }\nmachine main() {}";
    for text in rejected_roster(source, "Checks::answer") {
        assert!(
            text.contains("test `emits_byte` in group `Checks::answer` cannot be produced"),
            "{text}"
        );
        assert!(
            text.contains("entry machine `emits_byte` returns a value"),
            "{text}"
        );
        assert!(text.contains("main.omg:2:9"), "{text}");
    }
}

#[test]
fn discovery_reports_each_unrunnable_machine_in_roster_order() {
    let source = "trait Checks { machine check(value: u8); }\n\
machine zebra(value: u8) satisfies Checks::check {}\n\
machine alpha(value: u8) satisfies Checks::check {}\n\
machine main() {}";
    for text in rejected_roster(source, "Checks::check") {
        let (alpha, zebra) = text
            .split_once("test `zebra` in group `Checks::check` is not runnable")
            .unwrap_or_else(|| panic!("{text}"));
        assert!(
            alpha.contains("test `alpha` in group `Checks::check` is not runnable"),
            "{text}"
        );
        assert!(alpha.contains("main.omg:3:9"), "{text}");
        assert!(zebra.contains("main.omg:2:9"), "{text}");
        assert!(
            !text.contains("passed on") && !text.contains("registered with no tests"),
            "{text}"
        );
    }
}

#[test]
fn a_batch_build_announces_both_targets_before_any_test_verdict() {
    let registration = format!(
        r"Build::bind(builder, linux_arm64::ProgramEntry, |app|::main);
{GROUP}"
    );
    let (ok, output) = build_with(
        "batch_schedule",
        "6u32",
        &registration,
        "linux_x86_64",
        &["--target", "linux_arm64", "--report-file", "report.txt"],
    );
    assert!(!ok, "{output}");
    let first_verdict = output
        .find("failed on the Psi interpreter engine")
        .unwrap_or_else(|| panic!("{output}"));
    let before = &output[..first_verdict];
    for target in ["linux_arm64", "linux_x86_64"] {
        for test in ["addition_is_correct", "subtraction_is_correct"] {
            let plan =
                format!("test plan for `{test}` in group `Tests::arithmetic` (target `{target}`)");
            assert!(before.contains(&plan), "{output}");
        }
    }
}

#[test]
fn a_required_native_engine_on_another_target_leaves_the_build_incomplete() {
    let target =
        if target::TargetProfile::host_if_supported() == Some(target::TargetProfile::LinuxArm64) {
            "linux_x86_64"
        } else {
            "linux_arm64"
        };
    let (ok, output) = build_with(
        "foreign_comparison",
        "5u32",
        GROUP,
        target,
        &["--compare-tests"],
    );
    assert!(!ok, "{output}");
    assert_eq!(
        output.matches("passed on Psi interpreter").count(),
        2,
        "{output}"
    );
    assert!(
        output.contains("its target does not execute on this host"),
        "{output}"
    );
    assert!(
        output.contains("native/Psi comparison unavailable"),
        "{output}"
    );
}

#[test]
fn a_group_selects_engines_for_one_target_without_weakening_required_runs() {
    let host = target::TargetProfile::host_if_supported();
    let cross = if host == Some(target::TargetProfile::LinuxArm64) {
        "linux_x86_64"
    } else {
        "linux_arm64"
    };
    let group = "let group: TestGroup = builder.tests.group::<|app|::Tests::arithmetic>();";
    let choose = |target, engine, policy| {
        format!(
            "TestGroup::engine(group, {target}, TestEngine::{engine}, TestEnginePolicy::{policy});"
        )
    };
    let required = choose(cross, "Native", "Required");
    let registration = format!("{group}\n{required}");
    let (ok, report) = build_with("required_native", "5u32", &registration, cross, &[]);
    assert!(!ok, "{report}");
    assert!(report.contains("native unavailable (required,"), "{report}");
    assert!(
        report.contains("required by the Build group but unavailable"),
        "{report}"
    );
    assert!(report.contains("passed on Psi interpreter"), "{report}");

    let optional = choose(cross, "Native", "IfAvailable");
    let registration = format!("{registration}\n{optional}");
    let (ok, report) = build_with("optional_native", "5u32", &registration, cross, &[]);
    assert!(ok, "{report}");
    assert!(
        report.contains("native unavailable (if available,"),
        "{report}"
    );

    let exclude = choose(cross, "Psi", "Disabled");
    let registration = format!("{registration}\n{exclude}");
    let (ok, report) = build_with("no_engine", "5u32", &registration, cross, &[]);
    assert!(!ok, "{report}");
    assert!(
        report.contains("Psi interpreter explicitly disabled"),
        "{report}"
    );
    assert!(report.contains("has no runnable engine"), "{report}");

    if let Some(host) = host {
        let target = host.target_name();
        let exclude_native = choose(target, "Native", "Disabled");
        let registration = format!("{registration}\n{exclude_native}");
        let (ok, report) = build_with(
            "excluded_comparison",
            "5u32",
            &registration,
            target,
            &["--compare-tests"],
        );
        assert!(!ok, "{report}");
        assert!(
            report.contains("Psi interpreter scheduled (required)"),
            "{report}"
        );
        assert!(report.contains("native explicitly disabled"), "{report}");
        assert!(report.contains("passed on Psi interpreter"), "{report}");
        assert!(
            report.contains("native/Psi comparison unavailable"),
            "{report}"
        );
    }
}

#[test]
fn explicit_process_exit_is_observed_on_both_engines_and_never_passes_the_test() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let report = failing_registered_fixture("registered_process_exit", host, &["--compare-tests"]);
    for (name, code, agreed) in [
        ("exits_with_23", 23, cfg!(windows)),
        ("exits_with_zero", 0, false),
        ("exits_with_256", 256, cfg!(windows)),
    ] {
        let row = report
            .lines()
            .find(|row| row.contains(&format!("test `{name}`")))
            .unwrap_or_else(|| panic!("{report}"));
        assert!(
            row.contains(&format!("exited the process with status {code}")),
            "{row}"
        );
        if code == 0 || (cfg!(unix) && (code & 255) == 0) {
            assert!(row.contains("exit 0 cannot distinguish"), "{row}");
        } else {
            assert!(
                row.contains(&format!("terminated with status {code}")),
                "{row}"
            );
        }
        let expected = if agreed {
            "native/Psi comparison agrees"
        } else {
            "native/Psi comparison unavailable because the host"
        };
        assert!(row.contains(expected), "{row}");
    }

    // All three failures have the same cause. The ordinary concise report must
    // still name each test and preserve the exact status supplied by Psi.
    let report = failing_registered_fixture("registered_process_exit", host, &[]);
    for (name, code) in [
        ("exits_with_zero", 0),
        ("exits_with_23", 23),
        ("exits_with_256", 256),
    ] {
        assert!(
            report
                .lines()
                .any(|row| row.contains(&format!("test `{name}`"))
                    && row.contains(&format!("exited the process with status {code}"))),
            "{report}"
        );
    }
}

fn failing_registered_fixture(name: &str, host: target::TargetProfile, options: &[&str]) -> String {
    let scratch =
        platform_custody::temporary_directory::TemporaryDirectory::create("registered-test", false)
            .unwrap();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/build/run")
        .join(name)
        .join("build.omg");
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .arg("build")
        .args(options)
        .args(["--target", host.target_name(), "--build-dir"])
        .arg(scratch.path())
        .arg(&fixture)
        .current_dir(scratch.path())
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    let report = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(!output.status.success(), "{report}");
    for artifact in ["omega-program", "omega-program.exe"] {
        assert!(
            !scratch.path().join(artifact).exists(),
            "{name} published {artifact}"
        );
    }
    report
}

#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn registered_output_ceiling_stops_only_the_exhausted_invocation() {
    let host = target::TargetProfile::host_if_supported().unwrap();
    let report = failing_registered_fixture("registered_output_limit", host, &["--compare-tests"]);
    let exhausted = report
        .lines()
        .find(|row| row.contains("test `fills_the_capture_buffer` in group"))
        .unwrap_or_else(|| panic!("{report}"));
    for engine in ["Psi interpreter", "native"] {
        let reason = format!(
            "has an incomplete result on the {engine} engine: stdout exceeded its 4194304-byte capture allowance"
        );
        assert!(exhausted.contains(&reason), "{exhausted}");
    }
    let returned = report
        .lines()
        .find(|row| row.contains("test `returns_normally` in group"))
        .unwrap_or_else(|| panic!("{report}"));
    assert!(
        returned.contains("passed on Psi interpreter, native")
            && returned.contains("native/Psi comparison agrees"),
        "{returned}"
    );
}

#[test]
fn a_package_build_runs_registered_tests_without_an_application_entry() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let target = host.target_name();
    let package = format!(
        "use main.omg as app;\nmachine build(builder: @mutable Build) {{ Build::package(builder, \"library\"); {GROUP} }}"
    );
    let (ok, report) = build_project(
        &package,
        "5u32",
        target,
        &["--compare-tests", "--report-file", "report.txt"],
    );
    assert!(ok, "{report}");
    assert_eq!(
        report.matches("native/Psi comparison agrees").count(),
        2,
        "{report}"
    );
    assert!(!report.contains("published native output"), "{report}");

    let bound_package = format!(
        "use main.omg as app;
machine build(builder: @mutable Build) {{ Build::package(builder, \"library\");
         Build::bind(builder, {target}::ProgramEntry, |app|::main); }}"
    );
    let (ok, report) = build_project(&bound_package, "5u32", target, &[]);
    assert!(
        !ok && report.contains("a package Product cannot bind an executable root"),
        "{report}"
    );
}
