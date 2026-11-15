use std::path::{Path, PathBuf};
use std::process::{self, Command, Stdio};

fn project(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures")
        .join(relative)
        .join("runtime_indexed_byte_view_writeback")
}

fn omega(directory: &Path, arguments: &[&str]) -> process::Output {
    Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(arguments)
        .current_dir(directory)
        .stdin(Stdio::null())
        .output()
        .expect("omega runs on the hosted target")
}

fn report(output: &process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    )
}

#[test]
fn a_call_replaces_only_the_selected_byte_view() {
    let project = project("run/ownership");
    let output = omega(&project, &["run", "--both", "--keep", "build.omg"]);
    let run = report(&output);
    assert!(output.status.success(), "{run}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(run.lines().any(|line| line == receipt), "{run}");
    }

    let kept = run
        .lines()
        .find_map(|line| line.strip_prefix("build dir kept: "))
        .expect("the differential run keeps its build directory");
    let artifact = Path::new(kept).join("program.oterm");
    let output = omega(
        &project,
        &["psi", "inspect", "--artifact", artifact.to_str().unwrap()],
    );
    let inspected = report(&output);
    assert!(output.status.success(), "{inspected}");
    assert!(
        inspected.lines().any(|line| {
            line.contains("kind=StructuralByteViewFieldStore {")
                && line.contains("path: [RuntimeIndex {")
        }),
        "{inspected}"
    );
    for index in ["FixedIndex(0)", "FixedIndex(1)"] {
        assert!(
            inspected.lines().any(|line| {
                line.contains("kind=StructuralByteSequenceFieldLength") && line.contains(index)
            }),
            "{inspected}"
        );
    }
    std::fs::remove_dir_all(kept).expect("remove the build directory kept by this test");
}

#[test]
fn a_written_entry_contract_cannot_omit_the_store_bound() {
    let project = project("fail/borrows");
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
