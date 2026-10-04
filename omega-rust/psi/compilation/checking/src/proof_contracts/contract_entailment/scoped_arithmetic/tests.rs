//! Per-proposition rosters: one symbol names both an invocation formal and its
//! current header binding, and the implication reads each under its own atoms.
use super::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticHypothesis, ScopedArithmeticValue, StrictArithmeticImplicationJudgment,
    scoped_arithmetic_implication,
};
use arena::HandleSpan;
use numerics::literals::IntegerLiteral;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, TableNamePath,
};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;

mod domain_self;
mod embeddings;
mod projections;

mod live_comparisons {
    use super::super::{ScopedArithmeticComparison, scoped_arithmetic_comparisons_unsatisfiable};
    use super::*;
    use numerics::bignum::BigInt;

    fn integer(value: i64) -> ScopedArithmeticValue {
        ScopedArithmeticValue::Integer(BigInt::from_i64(value))
    }

    fn atom(identity: &str) -> ScopedArithmeticValue {
        ScopedArithmeticValue::Atom {
            identity: identity.to_owned(),
            unsigned: false,
        }
    }

    fn operation(
        operator: BinaryOperator,
        left: ScopedArithmeticValue,
        right: ScopedArithmeticValue,
    ) -> ScopedArithmeticValue {
        ScopedArithmeticValue::IntegerOperation {
            operator,
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    fn equal(
        left: ScopedArithmeticValue,
        right: ScopedArithmeticValue,
    ) -> ScopedArithmeticComparison {
        ScopedArithmeticComparison {
            operator: BinaryOperator::Equal,
            left,
            right,
        }
    }

    fn impossible(comparisons: &[ScopedArithmeticComparison]) -> bool {
        scoped_arithmetic_comparisons_unsatisfiable(
            &TypedTrees::default(),
            &Machine::default(),
            comparisons,
        )
    }

    #[test]
    fn integer_coefficient_residue_excludes_zero_without_a_sign_bound() {
        let twice = operation(BinaryOperator::Multiply, integer(2), atom("value"));
        let odd = operation(BinaryOperator::Add, twice.clone(), integer(1));
        assert!(impossible(&[equal(odd.clone(), integer(0))]));
        assert!(!impossible(&[equal(odd, integer(-1))]));
        assert!(!impossible(&[equal(twice, integer(0))]));
        // The rule concerns integer monomials, not an affine source shape.
        let product = operation(BinaryOperator::Multiply, atom("left"), atom("right"));
        let even_product = operation(BinaryOperator::Multiply, integer(-6), product);
        let incompatible = operation(BinaryOperator::Add, even_product, integer(3));
        assert!(impossible(&[equal(incompatible, integer(0))]));
    }

    #[test]
    fn correlated_equations_reuse_polynomial_bound_contradiction() {
        let numerator = operation(
            BinaryOperator::Multiply,
            operation(BinaryOperator::Add, atom("value"), integer(16384)),
            integer(-2),
        );
        let denominator = |identity| {
            operation(
                BinaryOperator::Add,
                operation(BinaryOperator::Multiply, atom(identity), integer(2)),
                integer(1),
            )
        };
        assert!(impossible(&[
            equal(numerator.clone(), integer(-32768)),
            equal(denominator("value"), integer(-1)),
        ]));
        assert!(!impossible(&[
            equal(numerator.clone(), integer(-32768)),
            equal(denominator("different"), integer(-1)),
        ]));
        assert!(!impossible(&[
            equal(numerator, integer(-32768)),
            equal(denominator("value"), integer(1)),
        ]));
    }

    #[test]
    fn unknown_operations_decline_even_beside_impossible_equalities() {
        let unsupported = operation(BinaryOperator::Divide, integer(1), integer(0));
        assert!(!impossible(&[
            equal(integer(0), integer(1)),
            equal(unsupported, integer(0)),
        ]));
        assert!(!impossible(&[ScopedArithmeticComparison {
            operator: BinaryOperator::Add,
            left: integer(0),
            right: integer(1),
        }]));
        assert!(!impossible(&[]));
    }
}

/// `descend(n, previous)`: `requires n <= previous`, the backedge
/// `descend(n - 1, n)` under `n > 0`, and the exit returning `previous`.
struct Countdown {
    program: TypedTrees,
    n: SymbolHandle,
    previous: SymbolHandle,
    n_expression: ExpressionHandle,
    requires: ExpressionHandle,
    guard: ExpressionHandle,
    decrement: ExpressionHandle,
}

impl Countdown {
    fn new() -> Self {
        let mut program = TypedTrees::default();
        let n = SymbolHandle::from_arena_index(201);
        let previous = SymbolHandle::from_arena_index(202);
        let mut name = |symbol: SymbolHandle, spelling: &'static str| {
            let mut members = HandleSpan::empty();
            program
                .expression_table
                .push_name_path_member(&mut members, Identifier::generated_static(spelling));
            program
                .expression_table
                .insert(ExpressionNode::Name(TableNamePath {
                    members,
                    head_symbol: symbol,
                    symbol,
                    ..Default::default()
                }))
        };
        let n_expression = name(n, "n");
        let previous_expression = name(previous, "previous");
        let zero = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(0)));
        let one = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(1)));
        let mut binary = |left, operator, right| {
            program
                .expression_table
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator,
                    right,
                }))
        };
        let requires = binary(
            n_expression,
            BinaryOperator::LessOrEqual,
            previous_expression,
        );
        let guard = binary(n_expression, BinaryOperator::Greater, zero);
        let decrement = binary(n_expression, BinaryOperator::Subtract, one);
        Self {
            program,
            n,
            previous,
            n_expression,
            requires,
            guard,
            decrement,
        }
    }

    fn atoms(&self, scope: &str) -> Vec<ScopedArithmeticBinding> {
        [self.n, self.previous]
            .into_iter()
            .map(|symbol| ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::Symbol(symbol),
                value: ScopedArithmeticValue::Atom {
                    identity: format!("{scope}:{symbol:?}"),
                    unsigned: true,
                },
            })
            .collect()
    }

    fn term(
        &self,
        expression: ExpressionHandle,
        bindings: Vec<ScopedArithmeticBinding>,
    ) -> ScopedArithmeticExpression {
        ScopedArithmeticExpression {
            expression,
            bindings,
        }
    }

    fn holds(&self, proposition: ScopedArithmeticExpression) -> ScopedArithmeticHypothesis {
        ScopedArithmeticHypothesis {
            proposition,
            holds: true,
        }
    }

    fn judge(
        &self,
        hypotheses: &[ScopedArithmeticHypothesis],
        goal: &ScopedArithmeticExpression,
    ) -> StrictArithmeticImplicationJudgment {
        scoped_arithmetic_implication(&self.program, &Machine::default(), hypotheses, goal)
    }
}

