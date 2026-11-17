/*

The Build, not the source, chooses which implementation backs a requirement on
each target (`wiki/spec/build/provider_selection.md`, "Build-owned target
selection"). A program declares ordinary named implementations, none of them
marked with a target, and its Build has one `Build::select_provider` row per
target.

The positive program runs through those rows under `omega run --both` on this
host, and runs in Psi under each hosted target's row. Its twin drops the
`windows_x86_64` row: the three implementations are still in scope, but the
compiler must refuse the program for that target rather than pick one.

*/

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixture(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures")
        .join(relative)
}

fn omega_run(directory: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_omega"))
        .arg("run")
        .args(extra)
        .arg("main.omg")
        .current_dir(directory)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("omega runs")
}

fn printed(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn each_target_runs_the_implementation_its_build_row_selects() {
    let directory = fixture("run/providers/target_selected_implementation");
    let output = omega_run(&directory, &["--both"]);
    let text = printed(&output);
    assert!(output.status.success(), "{text}");
    assert!(text.contains("native exit 0"), "{text}");
    assert!(text.contains("interp exit: 0"), "{text}");
    for target in [
        "windows_x86_64",
        "linux_x86_64",
        "linux_arm64",
        "macos_arm64",
    ] {
        let output = omega_run(&directory, &["--engine", "psi", "--target", target]);
        let text = printed(&output);
        assert!(output.status.success(), "{target}: {text}");
        assert!(text.contains("Psi exit 0"), "{target}: {text}");
    }
}

#[test]
fn a_target_without_a_build_row_has_no_implementation() {
    let directory = fixture("fail/build/target_selected_implementation_missing_row");
    let output = omega_run(
        &directory,
        // Both requirements share one cause; ask for every diagnostic so the
        // assertions below check both exact missing selections.
        &["--verbose", "--engine", "psi", "--target", "windows_x86_64"],
    );
    let text = printed(&output);
    assert!(!output.status.success(), "{text}");
    for member in ["record", "no_follow_flag"] {
        let refusal = format!(
            "boundary requirement `TargetConstants::{member}` has no provider on target \
             `windows_x86_64`: the Build selects none and the target package declares no default"
        );
        assert!(text.contains(&refusal), "{text}");
    }
}
