use super::{assert_diagnostic, source, write_program};
use compiler::{CheckedCompileRequest, compile_to_checked};

#[test]
fn source_placement_custody_accepts_the_exact_erased_field_projection() {
    let main = write_program("exact", &source("    authority: Evidence;"));
    compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("exact placement custody should compile");
}

#[test]
fn source_placement_custody_rejects_a_missing_erased_field() {
    let main = write_program("missing", &source(""));
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("missing placement custody must fail closed");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "custody-carried",
            "Packet.authority",
            "omits",
        ],
    );
}

#[test]
fn source_placement_custody_rejects_an_extra_represented_field() {
    let main = write_program(
        "represented",
        &source("    authority: Evidence;\n    bits: u32;"),
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("represented placement fields must remain absent from custody");
    assert_diagnostic(
        &diagnostics,
        &[
            "Native::plan",
            "Packet.bits",
            "represented at offset 0 with width 4",
            "must be absent",
        ],
    );
}
