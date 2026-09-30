//! Root sources and compositions that cross one exact cast spine.
//!
//! The root sources here start from a goal endpoint defined by an exact cast
//! spine and propose a bound on the spine's non-cast source. The
//! cast-adjacent compositions run the other way: an affine word resumes after
//! the spine's last cast, so cast custody first carries a cited bound onto the
//! cast root and affine custody then finishes from there.

use proof_admission::{PrimitiveJudgment, ProofNode, ProofRule, lift_fixed_integer_relation};
use semantic_vocabulary::{IntegerMathTerm, IntegerType, Proposition, ScalarTerm, ScalarType};

use super::super::affine_custody::{self, DefinitionIndex};
use super::super::integer_evidence::projected_facts;
use super::super::{cast_custody, integer_selection};
use super::literal::goal_literal_at;
use super::roster::{is_value, ordered_at};
use super::{Complete, Request};

/// Preserve a guarded live endpoint across an independently checked cast
/// word.
///
/// A byte index is widened after its mixed-width source comparison. The
/// original comparison and later storage reads must first rejoin through
/// retained equalities; the cast witness then moves just that endpoint. No
/// guessed length or source-shape bound substitutes for the original guard.
pub(super) fn relational_guard(request: Request, complete: &mut Complete) -> Option<ProofNode> {
    let (strict, left, right) = match request.goal {
        Proposition::LessThan(left, right) => (true, left, right),
        Proposition::LessOrEqual(left, right) => (false, left, right),
        _ => return None,
    };
    for (endpoint, target) in [left, right].into_iter().enumerate() {
        let Some((root, _)) = cast_custody::source_root(target, request.semantic_axioms) else {
            continue;
        };
        let (root_left, root_right) = if endpoint == 0 {
            (root.clone(), right.clone())
        } else {
            (left.clone(), root.clone())
        };
        let scalar_shape = if strict {
            Proposition::LessThan(root_left, root_right)
        } else {
            Proposition::LessOrEqual(root_left, root_right)
        };
        // Lift without pretending the mixed-width scalar shape is a valid
        // scalar proposition. Only the mathematical relation is published.
        let Some(root_goal) = lift_fixed_integer_relation(&scalar_shape) else {
            continue;
        };
        let Some(root_bound) = rejoin_guard(request, &root_goal) else {
            continue;
        };
        if let Some(proof) = complete(&root, root_bound) {
            return Some(proof);
        }
    }
    None
}

fn rejoin_guard(request: Request, goal: &Proposition) -> Option<ProofNode> {
    let (strict, goal_left, goal_right) = order(goal)?;
    for fact in projected_facts(request.assumptions, request.semantic_axioms) {
        let Some((fact_strict, _, _)) = order(fact.proposition) else {
            continue;
        };
        if strict != fact_strict {
            continue;
        }
        let mut proof = fact.proof();
        for (endpoint, target) in [goal_left, goal_right].into_iter().enumerate() {
            let (_, left, right) = order(&proof.conclusion)?;
            let old = if endpoint == 0 { left } else { right };
            if old == target {
                continue;
            }
            let (Some(old_scalar), Some(target_scalar)) = (scalar_value(old), scalar_value(target))
            else {
                break;
            };
            let equality_goal = Proposition::Equal(old_scalar, target_scalar.clone());
            let Some(equality) = integer_selection::build(
                request.context,
                &equality_goal,
                request.assumptions,
                request.semantic_axioms,
            ) else {
                break;
            };
            let Some(mathematical_equality) = lift_fixed_integer_relation(&equality_goal) else {
                break;
            };
            // A checked transitivity node lifts the scalar equality without
            // relabeling an assumption or conjunction-projection certificate.
            let equality = ProofNode {
                conclusion: mathematical_equality,
                rule: ProofRule::EqualityTransitivity {
                    left_equals_middle: Box::new(equality),
                    middle_equals_right: Box::new(ProofNode {
                        conclusion: Proposition::Equal(target_scalar.clone(), target_scalar),
                        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
                    }),
                },
            };
            let (left, right) = if endpoint == 0 {
                (target.clone(), right.clone())
            } else {
                (left.clone(), target.clone())
            };
            let conclusion = if strict {
                Proposition::IntegerMathLessThan(left, right)
            } else {
                Proposition::IntegerMathLessOrEqual(left, right)
            };
            proof = ProofNode {
                conclusion,
                rule: ProofRule::IntegerOrderSubstitution {
                    relation: Box::new(proof),
                    equality: Box::new(equality),
                    endpoint,
                },
            };
        }
        if proof.conclusion == *goal {
            return Some(proof);
        }
    }
    None
}

