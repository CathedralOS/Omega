//! Simultaneous mathematical argument substitution, independent of source admission.
use super::{
    BinaryOperator, ExpressionHandle, ExpressionNode, Machine, StrictArithmeticBindingValue,
    StrictArithmeticExpressionBinding, StrictArithmeticImplicationJudgment,
    StrictArithmeticSymbolBinding, TypedTrees,
    strict_arithmetic_expression_implication_with_arguments,
};
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
fn retained_disequality_follows_later_equations_without_equating_scoped_atoms() {
    use super::arithmetic_judgment::{Engine, Judgment};
    use super::{ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticValue};

    let mut fixture = Fixture::new();
    let zero = fixture.integer(0);
    let original_nonzero =
        fixture.binary(fixture.caller_expression, BinaryOperator::NotEqual, zero);
    let other_nonzero = fixture.binary(fixture.second_expression, BinaryOperator::NotEqual, zero);
    let same_value = fixture.binary(
        fixture.caller_expression,
        BinaryOperator::Equal,
        fixture.second_expression,
    );
    let bindings = [
        StrictArithmeticSymbolBinding {
            symbol: fixture.caller,
            value: StrictArithmeticBindingValue::Atom {
                identity: "original".to_owned(),
                unsigned: false,
            },
        },
        StrictArithmeticSymbolBinding {
            symbol: fixture.second,
            value: StrictArithmeticBindingValue::Atom {
                identity: "other".to_owned(),
                unsigned: false,
            },
        },
    ];
    let mut engine =
        Engine::strict_with_symbol_bindings(&fixture.program, &Machine::default(), &bindings);
    let mut comparisons = Vec::new();
    assert!(engine.collect_comparisons(&[original_nonzero], &mut comparisons));
    assert!(engine.install_hypotheses(comparisons));
    assert!(matches!(
        engine.judge(other_nonzero),
        Judgment::Unknown { .. }
    ));
    let mut comparisons = Vec::new();
    assert!(engine.collect_comparisons(&[same_value], &mut comparisons));
    assert!(engine.install_hypotheses(comparisons));
    assert!(matches!(engine.judge(other_nonzero), Judgment::Proven));

    let roster = |identity: &str| {
        [ScopedArithmeticBinding {
            binder: ScopedArithmeticBinder::Symbol(fixture.caller),
            value: ScopedArithmeticValue::Atom {
                identity: identity.to_owned(),
                unsigned: false,
            },
        }]
    };
    let mut engine =
        Engine::strict_with_symbol_bindings(&fixture.program, &Machine::default(), &[]);
    assert!(engine.install_scoped_bindings(&roster("before")));
    let mut comparisons = Vec::new();
    assert!(engine.collect_comparisons(&[original_nonzero], &mut comparisons));
    assert!(engine.install_hypotheses(comparisons));
    assert!(engine.install_scoped_bindings(&roster("after")));
    assert!(matches!(
        engine.judge(original_nonzero),
        Judgment::Unknown { .. }
    ));
    assert!(engine.install_scoped_bindings(&roster("before")));
    assert!(matches!(engine.judge(original_nonzero), Judgment::Proven));
}

#[test]
fn exact_argument_widening_does_not_change_other_strict_reader_languages() {
    let source = "machine caller(input: u8) -> u64 { input as u64 }";
    let program = crate::front_end::typed_program(source);
    let machine = &program.machines()[0];
    let parameter = &program.state_parameters(&program.machine_states(machine)[0])[0];
    let cast = program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, expression)| {
            matches!(expression, ExpressionNode::Cast(_)).then_some(handle)
        })
        .unwrap();
    let bindings = [StrictArithmeticSymbolBinding {
        symbol: parameter.symbol,
        value: StrictArithmeticBindingValue::Atom {
            identity: "input".to_owned(),
            unsigned: true,
        },
    }];
    let mut engine = super::arithmetic_judgment::Engine::strict_with_symbol_bindings(
        &program, machine, &bindings,
    );
    let arguments = [argument(SymbolHandle::from_arena_index(10001), cast)];
    assert!(engine.normalize(cast).is_none());
    assert!(!engine.bind_strict_arguments(&arguments));
    assert!(engine.exact_argument_values(&arguments).is_some());
    assert!(
        engine.normalize(cast).is_none(),
        "successful capture restores the ordinary language"
    );
    assert!(
        engine
            .exact_argument_values(&[argument(SymbolHandle::invalid(), cast)])
            .is_none()
    );
    assert!(
        engine.normalize(cast).is_none(),
        "failed capture also restores the ordinary language"
    );
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
