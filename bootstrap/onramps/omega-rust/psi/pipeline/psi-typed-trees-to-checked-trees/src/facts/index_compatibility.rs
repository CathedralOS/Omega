use psi_arena::HandleSpan;
use psi_checked_trees::{
    CheckedOperatorFacts, FlowFacts, FlowSemanticContextRef, IndexCompatibilityDischarge,
    IndexCompatibilityFact, IndexCompatibilityFacts,
};
use psi_diagnostics::Diagnostic;
use psi_facts::{FactHandle, FactPayload, FactPlan, ProgramPoint};
use psi_language_semantics::SemanticDomainId;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::DataMember;
use psi_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;
use psi_typed_trees::statement::{StatementNode, TransitionTargetNode};
use psi_typed_trees::types::{
    DomainConstraint, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

#[derive(Debug, Clone)]
struct IndexedInstance {
    family: SymbolHandle,
    semantic_id: SemanticDomainId,
    arguments: Vec<TypeReferenceHandle>,
    label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CompatibilityKey {
    point: ProgramPoint,
    value: ExpressionHandle,
    target_type: TypeReferenceHandle,
    family: SymbolHandle,
    actual: SemanticDomainId,
    expected: SemanticDomainId,
}

struct ResolvedStateCall<'program, 'flow> {
    fact: &'flow psi_checked_trees::FlowCallFact,
    site: crate::CallSite<'program>,
}

struct StateCallIndex<'program, 'flow> {
    calls: Vec<ResolvedStateCall<'program, 'flow>>,
}

impl<'program, 'flow> StateCallIndex<'program, 'flow> {
    fn new(
        program: &'program TypedTrees,
        flow: &'flow FlowFacts,
        state_flow: &'flow psi_checked_trees::FlowStateFact,
    ) -> Self {
        let calls = flow
            .control
            .calls
            .span_or_empty(state_flow.calls)
            .iter()
            .filter_map(|fact| {
                crate::find_call_site(
                    program,
                    state_flow.machine_symbol,
                    state_flow.state_symbol,
                    fact.statement_index,
                    fact.call_ordinal,
                )
                .map(|site| ResolvedStateCall { fact, site })
            })
            .collect();
        Self { calls }
    }

    fn contexts_after_value(
        &self,
        statement_index: usize,
        value: ExpressionHandle,
        fallback: HandleSpan<FlowSemanticContextRef>,
    ) -> HandleSpan<FlowSemanticContextRef> {
        self.calls
            .iter()
            .filter(|call| call.fact.statement_index == statement_index)
            .find_map(|call| match &call.site {
                crate::CallSite::Expression { expression, .. } if *expression == value => {
                    Some(call.fact.exit_semantic_contexts)
                }
                _ => None,
            })
            .unwrap_or(fallback)
    }
}

pub(super) fn build_index_compatibility_facts(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    semantic: &FactPlan,
    flow: &FlowFacts,
) -> Result<IndexCompatibilityFacts, Vec<Diagnostic>> {
    let mut conditions = Vec::new();
    let mut diagnostics = Vec::new();
    let mut unresolved = Vec::new();

    for (_, state_flow) in flow.control.states.iter() {
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == state_flow.machine_symbol)
        else {
            continue;
        };
        let Some(state) = crate::find_state_in_machine(
            program,
            state_flow.machine_symbol,
            state_flow.state_symbol,
        ) else {
            continue;
        };

        let state_calls = StateCallIndex::new(program, flow, state_flow);
        for resolved in &state_calls.calls {
            let call = resolved.fact;
            let Some(parameters) = crate::call_target_parameters(program, call.target_symbol)
            else {
                continue;
            };
            let arguments = crate::call_site_argument_expressions(program, &resolved.site);
            let point = ProgramPoint::Call {
                machine_symbol: state_flow.machine_symbol,
                state_symbol: state_flow.state_symbol,
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
            };
            for (argument, parameter) in arguments
                .iter()
                .zip(parameters.iter().filter(|parameter| !parameter.is_self))
            {
                append_expression_compatibilities(
                    program,
                    operators,
                    semantic,
                    flow,
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
                    if let Some(target_type) = crate::flow::expression_type_reference_in_state(
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
                        machine,
                        state,
                        statement_index,
                        *expression,
                        state.return_type,
                        ProgramPoint::Exit {
                            machine_symbol: machine.symbol,
                            state_symbol: state.symbol,
                            statement_index,
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
                    for target in [transition.target, transition.continuation] {
                        if !target.is_valid() {
                            continue;
                        }
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
                            machine,
                            state,
                            statement_index,
                            *value,
                            state.return_type,
                            ProgramPoint::Exit {
                                machine_symbol: machine.symbol,
                                state_symbol: state.symbol,
                                statement_index,
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
                StatementNode::AssemblyFact(_)
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
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    semantic: &FactPlan,
    flow: &FlowFacts,
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
    let actual =
        expression_indexed_instances(program, operators, machine, state, statement_index, value);
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
                    .all(|argument| is_closed_index_argument(program, *argument))
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
                semantic,
                flow,
                contexts,
                &actual.arguments,
                &expected.arguments,
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

    match program.expression_table.expression(value) {
        ExpressionNode::StructLiteral(literal) => {
            if let Some(definition) = program
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == literal.type_name.as_str())
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

fn construction_field_type(
    program: &TypedTrees,
    definition: &psi_typed_trees::data::DataDefinition,
    case_name: Option<&str>,
    field_name: &str,
) -> Option<TypeReferenceHandle> {
    if let Some(case_name) = case_name
        && let Some(variant) =
            program
                .data_members(definition)
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(variant) if variant.name.as_str() == case_name => {
                        Some(variant)
                    }
                    _ => None,
                })
    {
        for field in program.data_payload_fields(variant) {
            if field.name.as_str() == field_name && field.type_reference.is_valid() {
                return Some(field.type_reference);
            }
        }
    }
    program.data_members(definition).iter().find_map(|member| {
        let DataMember::Field(field) = member else {
            return None;
        };
        (field.name.as_str() == field_name && field.type_reference.is_valid())
            .then_some(field.type_reference)
    })
}

fn literal_element_type(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    if !type_reference.is_valid() {
        return None;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => literal_element_type(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            literal_element_type(program, *base_type)
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => Some(*element_type),
        _ => None,
    }
}

fn compatibility_name(
    program: &TypedTrees,
    actual: &IndexedInstance,
    expected: &IndexedInstance,
    point: ProgramPoint,
) -> String {
    format!(
        "index-equality:{}:{}:{}=={}",
        program.symbols.display_path(actual.family, "::"),
        point_label(program, point),
        actual.label,
        expected.label,
    )
}

#[derive(Debug, Clone, Copy)]
struct ExpressionSubstitution<'program> {
    symbol: SymbolHandle,
    name: &'program str,
    value: ExpressionHandle,
}

fn established_index_equalities(
    program: &TypedTrees,
    semantic: &FactPlan,
    flow: &FlowFacts,
    contexts: HandleSpan<FlowSemanticContextRef>,
    actual: &[TypeReferenceHandle],
    expected: &[TypeReferenceHandle],
) -> Option<Vec<FactHandle>> {
    if actual.len() != expected.len() {
        return None;
    }
    let differing = actual
        .iter()
        .zip(expected)
        .filter(|(left, right)| !index_arguments_structurally_equal(program, **left, **right))
        .collect::<Vec<_>>();
    if differing.is_empty() {
        return None;
    }

    let mut evidence = Vec::new();
    for (actual, expected) in differing {
        let fact = established_index_equality_for_argument(
            program, semantic, flow, contexts, *actual, *expected,
        )?;
        if !evidence.contains(&fact) {
            evidence.push(fact);
        }
    }
    Some(evidence)
}

fn established_index_equality_for_argument(
    program: &TypedTrees,
    semantic: &FactPlan,
    flow: &FlowFacts,
    contexts: HandleSpan<FlowSemanticContextRef>,
    actual: TypeReferenceHandle,
    expected: TypeReferenceHandle,
) -> Option<FactHandle> {
    for context_ref in flow.contexts.semantic_context_refs.span_or_empty(contexts) {
        let context = semantic.contexts.get(context_ref.context);
        for fact_ref in semantic.refs.span_or_empty(context.facts) {
            let fact = semantic.facts.get(fact_ref.fact);
            let expression = match fact.payload {
                FactPayload::BooleanExpression(expression) => expression,
                FactPayload::ContractBooleanExpression { expression, .. } => expression,
                _ => continue,
            };
            let substitutions = fact_substitutions(program, flow, fact.point);
            if expression_proves_index_equality(
                program,
                expression,
                &substitutions,
                actual,
                expected,
            ) {
                return Some(fact_ref.fact);
            }
        }
    }
    None
}

fn expression_proves_index_equality(
    program: &TypedTrees,
    expression: ExpressionHandle,
    substitutions: &[ExpressionSubstitution<'_>],
    actual: TypeReferenceHandle,
    expected: TypeReferenceHandle,
) -> bool {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return false;
    };
    if binary.operator == BinaryOperator::And {
        // Conjunction introduction establishes each authored conjunct. This is
        // still an exact local lookup: do not derive transitive equalities or
        // search for any theorem outside the active fact context.
        return expression_proves_index_equality(
            program,
            binary.left,
            substitutions,
            actual,
            expected,
        ) || expression_proves_index_equality(
            program,
            binary.right,
            substitutions,
            actual,
            expected,
        );
    }
    if binary.operator != BinaryOperator::Equal {
        return false;
    }
    let direct =
        substituted_expression_matches_index_argument(program, binary.left, substitutions, actual)
            && substituted_expression_matches_index_argument(
                program,
                binary.right,
                substitutions,
                expected,
            );
    let symmetric = substituted_expression_matches_index_argument(
        program,
        binary.left,
        substitutions,
        expected,
    ) && substituted_expression_matches_index_argument(
        program,
        binary.right,
        substitutions,
        actual,
    );
    direct || symmetric
}

fn fact_substitutions<'program>(
    program: &'program TypedTrees,
    flow: &FlowFacts,
    point: ProgramPoint,
) -> Vec<ExpressionSubstitution<'program>> {
    let ProgramPoint::CallEnsures {
        machine_symbol,
        state_symbol,
        statement_index,
        call_ordinal,
    } = point
    else {
        return Vec::new();
    };
    let Some(call_site) = crate::find_call_site(
        program,
        machine_symbol,
        state_symbol,
        statement_index,
        call_ordinal,
    ) else {
        return Vec::new();
    };
    let target_symbol = match &call_site {
        crate::CallSite::Statement(call) => call.target_symbol,
        crate::CallSite::Expression { call, .. } => call.target_symbol,
        crate::CallSite::TransitionNamed { .. } => call_flow_at_point(flow, point)
            .map_or_else(SymbolHandle::invalid, |call| call.target_symbol),
    };
    let Some(parameters) = crate::call_target_parameters(program, target_symbol) else {
        return Vec::new();
    };
    let arguments = crate::call_site_argument_expressions(program, &call_site);
    parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .zip(arguments)
        .map(|(parameter, value)| ExpressionSubstitution {
            symbol: parameter.symbol,
            name: parameter.name.as_str(),
            value: *value,
        })
        .collect()
}

fn call_flow_at_point(
    flow: &FlowFacts,
    point: ProgramPoint,
) -> Option<&psi_checked_trees::FlowCallFact> {
    let (machine_symbol, state_symbol, statement_index, call_ordinal) = match point {
        ProgramPoint::Call {
            machine_symbol,
            state_symbol,
            statement_index,
            call_ordinal,
        }
        | ProgramPoint::CallRequires {
            machine_symbol,
            state_symbol,
            statement_index,
            call_ordinal,
        }
        | ProgramPoint::CallEnsures {
            machine_symbol,
            state_symbol,
            statement_index,
            call_ordinal,
        } => (machine_symbol, state_symbol, statement_index, call_ordinal),
        _ => return None,
    };
    let state = flow.control.states.iter().find_map(|(_, state)| {
        (state.machine_symbol == machine_symbol && state.state_symbol == state_symbol)
            .then_some(state)
    })?;
    flow.control
        .calls
        .span_or_empty(state.calls)
        .iter()
        .find(|call| call.statement_index == statement_index && call.call_ordinal == call_ordinal)
}

fn index_arguments_structurally_equal(
    program: &TypedTrees,
    left: TypeReferenceHandle,
    right: TypeReferenceHandle,
) -> bool {
    if left == right {
        return true;
    }
    match (
        program.type_reference_table.type_reference(left),
        program.type_reference_table.type_reference(right),
    ) {
        (TypeReferenceNode::ConstExpression(left), TypeReferenceNode::ConstExpression(right)) => {
            program
                .expression_table
                .expressions_structurally_equal(*left, *right)
        }
        (
            TypeReferenceNode::Named {
                symbol: left_symbol,
                name: left_name,
            },
            TypeReferenceNode::Named {
                symbol: right_symbol,
                name: right_name,
            },
        ) => left_symbol == right_symbol && left_name.as_str() == right_name.as_str(),
        _ => false,
    }
}

fn substituted_expression_matches_index_argument(
    program: &TypedTrees,
    expression: ExpressionHandle,
    substitutions: &[ExpressionSubstitution<'_>],
    argument: TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(argument) {
        TypeReferenceNode::ConstExpression(expected) => {
            substituted_expressions_equal(program, expression, substitutions, *expected)
        }
        TypeReferenceNode::Named { symbol, name } => {
            let expression =
                substituted_root(program, expression, substitutions).unwrap_or(expression);
            match program.expression_table.expression(expression) {
                ExpressionNode::Name(path) => {
                    if symbol.is_valid() && (path.symbol.is_valid() || path.head_symbol.is_valid())
                    {
                        path.symbol == *symbol || path.head_symbol == *symbol
                    } else {
                        matches!(
                            program.expression_table.name_path_members(path.members),
                            [only] if only.as_str() == name.as_str()
                        )
                    }
                }
                ExpressionNode::Integer(literal) => named_integer_value(name.as_str())
                    .is_some_and(|expected| literal.value_bignum() == Some(expected)),
                ExpressionNode::Boolean(value) => match name.as_str() {
                    "true" => *value,
                    "false" => !*value,
                    _ => false,
                },
                _ => false,
            }
        }
        _ => false,
    }
}

fn substituted_root(
    program: &TypedTrees,
    expression: ExpressionHandle,
    substitutions: &[ExpressionSubstitution<'_>],
) -> Option<ExpressionHandle> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    let members = program.expression_table.name_path_members(path.members);
    let [only] = members else {
        return None;
    };
    substitutions
        .iter()
        .find(|substitution| {
            if substitution.symbol.is_valid()
                && (path.symbol.is_valid() || path.head_symbol.is_valid())
            {
                path.symbol == substitution.symbol || path.head_symbol == substitution.symbol
            } else {
                only.as_str() == substitution.name
            }
        })
        .map(|substitution| substitution.value)
}

fn substituted_expressions_equal(
    program: &TypedTrees,
    authored: ExpressionHandle,
    substitutions: &[ExpressionSubstitution<'_>],
    local: ExpressionHandle,
) -> bool {
    if authored == local {
        return true;
    }
    if !authored.is_valid() || !local.is_valid() {
        return false;
    }
    if let Some(substituted) = substituted_root(program, authored, substitutions) {
        return substituted_expressions_equal(program, substituted, &[], local);
    }
    if substitutions.is_empty()
        && program
            .expression_table
            .expressions_structurally_equal(authored, local)
    {
        return true;
    }
    match (
        program.expression_table.expression(authored),
        program.expression_table.expression(local),
    ) {
        (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => left == right,
        (ExpressionNode::Integer(left), ExpressionNode::Name(right))
        | (ExpressionNode::Name(right), ExpressionNode::Integer(left)) => {
            expression_name_atom(program, right)
                .and_then(named_integer_value)
                .is_some_and(|right| left.value_bignum() == Some(right))
        }
        (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) => left == right,
        (ExpressionNode::Boolean(left), ExpressionNode::Name(right))
        | (ExpressionNode::Name(right), ExpressionNode::Boolean(left)) => {
            matches!(expression_name_atom(program, right), Some("true") if *left)
                || matches!(expression_name_atom(program, right), Some("false") if !*left)
        }
        (ExpressionNode::String(left), ExpressionNode::String(right)) => left == right,
        (ExpressionNode::Float(left), ExpressionNode::Float(right)) => left == right,
        (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
            left.head_symbol == right.head_symbol
                && left.symbol == right.symbol
                && program
                    .expression_table
                    .name_path_members(left.members)
                    .iter()
                    .map(|member| member.as_str())
                    .eq(program
                        .expression_table
                        .name_path_members(right.members)
                        .iter()
                        .map(|member| member.as_str()))
        }
        (ExpressionNode::Borrow(left), ExpressionNode::Borrow(right)) => {
            left.access == right.access
                && substituted_expressions_equal(program, left.target, substitutions, right.target)
        }
        (ExpressionNode::Unary(left), ExpressionNode::Unary(right)) => {
            left.operator == right.operator
                && substituted_expressions_equal(
                    program,
                    left.operand,
                    substitutions,
                    right.operand,
                )
        }
        (ExpressionNode::Binary(left), ExpressionNode::Binary(right)) => {
            left.operator == right.operator
                && substituted_expressions_equal(program, left.left, substitutions, right.left)
                && substituted_expressions_equal(program, left.right, substitutions, right.right)
        }
        (ExpressionNode::Indexed(left), ExpressionNode::Indexed(right)) => {
            substituted_expressions_equal(program, left.collection, substitutions, right.collection)
                && substituted_expressions_equal(program, left.index, substitutions, right.index)
        }
        (ExpressionNode::Member(left), ExpressionNode::Member(right)) => {
            left.member_symbol == right.member_symbol
                && left.member.as_str() == right.member.as_str()
                && substituted_expressions_equal(
                    program,
                    left.receiver,
                    substitutions,
                    right.receiver,
                )
        }
        (ExpressionNode::Call(left), ExpressionNode::Call(right)) => {
            let left_arguments = program.expression_table.expression_handles(left.arguments);
            let right_arguments = program.expression_table.expression_handles(right.arguments);
            left.target_symbol == right.target_symbol
                && left.target.as_str() == right.target.as_str()
                && substituted_expressions_equal(
                    program,
                    left.receiver,
                    substitutions,
                    right.receiver,
                )
                && left_arguments.len() == right_arguments.len()
                && left_arguments
                    .iter()
                    .zip(right_arguments)
                    .all(|(left, right)| {
                        substituted_expressions_equal(program, *left, substitutions, *right)
                    })
        }
        (ExpressionNode::ArrayLiteral(left), ExpressionNode::ArrayLiteral(right)) => {
            let left = program.expression_table.expression_handles(*left);
            let right = program.expression_table.expression_handles(*right);
            left.len() == right.len()
                && left.iter().zip(right).all(|(left, right)| {
                    substituted_expressions_equal(program, *left, substitutions, *right)
                })
        }
        (ExpressionNode::StructLiteral(left), ExpressionNode::StructLiteral(right)) => {
            let left_fields = program.expression_table.struct_fields(left.fields);
            let right_fields = program.expression_table.struct_fields(right.fields);
            left.type_name.as_str() == right.type_name.as_str()
                && left.case_name.as_ref().map(|name| name.as_str())
                    == right.case_name.as_ref().map(|name| name.as_str())
                && left_fields.len() == right_fields.len()
                && left_fields.iter().zip(right_fields).all(|(left, right)| {
                    left.name.as_str() == right.name.as_str()
                        && substituted_expressions_equal(
                            program,
                            left.value,
                            substitutions,
                            right.value,
                        )
                })
        }
        _ => false,
    }
}

fn expression_name_atom<'program>(
    program: &'program TypedTrees,
    path: &psi_typed_trees::expression::TableNamePath,
) -> Option<&'program str> {
    let [only] = program.expression_table.name_path_members(path.members) else {
        return None;
    };
    Some(only.as_str())
}

fn expression_indexed_instances(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Vec<IndexedInstance> {
    let mut instances = Vec::new();
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            return expression_indexed_instances(
                program,
                operators,
                machine,
                state,
                statement_index,
                inner.target,
            );
        }
        ExpressionNode::Atomic(atomic) => {
            return expression_indexed_instances(
                program,
                operators,
                machine,
                state,
                statement_index,
                atomic.result,
            );
        }
        ExpressionNode::Cast(cast) => {
            collect_type_indexed_instances(
                program,
                cast.target_type,
                &mut instances,
                &mut Vec::new(),
            );
            if cast.semantic_domain_symbol.is_valid() && !cast.semantic_domain_arguments.is_empty()
            {
                instances.push(IndexedInstance {
                    family: cast.semantic_domain_symbol,
                    semantic_id: cast.semantic_domain_id,
                    arguments: program
                        .type_reference_table
                        .type_reference_handles(cast.semantic_domain_arguments)
                        .to_vec(),
                    label: qualification_label(program, cast),
                });
            }
            return instances;
        }
        ExpressionNode::Call(call) => {
            if let Some(return_type) = call_return_type(program, call.target_symbol) {
                collect_type_indexed_instances(
                    program,
                    return_type,
                    &mut instances,
                    &mut Vec::new(),
                );
            } else if let Some(operator_use) = operators
                .named_uses()
                .find(|operator_use| operator_use.expression == expression)
                && let Some(operator) = program
                    .operators()
                    .iter()
                    .find(|operator| operator.symbol == operator_use.selected_operator_symbol)
            {
                collect_type_indexed_instances(
                    program,
                    operator.return_type,
                    &mut instances,
                    &mut Vec::new(),
                );
            }
            return instances;
        }
        ExpressionNode::Binary(_) | ExpressionNode::Unary(_) | ExpressionNode::Indexed(_) => {
            if let Some(operator_use) = operators.expression_use(expression)
                && let Some(operator) = program
                    .operators()
                    .iter()
                    .find(|operator| operator.symbol == operator_use.selected_operator_symbol)
            {
                collect_type_indexed_instances(
                    program,
                    operator.return_type,
                    &mut instances,
                    &mut Vec::new(),
                );
                return instances;
            }
        }
        _ => {}
    }
    if let Some(type_reference) = crate::flow::expression_type_reference_in_state(
        program,
        state.symbol,
        statement_index,
        expression,
    ) {
        collect_type_indexed_instances(program, type_reference, &mut instances, &mut Vec::new());
    } else if let Some(type_reference) =
        psi_validation::declared_place_type_raw(program, machine, Some(state), expression)
    {
        collect_type_indexed_instances(program, type_reference, &mut instances, &mut Vec::new());
    }
    instances
}

fn call_return_type(
    program: &TypedTrees,
    state_symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    if let Some(state) = crate::find_state(program, state_symbol) {
        return Some(state.return_type);
    }
    if let Some((_, signature)) = program.machine_parameter_signature(state_symbol) {
        return Some(signature.return_type);
    }
    program.traits().iter().find_map(|trait_definition| {
        program
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|signature| signature.symbol == state_symbol)
            .map(|signature| signature.return_type)
    })
}

fn collect_type_indexed_instances(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    instances: &mut Vec<IndexedInstance>,
    visited: &mut Vec<TypeReferenceHandle>,
) {
    if !type_reference.is_valid() || visited.contains(&type_reference) {
        return;
    }
    visited.push(type_reference);
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            collect_type_indexed_instances(program, *referee, instances, visited)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            collect_type_indexed_instances(program, *base_type, instances, visited);
            for constraint in program.type_reference_table.constraints(*constraints) {
                let TypeConstraintNode::Domain(domain) = constraint else {
                    continue;
                };
                if domain.symbol.is_valid() && !domain.arguments.is_empty() {
                    instances.push(instance_from_constraint(program, domain));
                }
            }
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Unit => {}
    }
}

fn instance_from_constraint(program: &TypedTrees, domain: &DomainConstraint) -> IndexedInstance {
    IndexedInstance {
        family: domain.symbol,
        semantic_id: domain.semantic_id,
        arguments: domain.arguments.clone(),
        label: domain_label(program, domain.name.as_str(), &domain.arguments),
    }
}

fn domain_label(program: &TypedTrees, name: &str, arguments: &[TypeReferenceHandle]) -> String {
    let arguments = arguments
        .iter()
        .map(|argument| index_argument_label(program, *argument))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{name}<{arguments}>")
}

fn qualification_label(
    program: &TypedTrees,
    cast: &psi_typed_trees::expression::TableCastExpression,
) -> String {
    let name = program
        .expression_table
        .name_path_members(cast.semantic_domain)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    domain_label(
        program,
        &name,
        program
            .type_reference_table
            .type_reference_handles(cast.semantic_domain_arguments),
    )
}

fn index_argument_label(program: &TypedTrees, argument: TypeReferenceHandle) -> String {
    match program.type_reference_table.type_reference(argument) {
        TypeReferenceNode::Named { name, .. } => {
            psi_language_semantics::const_value::CanonicalConstValue::from_atom(name.as_str())
                .map_or_else(|| name.to_string(), |value| value.display)
        }
        TypeReferenceNode::ConstExpression(expression) => {
            program.expression_table.display_name(*expression)
        }
        _ => program.display_type_reference(argument),
    }
}

fn is_closed_index_argument(program: &TypedTrees, argument: TypeReferenceHandle) -> bool {
    match program.type_reference_table.type_reference(argument) {
        TypeReferenceNode::Named { name, .. } => {
            psi_language_semantics::const_value::CanonicalConstValue::from_atom(name.as_str())
                .is_some()
                || name.as_str().parse::<i128>().is_ok()
        }
        _ => false,
    }
}

fn named_integer_value(name: &str) -> Option<psi_numerics::bignum::BigInt> {
    let display = psi_language_semantics::const_value::CanonicalConstValue::from_atom(name)
        .map_or_else(|| name.to_owned(), |value| value.display);
    let (negative, unsigned) = display
        .strip_prefix('-')
        .map_or((false, display.as_str()), |unsigned| (true, unsigned));
    let (base, digits) = if let Some(digits) = unsigned.strip_prefix("0b") {
        (2, digits)
    } else if let Some(digits) = unsigned.strip_prefix("0o") {
        (8, digits)
    } else if let Some(digits) = unsigned.strip_prefix("0x") {
        (16, digits)
    } else {
        (10, unsigned)
    };
    let digits = if negative {
        format!("-{digits}")
    } else {
        digits.to_owned()
    };
    psi_numerics::bignum::BigInt::from_str_radix(&digits, base)
}

fn selected_operation_count(
    program: &TypedTrees,
    arguments: impl Iterator<Item = TypeReferenceHandle>,
) -> usize {
    let mut expressions = Vec::new();
    for argument in arguments {
        let TypeReferenceNode::ConstExpression(expression) =
            program.type_reference_table.type_reference(argument)
        else {
            continue;
        };
        if !expressions.contains(expression) {
            expressions.push(*expression);
        }
    }
    program
        .open_index_normalizations
        .iter()
        .filter(|normalization| expressions.contains(&normalization.expression))
        .map(|normalization| normalization.operations.len())
        .sum()
}

fn point_label(program: &TypedTrees, point: ProgramPoint) -> String {
    let symbol = |symbol| {
        let path = program.symbols.display_path(symbol, "::");
        if path.is_empty() {
            format!("symbol-{}", symbol.arena_index())
        } else {
            path
        }
    };
    match point {
        ProgramPoint::Global => "global".to_owned(),
        ProgramPoint::Definition { symbol: definition } => symbol(definition),
        ProgramPoint::Machine { machine_symbol } => symbol(machine_symbol),
        ProgramPoint::State { state_symbol, .. } => symbol(state_symbol),
        ProgramPoint::Call {
            state_symbol,
            statement_index,
            call_ordinal,
            ..
        } => format!(
            "{}:call-{statement_index}-{call_ordinal}",
            symbol(state_symbol)
        ),
        ProgramPoint::CallRequires {
            state_symbol,
            statement_index,
            call_ordinal,
            ..
        } => format!(
            "{}:call-requires-{statement_index}-{call_ordinal}",
            symbol(state_symbol)
        ),
        ProgramPoint::CallEnsures {
            state_symbol,
            statement_index,
            call_ordinal,
            ..
        } => format!(
            "{}:call-ensures-{statement_index}-{call_ordinal}",
            symbol(state_symbol)
        ),
        ProgramPoint::Exit {
            state_symbol,
            statement_index,
            ..
        } => format!("{}:exit-{statement_index}", symbol(state_symbol)),
        ProgramPoint::Statement {
            state_symbol,
            statement_index,
            ..
        } => format!("{}:statement-{statement_index}", symbol(state_symbol)),
    }
}
