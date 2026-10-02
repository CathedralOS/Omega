//! Rejoin nested scalar computations through their established equations.
//!
//! Predicate case analysis can expand a compact computation into many Boolean
//! alternatives. Transport instead cites the exact equations reachable from
//! the goal, preserving every constructor and operand identity. The simplified
//! question is proved under the original premises, then a carried inference
//! rejoins it to the original question. No ambient citation is rewritten.

use std::collections::BTreeSet;

use proof_admission::{ProofNode, ProofRule, check_value_equality_denotation};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm, ValueId};

use super::integer_evidence::projected_facts;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameters: &BTreeSet<ValueId>,
) -> Option<ProofNode> {
    // Transport through the field a read observes first. Rejoining a live
    // store replaces that field with the stored computation, which can drop a
    // relation proved about the field itself, so it only runs when the
    // field-level transport fails.
    transport(
        context,
        goal,
        assumptions,
        semantic_axioms,
        machine_parameters,
        false,
    )
    .or_else(|| {
        transport(
            context,
            goal,
            assumptions,
            semantic_axioms,
            machine_parameters,
            true,
        )
    })
}

fn retained_equalities(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    rejoin_stores: bool,
) -> Option<Vec<ProofNode>> {
    let facts = projected_facts(assumptions, semantic_axioms);
    let mut pending = Vec::new();
    goal.visit_value_ids(|value| pending.push(value));
    let mut visited = BTreeSet::new();
    let mut equalities = Vec::new();
    let mut remaining = 4096usize;
    while let Some(value) = pending.pop() {
        remaining = remaining.checked_sub(1)?;
        super::search_context::spend(1)?;
        if !visited.insert(value) {
            continue;
        }
        // Prefer a defining equation over an alias in the opposite direction.
        // In particular an incoming formal must not be expanded back into the
        // result whose definition already depends on that formal.
        let fact = facts.iter().find(|fact| {
            matches!(fact.proposition,
                Proposition::Equal(left @ ScalarTerm::Value { id, .. }, right)
                    if *id == value && left != right
            )
        });
        let Some(fact) = fact else { continue };
        let Proposition::Equal(endpoint, observed) = fact.proposition else {
            continue;
        };
        // A definition may stop at a field, while a prior guard relates that
        // same live field to a literal through another read. Carry the proved
        // equality chain instead of discarding it during forward expansion.
        // Only this site's retained facts participate; no source-level fact
        // or invalidated storage observation is recovered here.
        let equality = super::integer_selection::cited_literal_equality(
            context,
            endpoint,
            assumptions,
            semantic_axioms,
        )
        .or_else(|| {
            // A live store may hold a computation rather than a literal.
            // Rejoin read = field and field = stored value with explicit
            // transitivity, then let the ordinary definition walk expand that
            // value. Only reconstruction's surviving store facts participate;
            // do not recover a write from source or another observation scope.
            if !rejoin_stores
                || !matches!(
                    observed,
                    ScalarTerm::IntegerField { .. } | ScalarTerm::BooleanField { .. }
                )
            {
                return None;
            }
            let store = facts.iter().find(|store| {
                matches!(store.proposition,
                    Proposition::Equal(field, stored) if field == observed && stored != endpoint
                )
            })?;
            let Proposition::Equal(_, stored) = store.proposition else {
                return None;
            };
            Some(ProofNode {
                conclusion: Proposition::Equal(endpoint.clone(), stored.clone()),
                rule: ProofRule::EqualityTransitivity {
                    left_equals_middle: Box::new(fact.proof()),
                    middle_equals_right: Box::new(store.proof()),
                },
            })
        })
        .unwrap_or_else(|| fact.proof());
        equality.conclusion.visit_value_ids(|dependency| {
            if dependency != value && !visited.contains(&dependency) {
                pending.push(dependency);
            }
        });
        equalities.push(equality);
    }
    Some(equalities)
}