#[test]
fn a_denied_guard_negates_one_comparison_only() {
    let mut countdown = Countdown::new();
    let zero = countdown
        .program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(0)));
    // `n > 0` denied is `n <= 0`; with `n >= 0` implicit the goal `n == 0`
    // follows at the header.
    let goal_expression = countdown
        .program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: countdown.n_expression,
            operator: BinaryOperator::Equal,
            right: zero,
        }));
    let denied = ScopedArithmeticHypothesis {
        proposition: countdown.term(countdown.guard, countdown.atoms("header")),
        holds: false,
    };
    assert_eq!(
        countdown.judge(
            &[denied],
            &countdown.term(goal_expression, countdown.atoms("header"))
        ),
        StrictArithmeticImplicationJudgment::Proven
    );
    // A denied conjunction is a disjunction: the implication stands down.
    let conjunction = countdown
        .program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: countdown.guard,
            operator: BinaryOperator::And,
            right: countdown.requires,
        }));
    let denied = ScopedArithmeticHypothesis {
        proposition: countdown.term(conjunction, countdown.atoms("header")),
        holds: false,
    };
    assert_eq!(
        countdown.judge(
            &[denied],
            &countdown.term(goal_expression, countdown.atoms("header"))
        ),
        StrictArithmeticImplicationJudgment::Unknown
    );
}

#[test]
fn scoped_value_relation_rejects_noncomparisons_even_with_false_premises() {
    use numerics::bignum::BigInt;
    let mut program = TypedTrees::default();
    let zero = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(0)));
    let one = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(1)));
    let impossible =
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: one,
                operator: BinaryOperator::Less,
                right: zero,
            }));
    let hypotheses = [ScopedArithmeticHypothesis {
        proposition: ScopedArithmeticExpression {
            expression: impossible,
            bindings: Vec::new(),
        },
        holds: true,
    }];
    let left = ScopedArithmeticValue::Integer(BigInt::from_i64(0));
    let right = ScopedArithmeticValue::Integer(BigInt::from_i64(1));
    // Pin the inconsistent context: it can discharge a comparison, so the
    // refusals below specifically exercise the operator boundary.
    assert_eq!(
        super::scoped_arithmetic_values_relation(
            &program,
            &Machine::default(),
            &hypotheses,
            BinaryOperator::Equal,
            &left,
            &right,
        ),
        StrictArithmeticImplicationJudgment::Proven
    );
    for operator in [
        BinaryOperator::Add,
        BinaryOperator::Subtract,
        BinaryOperator::Multiply,
        BinaryOperator::And,
        BinaryOperator::Or,
    ] {
        assert_eq!(
            super::scoped_arithmetic_values_relation(
                &program,
                &Machine::default(),
                &hypotheses,
                operator,
                &left,
                &right,
            ),
            StrictArithmeticImplicationJudgment::Unknown
        );
    }
}
