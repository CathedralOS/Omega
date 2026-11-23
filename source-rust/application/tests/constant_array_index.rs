//! A named array supports the same bounded index as a local array.
#[path = "support/program.rs"]
mod program;

const SOURCE: &str = include_str!("fixtures/constants/run/constant_array_index/main.omg");

#[test]
fn a_proven_index_reads_the_selected_constant() {
    let (output, report) = program::invoke(SOURCE, &["run", "--both"]);
    assert_eq!(output.status.code(), Some(0), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

#[test]
fn the_index_must_be_strictly_below_the_length() {
    let (before, after) = SOURCE.split_once("position < 3u64").unwrap();
    let source = format!("{before}position <= 3u64{after}");
    let (output, report) = program::invoke(&source, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("cannot prove index is within length 3"),
        "{report}"
    );
}
