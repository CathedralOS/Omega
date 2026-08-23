use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};

use crate::symbols::symbol_table::names::{operator_symbol_name, symbol_seed};

pub(in crate::symbols::symbol_table) fn insert_domain_symbol_children(
    builder: &mut SymbolTableBuilder,
    program: &SymbolResolvedTrees,
    domain_symbol: SymbolHandle,
    domain: &psi_symbol_resolved_trees::domain::DomainDefinition,
    has_sources: bool,
) {
    let operator_names = program
        .operator_definitions(domain.operators)
        .iter()
        .map(|operator| operator_symbol_name(program, operator))
        .collect::<Vec<_>>();
    let domain_children = builder.insert_children(
        domain_symbol,
        program
            .data_type_parameters(domain.type_parameters)
            .iter()
            .map(|parameter| symbol_seed(SymbolKind::TypeParameter, &parameter.name, has_sources))
            .chain(
                operator_names
                    .iter()
                    .map(|name| (SymbolKind::Operator, SymbolNameRef::Borrowed(name.as_str()))),
            ),
    );

    for (operator_symbol, operator) in SymbolTableBuilder::child_handles(domain_children)
        .skip(domain.type_parameters.len())
        .zip(program.operator_definitions(domain.operators).iter())
    {
        insert_operator_symbol_children(builder, program, operator_symbol, operator, has_sources);
    }
}

pub(in crate::symbols::symbol_table) fn insert_operator_symbol_children(
    builder: &mut SymbolTableBuilder,
    program: &SymbolResolvedTrees,
    operator_symbol: SymbolHandle,
    operator: &psi_symbol_resolved_trees::operator::OperatorDefinition,
    has_sources: bool,
) {
    builder.insert_children(
        operator_symbol,
        program
            .data_type_parameters(operator.type_parameters)
            .iter()
            .map(|parameter| symbol_seed(SymbolKind::TypeParameter, &parameter.name, has_sources))
            .chain(
                program
                    .state_parameters(operator.parameters)
                    .iter()
                    .map(|parameter| {
                        symbol_seed(SymbolKind::Parameter, &parameter.name, has_sources)
                    }),
            ),
    );
}
