use psi_symbols::{SymbolHandle, SymbolKind, SymbolTable};

use super::super::expression_paths::{
    resolve_expression_table_call_target_symbol, resolve_expression_table_member_symbol,
    resolve_expression_table_receiver_path_symbols, stamp_receiver_path_symbols_in_table,
};
use super::super::lookup::{
    call_target_for_attached_data, child_symbol_by_kinds, top_level_symbol,
};
use super::super::scope::MachineScope;
use super::super::scoped_paths::resolve_state_scoped_table_path;
use super::super::targets::assign_static_argument_symbols;

/// The spelled member names of a `self`-rooted receiver path, root -> leaf
/// (`["self", "p"]` for the receiver `self.p`). `None` for non-place receivers
/// (calls, literals, indexed). Mirrors the state-call plan's receiver walk and
/// validation's `receiver_member_chain` at this layer.
fn spelled_receiver_chain(
    expression_table: &psi_symbol_resolved_trees::expression::ExpressionTable,
    receiver: psi_symbol_resolved_trees::expression::ExpressionHandle,
) -> Option<Vec<String>> {
    if !receiver.is_valid() {
        return None;
    }
    match expression_table.expression(receiver) {
        psi_symbol_resolved_trees::expression::ExpressionNode::Name(path) => {
            let members = expression_table.name_path_members(path.members);
            (!members.is_empty()).then(|| {
                members
                    .iter()
                    .map(|member| member.as_str().to_string())
                    .collect()
            })
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            let mut chain = spelled_receiver_chain(expression_table, member.receiver)?;
            chain.push(member.member.as_str().to_string());
            Some(chain)
        }
        psi_symbol_resolved_trees::expression::ExpressionNode::Borrow(inner) => {
            spelled_receiver_chain(expression_table, inner.target)
        }
        _ => None,
    }
}

/// The leaf FIELD symbol of a NESTED `self`-rooted receiver chain: walk the
/// chain's declared field types to the leaf's OWNER type, then look up the leaf
/// field in that data's symbol. `["self", "p", "a"]` -> field `a` in `PairD`'s
/// data symbol. `None` for direct (`len < 3`) chains -- those resolve by the
/// normal machine-child lookup -- or when any hop is unresolvable. Rung 2b of
/// the receiver-place staircase.
fn nested_receiver_leaf_field_symbol(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    chain: &[String],
) -> SymbolHandle {
    if chain.len() < 3 {
        return SymbolHandle::invalid();
    }
    let borrowed: Vec<&str> = chain.iter().map(String::as_str).collect();
    let leaf = borrowed[borrowed.len() - 1];
    let Some(owner_type) = machine.nested_self_chain_type(&borrowed[..borrowed.len() - 1]) else {
        return SymbolHandle::invalid();
    };
    let owner_data = top_level_symbol(symbols, SymbolKind::Data, owner_type);
    if !owner_data.is_valid() {
        return SymbolHandle::invalid();
    }
    child_symbol_by_kinds(
        symbols,
        owner_data,
        &[SymbolKind::Field, SymbolKind::Variant],
        leaf,
    )
}

/// The callee STATE symbol for a method called on a NESTED `self`-rooted
/// receiver (`self.p.a.stored()` -> `BoxI::stored`'s state): walk the full
/// receiver chain to the leaf TYPE, then resolve the method on the machine
/// attached to that type. `None` for direct chains or unresolvable hops.
fn nested_receiver_call_target_symbol(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    receiver_chain: &[String],
    target: &str,
) -> SymbolHandle {
    if receiver_chain.len() < 3 {
        return SymbolHandle::invalid();
    }
    let borrowed: Vec<&str> = receiver_chain.iter().map(String::as_str).collect();
    let Some(leaf_type) = machine.nested_self_chain_type(&borrowed) else {
        return SymbolHandle::invalid();
    };
    call_target_for_attached_data(symbols, leaf_type, target)
}

