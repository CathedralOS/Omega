use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{SymbolHandle, SymbolKind, SymbolTable};

use super::super::top_level::next_child_of_kind;

pub(super) fn assign_proposition_symbols(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
    root_children: &mut impl Iterator<Item = SymbolHandle>,
) {
    let binders = &mut program.tables.declarations.proposition_binders;
    let parameters = &mut program.tables.declarations.state_parameters;
    let propositions = &mut program.roots.propositions;

    propositions.for_each_mut(|proposition| {
        if !proposition.symbol.is_valid() {
            proposition.symbol =
                next_child_of_kind(root_children, symbols, SymbolKind::Proposition);
        }
        let mut children = symbols
            .child_handles(proposition.symbol)
            .into_iter()
            .flatten();
        for binder in binders.span_mut_or_empty(proposition.binders) {
            let kind = match binder.kind {
                symbol_resolved_trees::proposition::PropositionBinderKind::Machine => {
                    SymbolKind::PropositionMachineParameter
                }
                _ => SymbolKind::TypeParameter,
            };
            binder.symbol = next_child_of_kind(&mut children, symbols, kind);
        }
        for parameter in parameters.span_mut_or_empty(proposition.parameters) {
            parameter.symbol = next_child_of_kind(&mut children, symbols, SymbolKind::Parameter);
        }
    });
}
