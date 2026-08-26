use std::sync::Arc;

use psi_source::SourceMap;
use psi_symbol_resolved_trees::SymbolResolvedTrees;

mod symbol_table;

use symbol_table::build_symbol_table;
use type_references::assign_type_reference_symbols;

mod contracts;
mod domain_facts;
mod expression_paths;
mod expressions;
mod lookup;
mod propositions;
mod scope;
mod scoped_paths;
mod statements;
mod targets;
mod top_level;
mod type_references;

use contracts::assign_contract_reference_symbols;
use domain_facts::assign_domain_fact_symbols;
use statements::assign_statement_reference_symbols;
use top_level::assign_top_level_symbols;

pub(crate) fn assign_symbols(
    program: &mut SymbolResolvedTrees,
    sources: Option<Arc<SourceMap>>,
    const_declarations: &[crate::lowerer::PendingConstDeclaration],
) {
    let mut symbols = build_symbol_table(program, sources, const_declarations);
    assign_top_level_symbols(program, &symbols);
    assign_type_reference_symbols(program, &symbols);
    bind_generated_data_origins(program, &mut symbols);
    propositions::assign_proposition_expression_symbols(program, &symbols);
    assign_contract_reference_symbols(program, &symbols);
    assign_domain_fact_symbols(program, &symbols);
    assign_statement_reference_symbols(program, &symbols);
    program.symbols = symbols;
}

fn bind_generated_data_origins(
    program: &SymbolResolvedTrees,
    symbols: &mut psi_symbols::SymbolTable,
) {
    for definition in &program.data_definitions {
        let Some(psi_symbol_resolved_trees::types::TypeReference::Generic(origin)) =
            definition.generic_instance.as_ref()
        else {
            continue;
        };
        if origin.base_symbol.is_valid() {
            symbols.bind_generated_symbol_origin(definition.symbol, origin.base_symbol);
        }
    }
}
