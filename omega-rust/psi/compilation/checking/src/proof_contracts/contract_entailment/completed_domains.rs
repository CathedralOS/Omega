//! Domain predicates consume completed values, never their own membership
//! claim. Positive evidence requires every reachable observation to prove the
//! predicate; a single refuted conjunct still vetoes admitted membership.
//! The normal-return owner captures constructors, locals and
//! public call equations in evaluation order. Unsupported conjuncts stay
//! unknown and cannot hide another conjunct that is independently false.

use super::citations::CitationFacts;
use super::refuted_requires::instantiated_selected_fact_judgment;
use super::structural_case_arms::structural_arm_judge;
use super::structural_judgment::{StructuralJudge, StructuralJudgment, StructuralTerm};
use super::structural_prefix::{
    StructuralPrefixSite, StructuralPrefixVisitor, observe_runtime_state_prefix,
};
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
    if !typed_trees::domain::has_symbol_only_proof(program, domain_symbol) || domain.alias.is_some()
    {
        return CompletedDomainJudgment::default();
    }
    // Anonymous values receive the selected domain's carrier at this admission
    // site. A typed value retains its own actual instance, including generics.
    let reference = crate::expression_result_type_reference(program, machine, state, expression)
        .unwrap_or(domain.target_type);
    let mut visitor = DomainObservation {
        site: StructuralPrefixSite {
            state: state.symbol,
            statement_index,
            in_transition_target: super::structural_prefix::expression_is_transition_target(
                program,
                state,
                statement_index,
                expression,
            ),
        },
        expression,
        reference,
        projection,
        domain,
        predicate,
        judgment: CompletedDomainJudgment {
            proven: true,
            ..Default::default()
        },
    };
    if observe_runtime_state_prefix(program, machine, state, &mut visitor).is_none() {
        return CompletedDomainJudgment::default();
    }
    visitor.judgment
}

struct DomainObservation<'program> {
    site: StructuralPrefixSite,
    expression: ExpressionHandle,
    reference: typed_trees::types::TypeReferenceHandle,
    projection: &'program [facts::PlaceSegment],
    domain: &'program typed_trees::domain::DomainDefinition,
    predicate: ExpressionHandle,
    judgment: CompletedDomainJudgment,
}

impl StructuralPrefixVisitor for DomainObservation<'_> {
    fn site(&self) -> StructuralPrefixSite {
        self.site
    }

    fn visit<'program>(
        &mut self,
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        judge: &StructuralJudge<'program>,
        environment: &[(String, StructuralTerm)],
        hypotheses: &[(String, StructuralTerm)],
        equations: &[(StructuralTerm, StructuralTerm)],
        guarantees: &CitationFacts,
    ) {
        let mut judge = structural_arm_judge(program, judge, hypotheses, equations);
        if judge.hypotheses_contradictory {
            return;
        }
        guarantees.intake(&mut judge);
        let Some(state) = program.state_in_machine(machine, self.site.state) else {
            self.judgment.proven = false;
            return;
        };
        let Some(outcomes) = judge.completed_statement_paths(
            self.site.statement_index,
            self.expression,
            environment,
            (machine, state, self.reference),
        ) else {
            self.judgment.proven = false;
            return;
        };
        self.judgment.observed |= !outcomes.is_empty();
        let Some(reference) =
            crate::project_type_reference_from_segments(program, self.reference, self.projection)
        else {
            self.judgment.proven = false;
            return;
        };
        let parameters = [typed_trees::signature::StateParameter {
            symbol: self.domain.subject_symbol,
            name: self.domain.subject_name.clone(),
            // Carrier-only families observe this exact completed instance.
            // Indexed theories remain fenced above: their arguments cannot be
            // recovered from a family symbol.
            type_reference: reference,
            ..Default::default()
        }];
        for outcome in outcomes {
            let Some(value) = super::observed_projections::project_observed_value(
                program,
                self.reference,
                outcome.judge.completed_value_identity(outcome.value),
                self.projection,
            ) else {
                self.judgment.proven = false;
                continue;
            };
            let bindings = [(self.domain.subject_name.to_string(), value)];
            // Positive callers supply the declared predicate (or a Boolean
            // component of it); sibling memberships have their own evidence
            // owner. Refutation scans all expression conjuncts independently.
            let expressions = self
                .predicate
                .is_valid()
                .then_some(self.predicate)
                .into_iter()
                .chain(
                    program
                        .proof_facts(self.domain)
                        .iter()
                        .filter(|_| !self.predicate.is_valid())
                        .filter_map(|fact| fact.as_expression().copied()),
                );
            for expression in expressions {
                let judgment = instantiated_selected_fact_judgment(
                    program,
                    &outcome.judge,
                    self.domain.symbol,
                    &parameters,
                    expression,
                    &bindings,
                    &[],
                    Some((machine, state)),
                );
                self.judgment.proven &= matches!(judgment, StructuralJudgment::Proven);
                self.judgment.refuted |= matches!(judgment, StructuralJudgment::Refuted);
            }
        }
    }
}
