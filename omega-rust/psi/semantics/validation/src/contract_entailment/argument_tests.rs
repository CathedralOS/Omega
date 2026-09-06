//! Simultaneous mathematical argument substitution, independent of source admission.

use super::*;
use arena::HandleSpan;
use numerics::literals::IntegerLiteral;
use symbols::SymbolHandle;
use typed_trees::expression::{TableBinaryExpression, TableNamePath};
use typed_trees::name::Identifier;

struct Fixture {
    program: TypedTrees,
    caller: SymbolHandle,
    formal: SymbolHandle,
    second: SymbolHandle,
    caller_expression: ExpressionHandle,
    formal_expression: ExpressionHandle,
    second_expression: ExpressionHandle,
}

impl Fixture {
    fn new() -> Self {
        let mut program = TypedTrees::default();
        let caller = SymbolHandle::from_arena_index(101);
        let formal = SymbolHandle::from_arena_index(102);
        let second = SymbolHandle::from_arena_index(103);
        let mut name = |symbol| {
            let mut members = HandleSpan::empty();
            program
                .expression_table
                .push_name_path_member(&mut members, Identifier::generated_static("value"));
            program
                .expression_table
                .insert(ExpressionNode::Name(TableNamePath {
                    members,
                    head_symbol: symbol,
                    symbol,
                    ..Default::default()
                }))
        };
        let caller_expression = name(caller);
        let formal_expression = name(formal);
        let second_expression = name(second);
        Self {
            program,
            caller,
            formal,
            second,
            caller_expression,
            formal_expression,
            second_expression,
        }
    }

    fn integer(&mut self, value: i64) -> ExpressionHandle {
        self.program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(value)))
    }

    fn binary(
        &mut self,
        left: ExpressionHandle,
        operator: BinaryOperator,
        right: ExpressionHandle,
    ) -> ExpressionHandle {
        self.program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left,
                operator,
                right,
            }))
    }

    fn judge(
        &self,
        hypotheses: &[ExpressionHandle],
        goal: ExpressionHandle,
        arguments: &[StrictArithmeticExpressionBinding],
    ) -> StrictArithmeticImplicationJudgment {
        strict_arithmetic_expression_implication_with_arguments(
            &self.program,
            &Machine::default(),
            hypotheses,
            goal,
            &[StrictArithmeticSymbolBinding {
                symbol: self.caller,
                value: StrictArithmeticBindingValue::Atom {
                    identity: "caller-value".to_owned(),
                    unsigned: false,
                },
            }],
            arguments,
        )
    }
}

fn argument(
    symbol: SymbolHandle,
    expression: ExpressionHandle,
) -> StrictArithmeticExpressionBinding {
    StrictArithmeticExpressionBinding { symbol, expression }
}

#[test]
fn arithmetic_arguments_transport_nontrivial_lower_and_upper_bounds() {
    for (operator, operand, lower, upper) in [
        (BinaryOperator::Add, 3, 5, 7),
        (BinaryOperator::Subtract, 1, 1, 3),
        (BinaryOperator::Multiply, 3, 6, 12),
    ] {
        let mut fixture = Fixture::new();
        let two = fixture.integer(2);
        let four = fixture.integer(4);
        let minimum = fixture.binary(two, BinaryOperator::LessOrEqual, fixture.caller_expression);
        let maximum = fixture.binary(fixture.caller_expression, BinaryOperator::LessOrEqual, four);
        let operand = fixture.integer(operand);
        let actual = fixture.binary(fixture.caller_expression, operator, operand);
        let lower = fixture.integer(lower);
        let upper = fixture.integer(upper);
        let lower_goal = fixture.binary(
            lower,
            BinaryOperator::LessOrEqual,
            fixture.formal_expression,
        );
        let upper_goal = fixture.binary(
            fixture.formal_expression,
            BinaryOperator::LessOrEqual,
            upper,
        );
        for goal in [lower_goal, upper_goal] {
            assert_eq!(
                fixture.judge(
                    &[minimum, maximum],
                    goal,
                    &[argument(fixture.formal, actual)]
                ),
                StrictArithmeticImplicationJudgment::Proven,
                "{operator:?}"
            );
        }
    }
}

