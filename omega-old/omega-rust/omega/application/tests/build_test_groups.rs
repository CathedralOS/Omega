/*

End-to-end CLI check that a Build's test groups run
(wiki/spec/build/testing.md).

`builder.tests.group<Tests::arithmetic>()` registers an ordinary trait
requirement as a test group. After the application compiles, every machine
that satisfies the requirement is a test: it is checked again as its own root,
run on the Psi interpreter and natively, and passes only by returning. A test
that crashes refuses the build. These twins differ only in one test's expected
sum.

The Build's own refusals come before any test runs: registering a group twice,
configuring a service for an unregistered group, and naming a service slot.
The runner backs only std's `FilesystemService`, and nothing yet identifies
that slot without trusting a package name, so every named slot is refused,
including a same-spelled `FilesystemService` declared by the project itself.

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
        true -> failed()
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
        true -> failed()
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

machine main() { }
";

// Compile a project whose Build registers tests with `registration`, and
// return whether it compiled and its combined output.
fn build(name: &str, expected_sum: &str, registration: &str) -> (bool, String) {
    let root = std::env::temp_dir().join(format!(
        "omega-build-test-groups-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("staging directory");
    std::fs::write(
        root.join("main.omg"),
        TESTS.replace("EXPECTED", expected_sum),
    )
    .expect("staged program");
    std::fs::write(
        root.join("build.omg"),
        format!(
            "machine build(builder: &mut Build) {{
    Build::application(builder, \"{name}\");
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
{registration}
}}
"
        ),
    )
    .expect("staged Build");
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["--target", "linux_x86_64", "build.omg"])
        .current_dir(&root)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("omega runs");
    let _ = std::fs::remove_dir_all(&root);
    (
        output.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    )
}

const GROUP: &str = "    _ = builder.tests.group<Tests::arithmetic>();";

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
}

#[test]
fn the_build_refuses_malformed_group_requests_before_any_test_runs() {
    for (name, registration, refusal) in [
        (
            "twice",
            "    _ = builder.tests.group<Tests::arithmetic>();\n    _ = builder.tests.group<Tests::arithmetic>();",
            "test group registration repeats the same requirement",
        ),
        (
            "unregistered",
            "    _ = builder.tests.service<Tests::arithmetic, FilesystemService>();",
            "test group service names a group this Build has not registered",
        ),
        (
            "slot",
            "    _ = builder.tests.group<Tests::arithmetic>();\n    _ = builder.tests.service<Tests::arithmetic, FilesystemService>();",
            "the test runner cannot back `FilesystemService`",
        ),
    ] {
        let (compiled, output) = build(name, "5u32", registration);
        assert!(!compiled, "{name}: {output}");
        assert!(output.contains(refusal), "{name}: {output}");
        assert!(!output.contains("passed on"), "{name}: {output}");
    }
}
