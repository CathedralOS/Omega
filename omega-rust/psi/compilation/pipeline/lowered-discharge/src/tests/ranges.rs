// Range propagation through divide, remainder, shift and bitwise results.
// Each test binds `local = a <operation> operand` and then adds a literal to
// it: the add is representable exactly when the local's propagated bound is
// the one the operation's arithmetic gives, and one more is unproved.

use super::*;

/// `local = a <first> operand; return local <second> literal`, over two
/// formals of `scalar_type`.
fn after(
    scalar_type: ScalarType,
    requires: Vec<Proposition>,
    first: Strict,
    operand: Expression,
    second: Strict,
    literal_operand: IntegerValue,
) -> Machine {
    Machine {
        locals: vec![scalar_type],
        requires,
        result: scalar_type,
        ..machine(
            vec![scalar_type, scalar_type],
            vec![
                Statement::Bind {
                    slot: 2,
                    value: strict(first, slot(0, scalar_type), operand),
                },
                returns(strict(
                    second,
                    slot(2, scalar_type),
                    literal(scalar_type, literal_operand),
                )),
            ],
        )
    }
}

const MAX: u128 = u32::MAX as u128;

/// The add after `a <first> operand` is discharged with `local <= upper`
/// exactly: `MAX - upper` fits and one more does not.
fn assert_upper(requires: Vec<Proposition>, first: Strict, operand: Expression, upper: u128) {
    let program = |addend| {
        after(
            u32_type(),
            requires.clone(),
            first,
            operand.clone(),
            Strict::Add,
            IntegerValue::Unsigned(addend),
        )
    };
    let (elaboration, result) = run(vec![program(MAX - upper)]);
    for (obligation, verdict) in verdicts(&elaboration, &result, 1, safety) {
        assert_kernel_accepted(&elaboration, 1, obligation, certificate(verdict));
    }
    let (elaboration, result) = run(vec![program(MAX - upper + 1)]);
    let verdicts = verdicts(&elaboration, &result, 1, safety);
    assert_unproved(verdicts.last().unwrap().1);
}

#[test]
fn a_quotient_by_a_literal_is_bounded_by_the_dividends_bound() {
    // a / 2 over the whole carrier is at most MAX / 2.
    assert_upper(Vec::new(), Strict::Divide, unsigned(2), MAX / 2);
    // a <= 1000 gives a / 7 <= 142.
    assert_upper(vec![at_most(1, 1000)], Strict::Divide, unsigned(7), 142);
}

#[test]
fn a_remainder_by_a_literal_is_below_the_literal_and_no_larger_than_the_dividend() {
    assert_upper(Vec::new(), Strict::Remainder, unsigned(10), 9);
    assert_upper(vec![at_most(1, 4)], Strict::Remainder, unsigned(10), 4);
}

#[test]
fn an_unsigned_remainder_is_below_a_positive_divisor() {
    // 1 <= b <= 10 gives a % b <= 9.
    assert_upper(
        vec![at_least(2, 1), at_most(2, 10)],
        Strict::Remainder,
        slot(1, u32_type()),
        9,
    );
    // b - a % b is safe because a % b < b, a relation with no literal.
    let difference = Machine {
        requires: vec![at_least(2, 1)],
        ..machine(
            vec![u32_type(), u32_type()],
            vec![returns(strict(
                Strict::Subtract,
                slot(1, u32_type()),
                strict(Strict::Remainder, slot(0, u32_type()), slot(1, u32_type())),
            ))],
        )
    };
    let (elaboration, result) = run(vec![difference]);
    for (obligation, verdict) in verdicts(&elaboration, &result, 1, safety) {
        assert_kernel_accepted(&elaboration, 1, obligation, certificate(verdict));
    }
}

/// `local = a % b; return local <second> literal` over two i32 formals with
/// `-7 <= a <= 7` and `divisor`, a clause about `b`.
fn signed_remainder(divisor: Proposition, second: Strict, operand: i128) -> Machine {
    let signed = |value: i128| bound(i32_type(), IntegerValue::Signed(value));
    after(
        i32_type(),
        vec![
            Proposition::LessOrEqual(signed(-7), formal(1, i32_type())),
            Proposition::LessOrEqual(formal(1, i32_type()), signed(7)),
            divisor,
        ],
        Strict::Remainder,
        slot(1, i32_type()),
        second,
        IntegerValue::Signed(operand),
    )
}

