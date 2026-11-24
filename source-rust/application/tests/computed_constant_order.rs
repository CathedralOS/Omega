//! A compile-time value can need another of a different kind. `[u8; width()]`
//! needs the constant `width` returns, and a constant computed through
//! `Buffer<capacity()>` needs that argument first. The compiler evaluates
//! each value once the values it reads have theirs, so all three programs
//! here compile and agree natively and in Psi. Each one stores bytes in an
//! array whose length was computed, passes it where a fixed length is
//! required and exits 0 only when the bytes read back as stored.
//!
//! Two twins differ from a program in one line. In one, `capacity` reads the
//! constant whose initializer holds `Buffer<capacity()>`, so neither can be
//! evaluated first. In the other, the extent's call may crash, so it is not a
//! value the compiler can compute.
//!
//! `wiki/spec/language/constants.md`, "Evaluation", and
//! `wiki/spec/language/evaluation.md`. The order itself is tested in the
//! source compiler's `driver/compile_time_invocations/demand_order.rs` and
//! `constants/tests.rs`.

#[path = "support/program.rs"]
mod program;
mod support;

use program::invoke;
use std::path::PathBuf;

fn fixture(tier: &str, case: &str) -> String {
    let path: PathBuf = support::repository_root()
        .join("source-rust/application/tests/fixtures/constants")
        .join(tier)
        .join(case);
    std::fs::read_to_string(path.join("main.omg")).unwrap()
}

fn expected(case: &str) -> String {
    let path = support::repository_root()
        .join("source-rust/application/tests/fixtures/constants/fail")
        .join(case)
        .join("expected.txt");
    std::fs::read_to_string(path).unwrap().trim_end().to_owned()
}

fn runs_in_both_engines(case: &str) {
    program::assert_silent_return(&fixture("run", case));
}

/// The twin differs from its program in exactly `line`, and is refused with
/// its expected message at `position`.
fn twin_is_refused(run: &str, fail: &str, line: (&str, &str), position: &str) {
    let program = fixture("run", run);
    let twin = fixture("fail", fail);
    assert_eq!(program.matches(line.0).count(), 1);
    assert_eq!(program.replace(line.0, line.1), twin);
    let (output, report) = invoke(&twin, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(report.contains(&expected(fail)), "{report}");
    assert!(report.contains(position), "{report}");
}

#[test]
fn an_extent_call_that_reads_a_computed_constant_runs_in_both_engines() {
    runs_in_both_engines("extent_call_reads_constant");
}

#[test]
fn a_constant_computed_through_a_const_argument_call_runs_in_both_engines() {
    runs_in_both_engines("constant_through_const_argument");
}

#[test]
fn a_chain_of_arguments_and_constants_runs_in_both_engines() {
    runs_in_both_engines("const_argument_constant_chain");
}

#[test]
fn a_constant_that_its_own_argument_reads_is_refused() {
    twin_is_refused(
        "constant_through_const_argument",
        "constant_through_its_own_argument",
        (
            "machine capacity() -> u64 crashes none { 3u64 }",
            "machine capacity() -> u64 crashes none { TOTAL }",
        ),
        // `capacity()` in `Buffer<capacity()>`.
        "main.omg:10:24",
    );
}

#[test]
fn an_extent_call_that_may_crash_is_refused() {
    twin_is_refused(
        "extent_call_reads_constant",
        "extent_call_may_crash",
        (
            "machine width() -> u64 crashes none { WIDTH }",
            "machine width() -> u64 crashes Trap { crash Trap; }",
        ),
        // `width()` in `[u8; width()]`.
        "main.omg:11:30",
    );
}
