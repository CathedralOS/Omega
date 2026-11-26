use super::*;
use semantic_vocabulary::{CanonicalStructuralPathSegment as Segment, Proposition};
use terminal_psi::{ProofNode, ProofRule, StructuralDomainDeclaration};

pub(crate) mod order;

pub(crate) fn close(
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
                && domain.establishment_routes.is_none()
        }) {
            let (supplied, _) = terminal_psi::StructuralPathQualification::select(
                &contract.qualifications,
                &contract.projected_qualifications,
                &storage,
            );
            let Some(predicate) = super::payload_predicates::introduction_predicate(
                types, domains, domain, &supplied,
            ) else {
                continue;
            };
            let Ok(goal) = crate::substitution::substitute_proposition_structural_places(
                &predicate.proposition,
                &BTreeMap::from([(predicate.subject, (root, path.clone()))]),
            ) else {
                continue;
            };
            let Ok(checked) = proof_admission::check_predicate_denotations_with_value_equalities(
                context, &goal, requirements, &[],
            ) else {
                continue;
            };
            let Some(proof) = prove(context, checked.goal(), checked.requirements(), 0)
            else {
                continue;
            };
            if checked.check_certificate(context, &proof).is_err() {
                continue;
            }
            if storage.is_empty() {
                contract.qualifications.push(domain.id);
            } else {
                contract.projected_qualifications.push(
                    terminal_psi::StructuralPathQualification {
                        path: storage.clone(),
                        domain: domain.id,
                    },
                );
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
                if let terminal_psi::StructuralFieldType::Structural(child) =
                    field.field_type
                {
                    let mut child_path = path.clone();
                    child_path.push(Segment::Field(field.id));
                    let mut child_storage = storage.clone();
                    child_storage.push(StructuralPathSegment::Field(field.identity));
                    pending.push((child, child_path, child_storage, depth + 1));
                }
            }
        }
        if let StructuralTypeShape::FixedArray {
            element,
            length: terminal_psi::ArrayExtent::Literal(length),
        } = declaration.shape
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

pub(crate) fn prove(
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
    if let Some(lowered) = proof_admission::lower_integer_math_relation(goal)
        && let Some(proof) =
            prove_with_budget(context, &lowered, requirements, depth + 1, remaining)
    {
        return Some(proof_admission::restate_integer_relation(
            goal.clone(), proof, requirements.len(),
        ));
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
            .map(|child| {
                prove_with_budget(context, child, requirements, depth + 1, remaining)
            })
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
    if proof_admission::lift_fixed_integer_relation(&proof.conclusion).as_ref()
        == Some(goal)
        || proof_admission::lower_integer_math_relation(&proof.conclusion).as_ref()
            == Some(goal)
    {
        return Some(proof_admission::restate_integer_relation(
            goal.clone(), proof, assumptions,
        ));
    }
    if let Some(derived) = order::derive(context, goal, &proof, assumptions) {
        return Some(derived);
    }
    if let Proposition::Conjunction(children) = &proof.conclusion {
        let conjunction = std::sync::Arc::new(proof.clone());
        for (conjunct, child) in children.iter().enumerate() {
            let selected = ProofNode {
                conclusion: child.clone(),
                rule: ProofRule::ConjunctionElimination {
                    conjunction: conjunction.clone(),
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

#[cfg(test)]
mod tests;
