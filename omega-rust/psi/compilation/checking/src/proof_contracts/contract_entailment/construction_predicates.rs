//! Constructor admission observes children before the constructed value exists.
//!
//! The normal operation owner supplies once-only ordered operands and the facts
//! of their completed calls. Binding those snapshots to exact declared fields
//! preserves nested projections without importing the constructor's own invariant.
//! A copied field cannot become a later version of its source after a sibling
//! writes that source. Every reachable entry must establish each predicate.

use super::citations::CitationFacts;
use super::refuted_requires::instantiated_selected_fact_judgment;
use super::structural_case_arms::structural_arm_judge;
use super::structural_judgment::{StructuralJudge, StructuralJudgment, StructuralTerm};
use super::structural_prefix::{
    StructuralPrefixSite, StructuralPrefixVisitor, expression_is_transition_target,
    observe_acyclic_invocation_prefix, observe_runtime_state_prefix,
};
use typed_trees::expression::{ExpressionHandle, TableStructLiteral};
use typed_trees::signature::StateParameter;
use typed_trees::{machine::Machine, state::State};

pub(crate) fn construction_predicate_judgments(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    literal: &TableStructLiteral,
    parameters: &[StateParameter],
    predicates: &[ExpressionHandle],
) -> Vec<StructuralJudgment> {
    if predicates.is_empty() {
        return Vec::new();
    }
    let mut visitor = ConstructionPredicates {
        site: StructuralPrefixSite {
            state: state.symbol,
            statement_index,
            in_transition_target: expression_is_transition_target(
                program,
                state,
                statement_index,
                expression,
            ),
        },
        expression,
        literal,
        parameters,
        predicates,
        observed: false,
        judgments: vec![StructuralJudgment::Proven; predicates.len()],
    };
    let mut incoming = visitor.clone();
    let local_complete = observe_runtime_state_prefix(program, machine, state, &mut visitor)
        .is_some()
        && visitor.observed;
    if super::struct_trace() {
        eprintln!(
            "STRUCT construction machine={} state={} statement={statement_index} expression={expression:?} complete={local_complete} observed={}",
            machine.name, state.name, visitor.observed
        );
    }
    if local_complete
        && visitor
            .judgments
            .iter()
            .all(|judgment| matches!(judgment, StructuralJudgment::Proven))
    {
        return visitor.judgments;
    }
    // A local proof is sufficient, but an unknown local parameter may have a
    // stronger fact on every acyclic arrival. The same operation-entry observer
    // consumes those paths; no scalar source recognizer supplies a fallback.
    // Start a fresh accumulator, never preserving the local attempt's verdicts.
    if observe_acyclic_invocation_prefix(program, machine, &mut incoming).is_none()
        || !incoming.observed
    {
        return if local_complete {
            visitor.judgments
        } else {
            vec![StructuralJudgment::Unknown; predicates.len()]
        };
    }
    if local_complete {
        for (incoming, local) in incoming.judgments.iter_mut().zip(visitor.judgments) {
            if matches!(incoming, StructuralJudgment::Unknown)
                && matches!(local, StructuralJudgment::Proven)
            {
                *incoming = StructuralJudgment::Proven;
            }
        }
    }
    incoming.judgments
}

#[derive(Clone)]
struct ConstructionPredicates<'query> {
    site: StructuralPrefixSite,
    expression: ExpressionHandle,
    literal: &'query TableStructLiteral,
    parameters: &'query [StateParameter],
    predicates: &'query [ExpressionHandle],
    observed: bool,
    judgments: Vec<StructuralJudgment>,
}

impl ConstructionPredicates<'_> {
    fn retain(&mut self, predicate_index: usize, judgment: StructuralJudgment) {
        self.judgments[predicate_index] = match (self.judgments[predicate_index], judgment) {
            (StructuralJudgment::Refuted, _) | (_, StructuralJudgment::Refuted) => {
                StructuralJudgment::Refuted
            }
            (StructuralJudgment::Unknown, _) | (_, StructuralJudgment::Unknown) => {
                StructuralJudgment::Unknown
            }
            _ => StructuralJudgment::Proven,
        };
    }
}

impl StructuralPrefixVisitor for ConstructionPredicates<'_> {
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
        let entries = program
            .state_in_machine(machine, self.site.state)
            .and_then(|state| {
                let reference = crate::expression_result_type_reference(
                    program,
                    machine,
                    state,
                    self.expression,
                )
                .unwrap_or_default();
                judge.normal_operation_entries(
                    self.site.statement_index,
                    self.expression,
                    environment,
                    (machine, state, reference),
                )
            });
        let Some(entries) = entries else {
            if super::struct_trace() {
                eprintln!(
                    "STRUCT construction no entries machine={} site={:?} expression={:?}",
                    machine.name, self.site.state, self.expression
                );
            }
            for predicate_index in 0..self.predicates.len() {
                self.retain(predicate_index, StructuralJudgment::Unknown);
            }
            return;
        };
        let supplied = program.expression_table.struct_fields(self.literal.fields);
        for entry in entries {
            self.observed = true;
            if entry.arguments.len() != supplied.len() {
                for predicate_index in 0..self.predicates.len() {
                    self.retain(predicate_index, StructuralJudgment::Unknown);
                }
                continue;
            }
            let mut parameters = Vec::new();
            let mut bindings = Vec::new();
            for parameter in self.parameters {
                let mut matches = supplied.iter().enumerate().filter(|(_, field)| {
                    field.field_symbol.is_valid() && field.field_symbol == parameter.symbol
                });
                let value = if let Some((position, _)) = matches.next() {
                    if matches.next().is_some() {
                        None
                    } else {
                        entry.arguments.get(position).cloned()
                    }
                } else {
                    // Independent omission checking still owns establishment.
                    // An unrelated field need not have a mathematical denotation
                    // to prove a predicate about another field. Only observed
                    // fields enter the formal substitution; an unsupported field
                    // therefore declines instead of resolving a caller namesake.
                    super::structural_terms::zero_value_structural_term(
                        program,
                        parameter.type_reference,
                    )
                };
                if let Some(value) = value {
                    parameters.push(parameter.clone());
                    bindings.push((parameter.name.to_string(), value));
                }
            }
            for (predicate_index, &predicate) in self.predicates.iter().enumerate() {
                let judgment = instantiated_selected_fact_judgment(
                    program,
                    &entry.judge,
                    self.literal.type_symbol,
                    &parameters,
                    predicate,
                    &bindings,
                    &[],
                    program
                        .state_in_machine(machine, self.site.state)
                        .map(|state| (machine, state)),
                );
                self.retain(predicate_index, judgment);
            }
        }
    }
}