#[test]
fn argument_order_cannot_resolve_another_callee_formal() {
    let mut fixture = Fixture::new();
    let zero = fixture.integer(0);
    let one = fixture.integer(1);
    let goal = fixture.binary(one, BinaryOperator::Equal, one);
    let contradiction = fixture.binary(zero, BinaryOperator::Equal, one);
    let first = argument(fixture.formal, fixture.caller_expression);
    let second = argument(fixture.second, fixture.formal_expression);
    for arguments in [[first, second], [second, first]] {
        for hypotheses in [&[][..], &[contradiction][..]] {
            assert_eq!(
                fixture.judge(hypotheses, goal, &arguments),
                StrictArithmeticImplicationJudgment::Unknown,
                "unbound actuals reject before a constant goal or contradictory hypotheses"
            );
        }
    }
}

#[test]
fn reused_formals_accept_equal_polynomials_and_reject_conflicting_values() {
    let mut fixture = Fixture::new();
    let zero = fixture.integer(0);
    let one = fixture.integer(1);
    let unchanged = fixture.binary(fixture.caller_expression, BinaryOperator::Add, zero);
    let goal = fixture.binary(
        fixture.formal_expression,
        BinaryOperator::Equal,
        fixture.caller_expression,
    );
    assert_eq!(
        fixture.judge(
            &[],
            goal,
            &[
                argument(fixture.formal, unchanged),
                argument(fixture.formal, fixture.caller_expression),
            ]
        ),
        StrictArithmeticImplicationJudgment::Proven
    );
    let constant_goal = fixture.binary(one, BinaryOperator::Equal, one);
    let contradiction = fixture.binary(zero, BinaryOperator::Equal, one);
    let original = argument(fixture.formal, fixture.caller_expression);
    let conflicting = argument(fixture.formal, one);
    for arguments in [[original, conflicting], [conflicting, original]] {
        for hypotheses in [&[][..], &[contradiction][..]] {
            assert_eq!(
                fixture.judge(hypotheses, constant_goal, &arguments),
                StrictArithmeticImplicationJudgment::Unknown
            );
        }
    }
}

#[test]
fn original_caller_symbols_cannot_be_rebound_to_different_arguments() {
    let mut fixture = Fixture::new();
    let zero = fixture.integer(0);
    let one = fixture.integer(1);
    let unchanged = fixture.binary(fixture.caller_expression, BinaryOperator::Add, zero);
    let changed = fixture.binary(fixture.caller_expression, BinaryOperator::Add, one);
    let goal = fixture.binary(
        fixture.second_expression,
        BinaryOperator::Equal,
        fixture.caller_expression,
    );
    for actual in [fixture.caller_expression, unchanged] {
        assert_eq!(
            fixture.judge(
                &[],
                goal,
                &[
                    argument(fixture.caller, actual),
                    argument(fixture.second, fixture.caller_expression),
                ]
            ),
            StrictArithmeticImplicationJudgment::Proven
        );
    }
    let constant_goal = fixture.binary(one, BinaryOperator::Equal, one);
    assert_eq!(
        fixture.judge(&[], constant_goal, &[argument(fixture.caller, changed)]),
        StrictArithmeticImplicationJudgment::Unknown
    );
}