#[test]
fn a_remainder_by_a_value_lies_between_zero_and_its_dividend() {
    // a <= 7 and 1 <= b give a % b <= 7; nothing bounds b from above, so
    // `a % b < b` gives no literal.
    assert_upper(
        vec![at_most(1, 7), at_least(2, 1)],
        Strict::Remainder,
        slot(1, u32_type()),
        7,
    );
    // -7 <= a <= 7 gives -7 <= a % b <= 7 for a divisor of either sign.
    let signed = |value: i128| bound(i32_type(), IntegerValue::Signed(value));
    let divisor = formal(2, i32_type());
    let maximum = i128::from(i32::MAX);
    for clause in [
        Proposition::LessOrEqual(signed(2), divisor.clone()),
        Proposition::LessOrEqual(divisor.clone(), signed(-2)),
    ] {
        for (second, fits) in [(Strict::Add, maximum - 7), (Strict::Subtract, maximum - 6)] {
            let (elaboration, result) = run(vec![signed_remainder(clause.clone(), second, fits)]);
            for (obligation, verdict) in verdicts(&elaboration, &result, 1, safety) {
                assert_kernel_accepted(&elaboration, 1, obligation, certificate(verdict));
            }
            let (elaboration, result) =
                run(vec![signed_remainder(clause.clone(), second, fits + 1)]);
            assert_unproved(verdicts(&elaboration, &result, 1, safety).last().unwrap().1);
        }
    }
}

#[test]
fn a_remainder_by_a_value_that_may_be_zero_or_minus_one_gets_no_bound() {
    let signed = |value: i128| bound(i32_type(), IntegerValue::Signed(value));
    let divisor = formal(2, i32_type());
    let fits = i128::from(i32::MAX) - 7;
    for clause in [
        // Admits zero.
        Proposition::LessOrEqual(signed(0), divisor.clone()),
        // Admits minus one.
        Proposition::LessOrEqual(divisor.clone(), signed(-1)),
        // Says nothing about the divisor.
        Proposition::Truth,
    ] {
        let (elaboration, result) = run(vec![signed_remainder(clause, Strict::Add, fits)]);
        assert_unproved(verdicts(&elaboration, &result, 1, safety).last().unwrap().1);
    }
    // Unsigned, with no `1 <= b`.
    let program = after(
        u32_type(),
        vec![at_most(1, 7)],
        Strict::Remainder,
        slot(1, u32_type()),
        Strict::Add,
        IntegerValue::Unsigned(MAX - 7),
    );
    let (elaboration, result) = run(vec![program]);
    assert_unproved(verdicts(&elaboration, &result, 1, safety).last().unwrap().1);
}

#[test]
fn a_mask_bounds_its_result_and_an_or_is_bounded_by_both_operands() {
    assert_upper(Vec::new(), Strict::BitwiseAnd, unsigned(15), 15);
    // a <= 8 gives a | 7 <= 15.
    assert_upper(vec![at_most(1, 8)], Strict::BitwiseOr, unsigned(7), 15);
}

#[test]
fn a_shift_by_a_literal_count_scales_the_bound() {
    assert_upper(Vec::new(), Strict::ShiftRight, unsigned(4), MAX >> 4);
    // a <= 25 gives a << 2 <= 100.
    assert_upper(vec![at_most(1, 25)], Strict::ShiftLeft, unsigned(2), 100);
}

#[test]
fn a_signed_quotient_by_a_negative_literal_reverses_its_bounds() {
    // a in [-10, 20] gives a / -2 in [-10, 5].
    let signed = |value: i128| bound(i32_type(), IntegerValue::Signed(value));
    let requires = || {
        vec![
            Proposition::LessOrEqual(signed(-10), formal(1, i32_type())),
            Proposition::LessOrEqual(formal(1, i32_type()), signed(20)),
        ]
    };
    let program = |second, operand| {
        after(
            i32_type(),
            requires(),
            Strict::Divide,
            literal(i32_type(), IntegerValue::Signed(-2)),
            second,
            IntegerValue::Signed(operand),
        )
    };
    let maximum = i128::from(i32::MAX);
    for (second, fits) in [(Strict::Add, maximum - 5), (Strict::Subtract, maximum - 9)] {
        let (elaboration, result) = run(vec![program(second, fits)]);
        for (obligation, verdict) in verdicts(&elaboration, &result, 1, safety) {
            assert_kernel_accepted(&elaboration, 1, obligation, certificate(verdict));
        }
        let (elaboration, result) = run(vec![program(second, fits + 1)]);
        assert_unproved(verdicts(&elaboration, &result, 1, safety).last().unwrap().1);
    }
}

