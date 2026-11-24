//! A named array supports the same bounded index as a local array.
#[path = "support/program.rs"]
mod program;

const SOURCE: &str = include_str!("fixtures/constants/run/constant_array_index/main.omg");

const BORROW: &str = include_str!("fixtures/constants/run/borrowed_constant_projection/main.omg");

#[test]
fn a_proven_index_reads_the_selected_constant() {
    program::assert_silent_return(SOURCE);
}

#[test]
fn borrows_of_elements_and_slices_read_the_constant_copy() {
    program::assert_silent_return(BORROW);
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
fn a_borrow_of_the_constant_copy_cannot_escape_its_invocation() {
    let source = SOURCE
        .replace("machine read(position:", "machine leak(position: u64) -> @u8 requires position < 3u64 { @BYTES[position] }\nmachine read(position:")
        .replace("{ BYTES[position] }", "{ let value: @u8 = leak(position); $value }");
    let (output, report) = program::invoke(&source, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("local storage cannot establish"),
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
