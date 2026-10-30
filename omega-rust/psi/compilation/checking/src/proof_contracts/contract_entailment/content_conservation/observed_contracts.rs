//! Conservation laws on completed selected calls.
//!
//! A set equation does not identify its individual endpoints. Keep the exact
//! normalized algebra, projection owner and observed scalar operands together,
//! rather than feeding a set equality to ordinary structural substitution.
//! Entry operands are detached call-entry observations; current operands use
//! only the frame owner's surviving or fresh post-call observations. No law
//! introduces a qualification, backing, or authority to observe a referent.

use super::{
    ContentConservationDependencies, ContentConservationImplication, ContentValues,
    StructuralJudge, StructuralJudgment, StructuralTerm,
};
use language_semantics::SemanticDomainId;
use language_semantics::content::{
    ContentAlgebraIdentity, ContentConservationPlan, ContentConservationTerm, ContentPlaceSegment,
    ContentPlaceVersion, ContentProjectionExpression,
};
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::expression::BinaryOperator;
use typed_trees::signature::StateParameter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::proof_contracts::contract_entailment) struct ObservedContentEquation {
    algebra: ContentAlgebraIdentity,
    left: ObservedContentTerm,
    right: ObservedContentTerm,
    requirements: Vec<ContentConservationPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ObservedContentTerm {
    Projection {
        domain: SymbolHandle,
        semantic_domain: SemanticDomainId,
        projection_machine: SymbolHandle,
        scalars: Vec<StructuralTerm>,
    },
    Separate(Vec<ObservedContentTerm>),
}

impl ObservedContentEquation {
    pub(in crate::proof_contracts::contract_entailment) fn retain_family_sources(
        &mut self,
        other: &Self,
    ) {
        for requirement in &other.requirements {
            if !self.requirements.contains(requirement) {
                self.requirements.push(requirement.clone());
            }
        }
    }

    pub(in crate::proof_contracts::contract_entailment) fn same_equation(
        &self,
        other: &Self,
    ) -> bool {
        self.algebra == other.algebra
            && ((same_term(&self.left, &other.left, &PartialEq::eq)
                && same_term(&self.right, &other.right, &PartialEq::eq))
                || (same_term(&self.left, &other.right, &PartialEq::eq)
                    && same_term(&self.right, &other.left, &PartialEq::eq)))
    }
}

/// The selected-contract owner calls this only after authenticating all entry
/// requirements. Its existing family intersection applies to these observations
/// as well as ordinary equations; one target body cannot donate a public law.
pub(in crate::proof_contracts::contract_entailment) fn call_content_guarantees(
    program: &dyn crate::CheckingInput,
    judge: &StructuralJudge<'_>,
    callable: SymbolHandle,
    parameters: &[StateParameter],
    entry_bindings: &[(SymbolHandle, StructuralTerm)],
    bindings: &[(SymbolHandle, StructuralTerm)],
    result: &StructuralTerm,
) -> Vec<ObservedContentEquation> {
    let plans = crate::value_custody::content_conservation::content_conservation_plans(program);
    let projections = crate::value_custody::content_conservation::content_projection_plans(program);
    let parameter_projection = |root, path: &[ContentPlaceSegment], version| {
        let parameter = parameters
            .iter()
            .find(|parameter| parameter.symbol == root)?;
        let values = match version {
            ContentPlaceVersion::Entry => entry_bindings,
            ContentPlaceVersion::Current => bindings,
        };
        let (_, value) = values.iter().find(|(symbol, _)| *symbol == root)?;
        judge
            .completed_value_projection(parameter.type_reference, value.clone(), path)
            .map(|(_, value)| value)
    };
    let values = ContentValues {
        program,
        judge,
        result,
        projections: &projections,
        parameter_projection: &parameter_projection,
    };
    plans
        .iter()
        .filter(|source| source.plan.callable == callable)
        .filter_map(|source| {
            let mut equation = observe_equation(&values, &source.plan)?;
            // A declared conditional return contract is not yet an independent
            // checked theorem. Preserve its complete normalized identity for
            // the one rooted-publication fixed point. Admitted requirements
            // and explicit admission claims retain their existing root role.
            if source.plan.owner_kind
                == language_semantics::content::ContentConservationOwnerKind::Machine
                && !program
                    .machine_by_symbol(source.plan.owner)
                    .is_some_and(|machine| {
                        machine.supply_mode == language_semantics::MachineSupplyMode::AdmissionClaim
                    })
            {
                equation.requirements.push(source.plan.clone());
            }
            Some(equation)
        })
        .collect()
}

fn observe_equation(
    values: &ContentValues<'_, '_>,
    plan: &ContentConservationPlan,
) -> Option<ObservedContentEquation> {
    Some(ObservedContentEquation {
        algebra: plan.algebra.clone(),
        left: observe_term(values, plan.equation.left())?,
        right: observe_term(values, plan.equation.right())?,
        requirements: Vec::new(),
    })
}

fn observe_term(
    values: &ContentValues<'_, '_>,
    term: &ContentConservationTerm,
) -> Option<ObservedContentTerm> {
    match term {
        ContentConservationTerm::Projection {
            domain,
            semantic_domain,
            projection_machine,
            subject,
            ..
        } => {
            let projection = values.projections.iter().find(|projection| {
                projection.machine == *projection_machine
                    && projection.domain == *domain
                    && projection.semantic_domain == *semantic_domain
            })?;
            let scalars = match &projection.expression {
                ContentProjectionExpression::CountedQuantity { magnitude } => {
                    vec![values.scalar(subject, magnitude)?]
                }
                ContentProjectionExpression::IntervalSet { members } => {
                    let mut scalars = Vec::with_capacity(members.len() * 2);
                    for member in members {
                        scalars.push(values.scalar(subject, member.start())?);
                        scalars.push(values.scalar(subject, member.end())?);
                    }
                    scalars
                }
            };
            Some(ObservedContentTerm::Projection {
                domain: *domain,
                semantic_domain: *semantic_domain,
                projection_machine: *projection_machine,
                scalars,
            })
        }
        ContentConservationTerm::Separate(terms) => Some(ObservedContentTerm::Separate(
            terms
                .iter()
                .map(|term| observe_term(values, term))
                .collect::<Option<Vec<_>>>()?,
        )),
    }
}

/// Retain all alternative law paths as a compact dependency graph. Equality
/// classes share endpoints without materializing a clique or enumerating proof
/// paths. Only independently proved numeric comparisons merge classes.
pub(super) fn proof_dependencies(
    values: &ContentValues<'_, '_>,
    plan: &ContentConservationPlan,
) -> Option<ContentConservationDependencies> {
    let goal = observe_equation(values, plan)?;
    let laws = values
        .judge
        .normal_content_guarantees
        .iter()
        .filter(|law| law.algebra == goal.algebra)
        .collect::<Vec<_>>();
    if laws.is_empty() {
        return None;
    }
    let mut endpoints = vec![&goal.left, &goal.right];
    for law in &laws {
        endpoints.extend([&law.left, &law.right]);
    }
    let mut parents = (0..endpoints.len()).collect::<Vec<_>>();
    let mut comparable = vec![false; endpoints.len()];
    fn representative(parents: &mut [usize], mut position: usize) -> usize {
        while parents[position] != position {
            let parent = parents[position];
            parents[position] = parents[parent];
            position = parents[position];
        }
        position
    }
    for left in 0..endpoints.len() {
        for right in 0..=left {
            if same_value(values, endpoints[left], endpoints[right]) {
                comparable[left] = true;
                comparable[right] = true;
                let left = representative(&mut parents, left);
                let right = representative(&mut parents, right);
                parents[left] = right;
            }
        }
    }
    let mut roots = Vec::new();
    let classes = (0..endpoints.len())
        .map(|position| {
            if !comparable[position] {
                return None;
            }
            let root = representative(&mut parents, position);
            Some(
                match roots.iter().position(|candidate| *candidate == root) {
                    Some(position) => position,
                    None => {
                        roots.push(root);
                        roots.len() - 1
                    }
                },
            )
        })
        .collect::<Vec<_>>();
    let mut successors = vec![Vec::new(); roots.len()];
    for (position, law) in laws.into_iter().enumerate() {
        let (Some(left), Some(right)) = (classes[position * 2 + 2], classes[position * 2 + 3])
        else {
            continue;
        };
        let requirements: Arc<[ContentConservationPlan]> = law.requirements.clone().into();
        successors[left].push(ContentConservationImplication {
            destination: right,
            requirements: Arc::clone(&requirements),
        });
        successors[right].push(ContentConservationImplication {
            destination: left,
            requirements,
        });
    }
    let proof = ContentConservationDependencies {
        initial: classes[0]?,
        goal: classes[1]?,
        successors,
    };
    // Availability is deliberately deferred, but a graph with no possible
    // proof is not a candidate. Require a law, not mere endpoint reflexivity.
    proof.derivation_depth(|_| Some(0)).map(|_| proof)
}

fn same_value(
    values: &ContentValues<'_, '_>,
    left: &ObservedContentTerm,
    right: &ObservedContentTerm,
) -> bool {
    same_term(left, right, &|left, right| {
        matches!(
            values.compare(BinaryOperator::Equal, left, right),
            StructuralJudgment::Proven
        )
    })
}

/// Separate is a multiset after actual substitution too. Source normalization
/// orders declared places, whose order need not match the caller's binders.
/// Each occurrence is consumed once, including repeated equal projections.
fn same_term(
    left: &ObservedContentTerm,
    right: &ObservedContentTerm,
    scalar_equal: &impl Fn(&StructuralTerm, &StructuralTerm) -> bool,
) -> bool {
    match (left, right) {
        (
            ObservedContentTerm::Projection {
                domain,
                semantic_domain,
                projection_machine,
                scalars,
            },
            ObservedContentTerm::Projection {
                domain: other_domain,
                semantic_domain: other_semantic_domain,
                projection_machine: other_projection,
                scalars: other_scalars,
            },
        ) => {
            domain == other_domain
                && semantic_domain == other_semantic_domain
                && projection_machine == other_projection
                && scalars.len() == other_scalars.len()
                && scalars
                    .iter()
                    .zip(other_scalars)
                    .all(|(left, right)| scalar_equal(left, right))
        }
        (ObservedContentTerm::Separate(terms), ObservedContentTerm::Separate(other)) => {
            if terms.len() != other.len() {
                return false;
            }
            let mut matched = vec![false; other.len()];
            for term in terms {
                let Some(index) = other.iter().enumerate().position(|(index, candidate)| {
                    !matched[index] && same_term(term, candidate, scalar_equal)
                }) else {
                    return false;
                };
                matched[index] = true;
            }
            true
        }
        _ => false,
    }
}
