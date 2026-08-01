use omega_checked_trees::{
    CheckedOperatorFacts, FlowFacts, IndexCompatibilityDischarge, IndexCompatibilityFact,
    IndexCompatibilityFacts,
};
use omega_core::semantics::SemanticDomainId;
use omega_core::symbols::SymbolHandle;
use omega_facts::ProgramPoint;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::state::State;
use omega_typed_trees::statement::{StatementNode, TransitionTargetNode};
use omega_typed_trees::types::{
    DomainConstraint, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

#[derive(Debug, Clone)]
struct IndexedInstance {
    family: SymbolHandle,
    semantic_id: SemanticDomainId,
    arguments: Vec<TypeReferenceHandle>,
    label: String,
}

pub(super) fn build_index_compatibility_facts(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    flow: &FlowFacts,
) -> IndexCompatibilityFacts {
    let mut conditions = Vec::new();

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

        for call in flow.control.calls.span_or_empty(state_flow.calls) {
            let Some(call_site) = crate::find_call_site(
                program,
                state_flow.machine_symbol,
                state_flow.state_symbol,
                call.statement_index,
                call.call_ordinal,
            ) else {
                continue;
            };
            let Some(parameters) = crate::call_target_parameters(program, call.target_symbol)
            else {
                continue;
            };
            let arguments = crate::call_site_argument_expressions(program, &call_site);
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
                    machine,
                    state,
                    call.statement_index,
                    *argument,
                    parameter.type_reference,
                    point,
                    &mut conditions,
                );
            }
        }

        let statements = program.statement_table.statements(state.statement_nodes);
        for (statement_index, statement) in statements.iter().enumerate() {
            let statement_point = ProgramPoint::Statement {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
                statement_index,
            };
            match statement {
                StatementNode::LocalData(local) => append_expression_compatibilities(
                    program,
                    operators,
                    machine,
                    state,
                    statement_index,
                    local.initial_value,
                    local.type_reference,
                    statement_point,
                    &mut conditions,
                ),
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
                            machine,
                            state,
                            statement_index,
                            assignment.value,
                            target_type,
                            statement_point,
                            &mut conditions,
                        );
                    }
                }
                StatementNode::Expression(expression)
                    if statement_index + 1 == statements.len() =>
                {
                    append_expression_compatibilities(
                        program,
                        operators,
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
                        &mut conditions,
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
                            &mut conditions,
                        );
                    }
                }
                StatementNode::AssemblyFact(_)
                | StatementNode::Call(_)
                | StatementNode::Expression(_) => {}
            }
        }
    }

    IndexCompatibilityFacts { conditions }
}

#[allow(clippy::too_many_arguments)]
fn append_expression_compatibilities(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    value: ExpressionHandle,
    target_type: TypeReferenceHandle,
    point: ProgramPoint,
    conditions: &mut Vec<IndexCompatibilityFact>,
) {
    if !value.is_valid() || !target_type.is_valid() {
        return;
    }
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
                || actual.semantic_id != expected.semantic_id
                || actual.arguments.is_empty()
                || expected.arguments.is_empty()
            {
                continue;
            }
            let discharge = if actual
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
            };
            let name = format!(
                "index-equality:{}:{}:{}=={}",
                program.symbols.display_path(actual.family, "::"),
                point_label(program, point),
                actual.label,
                expected.label,
            );
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
        ExpressionNode::Mutable(inner) => {
            return expression_indexed_instances(
                program,
                operators,
                machine,
                state,
                statement_index,
                *inner,
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
        omega_validation::declared_place_type_raw(program, machine, Some(state), expression)
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
    cast: &omega_typed_trees::expression::TableCastExpression,
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
            omega_core::const_value::CanonicalConstValue::from_atom(name.as_str())
                .map_or_else(|| name.to_string(), |value| value.display)
        }
        _ => program.display_type_reference(argument),
    }
}

fn is_closed_index_argument(program: &TypedTrees, argument: TypeReferenceHandle) -> bool {
    match program.type_reference_table.type_reference(argument) {
        TypeReferenceNode::Named { name, .. } => {
            omega_core::const_value::CanonicalConstValue::from_atom(name.as_str()).is_some()
                || name.as_str().parse::<i128>().is_ok()
        }
        _ => false,
    }
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
