use psi_symbols::{SymbolHandle, SymbolKind, SymbolTable};

use super::super::scope::MachineScope;
use super::super::targets::resolve_call_target_symbol;
use super::receivers::resolve_expression_table_receiver_symbol;

pub(in crate::symbols) fn resolve_expression_table_call_target_symbol(
    machine: &MachineScope<'_>,
    parameters: &[psi_symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    call: &psi_symbol_resolved_trees::expression::TableCallExpression,
    expression_table: &psi_symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &psi_arena::Arena<psi_symbol_resolved_trees::types::TypeReference>,
    symbols: &SymbolTable,
) -> SymbolHandle {
    if call.receiver.is_valid() {
        if receiver_needs_result_type(expression_table, call.receiver) {
            // A result receiver is not a receiverless call. Its declared type
            // settles during typing; a same-spelling free machine cannot select
            // the method before that type is available.
            return SymbolHandle::invalid();
        }
        if super::indexed_receivers::contains_index(expression_table, call.receiver) {
            return super::indexed_receivers::call_target(
                machine,
                parameters,
                state_symbol,
                call,
                expression_table,
                child_type_references,
                symbols,
            );
        }
        let receiver_symbol = resolve_expression_table_receiver_symbol(
            symbols,
            machine.symbol,
            state_symbol,
            expression_table,
            call.receiver,
        );
        let resolved = resolve_call_target_symbol(
            machine,
            parameters,
            true,
            receiver_symbol,
            &call.target,
            child_type_references,
            symbols,
        );
        if resolved.is_valid() {
            return resolved;
        }

        // A short domain home can remain symbol-unresolved in a contract
        // expression even though its exact attached machine is present as a
        // top-level declaration. Preserve the call-shaped source surface by
        // resolving that declaration directly; content validation still
        // requires the exact compiler-owned projection plan.
        if (!receiver_symbol.is_valid()
            || matches!(symbols.get(receiver_symbol).kind, SymbolKind::Domain))
            && let psi_symbol_resolved_trees::expression::ExpressionNode::Name(path) =
                expression_table.expression(call.receiver)
            && let [owner] = expression_table.name_path_members(path.members)
        {
            return super::super::lookup::call_target_for_attached_data(
                symbols,
                owner.as_str(),
                call.target.as_str(),
                call.target.source_span(),
            );
        }
        return SymbolHandle::invalid();
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

fn receiver_needs_result_type(
    expressions: &psi_symbol_resolved_trees::expression::ExpressionTable,
    mut receiver: psi_symbol_resolved_trees::expression::ExpressionHandle,
) -> bool {
    use psi_symbol_resolved_trees::expression::ExpressionNode;
    loop {
        receiver = match expressions.expression(receiver) {
            ExpressionNode::Call(_) => return true,
            ExpressionNode::Member(member) => member.receiver,
            ExpressionNode::Indexed(indexed) => indexed.collection,
            ExpressionNode::Borrow(borrow) => borrow.target,
            _ => return false,
        };
    }
}