fn order(proposition: &Proposition) -> Option<(bool, &IntegerMathTerm, &IntegerMathTerm)> {
    match proposition {
        Proposition::IntegerMathLessThan(left, right) => Some((true, left, right)),
        Proposition::IntegerMathLessOrEqual(left, right) => Some((false, left, right)),
        _ => None,
    }
}

fn scalar_value(term: &IntegerMathTerm) -> Option<ScalarTerm> {
    let IntegerMathTerm::MathValue { source_type, value } = term else {
        return None;
    };
    Some(ScalarTerm::value(*value, ScalarType::Integer(*source_type)))
}

/// The cast chain's own carrier bound: `Truth` at the non-cast source of each
/// value goal endpoint, left first.
pub(super) fn cast_truth(request: Request, complete: &mut Complete) -> Option<ProofNode> {
    let Proposition::LessOrEqual(left, right) = request.goal else {
        return None;
    };
    [left, right]
        .into_iter()
        .filter(|target| is_value(target))
        .find_map(|target| {
            let (root, _) = cast_custody::source_root(target, request.semantic_axioms)?;
            let truth = ProofNode {
                conclusion: Proposition::Truth,
                rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
            };
            complete(&root, truth)
        })
}

/// The goal's literal bound proved on the non-cast source of each value goal
/// endpoint, right first, by the full integer selection restricted to the
/// semantic axioms before the spine's first cast.
pub(super) fn source_side_bound(request: Request, complete: &mut Complete) -> Option<ProofNode> {
    let Proposition::LessOrEqual(left, right) = request.goal else {
        return None;
    };
    [(1, right), (0, left)]
        .into_iter()
        .find_map(|(endpoint, target)| {
            if !is_value(target) {
                return None;
            }
            let (source, first_cast) = cast_custody::source_root(target, request.semantic_axioms)?;
            let goal_literal = goal_literal_at(request.goal, &source, endpoint)?;
            let root_bound = integer_selection::build(
                request.context,
                &ordered_at(&source, &goal_literal, endpoint),
                request.assumptions,
                &request.semantic_axioms[..first_cast],
            )?;
            complete(&source, root_bound)
        })
}

/// A cited assumption bound on a cast spine's source, carried onto the cast
/// root by cast custody and finished by an affine word that starts after the
/// spine's last cast.
pub(super) fn prove_after_cast_spine(
    request: Request,
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    let cast_roots = definitions.cast_roots();
    cast_roots.iter().find_map(|cast_root| {
        let ScalarType::Integer(cast_type) = cast_root.scalar_type() else {
            return None;
        };
        let (source, cast_word) = definitions.cast_spine(cast_root)?;
        let last_cast = *cast_word.last()?;
        if !has_target_after(request, definitions, cast_root, last_cast) {
            return None;
        }
        request
            .assumptions
            .iter()
            .enumerate()
            .find_map(|(assumption, root_bound)| {
                let cast_goal = cast_goal(root_bound, &source, cast_root, cast_type)?;
                let cast_bound = cast_custody::prove_from_root(
                    request.context,
                    &cast_goal,
                    request.assumptions,
                    request.semantic_axioms,
                    &source,
                    ProofNode {
                        conclusion: root_bound.clone(),
                        rule: ProofRule::Assumption { index: assumption },
                    },
                )?;
                prove_from_cast_root(request, definitions, cast_root, last_cast, cast_bound)
            })
    })
}

/// An affine/cast/affine sandwich: a cited assumption bound on a value maps
/// through an affine word ending before the spine's first cast onto the
/// spine's source, crosses the spine by cast custody, and finishes through an
/// affine word that starts after the spine's last cast.
pub(super) fn prove_across_cast_spine(
    request: Request,
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    let cast_roots = definitions.cast_roots();
    cast_roots.iter().find_map(|cast_root| {
        let ScalarType::Integer(cast_type) = cast_root.scalar_type() else {
            return None;
        };
        let (source, cast_word) = definitions.cast_spine(cast_root)?;
        let first_cast = cast_word[0];
        let last_cast = *cast_word.last()?;
        if !has_target_after(request, definitions, cast_root, last_cast) {
            return None;
        }
        request
            .assumptions
            .iter()
            .enumerate()
            .find_map(|(assumption, root_bound)| {
                let Proposition::LessOrEqual(left, right) = root_bound else {
                    return None;
                };
                [left, right].into_iter().find_map(|root| {
                    if !is_value(root) || root == &source {
                        return None;
                    }
                    if !affine_custody::has_target_before(
                        request.context,
                        request.semantic_axioms,
                        definitions,
                        root,
                        &source,
                        first_cast,
                    ) {
                        return None;
                    }
                    let source_bound = affine_custody::prove_mapped_to_target_before(
                        request.context,
                        request.assumptions,
                        request.semantic_axioms,
                        definitions,
                        root,
                        &source,
                        first_cast,
                        &ProofNode {
                            conclusion: root_bound.clone(),
                            rule: ProofRule::Assumption { index: assumption },
                        },
                    )?;
                    let cast_goal =
                        cast_goal(&source_bound.conclusion, &source, cast_root, cast_type)?;
                    let cast_bound = cast_custody::prove_from_root(
                        request.context,
                        &cast_goal,
                        request.assumptions,
                        request.semantic_axioms,
                        &source,
                        source_bound,
                    )?;
                    prove_from_cast_root(request, definitions, cast_root, last_cast, cast_bound)
                })
            })
    })
}

