//! A registered adapter supplies each test's mutable argument and checks the value it hands back.

use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Output, Stdio};

const BUILD: &str = include_str!("fixtures/build/run/registered_test_adapter/build.omg");
const PRODUCT: &str = include_str!("fixtures/build/run/registered_test_adapter/main.omg");
const CALL: &str = "    Test(@mutable fixture);";

fn compare(build: &str, product: &str, target: &str) -> (bool, String) {
    let package = TemporaryDirectory::create("omega-registered-test-adapter", false).unwrap();
    std::fs::write(package.path().join("main.omg"), product).unwrap();
    std::fs::write(
        package.path().join("build.omg"),
        build.replace("linux_x86_64", target),
    )
    .unwrap();
    let Output {
        status,
        stdout,
        stderr,
    } = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["build", "--compare-tests", "--target", target, "build.omg"])
        .current_dir(package.path())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (
        status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&stdout),
            String::from_utf8_lossy(&stderr)
        ),
    )
}

fn result_for<'a>(output: &'a str, test: &str) -> &'a str {
    let label = format!("test `{test}` in group `Checks::modify`");
    output
        .lines()
        .find(|line| line.contains(&label))
        .unwrap_or_else(|| panic!("missing result for {test}\n{output}"))
}

#[test]
fn each_test_receives_fresh_registers_and_hands_them_back_on_both_engines() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let (passed, output) = compare(BUILD, PRODUCT, host.target_name());
    assert!(passed, "{output}");
    for name in ["edit", "second"] {
        let result = result_for(&output, name);
        assert!(
            result.contains("passed on Psi interpreter, native"),
            "{output}"
        );
        assert!(result.contains("native/Psi comparison agrees"), "{output}");
    }

    let wrong_write = PRODUCT.replace("view.transmit = 9u8;", "view.transmit = 11u8;");
    let (passed, output) = compare(BUILD, &wrong_write, host.target_name());
    assert!(!passed, "{output}");
    let edit = result_for(&output, "edit");
    assert!(
        edit.contains("crashed with Trap") && edit.contains("native/Psi comparison agrees"),
        "{output}"
    );
    let second = result_for(&output, "second");
    assert!(
        second.contains("passed on Psi interpreter, native")
            && second.contains("native/Psi comparison agrees"),
        "{output}"
    );
}

#[test]
fn an_adapter_receives_only_the_satisfiers_of_its_concrete_requirement() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let group = "Checks<Primary>::modify";
    let build = BUILD.replace("Checks::modify", group);
    let product = PRODUCT
        .replace(
            "trait Checks {",
            "data Primary {}\ndata Alternate {}\ntrait Checks<Which> {",
        )
        .replacen(
            "satisfies Checks::modify",
            "satisfies Checks<Primary>::modify",
            1,
        )
        .replace(
            "satisfies Checks::modify",
            "satisfies Checks<Alternate>::modify",
        );
    let (passed, output) = compare(&build, &product, host.target_name());
    assert!(passed, "{output}");
    assert!(
        output.lines().any(
            |line| line.contains("test `edit` in group `Checks<Primary>::modify`")
                && line.contains("passed on Psi interpreter, native")
                && line.contains("native/Psi comparison agrees")
        ),
        "{output}"
    );
    assert!(!output.contains("test `second` in group"), "{output}");
}

#[test]
fn an_adapter_must_call_the_selected_test_on_each_normal_return() {
    // The unconditional bypass passes the adapter's handback check. A run
    // that merely executed this body would falsely report a passing test.
    let skipped = PRODUCT.replace(CALL, "    fixture.transmit = 9u8;");
    let (before, rest) = PRODUCT.split_once(CALL).unwrap();
    let (_, after) = rest.split_once("    machine done() {}").unwrap();
    let conditional = format!(
        "{before}    transition fixture.transmit == 3u8 {{
        true -> done(),
        _ -> Test(@mutable fixture)
    }}
    machine done() {{}}{after}"
    );
    // For edit, second is a valid handback but a different machine.
    let sibling = PRODUCT.replace(CALL, "    second(@mutable fixture);");
    for product in [skipped, conditional, sibling] {
        let (passed, output) = compare(BUILD, &product, "linux_x86_64");
        assert!(!passed, "{output}");
        let refused = output.lines().any(|line| {
            line.contains("test adapter `registers_fixture`")
                && line.contains("selected test `edit`")
        });
        assert!(refused, "{output}");
        assert!(
            !output
                .lines()
                .any(|line| line.contains("test `edit` in group")
                    && line.contains("native/Psi comparison agrees")),
            "{output}"
        );
    }
}
