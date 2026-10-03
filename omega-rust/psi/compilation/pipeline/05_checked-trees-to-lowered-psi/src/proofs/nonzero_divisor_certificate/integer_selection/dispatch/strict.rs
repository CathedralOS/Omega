//! Strict order from exact endpoint equalities and checked discrete bounds.

use crate::proofs::nonzero_divisor_certificate::integer_evidence::literal_value;
use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::{
    IntegerCarrier, IntegerValue, Proposition, PropositionContext, ScalarTerm, ScalarType,
};

use super::super::super::affine_custody::DefinitionIndex;
use super::super::super::integer_evidence::{closed_integer_relation, projected_facts};
use super::super::{bound, exact, policy_translation};

mod antitone;
mod remainder;
mod subtract;
mod transitive;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    prove_without_subtract(context, goal, assumptions, semantic_axioms)
        .or_else(|| {
            super::super::super::root_selection::prove_relational_cast_bound(
                context,
                goal,
                assumptions,
                semantic_axioms,
            )
        })
        .or_else(|| remainder::prove(context, goal, assumptions, semantic_axioms, definitions))
        .or_else(|| subtract::prove(context, goal, assumptions, semantic_axioms, definitions))
        // Strict endpoints traverse checked wrapping-update chains that the
        // non-strict affine selection cannot cite.
        .or_else(|| {
            policy_translation::prove(context, goal, assumptions, semantic_axioms, definitions)
        })
        .or_else(|| {
            prove_discrete_endpoint(context, goal, assumptions, semantic_axioms, |nonstrict| {
                // Use bound selection directly: proposition dispatch itself
                // falls back to strict order and would recurse here.
                bound::prove(
                    context,
                    nonstrict,
                    assumptions,
                    semantic_axioms,
                    definitions,
                )
            })
        })
        .or_else(|| transitive::prove(context, goal, assumptions, semantic_axioms, definitions))
        // Relational steps come last, so every goal an earlier leg already
        // answered keeps its certificate.
        .or_else(|| antitone::increase(context, goal, assumptions, semantic_axioms))
        .or_else(|| antitone::prove(context, goal, assumptions, semantic_axioms, definitions))
}

/// A literal endpoint, including a separately observed equal value, turns a
/// strict goal into an adjacent non-strict bound. The caller supplies its bound
/// producer, retaining its own search scope and cast/definition evidence; this
/// shared conversion replays discreteness and the exact endpoint equality.
/// Canonical and relaxed selection must not diverge on adjacency or identity.
pub(in super::super) fn prove_discrete_endpoint(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    mut prove_bound: impl FnMut(&Proposition) -> Option<ProofNode>,
) -> Option<ProofNode> {
    let Proposition::LessThan(left, right) = goal else {
        return None;
    };
    // The endpoint/literal loops below ask many equality questions against
    // one unchanged cited roster; the session resolves that roster once.
    let session = exact::session(context, assumptions, semantic_axioms);
    let facts = projected_facts(assumptions, semantic_axioms);
    let mut literals = Vec::new();
    for candidate in [left, right].into_iter().chain(
        facts
            .iter()
            .filter_map(|fact| match fact.proposition {
                Proposition::Equal(left, right) => Some([left, right]),
                _ => None,
            })
            .flatten(),
    ) {
        if literal_value(candidate).is_some() && !literals.contains(candidate) {
            literals.push(candidate.clone());
        }
    }
    for (endpoint, target) in [left, right].into_iter().enumerate() {
        // A live length can be a closed definition such as two concatenated
        // extents. Retain its checked equation transport: cited leaf equality
        // alone cannot connect that computed endpoint to its literal value.
        let evaluated = session.literal_equality(target);
        let evaluated_literal = evaluated
            .as_ref()
            .and_then(|proof| match &proof.conclusion {
                Proposition::Equal(source, literal) if source == target => Some(literal),
                _ => None,
            });
        for literal in literals.iter().chain(evaluated_literal) {
            if literal.scalar_type() != target.scalar_type() {
                continue;
            }
            let Some(equality) = session.prove_equality(target, literal).or_else(|| {
                (evaluated_literal == Some(literal))
                    .then(|| evaluated.clone())
                    .flatten()
            }) else {
                continue;
            };
            let Some(adjacent) = adjacent(literal, endpoint == 0) else {
                continue;
            };
            let (nonstrict, strict) = if endpoint == 0 {
                (
                    Proposition::LessOrEqual(adjacent, right.clone()),
                    Proposition::LessThan(literal.clone(), right.clone()),
                )
            } else {
                (
                    Proposition::LessOrEqual(left.clone(), adjacent),
                    Proposition::LessThan(left.clone(), literal.clone()),
                )
            };
            let Some(relation) = prove_bound(&nonstrict) else {
                continue;
            };
            let discrete = ProofNode {
                conclusion: strict,
                rule: ProofRule::IntegerOrderDiscreteness {
                    relation: Box::new(relation),
                },
            };
            if discrete.conclusion == *goal {
                return Some(discrete);
            }
            return Some(ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::IntegerOrderSubstitution {
                    relation: Box::new(discrete),
                    equality: Box::new(equality),
                    endpoint,
                },
            });
        }
    }
    None
}

