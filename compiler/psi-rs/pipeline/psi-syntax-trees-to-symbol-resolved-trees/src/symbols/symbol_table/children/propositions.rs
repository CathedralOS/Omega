use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_symbols::{SymbolHandle, SymbolKind, SymbolTableBuilder};

use super::super::names::symbol_seed;

pub(in crate::symbols) fn insert_proposition_symbol_children(
    builder: &mut SymbolTableBuilder,
    program: &SymbolResolvedTrees,
    proposition_symbol: SymbolHandle,
    proposition: &psi_symbol_resolved_trees::proposition::PropositionDefinition,
    has_sources: bool,
) {
    builder.insert_children(
        proposition_symbol,
        program
            .tables
            .declarations
            .proposition_binders
            .span_or_empty(proposition.binders)
            .iter()
            .map(|binder| {
                let kind = match binder.kind {
                    psi_symbol_resolved_trees::proposition::PropositionBinderKind::Machine => {
                        SymbolKind::PropositionMachineParameter
                    }
                    _ => SymbolKind::TypeParameter,
                };
                symbol_seed(kind, &binder.name, has_sources)
            })
            .chain(
                program
                    .state_parameters(proposition.parameters)
                    .iter()
                    .map(|parameter| {
                        symbol_seed(SymbolKind::Parameter, &parameter.name, has_sources)
                    }),
            ),
    );
}
