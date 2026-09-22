//! Canonical bound transport through unsigned wrapping definitions.
//!
//! `WrappingIntegerAdd`/`WrappingIntegerDivide` rows are affine in shape but
//! modular in semantics. Unsigned wrapping division maps bounds exactly;
//! wrapping addition maps an upper bound unconditionally while a lower bound
//! is sound only when the same definition supplies the checked no-wrap
//! conjunct `operand <= maximum - addend`. Traversed toward its operand the
//! equation inverts those directions. The proof kernel independently
//! reconstructs every requirement at admission, so this leg only assembles
//! the candidate root-bound conjunction from cited facts.

use proof_admission::{
    IntegerAffineWitness, ProofNode, ProofRule, check_certificate, check_integer_affine_witness,
    integer_affine_wrapping_evidence, map_integer_affine_bound,
};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm};

use super::super::affine_custody::{self, DefinitionIndex};
use super::super::integer_evidence::projected_facts;
use super::{bound, exact};

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    if let Some(proof) = definitions.cached_wrapping_proof(goal) {
        return proof;
    }
    definitions.begin_wrapping_proof(goal);
    let proof = prove_uncached(context, goal, assumptions, semantic_axioms, definitions);
    definitions.cache_wrapping_proof(goal, proof.clone());
    proof
}

/// One order bound standing on a value term: either cited on the value itself
/// or transported there through a cited equality. A guard fact can name a
/// storage observation (`self.counter < 3`) while the definition chain roots
/// at the stored value; the checked endpoint substitution keeps the bound
/// exact instead of inventing a second citation.
pub(super) struct RootedBound {
    /// The bound proposition restated on `root`.
    pub(super) proposition: Proposition,
    pub(super) proof: ProofNode,
    pub(super) root: ScalarTerm,
}