fn prove_without_subtract(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessThan(left, right) = goal else {
        return None;
    };
    let ScalarType::Integer(integer_type) = left.scalar_type() else {
        return None;
    };
    if integer_type.is_address() || right.scalar_type() != left.scalar_type() {
        return None;
    }
    if let Some(closed) = closed_integer_relation(goal.clone()) {
        return Some(closed);
    }
    // The fact walk and the literal search below ask many equality questions
    // against one unchanged cited roster; the session resolves it once.
    let session = exact::session(context, assumptions, semantic_axioms);
    let facts = projected_facts(assumptions, semantic_axioms);
    let mut equalities = Vec::new();
    for fact in facts.iter().rev() {
        match fact.proposition {
            Proposition::LessThan(_, _) => {
                if let Some(proof) =
                    complete_relation(&session, goal, fact.proposition, || fact.proof())
                {
                    return Some(proof);
                }
            }
            Proposition::LessOrEqual(left, right) => {
                let conclusions = [
                    adjacent(left, false)
                        .map(|previous| Proposition::LessThan(previous, right.clone())),
                    adjacent(right, true).map(|next| Proposition::LessThan(left.clone(), next)),
                ];
                for conclusion in conclusions.into_iter().flatten() {
                    let discrete = || ProofNode {
                        conclusion: conclusion.clone(),
                        rule: ProofRule::IntegerOrderDiscreteness {
                            relation: Box::new(fact.proof()),
                        },
                    };
                    if let Some(proof) = complete_relation(&session, goal, &conclusion, discrete) {
                        return Some(proof);
                    }
                }
            }
            Proposition::Equal(left, right) => equalities.push((left, right)),
            _ => {}
        }
    }

    let literal = |term: &ScalarTerm| {
        if literal_value(term).is_some() {
            return Some(term.clone());
        }
        equalities
            .iter()
            .flat_map(|(left, right)| [*left, *right])
            .find_map(|candidate| {
                (literal_value(candidate).is_some()
                    && candidate.scalar_type() == term.scalar_type())
                .then(|| session.prove_equality(term, candidate))
                .flatten()
                .map(|_| candidate.clone())
            })
    };
    let closed = closed_integer_relation(Proposition::LessThan(literal(left)?, literal(right)?))?;
    complete(&session, goal, closed)
}

pub(super) fn adjacent(literal: &ScalarTerm, increasing: bool) -> Option<ScalarTerm> {
    // This constructs the premise for a requested endpoint. Moving a literal
    // back inward must recover that exact endpoint; evaluating a compound
    // first would recover only its value and produce an invalid certificate.
    let ScalarTerm::Integer {
        scalar_type: integer_type,
        value,
    } = literal
    else {
        return None;
    };
    let (integer_type, value) = (*integer_type, *value);
    if integer_type.carrier() != IntegerCarrier::Fixed || integer_type.is_address() {
        return None;
    }
    let adjacent = match (value, increasing) {
        (IntegerValue::Signed(value), true) => IntegerValue::Signed(value.checked_add(1)?),
        (IntegerValue::Signed(value), false) => IntegerValue::Signed(value.checked_sub(1)?),
        (IntegerValue::Unsigned(value), true) => IntegerValue::Unsigned(value.checked_add(1)?),
        (IntegerValue::Unsigned(value), false) => IntegerValue::Unsigned(value.checked_sub(1)?),
    };
    ScalarTerm::integer(integer_type, adjacent).ok()
}

