//! Select a remainder's exact divisor and prove it positive independently.
//! The existing affine rule checks the selected expression or SSA definition;
//! its presence never supplies its own nonzero premise. Restrict lookup to
//! definitions of the requested result or its proved value aliases. The shared
//! order machinery transports those equalities; no scan of unrelated arithmetic
//! rows or new cast authority is needed for a policy-erased remainder.

use proof_admission::{
    IntegerAffineWitness, ProofNode, ProofRule, check_integer_affine_witness,
    map_integer_affine_bound,
};
use semantic_vocabulary::{IntegerValue, Proposition, PropositionContext, ScalarTerm};

use super::super::super::super::affine_custody::DefinitionIndex;
use super::super::super::exact;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    let Proposition::LessThan(goal_left, _) = goal else {
        return None;
    };
    let session = exact::session(context, assumptions, semantic_axioms);
    let aliases = session.value_aliases(goal_left);
    for target in std::iter::once(goal_left).chain(aliases.iter().map(|(alias, _)| alias)) {
        let positions = definitions.output_definitions_before(target, semantic_axioms.len());
        for position in std::iter::once(None).chain(positions.into_iter().map(Some)) {
            let expression = match position {
                None => target,
                Some(position) => {
                    let Proposition::Equal(result, expression) = &semantic_axioms[position] else {
                        continue;
                    };
                    if result != target {
                        continue;
                    }
                    expression
                }
            };
            let (integer_type, divisor) = match expression {
                ScalarTerm::ExactIntegerRemainder {
                    scalar_type, right, ..
                }
                | ScalarTerm::WrappingIntegerRemainder {
                    scalar_type, right, ..
                }
                | ScalarTerm::SaturatingIntegerRemainder {
                    scalar_type, right, ..
                } => (*scalar_type, right.as_ref()),
                _ => continue,
            };
            let witness = IntegerAffineWitness {
                root: divisor.clone(),
                target: target.clone(),
                definition_axioms: position.into_iter().collect(),
                literal_axioms: position.into_iter().map(|_| None).collect(),
            };
            let Ok(form) = check_integer_affine_witness(context, semantic_axioms, &witness) else {
                continue;
            };
            let zero = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(0)).ok()?;
            // This leaf selector does not re-enter remainder production: an
            // expression cannot establish its own formation by recursive search.
            let Some(positive) = super::prove_without_subtract(
                context,
                &Proposition::LessThan(zero, divisor.clone()),
                assumptions,
                semantic_axioms,
            ) else {
                continue;
            };
            let Ok(conclusion) = map_integer_affine_bound(&form, &positive.conclusion) else {
                continue;
            };
            let proof = ProofNode {
                conclusion,
                rule: ProofRule::IntegerAffineBound {
                    root_bound: Box::new(positive),
                    witness,
                },
            };
            if let Some(proof) = super::complete(&session, goal, proof) {
                return Some(proof);
            }
        }
    }
    None
}
