//! Refutation of an admitted domain consumes completed values, never its own
//! membership claim. The normal-return owner captures constructors, locals and
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
    let Some(domain) = program.domain_definition(domain_symbol) else {
        return false;
    };
    if !domain.type_parameters.is_empty()
        || !domain.index_arguments.is_empty()
        || domain.alias.is_some()
    {
        return false;
    }
    let Some(reference) =
        crate::expression_result_type_reference(program, machine, state, expression)
    else {
        return false;
    };
    let mut visitor = DomainRefutation {
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
        domain,
        refuted: false,
    };
    observe_runtime_state_prefix(program, machine, state, &mut visitor).is_some() && visitor.refuted
}

struct DomainRefutation<'program> {
    site: StructuralPrefixSite,
    expression: ExpressionHandle,
    reference: typed_trees::types::TypeReferenceHandle,
    domain: &'program typed_trees::domain::DomainDefinition,
    refuted: bool,
}

impl StructuralPrefixVisitor for DomainRefutation<'_> {
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
            return;
        };
        let Some(outcomes) = judge.completed_statement_paths(
            self.site.statement_index,
            self.expression,
            environment,
            (machine, state, self.reference),
        ) else {
            return;
        };
        let parameters = [typed_trees::signature::StateParameter {
            symbol: self.domain.subject_symbol,
            name: self.domain.subject_name.clone(),
            type_reference: self.domain.target_type,
            ..Default::default()
        }];
        for outcome in outcomes {
            let bindings = [(self.domain.subject_name.to_string(), outcome.value)];
            self.refuted |= program
                .proof_facts(self.domain)
                .iter()
                .filter_map(|fact| fact.as_expression())
                .any(|expression| {
                    matches!(
                        instantiated_selected_fact_judgment(
                            program,
                            &outcome.judge,
                            self.domain.symbol,
                            &parameters,
                            *expression,
                            &bindings,
                            &[],
                            Some((machine, state)),
                        ),
                        StructuralJudgment::Refuted
                    )
                });
        }
    }
}
