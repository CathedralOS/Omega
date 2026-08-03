use psi_arena::Arena;
use psi_symbols::{SymbolHandle, SymbolKind, SymbolTable};

use crate::symbols::lookup::{call_target_for_attached_data, child_symbol_by_kinds};

fn type_reference_symbol(
    child_type_references: &Arena<psi_symbol_resolved_trees::types::TypeReference>,
    type_reference: &psi_symbol_resolved_trees::types::TypeReference,
) -> SymbolHandle {
    match type_reference {
        psi_symbol_resolved_trees::types::TypeReference::Reference(reference) => {
            type_reference_symbol(
                child_type_references,
                child_type_references.get(reference.referee),
            )
        }
        psi_symbol_resolved_trees::types::TypeReference::Constrained(constrained) => {
            type_reference_symbol(
                child_type_references,
                child_type_references.get(constrained.base_type),
            )
        }
        psi_symbol_resolved_trees::types::TypeReference::FixedArray(fixed_array) => {
            type_reference_symbol(
                child_type_references,
                child_type_references.get(fixed_array.element_type),
            )
        }
        psi_symbol_resolved_trees::types::TypeReference::Slice(slice) => type_reference_symbol(
            child_type_references,
            child_type_references.get(slice.element_type),
        ),
        psi_symbol_resolved_trees::types::TypeReference::Generic(generic) => generic.base_symbol,
        psi_symbol_resolved_trees::types::TypeReference::ConstExpression(_) => {
            SymbolHandle::invalid()
        }
        psi_symbol_resolved_trees::types::TypeReference::DynamicTrait { symbol, .. } => *symbol,
        psi_symbol_resolved_trees::types::TypeReference::Named { symbol, .. } => *symbol,
        psi_symbol_resolved_trees::types::TypeReference::SelfType { symbol } => *symbol,
        psi_symbol_resolved_trees::types::TypeReference::Unit => SymbolHandle::invalid(),
    }
}

pub(in crate::symbols) fn call_target_for_type_reference(
    symbols: &SymbolTable,
    child_type_references: &Arena<psi_symbol_resolved_trees::types::TypeReference>,
    type_reference: &psi_symbol_resolved_trees::types::TypeReference,
    target_name: &str,
) -> SymbolHandle {
    let type_symbol = type_reference_symbol(child_type_references, type_reference);
    let direct_child =
        child_symbol_by_kinds(symbols, type_symbol, &[SymbolKind::State], target_name);
    if direct_child.is_valid() {
        return direct_child;
    }

    if type_symbol.is_valid() && matches!(symbols.get(type_symbol).kind, SymbolKind::Data) {
        return call_target_for_attached_data(symbols, symbols.name(type_symbol), target_name);
    }

    SymbolHandle::invalid()
}
