// Contract clauses that compare mathematical integers.
//
//     machine packed(rest: u32, count: u32) -> u32
//         requires embed(rest) + embed(count) * 10 <= 99
//     { rest }
//
// `embed` takes a runtime integer into the unbounded integers
// (`wiki/spec/proofs/contracts.md`, "Mathematical values and
// quantification"), so the sum and the product cannot overflow and the clause
// owes nothing before it is stated. A caller owes it over its own actuals:
// each embedded formal becomes the actual embedded, in the actual's carrier.
//
// What the prover can do with the stated goal is narrower than what a clause
// can say. The kernel evaluates it when every actual is a constant. One
// operation over values of one carrier is bounded from their intervals, the
// rule an exact operation's representability goal already uses. A deeper
// term over values that are not constants has no rule, and the route says so
// instead of calling the program wrong.

use semantic_vocabulary::{IntegerMathLiteral, IntegerMathTerm};

use super::*;

/// Formal or value `position` of a `u32` machine, embedded.
fn embedded(position: u64) -> IntegerMathTerm {
    IntegerMathTerm::MathValue {
        source_type: integer_type(IntegerSign::Unsigned),
        value: id(position),
    }
}

fn integer(value: u128) -> IntegerMathTerm {
    IntegerMathTerm::IntegerLiteral(IntegerMathLiteral::new(false, value).unwrap())
}

fn sum(left: IntegerMathTerm, right: IntegerMathTerm) -> IntegerMathTerm {
    IntegerMathTerm::Add(Box::new(left), Box::new(right))
}

fn within(term: IntegerMathTerm, limit: u128) -> Proposition {
    Proposition::IntegerMathLessOrEqual(term, integer(limit))
}

/// `embed(rest) + embed(count) * 10 <= 99` over formals 1 and 2.
fn packed_clause() -> Proposition {
    within(
        sum(
            embedded(1),
            IntegerMathTerm::Multiply(Box::new(embedded(2)), Box::new(integer(10))),
        ),
        99,
    )
}

/// Machine 1 calls machine 2 with `rest` and `count`; machine 2 requires
/// `clause` of its two formals.
fn calling(
    requires: Vec<Proposition>,
    clause: Proposition,
    rest: Expression,
    count: Expression,
) -> Vec<Machine> {
    let both = || vec![u32_type(), u32_type()];
    let callee = Machine {
        requires: vec![clause],
        ..machine(both(), vec![returns(slot(0, u32_type()))])
    };
    let caller = Machine {
        requires,
        ..machine(both(), vec![returns(call(2, vec![rest, count]))])
    };
    vec![caller, callee]
}

fn formals() -> (Expression, Expression) {
    (slot(0, u32_type()), slot(1, u32_type()))
}

/// The one call requirement of machine 1.
fn requirement_of(machines: Vec<Machine>) -> (structure_elaboration::Elaboration, Discharge) {
    let (elaboration, result) = run(machines);
    assert_eq!(verdicts(&elaboration, &result, 1, requirement).len(), 1);
    (elaboration, result)
}

fn cause_of(verdict: &Checked<Discharged>) -> UnprovedCause {
    match verdict
        .as_ref()
        .expect_err("the requirement must be refused")
        .reason
    {
        RefusalReason::Unproved(cause) => cause,
        reason => panic!("an unproved requirement, found {reason:?}"),
    }
}

