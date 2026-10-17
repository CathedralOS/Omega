//! Entry predicates belong to the caller's actual formal payloads. Instantiate
//! each pure domain on that storage, then check a certificate citing only the
//! caller-owed requirements. Copies and writes use ordinary payload history;
//! a receiving annotation never participates in this entry judgment.
use super::*;
use semantic_vocabulary::{CanonicalStructuralPathSegment as Segment, Proposition};
use terminal_psi::{ProofNode, ProofRule, StructuralDomainDeclaration};

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
        if let Some(proof) = select(goal, proof, depth + 1) {
            return Some(proof);
        }
    }
    if let Proposition::Conjunction(children) = goal {
        let proofs = children
            .iter()
            .map(|child| prove(context, child, requirements, depth + 1))
            .collect::<Option<Vec<_>>>()?;
        return Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::ConjunctionIntroduction(proofs),
        });
    }
    None
}

fn select(goal: &Proposition, proof: ProofNode, depth: usize) -> Option<ProofNode> {
    if proof.conclusion == *goal {
        return Some(proof);
    }
    if depth > 64 {
        return None;
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
            if let Some(selected) = select(goal, selected, depth + 1) {
                return Some(selected);
            }
        }
    }
    None
}