fn complete(session: &exact::Session, goal: &Proposition, proof: ProofNode) -> Option<ProofNode> {
    let equalities = endpoint_equalities(session, goal, &proof.conclusion)?;
    substitute_endpoints(goal, proof, equalities)
}

/// `complete` for a relation whose proof is built only once both endpoints
/// reach the goal: the substitutions depend on the relation alone.
fn complete_relation(
    session: &exact::Session,
    goal: &Proposition,
    relation: &Proposition,
    relation_proof: impl FnOnce() -> ProofNode,
) -> Option<ProofNode> {
    let equalities = endpoint_equalities(session, goal, relation)?;
    substitute_endpoints(goal, relation_proof(), equalities)
}

/// The endpoint equalities that carry a strict `relation` onto `goal`, left
/// endpoint first; `None` in a slot means that endpoint already matches.
/// Substituting the left endpoint leaves the right one in place, so both
/// steps are decided on the original relation.
fn endpoint_equalities(
    session: &exact::Session,
    goal: &Proposition,
    relation: &Proposition,
) -> Option<[Option<ProofNode>; 2]> {
    let Proposition::LessThan(goal_left, goal_right) = goal else {
        return None;
    };
    let Proposition::LessThan(left, right) = relation else {
        return None;
    };
    let mut equalities = [None, None];
    for (endpoint, (old, target)) in [(left, goal_left), (right, goal_right)]
        .into_iter()
        .enumerate()
    {
        if old != target {
            equalities[endpoint] = Some(session.prove_equality(old, target)?);
        }
    }
    Some(equalities)
}

