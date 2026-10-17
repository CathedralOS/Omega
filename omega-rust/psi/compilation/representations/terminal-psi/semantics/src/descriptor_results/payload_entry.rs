//! Entry predicates belong to the caller's actual formal payloads. Instantiate
//! each pure domain on that storage, then check a certificate citing only the
//! caller-owed requirements. Copies and writes use ordinary payload history;
//! a receiving annotation never participates in this entry judgment.
use super::*;
use semantic_vocabulary::{CanonicalStructuralPathSegment as Segment, Proposition};
use terminal_psi::{ProofNode, ProofRule, StructuralDomainDeclaration};

mod order;

pub(super) fn close(
    types: &[StructuralTypeDeclaration],
    domains: &[StructuralDomainDeclaration],
    machine: &DescriptorMachine,
    root: PlaceId,
    contract: &mut PayloadMembershipContract,
) {
    let Some((context, requirements)) = &machine.payload_entry_predicates else {
        return;
    };
    let mut pending = vec![(contract.carrier, Vec::new(), Vec::new(), 0usize)];
    let mut remaining = 4096usize;
    while let Some((carrier, path, storage, depth)) = pending.pop() {
        let Some(next) = remaining.checked_sub(1) else {
            break;
        };
        remaining = next;
        for domain in domains.iter().filter(|domain| {
            domain.carrier == carrier
                && domain.content_projection.is_none()
                && domain.establishment_routes.is_empty()
        }) {
            let Some(predicate) = &domain.predicate else {
                continue;
            };
            let goal = crate::substitution::substitute_proposition_structural_places(
                &predicate.proposition,
                &BTreeMap::from([(predicate.subject, (root, path.clone()))]),
            );
            let Ok(checked) = proof_admission::check_predicate_denotations_with_value_equalities(
                context,
                &goal,
                requirements,
                &[],
            ) else {
                continue;
            };
            let Some(proof) = prove(context, checked.goal(), checked.requirements(), 0) else {
                continue;
            };
            if checked.check_certificate(context, &proof).is_err() {
                continue;
            }
            if storage.is_empty() {
                contract.qualifications.push(domain.id);
            } else {
                contract
                    .projected_qualifications
                    .push(terminal_psi::StructuralPathQualification {
                        path: storage.clone(),
                        domain: domain.id,
                    });
            }
        }
        if depth >= 64 {
            continue;
        }
        let Some(declaration) = terminal_psi::find_structural_type(types, carrier) else {
            continue;
        };
        if let Some(fields) = declaration.shape.direct_fields() {
            for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                if pending.len() >= remaining {
                    break;
                }
                if let terminal_psi::StructuralFieldType::Structural(child) = field.field_type {
                    let mut child_path = path.clone();
                    child_path.push(Segment::Field(field.id));
                    let mut child_storage = storage.clone();
                    child_storage.push(StructuralPathSegment::Field(field.identity));
                    pending.push((child, child_path, child_storage, depth + 1));
                }
            }
        }
        if let StructuralTypeShape::FixedArray { element, length } = declaration.shape
            && length <= 4096
        {
            for index in 0..length {
                if pending.len() >= remaining {
                    break;
                }
                let mut child_path = path.clone();
                child_path.push(Segment::FixedIndex(index));
                let mut child_storage = storage.clone();
                child_storage.push(StructuralPathSegment::FixedIndex(index));
                pending.push((element, child_path, child_storage, depth + 1));
            }
        }
    }
    contract.qualifications.sort();
    contract.qualifications.dedup();
    contract.projected_qualifications.sort();
    contract.projected_qualifications.dedup();
}

pub(super) fn prove(
    context: &semantic_vocabulary::PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    depth: usize,
) -> Option<ProofNode> {
    prove_with_budget(context, goal, requirements, depth, &mut 4096)
}

