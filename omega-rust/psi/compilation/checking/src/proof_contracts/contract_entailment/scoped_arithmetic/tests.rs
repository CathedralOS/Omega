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
