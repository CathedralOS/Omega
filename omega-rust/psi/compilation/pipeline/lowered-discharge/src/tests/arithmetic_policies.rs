// Integers whose type carries the `Wrapping` or `Saturating` policy.
//
//     machine mix(hash: u32 in Wrapping, byte: u32 in Wrapping) -> u32 in Wrapping {
//         hash * byte + byte
//     }
//
// The policy chooses the operation: an add, subtract or multiply under either
// policy is total and owes nothing, a divide or remainder still owes its
// divisor, and what the operation returns is the wrapped or clamped value,
// which is not the mathematical result. Each obligation and each result
// equation here comes from Terminal semantics for the operation the lowering
// named; these tests pin that a policy operation is never judged as the Exact
// one, in either direction.

use super::mutable_formals::unsafe_operations;
use super::references::{Lending, accepted, lower_lending, named, unit};
use super::*;
use structure_elaboration::Policy;
use terminal_psi::OperationKind;

fn qualified(policy: Policy) -> ValueType {
    ValueType::Qualified(u32_type(), policy)
}

fn plain() -> ValueType {
    u32_type().into()
}

fn of(kind: ExpressionKind, result: ValueType) -> Expression {
    Expression {
        kind,
        result: Some(result),
        source: nowhere(),
    }
}

/// `left <operation> right`, whose result has type `result`.
fn operate(
    operation: Strict,
    left: Expression,
    right: Expression,
    result: ValueType,
) -> Expression {
    of(
        ExpressionKind::Strict {
            operation,
            left: Box::new(left),
            right: Box::new(right),
        },
        result,
    )
}

/// `operand as <target>`, a cast that changes only the policy.
fn requalified(operand: Expression, target: ValueType) -> Expression {
    of(ExpressionKind::Requalify(Box::new(operand)), target)
}

/// `f(a: T, b: T) { a <operation> b; }` with both formals and the result of
/// type `operand`.
fn binary(operation: Strict, operand: ValueType, requires: Vec<Proposition>) -> Lending {
    Lending {
        requires,
        ..unit(
            vec![operand, operand],
            vec![Statement::Evaluate(operate(
                operation,
                named(0, operand),
                named(1, operand),
                operand,
            ))],
        )
    }
}

fn operations(elaboration: &structure_elaboration::Elaboration) -> Vec<&OperationKind> {
    elaboration
        .module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .map(|operation| &operation.kind)
        .collect()
}

#[test]
fn an_add_under_a_policy_is_total_and_the_exact_add_is_not() {
    for (policy, operation) in [
        (Policy::Wrapping, Strict::Add),
        (Policy::Wrapping, Strict::Subtract),
        (Policy::Wrapping, Strict::Multiply),
        (Policy::Saturating, Strict::Add),
        (Policy::Saturating, Strict::Subtract),
        (Policy::Saturating, Strict::Multiply),
    ] {
        let closure = accepted(vec![binary(operation, qualified(policy), Vec::new())]);
        let kinds = operations(&closure.elaboration);
        assert_eq!(kinds.len(), 1, "{policy:?} {operation:?}");
        assert!(
            matches!(
                (policy, kinds[0]),
                (
                    Policy::Wrapping,
                    OperationKind::WrappingIntegerAdd { .. }
                        | OperationKind::WrappingIntegerSubtract { .. }
                        | OperationKind::WrappingIntegerMultiply { .. }
                ) | (
                    Policy::Saturating,
                    OperationKind::SaturatingIntegerAdd { .. }
                        | OperationKind::SaturatingIntegerSubtract { .. }
                        | OperationKind::SaturatingIntegerMultiply { .. }
                )
            ),
            "{policy:?} {operation:?}: {:?}",
            kinds[0]
        );
        // The same operands under no policy: the Exact operation, which
        // owes that its result fits and has nothing to prove it from.
        let exact = binary(operation, plain(), Vec::new());
        assert_eq!(
            unsafe_operations(judge(lower_lending(vec![exact])).unwrap()),
            1,
            "{operation:?}"
        );
    }
}

