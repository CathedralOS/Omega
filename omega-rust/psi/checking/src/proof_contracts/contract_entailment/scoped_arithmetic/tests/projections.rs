use super::{
    Countdown, ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticHypothesis,
    ScopedArithmeticValue, StrictArithmeticImplicationJudgment,
};
use numerics::literals::IntegerLiteral;
use symbols::SymbolHandle;
use typed_trees::expression::{
    BinaryOperator, CallExpression, Expression, ExpressionHandle, ExpressionNode,
    TableBinaryExpression, TableMemberExpression,
};
use typed_trees::name::Identifier;

struct Projections {
    countdown: Countdown,
    first: ExpressionHandle,
    reflexive: ExpressionHandle,
    constant: ExpressionHandle,
}

impl Projections {
    fn new() -> Self {
        let mut countdown = Countdown::new();
        let mut member = || {
            countdown
                .program
                .expression_table
                .insert(ExpressionNode::Member(TableMemberExpression {
                    receiver: countdown.n_expression,
                    member_symbol: SymbolHandle::from_arena_index(203),
                    member: Identifier::generated_static("count"),
                    case_variant: None,
                }))
        };
        let first = member();
        let reflexive = countdown
            .program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: first,
                operator: BinaryOperator::Equal,
                right: first,
            }));
        let constant = countdown
            .program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(7)));
        Self {
            countdown,
            first,
            reflexive,
            constant,
        }
    }

    fn atom(&self, expression: ExpressionHandle, identity: &str) -> ScopedArithmeticBinding {
        ScopedArithmeticBinding {
            binder: ScopedArithmeticBinder::Projection(expression),
            value: ScopedArithmeticValue::Atom {
                identity: identity.to_owned(),
                unsigned: false,
            },
        }
    }
}

#[test]
fn conflicting_projection_rosters_reject_before_constant_or_inconsistent_proofs() {
    let mut fixture = Projections::new();
    let constant_truth = fixture
        .countdown
        .program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: fixture.constant,
            operator: BinaryOperator::Equal,
            right: fixture.constant,
        }));
    let conflict = vec![
        fixture.atom(fixture.first, "first"),
        fixture.atom(fixture.first, "second"),
    ];
    let countdown = &fixture.countdown;
    let impossible = ScopedArithmeticHypothesis {
        proposition: countdown.term(constant_truth, Vec::new()),
        holds: false,
    };
    for hypotheses in [vec![], vec![impossible]] {
        assert_eq!(
            countdown.judge(
                &hypotheses,
                &countdown.term(constant_truth, conflict.clone())
            ),
            StrictArithmeticImplicationJudgment::Unknown
        );
    }
    let bad_hypothesis = countdown.holds(countdown.term(constant_truth, conflict));
    assert_eq!(
        countdown.judge(
            &[bad_hypothesis],
            &countdown.term(constant_truth, Vec::new())
        ),
        StrictArithmeticImplicationJudgment::Unknown
    );
    let duplicate = vec![
        fixture.atom(fixture.first, "same"),
        fixture.atom(fixture.first, "same"),
    ];
    assert_eq!(
        countdown.judge(&[], &countdown.term(fixture.reflexive, duplicate)),
        StrictArithmeticImplicationJudgment::Proven
    );
}

#[test]
fn only_valid_member_expressions_can_be_projection_binders() {
    let mut fixture = Projections::new();
    let call = fixture
        .countdown
        .program
        .expression_table
        .insert_tree(&Expression::Call(Box::new(CallExpression {
            receiver: None,
            target_symbol: SymbolHandle::from_arena_index(204),
            target: Identifier::generated_static("count"),
            arguments: [].into(),
            evidence_arguments: [].into(),
            operational_acknowledgement: Default::default(),
        })));
    let countdown = &fixture.countdown;
    for expression in [
        ExpressionHandle::invalid(),
        countdown.n_expression,
        countdown.decrement,
        fixture.constant,
        call,
    ] {
        let bindings = vec![
            fixture.atom(expression, "value"),
            fixture.atom(fixture.first, "value"),
        ];
        assert_eq!(
            countdown.judge(&[], &countdown.term(fixture.reflexive, bindings)),
            StrictArithmeticImplicationJudgment::Unknown
        );
    }
}

#[test]
fn empty_rosters_cannot_inherit_projection_bindings_from_goals_or_hypotheses() {
    let fixture = Projections::new();
    let countdown = &fixture.countdown;
    let roster = vec![fixture.atom(fixture.first, "value")];
    let bound = countdown.term(fixture.reflexive, roster);
    let unbound = countdown.term(fixture.reflexive, Vec::new());
    assert_eq!(
        countdown.judge(&[countdown.holds(bound.clone())], &unbound),
        StrictArithmeticImplicationJudgment::Unknown
    );
    // A denied unbound comparison must stand down, not become an inconsistent
    // hypothesis by inheriting the goal's initial projection roster.
    let denied = ScopedArithmeticHypothesis {
        proposition: unbound.clone(),
        holds: false,
    };
    assert_eq!(
        countdown.judge(std::slice::from_ref(&denied), &bound),
        StrictArithmeticImplicationJudgment::Unknown
    );
    assert_eq!(
        countdown.judge(&[countdown.holds(bound.clone()), denied], &bound),
        StrictArithmeticImplicationJudgment::Unknown
    );
}