pub(super) fn assign_call_symbol(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    parameters: &[psi_symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut psi_symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &psi_arena::Arena<psi_symbol_resolved_trees::types::TypeReference>,
    receiver: psi_symbol_resolved_trees::expression::ExpressionHandle,
    call: &psi_symbol_resolved_trees::expression::TableCallExpression,
    expression: psi_symbol_resolved_trees::expression::ExpressionHandle,
) {
    let (head_symbol, symbol) = resolve_expression_table_receiver_path_symbols(
        symbols,
        machine.symbol,
        state_symbol,
        expression_table,
        receiver,
    );
    if symbol.is_valid() {
        stamp_receiver_path_symbols_in_table(expression_table, receiver, head_symbol, symbol);
    }

    let mut target_symbol = resolve_expression_table_call_target_symbol(
        machine,
        parameters,
        state_symbol,
        call,
        expression_table,
        child_type_references,
        symbols,
    );
    // A method on a NESTED `self`-rooted receiver (`self.p.a.stored()`) leaves
    // the leaf receiver symbol -- and so the target -- unresolved by the normal
    // routes (they resolve a member on a field symbol, which has no children).
    // Walk the receiver's declared field types to the leaf type and resolve the
    // method there (rung 2b). The nested-receiver storage binding is guarded by
    // the emission-planning contained-receiver blocker (rung 2a).
    if !target_symbol.is_valid()
        && let Some(receiver_chain) = spelled_receiver_chain(expression_table, receiver)
    {
        target_symbol = nested_receiver_call_target_symbol(
            symbols,
            machine,
            &receiver_chain,
            call.target.as_str(),
        );
    }
    if let psi_symbol_resolved_trees::expression::ExpressionNode::Call(call) =
        expression_table.expression_mut(expression)
    {
        call.target_symbol = target_symbol;
        for argument in &mut call.machine_arguments {
            let proof_static = target_symbol.is_valid()
                && matches!(
                    symbols.get(target_symbol).kind,
                    SymbolKind::Proposition | SymbolKind::PropositionParameter
                );
            assign_static_argument_symbols(symbols, machine.symbol, argument, proof_static);
        }
    }
}

pub(super) fn assign_member_symbol(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    state_symbol: SymbolHandle,
    expression_table: &mut psi_symbol_resolved_trees::expression::ExpressionTable,
    receiver: psi_symbol_resolved_trees::expression::ExpressionHandle,
    member_name: &psi_symbol_resolved_trees::name::DiagnosticName,
    expression: psi_symbol_resolved_trees::expression::ExpressionHandle,
) {
    let mut member_symbol = resolve_expression_table_member_symbol(
        symbols,
        machine.symbol,
        state_symbol,
        expression_table,
        receiver,
        member_name,
    );
    // A leaf member of a NESTED `self`-rooted chain (`a` in `self.p.a`) resolves
    // to nothing by the normal walk (the receiver `self.p` is a field symbol
    // with no children). Recover the leaf FIELD symbol from the chain's declared
    // field types so downstream (the state-call plan's receiver) sees a valid
    // symbol (rung 2b of the receiver-place staircase).
    if !member_symbol.is_valid()
        && let Some(mut chain) = spelled_receiver_chain(expression_table, receiver)
    {
        chain.push(member_name.as_str().to_string());
        member_symbol = nested_receiver_leaf_field_symbol(symbols, machine, &chain);
    }
    if let (symbol, psi_symbol_resolved_trees::expression::ExpressionNode::Member(member)) =
        (member_symbol, expression_table.expression_mut(expression))
        && symbol.is_valid()
    {
        member.member_symbol = symbol;
    }
}

pub(super) fn assign_membership_symbol(
    symbols: &SymbolTable,
    expression_table: &mut psi_symbol_resolved_trees::expression::ExpressionTable,
    domain: psi_arena::HandleSpan<psi_symbol_resolved_trees::name::DiagnosticName>,
    expression: psi_symbol_resolved_trees::expression::ExpressionHandle,
) {
    let name = expression_table
        .name_path_members(domain)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    if let psi_symbol_resolved_trees::expression::ExpressionNode::Membership(membership) =
        expression_table.expression_mut(expression)
    {
        membership.domain_symbol = symbols
            .find_child_by_name_and_kind(symbols.root(), &name, SymbolKind::Domain)
            .unwrap_or_else(SymbolHandle::invalid);
    }
}

pub(super) fn assign_name_symbol(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &mut psi_symbol_resolved_trees::expression::ExpressionTable,
    path: &psi_symbol_resolved_trees::expression::TableNamePath,
    expression: psi_symbol_resolved_trees::expression::ExpressionHandle,
) {
    let (head_symbol, symbol) = resolve_state_scoped_table_path(
        symbols,
        machine_symbol,
        state_symbol,
        expression_table,
        path,
    );
    if symbol.is_valid()
        && let psi_symbol_resolved_trees::expression::ExpressionNode::Name(path) =
            expression_table.expression_mut(expression)
    {
        path.head_symbol = head_symbol;
        path.symbol = symbol;
    }
}
