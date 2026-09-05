use symbols::{SymbolHandle, SymbolKind, SymbolTable};

use super::super::expression_paths::{
    resolve_expression_table_call_target_symbol, resolve_expression_table_member_symbol,
    resolve_expression_table_receiver_path_symbols, stamp_receiver_path_symbols_in_table,
};
use super::super::lookup::{
    call_target_for_attached_data, child_symbol_by_kinds, diagnostic_path_source_span,
    top_level_symbol_for_source,
};
use super::super::scope::MachineScope;
use super::super::scoped_paths::{
    resolve_state_scoped_table_path, resolve_state_scoped_table_path_member_symbols,
};
use super::super::targets::{
    assign_provider_selection_argument_symbol, assign_representation_selection_argument_symbol,
    assign_static_argument_symbols,
};

/// The spelled member names of a `self`-rooted receiver path, root -> leaf
/// (`["self", "p"]` for the receiver `self.p`). `None` for non-place receivers
/// (calls, literals, indexed). Mirrors the state-call plan's receiver walk and
/// validation's `receiver_member_chain` at this layer.
fn spelled_receiver_chain(
    expression_table: &symbol_resolved_trees::expression::ExpressionTable,
    receiver: symbol_resolved_trees::expression::ExpressionHandle,
) -> Option<Vec<String>> {
    if !receiver.is_valid() {
        return None;
    }
    match expression_table.expression(receiver) {
        symbol_resolved_trees::expression::ExpressionNode::Name(path) => {
            let members = expression_table.name_path_members(path.members);
            (!members.is_empty()).then(|| {
                members
                    .iter()
                    .map(|member| member.as_str().to_string())
                    .collect()
            })
        }
        symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            let mut chain = spelled_receiver_chain(expression_table, member.receiver)?;
            chain.push(member.member.as_str().to_string());
            Some(chain)
        }
        symbol_resolved_trees::expression::ExpressionNode::Borrow(inner) => {
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
    let reference = symbols
        .symbol_provenance_source_span(machine.symbol)
        .unwrap_or_default();
    let owner_data = symbols
        .find_top_level_by_name_and_kinds_from_source(owner_type, &[SymbolKind::Data], reference)
        .unwrap_or_else(SymbolHandle::invalid);
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
    target: &symbol_resolved_trees::name::DiagnosticName,
) -> SymbolHandle {
    if receiver_chain.len() < 3 {
        return SymbolHandle::invalid();
    }
    let borrowed: Vec<&str> = receiver_chain.iter().map(String::as_str).collect();
    let Some(leaf_type) = machine.nested_self_chain_type(&borrowed) else {
        return SymbolHandle::invalid();
    };
    call_target_for_attached_data(symbols, leaf_type, target.as_str(), target.source_span())
}

pub(super) fn assign_call_symbol(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    parameters: &[symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &arena::Arena<symbol_resolved_trees::types::TypeReference>,
    receiver: symbol_resolved_trees::expression::ExpressionHandle,
    call: &symbol_resolved_trees::expression::TableCallExpression,
    expression: symbol_resolved_trees::expression::ExpressionHandle,
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
        target_symbol =
            nested_receiver_call_target_symbol(symbols, machine, &receiver_chain, &call.target);
    }
    if let symbol_resolved_trees::expression::ExpressionNode::Call(call) =
        expression_table.expression_mut(expression)
    {
        call.target_symbol = target_symbol;
        let provider_selection = call.target.as_str() == "select_provider";
        let representation_selection = call.target.as_str() == "select_representation";
        for (index, argument) in call.machine_arguments.iter_mut().enumerate() {
            if provider_selection {
                assign_provider_selection_argument_symbol(symbols, argument, index == 0);
            } else if representation_selection {
                assign_representation_selection_argument_symbol(symbols, argument, index == 0);
            } else {
                let proof_static = target_symbol.is_valid()
                    && matches!(
                        symbols.get(target_symbol).kind,
                        SymbolKind::Proposition | SymbolKind::PropositionParameter
                    );
                assign_static_argument_symbols(symbols, machine.symbol, argument, proof_static);
            }
        }
    }
}

pub(in crate::symbols) fn assign_member_symbol(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    state_symbol: SymbolHandle,
    expression_table: &mut symbol_resolved_trees::expression::ExpressionTable,
    receiver: symbol_resolved_trees::expression::ExpressionHandle,
    member_name: &symbol_resolved_trees::name::DiagnosticName,
    expression: symbol_resolved_trees::expression::ExpressionHandle,
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
    if let (symbol, symbol_resolved_trees::expression::ExpressionNode::Member(member)) =
        (member_symbol, expression_table.expression_mut(expression))
        && symbol.is_valid()
    {
        member.member_symbol = symbol;
    }
}

pub(in crate::symbols) fn assign_membership_symbol(
    symbols: &SymbolTable,
    expression_table: &mut symbol_resolved_trees::expression::ExpressionTable,
    domain: arena::HandleSpan<symbol_resolved_trees::name::DiagnosticName>,
    expression: symbol_resolved_trees::expression::ExpressionHandle,
) {
    let name = expression_table
        .name_path_members(domain)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    let members = expression_table.name_path_members(domain);
    let reference_span = diagnostic_path_source_span(members);
    let domain_symbol = symbols
        .find_top_level_by_name_and_kinds_from_source(&name, &[SymbolKind::Domain], reference_span)
        .unwrap_or_else(SymbolHandle::invalid);
    let (case_type_symbol, case_symbol) = if domain_symbol.is_valid() {
        (SymbolHandle::invalid(), SymbolHandle::invalid())
    } else {
        let [type_name, case_name] = members else {
            return;
        };
        let type_symbol = top_level_symbol_for_source(symbols, SymbolKind::Data, type_name);
        let case_symbol = if type_symbol.is_valid() {
            {
                child_symbol_by_kinds(
                    symbols,
                    type_symbol,
                    &[SymbolKind::Variant],
                    case_name.as_str(),
                )
            }
        } else {
            SymbolHandle::invalid()
        };
        (type_symbol, case_symbol)
    };
    if let symbol_resolved_trees::expression::ExpressionNode::Membership(membership) =
        expression_table.expression_mut(expression)
    {
        membership.domain_symbol = domain_symbol;
        membership.case_type_symbol = case_type_symbol;
        membership.case_symbol = case_symbol;
    }
}

pub(in crate::symbols) fn assign_name_symbol(
    symbols: &SymbolTable,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression_table: &mut symbol_resolved_trees::expression::ExpressionTable,
    path: &symbol_resolved_trees::expression::TableNamePath,
    expression: symbol_resolved_trees::expression::ExpressionHandle,
) {
    let member_symbols = resolve_state_scoped_table_path_member_symbols(
        symbols,
        machine_symbol,
        state_symbol,
        expression_table,
        path,
    );
    let (head_symbol, symbol) = resolve_state_scoped_table_path(
        symbols,
        machine_symbol,
        state_symbol,
        expression_table,
        path,
    );
    if symbol.is_valid()
        && member_symbols.len() == path.members.count() as usize
        && member_symbols.iter().all(|symbol| symbol.is_valid())
    {
        for (offset, member_symbol) in member_symbols.into_iter().enumerate() {
            expression_table.set_name_path_member_symbol_at_offset(
                path.member_symbols,
                offset
                    .try_into()
                    .expect("name path member symbol count overflow"),
                member_symbol,
            );
        }
    }
    if symbol.is_valid()
        && let symbol_resolved_trees::expression::ExpressionNode::Name(path) =
            expression_table.expression_mut(expression)
    {
        path.head_symbol = head_symbol;
        path.symbol = symbol;
    }
}

pub(in crate::symbols) fn assign_struct_literal_symbols(
    symbols: &SymbolTable,
    expression_table: &mut symbol_resolved_trees::expression::ExpressionTable,
    expression: symbol_resolved_trees::expression::ExpressionHandle,
) {
    let symbol_resolved_trees::expression::ExpressionNode::StructLiteral(literal) =
        expression_table.expression(expression).clone()
    else {
        return;
    };
    let type_symbol = top_level_symbol_for_source(symbols, SymbolKind::Data, &literal.type_name);
    let case_symbol = literal.case_name.as_ref().map(|case_name| {
        if type_symbol.is_valid() {
            child_symbol_by_kinds(
                symbols,
                type_symbol,
                &[SymbolKind::Variant],
                case_name.as_str(),
            )
        } else {
            SymbolHandle::invalid()
        }
    });
    let field_symbols = expression_table
        .struct_fields(literal.fields)
        .iter()
        .map(|field| {
            let case_field = case_symbol
                .filter(|case_symbol| case_symbol.is_valid())
                .map(|case_symbol| {
                    child_symbol_by_kinds(
                        symbols,
                        case_symbol,
                        &[SymbolKind::Field],
                        field.name.as_str(),
                    )
                })
                .unwrap_or_else(SymbolHandle::invalid);
            if case_field.is_valid() {
                case_field
            } else if type_symbol.is_valid() {
                child_symbol_by_kinds(
                    symbols,
                    type_symbol,
                    &[SymbolKind::Field],
                    field.name.as_str(),
                )
            } else {
                SymbolHandle::invalid()
            }
        })
        .collect::<Vec<_>>();

    for (offset, field_symbol) in field_symbols.into_iter().enumerate() {
        let field = expression_table.struct_fields(literal.fields)[offset].clone();
        expression_table.set_struct_field_at_offset(
            literal.fields,
            offset
                .try_into()
                .expect("struct literal field count overflow"),
            symbol_resolved_trees::expression::TableStructLiteralField {
                field_symbol,
                ..field
            },
        );
    }
    if let symbol_resolved_trees::expression::ExpressionNode::StructLiteral(literal) =
        expression_table.expression_mut(expression)
    {
        literal.type_symbol = type_symbol;
        literal.case_symbol = case_symbol;
    }
}
