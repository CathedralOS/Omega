use omega_core::arena::Arena;
use omega_core::symbols::{SymbolHandle, SymbolKind, SymbolTable};

use super::type_references::assign_type_reference_symbol_with_locals;

pub(super) fn next_child_of_kind(
    children: &mut impl Iterator<Item = SymbolHandle>,
    symbols: &SymbolTable,
    kind: SymbolKind,
) -> SymbolHandle {
    let Some(child) = children.next() else {
        return SymbolHandle::invalid();
    };

    if symbols.get(child).kind == kind {
        child
    } else {
        SymbolHandle::invalid()
    }
}

pub(super) fn inherited_field_count<'data>(
    data_definitions: impl IntoIterator<Item = &'data omega_symbol_resolved_trees::data::DataDefinition>,
    data_members: &Arena<omega_symbol_resolved_trees::data::DataMember>,
    attached_data: Option<&omega_symbol_resolved_trees::name::DiagnosticName>,
) -> usize {
    let Some(attached_data) = attached_data else {
        return 0;
    };

    data_definitions
        .into_iter()
        .find(|data_definition| data_definition.name == *attached_data)
        .map(|data_definition| {
            data_members
                .span_or_empty(data_definition.members)
                .iter()
                .filter(|member| {
                    matches!(
                        member,
                        omega_symbol_resolved_trees::data::DataMember::Field(_)
                    )
                })
                .count()
        })
        .unwrap_or(0)
}

pub(super) fn assign_operator_symbols(
    symbols: &SymbolTable,
    siblings: &mut impl Iterator<Item = SymbolHandle>,
    data_type_parameters: &mut Arena<omega_symbol_resolved_trees::data::TypeParameter>,
    state_parameters: &mut Arena<omega_symbol_resolved_trees::signature::StateParameter>,
    child_type_references: &mut Arena<omega_symbol_resolved_trees::types::TypeReference>,
    operator: &mut omega_symbol_resolved_trees::operator::OperatorDefinition,
) {
    operator.symbol = next_child_of_kind(siblings, symbols, SymbolKind::Operator);
    let mut operator_children = symbols.child_handles(operator.symbol).into_iter().flatten();

    for type_parameter in data_type_parameters.span_mut_or_empty(operator.type_parameters) {
        type_parameter.symbol =
            next_child_of_kind(&mut operator_children, symbols, SymbolKind::TypeParameter);
    }
    let local_type_parameters = data_type_parameters
        .span_or_empty(operator.type_parameters)
        .to_vec();
    for parameter in state_parameters.span_mut_or_empty(operator.parameters) {
        parameter.symbol =
            next_child_of_kind(&mut operator_children, symbols, SymbolKind::Parameter);
        assign_type_reference_symbol_with_locals(
            symbols,
            child_type_references,
            &local_type_parameters,
            &mut parameter.type_reference,
        );
    }
    if let Some(return_type) = &mut operator.return_type {
        assign_type_reference_symbol_with_locals(
            symbols,
            child_type_references,
            &local_type_parameters,
            return_type,
        );
    }
}
