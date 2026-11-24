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

public boundary trait FilesystemService {
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
    std::fs::write(
        root.join("build.omg"),
        format!(
            "machine build(builder: @mutable Build) {{
    Build::application(builder, \"{name}\");
    Build::bind(builder, {target}\\ProgramEntry, main);
{registration}
}}
"
        ),
    )
    .expect("staged Build");
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .arg("build")
        .args(flags)
        .args(["--target", target, "build.omg"])
        .current_dir(root)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("omega runs");
    if flags.contains(&"--report-file") && output.status.success() {
        let report = std::fs::read_to_string(root.join("report.txt")).expect("build report");
        let stderr = String::from_utf8_lossy(&output.stderr);
        for test in stderr.lines().filter(|line| line.starts_with("test `")) {
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

const GROUP: &str = "    _ = builder.tests.group::<Tests::arithmetic>();";

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
}

#[test]
fn the_build_refuses_malformed_group_requests_before_any_test_runs() {
    for (name, registration, refusal) in [
        (
            "twice",
            "    _ = builder.tests.group::<Tests::arithmetic>();\n    _ = builder.tests.group::<Tests::arithmetic>();",
            "test group registration repeats the same requirement",
        ),
        (
            "wrong_adapter",
            "    _ = builder.tests.group::<Tests::arithmetic, Tests>();",
            "test group operand `Tests` must name one visible adapter machine with a body",
        ),
        (
            "extra_adapter",
            "    _ = builder.tests.group::<Tests::arithmetic, main, main>();",
            "one exact trait machine requirement and an optional adapter machine",
        ),
        (
            "retired_service",
            "    _ = builder.tests.group::<Tests::arithmetic>();\n    _ = builder.tests.service::<Tests::arithmetic, FilesystemService>();",
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
        &["--compare-tests"],
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
fn explicit_process_exit_is_observed_on_both_engines_and_never_passes_the_test() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let scratch =
        platform_custody::temporary_directory::TemporaryDirectory::create("exit-test", false)
            .unwrap();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/build/run/registered_process_exit/build.omg");
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args([
            "build",
            "--compare-tests",
            "--target",
            host.target_name(),
            "--build-dir",
        ])
        .arg(scratch.path())
        .arg(&fixture)
        .current_dir(scratch.path())
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let report = String::from_utf8_lossy(&output.stderr);
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
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["build", "--target", host.target_name(), "--build-dir"])
        .arg(scratch.path())
        .arg(&fixture)
        .current_dir(scratch.path())
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let report = String::from_utf8_lossy(&output.stderr);
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