/// `when <guard> -> <dead>; return <live>` over two u32 formals.
fn branching(requires: Vec<Proposition>, guard: Expression, dead: Expression) -> Machine {
    Machine {
        requires,
        ..machine(
            vec![u32_type(), u32_type()],
            vec![guarded(guard, dead), returns(unsigned(0))],
        )
    }
}

#[test]
fn an_obligation_on_a_branch_its_guard_rules_out_closes_by_contradiction() {
    // requires a >= 10; when a < 5 -> a - 100. Nothing bounds `a` by 100, but
    // no execution reaches the subtraction.
    let dead = || strict(Strict::Subtract, slot(0, u32_type()), unsigned(100));
    let guard = |limit| compare(Strict::Less, slot(0, u32_type()), unsigned(limit));
    let (elaboration, result) = run(vec![branching(vec![at_least(1, 10)], guard(5), dead())]);
    let (obligation, verdict) = verdicts(&elaboration, &result, 1, safety)[0];
    assert_kernel_accepted(&elaboration, 1, obligation, certificate(verdict));
    // `a < 11` leaves `a == 10` reachable, where the subtraction underflows.
    let (elaboration, result) = run(vec![branching(vec![at_least(1, 10)], guard(11), dead())]);
    assert_unproved(verdicts(&elaboration, &result, 1, safety)[0].1);
    // Without the requirement the branch is reachable.
    let (elaboration, result) = run(vec![branching(Vec::new(), guard(5), dead())]);
    assert_unproved(verdicts(&elaboration, &result, 1, safety)[0].1);
}

#[test]
fn an_order_between_two_values_rules_out_the_guard_that_contradicts_it() {
    // requires a <= b; when b < a -> a - 100. Chaining the two orders gives
    // only `b < b`, which no kernel rule calls false without a literal. The
    // guard's own value does it: the arm knows `c == true`, and `c == false`
    // denotes `a <= b`, which is known, so `true == false` follows.
    let dead = || strict(Strict::Subtract, slot(0, u32_type()), unsigned(100));
    let guard = |operation| compare(operation, slot(1, u32_type()), slot(0, u32_type()));
    for (strict_requirement, operation) in [
        (false, Strict::Less),
        // `a < b` proves the negation `a <= b` of `b < a` by weakening.
        (true, Strict::Less),
        (true, Strict::LessOrEqual),
    ] {
        let requires = vec![relate(strict_requirement, 1, 2)];
        let (elaboration, result) = run(vec![branching(requires, guard(operation), dead())]);
        let (obligation, verdict) = verdicts(&elaboration, &result, 1, safety)[0];
        // Every step is a rule the mathematical core derives.
        assert_kernel_checked(&elaboration, 1, obligation, certificate(verdict));
    }
}

#[test]
fn a_guard_that_an_order_between_two_values_admits_leaves_its_arm_owing() {
    let dead = || strict(Strict::Subtract, slot(0, u32_type()), unsigned(100));
    let guard = |operation| compare(operation, slot(1, u32_type()), slot(0, u32_type()));
    for (requires, operation) in [
        // `a <= b` and `b <= a` hold together when the two are equal.
        (vec![relate(false, 1, 2)], Strict::LessOrEqual),
        // An order the other way agrees with `b < a`.
        (vec![relate(false, 2, 1)], Strict::Less),
        // With no requirement nothing contradicts it.
        (Vec::new(), Strict::Less),
    ] {
        let (elaboration, result) = run(vec![branching(requires, guard(operation), dead())]);
        assert_unproved(verdicts(&elaboration, &result, 1, safety)[0].1);
    }
}
