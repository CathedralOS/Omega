/*

End-to-end CLI check that a dependency's provider default is laid out only for
a target that chooses it (`wiki/spec/layouts/plans.md`, "Layout demand and
validation").

The kit package `tests/fixtures/packages/layout-default-kit` declares target
defaults for `Measure`. Its Windows default stores an `Overlapping` record
whose layout policy places both fields at offset 0, which layout validation
refuses. Before this repair, every program depending on the kit was refused on
every target with that Windows overlap, even one that never calls `Measure`.

The kept program reaches `Measure` and replaces the Windows default in its own
Build, so the other targets choose the kit's `PairMeasure` default, whose Pair
layout is prepared for them; its entry traps unless byte 8 holds 42, the value
only the authored layout places there. Two variants drop the replacement: one
never reaches `Measure` and runs, because no target chooses a default; the
other reaches it, so Windows chooses the overlapping default and its layout
refuses the program.

*/

use std::path::{Path, PathBuf};
use std::process::Command;

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn omega(arguments: &[&str], directory: &Path) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(arguments)
        .current_dir(directory)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("omega runs");
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn fixture() -> PathBuf {
    repository().join("tests/fixtures/pass/providers/default_provider_layout_demand_exit")
}

/// A copy of the kept program whose Build keeps every kit default, with `main`
/// as its entry source.
fn default_only(name: &str, main: &str) -> PathBuf {
    let kit = repository()
        .join("tests/fixtures/packages/layout-default-kit")
        .canonicalize()
        .expect("kit package");
    let build = std::fs::read_to_string(fixture().join("build.omg")).expect("kept Build");
    let defaults = build
        .replace(
            "../../../packages/layout-default-kit",
            kit.to_str().expect("UTF-8 repository path"),
        )
        .lines()
        .filter(|line| !line.contains("select_provider") && !line.contains("Windows takes"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_ne!(defaults, build);
    let root = std::env::temp_dir().join(format!(
        "omega-default-provider-layout-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("staged project");
    std::fs::write(root.join("build.omg"), defaults).expect("staged Build");
    std::fs::write(root.join("main.omg"), main).expect("staged program");
    root
}

fn assert_agreed_exit_zero(report: &str) {
    assert!(report.contains("native exit 0"), "{report}");
    assert!(report.contains("interp exit: 0"), "{report}");
}

#[test]
fn a_chosen_dependency_default_is_laid_out_for_its_target_and_runs() {
    assert_agreed_exit_zero(&omega(&["run", "--both", "main.omg"], &fixture()));
}

#[test]
fn an_unchosen_dependency_default_is_never_laid_out() {
    let root = default_only(
        "unreached",
        "use layout_default_kit::main::Measure;\n\nmachine main() {\n}\n",
    );
    let report = omega(&["run", "--both", "main.omg"], &root);
    let _ = std::fs::remove_dir_all(&root);
    assert_agreed_exit_zero(&report);
}

#[test]
fn a_chosen_dependency_default_with_an_invalid_layout_is_refused() {
    let main = std::fs::read_to_string(fixture().join("main.omg")).expect("kept program");
    let root = default_only("reached", &main);
    let report = omega(&["--check", "main.omg"], &root);
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        report.contains(
            "layout policy for `Overlapping` on `windows_x86_64`: layout placements of field keys 1 and 2 overlap"
        ),
        "{report}"
    );
}