fn has_target_after(
    request: Request,
    definitions: &mut DefinitionIndex,
    cast_root: &ScalarTerm,
    last_cast: usize,
) -> bool {
    affine_custody::has_target_after(
        request.context,
        request.goal,
        request.semantic_axioms,
        definitions,
        cast_root,
        last_cast,
    )
}

/// The bound `source_bound` states about the spine's `source`, restated about
/// `cast_root` with its literal side remapped into the cast root's carrier.
fn cast_goal(
    source_bound: &Proposition,
    source: &ScalarTerm,
    cast_root: &ScalarTerm,
    cast_type: IntegerType,
) -> Option<Proposition> {
    let Proposition::LessOrEqual(left, right) = source_bound else {
        return None;
    };
    if left == source {
        Some(Proposition::LessOrEqual(
            cast_root.clone(),
            cast_custody::remap_integer_literal(right, cast_type)?,
        ))
    } else if right == source {
        Some(Proposition::LessOrEqual(
            cast_custody::remap_integer_literal(left, cast_type)?,
            cast_root.clone(),
        ))
    } else {
        None
    }
}

fn prove_from_cast_root(
    request: Request,
    definitions: &mut DefinitionIndex,
    cast_root: &ScalarTerm,
    last_cast: usize,
    cast_bound: ProofNode,
) -> Option<ProofNode> {
    affine_custody::prove_from_root_after(
        request.context,
        request.goal,
        request.assumptions,
        request.semantic_axioms,
        definitions,
        cast_root,
        last_cast,
        cast_bound,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use proof_admission::check_certificate;
    use semantic_vocabulary::{IntegerSign, PropositionContext, ValueId};

    #[test]
    fn mixed_width_guard_rejoins_widened_index_and_reloaded_length() {
        let narrow = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
        let wide = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let integer_type = |identity| if identity <= 3 { narrow } else { wide };
        let value = |identity| {
            ScalarTerm::value(
                ValueId::new(identity).unwrap(),
                ScalarType::Integer(integer_type(identity)),
            )
        };
        let mathematical = |identity| IntegerMathTerm::MathValue {
            source_type: integer_type(identity),
            value: ValueId::new(identity).unwrap(),
        };
        let context = PropositionContext::from_value_types((1..=7).map(|identity| {
            (
                ValueId::new(identity).unwrap(),
                ScalarType::Integer(integer_type(identity)),
            )
        }))
        .unwrap();
        let axioms = vec![
            Proposition::Equal(value(1), value(3)),
            Proposition::Equal(value(2), value(3)),
            Proposition::Equal(value(4), value(7)),
            Proposition::Equal(value(5), value(7)),
            Proposition::IntegerMathLessThan(mathematical(1), mathematical(4)),
            Proposition::Equal(
                value(6),
                ScalarTerm::integer_widen(narrow, wide, value(2)).unwrap(),
            ),
        ];
        let goal = Proposition::LessThan(value(6), value(5));
        let produce = |facts: &[Proposition]| {
            crate::proofs::nonzero_divisor_certificate::prove_canonical_integer_proposition(
                &context,
                &goal,
                &[],
                facts,
            )
        };
        let proof = produce(&axioms)
            .expect("guarded index preserves both reloaded endpoints through widening");
        check_certificate(&context, &goal, &[], &axioms, &proof).unwrap();
        for missing in 0..axioms.len() {
            let mut incomplete = axioms.clone();
            incomplete[missing] = Proposition::Truth;
            assert!(produce(&incomplete).is_none(), "missing citation {missing}");
            assert!(check_certificate(&context, &goal, &[], &incomplete, &proof).is_err());
        }
        let mut inclusive = axioms.clone();
        inclusive[4] = Proposition::IntegerMathLessOrEqual(mathematical(1), mathematical(4));
        assert!(produce(&inclusive).is_none());
        assert!(check_certificate(&context, &goal, &[], &inclusive, &proof).is_err());
        let mut conditional = axioms.clone();
        conditional[4] = Proposition::Implication {
            premise: Box::new(Proposition::Falsehood),
            conclusion: Box::new(axioms[4].clone()),
        };
        assert!(produce(&conditional).is_none());
    }
}
