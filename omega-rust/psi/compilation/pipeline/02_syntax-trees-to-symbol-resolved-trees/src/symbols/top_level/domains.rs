use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{SymbolHandle, SymbolKind, SymbolTable};

use crate::symbols::top_level::next_child_of_kind;
use crate::symbols::top_level::operators::assign_operator_symbols;

pub(super) fn assign_domain_symbols(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
    root_children: &mut impl Iterator<Item = SymbolHandle>,
) {
    let type_constraints = &program.tables.types.constraints;
    let roots = &mut program.roots;
    let declarations = &mut program.tables.declarations;
    let operator_definitions = &mut declarations.operator_definitions;
    let data_type_parameters = &mut declarations.data_type_parameters;
    let state_parameters = &mut declarations.state_parameters;
    let child_type_references = &mut declarations.child_type_references;
    roots.domain_definitions.for_each_mut(|domain| {
        if !domain.symbol.is_valid() {
            domain.symbol = next_child_of_kind(root_children, symbols, SymbolKind::Domain);
        }
        let mut domain_children = symbols.child_handles(domain.symbol).into_iter().flatten();
        for parameter in data_type_parameters.span_mut_or_empty(domain.type_parameters) {
            parameter.symbol =
                next_child_of_kind(&mut domain_children, symbols, SymbolKind::TypeParameter);
        }
        for operator in operator_definitions.span_mut_or_empty(domain.operators) {
            assign_operator_symbols(
                symbols,
                &mut domain_children,
                data_type_parameters,
                state_parameters,
                child_type_references,
                type_constraints,
                operator,
            );
        }
    });
}
