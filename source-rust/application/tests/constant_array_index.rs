//! A named array supports the same bounded index as a local array.
#[path = "support/program.rs"]
mod program;

const SOURCE: &str = include_str!("fixtures/constants/run/constant_array_index/main.omg");

fn expect_native_and_psi_success(source: &str) {
    let (output, report) = program::invoke(source, &["run", "--both"]);
    assert_eq!(output.status.code(), Some(0), "{report}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(report.lines().any(|line| line == receipt), "{report}");
    }
}

const BORROW: &str = include_str!("fixtures/constants/run/borrowed_constant_projection/main.omg");

#[test]
fn a_proven_index_reads_the_selected_constant() {
    expect_native_and_psi_success(SOURCE);
}

#[test]
fn borrows_of_elements_and_slices_read_the_constant_copy() {
    expect_native_and_psi_success(BORROW);
}

#[test]
fn an_element_of_a_constant_cannot_be_borrowed_for_writing() {
    let source = BORROW
        .replace("value: @u8", "value: @mutable u8")
        .replace("deref(@BYTES", "deref(@mutable BYTES");
    let (output, report) = program::invoke(&source, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("a constant has no writable storage"),
        "{report}"
    );
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