#[test]
fn a_mathematical_requirement_over_constants_is_evaluated() {
    // Shows: `packed(9, 9)` owes `embed(a) + embed(b) * 10 <= 99` over the two
    // constants' values. Each is fixed to 9 by its result equation, so the
    // kernel evaluates 9 + 9 * 10 = 99 and accepts. The goal still names the
    // caller's values embedded in their carrier, never a carrier sum.
    let (elaboration, result) = requirement_of(calling(
        Vec::new(),
        packed_clause(),
        unsigned(9),
        unsigned(9),
    ));
    let (obligation, verdict) = verdicts(&elaboration, &result, 1, requirement)[0];
    let evidence = certificate(verdict);
    let Proposition::IntegerMathLessOrEqual(IntegerMathTerm::Add(rest, tens), limit) =
        &evidence.goal
    else {
        panic!("a mathematical order: {:?}", evidence.goal);
    };
    assert!(matches!(rest.as_ref(), IntegerMathTerm::MathValue { .. }));
    assert!(matches!(tens.as_ref(), IntegerMathTerm::Multiply(..)));
    assert_eq!(limit, &integer(99));
    assert_kernel_accepted(&elaboration, 1, obligation, evidence);
}

#[test]
fn a_mathematical_requirement_broken_by_a_constant_is_rejected() {
    // Shows: `packed(9, 10)` is 9 + 10 * 10 = 109. The goal is closed once
    // the constants are read, the kernel evaluates it as false, and the
    // failure is the program's: `NoDerivation`, which rejects, and names the
    // call's requirement.
    let machines = || calling(Vec::new(), packed_clause(), unsigned(9), unsigned(10));
    let (elaboration, result) = requirement_of(machines());
    let verdict = verdicts(&elaboration, &result, 1, requirement)[0].1;
    assert_eq!(cause_of(verdict), UnprovedCause::NoDerivation);
    let unproved = rejected(machines());
    assert_eq!(unproved.len(), 1);
    assert!(requirement(&unproved[0].kind));
    assert_eq!(unproved[0].cause, UnprovedCause::NoDerivation);
}

#[test]
fn a_mathematical_sum_of_two_values_is_bounded_from_their_ranges() {
    // Shows: `requires embed(a) + embed(b) <= 300` at a call whose actuals
    // are the caller's formals, known only by `a <= 100` and `b <= 200`. One
    // operation over two values of one carrier is the shape the interval rule
    // serves, so 100 + 200 <= 300 is proved and the kernel re-decides it.
    // With `b <= 201` the sum may be 301 and the call is rejected.
    let clause = || within(sum(embedded(1), embedded(2)), 300);
    let (rest, count) = formals();
    let (elaboration, result) = requirement_of(calling(
        vec![at_most(1, 100), at_most(2, 200)],
        clause(),
        rest,
        count,
    ));
    let (obligation, verdict) = verdicts(&elaboration, &result, 1, requirement)[0];
    assert_kernel_checked(&elaboration, 1, obligation, certificate(verdict));
    let (rest, count) = formals();
    let (elaboration, result) = requirement_of(calling(
        vec![at_most(1, 100), at_most(2, 201)],
        clause(),
        rest,
        count,
    ));
    let verdict = verdicts(&elaboration, &result, 1, requirement)[0].1;
    assert_eq!(cause_of(verdict), UnprovedCause::NoDerivation);
}

#[test]
fn a_caller_that_requires_the_same_clause_passes_it_on() {
    // Shows: an entry `requires` that compares mathematical integers is a
    // hypothesis of the body like any other. A machine that requires the
    // clause of its own formals and forwards them owes exactly what it
    // assumed.
    let (rest, count) = formals();
    let (elaboration, result) =
        requirement_of(calling(vec![packed_clause()], packed_clause(), rest, count));
    let (obligation, verdict) = verdicts(&elaboration, &result, 1, requirement)[0];
    let evidence = certificate(verdict);
    assert_eq!(evidence.goal, packed_clause());
    assert_eq!(evidence.assumptions.len(), 1);
    assert_kernel_accepted(&elaboration, 1, obligation, evidence);
}

