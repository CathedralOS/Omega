use omega_core::symbols::{SymbolHandle, SymbolKind, SymbolTable};

use crate::symbols::lookup::{
    call_target_for_attached_data, child_symbol_by_kinds, top_level_symbol_by_kinds,
};
use crate::symbols::scope::MachineScope;
use crate::symbols::type_references::call_target_for_type_reference;

pub(in crate::symbols) fn resolve_call_target_symbol(
    machine: &MachineScope<'_>,
    parameters: &[omega_symbol_resolved_trees::signature::StateParameter],
    has_receiver: bool,
    receiver_symbol: SymbolHandle,
    target: &omega_symbol_resolved_trees::name::DiagnosticName,
    child_type_references: &omega_core::arena::Arena<
        omega_symbol_resolved_trees::types::TypeReference,
    >,
    symbols: &SymbolTable,
) -> SymbolHandle {
    if has_receiver && receiver_symbol.is_valid() {
        if let Some(contained) = machine
            .contains
            .iter()
            .find(|contained| contained.symbol == receiver_symbol)
        {
            return child_symbol_by_kinds(
                symbols,
                contained.type_symbol,
                &[SymbolKind::State],
                target.as_str(),
            );
        }

        if let Some(field_type_reference) = machine.field_type_reference(symbols, receiver_symbol) {
            let symbol = call_target_for_type_reference(
                symbols,
                child_type_references,
                field_type_reference,
                target.as_str(),
            );
            return symbol;
        }

        let receiver_kind = symbols.get(receiver_symbol).kind;
        if let Some(parameter) = parameters
            .iter()
            .find(|parameter| parameter.symbol == receiver_symbol)
        {
            let direct = call_target_for_type_reference(
                symbols,
                child_type_references,
                &parameter.type_reference,
                target.as_str(),
            );
            if direct.is_valid() {
                return direct;
            }
        }
        if matches!(receiver_kind, SymbolKind::BuiltinType) {
            return child_symbol_by_kinds(
                symbols,
                receiver_symbol,
                &[SymbolKind::BuiltinFunction],
                target.as_str(),
            );
        }
        if matches!(
            receiver_kind,
            SymbolKind::Machine | SymbolKind::Platform | SymbolKind::Trait
        ) {
            if receiver_symbol == machine.symbol
                && let Some(attached_data) = machine.attached_data
            {
                let target_symbol =
                    call_target_for_attached_data(symbols, attached_data.as_str(), target.as_str());
                if target_symbol.is_valid() {
                    return target_symbol;
                }
            }

            return child_symbol_by_kinds(
                symbols,
                receiver_symbol,
                &[SymbolKind::State],
                target.as_str(),
            );
        }
    }

    let machine_state = child_symbol_by_kinds(
        symbols,
        machine.symbol,
        &[SymbolKind::State],
        target.as_str(),
    );
    if machine_state.is_valid() {
        return machine_state;
    }

    top_level_symbol_by_kinds(symbols, &[SymbolKind::BuiltinFunction], target.as_str())
}
