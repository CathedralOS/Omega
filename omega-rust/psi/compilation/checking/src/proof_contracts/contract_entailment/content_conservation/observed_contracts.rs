//! Conservation laws on completed selected calls.
//!
//! A set equation does not identify its individual endpoints. Keep the exact
//! normalized algebra, projection owner and observed scalar operands together,
//! rather than feeding a set equality to ordinary structural substitution.
//! Entry operands are detached call-entry observations; current operands use
//! only the frame owner's surviving or fresh post-call observations. No law
//! introduces a qualification, backing, or authority to observe a referent.

use super::{ContentValues, StructuralJudge, StructuralJudgment, StructuralTerm};
use language_semantics::SemanticDomainId;
use language_semantics::content::{
    ContentAlgebraIdentity, ContentConservationPlan, ContentConservationTerm, ContentPlaceSegment,
    ContentPlaceVersion, ContentProjectionExpression,
};
use symbols::SymbolHandle;
use typed_trees::expression::BinaryOperator;
use typed_trees::signature::StateParameter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::proof_contracts::contract_entailment) struct ObservedContentEquation {
    algebra: ContentAlgebraIdentity,
    left: ObservedContentTerm,
    right: ObservedContentTerm,
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
        .filter_map(|source| observe_equation(&values, &source.plan))
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

pub(super) fn proves(values: &ContentValues<'_, '_>, plan: &ContentConservationPlan) -> bool {
    let Some(goal) = observe_equation(values, plan) else {
        return false;
    };
    let laws = &values.judge.normal_content_guarantees;
    // Each completed law has two fixed endpoints. Traversal is finite even
    // when a scalar observation's reflexive comparison is Unknown; proof
    // comparisons establish edges, never the identity of the work queue.
    let mut reached = vec![false; laws.len() * 2];
    let mut pending = vec![(None, &goal.left)];
    while let Some((endpoint, current)) = pending.pop() {
        // Reflexivity without a completed law remains the numeric judge's
        // obligation, including interval validity and separation.
        if endpoint.is_some() && same_value(values, current, &goal.right) {
            return true;
        }
        for (index, law) in laws.iter().enumerate() {
            if law.algebra != goal.algebra {
                continue;
            }
            for (destination, from, to) in [
                (index * 2 + 1, &law.left, &law.right),
                (index * 2, &law.right, &law.left),
            ] {
                if !reached[destination] && same_value(values, current, from) {
                    reached[destination] = true;
                    pending.push((Some(destination), to));
                }
            }
        }
    }
    false
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