/// Retain both the original citation and an equivalent, explicitly transported
/// view. Equations stay proved children; no caller premise roster is rewritten.
pub(super) fn citation_forms(
    context: &PropositionContext,
    citation: ProofNode,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Vec<ProofNode> {
    let mut forms = Vec::new();
    if let Some(equalities) = retained_equalities(
        context,
        &citation.conclusion,
        assumptions,
        semantic_axioms,
        false,
    ) && !equalities.is_empty()
        && let Ok(transported) = check_value_equality_denotation(
            context,
            &citation.conclusion,
            equalities.iter().map(|equality| &equality.conclusion),
        )
        && transported != citation.conclusion
    {
        forms.push(ProofNode {
            conclusion: transported,
            rule: ProofRule::ValueEqualityTransport {
                premise: Box::new(citation.clone()),
                equalities,
            },
        });
    }
    forms.push(citation);
    forms
}

fn transport(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameters: &BTreeSet<ValueId>,
    rejoin_stores: bool,
) -> Option<ProofNode> {
    let equalities =
        retained_equalities(context, goal, assumptions, semantic_axioms, rejoin_stores)?;
    if equalities.is_empty() {
        return None;
    }
    let transported = check_value_equality_denotation(
        context,
        goal,
        equalities.iter().map(|equality| &equality.conclusion),
    )
    .ok()?;
    if &transported == goal {
        return None;
    }
    let premise = super::integer_selection::build_with_machine_parameters(
        context,
        &transported,
        assumptions,
        semantic_axioms,
        machine_parameters,
    )?;
    Some(ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::ValueEqualityTransport {
            premise: Box::new(premise),
            equalities,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::prove;
    use semantic_vocabulary::{
        CanonicalStructuralPathSegment, IntegerSign, IntegerType, IntegerValue, PlaceId,
        Proposition, PropositionContext, ScalarTerm, ScalarType, StructuralFieldId,
        StructuralPlaceKind, ValueId,
    };
    use std::collections::BTreeSet;

    #[test]
    fn computed_store_transport_requires_the_same_live_field_equation() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
        let scalar_type = ScalarType::Integer(integer_type);
        let context = PropositionContext::from_value_types_and_places(
            (1..=3).map(|identity| (ValueId::new(identity).unwrap(), scalar_type)),
            [(
                PlaceId::new(1).unwrap(),
                StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            )],
        )
        .unwrap();
        let value = |identity| ScalarTerm::value(ValueId::new(identity).unwrap(), scalar_type);
        let literal =
            |number| ScalarTerm::integer(integer_type, IntegerValue::Signed(number)).unwrap();
        let field = |identity| {
            ScalarTerm::integer_field_path(
                PlaceId::new(1).unwrap(),
                vec![CanonicalStructuralPathSegment::Field(
                    StructuralFieldId::new(identity).unwrap(),
                )],
                integer_type,
            )
        };
        let mut axioms = vec![
            Proposition::Equal(value(1), literal(25)),
            Proposition::Equal(
                value(2),
                ScalarTerm::exact_integer_divide(integer_type, value(1), literal(10)).unwrap(),
            ),
            Proposition::Equal(field(1), value(2)),
            Proposition::Equal(value(3), field(1)),
        ];
        let goal = Proposition::LessOrEqual(value(3), literal(2));
        let parameters = BTreeSet::new();
        let proof = prove(&context, &goal, &[], &axioms, &parameters)
            .expect("the computed quotient survives its store and read");
        proof_admission::accept_certificate(&context, &goal, &[], &axioms, &proof).unwrap();

        for replacement in [Proposition::Truth, Proposition::Equal(field(2), value(2))] {
            axioms[2] = replacement;
            assert!(prove(&context, &goal, &[], &axioms, &parameters).is_none());
            assert!(
                proof_admission::accept_certificate(&context, &goal, &[], &axioms, &proof).is_err(),
                "a missing or different field store cannot replay the certificate"
            );
        }
    }

    /// A relation proved about the live field itself must stay usable when
    /// that field also holds a stored computation: transport through the
    /// field first, and rejoin the store only when that fails. Rejoining
    /// first replaced `field <= 4` with an unbounded `unknown + 1 <= 4`.
    #[test]
    fn a_field_relation_is_not_lost_to_its_stored_computation() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
        let scalar_type = ScalarType::Integer(integer_type);
        let context = PropositionContext::from_value_types_and_places(
            (1..=3).map(|identity| (ValueId::new(identity).unwrap(), scalar_type)),
            [(
                PlaceId::new(1).unwrap(),
                StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            )],
        )
        .unwrap();
        let value = |identity| ScalarTerm::value(ValueId::new(identity).unwrap(), scalar_type);
        let literal =
            |number| ScalarTerm::integer(integer_type, IntegerValue::Signed(number)).unwrap();
        let field = ScalarTerm::integer_field_path(
            PlaceId::new(1).unwrap(),
            vec![CanonicalStructuralPathSegment::Field(
                StructuralFieldId::new(1).unwrap(),
            )],
            integer_type,
        );
        let axioms = vec![
            Proposition::Equal(
                value(2),
                ScalarTerm::exact_integer_add(integer_type, value(1), literal(1)).unwrap(),
            ),
            Proposition::Equal(field.clone(), value(2)),
            Proposition::Equal(value(3), field.clone()),
        ];
        let assumptions = vec![Proposition::LessOrEqual(field, literal(4))];
        let goal = Proposition::LessOrEqual(value(3), literal(4));
        let parameters = BTreeSet::new();
        let proof = prove(&context, &goal, &assumptions, &axioms, &parameters)
            .expect("the read transports to the bounded field");
        proof_admission::accept_certificate(&context, &goal, &assumptions, &axioms, &proof)
            .unwrap();
        assert!(
            prove(&context, &goal, &[], &axioms, &parameters).is_none(),
            "without the field bound neither route proves the goal"
        );
    }
}
