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

use std::rc::Rc;

use proof_admission::{
    IntegerAffineWitness, ProofNode, ProofRule, check_certificate, check_integer_affine_witness,
    integer_affine_wrapping_evidence, map_integer_affine_bound,
};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm};

use super::super::affine_custody::{self, CheckedWord, DefinitionIndex};
use super::super::integer_evidence::{projected_facts, relax};
use super::{bound, exact, range};

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
    // Every endpoint below resolves its aliases against this same unchanged
    // roster; one session keeps each lookup proportional to the endpoint's
    // own incident equalities instead of rescanning the roster per endpoint.
    let session = exact::session(context, assumptions, semantic_axioms);
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
            for (root, equality) in value_aliases(&session, endpoint) {
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
fn value_aliases(session: &exact::Session, endpoint: &ScalarTerm) -> Vec<(ScalarTerm, ProofNode)> {
    let mut aliases = session.value_aliases(endpoint);
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
    // Operation-semantic bounds are not cited in the source roster: a
    // remainder's `value < divisor` range exists only through its own
    // definition. Seed the same transport with each definition target's
    // synthesized bounds so chains like `48 + (x % 10) <= N` carry the
    // operand's derived range to the goal.
    let mut bounds = rooted_bounds(context, assumptions, semantic_axioms);
    bounds.extend(synthesized_operand_bounds(context, semantic_axioms));
    for bound in bounds {
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

/// Bounds each definition target earns from its operation semantics alone:
/// `x % 10` lands in `[0, 9]` without any cited relation. The transport
/// treats these like cited roots so wrapping chains carry them onward.
fn synthesized_operand_bounds(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
) -> Vec<RootedBound> {
    let mut bounds = Vec::new();
    for proposition in semantic_axioms {
        let Proposition::Equal(left, right) = proposition else {
            continue;
        };
        for candidate in [left, right] {
            if !matches!(candidate, ScalarTerm::Value { .. }) {
                continue;
            }
            for proof in range::target_bounds(context, candidate, semantic_axioms) {
                bounds.push(RootedBound {
                    proposition: proof.conclusion.clone(),
                    proof,
                    root: candidate.clone(),
                });
            }
        }
    }
    bounds
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
        // A mapped bound like `v <= 57` closes the goal `v <= 255` through
        // transitivity with a closed literal relation.
        let relaxed = relax(goal, proof)?;
        return check_certificate(context, goal, assumptions, semantic_axioms, &relaxed)
            .is_ok()
            .then_some(relaxed);
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
    let checked = check_word(
        context,
        semantic_axioms,
        definitions,
        bound,
        target,
        definition_axioms,
    )?;
    if checked.evidence.is_empty()
        && !matches!(bound.proposition, Proposition::LessThan(_, _))
        && !definition_axioms.iter().any(|&index| {
            let Proposition::Equal(left, right) = &semantic_axioms[index] else {
                return false;
            };
            [left, right].iter().any(|term| is_wrapping_term(term))
        })
    {
        // A bare non-strict relation is already the ordinary affine path's
        // root bound. This leg exists for strict roots and for the wrapping
        // evidence conjunction the ordinary path cannot construct. A word
        // containing a wrapping definition has no affine coverage either:
        // its unconditional transports (an operand's derived range into the
        // target's upper bound) land only through this leg.
        return None;
    }
    map_checked_word(
        context,
        assumptions,
        semantic_axioms,
        definitions,
        bound,
        &checked,
        target,
        definition_axioms,
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
    let checked = check_word(
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
        &checked,
        target,
        definition_axioms,
    )
}

/// The kernel witness check for one `(bound, root, target, word)`, memoized
/// in the definition index: both the goal-directed path and the
/// contradiction leg pool meet the same candidates, so the check runs once
/// per word under this scope and each caller still assembles its own proof.
#[allow(clippy::too_many_arguments)]
fn check_word(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    bound: &RootedBound,
    target: &ScalarTerm,
    definition_axioms: &[usize],
) -> Option<Rc<CheckedWord>> {
    if let Some(checked) =
        definitions.cached_checked_word(&bound.proposition, &bound.root, target, definition_axioms)
    {
        return checked;
    }
    let checked = (|definitions: &mut DefinitionIndex| {
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
        Some(Rc::new(CheckedWord {
            form,
            evidence,
            literal_axioms: witness.literal_axioms,
        }))
    })(definitions);
    definitions.cache_checked_word(
        &bound.proposition,
        &bound.root,
        target,
        definition_axioms,
        checked.clone(),
    );
    checked
}

#[allow(clippy::too_many_arguments)]
fn map_checked_word(
    context: &PropositionContext,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    bound: &RootedBound,
    checked: &CheckedWord,
    target: &ScalarTerm,
    definition_axioms: &[usize],
) -> Option<ProofNode> {
    let witness = IntegerAffineWitness {
        root: bound.root.clone(),
        target: target.clone(),
        literal_axioms: checked.literal_axioms.clone(),
        definition_axioms: definition_axioms.to_vec(),
    };
    let mut children = Vec::with_capacity(checked.evidence.len() + 1);
    children.push(bound.proof.clone());
    for required in &checked.evidence {
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
    let mapped = map_integer_affine_bound(&checked.form, &root_bound.conclusion).ok()?;
    Some(ProofNode {
        conclusion: mapped,
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(root_bound),
            witness,
        },
    })
}

fn is_wrapping_term(term: &ScalarTerm) -> bool {
    matches!(
        term,
        ScalarTerm::WrappingIntegerAdd { .. }
            | ScalarTerm::WrappingIntegerSubtract { .. }
            | ScalarTerm::WrappingIntegerMultiply { .. }
            | ScalarTerm::WrappingIntegerDivide { .. }
            | ScalarTerm::WrappingIntegerRemainder { .. }
            | ScalarTerm::WrappingIntegerShiftLeft { .. }
            | ScalarTerm::WrappingIntegerShiftRight { .. }
    )
}
