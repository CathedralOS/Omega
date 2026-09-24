use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{SymbolHandle, SymbolKind, SymbolTable};

use super::super::top_level::next_child_of_kind;

/// Write back the allocated top-level symbol for each mathematical
/// `let`/`boundary let`, then its binder and parameter children in the same
/// order `insert_mathematical_symbol_children` registered them.
pub(super) fn assign_mathematical_symbols(
    program: &mut SymbolResolvedTrees,
    symbols: &SymbolTable,
    root_children: &mut impl Iterator<Item = SymbolHandle>,
) {
    let binders = &mut program.tables.declarations.data_type_parameters;
    let parameters = &mut program.tables.declarations.mathematical_parameters;
    let definitions = &mut program.roots.mathematical_definitions;

    definitions.for_each_mut(|definition| {
        if !definition.symbol.is_valid() {
            definition.symbol =
                next_child_of_kind(root_children, symbols, SymbolKind::MathematicalDefinition);
        }
        let mut children = symbols
            .child_handles(definition.symbol)
            .into_iter()
            .flatten();
        for binder in binders.span_mut_or_empty(definition.binders) {
            let kind = match binder.kind {
                symbol_resolved_trees::data::TypeParameterKind::Machine { .. } => {
                    SymbolKind::MachineParameter
                }
                _ => SymbolKind::TypeParameter,
            };
            binder.symbol = next_child_of_kind(&mut children, symbols, kind);
        }
        for parameter in parameters.span_mut_or_empty(definition.parameters) {
            parameter.symbol = next_child_of_kind(&mut children, symbols, SymbolKind::Parameter);
        }
    });
}
