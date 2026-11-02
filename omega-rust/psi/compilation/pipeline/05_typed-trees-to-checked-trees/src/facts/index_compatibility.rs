use crate::flow::expressions;
use arena::HandleSpan;
use checked_trees::{
    CheckedOperatorFacts, FlowFacts, FlowSemanticContextRef, IndexCompatibilityDischarge,
    IndexCompatibilityFact, IndexCompatibilityFacts, ProofFacts,
};
use diagnostics::Diagnostic;
use facts::{FactPlan, ProgramPoint};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionTargetNode};
use typed_trees::types::TypeReferenceHandle;

mod index_terms;
mod instance_collection;
mod labels;
mod substitutions;
mod unevidenced_diagnostics;

pub(crate) use index_terms::index_identity_bindings;
use index_terms::{bound_index_arguments_equal, is_closed_index_argument};
use instance_collection::{
    CompatibilityKey, StateCallIndex, append_call_result_ensured_instances,
    collect_type_indexed_instances, construction_field_type, expression_indexed_instances,
    literal_element_type, selected_operation_count,
};
use labels::compatibility_name;
pub(crate) use substitutions::{
    BoundIndexArgument, BoundIndexSubstitutions, call_index_substitutions,
};
use substitutions::{
    enclosing_call_substitutions, established_index_equalities, value_call_substitutions,
};
use unevidenced_diagnostics::append_unevidenced_establishment_diagnostics;

pub(super) fn build_index_compatibility_facts(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    semantic: &FactPlan,
    flow: &FlowFacts,
    proof: &ProofFacts,
) -> Result<IndexCompatibilityFacts, Vec<Diagnostic>> {
    let mut scan = CompatibilityScan {
        program,
        operators,
        semantic,
        flow,
        proof,
        conditions: Vec::new(),
        diagnostics: Vec::new(),
        unresolved: Vec::new(),
    };

    for (_, state_flow) in flow.control.states.iter() {
        let Some((machine, state)) = validation::machine_state_by_symbols(
            program,
            state_flow.machine_symbol,
            state_flow.state_symbol,
        ) else {
            continue;
        };

        let state_calls = StateCallIndex::new(program, flow, state_flow);
        for resolved in &state_calls.calls {
            let call = resolved.fact;
            let Some(parameters) = resolved.parameters(program, machine.symbol) else {
                continue;
            };
            let arguments = validation::call_site_argument_expressions(program, &resolved.site);
            let point = ProgramPoint::Call {
                machine_symbol: state_flow.machine_symbol,
                state_symbol: state_flow.state_symbol,
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
            };
            for (argument, parameter) in arguments.iter().zip(parameters.iter()) {
                scan.append(
                    machine,
                    state,
                    &state_calls,
                    call.statement_index,
                    *argument,
                    parameter.type_reference,
                    point,
                    call.entry_semantic_contexts,
                );
            }
        }

        let statements = program.statement_table.statements(state.statement_nodes);
        for (statement_index, statement) in statements.iter().enumerate() {
            let statement_contexts = flow
                .state_statement(state_flow, statement_index)
                .map(|statement| statement.entry_semantic_contexts)
                .unwrap_or(state_flow.entry_semantic_contexts);
            let statement_point = ProgramPoint::Statement {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
                statement_index,
            };
            // A statement form names only the value position it carries: the
            // scan's fixed scope — the check inputs, this machine and state, the
            // call index and the statement's ordinal — is applied once here.
            let mut append =
                |value: ExpressionHandle, target_type: TypeReferenceHandle, point: ProgramPoint| {
                    scan.append(
                        machine,
                        state,
                        &state_calls,
                        statement_index,
                        value,
                        target_type,
                        point,
                        statement_contexts,
                    );
                };
            let exit_point = |transition_target| ProgramPoint::Exit {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
                statement_index,
                transition_target,
            };
            match statement {
                StatementNode::Assembly(_) => {}
                StatementNode::LocalData(local) => {
                    append(local.initial_value, local.type_reference, statement_point);
                }
                StatementNode::Assignment(assignment) => {
                    if let Some(target_type) = validation::expression_type_reference_in_state(
                        program,
                        state.symbol,
                        statement_index,
                        assignment.target,
                    ) {
                        append(assignment.value, target_type, statement_point);
                    }
                }
                StatementNode::Expression(expression)
                    if statement_index + 1 == statements.len() =>
                {
                    append(
                        *expression,
                        state.return_type,
                        exit_point(Default::default()),
                    );
                }
                StatementNode::Transition(transition) => {
                    for target in transition.targets() {
                        let TransitionTargetNode::Value(value) =
                            program.statement_table.transition_target(target)
                        else {
                            continue;
                        };
                        append(*value, state.return_type, exit_point(target));
                    }
                }
                StatementNode::AssemblyFact(_)
                | StatementNode::Call(_)
                | StatementNode::Expression(_) => {}
            }
        }
    }

    if scan.diagnostics.is_empty() {
        Ok(IndexCompatibilityFacts {
            conditions: scan.conditions,
        })
    } else {
        Err(scan.diagnostics)
    }
}

