//! Domain predicates consume completed values, never their own membership
//! claim. Positive evidence requires every reachable observation to prove the
//! predicate; a single refuted conjunct still vetoes admitted membership.
//! The normal-return owner captures constructors, locals and
//! public call equations in evaluation order. Unsupported conjuncts stay
//! unknown and cannot hide another conjunct that is independently false.

use super::refuted_requires::instantiated_selected_fact_judgment;
use super::structural_judgment::StructuralJudgment;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::{machine::Machine, state::State};

/// A known false predicate on an exact completed value vetoes admission.
/// Failure to observe a value or decide a predicate supplies no verdict.
pub fn completed_value_refutes_domain(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    completed_domain_judgment(
        program,
        machine,
        state,
        statement_index,
        expression,
        domain_symbol,
        ExpressionHandle::invalid(),
        &[],
    )
    .refuted
}

/// Establish a declared predicate on the value captured at its evaluation
/// occurrence. This proves truth only; callers retain qualification authority.
pub fn completed_value_proves_domain_predicate(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    domain_symbol: SymbolHandle,
    predicate: ExpressionHandle,
    projection: &[facts::PlaceSegment],
) -> bool {
    let judgment = completed_domain_judgment(
        program,
        machine,
        state,
        statement_index,
        expression,
        domain_symbol,
        predicate,
        projection,
    );
    judgment.observed && judgment.proven
}

/// Prove a predicate from the exact already-applied theory. The selected
/// tuple supplies substitution only; the observed value must establish truth.
pub fn completed_value_proves_bound_domain_predicate(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    domain: &typed_trees::domain::DomainDefinition,
    predicate: ExpressionHandle,
    projection: &[facts::PlaceSegment],
) -> bool {
    let judgment = completed_bound_domain_judgment(
        program,
        machine,
        state,
        statement_index,
        expression,
        domain,
        predicate,
        projection,
    );
    if super::struct_trace() {
        eprintln!(
            "STRUCT completed-bound-domain machine={} domain={} instance={:?} observed={} proven={}",
            machine.name, domain.name, domain.semantic_id, judgment.observed, judgment.proven
        );
    }
    judgment.observed && judgment.proven
}

#[derive(Default)]
struct CompletedDomainJudgment {
    observed: bool,
    proven: bool,
    refuted: bool,
}

fn completed_domain_judgment(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    domain_symbol: SymbolHandle,
    predicate: ExpressionHandle,
    projection: &[facts::PlaceSegment],
) -> CompletedDomainJudgment {
    let Some(domain) = program.domain_definition(domain_symbol) else {
        return CompletedDomainJudgment::default();
    };
    completed_bound_domain_judgment(
        program,
        machine,
        state,
        statement_index,
        expression,
        domain,
        predicate,
        projection,
    )
}

fn completed_bound_domain_judgment(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    domain: &typed_trees::domain::DomainDefinition,
    predicate: ExpressionHandle,
    projection: &[facts::PlaceSegment],
) -> CompletedDomainJudgment {
    if !typed_trees::domain::has_bound_symbol_only_proof(program, domain) || domain.alias.is_some()
    {
        return CompletedDomainJudgment::default();
    }
    // Anonymous values receive the selected domain's carrier at this admission
    // site. A typed value retains its own actual instance, including generics.
    let reference = crate::expression_result_type_reference(program, machine, state, expression)
        .unwrap_or(domain.target_type);
    let mut judgment = CompletedDomainJudgment {
        proven: true,
        ..Default::default()
    };
    let coverage = super::completed_observations::observe_completed_values(
        program,
        machine,
        state,
        statement_index,
        expression,
        reference,
        |judge, value| {
            let Some(projected_reference) =
                crate::project_type_reference_from_segments(program, reference, projection)
            else {
                judgment.proven = false;
                return;
            };
            let parameters = [typed_trees::signature::StateParameter {
                symbol: domain.subject_symbol,
                name: domain.subject_name.clone(),
                // Carrier-only families observe this exact completed instance.
                // Indexed theories remain fenced above: their arguments cannot be
                // recovered from a family symbol.
                type_reference: projected_reference,
                ..Default::default()
            }];
            let Some(value) = super::observed_projections::project_observed_value(
                program, reference, value, projection,
            ) else {
                judgment.proven = false;
                return;
            };
            let bindings = [(domain.subject_name.to_string(), value)];
            // Positive callers supply the declared predicate (or a Boolean
            // component of it); sibling memberships have their own evidence
            // owner. Refutation scans all expression conjuncts independently.
            let expressions = predicate.is_valid().then_some(predicate).into_iter().chain(
                program
                    .proof_facts(domain)
                    .iter()
                    .filter(|_| !predicate.is_valid())
                    .filter_map(|fact| fact.as_expression().copied()),
            );
            for expression in expressions {
                let predicate_judgment = instantiated_selected_fact_judgment(
                    program,
                    judge,
                    domain.symbol,
                    &parameters,
                    expression,
                    &bindings,
                    &[],
                    Some((machine, state)),
                );
                judgment.proven &= matches!(predicate_judgment, StructuralJudgment::Proven);
                judgment.refuted |= matches!(predicate_judgment, StructuralJudgment::Refuted);
            }
        },
    );
    let Some(complete) = coverage else {
        return CompletedDomainJudgment::default();
    };
    judgment.observed = complete;
    judgment.proven &= complete;
    judgment
}