fn substitute_endpoints(
    goal: &Proposition,
    mut proof: ProofNode,
    equalities: [Option<ProofNode>; 2],
) -> Option<ProofNode> {
    let Proposition::LessThan(goal_left, goal_right) = goal else {
        return None;
    };
    for (endpoint, (target, equality)) in [goal_left, goal_right]
        .into_iter()
        .zip(equalities)
        .enumerate()
    {
        let Some(equality) = equality else {
            continue;
        };
        let Proposition::LessThan(left, right) = &proof.conclusion else {
            return None;
        };
        let conclusion = if endpoint == 0 {
            Proposition::LessThan(target.clone(), right.clone())
        } else {
            Proposition::LessThan(left.clone(), target.clone())
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
    (proof.conclusion == *goal).then_some(proof)
}

#[cfg(test)]
mod tests {
    use super::{DefinitionIndex, ProofNode, Proposition, ScalarTerm, ScalarType, prove};
    use proof_admission::check_certificate;
    use semantic_vocabulary::{
        IntegerSign, IntegerType, IntegerValue, PropositionContext, ValueId,
    };

    fn prove_with_definitions(
        context: &PropositionContext,
        goal: &Proposition,
        assumptions: &[Proposition],
        semantic_axioms: &[Proposition],
    ) -> Option<ProofNode> {
        prove(
            context,
            goal,
            assumptions,
            semantic_axioms,
            &mut DefinitionIndex::new(context, assumptions, semantic_axioms),
        )
    }

    #[test]
    fn strict_endpoint_composes_integer_conversion_and_live_equality() {
        let target_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let target_scalar = ScalarType::Integer(target_type);
        let target = |identity| ScalarTerm::value(ValueId::new(identity).unwrap(), target_scalar);
        let literal =
            |number| ScalarTerm::integer(target_type, IntegerValue::Unsigned(number)).unwrap();
        for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
            let source_type = IntegerType::new(sign, 32).unwrap();
            let source_scalar = ScalarType::Integer(source_type);
            let source = ScalarTerm::value(ValueId::new(1).unwrap(), source_scalar);
            let source_literal = |number| {
                ScalarTerm::integer(
                    source_type,
                    match sign {
                        IntegerSign::Signed => IntegerValue::Signed(number as i128),
                        IntegerSign::Unsigned => IntegerValue::Unsigned(number),
                    },
                )
                .unwrap()
            };
            let context = PropositionContext::from_value_types([
                (ValueId::new(1).unwrap(), source_scalar),
                (ValueId::new(2).unwrap(), target_scalar),
                (ValueId::new(3).unwrap(), target_scalar),
                (ValueId::new(4).unwrap(), target_scalar),
            ])
            .unwrap();
            let converted = if sign == IntegerSign::Signed {
                ScalarTerm::integer_exact_cast(source_type, target_type, source.clone()).unwrap()
            } else {
                ScalarTerm::integer_widen(source_type, target_type, source.clone()).unwrap()
            };
            let assumptions = [Proposition::Conjunction(vec![
                Proposition::LessOrEqual(source_literal(0), source.clone()),
                Proposition::LessOrEqual(source.clone(), source_literal(2)),
            ])];
            let axioms = [
                Proposition::Equal(target(2), converted),
                Proposition::Equal(target(3), target(4)),
                Proposition::Equal(target(4), literal(3)),
            ];
            let goal = Proposition::LessThan(target(2), target(3));
            let proof = prove_with_definitions(&context, &goal, &assumptions, &axioms)
                .expect("converted upper bound and current endpoint equality");
            check_certificate(&context, &goal, &assumptions, &axioms, &proof).unwrap();
            for missing in 0..axioms.len() {
                let mut incomplete = axioms.to_vec();
                incomplete.remove(missing);
                assert!(
                    prove_with_definitions(&context, &goal, &assumptions, &incomplete).is_none()
                );
                assert!(
                    check_certificate(&context, &goal, &assumptions, &incomplete, &proof).is_err()
                );
            }
            let past_end = [Proposition::Conjunction(vec![
                Proposition::LessOrEqual(source_literal(0), source.clone()),
                Proposition::LessOrEqual(source.clone(), source_literal(3)),
            ])];
            assert!(prove_with_definitions(&context, &goal, &past_end, &axioms).is_none());
            assert!(check_certificate(&context, &goal, &past_end, &axioms, &proof).is_err());

            // A live observation need not equal a constant: its guard supplies
            // the strict leg, while the index bound still crosses conversion.
            for guard in [
                Proposition::LessThan(literal(2), target(4)),
                Proposition::LessOrEqual(literal(3), target(4)),
            ] {
                let guarded = [axioms[0].clone(), axioms[1].clone(), guard];
                let proof = prove_with_definitions(&context, &goal, &assumptions, &guarded)
                    .expect("converted bound joins the live strict guard");
                check_certificate(&context, &goal, &assumptions, &guarded, &proof).unwrap();
                for missing in 0..guarded.len() {
                    let mut incomplete = guarded.to_vec();
                    incomplete.remove(missing);
                    assert!(
                        prove_with_definitions(&context, &goal, &assumptions, &incomplete)
                            .is_none()
                    );
                    assert!(
                        check_certificate(&context, &goal, &assumptions, &incomplete, &proof)
                            .is_err()
                    );
                }
                let weak = [
                    axioms[0].clone(),
                    axioms[1].clone(),
                    Proposition::LessOrEqual(literal(2), target(4)),
                ];
                assert!(prove_with_definitions(&context, &goal, &assumptions, &weak).is_none());
                assert!(check_certificate(&context, &goal, &assumptions, &weak, &proof).is_err());
            }
        }
    }

    #[test]
    fn remainder_bound_rejoins_a_computed_closed_strict_endpoint() {
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let scalar_type = ScalarType::Integer(integer_type);
        let value = |identity| ScalarTerm::value(ValueId::new(identity).unwrap(), scalar_type);
        let literal =
            |number| ScalarTerm::integer(integer_type, IntegerValue::Unsigned(number)).unwrap();
        let context = PropositionContext::from_value_types(
            (1..=5).map(|identity| (ValueId::new(identity).unwrap(), scalar_type)),
        )
        .unwrap();
        let goal = Proposition::LessThan(value(5), value(4));
        for divisor in [3, 10, 17] {
            let axioms = [
                Proposition::Equal(value(2), literal(divisor)),
                Proposition::Equal(value(3), literal(0)),
                Proposition::Equal(
                    value(4),
                    ScalarTerm::ExactIntegerAdd {
                        scalar_type: integer_type,
                        left: Box::new(value(2)),
                        right: Box::new(value(3)),
                    },
                ),
                Proposition::Equal(
                    value(5),
                    ScalarTerm::ExactIntegerRemainder {
                        scalar_type: integer_type,
                        left: Box::new(value(1)),
                        right: Box::new(value(2)),
                    },
                ),
            ];
            let proof = prove_with_definitions(&context, &goal, &[], &axioms)
                .expect("remainder image joins the independently computed live extent");
            check_certificate(&context, &goal, &[], &axioms, &proof).unwrap();
            for missing in 0..axioms.len() {
                let mut incomplete = axioms.to_vec();
                incomplete.remove(missing);
                assert!(check_certificate(&context, &goal, &[], &incomplete, &proof).is_err());
            }
            let mut too_short = axioms.clone();
            too_short[2] = Proposition::Equal(value(4), literal(divisor - 1));
            assert!(prove_with_definitions(&context, &goal, &[], &too_short).is_none());
            assert!(check_certificate(&context, &goal, &[], &too_short, &proof).is_err());
            let mut zero_divisor = axioms.clone();
            zero_divisor[0] = Proposition::Equal(value(2), literal(0));
            assert!(prove_with_definitions(&context, &goal, &[], &zero_divisor).is_none());
        }
    }

    fn wrapping_add_definition(
        integer_type: IntegerType,
        defined: &ScalarTerm,
        operand: &ScalarTerm,
        addend: u128,
    ) -> Proposition {
        Proposition::Equal(
            defined.clone(),
            ScalarTerm::wrapping_integer_add(
                integer_type,
                operand.clone(),
                ScalarTerm::integer(integer_type, IntegerValue::Unsigned(addend)).unwrap(),
            )
            .unwrap(),
        )
    }

    #[test]
    fn wrapping_add_backward_derives_headroom_from_the_strict_guard() {
        // `v_new = wrapping(v_old + 1)`; the guard `v_old < 3` supplies the
        // `v_old <= MAX - 1` headroom through weakening plus closed
        // transitivity, so `v_new < 2` yields `v_old < 1`.
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let scalar_type = ScalarType::Integer(integer_type);
        let defined = ScalarTerm::value(ValueId::new(1).unwrap(), scalar_type);
        let operand = ScalarTerm::value(ValueId::new(2).unwrap(), scalar_type);
        let literal =
            |number| ScalarTerm::integer(integer_type, IntegerValue::Unsigned(number)).unwrap();
        let context = PropositionContext::from_value_types(
            (1..=2).map(|identity| (ValueId::new(identity).unwrap(), scalar_type)),
        )
        .unwrap();
        let axioms = [
            wrapping_add_definition(integer_type, &defined, &operand, 1),
            Proposition::LessThan(defined.clone(), literal(2)),
            Proposition::LessThan(operand.clone(), literal(3)),
        ];
        let goal = Proposition::LessThan(operand.clone(), literal(1));
        let proof = prove_with_definitions(&context, &goal, &[], &axioms)
            .expect("strict backward traversal with derived headroom");
        check_certificate(&context, &goal, &[], &axioms, &proof).unwrap();
        assert!(prove_with_definitions(&context, &goal, &[], &axioms[..2]).is_none());
        // The same equation must not over-derive: `operand < 2` does not
        // follow, since a wrapped sum can satisfy `v_new < 2` at any operand.
        assert!(
            prove_with_definitions(
                &context,
                &Proposition::LessThan(operand.clone(), literal(2)),
                &[],
                &axioms,
            )
            .is_none()
        );
    }
}
