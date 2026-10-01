use crate::TerminalMachineSelection;

/// The midpoint fold of two clamped bounds: the exact sum is provable only
/// from the intervals the two `min`/`max` selections give their results.
const CLAMPED_MIDPOINT: &str = "
    machine midpoint(lo: i32, hi: i32) -> i32 {
        (min(max(lo, 0), 31) + min(max(hi, 0 - 1), 30)) / 2
    }";

#[test]
fn clamped_selections_bound_an_exact_sum_of_their_results() {
    let checked = crate::front_end::checked_program(CLAMPED_MIDPOINT);
    let lowered =
        crate::machine_lowering::lower_machine(&checked, TerminalMachineSelection::Name("midpoint"))
            .expect("the clamped operands prove the exact sum");
    let predicates = lowered
        .semantic_module
        .scalar_block_invariants
        .iter()
        .map(|row| format!("{:?}", row.predicate))
        .collect::<Vec<_>>();
    for literal in ["Signed(0)", "Signed(31)", "Signed(-1)", "Signed(30)"] {
        assert!(
            predicates.iter().any(|predicate| predicate.contains(literal)),
            "a selection interval names {literal}: {predicates:?}"
        );
    }
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("every selection interval and the sum replay independently");
}

#[test]
fn a_selection_interval_is_proved_on_both_arms_before_it_is_retained() {
    let checked = crate::front_end::checked_program(CLAMPED_MIDPOINT);
    let mut lowered =
        crate::machine_lowering::lower_machine(&checked, TerminalMachineSelection::Name("midpoint"))
            .unwrap();
    // Widening one retained interval past what an arm forwards leaves the
    // module's evidence unable to replay that arrival.
    let row = lowered
        .semantic_module
        .scalar_block_invariants
        .iter_mut()
        .find(|row| format!("{:?}", row.predicate).contains("Signed(31)"))
        .expect("the low clamp's interval is retained");
    row.predicate = rewrite_literal(row.predicate.clone(), 31, 29);
    assert!(
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .is_err()
    );
}

fn rewrite_literal(
    predicate: semantic_vocabulary::Proposition,
    from: i128,
    to: i128,
) -> semantic_vocabulary::Proposition {
    use semantic_vocabulary::{IntegerValue, Proposition, ScalarTerm};
    let term = |term: ScalarTerm| match term {
        ScalarTerm::Integer {
            scalar_type,
            value: IntegerValue::Signed(value),
        } if value == from => ScalarTerm::Integer {
            scalar_type,
            value: IntegerValue::Signed(to),
        },
        other => other,
    };
    match predicate {
        Proposition::LessOrEqual(left, right) => Proposition::LessOrEqual(term(left), term(right)),
        Proposition::Conjunction(members) => Proposition::Conjunction(
            members
                .into_iter()
                .map(|member| rewrite_literal(member, from, to))
                .collect(),
        ),
        other => other,
    }
}
