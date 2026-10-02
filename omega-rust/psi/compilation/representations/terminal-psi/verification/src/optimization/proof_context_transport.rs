//! Transport existing scalar certificates through independently checked rewrites.
//!
//! A copy relation supplies exact value substitutions; total-scalar removal
//! supplies none. Reconstruct both complete question rosters. Owners, goals,
//! classes, ordered requirements and invariant arrivals must agree under that
//! relation. The output may forget definitional axioms, but may introduce none.
//! Certificate citations then move by proposition identity, not by old position.
//! A vanished copy equation can become reflexivity; a vanished material premise
//! refuses the rewrite. No proof search runs here. Final kernel replay checks the
//! transported bundle, including unchanged crash and ranking evidence, before
//! the producer can publish it. The input bundle is replayed too, so transport
//! cannot repair an originally invalid certificate by erasing its bad citation.

use crate::verification::substitute_proposition_values;
use crate::{
    ReconstructedTerminalObligationSet, reconstruct_optimizable_terminal_obligations,
    validate_module_for_optimization, verify_module_for_optimization,
};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{Proposition, ScalarTerm, ValueId};
use std::collections::BTreeMap;
use terminal_psi::{EvidenceRoute, ProofBundle, ProofNode, ProofRule, TerminalModule};

type Substitutions = BTreeMap<ValueId, ScalarTerm>;

pub(super) fn questions_match(
    before: &ReconstructedTerminalObligationSet,
    after: &ReconstructedTerminalObligationSet,
    substitutions: &Substitutions,
) -> bool {
    before.obligations().len() == after.obligations().len()
        && before
            .obligations()
            .iter()
            .zip(after.obligations())
            .all(|(old, new)| {
                old.owner == new.owner
                    && old.obligation.id == new.obligation.id
                    && old.obligation.class == new.obligation.class
                    && old.canonical_certificate == new.canonical_certificate
                    && substitute_proposition_values(&old.obligation.proposition, substitutions)
                        == new.obligation.proposition
                    && old
                        .requirements
                        .iter()
                        .map(|fact| substitute_proposition_values(fact, substitutions))
                        .eq(new.requirements.iter().cloned())
                    && new.semantic_axioms.iter().all(|fact| {
                        old.semantic_axioms.iter().any(|old_fact| {
                            substitute_proposition_values(old_fact, substitutions) == *fact
                        })
                    })
            })
}

pub(super) fn transport(
    before: &TerminalModule,
    after: &TerminalModule,
    proof: &ProofBundle,
    substitutions: &Substitutions,
) -> Option<ProofBundle> {
    let old = reconstruct_optimizable_terminal_obligations(
        validate_module_for_optimization(before).ok()?,
    )
    .ok()?;
    let new =
        reconstruct_optimizable_terminal_obligations(validate_module_for_optimization(after).ok()?)
            .ok()?;
    if old == new {
        return Some(proof.clone());
    }
    if !questions_match(&old, &new, substitutions) {
        return None;
    }
    verify_module_for_optimization(before, proof, &AdmissionProfile::default()).ok()?;
    let mut transported = proof.clone();
    for evidence in &mut transported.evidence {
        let old_question = old
            .obligations()
            .iter()
            .find(|site| site.obligation.id == evidence.obligation)?;
        let new_question = new
            .obligations()
            .iter()
            .find(|site| site.obligation.id == evidence.obligation)?;
        let axioms = old_question
            .semantic_axioms
            .iter()
            .map(|fact| substitute_proposition_values(fact, substitutions))
            .collect::<Vec<_>>();
        let mapping = axioms
            .iter()
            .map(|fact| {
                new_question
                    .semantic_axioms
                    .iter()
                    .position(|candidate| candidate == fact)
            })
            .collect::<Vec<_>>();
        if let EvidenceRoute::CertificateDerived(certificate) = &mut evidence.route {
            transport_node(&mut certificate.proof, substitutions, &mapping)?;
        }
    }
    // Recursive components, cycle witnesses, admission records, and crash rows
    // remain exact. Their own replay is the refusal boundary if an incidental
    // removed definition changed a context they consume.
    verify_module_for_optimization(after, &transported, &AdmissionProfile::default()).ok()?;
    Some(transported)
}

fn index(index: &mut usize, mapping: &[Option<usize>]) -> Option<()> {
    *index = mapping.get(*index).copied().flatten()?;
    Some(())
}

fn term(term: &mut ScalarTerm, substitutions: &Substitutions) {
    // Reuse the exhaustive retained-proposition substitution owner rather than
    // introducing another recursive scalar-term vocabulary in the optimizer.
    let relation = Proposition::Equal(term.clone(), term.clone());
    let Proposition::Equal(mapped, _) = substitute_proposition_values(&relation, substitutions)
    else {
        unreachable!("scalar substitution preserves equality shape")
    };
    *term = mapped;
}

