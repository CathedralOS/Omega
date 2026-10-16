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

use index_terms::{bound_index_arguments_equal, is_closed_index_argument};
use instance_collection::{
    CompatibilityKey, StateCallIndex, append_call_result_ensured_instances,
    collect_type_indexed_instances, construction_field_type, expression_indexed_instances,
    literal_element_type, selected_operation_count,
};
use labels::compatibility_name;
pub(crate) use substitutions::{
    BoundIndexArgument, BoundIndexSubstitutions, bound_index_substitutions,
};
use substitutions::{
    enclosing_call_substitutions, established_index_equalities, value_call_substitutions,
};
use unevidenced_diagnostics::append_unevidenced_establishment_diagnostics;

pub(crate) use index_terms::{bound_index_expressions_equal, expression_uses_binder};

pub(super) fn build_index_compatibility_facts(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    semantic: &FactPlan,
    flow: &FlowFacts,
    proof: &ProofFacts,
) -> Result<IndexCompatibilityFacts, Vec<Diagnostic>> {
    let mut conditions = Vec::new();
    let mut diagnostics = Vec::new();
    let mut unresolved = Vec::new();

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
            let Some(parameters) = validation::call_target_parameters(program, call.target_symbol)
            else {
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
                append_expression_compatibilities(
                    program,
                    operators,
                    semantic,
                    flow,
                    proof,
                    machine,
                    state,
                    call.statement_index,
                    *argument,
                    parameter.type_reference,
                    point,
                    call.entry_semantic_contexts,
                    &state_calls,
                    &mut conditions,
                    &mut diagnostics,
                    &mut unresolved,
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
            match statement {
                StatementNode::Assembly(_) => {}
                StatementNode::LocalData(local) => {
                    let contexts = state_calls.contexts_after_value(
                        statement_index,
                        local.initial_value,
                        statement_contexts,
                    );
                    append_expression_compatibilities(
                        program,
                        operators,
                        semantic,
                        flow,
                        proof,
                        machine,
                        state,
                        statement_index,
                        local.initial_value,
                        local.type_reference,
                        statement_point,
                        contexts,
                        &state_calls,
                        &mut conditions,
                        &mut diagnostics,
                        &mut unresolved,
                    );
                }
                StatementNode::Assignment(assignment) => {
                    if let Some(target_type) = validation::expression_type_reference_in_state(
                        program,
                        state.symbol,
                        statement_index,
                        assignment.target,
                    ) {
                        append_expression_compatibilities(
                            program,
                            operators,
                            semantic,
                            flow,
                            proof,
                            machine,
                            state,
                            statement_index,
                            assignment.value,
                            target_type,
                            statement_point,
                            state_calls.contexts_after_value(
                                statement_index,
                                assignment.value,
                                statement_contexts,
                            ),
                            &state_calls,
                            &mut conditions,
                            &mut diagnostics,
                            &mut unresolved,
                        );
                    }
                }
                StatementNode::Expression(expression)
                    if statement_index + 1 == statements.len() =>
                {
                    append_expression_compatibilities(
                        program,
                        operators,
                        semantic,
                        flow,
                        proof,
                        machine,
                        state,
                        statement_index,
                        *expression,
                        state.return_type,
                        ProgramPoint::Exit {
                            machine_symbol: machine.symbol,
                            state_symbol: state.symbol,
                            statement_index,
                            transition_target: Default::default(),
                        },
                        state_calls.contexts_after_value(
                            statement_index,
                            *expression,
                            statement_contexts,
                        ),
                        &state_calls,
                        &mut conditions,
                        &mut diagnostics,
                        &mut unresolved,
                    );
                }
                StatementNode::Transition(transition) => {
                    for target in transition.targets() {
                        let TransitionTargetNode::Value(value) =
                            program.statement_table.transition_target(target)
                        else {
                            continue;
                        };
                        append_expression_compatibilities(
                            program,
                            operators,
                            semantic,
                            flow,
                            proof,
                            machine,
                            state,
                            statement_index,
                            *value,
                            state.return_type,
                            ProgramPoint::Exit {
                                machine_symbol: machine.symbol,
                                state_symbol: state.symbol,
                                statement_index,
                                transition_target: target,
                            },
                            state_calls.contexts_after_value(
                                statement_index,
                                *value,
                                statement_contexts,
                            ),
                            &state_calls,
                            &mut conditions,
                            &mut diagnostics,
                            &mut unresolved,
                        );
                    }
                }
                StatementNode::RootBinding(_)
                | StatementNode::AssemblyFact(_)
                | StatementNode::Call(_)
                | StatementNode::Expression(_) => {}
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(IndexCompatibilityFacts { conditions })
    } else {
        Err(diagnostics)
    }
}

#[allow(clippy::too_many_arguments)]
fn append_expression_compatibilities(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    semantic: &FactPlan,
    flow: &FlowFacts,
    proof: &ProofFacts,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    value: ExpressionHandle,
    target_type: TypeReferenceHandle,
    point: ProgramPoint,
    contexts: HandleSpan<FlowSemanticContextRef>,
    state_calls: &StateCallIndex<'_, '_>,
    conditions: &mut Vec<IndexCompatibilityFact>,
    diagnostics: &mut Vec<Diagnostic>,
    unresolved: &mut Vec<CompatibilityKey>,
) {
    if !value.is_valid() || !target_type.is_valid() {
        return;
    }
    // A value-position call's result is checked after that call has completed,
    // so its own established `ensures` are part of the exact local context at
    // this boundary. Recompute this for recursive literal members as well as
    // top-level stores/returns.
    let contexts = state_calls.contexts_after_value(statement_index, value, contexts);
    let mut substitutions = enclosing_call_substitutions(program, state_calls, point);
    for substitution in value_call_substitutions(program, value) {
        if !substitutions
            .iter()
            .any(|(binder, _)| *binder == substitution.0)
        {
            substitutions.push(substitution);
        }
    }
    let mut actual =
        expression_indexed_instances(program, operators, machine, state, statement_index, value);
    append_call_result_ensured_instances(
        program,
        proof,
        state_calls,
        machine,
        state,
        statement_index,
        value,
        &mut actual,
    );
    let mut expected = Vec::new();
    collect_type_indexed_instances(program, target_type, &mut expected, &mut Vec::new());
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
            let discharge = if actual.semantic_id == expected.semantic_id {
                if actual
                    .arguments
                    .iter()
                    .chain(&expected.arguments)
                    .all(|argument| is_closed_index_argument(program, *argument, &Vec::new()))
                {
                    IndexCompatibilityDischarge::ClosedEvaluation
                } else {
                    IndexCompatibilityDischarge::LicensedNormalization {
                        operation_count: selected_operation_count(
                            program,
                            actual.arguments.iter().chain(&expected.arguments).copied(),
                        ),
                    }
                }
            } else if bound_index_arguments_equal(
                program,
                &actual.arguments,
                &expected.arguments,
                &substitutions,
            ) {
                // Simultaneous call bindings and the same selected laws compose
                // in the canonical normalizer, without manufacturing typed nodes.
                if actual
                    .arguments
                    .iter()
                    .chain(&expected.arguments)
                    .all(|argument| is_closed_index_argument(program, *argument, &substitutions))
                {
                    IndexCompatibilityDischarge::ClosedEvaluation
                } else {
                    IndexCompatibilityDischarge::LicensedNormalization {
                        operation_count: selected_operation_count(
                            program,
                            actual.arguments.iter().chain(&expected.arguments).copied(),
                        ),
                    }
                }
            } else if let Some(facts) = established_index_equalities(
                program,
                operators,
                semantic,
                flow,
                contexts,
                &actual.arguments,
                &expected.arguments,
                &substitutions,
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
                if !unresolved.contains(&key) {
                    unresolved.push(key);
                    let name = compatibility_name(program, actual, expected, point);
                    diagnostics.push(Diagnostic::error(format!(
                        "index compatibility condition `{name}` is not established: actual `{}` \
                         and expected `{}` are distinct normalized instances; closed evaluation, \
                         licensed normalization, or an exact local equality fact is required",
                        actual.label, expected.label,
                    )));
                }
                continue;
            };
            let name = compatibility_name(program, actual, expected, point);
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
            if !conditions.iter().any(|existing| {
                existing.point == candidate.point
                    && existing.value == candidate.value
                    && existing.target_type == candidate.target_type
                    && existing.family == candidate.family
                    && existing.actual_instance == candidate.actual_instance
                    && existing.expected_instance == candidate.expected_instance
                    && existing.actual_label == candidate.actual_label
                    && existing.expected_label == candidate.expected_label
            }) {
                conditions.push(candidate);
            }
        }
    }

    append_unevidenced_establishment_diagnostics(
        program,
        value,
        target_type,
        point,
        &actual,
        &expected,
        diagnostics,
        unresolved,
    );

    match program.expression_table.expression(value) {
        ExpressionNode::StructLiteral(literal) => {
            if let Some(definition) = program
                .data_definition_by_name_str(literal.type_name.as_str())
                .filter(|definition| definition.type_parameters.is_empty())
            {
                for field in program.expression_table.struct_fields(literal.fields) {
                    let Some(field_type) = construction_field_type(
                        program,
                        definition,
                        literal.case_name.as_ref().map(|name| name.as_str()),
                        field.name.as_str(),
                    ) else {
                        continue;
                    };
                    append_expression_compatibilities(
                        program,
                        operators,
                        semantic,
                        flow,
                        proof,
                        machine,
                        state,
                        statement_index,
                        field.value,
                        field_type,
                        point,
                        contexts,
                        state_calls,
                        conditions,
                        diagnostics,
                        unresolved,
                    );
                }
            }
        }
        ExpressionNode::ArrayLiteral(elements) => {
            if let Some(element_type) = literal_element_type(program, target_type) {
                for element in program.expression_table.expression_handles(*elements) {
                    append_expression_compatibilities(
                        program,
                        operators,
                        semantic,
                        flow,
                        proof,
                        machine,
                        state,
                        statement_index,
                        *element,
                        element_type,
                        point,
                        contexts,
                        state_calls,
                        conditions,
                        diagnostics,
                        unresolved,
                    );
                }
            }
        }
        _ => {}
    }
}
