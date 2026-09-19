use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{SymbolHandle, SymbolKind, SymbolTableAppender};

use super::super::names::symbol_seed;

/// Children of a mathematical `let`/`boundary let`: its generic binders in
/// authored order, then its ordinary telescope parameters. Universe-shaped
/// binders (`u: core::Level`) register as `TypeParameter` like every other
/// non-machine binder — their carrier's resolved declaration is what makes
/// them levels, a classification the elaboration leg owns.
pub(in crate::symbols) fn insert_mathematical_symbol_children(
    builder: &mut impl SymbolTableAppender,
    program: &SymbolResolvedTrees,
    definition_symbol: SymbolHandle,
    definition: &symbol_resolved_trees::mathematical::MathematicalDefinition,
    has_sources: bool,
) {
    builder.insert_children(
        definition_symbol,
        program
            .tables
            .declarations
            .data_type_parameters
            .span_or_empty(definition.binders)
            .iter()
            .map(|binder| {
                let kind = match binder.kind {
                    symbol_resolved_trees::data::TypeParameterKind::Machine { .. } => {
                        SymbolKind::MachineParameter
                    }
                    _ => SymbolKind::TypeParameter,
                };
                symbol_seed(kind, &binder.name, has_sources)
            })
            .chain(
                program
                    .tables
                    .declarations
                    .mathematical_parameters
                    .span_or_empty(definition.parameters)
                    .iter()
                    .map(|parameter| {
                        symbol_seed(SymbolKind::Parameter, &parameter.name, has_sources)
                    }),
            ),
    );
}
