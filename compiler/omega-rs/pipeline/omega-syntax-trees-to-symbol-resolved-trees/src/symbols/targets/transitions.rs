use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::{SymbolHandle, SymbolKind, SymbolTable};

use crate::symbols::expressions::{
    assign_expression_span_symbols, assign_statement_expression_symbols,
};
use crate::symbols::lookup::{call_target_for_attached_data, child_symbol_by_kinds};
use crate::symbols::scope::MachineScope;
use crate::symbols::scoped_paths::resolve_state_scoped_members;

pub(in crate::symbols) fn assign_transition_target_symbols(
    machine: &MachineScope<'_>,
    parameters: &[omega_symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut omega_symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &mut omega_core::arena::Arena<
        omega_symbol_resolved_trees::types::TypeReference,
    >,
    statement_path_members: &mut Arena<omega_symbol_resolved_trees::name::DiagnosticName>,
    target: &mut omega_symbol_resolved_trees::statement::TransitionTarget,
    symbols: &SymbolTable,
) {
    let omega_symbol_resolved_trees::statement::TransitionTarget::Named(named) = target else {
        if let omega_symbol_resolved_trees::statement::TransitionTarget::Value(expression) = target
        {
            // The parser leaves a parenthesized lone call (`-> (count(n))`)
            // as a value expression -- only here, where the machine's states
            // are known, can it be told apart from a state transition. A call
            // that names a sibling state (or the machine's own self-recursion
            // entry) keeps its historical transition meaning; any other
            // callee really is a value expression (`-> (burn(4, 12))`).
            if let Some(named) = reclassify_state_call_value_target(
                machine,
                expression_table,
                statement_path_members,
                *expression,
                symbols,
            ) {
                *target = omega_symbol_resolved_trees::statement::TransitionTarget::Named(named);
                let omega_symbol_resolved_trees::statement::TransitionTarget::Named(named) = target
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
    let (head_symbol, symbol) = resolve_state_scoped_members(
        symbols,
        machine.symbol,
        state_symbol,
        path,
        named.path_starts_at_self,
    );
    if symbol.is_valid() {
        named.head_symbol = head_symbol;
        named.symbol = symbol;
        return;
    }

    let Some(target_name) = target_name else {
        return;
    };

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

        if named.path_starts_at_self
            && let Some(attached_data) = machine.attached_data
        {
            let target_symbol = call_target_for_attached_data(
                symbols,
                attached_data.as_str(),
                target_name.as_str(),
            );
            if target_symbol.is_valid() {
                named.head_symbol = machine.symbol;
                named.symbol = target_symbol;
            }
        }
    }
}

/// Re-classify a VALUE transition target that is a lone receiver-less call
/// back into a NAMED state transition when the callee names a sibling state
/// of the current machine, or the machine's own free-machine self-recursion
/// (`-> (count(n))` inside top-level `machine count`). The parser cannot make
/// this call -- arm bodies parse before later sibling states are seen -- so a
/// parenthesized call arm reaches this stage as a value expression and only
/// here keeps its historical transition meaning. Any other callee (a free
/// machine, a method) stays a value expression, exactly like the same call
/// wrapped in arithmetic.
fn reclassify_state_call_value_target(
    machine: &MachineScope<'_>,
    expression_table: &omega_symbol_resolved_trees::expression::ExpressionTable,
    statement_path_members: &mut Arena<omega_symbol_resolved_trees::name::DiagnosticName>,
    expression: omega_symbol_resolved_trees::expression::ExpressionHandle,
    symbols: &SymbolTable,
) -> Option<omega_symbol_resolved_trees::statement::NamedTransitionTarget> {
    let omega_symbol_resolved_trees::expression::ExpressionNode::Call(call) =
        expression_table.expression(expression)
    else {
        return None;
    };
    if call.receiver.is_valid() {
        return None;
    }

    let state_symbol = child_symbol_by_kinds(
        symbols,
        machine.symbol,
        &[SymbolKind::State],
        call.target.as_str(),
    );
    let machine_leaf_name = symbols
        .name(machine.symbol)
        .rsplit("::")
        .next()
        .unwrap_or_default();
    let is_free_machine_self_recursion =
        machine.attached_data.is_none() && machine_leaf_name == call.target.as_str();
    if !state_symbol.is_valid() && !is_free_machine_self_recursion {
        return None;
    }

    let mut path = HandleSpan::empty();
    statement_path_members.append_to_span(&mut path, call.target.clone());

    Some(omega_symbol_resolved_trees::statement::NamedTransitionTarget {
        head_symbol: state_symbol,
        symbol: state_symbol,
        storage: omega_symbol_resolved_trees::statement::NamedTransitionTargetStorage {
            path,
            path_starts_at_self: false,
            arguments: call.arguments,
        },
    })
}
