use arena::Arena;
use symbols::{SymbolHandle, SymbolKind, SymbolTable};

use crate::symbols::lookup::child_symbol_by_kinds;
use crate::symbols::scope::MachineScope;

fn type_reference_symbol(
    child_type_references: &Arena<symbol_resolved_trees::types::TypeReference>,
    type_reference: &symbol_resolved_trees::types::TypeReference,
) -> SymbolHandle {
    match type_reference {
        symbol_resolved_trees::types::TypeReference::Reference(reference) => type_reference_symbol(
            child_type_references,
            child_type_references.get(reference.referee),
        ),
        symbol_resolved_trees::types::TypeReference::Constrained(constrained) => {
            type_reference_symbol(
                child_type_references,
                child_type_references.get(constrained.base_type),
            )
        }
        symbol_resolved_trees::types::TypeReference::FixedArray(fixed_array) => {
            type_reference_symbol(
                child_type_references,
                child_type_references.get(fixed_array.element_type),
            )
        }
        symbol_resolved_trees::types::TypeReference::Slice(slice) => type_reference_symbol(
            child_type_references,
            child_type_references.get(slice.element_type),
        ),
        symbol_resolved_trees::types::TypeReference::Generic(generic) => generic.base_symbol,
        symbol_resolved_trees::types::TypeReference::ConstExpression(_) => SymbolHandle::invalid(),
        symbol_resolved_trees::types::TypeReference::DynamicTrait { symbol, .. } => *symbol,
        symbol_resolved_trees::types::TypeReference::Named { symbol, .. } => *symbol,
        symbol_resolved_trees::types::TypeReference::SelfType { symbol } => *symbol,
        symbol_resolved_trees::types::TypeReference::Unit => SymbolHandle::invalid(),
    }
}

/// Whether `symbol` is the toolchain `core/service.omg` `Service` carrier
/// declaration. The full carrier shape is classified later by typed-trees;
/// resolution needs only the exact source identity to route receiver calls
/// through the carrier's closed requirement.
fn exact_service_carrier_data(symbols: &SymbolTable, symbol: SymbolHandle) -> bool {
    if !symbol.is_valid()
        || symbols.get(symbol).kind != SymbolKind::Data
        || symbols.name(symbol) != "Service"
    {
        return false;
    }
    let Some(span) = symbols.symbol_source_span(symbol) else {
        return false;
    };
    let Some(source) = symbols.source_file(span) else {
        return false;
    };
    source.origin == source::SourceOrigin::Toolchain
        && source
            .path
            .strip_prefix(&source.package_root)
            .ok()
            .is_some_and(|path| path == std::path::Path::new("service.omg"))
}

pub(in crate::symbols) fn call_target_for_type_reference(
    machine: &MachineScope<'_>,
    symbols: &SymbolTable,
    child_type_references: &Arena<symbol_resolved_trees::types::TypeReference>,
    type_reference: &symbol_resolved_trees::types::TypeReference,
    target: &symbol_resolved_trees::name::DiagnosticName,
) -> SymbolHandle {
    // The exact `Service<R>` carrier owns no call surface: a receiver call
    // resolves against the closed boundary requirement `R` it carries, the
    // same target a bare requirement receiver would select.
    if let symbol_resolved_trees::types::TypeReference::Generic(generic) = type_reference
        && generic.arguments.len() == 1
        && exact_service_carrier_data(symbols, generic.base_symbol)
    {
        return call_target_for_type_reference(
            machine,
            symbols,
            child_type_references,
            child_type_references.get(generic.arguments.start()),
            target,
        );
    }
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
