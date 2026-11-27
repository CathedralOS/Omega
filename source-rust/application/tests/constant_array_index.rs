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

/// A lookup table wider than an application key: 3,072 elements of `u32`
/// encode past the 64 KiB and 4,096-node bounds on a key, but a constant's
/// own value has its own, larger bounds.
fn wide_table(guard: &str) -> String {
    let elements = (0..3_072u32)
        .map(|index| format!("{}u32", 4_000_000_000u32 - index))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "const TABLE: [u32; 3072] = [{elements}];\n\
         \n\
         machine read(position: u64) -> u32\n\
         requires {guard}\n\
         crashes none\n\
         {{ TABLE[position] }}\n\
         \n\
         machine main() crashes Trap {{\n    \
             transition read(0u64) == 4000000000u32 && read(3071u64) == 3999996929u32 {{\n        \
                 true -> done(), false -> bad()\n    \
             }}\n    \
             machine done() {{}}\n    \
             machine bad() crashes Trap {{ crash Trap; }}\n\
         }}\n"
    )
}

#[test]
fn a_table_wider_than_an_application_key_reads_its_last_element() {
    program::assert_silent_return(&wide_table("position < 3072u64"));
}

#[test]
fn the_wide_table_still_checks_every_index_against_its_length() {
    let (output, report) = program::invoke(&wide_table("position <= 3072u64"), &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("cannot prove index is within length 3072"),
        "{report}"
    );
}
