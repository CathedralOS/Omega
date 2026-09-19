use super::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticHypothesis, ScopedArithmeticValue, StrictArithmeticImplicationJudgment,
    scoped_arithmetic_implication,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
};
use typed_trees::machine::Machine;

struct DomainSelf {
    program: TypedTrees,
    subject: ExpressionHandle,
    goal: ExpressionHandle,
    requirement: ExpressionHandle,
    parameter: SymbolHandle,
}

impl DomainSelf {
    fn new() -> Self {
        let source = "domain i64::Large requires self > 100;
            machine retain(value: i64) -> i64 requires value > 100 { value }";
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .unwrap();
        let program =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let domain = program
            .domain_definitions()
            .iter()
            .find(|domain| domain.predicate_body.is_present())
            .unwrap();
        let ProofFact::Expression(goal) = program.proof_facts.span_or_empty(domain.facts)[0] else {
            panic!("domain predicate")
        };
        let ExpressionNode::Binary(binary) = program.expression_table.expression(goal) else {
            panic!("comparison")
        };
        let subject = binary.left;
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "retain")
            .unwrap();
        let contract = &program.machine_contracts(machine)[0];
        let ProofFact::Expression(requirement) =
            program.proof_facts.span_or_empty(contract.facts)[0]
        else {
            panic!("requirement")
        };
        let ExpressionNode::Binary(binary) = program.expression_table.expression(requirement)
        else {
            panic!("comparison")
        };
        let ExpressionNode::Name(path) = program.expression_table.expression(binary.left) else {
            panic!("parameter")
        };
        let parameter = path.symbol;
        Self {
            program,
            subject,
            goal,
            requirement,
            parameter,
        }
    }

    fn atom(binder: ScopedArithmeticBinder, identity: &str) -> ScopedArithmeticBinding {
        ScopedArithmeticBinding {
            binder,
            value: ScopedArithmeticValue::Atom {
                identity: identity.to_owned(),
                unsigned: false,
            },
        }
    }

    fn expression(
        expression: ExpressionHandle,
        bindings: Vec<ScopedArithmeticBinding>,
    ) -> ScopedArithmeticExpression {
        ScopedArithmeticExpression {
            expression,
            bindings,
        }
    }

    fn judge(
        &self,
        hypotheses: &[ScopedArithmeticHypothesis],
        expression: ExpressionHandle,
        bindings: Vec<ScopedArithmeticBinding>,
    ) -> StrictArithmeticImplicationJudgment {
        scoped_arithmetic_implication(
            &self.program,
            &Machine::default(),
            hypotheses,
            &Self::expression(expression, bindings),
        )
    }

    fn equal(&mut self, left: ExpressionHandle, right: ExpressionHandle) -> ExpressionHandle {
        self.program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left,
                operator: BinaryOperator::Equal,
                right,
            }))
    }
}

#[test]
fn declaration_self_receives_independently_proven_parameter_value() {
    let fixture = DomainSelf::new();
    let hypothesis = ScopedArithmeticHypothesis {
        proposition: DomainSelf::expression(
            fixture.requirement,
            vec![DomainSelf::atom(
                ScopedArithmeticBinder::Symbol(fixture.parameter),
                "returned",
            )],
        ),
        holds: true,
    };
    let bindings = vec![DomainSelf::atom(
        ScopedArithmeticBinder::DomainSelf(fixture.subject),
        "returned",
    )];
    assert_eq!(
        fixture.judge(&[hypothesis], fixture.goal, bindings.clone()),
        StrictArithmeticImplicationJudgment::Proven
    );
    assert_eq!(
        fixture.judge(&[], fixture.goal, bindings),
        StrictArithmeticImplicationJudgment::Unknown
    );
}

#[test]
fn domain_self_rejects_invalid_stale_resolved_and_nonself_occurrences() {
    let mut fixture = DomainSelf::new();
    let ExpressionNode::Name(path) = fixture
        .program
        .expression_table
        .expression(fixture.subject)
        .clone()
    else {
        panic!("self")
    };
    let arithmetic = fixture
        .program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: fixture.subject,
            operator: BinaryOperator::Add,
            right: fixture.subject,
        }));
    let mut invalid = vec![
        arithmetic,
        ExpressionHandle::invalid(),
        ExpressionHandle::from_arena_index(999_999),
        ExpressionHandle::from_parts(
            fixture.subject.arena_index(),
            fixture.subject.generation() + 1,
        ),
        fixture.goal,
    ];
    for role in 0..5 {
        let mut path = path.clone();
        let symbol = SymbolHandle::from_arena_index(211);
        match role {
            0 => path.symbol = symbol,
            1 => path.head_symbol = symbol,
            2 => {
                path.member_symbols = arena::HandleSpan::empty();
                fixture
                    .program
                    .expression_table
                    .push_name_path_member_symbol(&mut path.member_symbols, symbol);
            }
            3 => {
                path.members = arena::HandleSpan::empty();
                fixture.program.expression_table.push_name_path_member(
                    &mut path.members,
                    typed_trees::name::Identifier::generated_static("other"),
                );
            }
            _ => {
                path.members = arena::HandleSpan::empty();
                for spelling in ["self", "field"] {
                    fixture.program.expression_table.push_name_path_member(
                        &mut path.members,
                        typed_trees::name::Identifier::generated_static(spelling),
                    );
                }
            }
        }
        invalid.push(
            fixture
                .program
                .expression_table
                .insert(ExpressionNode::Name(path)),
        );
    }
    for expression in invalid {
        assert_eq!(
            fixture.judge(
                &[],
                fixture.goal,
                vec![DomainSelf::atom(
                    ScopedArithmeticBinder::DomainSelf(expression),
                    "value"
                )]
            ),
            StrictArithmeticImplicationJudgment::Unknown
        );
    }
}