fn prove_with_budget(
    context: &semantic_vocabulary::PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    depth: usize,
    remaining: &mut usize,
) -> Option<ProofNode> {
    *remaining = remaining.checked_sub(1)?;
    if depth > 64 {
        return None;
    }
    // Predicate conversion preserves arithmetic constructors. Ask the existing
    // kernel for their primitive judgment instead of treating only literal
    // predicates as provable or introducing a second arithmetic evaluator.
    if let Some(judgment) = [
        terminal_psi::PrimitiveJudgment::Truth,
        terminal_psi::PrimitiveJudgment::ReflexiveEquality,
        terminal_psi::PrimitiveJudgment::ClosedIntegerRelation,
        terminal_psi::PrimitiveJudgment::IntegerCarrierBound,
    ]
    .into_iter()
    .find(|judgment| proof_admission::decide_primitive(context, goal, *judgment).is_ok())
    {
        return Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::Primitive(judgment),
        });
    }
    for (index, requirement) in requirements.iter().enumerate() {
        let proof = ProofNode {
            conclusion: requirement.clone(),
            rule: ProofRule::Assumption { index },
        };
        if let Some(proof) = select(
            context,
            goal,
            proof,
            requirements.len(),
            depth + 1,
            remaining,
        ) {
            return Some(proof);
        }
    }
    if let Proposition::Conjunction(children) = goal {
        let proofs = children
            .iter()
            .map(|child| prove_with_budget(context, child, requirements, depth + 1, remaining))
            .collect::<Option<Vec<_>>>()?;
        return Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::ConjunctionIntroduction(proofs),
        });
    }
    if let Proposition::Disjunction(children) = goal {
        for (index, child) in children.iter().enumerate() {
            if let Some(disjunct) =
                prove_with_budget(context, child, requirements, depth + 1, remaining)
            {
                return Some(ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::DisjunctionIntroduction {
                        disjunct: Box::new(disjunct),
                        index,
                    },
                });
            }
        }
    }
    None
}

fn select(
    context: &semantic_vocabulary::PropositionContext,
    goal: &Proposition,
    proof: ProofNode,
    assumptions: usize,
    depth: usize,
    remaining: &mut usize,
) -> Option<ProofNode> {
    *remaining = remaining.checked_sub(1)?;
    if proof.conclusion == *goal {
        return Some(proof);
    }
    if depth > 64 {
        return None;
    }
    if proof_admission::lift_fixed_integer_relation(&proof.conclusion).as_ref() == Some(goal)
        || proof_admission::lower_integer_math_relation(&proof.conclusion).as_ref() == Some(goal)
    {
        // The kernel's assumption rule already recognizes this carrier-preserving
        // bridge. Discharge the proved relation locally so a selected conjunct
        // uses that same rule without changing the ambient premise roster.
        return Some(bridge(goal.clone(), proof, assumptions));
    }
    if let Some(derived) = order::derive(context, goal, &proof, assumptions) {
        return Some(derived);
    }
    if let Proposition::Conjunction(children) = &proof.conclusion {
        for (conjunct, child) in children.iter().enumerate() {
            let selected = ProofNode {
                conclusion: child.clone(),
                rule: ProofRule::ConjunctionElimination {
                    conjunction: Box::new(proof.clone()),
                    conjunct,
                },
            };
            if let Some(selected) =
                select(context, goal, selected, assumptions, depth + 1, remaining)
            {
                return Some(selected);
            }
        }
    }
    if let Proposition::Disjunction(children) = &proof.conclusion {
        // Each branch uses only its newly appended assumption. This transport
        // cannot mix alternatives or repeatedly split an unchanged ambient row:
        // recursive selection descends into this child, not the premise roster.
        let branches = children
            .iter()
            .map(|child| {
                select(
                    context,
                    goal,
                    ProofNode {
                        conclusion: child.clone(),
                        rule: ProofRule::Assumption { index: assumptions },
                    },
                    assumptions + 1,
                    depth + 1,
                    remaining,
                )
            })
            .collect::<Option<Vec<_>>>();
        if let Some(branches) = branches {
            return Some(ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::DisjunctionElimination {
                    disjunction: Box::new(proof),
                    branches,
                },
            });
        }
    }
    if let Proposition::Disjunction(children) = goal {
        for (index, child) in children.iter().enumerate() {
            if let Some(disjunct) = select(
                context,
                child,
                proof.clone(),
                assumptions,
                depth + 1,
                remaining,
            ) {
                return Some(ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::DisjunctionIntroduction {
                        disjunct: Box::new(disjunct),
                        index,
                    },
                });
            }
        }
    }
    None
}

