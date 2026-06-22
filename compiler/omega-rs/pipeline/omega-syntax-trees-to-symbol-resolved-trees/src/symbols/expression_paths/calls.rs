use omega_core::symbols::{SymbolHandle, SymbolTable};

use super::super::scope::MachineScope;
use super::super::targets::resolve_call_target_symbol;
use super::receivers::resolve_expression_table_receiver_symbol;

pub(in crate::symbols) fn resolve_expression_table_call_target_symbol(
    machine: &MachineScope<'_>,
    parameters: &[omega_symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    call: &omega_symbol_resolved_trees::expression::TableCallExpression,
    expression_table: &omega_symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &omega_core::arena::Arena<
        omega_symbol_resolved_trees::types::TypeReference,
    >,
    symbols: &SymbolTable,
) -> SymbolHandle {
    if call.receiver.is_valid() {
        let receiver_symbol = resolve_expression_table_receiver_symbol(
            symbols,
            machine.symbol,
            state_symbol,
            expression_table,
            call.receiver,
        );
        return resolve_call_target_symbol(
            machine,
            parameters,
            true,
            receiver_symbol,
            &call.target,
            child_type_references,
            symbols,
        );
    }

    resolve_call_target_symbol(
        machine,
        parameters,
        false,
        SymbolHandle::invalid(),
        &call.target,
        child_type_references,
        symbols,
    )
}
