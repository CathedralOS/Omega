use super::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticHypothesis, ScopedArithmeticValue, StrictArithmeticImplicationJudgment,
    scoped_arithmetic_implication,
};
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

struct EmbeddedFields {
    program: TypedTrees,
    goal: ExpressionHandle,
    left: ExpressionHandle,
    right: ExpressionHandle,
}

impl EmbeddedFields {
    fn new(predicate: &str, policy: &str) -> Self {
        let source = format!(
            "data Extent {{ length: u8 in {policy}; }}
            machine compare(left: Extent, right: Extent)
            requires {predicate} {{ }}"
        );
        let tokens = source_files_to_tokens::Lexer::new(&source)
            .tokenize()
            .expect("tokens");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .expect("resolved");
        let program = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("typed");
        let machine = &program.machines()[0];
        let contract = &program.machine_contracts(machine)[0];
        let ProofFact::Expression(goal) = program.proof_facts.span_or_empty(contract.facts)[0]
        else {
            panic!("expression requirement");
        };
        let members: Vec<_> = program
            .expression_table
            .iter_expressions()
            .filter_map(|(handle, node)| {
                matches!(node, ExpressionNode::Member(_)).then_some(handle)
            })
            .collect();
        Self {
            program,
            goal,
            left: members[0],
            right: members[1],
        }
    }

    fn binding(expression: ExpressionHandle, identity: &str) -> ScopedArithmeticBinding {
        ScopedArithmeticBinding {
            binder: ScopedArithmeticBinder::Projection(expression),
            value: ScopedArithmeticValue::Atom {
                identity: identity.to_owned(),
                unsigned: true,
            },
        }
    }

    fn judge(&self, bindings: Vec<ScopedArithmeticBinding>) -> StrictArithmeticImplicationJudgment {
        scoped_arithmetic_implication(
            &self.program,
            &self.program.machines()[0],
            &[],
            &ScopedArithmeticExpression {
                expression: self.goal,
                bindings,
            },
        )
    }
}

#[test]
fn embedded_fields_use_exact_scoped_atoms_and_nested_terms() {
    let fixture = EmbeddedFields::new("embed(left.length) == embed(right.length)", "Wrapping");
    let left = EmbeddedFields::binding(fixture.left, "captured");
    let right = EmbeddedFields::binding(fixture.right, "captured");
    assert_eq!(
        fixture.judge(vec![left.clone(), right.clone()]),
        StrictArithmeticImplicationJudgment::Proven
    );
    let nested = ScopedArithmeticBinding {
        binder: ScopedArithmeticBinder::Projection(fixture.right),
        value: ScopedArithmeticValue::Term(ScopedArithmeticExpression {
            expression: fixture.left,
            bindings: vec![left.clone()],
        }),
    };
    assert_eq!(
        fixture.judge(vec![left, nested]),
        StrictArithmeticImplicationJudgment::Proven
    );
}

#[test]
fn embedded_fields_require_each_exact_occurrence_and_current_roster() {
    let fixture = EmbeddedFields::new("embed(left.length) == embed(right.length)", "Saturating");
    let left = EmbeddedFields::binding(fixture.left, "first");
    let right = EmbeddedFields::binding(fixture.right, "second");
    for bindings in [vec![], vec![left.clone()], vec![left.clone(), right]] {
        assert_eq!(
            fixture.judge(bindings),
            StrictArithmeticImplicationJudgment::Unknown
        );
    }
    let hypothesis = ScopedArithmeticHypothesis {
        proposition: ScopedArithmeticExpression {
            expression: fixture.goal,
            bindings: vec![left, EmbeddedFields::binding(fixture.right, "first")],
        },
        holds: true,
    };
    assert_eq!(
        scoped_arithmetic_implication(
            &fixture.program,
            &fixture.program.machines()[0],
            &[hypothesis],
            &ScopedArithmeticExpression {
                expression: fixture.goal,
                bindings: vec![]
            },
        ),
        StrictArithmeticImplicationJudgment::Unknown
    );
}

#[test]
fn embedded_fields_retain_carrier_bounds_for_bound_values() {
    let fixture = EmbeddedFields::new(
        "embed(left.length) + embed(right.length) <= 510",
        "Wrapping",
    );
    assert_eq!(
        fixture.judge(vec![
            EmbeddedFields::binding(fixture.left, "left"),
            EmbeddedFields::binding(fixture.right, "right"),
        ]),
        StrictArithmeticImplicationJudgment::Proven
    );
}

#[test]
fn embedded_computed_policy_values_do_not_become_mathematical_sums() {
    for policy in ["Wrapping", "Saturating"] {
        let fixture = EmbeddedFields::new("embed(left.length + right.length) == 256", policy);
        let bindings = [(fixture.left, 255), (fixture.right, 1)]
            .into_iter()
            .map(|(expression, value)| ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::Projection(expression),
                value: ScopedArithmeticValue::Integer(numerics::bignum::BigInt::from_i64(value)),
            })
            .collect();
        assert_eq!(
            fixture.judge(bindings),
            StrictArithmeticImplicationJudgment::Unknown
        );
    }
}