#[test]
fn domain_self_conflicts_fail_and_identical_bindings_agree() {
    let mut fixture = DomainSelf::new();
    let reflexive = fixture.equal(fixture.subject, fixture.subject);
    let binding = DomainSelf::atom(ScopedArithmeticBinder::DomainSelf(fixture.subject), "value");
    assert_eq!(
        fixture.judge(&[], reflexive, vec![binding.clone(), binding.clone()]),
        StrictArithmeticImplicationJudgment::Proven
    );
    let conflict = DomainSelf::atom(ScopedArithmeticBinder::DomainSelf(fixture.subject), "other");
    assert_eq!(
        fixture.judge(&[], reflexive, vec![binding, conflict]),
        StrictArithmeticImplicationJudgment::Unknown
    );
}

#[test]
fn domain_self_rosters_are_isolated_across_propositions_and_nested_terms() {
    let mut fixture = DomainSelf::new();
    let second_node = fixture
        .program
        .expression_table
        .expression(fixture.subject)
        .clone();
    let second = fixture.program.expression_table.insert(second_node);
    let equal = fixture.equal(fixture.subject, second);
    let reflexive = fixture.equal(fixture.subject, fixture.subject);
    let binding = DomainSelf::atom(ScopedArithmeticBinder::DomainSelf(fixture.subject), "inner");
    let hypothesis = ScopedArithmeticHypothesis {
        proposition: DomainSelf::expression(reflexive, vec![binding.clone()]),
        holds: true,
    };
    assert_eq!(
        fixture.judge(&[hypothesis], reflexive, vec![]),
        StrictArithmeticImplicationJudgment::Unknown
    );
    let unbound_nested = ScopedArithmeticBinding {
        binder: ScopedArithmeticBinder::DomainSelf(second),
        value: ScopedArithmeticValue::Term(DomainSelf::expression(fixture.subject, vec![])),
    };
    assert_eq!(
        fixture.judge(&[], equal, vec![binding.clone(), unbound_nested]),
        StrictArithmeticImplicationJudgment::Unknown
    );
    let nested = ScopedArithmeticBinding {
        binder: ScopedArithmeticBinder::DomainSelf(second),
        value: ScopedArithmeticValue::Term(DomainSelf::expression(
            fixture.subject,
            vec![binding.clone()],
        )),
    };
    assert_eq!(
        fixture.judge(&[], equal, vec![nested.clone()]),
        StrictArithmeticImplicationJudgment::Unknown
    );
    assert_eq!(
        fixture.judge(&[], equal, vec![binding, nested.clone()]),
        StrictArithmeticImplicationJudgment::Proven
    );
    assert_eq!(
        fixture.judge(
            &[],
            equal,
            vec![
                DomainSelf::atom(ScopedArithmeticBinder::DomainSelf(fixture.subject), "outer"),
                nested
            ]
        ),
        StrictArithmeticImplicationJudgment::Unknown
    );
}

#[test]
fn exact_integer_bindings_support_symbols_domain_self_and_nested_terms() {
    let fixture = DomainSelf::new();
    for (expression, binder) in [
        (
            fixture.requirement,
            ScopedArithmeticBinder::Symbol(fixture.parameter),
        ),
        (
            fixture.goal,
            ScopedArithmeticBinder::DomainSelf(fixture.subject),
        ),
    ] {
        for (value, expected) in [
            (101, StrictArithmeticImplicationJudgment::Proven),
            (100, StrictArithmeticImplicationJudgment::Refuted),
            (-1, StrictArithmeticImplicationJudgment::Refuted),
        ] {
            let constant = ScopedArithmeticBinding {
                binder,
                value: ScopedArithmeticValue::Integer(numerics::bignum::BigInt::from_i64(value)),
            };
            assert_eq!(
                fixture.judge(&[], expression, vec![constant.clone(), constant]),
                expected
            );
        }
        let bindings = [100, 101]
            .into_iter()
            .map(|value| ScopedArithmeticBinding {
                binder,
                value: ScopedArithmeticValue::Integer(numerics::bignum::BigInt::from_i64(value)),
            })
            .collect();
        assert_eq!(
            fixture.judge(&[], expression, bindings),
            StrictArithmeticImplicationJudgment::Unknown
        );
    }
    let nested = ScopedArithmeticBinding {
        binder: ScopedArithmeticBinder::DomainSelf(fixture.subject),
        value: ScopedArithmeticValue::Term(DomainSelf::expression(
            fixture.subject,
            vec![ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::DomainSelf(fixture.subject),
                value: ScopedArithmeticValue::Integer(numerics::bignum::BigInt::from_i64(101)),
            }],
        )),
    };
    assert_eq!(
        fixture.judge(&[], fixture.goal, vec![nested]),
        StrictArithmeticImplicationJudgment::Proven
    );
}
