//! Alias parameters belong to the source declaration, without runtime children.

use super::insert_machine_parameter_signature_children;
use crate::symbols::symbol_table::names::{parameter_seeds, type_parameter_seeds};
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::data::TypeParameterKind;
use symbol_resolved_trees::type_aliases::TypeAliasDefinition;
use symbols::{SymbolHandle, SymbolTableAppender, SymbolTableBuilder};

pub(in crate::symbols::symbol_table) fn insert_type_alias_symbol_children(
    builder: &mut impl SymbolTableAppender,
    program: &SymbolResolvedTrees,
    alias_symbol: SymbolHandle,
    alias: &TypeAliasDefinition,
    has_sources: bool,
) {
    let parameters = program.data_type_parameters(alias.type_parameters);
    let children = builder.insert_children(
        alias_symbol,
        type_parameter_seeds(parameters, true, has_sources),
    );
    for (parameter, symbol) in parameters
        .iter()
        .zip(SymbolTableBuilder::child_handles(children))
    {
        match &parameter.kind {
            TypeParameterKind::Machine { contract } => {
                if let Some(contract) = contract.structural() {
                    insert_machine_parameter_signature_children(
                        builder,
                        program,
                        symbol,
                        contract,
                        has_sources,
                    );
                }
            }
            TypeParameterKind::Proposition { contract } => {
                builder.insert_children(
                    symbol,
                    parameter_seeds(program.state_parameters(contract.parameters), has_sources),
                );
            }
            _ => {}
        }
    }
}