#[test]
fn a_deeper_term_over_open_values_has_no_rule_and_is_not_a_rejection() {
    // Shows the limit: with `a <= 9` and `b <= 9` the clause holds (at most
    // 99), but bounding a sum whose operand is a product is more than one
    // operation, and the prover has no rule for it. The route does not
    // accept, and does not call the program wrong either:
    // `NoRuleForGoalForm`, published `Incomplete`.
    let machines = || {
        let (rest, count) = formals();
        calling(
            vec![at_most(1, 9), at_most(2, 9)],
            packed_clause(),
            rest,
            count,
        )
    };
    let (elaboration, result) = requirement_of(machines());
    let verdict = verdicts(&elaboration, &result, 1, requirement)[0].1;
    assert_eq!(cause_of(verdict), UnprovedCause::NoRuleForGoalForm);
    match judge(lower(machines())).unwrap() {
        RouteVerdict::Incomplete(unproved) => {
            assert_eq!(unproved.len(), 1);
            assert!(requirement(&unproved[0].kind));
        }
        verdict => panic!("neither accepted nor rejected: {verdict:?}"),
    }
}

#[test]
fn a_mathematical_guarantee_is_proved_at_the_return_and_known_after_a_call() {
    // Shows: `ensures embed(result) + 1 <= 11` on `{ x }` under `requires
    // x <= 10`. At the return the embedded `result` is the returned value
    // embedded, and x + 1 <= 11 follows from x <= 10. A caller that promises
    // the same of its own result gets it from the call: the callee's clause
    // over the call's result is what it owes. Without `x <= 10` the callee's
    // guarantee is unproved.
    let promise = || within(sum(embedded(2), integer(1)), 11);
    let callee = |requires| Machine {
        requires,
        ensures: vec![promise()],
        ..machine(vec![u32_type()], vec![returns(slot(0, u32_type()))])
    };
    let caller = Machine {
        requires: vec![at_most(1, 10)],
        ensures: vec![promise()],
        ..machine(
            vec![u32_type()],
            vec![returns(call(2, vec![slot(0, u32_type())]))],
        )
    };
    let (elaboration, result) = run(vec![caller, callee(vec![at_most(1, 10)])]);
    for machine in [1, 2] {
        let proved = verdicts(&elaboration, &result, machine, guarantee);
        assert_eq!(proved.len(), 1);
        let evidence = certificate(proved[0].1);
        // The callee returns its formal, value 1, where `result` stood.
        if machine == 2 {
            assert_eq!(evidence.goal, within(sum(embedded(1), integer(1)), 11));
        }
        assert_kernel_accepted(&elaboration, machine, proved[0].0, evidence);
    }
    let (elaboration, result) = run(vec![callee(Vec::new())]);
    assert_unproved(verdicts(&elaboration, &result, 1, guarantee)[0].1);
}

#[test]
fn rule_coverage_of_a_mathematical_goal_is_read_from_its_form() {
    // Shows which mathematical goals a failure counts against the program
    // for. A comparison of two closed terms, whatever its relation, is
    // evaluated. An order of one operation over values of one carrier
    // against a literal is bounded. A deeper term, operands of two carriers,
    // and an equality or strict order over values are not.
    let closed = sum(integer(9), integer(90));
    assert!(crate::has_rules_for(&within(closed.clone(), 99)));
    assert!(crate::has_rules_for(&Proposition::IntegerMathEqual(
        closed.clone(),
        integer(99)
    )));
    assert!(crate::has_rules_for(&Proposition::IntegerMathLessThan(
        closed,
        integer(100)
    )));
    assert!(crate::has_rules_for(&within(
        sum(embedded(1), embedded(2)),
        300
    )));
    assert!(crate::has_rules_for(&within(
        sum(embedded(1), integer(1)),
        11
    )));
    assert!(!crate::has_rules_for(&packed_clause()));
    let narrow = IntegerMathTerm::MathValue {
        source_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        value: id(2),
    };
    assert!(!crate::has_rules_for(&within(
        sum(embedded(1), narrow),
        300
    )));
    assert!(!crate::has_rules_for(&Proposition::IntegerMathEqual(
        embedded(1),
        integer(7)
    )));
    assert!(!crate::has_rules_for(&Proposition::IntegerMathLessThan(
        embedded(1),
        integer(7)
    )));
}
