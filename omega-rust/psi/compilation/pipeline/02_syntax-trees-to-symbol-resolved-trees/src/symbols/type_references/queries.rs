use arena::Arena;
use symbols::{SymbolHandle, SymbolKind, SymbolTable};

use crate::symbols::lookup::child_symbol_by_kinds;
use crate::symbols::scope::MachineScope;
use symbol_resolved_trees::name::DiagnosticName;
use symbol_resolved_trees::types::TypeReference;

fn type_reference_symbol(
    child_type_references: &Arena<TypeReference>,
    type_reference: &TypeReference,
) -> SymbolHandle {
    match type_reference {
        symbol_resolved_trees::types::TypeReference::AliasUse { expansion, .. } => {
            type_reference_symbol(child_type_references, expansion)
        }
        symbol_resolved_trees::types::TypeReference::Reference(reference) => type_reference_symbol(
            child_type_references,
            child_type_references.get(reference.referee),
        ),
        TypeReference::Constrained(constrained) => type_reference_symbol(
            child_type_references,
            child_type_references.get(constrained.base_type),
        ),
        TypeReference::FixedArray(fixed_array) => type_reference_symbol(
            child_type_references,
            child_type_references.get(fixed_array.element_type),
        ),
        TypeReference::Slice(slice) => type_reference_symbol(
            child_type_references,
            child_type_references.get(slice.element_type),
        ),
        TypeReference::Generic(generic) => generic.base_symbol,
        TypeReference::ConstExpression(_) => SymbolHandle::invalid(),
        TypeReference::DynamicTrait { symbol, .. } => *symbol,
        TypeReference::Named { symbol, .. } => *symbol,
        TypeReference::Unit => SymbolHandle::invalid(),
    }
}

pub(in crate::symbols) fn call_target_for_type_reference(
    machine: &MachineScope<'_>,
    symbols: &SymbolTable,
    child_type_references: &Arena<TypeReference>,
    type_reference: &TypeReference,
    target: &DiagnosticName,
) -> SymbolHandle {
    let type_symbol = type_reference_symbol(child_type_references, type_reference);
    let direct_child =
        child_symbol_by_kinds(symbols, type_symbol, &[SymbolKind::State], target.as_str());
    if direct_child.is_valid() {
        return direct_child;
    }

    if type_symbol.is_valid() && matches!(symbols.get(type_symbol).kind, SymbolKind::Data) {
        return machine.attached_call_target(symbols, type_symbol, target);
    }

    SymbolHandle::invalid()
}
