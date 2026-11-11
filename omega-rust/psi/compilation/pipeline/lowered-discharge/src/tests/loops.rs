// Call cycles lowered as loops: a head's invariant is proved on every
// arrival and is all that is known about the head's parameters.

use super::*;

fn compare(operation: Strict, left: Expression, right: Expression) -> Expression {
    expression(
        ExpressionKind::Strict {
            operation,
            left: Box::new(left),
            right: Box::new(right),
        },
        Some(ScalarType::Boolean),
    )
}

fn positive(slot_index: usize) -> Expression {
    compare(Strict::Greater, slot(slot_index, u32_type()), unsigned(0))
}

fn arrival(kind: &ObligationKind) -> bool {
    matches!(kind, ObligationKind::ArrivalInvariant { .. })
}

/// `machine countdown(remaining: u32) -> u32 <requires>
///  { remaining > 0 -> countdown(<next>); 0 }`
fn countdown(requires: Vec<Proposition>, next: Expression) -> Machine {
    Machine {
        requires,
        ..machine(
            vec![u32_type()],
            vec![
                guarded(positive(0), call(1, vec![next])),
                returns(unsigned(0)),
            ],
        )
    }
}

fn less_one() -> Expression {
    strict(Strict::Subtract, slot(0, u32_type()), unsigned(1))
}

#[test]
fn a_loop_body_is_proved_from_what_holds_on_its_own_path() {
    // `remaining - 1` is safe under the arm's guard, which is over the
    // head's parameter: the fact is this iteration's own.
    let (elaboration, result) = run(vec![countdown(Vec::new(), less_one())]);
    let [(obligation, Ok(Discharged::Certificate(evidence)))] =
        verdicts(&elaboration, &result, 1, safety)[..]
    else {
        panic!("the subtraction is discharged");
    };
    assert_kernel_accepted(&elaboration, 1, obligation, evidence);
    assert!(matches!(
        judge(lower(vec![countdown(Vec::new(), less_one())])).unwrap(),
        RouteVerdict::Accepted(_)
    ));
}

#[test]
fn a_heads_invariant_is_proved_on_the_entry_and_on_the_jump_back() {
    // requires remaining <= 9: the entry proves it from the machine's own
    // `requires`, the jump back from the invariant of the iteration it
    // leaves and the subtraction's result.
    let program = || vec![countdown(vec![at_most(1, 9)], less_one())];
    let (elaboration, result) = run(program());
    let arrivals = verdicts(&elaboration, &result, 1, arrival);
    assert_eq!(arrivals.len(), 2);
    for (obligation, verdict) in arrivals {
        let Ok(Discharged::Certificate(evidence)) = verdict else {
            panic!("the arrival is discharged: {verdict:?}");
        };
        assert_kernel_accepted(&elaboration, 1, obligation, evidence);
    }
    // The invariant is one knowledge row, over the head's parameter and
    // available from the head's entry.
    let tables = elaboration.tables.machines.get(id(1)).unwrap();
    let rows: Vec<_> = tables
        .knowledge
        .iter()
        .filter(|(_, row)| matches!(row.justification, Justification::BlockInvariant { .. }))
        .collect();
    let [(_, row)] = rows.as_slice() else {
        panic!("one invariant row");
    };
    assert_eq!(row.available_from, ProgramPoint::BlockEntry(id(2)));
    assert_eq!(
        row.justification,
        Justification::BlockInvariant { block: id(2) }
    );
    assert!(matches!(
        judge(lower(program())).unwrap(),
        RouteVerdict::Accepted(_)
    ));
}

#[test]
fn a_jump_back_that_does_not_keep_the_invariant_rejects_the_program() {
    // requires remaining <= 9, and the loop calls itself with
    // `remaining + 1`: from `remaining <= 9` the argument is only `<= 10`.
    let grows = strict(Strict::Add, slot(0, u32_type()), unsigned(1));
    let unproved = rejected(vec![countdown(vec![at_most(1, 9)], grows)]);
    let [refused] = unproved.as_slice() else {
        panic!("one unproved obligation: {unproved:?}");
    };
    assert!(matches!(
        refused.kind,
        ObligationKind::ArrivalInvariant { .. }
    ));
    assert_eq!(refused.cause, UnprovedCause::NoDerivation);
}

/// `machine left(n: u32) -> u32 requires n <= 9 { right(n) }`
/// `machine right(m: u32) -> u32 <requires> { m > 100 -> left(0); m + 4294967290 }`
fn mutual(right_requires: Vec<Proposition>) -> Vec<Machine> {
    vec![
        Machine {
            requires: vec![at_most(1, 9)],
            ..machine(
                vec![u32_type()],
                vec![returns(call(2, vec![slot(0, u32_type())]))],
            )
        },
        Machine {
            requires: right_requires,
            ..machine(
                vec![u32_type()],
                vec![
                    guarded(
                        compare(Strict::Greater, slot(0, u32_type()), unsigned(100)),
                        call(1, vec![unsigned(0)]),
                    ),
                    returns(strict(
                        Strict::Add,
                        slot(0, u32_type()),
                        unsigned(4_294_967_290),
                    )),
                ],
            )
        },
    ]
}

#[test]
fn what_a_caller_knows_about_an_argument_is_not_known_about_the_parameter() {
    // `left` passes `n <= 9` to `right`, where `m + 4294967290` needs
    // `m <= 5`. `right` states no `requires`, so its head has no invariant
    // and the addition is unproved in both machines' graphs, though every
    // arrival from `left` would make it safe.
    for machine in [1, 2] {
        let (elaboration, result) = run(mutual(Vec::new()));
        let [(_, verdict)] = verdicts(&elaboration, &result, machine, safety)[..] else {
            panic!("one partial operation");
        };
        assert_unproved(verdict);
    }
    // With `requires m <= 5` the addition is safe from the head's
    // invariant, and the arrival from `left` cannot prove it from `n <= 9`.
    let (elaboration, result) = run(mutual(vec![at_most(1, 5)]));
    for machine in [1, 2] {
        let [(_, Ok(_))] = verdicts(&elaboration, &result, machine, safety)[..] else {
            panic!("the addition is discharged under the invariant");
        };
    }
    let from_left: Vec<_> = verdicts(&elaboration, &result, 1, arrival);
    assert!(
        from_left.iter().any(|(_, verdict)| verdict.is_err()),
        "the arrival at `right` from `left` is unproved"
    );
}

#[test]
fn an_invariant_row_that_misses_an_arrival_refuses_the_machine() {
    let mut elaboration = lower(vec![countdown(vec![at_most(1, 9)], less_one())]).unwrap();
    elaboration.module.scalar_block_invariants[0].arrivals.pop();
    let refusal = discharge(&mut elaboration).unwrap_err();
    assert_eq!(refusal.reason, RefusalReason::ForeignIdentity);
}
