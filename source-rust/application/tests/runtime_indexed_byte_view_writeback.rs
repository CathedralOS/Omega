#[path = "support/project.rs"]
mod project;
mod support;

use project::omega;
use std::path::{Path, PathBuf};
use std::process;

fn project(relative: &str, fixture: &str) -> PathBuf {
    support::repository_root().join(relative).join(fixture)
}

fn report(output: &process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    )
}

fn run_and_reopen(project: &Path) -> String {
    let interpreted = omega(project, &["run", "--engine", "psi", "build.omg"]);
    let psi = report(&interpreted);
    assert!(
        interpreted.status.success() && psi.lines().any(|line| line == "Psi exit 0"),
        "{psi}"
    );
    let output = omega(
        project,
        &["run", "--engine", "native", "--keep", "build.omg"],
    );
    let run = report(&output);
    assert!(
        output.status.success() && run.lines().any(|line| line == "native exit 0"),
        "{run}"
    );

    let kept = run
        .lines()
        .find_map(|line| line.strip_prefix("build dir kept: "))
        .expect("the native run keeps the product containing its input Psi");
    let artifact = Path::new(kept).join("program.oterm");
    let output = omega(
        project,
        &["psi", "inspect", "--artifact", artifact.to_str().unwrap()],
    );
    let inspected = report(&output);
    assert!(output.status.success(), "{inspected}");
    std::fs::remove_dir_all(kept).expect("remove the build directory kept by this test");
    inspected
}

fn indexed_view_store(inspected: &str) -> usize {
    let store = inspected
        .lines()
        .position(|line| {
            line.contains("kind=StructuralByteViewFieldStore {")
                && line.contains("path: [RuntimeIndex {")
                && line.contains("obligation: ObligationId(")
        })
        .unwrap_or_else(|| panic!("{inspected}"));
    for index in ["FixedIndex(0)", "FixedIndex(1)"] {
        assert!(
            inspected.lines().any(|line| {
                line.contains("kind=StructuralByteSequenceFieldLength") && line.contains(index)
            }),
            "{inspected}"
        );
    }
    store
}

#[test]
fn a_call_replaces_only_the_selected_byte_view() {
    let project = project(
        "source-rust/application/tests/fixtures/ownership/run",
        "runtime_indexed_byte_view_writeback",
    );
    indexed_view_store(&run_and_reopen(&project));
}

#[test]
fn a_runtime_store_keeps_the_old_owned_array_contents_until_replacement() {
    let project = project(
        "source-rust/application/tests/fixtures/ownership/run",
        "owned_runtime_indexed_byte_view_writeback",
    );
    let inspected = run_and_reopen(&project);
    let store = indexed_view_store(&inspected);
    assert!(
        !inspected.lines().take(store).any(|line| {
            line.contains("kind=ReleaseReference {") && line.contains("FixedIndex(1)")
        }),
        "{inspected}"
    );
    let sibling_read = inspected.lines().position(|line| {
        line.contains("kind=StructuralByteSequenceFieldLength") && line.contains("FixedIndex(0)")
    });
    let sibling_release = inspected.lines().position(|line| {
        line.contains("kind=ReleaseReference {") && line.contains("FixedIndex(0)")
    });
    assert!(
        sibling_read
            .zip(sibling_release)
            .is_some_and(|(read, release)| store < read && read < release),
        "{inspected}"
    );
}

#[test]
fn an_indexed_store_refuses_an_owned_array_with_a_moved_element() {
    let project = project(
        "source-rust/application/tests/fixtures/borrows/fail",
        "owned_runtime_indexed_byte_view_writeback",
    );
    let output = omega(&project, &["--check", "build.omg"]);
    let refused = report(&output);
    assert_eq!(output.status.code(), Some(1), "{refused}");
    assert!(
        refused.contains("runtime-indexed replacement in an incomplete affine array")
            && refused.contains("main.omg:10:21"),
        "{refused}"
    );
}

#[test]
fn a_written_entry_contract_cannot_omit_the_store_bound() {
    let project = project(
        "source-rust/application/tests/fixtures/borrows/fail",
        "runtime_indexed_byte_view_writeback",
    );
    let output = omega(&project, &["--check", "build.omg"]);
    let refused = report(&output);
    assert_eq!(output.status.code(), Some(1), "{refused}");
    assert!(
        refused.lines().any(|line| {
            line.contains("cannot prove operation safety in machine `replace_identifier` state `entry`: cannot prove index is within length 2.")
        }),
        "{refused}"
    );
    assert!(refused.contains("main.omg:6:21"), "{refused}");
}