pub(super) fn rooted_bounds(
    context: &PropositionContext,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Vec<RootedBound> {
    let mut bounds = Vec::new();
    for fact in projected_facts(assumptions, semantic_axioms) {
        let (fact_left, fact_right) = match fact.proposition {
            Proposition::LessThan(left, right) | Proposition::LessOrEqual(left, right) => {
                (left, right)
            }
            _ => continue,
        };
        for (endpoint, position) in [(fact_left, 0usize), (fact_right, 1)] {
            if matches!(
                endpoint,
                ScalarTerm::Integer { .. } | ScalarTerm::Boolean(_)
            ) {
                continue;
            }
            for (root, equality) in value_aliases(context, endpoint, assumptions, semantic_axioms) {
                let (proposition, proof) = if root == *endpoint {
                    (fact.proposition.clone(), fact.proof())
                } else {
                    let (left, right) = if position == 0 {
                        (root.clone(), fact_right.clone())
                    } else {
                        (fact_left.clone(), root.clone())
                    };
                    let proposition = match fact.proposition {
                        Proposition::LessThan(..) => Proposition::LessThan(left, right),
                        _ => Proposition::LessOrEqual(left, right),
                    };
                    (
                        proposition.clone(),
                        ProofNode {
                            conclusion: proposition,
                            rule: ProofRule::IntegerOrderSubstitution {
                                relation: Box::new(fact.proof()),
                                equality: Box::new(equality),
                                endpoint: position,
                            },
                        },
                    )
                };
                bounds.push(RootedBound {
                    proposition,
                    proof,
                    root,
                });
            }
        }
    }
    bounds
}

/// The value terms one endpoint can stand on through cited equalities: itself
/// when it already is a value, plus every cited `endpoint == value` or
/// `value == endpoint` partner. Each alias carries the equality certificate the
/// endpoint substitution must cite.
fn value_aliases(
    context: &PropositionContext,
    endpoint: &ScalarTerm,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Vec<(ScalarTerm, ProofNode)> {
    let mut aliases = Vec::new();
    for fact in projected_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(left, right) = fact.proposition else {
            continue;
        };
        let alias = if left == endpoint {
            right
        } else if right == endpoint {
            left
        } else {
            continue;
        };
        if !matches!(alias, ScalarTerm::Value { .. })
            || aliases.iter().any(|(root, _)| root == alias)
        {
            continue;
        }
        let Some(equality) = exact::prove(
            context,
            &Proposition::Equal(endpoint.clone(), alias.clone()),
            assumptions,
            semantic_axioms,
        ) else {
            continue;
        };
        aliases.push((alias.clone(), equality));
    }
    if matches!(endpoint, ScalarTerm::Value { .. })
        && !aliases.iter().any(|(root, _)| root == endpoint)
    {
        aliases.push((
            endpoint.clone(),
            ProofNode {
                conclusion: Proposition::Equal(endpoint.clone(), endpoint.clone()),
                rule: ProofRule::Primitive(proof_admission::PrimitiveJudgment::ReflexiveEquality),
            },
        ));
    }
    aliases
}

fn prove_uncached(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    let (goal_left, goal_right) = match goal {
        Proposition::LessThan(left, right) | Proposition::LessOrEqual(left, right) => (left, right),
        _ => return None,
    };
    for bound in rooted_bounds(context, assumptions, semantic_axioms) {
        for target in [goal_left, goal_right] {
            if bound.root == *target || !matches!(target, ScalarTerm::Value { .. }) {
                continue;
            }
            let words = affine_custody::definition_words_to_target(
                context,
                semantic_axioms,
                definitions,
                &bound.root,
                target,
            );
            for word in words.iter() {
                if let Some(proof) = prove_word(
                    context,
                    goal,
                    assumptions,
                    semantic_axioms,
                    definitions,
                    &bound,
                    target,
                    word,
                ) {
                    return Some(proof);
                }
            }
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn prove_word(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    bound: &RootedBound,
    target: &ScalarTerm,
    definition_axioms: &[usize],
) -> Option<ProofNode> {
    let proof = map_word(
        context,
        assumptions,
        semantic_axioms,
        definitions,
        bound,
        target,
        definition_axioms,
    )?;
    if proof.conclusion != *goal {
        return None;
    }
    check_certificate(context, goal, assumptions, semantic_axioms, &proof)
        .is_ok()
        .then_some(proof)
}

/// Map `bound` through `definition_axioms` to `target`, returning the
/// transport proof whose conclusion is the mapped relation. The goal check
/// stays with the caller so contradiction search can reuse one mapped bound
/// that no `prove` goal names.
#[allow(clippy::too_many_arguments)]
pub(super) fn map_word(
    context: &PropositionContext,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    bound: &RootedBound,
    target: &ScalarTerm,
    definition_axioms: &[usize],
) -> Option<ProofNode> {
    let (form, evidence, witness) = check_word(
        context,
        semantic_axioms,
        definitions,
        bound,
        target,
        definition_axioms,
    )?;
    if evidence.is_empty() && !matches!(bound.proposition, Proposition::LessThan(_, _)) {
        // A bare non-strict relation is already the ordinary affine path's
        // root bound. This leg exists for strict roots and for the wrapping
        // evidence conjunction the ordinary path cannot construct.
        return None;
    }
    map_checked_word(
        context,
        assumptions,
        semantic_axioms,
        definitions,
        bound,
        &form,
        &evidence,
        witness,
    )
}

/// The contradiction leg pool has no goal-directed ordinary path that already
/// produces these relations: a carrier root such as `0 <= counter` mapped
/// through `next = counter + 1` is exactly the `1 <= next` leg the strict
/// premise contradicts. Map every cited bound here; the de-duplication gate
/// above only belongs to the goal-checking caller.
#[allow(clippy::too_many_arguments)]
pub(super) fn map_derived_word(
    context: &PropositionContext,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    bound: &RootedBound,
    target: &ScalarTerm,
    definition_axioms: &[usize],
) -> Option<ProofNode> {
    let (form, evidence, witness) = check_word(
        context,
        semantic_axioms,
        definitions,
        bound,
        target,
        definition_axioms,
    )?;
    map_checked_word(
        context,
        assumptions,
        semantic_axioms,
        definitions,
        bound,
        &form,
        &evidence,
        witness,
    )
}

#[allow(clippy::too_many_arguments)]
fn check_word(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    bound: &RootedBound,
    target: &ScalarTerm,
    definition_axioms: &[usize],
) -> Option<(
    proof_admission::CheckedIntegerAffineForm,
    Vec<Proposition>,
    IntegerAffineWitness,
)> {
    let literal_axioms = affine_custody::literal_axioms(
        context,
        semantic_axioms,
        definitions,
        &bound.root,
        definition_axioms,
        target,
    )?;
    let witness = IntegerAffineWitness {
        root: bound.root.clone(),
        target: target.clone(),
        literal_axioms,
        definition_axioms: definition_axioms.to_vec(),
    };
    let form = check_integer_affine_witness(context, semantic_axioms, &witness).ok()?;
    let evidence = integer_affine_wrapping_evidence(&form, &bound.proposition).ok()?;
    Some((form, evidence, witness))
}

#[allow(clippy::too_many_arguments)]
fn map_checked_word(
    context: &PropositionContext,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    bound: &RootedBound,
    form: &proof_admission::CheckedIntegerAffineForm,
    evidence: &[Proposition],
    witness: IntegerAffineWitness,
) -> Option<ProofNode> {
    let mut children = Vec::with_capacity(evidence.len() + 1);
    children.push(bound.proof.clone());
    for required in evidence {
        children.push(bound::prove_candidate_endpoint(
            context,
            required,
            assumptions,
            semantic_axioms,
            definitions,
        )?);
    }
    let root_bound = if children.len() == 1 {
        children
            .pop()
            .unwrap_or_else(|| unreachable!("one cited relation"))
    } else {
        ProofNode {
            conclusion: Proposition::Conjunction(
                children
                    .iter()
                    .map(|child| child.conclusion.clone())
                    .collect(),
            ),
            rule: ProofRule::ConjunctionIntroduction(children),
        }
    };
    let mapped = map_integer_affine_bound(form, &root_bound.conclusion).ok()?;
    Some(ProofNode {
        conclusion: mapped,
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(root_bound),
            witness,
        },
    })
}