fn transport_node(
    node: &mut ProofNode,
    substitutions: &Substitutions,
    mapping: &[Option<usize>],
) -> Option<()> {
    node.conclusion = substitute_proposition_values(&node.conclusion, substitutions);
    match &mut node.rule {
        ProofRule::Primitive(_) | ProofRule::Assumption { .. } => {}
        ProofRule::SemanticAxiom { index: citation } => {
            if index(citation, mapping).is_none() {
                if matches!(&node.conclusion, Proposition::Equal(left, right) if left == right) {
                    node.rule =
                        ProofRule::Primitive(terminal_psi::PrimitiveJudgment::ReflexiveEquality);
                } else {
                    return None;
                }
            }
        }
        ProofRule::ValueEqualityTransport {
            premise,
            equalities,
        } => {
            transport_node(premise, substitutions, mapping)?;
            for equality in equalities {
                transport_node(equality, substitutions, mapping)?;
            }
        }
        ProofRule::PredicateDenotation { premise }
        | ProofRule::DisjunctionIntroduction {
            disjunct: premise, ..
        }
        | ProofRule::ConjunctionElimination {
            conjunction: premise,
            ..
        }
        | ProofRule::ImplicationIntroduction { body: premise }
        | ProofRule::EqualitySymmetry { equality: premise }
        | ProofRule::IntegerOrderWeakening { relation: premise }
        | ProofRule::IntegerOrderDiscreteness { relation: premise } => {
            transport_node(premise, substitutions, mapping)?;
        }
        ProofRule::ConjunctionIntroduction(children) => {
            for child in children {
                transport_node(child, substitutions, mapping)?;
            }
        }
        ProofRule::DisjunctionElimination {
            disjunction,
            branches,
        } => {
            transport_node(disjunction, substitutions, mapping)?;
            for branch in branches {
                transport_node(branch, substitutions, mapping)?;
            }
        }
        ProofRule::ImplicationElimination {
            implication: left,
            premise: right,
        }
        | ProofRule::EqualityTransitivity {
            left_equals_middle: left,
            middle_equals_right: right,
        }
        | ProofRule::IntegerOrderAntisymmetry {
            forward: left,
            backward: right,
        }
        | ProofRule::IntegerSubtractOrder {
            difference: left,
            positive: right,
        }
        | ProofRule::IntegerAddOrder {
            sum: left,
            positive: right,
        }
        | ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle: left,
            middle_less_or_equal_right: right,
        }
        | ProofRule::IntegerStrictOrderTransitivity {
            left_to_middle: left,
            middle_to_right: right,
        }
        | ProofRule::IntegerOrderSubstitution {
            relation: left,
            equality: right,
            ..
        } => {
            transport_node(left, substitutions, mapping)?;
            transport_node(right, substitutions, mapping)?;
        }
        ProofRule::IntegerSubtractAntitone {
            smaller,
            larger,
            order,
        } => {
            for child in [smaller, larger, order] {
                transport_node(child, substitutions, mapping)?;
            }
        }
        ProofRule::IntegerMathOrderTranslation { relation, addend } => {
            transport_node(relation, substitutions, mapping)?;
            // The translation witness must follow the same exact value
            // substitution as its conclusion and independently proved child.
            let equality = Proposition::IntegerMathEqual(addend.clone(), addend.clone());
            let Proposition::IntegerMathEqual(mapped, _) =
                substitute_proposition_values(&equality, substitutions)
            else {
                unreachable!("mathematical substitution preserves equality shape")
            };
            *addend = mapped;
        }
        ProofRule::IntegerAffineBound {
            root_bound,
            witness,
        } => {
            transport_node(root_bound, substitutions, mapping)?;
            term(&mut witness.root, substitutions);
            term(&mut witness.target, substitutions);
            for citation in &mut witness.definition_axioms {
                index(citation, mapping)?;
            }
            for citation in witness.literal_axioms.iter_mut().flatten() {
                index(citation, mapping)?;
            }
        }
        ProofRule::IntegerCastBound {
            root_bound,
            witness,
        } => {
            transport_node(root_bound, substitutions, mapping)?;
            term(&mut witness.root, substitutions);
            term(&mut witness.target, substitutions);
            for citation in &mut witness.definition_axioms {
                index(citation, mapping)?;
            }
        }
        ProofRule::IntegerExactAddDefinitionBound {
            left_bound,
            right_bound,
            definition_axiom,
        }
        | ProofRule::IntegerExactSubtractDefinitionBound {
            left_bound,
            right_bound,
            definition_axiom,
        } => {
            transport_node(left_bound, substitutions, mapping)?;
            transport_node(right_bound, substitutions, mapping)?;
            index(definition_axiom, mapping)?;
        }
        ProofRule::IntegerCorrelatedForbiddenRoots { witness } => {
            term(&mut witness.dividend.root, substitutions);
            term(&mut witness.dividend.target, substitutions);
            term(&mut witness.divisor.root, substitutions);
            term(&mut witness.divisor.target, substitutions);
            for branch in [&mut witness.dividend, &mut witness.divisor] {
                for step in &mut branch.steps {
                    index(&mut step.definition_axiom, mapping)?;
                    if let Some(citation) = &mut step.literal_axiom {
                        index(citation, mapping)?;
                    }
                }
            }
            witness.definition_axiom_count = mapping
                .get(..witness.definition_axiom_count)?
                .iter()
                .flatten()
                .max()
                .map_or(0, |last| last + 1);
            index(&mut witness.lower_bound_axiom, mapping)?;
            index(&mut witness.upper_bound_axiom, mapping)?;
            witness.conclusion = substitute_proposition_values(&witness.conclusion, substitutions);
        }
    }
    Some(())
}
