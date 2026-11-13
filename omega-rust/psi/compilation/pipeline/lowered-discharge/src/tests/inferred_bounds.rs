// What inference states of a value that crosses a contract boundary beyond
// the facts held about it: the interval the caller derives for an actual, a
// bound that holds only by cases, and what a helper with an omitted input
// returns once that input is inferred.

use super::*;

fn with_private(
    machines: Vec<Machine>,
    inputs: &[u64],
    results: &[u64],
) -> structure_elaboration::Elaboration {
    let mut elaboration = lower(machines).unwrap();
    elaboration.private_inputs = inputs.iter().copied().map(id).collect();
    elaboration.local_results = results.iter().copied().map(id).collect();
    elaboration
}

fn accepted(elaboration: structure_elaboration::Elaboration) -> Box<crate::AcceptedClosure> {
    let verdict = judge(Ok(elaboration)).unwrap();
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("the program must be accepted: {verdict:?}")
    };
    for (machine, result) in accepted.discharge.machines.iter() {
        for (obligation, verdict) in result.verdicts.iter() {
            if let Ok(Discharged::Certificate(evidence)) = verdict {
                assert_kernel_accepted(&accepted.elaboration, machine.get(), obligation, evidence);
            }
        }
    }
    accepted
}

fn unproved(elaboration: structure_elaboration::Elaboration) -> Vec<UnprovedObligation> {
    match judge(Ok(elaboration)).unwrap() {
        RouteVerdict::Rejected { unproved, .. } => unproved,
        verdict => panic!("the program must be rejected: {verdict:?}"),
    }
}

/// `value - subtrahend`, with no contract.
fn lowered_by(subtrahend: u128) -> Machine {
    machine(
        vec![u32_type()],
        vec![returns(strict(
            Strict::Subtract,
            slot(0, u32_type()),
            unsigned(subtrahend),
        ))],
    )
}

#[test]
fn an_inferred_input_carries_the_interval_of_a_computed_actual() {
    // `total` is a sum the caller holds no fact about; only its derived
    // interval says the helper's subtraction is safe.
    let caller = |low: u128| Machine {
        requires: vec![
            at_least(1, low),
            at_most(1, 30),
            at_least(2, 10),
            at_most(2, 30),
        ],
        ..machine(
            vec![u32_type(), u32_type()],
            vec![returns(call(
                2,
                vec![strict(
                    Strict::Add,
                    slot(0, u32_type()),
                    slot(1, u32_type()),
                )],
            ))],
        )
    };
    accepted(with_private(vec![caller(10), lowered_by(20)], &[2], &[]));
    let unproved = unproved(with_private(vec![caller(9), lowered_by(20)], &[2], &[]));
    assert!(
        unproved
            .iter()
            .any(|row| row.machine == id(2) && safety(&row.kind))
    );
}

/// Returns 5 when its formal is positive, else 9.
fn two_exits() -> Machine {
    machine(
        vec![u32_type()],
        vec![
            guarded(
                compare(Strict::Less, unsigned(0), slot(0, u32_type())),
                unsigned(5),
            ),
            returns(unsigned(9)),
        ],
    )
}

#[test]
fn a_helper_with_an_omitted_input_is_summarized_again_once_its_input_is_inferred() {
    // Both helpers omit `requires`. The first returns 5 or 9 whatever it is
    // given, which the second's inferred input needs. The second returns
    // `value - 5` and is summarized twice: before its input is known, and
    // again with it, which is what the entry's own subtraction needs.
    let entry = |limit: u128| {
        machine(
            vec![u32_type()],
            vec![returns(strict(
                Strict::Subtract,
                unsigned(limit),
                call(3, vec![call(2, vec![slot(0, u32_type())])]),
            ))],
        )
    };
    accepted(with_private(
        vec![entry(4), two_exits(), lowered_by(5)],
        &[2, 3],
        &[2, 3],
    ));
    let unproved = unproved(with_private(
        vec![entry(3), two_exits(), lowered_by(5)],
        &[2, 3],
        &[2, 3],
    ));
    assert!(
        unproved
            .iter()
            .any(|row| row.machine == id(1) && safety(&row.kind))
    );
    let unproved = unproved_in_helper(6);
    assert!(
        unproved
            .iter()
            .any(|row| row.machine == id(3) && safety(&row.kind))
    );
}

fn unproved_in_helper(subtrahend: u128) -> Vec<UnprovedObligation> {
    let entry = machine(
        vec![u32_type()],
        vec![returns(call(3, vec![call(2, vec![slot(0, u32_type())])]))],
    );
    unproved(with_private(
        vec![entry, two_exits(), lowered_by(subtrahend)],
        &[2, 3],
        &[2, 3],
    ))
}

#[test]
fn a_result_bounded_only_under_a_conjunction_guard_is_described() {
    // `when 20 <= v && v <= 60 -> v; else 40`: on the first return the
    // limits of `v` lie inside the join of the two comparisons, a
    // disjunctive fact, so they hold by cases only.
    let guard = expression(
        ExpressionKind::ShortCircuit {
            conjunction: true,
            left: Box::new(compare(
                Strict::LessOrEqual,
                unsigned(20),
                slot(0, u32_type()),
            )),
            right: Box::new(compare(
                Strict::LessOrEqual,
                slot(0, u32_type()),
                unsigned(60),
            )),
        },
        Some(ScalarType::Boolean),
    );
    let helper = || {
        machine(
            vec![u32_type()],
            vec![
                guarded(guard.clone(), slot(0, u32_type())),
                returns(unsigned(40)),
            ],
        )
    };
    let caller = |limit: u128| {
        machine(
            vec![u32_type()],
            vec![returns(strict(
                Strict::Subtract,
                unsigned(limit),
                call(2, vec![slot(0, u32_type())]),
            ))],
        )
    };
    accepted(with_private(vec![caller(60), helper()], &[], &[2]));
    let unproved = unproved(with_private(vec![caller(59), helper()], &[], &[2]));
    assert!(
        unproved
            .iter()
            .any(|row| row.machine == id(1) && safety(&row.kind))
    );
}
