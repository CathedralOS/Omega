//! Expected types from local declarations and assignments.

use super::super::*;

/// An assignment target is another explicit destination type. Relabel a bare
/// generic literal only when that type is available directly from a local or
/// an attached data field; computed targets remain fail-closed.
pub(in crate::generic_data) fn relabel_closed_data_uses_in_exact_assignments(
    syntax: &mut SyntaxTrees,
    instances: &[Instantiation],
    selection: Option<&constant_selection::ConstantSelection>,
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

        for (index, statement) in statements.iter().enumerate() {
            let frontier = ConstructorFrontier {
                parameters: state.parameters,
                prior_statements: &statements[..index],
            };
            let assignment = match syntax.statements.statement(*statement) {
                StatementNode::LocalData(local) => {
                    relabel_data_literal_for_expected_type(
                        syntax,
                        local.initial_value,
                        local.type_reference,
                        instances,
                        selection,
                        &frontier,
                    );
                    continue;
                }
                StatementNode::Assignment(assignment) => *assignment,
                _ => continue,
            };
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
                    instances,
                    selection,
                    &frontier,
                );
            }
        }
    }
}
