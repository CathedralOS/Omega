//! Relabel authored uses only from the exact expected-type context available here.

mod assignments;
mod calls;
mod constructors;
mod expressions;
mod patterns;

pub(crate) use expressions::{statement_call_inputs, statement_type_inputs};
pub(super) use expressions::{visit_expression_type_inputs, visit_static_argument_type_inputs};

pub(super) use assignments::{
    relabel_closed_data_uses_in_constants, relabel_closed_data_uses_in_exact_assignments,
};
pub(super) use calls::relabel_closed_data_uses_in_exact_calls_and_returns;
pub(super) use constructors::{
    closed_constructor_carrier, constructor_carrier_span, constructor_path_name, expected_instance,
    relabel_data_literal_for_expected_type, selected_case_value, selected_constructor,
};
pub(super) use expressions::{
    ConstructorFrontier, collect_expression_handles, collect_statement_expression_handles,
    concrete_machine_declarations, concrete_machine_expression_handles,
    selected_machine_expression_handles, selected_machine_state_handles,
};
pub(super) use patterns::{
    closed_sum_path, named_type_name, relabel_closed_sum_memberships_from_local_types,
    relabel_unique_closed_sum_paths,
};

/// Concrete machine states and their declared parameter/local types — the
/// shared prelude every local-typed relabel pass walks before its own scan.
pub(super) fn concrete_state_local_types(
    syntax: &super::SyntaxTrees,
    declarations: &[syntax_trees::item::ItemHandle],
) -> Vec<(
    syntax_trees::item::StateNode,
    super::HashMap<String, super::TypeReferenceHandle>,
    Vec<syntax_trees::statement::StatementHandle>,
)> {
    use super::StatementNode;
    let concrete_states = selected_machine_state_handles(syntax, declarations);

    let mut states = Vec::with_capacity(concrete_states.len());
    for state_handle in concrete_states {
        let state = syntax.tables.items.state(state_handle).clone();
        let mut local_types = super::HashMap::<String, super::TypeReferenceHandle>::new();
        for parameter in syntax.tables.items.state_parameters(state.parameters) {
            let parameter = syntax.tables.items.state_parameter(*parameter);
            local_types.insert(
                parameter.name.as_str(syntax).to_owned(),
                parameter.type_reference,
            );
        }
        let statements = syntax.tables.items.statements(state.statements).to_vec();
        for statement in &statements {
            if let StatementNode::LocalData(local) = syntax.tables.statements.statement(*statement)
            {
                local_types.insert(local.name.as_str(syntax).to_owned(), local.type_reference);
            }
        }
        states.push((state, local_types, statements));
    }
    states
}
