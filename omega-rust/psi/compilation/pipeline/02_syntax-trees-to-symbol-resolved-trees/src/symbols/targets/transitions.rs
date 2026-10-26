use arena::{Arena, HandleSpan};
use symbols::{JoinedPath, SymbolHandle, SymbolKind, SymbolTable};

use crate::symbols::expressions::{
    assign_expression_span_symbols, assign_statement_expression_symbols,
};
use crate::symbols::lookup::{
    call_target_for_attached_data, child_symbol_by_kinds, diagnostic_path_source_span,
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
            // The parser leaves a parenthesized lone call (`-> (count(n))`)
            // as a value expression -- only here, where the machine's states
            // are known, can it be told apart from a state transition. A call
            // that names an unmarked sibling state keeps its transition
            // meaning; a recursive machine invocation and any other
            // callee really is a value expression (`-> (burn(4, 12))`).
            if let Some(named) = reclassify_state_call_value_target(
                machine,
                expression_table,
                statement_path_members,
                *expression,
                symbols,
            ) {
                *target = symbol_resolved_trees::statement::TransitionTarget::Named(named);
                let symbol_resolved_trees::statement::TransitionTarget::Named(named) = target
                else {
                    unreachable!("transition target was just reclassified as named");
                };
                assign_expression_span_symbols(
                    symbols,
                    machine,
                    parameters,
                    state_symbol,
                    expression_table,
                    child_type_references,
                    named.arguments,
                );
                return;
            }
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

    // A transition target is a state coordinate, even when an attached-data
    // field has the same spelling. Ordinary scoped lookup includes fields and
    // would otherwise bind `-> next()` to `self.next` before the state-only
    // fallback below had a chance to run.
    if path.len() == 1
        && let Some(target_name) = target_name.as_ref()
    {
        let target_symbol = child_symbol_by_kinds(
            symbols,
            machine.symbol,
            &[SymbolKind::State],
            target_name.as_str(),
        );
        if target_symbol.is_valid() {
            named.head_symbol = target_symbol;
            named.symbol = target_symbol;
            return;
        }
    }

    // Terminal calls retain the same exact lexical entry or requirement as
    // ordinary calls. Discarding an independently owned entry here forced
    // typed checking to rediscover a target by its unqualified name.
    // The current invocation's private state targets keep precedence above.
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
        if matches!(
            symbols.get(target_symbol).kind,
            SymbolKind::MachineParameter | SymbolKind::State
        ) {
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
        if receiver_head.is_valid() {
            named.head_symbol = receiver_head;
        }
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
                named.symbol = selected;
                return;
            }
        }
    }
    if symbol.is_valid() {
        named.symbol = symbol;
        return;
    }

    let Some(target_name) = target_name else {
        return;
    };

    // A qualified tail call (`-> Main::pack(left, right)`) is stored as a
    // named transition target, while the same call in value position is
    // resolved by the expression-call path. Mirror that static attached-data
    // lookup here. The ordinary scoped-path walk cannot descend from a data
    // symbol into a separately declared attached machine, so without this
    // fallback the target stayed invalid and downstream ownership/call facts
    // silently treated the arguments as non-transferring.
    if path.len() >= 2 {
        let owner = path[..path.len() - 1].joined_path();
        let target_symbol = call_target_for_attached_data(
            symbols,
            owner.as_str(),
            target_name.as_str(),
            target_name.source_span(),
        );
        if target_symbol.is_valid() {
            if !named.head_symbol.is_valid() {
                named.head_symbol = symbols
                    .find_top_level_by_name_and_kinds_from_source(
                        &owner,
                        &[SymbolKind::Data],
                        diagnostic_path_source_span(&path[..path.len() - 1]),
                    )
                    .unwrap_or_else(SymbolHandle::invalid);
            }
            named.symbol = terminal_call_target(symbols, target_symbol);
            return;
        }
    }

    if path.len() <= 2 {
        let target_symbol = child_symbol_by_kinds(
            symbols,
            machine.symbol,
            &[SymbolKind::State],
            target_name.as_str(),
        );
        if target_symbol.is_valid() {
            named.head_symbol = target_symbol;
            named.symbol = target_symbol;
            return;
        }
    }
}

/// Callable entry coordinates supply a signature, while the owning machine
/// supplies invocation identity. Private state arrivals bypass this conversion.
fn terminal_call_target(symbols: &SymbolTable, target: SymbolHandle) -> SymbolHandle {
    if symbols.get(target).kind == SymbolKind::State
        && symbols.get(symbols.get(target).parent).kind == SymbolKind::Machine
    {
        symbols.get(target).parent
    } else {
        target
    }
}

/// Recover only an unmarked sibling-state jump from a parenthesized call.
/// Machine recursion must first resolve as an ordinary invocation: spelling a
/// machine's leaf name does not establish a state target, and converting its
/// call here would erase its operational acknowledgement. Identity-based tail
/// normalization happens afterward in `statements::tail_calls`, independently
/// of call admission (language/state_contracts.md and language/effects.md).
fn reclassify_state_call_value_target(
    machine: &MachineScope<'_>,
    expression_table: &symbol_resolved_trees::expression::ExpressionTable,
    statement_path_members: &mut Arena<symbol_resolved_trees::name::DiagnosticName>,
    expression: symbol_resolved_trees::expression::ExpressionHandle,
    symbols: &SymbolTable,
) -> Option<symbol_resolved_trees::statement::NamedTransitionTarget> {
    let symbol_resolved_trees::expression::ExpressionNode::Call(call) =
        expression_table.expression(expression)
    else {
        return None;
    };
    if call.receiver.is_valid()
        || !call.machine_arguments.is_empty()
        || call.operational_acknowledgement.acknowledges_suspend
        || call.operational_acknowledgement.acknowledges_block
    {
        return None;
    }

    let state_symbol = child_symbol_by_kinds(
        symbols,
        machine.symbol,
        &[SymbolKind::State],
        call.target.as_str(),
    );
    if !state_symbol.is_valid() {
        return None;
    }

    let mut path = HandleSpan::empty();
    statement_path_members.append_to_span(&mut path, call.target.clone());

    Some(symbol_resolved_trees::statement::NamedTransitionTarget {
        receiver_symbol: Default::default(),
        head_symbol: state_symbol,
        symbol: state_symbol,
        storage: symbol_resolved_trees::statement::NamedTransitionTargetStorage {
            operational_acknowledgement: Default::default(),
            path,
            arguments: call.arguments,
            evidence_arguments: call.evidence_arguments.clone(),
            source_span: call.target.source_span(),
            authored_call_selection: None,
        },
    })
}
