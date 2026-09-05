//! Expected types from local declarations and assignments.

use super::super::*;

/// Give a bare generic record literal the exact closed instance selected by an
/// explicitly typed local. This is contextual elaboration, not inference: only
/// `let value: Box<i32> = Box { ... }` (and record literals nested beneath that
/// known destination shape) are rewritten. Calls, returns, assignments, generic
/// sums, and literals without an annotated local destination remain untouched.
pub(in crate::generic_data) fn relabel_closed_data_uses_in_annotated_locals(
    syntax: &mut SyntaxTrees,
    synthesized_origins: &HashMap<String, String>,
) {
    let locals = syntax
        .root_items()
        .filter_map(|item| match item {
            Item::Machine(machine) if machine.type_parameters.is_empty() => Some(machine),
            _ => None,
        })
        .flat_map(|machine| syntax.tables.items.state_handles(machine.states))
        .flat_map(|state| {
            let state = syntax.tables.items.state(*state);
            syntax.tables.items.statements(state.statements)
        })
        .filter_map(
            |statement| match syntax.tables.statements.statement(*statement) {
                StatementNode::LocalData(local) if local.initial_value.is_valid() => {
                    Some((local.type_reference, local.initial_value))
                }
                _ => None,
            },
        )
        .collect::<Vec<_>>();

    for (expected_type, expression) in locals {
        relabel_data_literal_for_expected_type(
            syntax,
            expression,
            expected_type,
            synthesized_origins,
        );
    }
}

/// An assignment target is another explicit destination type. Relabel a bare
/// generic literal only when that type is available directly from a local or
/// an attached data field; computed targets remain fail-closed.
pub(in crate::generic_data) fn relabel_closed_data_uses_in_exact_assignments(
    syntax: &mut SyntaxTrees,
    synthesized_origins: &HashMap<String, String>,
) {
    let concrete_states = syntax
        .root_items()
        .filter_map(|item| match item {
            Item::Machine(machine) if machine.type_parameters.is_empty() => Some(machine),
            _ => None,
        })
        .flat_map(|machine| {
            syntax
                .tables
                .items
                .state_handles(machine.states)
                .iter()
                .map(|state| (*state, machine.attached_data.clone()))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    for (state_handle, attached_data) in concrete_states {
        let state = syntax.tables.items.state(state_handle).clone();
        let mut local_types = HashMap::<String, TypeReferenceHandle>::new();
        for parameter in syntax.tables.items.state_parameters(state.parameters) {
            let parameter = syntax.tables.items.state_parameter(*parameter);
            local_types.insert(parameter.name.as_str().to_owned(), parameter.type_reference);
        }
        let statements = syntax.tables.items.statements(state.statements).to_vec();
        for statement in &statements {
            if let StatementNode::LocalData(local) = syntax.tables.statements.statement(*statement)
            {
                local_types.insert(local.name.as_str().to_owned(), local.type_reference);
            }
        }
        let self_field_types = attached_data
            .as_ref()
            .and_then(|attached| {
                syntax.root_items().find_map(|item| match item {
                    Item::Data(definition) if definition.name == *attached => Some(
                        syntax
                            .tables
                            .items
                            .data_members(definition.members)
                            .iter()
                            .filter_map(|member| match member {
                                DataMember::Field(field) => {
                                    Some((field.name.as_str().to_owned(), field.type_reference))
                                }
                                _ => None,
                            })
                            .collect::<HashMap<_, _>>(),
                    ),
                    _ => None,
                })
            })
            .unwrap_or_default();

        let assignments = statements
            .iter()
            .filter_map(
                |statement| match syntax.tables.statements.statement(*statement) {
                    StatementNode::Assignment(assignment) => Some(*assignment),
                    _ => None,
                },
            )
            .collect::<Vec<_>>();
        for assignment in assignments {
            let expected_type = match syntax.expressions.expression(assignment.target) {
                ExpressionNode::Name(path) => {
                    let [name] = syntax.expressions.identifier_path_members(*path) else {
                        continue;
                    };
                    local_types.get(name.as_str()).copied()
                }
                ExpressionNode::Member(member)
                    if matches!(
                        syntax.expressions.expression(member.receiver),
                        ExpressionNode::SelfValue
                    ) =>
                {
                    self_field_types.get(member.member.as_str()).copied()
                }
                _ => None,
            };
            if let Some(expected_type) = expected_type {
                relabel_data_literal_for_expected_type(
                    syntax,
                    assignment.value,
                    expected_type,
                    synthesized_origins,
                );
            }
        }
    }
}
