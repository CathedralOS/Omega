use arena::Arena;
use symbols::{SymbolHandle, SymbolKind, SymbolTable};

use crate::symbols::expressions::{
    assign_expression_span_symbols, assign_statement_expression_symbols,
};
use crate::symbols::scope::MachineScope;
use crate::symbols::scoped_paths::resolve_state_scoped_members;

pub(in crate::symbols) fn assign_transition_target_symbols(
    machine: &MachineScope<'_>,
    parameters: &[symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &mut arena::Arena<symbol_resolved_trees::types::TypeReference>,
    statement_path_members: &mut Arena<symbol_resolved_trees::name::DiagnosticName>,
    target: &mut symbol_resolved_trees::statement::TransitionTarget,
    symbols: &SymbolTable,
) {
    let symbol_resolved_trees::statement::TransitionTarget::Named(named) = target else {
        if let symbol_resolved_trees::statement::TransitionTarget::Value(expression) = target {
            // Parenthesized terminal calls keep ordinary callable selection.
            // Generated body coordinates are never source declarations.
            assign_statement_expression_symbols(
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
                *expression,
            );
        }
        return;
    };

    named.receiver_symbol = SymbolHandle::invalid();
    named.head_symbol = SymbolHandle::invalid();
    named.symbol = SymbolHandle::invalid();
    assign_expression_span_symbols(
        symbols,
        machine,
        parameters,
        state_symbol,
        expression_table,
        child_type_references,
        named.arguments,
    );

    let path = statement_path_members.span_or_empty(named.path);
    let target_name = path.last().cloned();

    // Terminal and ordinary calls select the same source declaration. The
    // callable resolver alone supplies signature coordinates; retain their
    // owning invocation here instead of exposing a generated body by name.
    if path.len() == 1
        && let Some(target_name) = target_name.as_ref()
    {
        let target_symbol = super::calls::resolve_call_target_symbol(
            machine,
            parameters,
            false,
            SymbolHandle::invalid(),
            target_name,
            child_type_references,
            symbols,
        );
        if target_symbol.is_valid() {
            // Callable lookup names a machine's entry for its signature.
            // A terminal invocation retains the machine identity: reusing
            // its entry state would turn recursion into an internal arrival
            // and erase the enclosing invocation's return obligations.
            let invocation = terminal_call_target(symbols, target_symbol);
            named.head_symbol = invocation;
            named.symbol = invocation;
            return;
        }
    }

    let (head_symbol, symbol) =
        resolve_state_scoped_members(symbols, machine.symbol, state_symbol, path, None);
    // A failed member suffix does not erase its lexical receiver. Later
    // callable selection must distinguish a value head from a declaration
    // namespace, exactly as expression calls do.
    named.head_symbol = head_symbol;
    if path.len() >= 2 {
        let (receiver_head, receiver) = resolve_state_scoped_members(
            symbols,
            machine.symbol,
            state_symbol,
            &path[..path.len() - 1],
            None,
        );
        // A certified address may resolve only as a complete declaration
        // prefix. Its selected callee is not the qualifier's lexical head.
        named.head_symbol = receiver_head;
        named.receiver_symbol = receiver;
        if let Some(leaf) = target_name.as_ref() {
            // Qualified terminal invocations use the ordinary scoped callable
            // owner, including trait requirements with Unit results. A failed
            // member never falls back into a global callable namespace.
            let selected = super::calls::resolve_call_target_symbol(
                machine,
                parameters,
                true,
                receiver,
                leaf,
                child_type_references,
                symbols,
            );
            if selected.is_valid() {
                named.symbol = terminal_call_target(symbols, selected);
                return;
            }
        }
    }
    let body_coordinate = symbols.get(symbol).kind == SymbolKind::State
        && symbols.get(symbols.get(symbol).parent).kind == SymbolKind::Machine;
    if symbol.is_valid() && !body_coordinate {
        named.symbol = symbol;
        return;
    }
    if body_coordinate && named.head_symbol == symbol {
        named.head_symbol = SymbolHandle::invalid();
    }
}

/// Callable entry coordinates supply a signature, while the owning machine
/// supplies invocation identity. This does not make a body name source-visible.
pub(super) fn terminal_call_target(symbols: &SymbolTable, target: SymbolHandle) -> SymbolHandle {
    if symbols.get(target).kind == SymbolKind::State
        && symbols.get(symbols.get(target).parent).kind == SymbolKind::Machine
    {
        symbols.get(target).parent
    } else {
        target
    }
}
