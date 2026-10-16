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
    for plan in plans {
        let mut definitions = program
            .domain_definitions()
            .iter()
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
    }
}
