use super::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticValue, StrictArithmeticImplicationJudgment, scoped_arithmetic_implication,
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
        let program = crate::front_end::typed_program(&source);
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