#[test]
fn unbound_same_spelled_and_unsupported_arguments_do_not_prove_constant_goals() {
    let mut fixture = Fixture::new();
    assert_ne!(fixture.caller, fixture.formal);
    let one = fixture.integer(1);
    let zero = fixture.integer(0);
    let goal = fixture.binary(one, BinaryOperator::Equal, one);
    let contradiction = fixture.binary(zero, BinaryOperator::Equal, one);
    let boolean = fixture
        .program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    for invalid in [
        argument(fixture.second, fixture.formal_expression),
        argument(fixture.second, ExpressionHandle::invalid()),
        argument(fixture.second, boolean),
        argument(SymbolHandle::invalid(), fixture.caller_expression),
    ] {
        for hypotheses in [&[][..], &[contradiction][..]] {
            assert_eq!(
                fixture.judge(hypotheses, goal, &[invalid]),
                StrictArithmeticImplicationJudgment::Unknown
            );
        }
    }
}

#[test]
fn nested_missing_and_stale_arguments_cannot_become_dummy_zero() {
    let mut fixture = Fixture::new();
    let zero = fixture.integer(0);
    let one = fixture.integer(1);
    let goal = fixture.binary(one, BinaryOperator::Equal, one);
    let contradiction = fixture.binary(zero, BinaryOperator::Equal, one);
    let stale = ExpressionHandle::from_parts(zero.arena_index(), zero.generation() + 1);
    let missing = ExpressionHandle::from_arena_index(u32::MAX);
    assert!(fixture.program.expression_table.expression_is_valid(zero));
    for unknown in [ExpressionHandle::invalid(), stale, missing] {
        assert!(
            !fixture
                .program
                .expression_table
                .expression_is_valid(unknown)
        );
        let addition = fixture.binary(fixture.caller_expression, BinaryOperator::Add, unknown);
        let multiplication = fixture.binary(unknown, BinaryOperator::Multiply, zero);
        let nested = fixture.binary(one, BinaryOperator::Add, multiplication);
        for actual in [unknown, addition, multiplication, nested] {
            for hypotheses in [&[][..], &[contradiction][..]] {
                assert_eq!(
                    fixture.judge(hypotheses, goal, &[argument(fixture.formal, actual)]),
                    StrictArithmeticImplicationJudgment::Unknown,
                    "a zero multiplier does not legitimize an invalid operand"
                );
            }
        }
    }
}

#[test]
fn original_strict_api_rejects_missing_and_stale_comparison_operands() {
    let mut fixture = Fixture::new();
    let zero = fixture.integer(0);
    let true_goal = fixture.binary(zero, BinaryOperator::Equal, zero);
    let stale = ExpressionHandle::from_parts(zero.arena_index(), zero.generation() + 1);
    for unknown in [ExpressionHandle::invalid(), stale] {
        let invalid_comparison = fixture.binary(unknown, BinaryOperator::Equal, zero);
        for goal in [unknown, invalid_comparison] {
            assert_eq!(
                strict_arithmetic_expression_implication(
                    &fixture.program,
                    &Machine::default(),
                    &[],
                    goal,
                    &[],
                ),
                StrictArithmeticImplicationJudgment::Unknown
            );
            assert_eq!(
                strict_arithmetic_expression_implication(
                    &fixture.program,
                    &Machine::default(),
                    &[goal],
                    true_goal,
                    &[],
                ),
                StrictArithmeticImplicationJudgment::Unknown
            );
        }
    }
}

#[test]
fn arithmetic_arguments_do_not_invent_missing_or_insufficient_bounds() {
    let mut fixture = Fixture::new();
    let one = fixture.integer(1);
    let three = fixture.integer(3);
    let five = fixture.integer(5);
    let actual = fixture.binary(fixture.caller_expression, BinaryOperator::Add, three);
    let goal = fixture.binary(five, BinaryOperator::LessOrEqual, fixture.formal_expression);
    let too_small = fixture.binary(fixture.caller_expression, BinaryOperator::LessOrEqual, one);
    let arguments = [argument(fixture.formal, actual)];
    assert_eq!(
        fixture.judge(&[], goal, &arguments),
        StrictArithmeticImplicationJudgment::Unknown
    );
    assert_eq!(
        fixture.judge(&[too_small], goal, &arguments),
        StrictArithmeticImplicationJudgment::Refuted
    );
}