fn bridge(goal: Proposition, proof: ProofNode, assumptions: usize) -> ProofNode {
    ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::ImplicationElimination {
            implication: Box::new(ProofNode {
                conclusion: Proposition::Implication {
                    premise: Box::new(proof.conclusion.clone()),
                    conclusion: Box::new(goal.clone()),
                },
                rule: ProofRule::ImplicationIntroduction {
                    body: Box::new(ProofNode {
                        conclusion: goal,
                        rule: ProofRule::Assumption { index: assumptions },
                    }),
                },
            }),
            premise: Box::new(proof),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{PropositionContext, ScalarTerm, ScalarType, ValueId};

    #[test]
    fn disjunction_transport_checks_every_alternative_in_its_own_scope() {
        use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
        let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let values = [1, 2, 3].map(|index| ValueId::new(index).unwrap());
        let context = PropositionContext::from_value_types(
            values.map(|value| (value, ScalarType::Integer(integer))),
        )
        .unwrap();
        let finite = values.map(|value| {
            Proposition::LessThan(
                ScalarTerm::value(value, ScalarType::Integer(integer)),
                ScalarTerm::integer(integer, IntegerValue::Unsigned(10)).unwrap(),
            )
        });
        let mathematical = finite
            .each_ref()
            .map(|proposition| proof_admission::lift_fixed_integer_relation(proposition).unwrap());
        let requirements = [Proposition::Conjunction(vec![
            Proposition::Truth,
            Proposition::Disjunction(finite[..2].to_vec()),
        ])];
        let goal = Proposition::Disjunction(mathematical[..2].to_vec());
        let proof = prove(&context, &goal, &requirements, 0).unwrap();
        proof_admission::check_certificate(&context, &goal, &requirements, &[], &proof).unwrap();
        proof_admission::verify_bounded_certificate(
            &context,
            &goal,
            &requirements,
            &[],
            &proof,
            &mut proof_admission::Budget::default(),
        )
        .unwrap();
        assert!(prove(&context, &mathematical[0], &requirements, 0).is_none());
        let wrong =
            Proposition::Disjunction(vec![mathematical[0].clone(), mathematical[2].clone()]);
        assert!(prove(&context, &wrong, &requirements, 0).is_none());
    }

    #[test]
    fn disjunction_proof_uses_one_established_alternative_without_inventing_the_other() {
        let left = ValueId::new(1).unwrap();
        let right = ValueId::new(2).unwrap();
        let context = PropositionContext::from_value_types([
            (left, ScalarType::Boolean),
            (right, ScalarType::Boolean),
        ])
        .unwrap();
        let predicates = [left, right].map(|value| {
            Proposition::Equal(
                ScalarTerm::value(value, ScalarType::Boolean),
                ScalarTerm::Boolean(true),
            )
        });
        let goal = Proposition::Disjunction(predicates.to_vec());
        for predicate in &predicates {
            let requirements = [predicate.clone()];
            let checked = proof_admission::check_predicate_denotations_with_value_equalities(
                &context,
                &goal,
                &requirements,
                &[],
            )
            .unwrap();
            let proof = prove(&context, checked.goal(), checked.requirements(), 0).unwrap();
            assert!(checked.check_certificate(&context, &proof).is_ok());
        }
        assert!(prove(&context, &goal, &[], 0).is_none());
        assert!(
            prove(
                &context,
                &Proposition::Conjunction(predicates.to_vec()),
                &predicates[..1],
                0
            )
            .is_none()
        );
    }

    #[test]
    fn selected_finite_relation_uses_the_kernel_mathematical_citation_bridge() {
        use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
        let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let value = ValueId::new(1).unwrap();
        let other = ValueId::new(2).unwrap();
        let context = PropositionContext::from_value_types([
            (value, ScalarType::Integer(integer)),
            (other, ScalarType::Integer(integer)),
        ])
        .unwrap();
        let finite = Proposition::LessThan(
            ScalarTerm::value(value, ScalarType::Integer(integer)),
            ScalarTerm::integer(integer, IntegerValue::Unsigned(10)).unwrap(),
        );
        let goal = proof_admission::lift_fixed_integer_relation(&finite).unwrap();
        let requirements = [Proposition::Conjunction(vec![
            Proposition::Truth,
            finite.clone(),
        ])];
        let proof = prove(&context, &goal, &requirements, 0).unwrap();
        assert!(
            proof_admission::check_certificate(&context, &goal, &requirements, &[], &proof).is_ok()
        );
        assert!(prove(&context, &goal, &[], 0).is_none());
        let reverse_requirements = [goal];
        let reverse = prove(&context, &finite, &reverse_requirements, 0).unwrap();
        assert!(
            proof_admission::check_certificate(
                &context,
                &finite,
                &reverse_requirements,
                &[],
                &reverse
            )
            .is_ok()
        );
        let wrong = Proposition::LessThan(
            ScalarTerm::value(other, ScalarType::Integer(integer)),
            ScalarTerm::integer(integer, IntegerValue::Unsigned(10)).unwrap(),
        );
        assert!(prove(&context, &wrong, &reverse_requirements, 0).is_none());
    }
}
