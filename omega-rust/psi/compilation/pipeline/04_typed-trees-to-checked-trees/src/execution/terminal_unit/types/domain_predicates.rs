//! Retain complete declaration meaning, never an inferred write permission list.
//! Unknown definitions remain opaque. Normalized atoms close conjunctions before
//! this catalog; indexed/open definitions require their exact substituted theory.
use checked_trees::{CheckedOperatorFacts, CheckedUnitStructuralDomainPlan};
use typed_trees::types::TypeReferenceHandle;

pub(in crate::execution::terminal_unit) fn retain_domain_predicates(
    program: &dyn validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    plans: &mut [CheckedUnitStructuralDomainPlan],
) {
    for plan in plans.iter_mut() {
        let mut definitions = program
            .domain_definitions()
            .iter()
            .chain(
                program
                    .typed_program()
                    .applied_domain_applications
                    .iter()
                    .map(|application| &application.theory),
            )
            .filter(|domain| domain.semantic_id == plan.domain);
        let Some(domain) = definitions.next() else {
            continue;
        };
        if definitions.next().is_some() || domain.alias.is_some() {
            continue;
        }
        let parameters = [typed_trees::signature::StateParameter {
            symbol: domain.subject_symbol,
            name: domain.subject_name.clone(),
            type_reference: domain.target_type,
            ..Default::default()
        }];
        let mut remaining = 4096;
        let retained_memberships = plan.required_memberships.as_ref();
        let predicates = program
            .proof_facts(domain)
            .iter()
            .map(|fact| match fact {
                typed_trees::domain::ProofFact::Expression(expression) => {
                    crate::values::lower_callable_scalar_contract_predicate(
                        program,
                        operators,
                        domain.symbol,
                        &parameters,
                        TypeReferenceHandle::invalid(),
                        *expression,
                        false,
                        &mut remaining,
                    )
                }
                typed_trees::domain::ProofFact::Membership(membership) => {
                    if retained_memberships
                        .is_some_and(|required| required.contains(&membership.semantic_domain))
                    {
                        return Some(checked_trees::CheckedBooleanExpression::Constant(true));
                    }
                    crate::values::scalar::result_contract::lower_callable_scalar_membership(
                        program,
                        operators,
                        domain.symbol,
                        &parameters,
                        membership,
                        &mut remaining,
                    )
                }
                typed_trees::domain::ProofFact::Proposition(_) => None,
            })
            .collect::<Option<Vec<_>>>();
        plan.predicate = predicates.map(|predicates| {
            predicates
                .into_iter()
                .reduce(|left, right| checked_trees::CheckedBooleanExpression::And {
                    left: Box::new(left),
                    right: Box::new(right),
                })
                .unwrap_or(checked_trees::CheckedBooleanExpression::Constant(true))
        });
        // The old complete scalar projector can represent a different-subject
        // predicate membership entirely as a value clause. It refuses routed
        // members recursively. Keep that existing value lane; source-policy
        // completeness remains false when its nominal dependency is omitted.
        if plan.required_memberships.is_none() && plan.predicate.is_some() {
            plan.required_memberships = Some(Vec::new());
        }
    }

    fn complete(
        plans: &[CheckedUnitStructuralDomainPlan],
        index: usize,
        active: &mut Vec<usize>,
    ) -> Option<bool> {
        let plan = &plans[index];
        if active.contains(&index) {
            return None;
        }
        let Some(required) = &plan.required_memberships else {
            return Some(false);
        };
        active.push(index);
        let result = required
            .iter()
            .map(|domain| {
                let mut matches = plans.iter().enumerate().filter(|(_, candidate)| {
                    candidate.domain == *domain
                        && candidate.carrier_type_identity == plan.carrier_type_identity
                });
                let (parent, _) = matches.next()?;
                if matches.next().is_some() {
                    return None;
                }
                complete(plans, parent, active)
            })
            .collect::<Option<Vec<_>>>();
        active.pop();
        result.map(|parents| {
            plan.complete_predicate_theory
                && plan.predicate.is_some()
                && parents.into_iter().all(|parent| parent)
        })
    }
    let complete = (0..plans.len())
        .map(|index| complete(plans, index, &mut Vec::new()))
        .collect::<Vec<_>>();
    for (plan, complete) in plans.iter_mut().zip(complete) {
        if complete.is_none() {
            // A source relation that cannot be finitely represented remains
            // unknown; it never becomes a vacuous empty relation on the wire.
            plan.required_memberships = None;
            plan.predicate = None;
        }
        plan.complete_predicate_theory = complete == Some(true);
    }

    // Public predicates keep their original meaning: the entire value
    // component, including inherited predicates. Provenance stays in the
    // separate graph. Storage invalidation and ordinary value readers must
    // not mistake an empty local predicate for an observation-free child.
    fn value_predicate(
        plans: &[CheckedUnitStructuralDomainPlan],
        index: usize,
        active: &mut Vec<usize>,
        values: &mut [Option<Option<checked_trees::CheckedBooleanExpression>>],
    ) -> Option<checked_trees::CheckedBooleanExpression> {
        use checked_trees::CheckedBooleanExpression as Boolean;
        if let Some(value) = &values[index] {
            return value.clone();
        }
        if active.contains(&index) {
            return None;
        }
        let plan = &plans[index];
        let mut value = plan.predicate.clone()?;
        active.push(index);
        let result = (|| {
            if let Some(required) = &plan.required_memberships {
                for domain in required {
                    let mut matches = plans.iter().enumerate().filter(|(_, parent)| {
                        parent.domain == *domain
                            && parent.carrier_type_identity == plan.carrier_type_identity
                    });
                    let (parent, _) = matches.next()?;
                    if matches.next().is_some() {
                        return None;
                    }
                    let predicate = value_predicate(plans, parent, active, values)?;
                    value = match (value, predicate) {
                        (Boolean::Constant(true), right) => right,
                        (left, Boolean::Constant(true)) => left,
                        (Boolean::Constant(false), _) | (_, Boolean::Constant(false)) => {
                            Boolean::Constant(false)
                        }
                        (left, right) if left == right => left,
                        (left, right) => Boolean::And {
                            left: Box::new(left),
                            right: Box::new(right),
                        },
                    };
                }
            }
            Some(value)
        })();
        active.pop();
        values[index] = Some(result.clone());
        result
    }
    let mut values = vec![None; plans.len()];
    for index in 0..plans.len() {
        value_predicate(plans, index, &mut Vec::new(), &mut values);
    }
    for (plan, value) in plans.iter_mut().zip(values) {
        plan.predicate = value.flatten();
        plan.complete_predicate_theory &= plan.predicate.is_some();
    }
}
