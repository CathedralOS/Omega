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
/// `descend(n - 1, n)` under `n > 0`, the exit returning `previous`, and the
/// guarantee `result <= previous`.
struct Countdown {
    program: TypedTrees,
    n: SymbolHandle,
    previous: SymbolHandle,
    n_expression: ExpressionHandle,
    previous_expression: ExpressionHandle,
    requires: ExpressionHandle,
    guard: ExpressionHandle,
    guarantee: ExpressionHandle,
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
        let result = name(SymbolHandle::invalid(), "result");
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
        let guarantee = binary(result, BinaryOperator::LessOrEqual, previous_expression);
        let decrement = binary(n_expression, BinaryOperator::Subtract, one);
        Self {
            program,
            n,
            previous,
            n_expression,
            previous_expression,
            requires,
            guard,
            guarantee,
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

    /// The guarantee over the invocation formals with `result` denoting the
    /// returned `previous` read under `returned`.
    fn transported(&self, returned: Vec<ScopedArithmeticBinding>) -> ScopedArithmeticExpression {
        let mut bindings = self.atoms("invocation");
        bindings.push(ScopedArithmeticBinding {
            binder: ScopedArithmeticBinder::Result,
            value: ScopedArithmeticValue::Term(self.term(self.previous_expression, returned)),
        });
        self.term(self.guarantee, bindings)
    }

    /// The backedge substitution `n := step, previous := n` at the header.
    fn arrival(&self, step: ExpressionHandle) -> Vec<ScopedArithmeticBinding> {
        vec![
            ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::Symbol(self.n),
                value: ScopedArithmeticValue::Term(self.term(step, self.atoms("header"))),
            },
            ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::Symbol(self.previous),
                value: ScopedArithmeticValue::Term(
                    self.term(self.n_expression, self.atoms("header")),
                ),
            },
        ]
    }

    fn holds(&self, proposition: ScopedArithmeticExpression) -> ScopedArithmeticHypothesis {
        ScopedArithmeticHypothesis {
            proposition,
            holds: true,
        }
    }

    fn preservation_hypotheses(&self) -> Vec<ScopedArithmeticHypothesis> {
        vec![
            self.holds(self.transported(self.atoms("header"))),
            self.holds(self.term(self.requires, self.atoms("header"))),
            self.holds(self.term(self.guard, self.atoms("header"))),
        ]
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
fn establishment_reads_header_and_formals_as_one_roster() {
    let countdown = Countdown::new();
    assert_eq!(
        countdown.judge(
            &[countdown.holds(countdown.term(countdown.requires, countdown.atoms("invocation")))],
            &countdown.transported(countdown.atoms("invocation")),
        ),
        StrictArithmeticImplicationJudgment::Proven
    );
}

#[test]
fn preservation_needs_the_transported_conjunct_and_the_requirement() {
    let countdown = Countdown::new();
    let goal = countdown.transported(countdown.arrival(countdown.decrement));
    assert_eq!(
        countdown.judge(&countdown.preservation_hypotheses(), &goal),
        StrictArithmeticImplicationJudgment::Proven
    );
    // Without the header conjunct the invocation atom is unconstrained.
    let mut hypotheses = countdown.preservation_hypotheses();
    hypotheses.remove(0);
    assert_eq!(
        countdown.judge(&hypotheses, &goal),
        StrictArithmeticImplicationJudgment::Unknown
    );
    // Without the requirement the two header atoms are unrelated.
    let mut hypotheses = countdown.preservation_hypotheses();
    hypotheses.remove(1);
    assert_eq!(
        countdown.judge(&hypotheses, &goal),
        StrictArithmeticImplicationJudgment::Unknown
    );
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
fn a_wrong_step_is_not_proven_and_an_unbound_name_stands_down() {
    let mut countdown = Countdown::new();
    let thousand = countdown
        .program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(1000)));
    // `previous := 1000` at the backedge: `1000 <= previous@invocation` has
    // no support, and the roster never fabricates it.
    let wrong = vec![
        ScopedArithmeticBinding {
            binder: ScopedArithmeticBinder::Symbol(countdown.n),
            value: ScopedArithmeticValue::Term(
                countdown.term(countdown.decrement, countdown.atoms("header")),
            ),
        },
        ScopedArithmeticBinding {
            binder: ScopedArithmeticBinder::Symbol(countdown.previous),
            value: ScopedArithmeticValue::Term(countdown.term(thousand, Vec::new())),
        },
    ];
    assert_eq!(
        countdown.judge(
            &countdown.preservation_hypotheses(),
            &countdown.transported(wrong)
        ),
        StrictArithmeticImplicationJudgment::Unknown
    );
    // A roster that leaves `previous` unbound is outside the language, even
    // though the same spelling is bound in every other proposition.
    let mut partial = countdown.atoms("invocation");
    partial.retain(|binding| binding.binder != ScopedArithmeticBinder::Symbol(countdown.previous));
    partial.push(ScopedArithmeticBinding {
        binder: ScopedArithmeticBinder::Result,
        value: ScopedArithmeticValue::Term(
            countdown.term(countdown.previous_expression, countdown.atoms("header")),
        ),
    });
    assert_eq!(
        countdown.judge(
            &countdown.preservation_hypotheses(),
            &countdown.term(countdown.guarantee, partial)
        ),
        StrictArithmeticImplicationJudgment::Unknown
    );
    // The synthetic result is only ever bound explicitly.
    assert_eq!(
        countdown.judge(
            &countdown.preservation_hypotheses(),
            &countdown.term(countdown.guarantee, countdown.atoms("invocation"))
        ),
        StrictArithmeticImplicationJudgment::Unknown
    );
}

#[test]
fn denied_partly_read_conjunction_cannot_create_a_contradiction() {
    let mut countdown = Countdown::new();
    let false_expression = countdown
        .program
        .expression_table
        .insert(ExpressionNode::Boolean(false));
    let conjunction = countdown
        .program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: countdown.guard,
            operator: BinaryOperator::And,
            right: false_expression,
        }));
    let bindings = countdown.atoms("header");
    let positive = countdown.holds(countdown.term(countdown.guard, bindings.clone()));
    let denied = ScopedArithmeticHypothesis {
        proposition: countdown.term(conjunction, bindings.clone()),
        holds: false,
    };
    // n > 0 and !(n > 0 && false) are consistent. Dropping the unread false
    // conjunct before negation would invent n <= 0 and then prove false.
    assert_eq!(
        countdown.judge(
            &[positive, denied],
            &countdown.term(false_expression, Vec::new())
        ),
        StrictArithmeticImplicationJudgment::Unknown,
    );
    // Asserted conjunctions may still contribute readable conjuncts. This
    // weakening must not be rejected by the denied-proposition shape guard.
    let asserted = countdown.holds(countdown.term(conjunction, bindings.clone()));
    assert_eq!(
        countdown.judge(&[asserted], &countdown.term(countdown.guard, bindings)),
        StrictArithmeticImplicationJudgment::Proven,
    );
}
