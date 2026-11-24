//! A wholly anonymous calculation keeps exact rational intermediates, and `as`
//! to an integer type lands its complete value: `((5 / 2) * 2) as u8` is 5.
//!
//! The program returns three such values (`((5 / 2) * 2) as u8`,
//! `(7 / 2 + 7 / 2) as i32` and the decimal literal `1.0e0` landing in `u8`)
//! and proves a fourth in a contract (`ensures result == ((9 / 2) * 2) as u8`),
//! and it exits 0 only when each is the exact value. Its twin drops the `as` from
//! the first, so the calculation reaches `u8` with a fractional intermediate
//! that nothing acknowledges, and the compiler rejects it at `5 / 2`.
//!
//! `wiki/spec/language/numeric_values.md`, "Literal landing and destinations"
//! ("Anonymous integer-literal division is rational division, not
//! truncation") and "Fractional-intermediate diagnostics" ("`as T` applied to
//! an anonymous expression, with a computational integer destination `T`,
//! explicitly lands its complete operand"; "An authored anonymous calculation
//! with a fractional intermediate rejects at an implicit computational integer
//! landing"). Lowering's own test is
//! `wholly_anonymous_arithmetic_lands_exactly_or_is_refused` in
//! `pipeline/05_prepared-typed-trees-to-lowered-program/src/read.rs`.

#[path = "support/program.rs"]
mod program;
mod support;

use program::invoke;

fn fixture(tier: &str, case: &str) -> String {
    let path = support::repository_root()
        .join("source-rust/application/tests/fixtures/numbers")
        .join(tier)
        .join(case)
        .join("main.omg");
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn as_lands_anonymous_calculations_exactly_natively_and_in_psi() {
    program::assert_silent_return(&fixture("run", "explicit_landing"));
}

#[test]
fn without_as_a_fractional_intermediate_is_rejected_where_it_arises() {
    // The twins differ only in the `as` around the first calculation.
    assert_eq!(
        fixture("run", "explicit_landing").replace("((5 / 2) * 2) as u8", "(5 / 2) * 2"),
        fixture("fail", "implicit_landing")
    );
    let (output, report) = invoke(&fixture("fail", "implicit_landing"), &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains(
            "this part of an anonymous calculation is fractional, and the calculation lands its exact value 5 in `u8` without `as`"
        ),
        "{report}"
    );
    // The rejection points at the division, column 27 of line 1.
    assert!(report.contains("main.omg:1:27"), "{report}");
    let line = fixture("fail", "implicit_landing");
    assert_eq!(&line.lines().next().unwrap()[26..27], "/");
}

#[test]
fn as_does_not_land_a_fractional_or_out_of_range_value() {
    for (calculation, rejection) in [
        (
            "(5 / 2) as u8",
            "this anonymous calculation has the exact value 5/2 and lands in `u8`",
        ),
        (
            "((513 / 2) * 2) as u8",
            "this anonymous calculation has the exact value 513, which does not fit in `u8`",
        ),
    ] {
        let source = fixture("run", "explicit_landing").replace("((5 / 2) * 2) as u8", calculation);
        let (output, report) = invoke(&source, &["--check"]);
        assert!(!output.status.success(), "{report}");
        assert!(report.contains(rejection), "{calculation}: {report}");
    }
}
