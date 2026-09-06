//! Bounded recursive affine-chain endpoint production.

use proof_admission::{PrimitiveJudgment, ProofNode, ProofRule};
use semantic_vocabulary::{IntegerType, Proposition, PropositionContext, ScalarTerm};

use super::super::definitions;
use super::super::model::{EndpointProof, Query, SearchState};
use super::combine;
use crate::nonzero_divisor_certificate::affine_custody::DefinitionIndex;

#[allow(clippy::too_many_arguments)]
pub(super) fn derive(
    context: &PropositionContext,
    integer_type: IntegerType,
    operand: &ScalarTerm,
    lower: bool,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definition_index: &DefinitionIndex,
    cutoff: usize,
    depth: usize,
    state: &mut SearchState,
) -> Option<EndpointProof> {
    let query = Query::new(operand, lower, cutoff);
    if let Some(result) = state.memoized(&query) {
        return result;
    }
    if !state.enter(&query, depth) {
        return None;
    }
    let result = derive_uncached(
        context,
        integer_type,
        operand,
        lower,
        assumptions,
        semantic_axioms,
        definition_index,
        cutoff,
        depth,
        state,
    );
    state.leave(query, result.clone());
    result
}

#[allow(clippy::too_many_arguments)]
fn derive_uncached(
    context: &PropositionContext,
    integer_type: IntegerType,
    operand: &ScalarTerm,
    lower: bool,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definition_index: &DefinitionIndex,
    cutoff: usize,
    depth: usize,
    state: &mut SearchState,
) -> Option<EndpointProof> {
    if integer_type.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
        || operand.scalar_type() != semantic_vocabulary::ScalarType::Integer(integer_type)
    {
        return None;
    }
    if let Some((actual, value)) = operand.integer_value() {
        return (actual == integer_type).then_some(EndpointProof {
            value,
            proof: ProofNode {
                conclusion: Proposition::Truth,
                rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
            },
        });
    }
    if let Some((value, proof)) =
        definitions::direct_literal(operand, integer_type, assumptions, semantic_axioms, cutoff)
    {
        let literal = ScalarTerm::integer(integer_type, value).ok()?;
        return Some(EndpointProof {
            value,
            proof: orient_exact(operand, literal, lower, proof),
        });
    }
    let definition = definitions::exact_add(
        operand,
        integer_type,
        semantic_axioms,
        definition_index,
        cutoff,
    )?;
    if !state.visit_definition() {
        return None;
    }
    for (root, sibling, root_is_left) in [
        (definition.left, definition.right, true),
        (definition.right, definition.left, false),
    ] {
        let Some(landing) = definitions::semantic_literal_landing(
            sibling,
            integer_type,
            semantic_axioms,
            definition.index,
        ) else {
            continue;
        };
        let Some(root_bound) = derive(
            context,
            integer_type,
            root,
            lower,
            assumptions,
            semantic_axioms,
            definition_index,
            definition.index,
            depth + 1,
            state,
        ) else {
            if state.exhausted() {
                return None;
            }
            continue;
        };
        let value = if root_is_left {
            combine::add(integer_type, root_bound.value, landing.value)
        } else {
            combine::add(integer_type, landing.value, root_bound.value)
        }?;
        let proof = combine::mapped_definition_bound(
            context,
            root,
            operand,
            definition.index,
            landing.semantic_index,
            root_bound.proof,
            integer_type,
            value,
            lower,
            semantic_axioms,
        )?;
        return Some(EndpointProof { value, proof });
    }
    definitions::exact_add(
        definition.left,
        integer_type,
        semantic_axioms,
        definition_index,
        definition.index,
    )?;
    definitions::exact_add(
        definition.right,
        integer_type,
        semantic_axioms,
        definition_index,
        definition.index,
    )?;
    if !state.visit_computed_join() {
        return None;
    }
    let left = derive(
        context,
        integer_type,
        definition.left,
        lower,
        assumptions,
        semantic_axioms,
        definition_index,
        definition.index,
        depth + 1,
        state,
    )?;
    let right = derive(
        context,
        integer_type,
        definition.right,
        lower,
        assumptions,
        semantic_axioms,
        definition_index,
        definition.index,
        depth + 1,
        state,
    )?;
    combine::computed_definition_bound(
        context,
        definition.expression,
        operand,
        definition.index,
        integer_type,
        left,
        right,
        lower,
        semantic_axioms,
    )
}

fn orient_exact(
    operand: &ScalarTerm,
    literal: ScalarTerm,
    lower: bool,
    equality: ProofNode,
) -> ProofNode {
    ProofNode {
        conclusion: if lower {
            Proposition::LessOrEqual(literal.clone(), operand.clone())
        } else {
            Proposition::LessOrEqual(operand.clone(), literal.clone())
        },
        rule: ProofRule::IntegerOrderSubstitution {
            relation: Box::new(ProofNode {
                conclusion: Proposition::LessOrEqual(literal.clone(), literal),
                rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
            }),
            equality: Box::new(equality),
            endpoint: usize::from(lower),
        },
    }
}
