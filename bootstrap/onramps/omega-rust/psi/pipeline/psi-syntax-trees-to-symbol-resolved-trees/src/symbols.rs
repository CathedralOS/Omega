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

pub(crate) fn assign_symbols(program: &mut SymbolResolvedTrees, sources: Option<Arc<SourceMap>>) {
    let symbols = build_symbol_table(program, sources);
    assign_top_level_symbols(program, &symbols);
    assign_type_reference_symbols(program, &symbols);
    propositions::assign_proposition_expression_symbols(program, &symbols);
    assign_contract_reference_symbols(program, &symbols);
    assign_domain_fact_symbols(program, &symbols);
    assign_statement_reference_symbols(program, &symbols);
    program.symbols = symbols;
}
