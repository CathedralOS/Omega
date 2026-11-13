// Which unproved goals are `Incomplete` rather than `Reject` (`prove::reach`):
// one that depends on a value an operation outside the integer rules
// defines, in the same machine or behind an inferred input or result, and
// never one that fails on operations the rules bound exactly.

use super::*;

fn incomplete(elaboration: structure_elaboration::Elaboration) -> Vec<UnprovedObligation> {
    match judge(Ok(elaboration)).unwrap() {
        RouteVerdict::Incomplete(unproved) => {
            assert!(
                unproved
                    .iter()
                    .all(|row| row.cause == UnprovedCause::NoRuleForGoalForm),
                "{unproved:?}"
            );
            unproved
        }
        verdict => panic!("the route must leave the program undecided: {verdict:?}"),
    }
}

fn no_derivation(elaboration: structure_elaboration::Elaboration) -> Vec<UnprovedObligation> {
    match judge(Ok(elaboration)).unwrap() {
        RouteVerdict::Rejected { unproved, .. } => {
            assert!(
                unproved
                    .iter()
                    .any(|row| row.cause == UnprovedCause::NoDerivation),
                "{unproved:?}"
            );
            unproved
        }
        verdict => panic!("the program must be rejected: {verdict:?}"),
    }
}

fn private(
    machines: Vec<Machine>,
    inputs: &[u64],
    results: &[u64],
) -> structure_elaboration::Elaboration {
    let mut elaboration = lower(machines).unwrap();
    elaboration.private_inputs = inputs.iter().copied().map(id).collect();
    elaboration.local_results = results.iter().copied().map(id).collect();
    elaboration
}

/// `limit - (value <operation> operand)` for `value` in `59..=59`.
fn headroom_of(limit: u128, operation: Strict, operand: Expression) -> Machine {
    Machine {
        requires: vec![at_most(1, 59), at_least(1, 59)],
        ..machine(
            vec![u32_type(), u32_type()],
            vec![returns(strict(
                Strict::Subtract,
                unsigned(limit),
                strict(operation, slot(0, u32_type()), operand),
            ))],
        )
    }
}

#[test]
fn a_goal_on_an_exclusive_or_is_incomplete() {
    // 59 ^ 15 is 52, so the subtraction is safe, and no rule says so.
    let unproved =
        incomplete(lower(vec![headroom_of(63, Strict::BitwiseXor, unsigned(15))]).unwrap());
    assert!(unproved.iter().all(|row| safety(&row.kind)));
}

#[test]
fn a_goal_on_a_quotient_by_a_value_is_incomplete_and_by_a_literal_rejects() {
    // 29 - 59 / d with d in 2..=3 is safe, and the rules know only that the
    // quotient is at most its dividend: they do not use what is known of `d`.
    let mut by_value = headroom_of(29, Strict::Divide, slot(1, u32_type()));
    by_value.requires = vec![
        at_most(1, 59),
        at_most(2, 3),
        at_least(1, 59),
        at_least(2, 2),
    ];
    incomplete(lower(vec![by_value]).unwrap());
    // 2 - 59 / 2 underflows, and the rules bound a quotient by a literal
    // exactly: the failure is the program's.
    let underflow = Machine {
        requires: vec![at_most(1, 59), at_least(1, 59)],
        ..machine(
            vec![u32_type()],
            vec![returns(strict(
                Strict::Subtract,
                unsigned(2),
                strict(Strict::Divide, slot(0, u32_type()), unsigned(2)),
            ))],
        )
    };
    no_derivation(lower(vec![underflow]).unwrap());
}

#[test]
fn a_goal_that_fails_on_exact_operations_alone_still_rejects() {
    // 58 - (value + 0) with value == 59: add and subtract are reached.
    let unproved = no_derivation(
        lower(vec![Machine {
            requires: vec![at_most(1, 59), at_least(1, 59)],
            ..machine(
                vec![u32_type()],
                vec![returns(strict(
                    Strict::Subtract,
                    unsigned(58),
                    strict(Strict::Add, slot(0, u32_type()), unsigned(0)),
                ))],
            )
        }])
        .unwrap(),
    );
    assert!(
        unproved
            .iter()
            .all(|row| row.cause == UnprovedCause::NoDerivation)
    );
}

/// Returns `value ^ 15`, with no contract.
fn mixed() -> Machine {
    machine(
        vec![u32_type()],
        vec![returns(strict(
            Strict::BitwiseXor,
            slot(0, u32_type()),
            unsigned(15),
        ))],
    )
}

fn subtracts_call(callee: u64) -> Machine {
    machine(
        vec![u32_type()],
        vec![returns(strict(
            Strict::Subtract,
            unsigned(63),
            call(callee, vec![slot(0, u32_type())]),
        ))],
    )
}

#[test]
fn an_unreached_result_of_a_private_helper_is_unreached_in_its_caller() {
    incomplete(private(vec![subtracts_call(2), mixed()], &[], &[2]));
    // A helper that is not a private subject body is known by its written
    // contract alone, and the caller needs more than it states.
    no_derivation(private(vec![subtracts_call(2), mixed()], &[], &[]));
}

#[test]
fn an_unreached_actual_is_unreached_in_the_helper_that_infers_its_input() {
    // The entry passes `value ^ 15` to a helper with no written input.
    let entry = machine(
        vec![u32_type()],
        vec![returns(call(
            2,
            vec![strict(
                Strict::BitwiseXor,
                slot(0, u32_type()),
                unsigned(15),
            )],
        ))],
    );
    let helper = || {
        machine(
            vec![u32_type()],
            vec![returns(strict(
                Strict::Subtract,
                unsigned(63),
                slot(0, u32_type()),
            ))],
        )
    };
    let unproved = incomplete(private(vec![entry, helper()], &[2], &[]));
    assert!(unproved.iter().all(|row| row.machine == id(2)));
    // With a reached actual the same helper is rejected: nothing bounds it.
    let forward = machine(
        vec![u32_type()],
        vec![returns(call(2, vec![slot(0, u32_type())]))],
    );
    no_derivation(private(vec![forward, helper()], &[2], &[]));
}
