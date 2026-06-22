use omega_core::arena::Arena;
use omega_core::symbols::{SymbolHandle, SymbolKind, SymbolTable};
use omega_symbol_resolved_trees::SymbolResolvedTrees;

use crate::symbols::top_level::next_child_of_kind;
use crate::symbols::type_references::assign_type_reference_symbol_with_locals;

pub(super) fn assign_root_operator_symbols(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
    root_children: &mut impl Iterator<Item = SymbolHandle>,
) {
    let declarations = &mut program.tables.declarations;
    let data_type_parameters = &mut declarations.data_type_parameters;
    let state_parameters = &mut declarations.state_parameters;
    let child_type_references = &mut declarations.child_type_references;
    program.roots.operators.for_each_mut(|operator| {
        assign_operator_symbols(
            symbols,
            root_children,
            data_type_parameters,
            state_parameters,
            child_type_references,
            operator,
        );
    });
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
