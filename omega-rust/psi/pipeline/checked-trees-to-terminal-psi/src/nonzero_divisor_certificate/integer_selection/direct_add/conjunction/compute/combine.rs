//! Kernel-native ordered conjunction and exact-add mapping.

use proof_admission::{
    IntegerAffineWitness, ProofNode, ProofRule, check_integer_affine_witness,
    map_integer_affine_bound,
};
use semantic_vocabulary::{IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm};

use super::super::model::EndpointProof;

pub(super) fn expression_bound(
    context: &PropositionContext,
    integer_type: IntegerType,
    target: &ScalarTerm,
    lower: bool,
    left: EndpointProof,
    right: EndpointProof,
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let evidence = ProofNode {
        conclusion: Proposition::Conjunction(vec![
            left.proof.conclusion.clone(),
            right.proof.conclusion.clone(),
        ]),
        rule: ProofRule::ConjunctionIntroduction(vec![left.proof, right.proof]),
    };
    let ScalarTerm::ExactIntegerAdd {
        left: target_left, ..
    } = target
    else {
        return None;
    };
    let witness = IntegerAffineWitness {
        root: target_left.as_ref().clone(),
        target: target.clone(),
        definition_axioms: Vec::new(),
        literal_axioms: Vec::new(),
    };
    let form = check_integer_affine_witness(context, semantic_axioms, &witness).ok()?;
    let mapped = map_integer_affine_bound(&form, &evidence.conclusion).ok()?;
    let expected = add(integer_type, left.value, right.value)?;
    if mapped_endpoint(&mapped, integer_type, lower) != Some(expected) {
        return None;
    }
    Some(ProofNode {
        conclusion: mapped,
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(evidence),
            witness,
        },
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn mapped_definition_bound(
    context: &PropositionContext,
    root: &ScalarTerm,
    target: &ScalarTerm,
    definition_index: usize,
    literal_index: Option<usize>,
    root_bound: ProofNode,
    integer_type: IntegerType,
    value: IntegerValue,
    lower: bool,
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let witness = IntegerAffineWitness {
        root: root.clone(),
        target: target.clone(),
        definition_axioms: vec![definition_index],
        literal_axioms: vec![literal_index],
    };
    let form = check_integer_affine_witness(context, semantic_axioms, &witness).ok()?;
    let mapped = map_integer_affine_bound(&form, &root_bound.conclusion).ok()?;
    let literal = ScalarTerm::integer(integer_type, value).ok()?;
    let expected = if lower {
        Proposition::LessOrEqual(literal, target.clone())
    } else {
        Proposition::LessOrEqual(target.clone(), literal)
    };
    (mapped == expected).then_some(ProofNode {
        conclusion: mapped,
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(root_bound),
            witness,
        },
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn computed_definition_bound(
    context: &PropositionContext,
    expression: &ScalarTerm,
    output: &ScalarTerm,
    definition_axiom: usize,
    integer_type: IntegerType,
    left: EndpointProof,
    right: EndpointProof,
    lower: bool,
    semantic_axioms: &[Proposition],
) -> Option<EndpointProof> {
    let value = add(integer_type, left.value, right.value)?;
    expression_bound(
        context,
        integer_type,
        expression,
        lower,
        left.clone(),
        right.clone(),
        semantic_axioms,
    )?;
    let literal = ScalarTerm::integer(integer_type, value).ok()?;
    Some(EndpointProof {
        value,
        proof: ProofNode {
            conclusion: if lower {
                Proposition::LessOrEqual(literal, output.clone())
            } else {
                Proposition::LessOrEqual(output.clone(), literal)
            },
            rule: ProofRule::IntegerExactAddDefinitionBound {
                left_bound: Box::new(left.proof),
                right_bound: Box::new(right.proof),
                definition_axiom,
            },
        },
    })
}

pub(super) fn add(
    integer_type: IntegerType,
    left: IntegerValue,
    right: IntegerValue,
) -> Option<IntegerValue> {
    let result = match (left, right) {
        (IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
            IntegerValue::Signed(left.checked_add(right)?)
        }
        (IntegerValue::Unsigned(left), IntegerValue::Unsigned(right)) => {
            IntegerValue::Unsigned(left.checked_add(right)?)
        }
        _ => return None,
    };
    integer_type.admits(result).then_some(result)
}

fn mapped_endpoint(
    proposition: &Proposition,
    integer_type: IntegerType,
    lower: bool,
) -> Option<IntegerValue> {
    let Proposition::IntegerMathLessOrEqual(left, right) = proposition else {
        return None;
    };
    let endpoint = if lower { left } else { right };
    let semantic_vocabulary::IntegerMathTerm::IntegerLiteral(literal) = endpoint else {
        return None;
    };
    literal.as_integer_value(integer_type)
}