#[test]
fn a_policy_does_not_supply_a_divisor() {
    // "An overflow policy does not invent a result for undefined operands
    // such as an integer zero divisor": `a / b` and `a % b` owe `b != 0`
    // under either policy, and `requires b >= 1` pays it.
    for policy in [Policy::Wrapping, Policy::Saturating] {
        for operation in [Strict::Divide, Strict::Remainder] {
            let unbounded = binary(operation, qualified(policy), Vec::new());
            assert_eq!(
                unsafe_operations(judge(lower_lending(vec![unbounded])).unwrap()),
                1,
                "{policy:?} {operation:?}"
            );
            accepted(vec![binary(
                operation,
                qualified(policy),
                vec![at_least(2, 1)],
            )]);
        }
    }
}

/// `f(a: u32, b: u32) requires a >= 2^31, b >= 2^31 { 1 / <sum>; }`, where
/// `<sum>` is `a + b` under `policy` cast back to `u32`.
fn divided_by_a_sum(policy: Policy) -> Lending {
    let half = 1u128 << 31;
    let sum = requalified(
        operate(
            Strict::Add,
            requalified(named(0, plain()), qualified(policy)),
            requalified(named(1, plain()), qualified(policy)),
            qualified(policy),
        ),
        plain(),
    );
    Lending {
        requires: vec![at_least(1, half), at_least(2, half)],
        ..unit(
            vec![plain(), plain()],
            vec![Statement::Evaluate(operate(
                Strict::Divide,
                unsigned(1),
                sum,
                plain(),
            ))],
        )
    }
}

#[test]
fn a_wrapped_sum_is_not_the_mathematical_sum() {
    // With both operands at least 2^31 the mathematical sum is at least
    // 2^32 and so nonzero, but the wrapped sum is `a + b - 2^32`, which is
    // zero for `a = b = 2^31`. The division by it must not be discharged: a
    // checker that read the Wrapping add as the Exact one would accept a
    // division by zero.
    //
    // The verdict is `Incomplete`, not `Rejected`. The goal is that the
    // divisor is nonzero, and the divisor is defined only by a Wrapping
    // add, an operation the integer rules have no bound for
    // (`prove::has_rules_for`). A goal that depends on such a value is one
    // the route cannot decide either way, so it reports a missing verdict
    // and not a defect in the program.
    let wrapped = divided_by_a_sum(Policy::Wrapping);
    let undecided = match judge(lower_lending(vec![wrapped])).unwrap() {
        RouteVerdict::Incomplete(undecided) => undecided,
        verdict => panic!("the division must be left undecided: {verdict:?}"),
    };
    assert!(
        matches!(
            undecided.as_slice(),
            [UnprovedObligation {
                kind: ObligationKind::OperationSafety { .. },
                cause: UnprovedCause::NoRuleForGoalForm,
                ..
            }]
        ),
        "{undecided:?}"
    );
    // The casts compute nothing: the operations are the literal, the add
    // and the divide.
    let elaboration = lower_lending(vec![divided_by_a_sum(Policy::Wrapping)]).unwrap();
    let kinds = operations(&elaboration);
    assert!(
        matches!(
            kinds.as_slice(),
            [
                OperationKind::IntegerConstant { .. },
                OperationKind::WrappingIntegerAdd { .. },
                OperationKind::ExactIntegerDivide { .. }
            ]
        ),
        "{kinds:?}"
    );
}

#[test]
fn a_saturating_shift_is_refused_by_name() {
    // A Saturating shift "still owes the Exact count proof" under an
    // operation the lowering does not emit; a Wrapping shift reduces its
    // count and is total.
    let shift = |policy| {
        unit(
            vec![qualified(policy), plain()],
            vec![Statement::Evaluate(operate(
                Strict::ShiftLeft,
                named(0, qualified(policy)),
                named(1, plain()),
                qualified(policy),
            ))],
        )
    };
    let closure = accepted(vec![shift(Policy::Wrapping)]);
    assert!(matches!(
        operations(&closure.elaboration).as_slice(),
        [OperationKind::WrappingIntegerShiftLeft { .. }]
    ));
    let refusal = lower_lending(vec![shift(Policy::Saturating)]).unwrap_err();
    assert_eq!(refusal.reason, RefusalReason::Unsupported);
    assert_eq!(refusal.detail, "expression saturating shift");
}
