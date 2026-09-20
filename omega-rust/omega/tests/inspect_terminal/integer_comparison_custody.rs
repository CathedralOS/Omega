//! Integer-comparison custody through the inspection route.

use super::{inspect, lower_source, remove_fixture, temporary_source};
use compiler::CheckedCompileRequest;

/// `inspect-terminal` runs the same integer-comparison custody join the
/// retained and direct native routes check: a selected `==` use must rejoin
/// its admitted emission triple before the machine inspects clean.
#[test]
fn integer_comparison_inspection_runs_the_exact_custody_join() {
    let source = temporary_source(
        "integer-custody",
        r#"
            boundary machine == Comparison::equal(left: i32, right: i32) -> bool
            crashes Trap !(right >= 0);

            data ComparisonProvider { }

            machine ComparisonProvider::equal(left: i32, right: i32) -> bool
                satisfies Comparison::equal
                via Binding::CompilerIntrinsic;

            machine may_crash(left: i32, right: i32) -> bool {
                left == right
            }
        "#,
    );

    let output = inspect("may_crash", &source);
    let lowered = lower_source("may_crash", &source);
    remove_fixture(source);

    assert!(
        output.status.success(),
        "integer-comparison inspection failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("verified=true"), "{stdout}");
    assert_eq!(
        lowered.selected_integer_comparison_occurrences.len(),
        1,
        "the fixture must carry one selected integer comparison"
    );
}

/// A drifted recorded triple is not a different admitted spelling: flipping
/// the negation flag on the lone occurrence must fail the join the inspection
/// route now runs.
#[test]
fn integer_comparison_custody_rejects_a_negated_recorded_triple() {
    let source = temporary_source(
        "integer-custody-negated",
        r#"
            boundary machine == Comparison::equal(left: i32, right: i32) -> bool
            crashes Trap !(right >= 0);

            data ComparisonProvider { }

            machine ComparisonProvider::equal(left: i32, right: i32) -> bool
                satisfies Comparison::equal
                via Binding::CompilerIntrinsic;

            machine may_crash(left: i32, right: i32) -> bool {
                left == right
            }
        "#,
    );

    let checked = compiler::compile_to_checked(CheckedCompileRequest::new(&source, None))
        .expect("check integer-comparison fixture");
    let mut lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "may_crash")
        .expect("lower the compared machine");
    remove_fixture(source);

    let [occurrence] = lowered.selected_integer_comparison_occurrences.as_slice() else {
        panic!("one selected integer comparison")
    };
    assert!(!occurrence.negated);
    lowered.selected_integer_comparison_occurrences[0].negated = true;
    assert!(
        compiler::validate_lowered_integer_comparison_custody(&checked, &lowered).is_err(),
        "a negation-drifting occurrence must fail the custody join"
    );
}
