//! Project authored type selections without confusing machine-slot ordinals.
//!
//! The producer materializes named type references before freezing. These
//! read-only views retain declaration identity; missing references are not
//! permission to resolve by spelling or manufacture a replacement type.

use symbols::SymbolKind;
use typed_trees::data::TypeParameterKind;
use typed_trees::expression::StaticMachineArgument;
use typed_trees::types::TypeReferenceHandle;

pub(super) fn selected_types<'arguments>(
    program: &'arguments dyn crate::CheckingInput,
    arguments: &'arguments [StaticMachineArgument],
) -> impl Iterator<Item = TypeReferenceHandle> + 'arguments {
    arguments.iter().filter_map(|argument| {
        if argument.type_reference.is_valid() {
            return Some(argument.type_reference);
        }
        let is_type = argument.symbol.is_valid()
            && match program.symbols.get(argument.symbol).kind {
                SymbolKind::BuiltinType | SymbolKind::Data => true,
                SymbolKind::TypeParameter => program
                    .data_type_parameter_by_symbol(argument.symbol)
                    .is_some_and(|parameter| matches!(parameter.kind, TypeParameterKind::Type)),
                _ => false,
            };
        if is_type {
            return program
                .type_reference_table
                .find_named_type_reference(argument.symbol);
        }
        None
    })
}