/// One index-compatibility scan: the fixed check scope every value position
/// resolves against, plus the three accumulators it appends to. Stating the
/// scope once keeps each occurrence to the value position it contributes.
struct CompatibilityScan<'program> {
    program: &'program dyn ::validation::CheckingInput,
    operators: &'program CheckedOperatorFacts,
    semantic: &'program FactPlan,
    flow: &'program FlowFacts,
    proof: &'program ProofFacts,
    conditions: Vec<IndexCompatibilityFact>,
    diagnostics: Vec<Diagnostic>,
    unresolved: Vec<CompatibilityKey>,
}

impl CompatibilityScan<'_> {
    /// Check one value position: the value's indexed instances against the
    /// receiving type's, then the children the value carries. `contexts` is the
    /// statement's fallback local context; the value's own call result `ensures`
    /// are applied here, so a caller never pre-resolves that span.
    fn append(
        &mut self,
        machine: &Machine,
        state: &State,
        state_calls: &StateCallIndex<'_, '_>,
        statement_index: usize,
        value: ExpressionHandle,
        target_type: TypeReferenceHandle,
        point: ProgramPoint,
        contexts: HandleSpan<FlowSemanticContextRef>,
    ) {
        if !value.is_valid() || !target_type.is_valid() {
            return;
        }
        // A value-position call's result is checked after that call has completed,
        // so its own established `ensures` are part of the exact local context at
        // this boundary. Recompute this for recursive literal members as well as
        // top-level stores/returns.
        let contexts = state_calls.contexts_after_value(statement_index, value, contexts);
        let expected_substitutions =
            enclosing_call_substitutions(self.program, machine.symbol, state_calls, point);
        // Two nested calls to the same declaration share binder identities but
        // not actuals. Normalize the produced value and receiving formal in their
        // own invocation namespaces, including when their template IDs are equal.
        let actual_substitutions = value_call_substitutions(self.program, machine.symbol, value);
        let mut actual = expression_indexed_instances(
            self.program,
            self.operators,
            machine,
            state,
            statement_index,
            value,
        );
        append_call_result_ensured_instances(
            self.program,
            self.proof,
            state_calls,
            machine,
            state,
            statement_index,
            value,
            &mut actual,
        );
        let mut expected = Vec::new();
        collect_type_indexed_instances(self.program, target_type, &mut expected, &mut Vec::new());
        for actual in &actual {
            for expected in expected
                .iter()
                .filter(|expected| expected.family == actual.family)
            {
                if !actual.semantic_id.is_valid()
                    || !expected.semantic_id.is_valid()
                    || actual.arguments.is_empty()
                    || expected.arguments.is_empty()
                {
                    continue;
                }
                let discharge = if bound_index_arguments_equal(
                    self.program,
                    &actual.arguments,
                    &expected.arguments,
                    &actual_substitutions,
                    &expected_substitutions,
                ) {
                    // Simultaneous call bindings and the same selected laws compose
                    // in the canonical normalizer, without manufacturing typed nodes.
                    if actual.arguments.iter().all(|argument| {
                        is_closed_index_argument(self.program, *argument, &actual_substitutions)
                    }) && expected.arguments.iter().all(|argument| {
                        is_closed_index_argument(self.program, *argument, &expected_substitutions)
                    }) {
                        IndexCompatibilityDischarge::ClosedEvaluation
                    } else {
                        IndexCompatibilityDischarge::LicensedNormalization {
                            operation_count: selected_operation_count(
                                self.program,
                                actual.arguments.iter().chain(&expected.arguments).copied(),
                            ),
                        }
                    }
                } else if let Some(facts) = established_index_equalities(
                    self.program,
                    self.operators,
                    self.semantic,
                    self.flow,
                    contexts,
                    &actual.arguments,
                    &expected.arguments,
                    &actual_substitutions,
                    &expected_substitutions,
                ) {
                    IndexCompatibilityDischarge::EstablishedLocalFacts { facts }
                } else {
                    let key = CompatibilityKey {
                        point,
                        value,
                        target_type,
                        family: actual.family,
                        actual: actual.semantic_id,
                        expected: expected.semantic_id,
                    };
                    if !self.unresolved.contains(&key) {
                        self.unresolved.push(key);
                        let name = compatibility_name(self.program, actual, expected, point);
                        self.diagnostics.push(Diagnostic::error(format!(
                        "index compatibility condition `{name}` is not established: actual `{}` \
                         and expected `{}` are distinct normalized instances; closed evaluation, \
                         licensed normalization, or an exact local equality fact is required",
                        actual.label, expected.label,
                    )));
                    }
                    continue;
                };
                let name = compatibility_name(self.program, actual, expected, point);
                let candidate = IndexCompatibilityFact {
                    name,
                    point,
                    value,
                    target_type,
                    family: actual.family,
                    actual_instance: actual.semantic_id,
                    expected_instance: expected.semantic_id,
                    actual_label: actual.label.clone(),
                    expected_label: expected.label.clone(),
                    discharge,
                };
                if !self.conditions.iter().any(|existing| {
                    existing.point == candidate.point
                        && existing.value == candidate.value
                        && existing.target_type == candidate.target_type
                        && existing.family == candidate.family
                        && existing.actual_instance == candidate.actual_instance
                        && existing.expected_instance == candidate.expected_instance
                        && existing.actual_label == candidate.actual_label
                        && existing.expected_label == candidate.expected_label
                }) {
                    self.conditions.push(candidate);
                }
            }
        }

        append_unevidenced_establishment_diagnostics(
            self.program,
            value,
            target_type,
            point,
            &actual,
            &expected,
            &mut self.diagnostics,
            &mut self.unresolved,
        );
        self.append_children(
            machine,
            state,
            state_calls,
            statement_index,
            value,
            target_type,
            point,
            contexts,
        );
    }

    /// The nested value positions one checked value carries: a constructor's
    /// fields at their declared types, an array literal's elements at the
    /// element type, and a match dispatch's arms at the receiving type.
    fn append_children(
        &mut self,
        machine: &Machine,
        state: &State,
        state_calls: &StateCallIndex<'_, '_>,
        statement_index: usize,
        value: ExpressionHandle,
        target_type: TypeReferenceHandle,
        point: ProgramPoint,
        contexts: HandleSpan<FlowSemanticContextRef>,
    ) {
        let mut children = Vec::new();
        match self.program.expression_table.expression(value) {
            ExpressionNode::StructLiteral(literal) => {
                if let Some(definition) = self
                    .program
                    .data_definition_by_name_str(literal.type_name.as_str())
                    .filter(|definition| definition.type_parameters.is_empty())
                {
                    for field in self.program.expression_table.struct_fields(literal.fields) {
                        if let Some(field_type) = construction_field_type(
                            self.program,
                            definition,
                            literal.case_name.as_ref().map(|name| name.as_str()),
                            field.name.as_str(),
                        ) {
                            children.push((field.value, field_type));
                        }
                    }
                }
            }
            ExpressionNode::ArrayLiteral(elements) => {
                if let Some(element_type) = literal_element_type(self.program, target_type) {
                    children.extend(
                        expressions(self.program)
                            .expression_handles(*elements)
                            .iter()
                            .map(|element| (*element, element_type)),
                    );
                }
            }
            ExpressionNode::Match(dispatch) => {
                children.extend(
                    expressions(self.program)
                        .match_arms(dispatch.arms)
                        .iter()
                        .map(|arm| (arm.value, target_type)),
                );
            }
            _ => {}
        }
        for (value, target_type) in children {
            self.append(
                machine,
                state,
                state_calls,
                statement_index,
                value,
                target_type,
                point,
                contexts,
            );
        }
    }
}
