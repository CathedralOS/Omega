//! Constructor admission observes children before the constructed value exists.
//!
//! The normal operation owner supplies once-only ordered operands and the facts
//! of their completed calls. Binding those snapshots to exact declared fields
//! preserves nested projections without importing the constructor's own invariant.
//! A copied field cannot become a later version of its source after a sibling
//! writes that source. Every reachable entry must establish each predicate.
//!
//! Closed literal operands have no storage observations or effects to replay.
//! Try their exact terms with an empty judge first, sharing the ordinary field
//! substitution and predicate judgment. This avoids repeatedly walking a large
//! enclosing constructor for each nested literal (notably reflected schemas).
//! Only an entirely proven result skips prefix observation; independent field,
//! omission and nested-construction admission still runs in the caller.

use super::refuted_requires::instantiated_selected_fact_judgment;
use super::structural_judgment::normal_returns::OperationOperandDemand;
use super::structural_judgment::{StructuralJudge, StructuralJudgment, StructuralTerm};
use super::structural_prefix::{
    PrefixObservation, StructuralPrefixSite, StructuralPrefixVisitor,
    expression_is_transition_target, observe_acyclic_invocation_prefix,
    observe_runtime_state_prefix,
};
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableStructLiteral};
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
    // The mathematical constructor owner proves the same declared predicates
    // under the exact immutable source telescope. Consume that checked lambda
    // at its own construction occurrence, not an invariant borrowed from the
    // destination or an unrelated constructor with the same representation.
    if program.prepared_mathematics().is_some_and(|view| {
        view.source_constructor(expression)
            .is_some_and(|constructor| {
                constructor.constructor == expression
                    && constructor.machine == machine.symbol
                    && constructor.state == state.symbol
                    && constructor.schema.is_valid()
                    && constructor.ty.is_valid()
                    && constructor.type_identity.is_valid()
                    && constructor.type_identity
                        == program.normalized_type_identity(constructor.source_type)
                    && predicates
                        .iter()
                        .all(|predicate| constructor.predicate_expressions.contains(predicate))
            })
    }) {
        return vec![StructuralJudgment::Proven; predicates.len()];
    }
    let mut demanded_fields = Vec::new();
    for &predicate in predicates {
        collect_predicate_fields(program, parameters, predicate, &mut demanded_fields);
    }
    // A source field selector can name its template while this construction
    // owns an applied declaration. Demand and substitution must consume the
    // same exact receiving-field correspondence.
    let selected_fields = program
        .expression_table
        .struct_fields(literal.fields)
        .iter()
        .map(|field| {
            crate::value_custody::struct_literals::selected_construction_field(
                program,
                expression,
                literal,
                field.field_symbol,
            )
            .map(|selected| selected.symbol)
            .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    let mut visitor = ConstructionPredicates {
        site: StructuralPrefixSite {
            state: state.symbol,
            statement_index,
            in_transition_target: false,
        },
        expression,
        literal,
        parameters,
        predicates,
        demanded_fields: &demanded_fields,
        selected_fields: &selected_fields,
        observed: false,
        judgments: vec![StructuralJudgment::Proven; predicates.len()],
    };
    if let Some(judgments) = visitor.closed_literal_judgments(program, machine, state) {
        return judgments;
    }
    // Searching the enclosing transition is part of prefix observation, not
    // literal proof. Delay its whole-root walk until that path is needed.
    visitor.site.in_transition_target =
        expression_is_transition_target(program, state, statement_index, expression);
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

/// Only the predicate's exact declared field subjects need value snapshots.
/// Other constructor operands still execute and establish their own validity;
/// their aggregate contents do not become premises of this query.
fn collect_predicate_fields(
    program: &dyn crate::CheckingInput,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
    fields: &mut Vec<symbols::SymbolHandle>,
) {
    if !expression.is_valid() {
        return;
    }
    if let typed_trees::expression::ExpressionNode::Name(path) =
        program.expression_table.expression(expression)
        && parameters
            .iter()
            .any(|parameter| parameter.symbol == path.symbol)
        && !fields.contains(&path.symbol)
    {
        fields.push(path.symbol);
    }
    crate::expression_traversal::children(
        program,
        program.expression_table.expression(expression),
        |child| collect_predicate_fields(program, parameters, child, fields),
    );
}

#[derive(Clone)]
struct ConstructionPredicates<'query> {
    site: StructuralPrefixSite,
    expression: ExpressionHandle,
    literal: &'query TableStructLiteral,
    parameters: &'query [StateParameter],
    predicates: &'query [ExpressionHandle],
    demanded_fields: &'query [symbols::SymbolHandle],
    selected_fields: &'query [symbols::SymbolHandle],
    observed: bool,
    judgments: Vec<StructuralJudgment>,
}

impl ConstructionPredicates<'_> {
    fn closed_literal_judgments(
        &self,
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
    ) -> Option<Vec<StructuralJudgment>> {
        let supplied = program.expression_table.struct_fields(self.literal.fields);
        if !supplied
            .iter()
            .all(|field| is_closed_literal(program, field.value))
        {
            return None;
        }
        // Do not intake state-entry domains or complete the constructor to get
        // its operands: either would allow its own obligation to become a premise.
        let judge = StructuralJudge::from_runtime_requires(program, machine, &[]);
        let judgments = self.judge_operands(program, machine, state, &judge, |position| {
            super::structural_terms::structural_term(
                program,
                Some((machine, state, Default::default())),
                None,
                supplied[position].value,
                None,
            )
        });
        judgments
            .iter()
            .all(|judgment| matches!(judgment, StructuralJudgment::Proven))
            .then_some(judgments)
    }

    fn judge_operands(
        &self,
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        judge: &StructuralJudge,
        mut supplied_value: impl FnMut(usize) -> Option<StructuralTerm>,
    ) -> Vec<StructuralJudgment> {
        let mut parameters = Vec::new();
        let mut bindings = Vec::new();
        let reference = crate::value_custody::struct_literals::construction_application(
            program,
            self.expression,
        );
        for (parameter, value) in judge.data_application_static_bindings(reference) {
            bindings.push((parameter.name.to_string(), value));
            parameters.push(parameter);
        }
        for parameter in self.parameters {
            if !self.demanded_fields.contains(&parameter.symbol) {
                continue;
            }
            let mut matches = self
                .selected_fields
                .iter()
                .enumerate()
                .filter(|(_, selected)| **selected == parameter.symbol);
            let value = if let Some((position, _)) = matches.next() {
                if matches.next().is_some() {
                    None
                } else {
                    supplied_value(position)
                }
            } else {
                // Independent omission checking still owns establishment.
                super::structural_terms::zero_value_structural_term(
                    program,
                    parameter.type_reference,
                )
            };
            // An unrelated field need not have a mathematical denotation.
            // Remove its formal as well as its binding so a caller namesake
            // cannot accidentally supply the missing observation.
            if let Some(value) = value {
                parameters.push(parameter.clone());
                bindings.push((parameter.name.to_string(), value));
            }
        }
        self.predicates
            .iter()
            .map(|&predicate| {
                instantiated_selected_fact_judgment(
                    program,
                    judge,
                    self.literal.type_symbol,
                    &parameters,
                    predicate,
                    &bindings,
                    &[],
                    Some((machine, state)),
                )
            })
            .collect()
    }

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

    fn unresolved(&mut self, machine: &Machine) {
        if super::struct_trace() {
            eprintln!(
                "STRUCT construction no entries machine={} site={:?} expression={:?}",
                machine.name, self.site.state, self.expression
            );
        }
        for predicate_index in 0..self.predicates.len() {
            self.retain(predicate_index, StructuralJudgment::Unknown);
        }
    }

    fn observe<'program>(&mut self, observation: PrefixObservation<'_, 'program>) {
        let (program, machine, state, judge, environment) = observation;
        let supplied = program.expression_table.struct_fields(self.literal.fields);
        let demanded_values = supplied
            .iter()
            .zip(self.selected_fields)
            .filter(|(_, selected)| self.demanded_fields.contains(selected))
            .map(|(field, _)| field.value)
            .collect::<Vec<_>>();
        let reference =
            crate::expression_result_type_reference(program, machine, state, self.expression)
                .unwrap_or_default();
        let entries = judge.normal_operation_entries(
            self.site.statement_index,
            self.expression,
            OperationOperandDemand::Selected(&demanded_values),
            environment,
            (machine, state, reference),
        );
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
        for entry in entries {
            self.observed = true;
            if entry.arguments.len() != supplied.len() {
                for predicate_index in 0..self.predicates.len() {
                    self.retain(predicate_index, StructuralJudgment::Unknown);
                }
                continue;
            }
            let judgments =
                self.judge_operands(program, machine, state, &entry.judge, |position| {
                    entry.arguments.get(position).cloned().flatten()
                });
            for (predicate_index, judgment) in judgments.into_iter().enumerate() {
                self.retain(predicate_index, judgment);
            }
        }
    }
}

/// Eligibility only, never an evaluator or a construction certificate. In
/// particular names, calls, casts and reads must use occurrence snapshots even
/// when the general term producer could assign them a mathematical denotation.
fn is_closed_literal(program: &dyn crate::CheckingInput, expression: ExpressionHandle) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::String(_) => true,
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .all(|field| is_closed_literal(program, field.value)),
        ExpressionNode::ArrayLiteral(elements) => program
            .expression_table
            .expression_handles(*elements)
            .iter()
            .all(|&element| is_closed_literal(program, element)),
        _ => false,
    }
}
