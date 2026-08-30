//! Canonical integer proposition-kind proof dispatch.

use psi_core::{IntegerMathTerm, Proposition, PropositionContext, ScalarTerm, ScalarType};
use psi_proof_admission::{PrimitiveJudgment, ProofNode, ProofRule, lift_fixed_integer_relation};

use super::super::affine_custody::DefinitionIndex;
use super::bound;

pub(super) fn prove_atomic(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<Option<ProofNode>> {
    match goal {
        Proposition::Truth => Some(Some(ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
        })),
        Proposition::LessOrEqual(_, _) => Some(bound::prove(
            context,
            goal,
            assumptions,
            semantic_axioms,
            definitions,
        )),
        Proposition::IntegerMathEqual(_, _)
        | Proposition::IntegerMathLessThan(_, _)
        | Proposition::IntegerMathLessOrEqual(_, _) => Some(prove_math_relation(
            context,
            goal,
            assumptions,
            semantic_axioms,
            definitions,
        )),
        _ => None,
    }
}

fn prove_math_relation(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    let scalar_goal = scalar_relation_for_math_goal(goal)?;
    let scalar_proof = match &scalar_goal {
        Proposition::LessOrEqual(_, _) => bound::prove(
            context,
            &scalar_goal,
            assumptions,
            semantic_axioms,
            definitions,
        )?,
        _ => return None,
    };
    (lift_fixed_integer_relation(&scalar_goal).as_ref() == Some(goal)).then(|| ProofNode {
        conclusion: goal.clone(),
        rule: scalar_proof.rule,
    })
}

fn scalar_relation_for_math_goal(goal: &Proposition) -> Option<Proposition> {
    let (kind, left, right) = match goal {
        Proposition::IntegerMathEqual(left, right) => (0, left, right),
        Proposition::IntegerMathLessThan(left, right) => (1, left, right),
        Proposition::IntegerMathLessOrEqual(left, right) => (2, left, right),
        _ => return None,
    };
    let (source_type, value) = match (left, right) {
        (IntegerMathTerm::MathValue { source_type, value }, _)
        | (_, IntegerMathTerm::MathValue { source_type, value }) => (*source_type, *value),
        _ => return None,
    };
    let convert = |term: &IntegerMathTerm| match term {
        IntegerMathTerm::MathValue {
            source_type: term_type,
            value: term_value,
        } if *term_type == source_type && *term_value == value => {
            Some(ScalarTerm::value(value, ScalarType::Integer(source_type)))
        }
        IntegerMathTerm::IntegerLiteral(literal) => {
            ScalarTerm::integer(source_type, literal.as_integer_value(source_type)?).ok()
        }
        _ => None,
    };
    let left = convert(left)?;
    let right = convert(right)?;
    Some(match kind {
        0 => Proposition::Equal(left, right),
        1 => Proposition::LessThan(left, right),
        _ => Proposition::LessOrEqual(left, right),
    })
}
