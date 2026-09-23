//! Dynamic receiver places and stored receivers.

use crate::execution::terminal_unit::{CheckFacts, ExpressionNode, SymbolHandle, TypedTrees};
use typed_trees::name::Identifier;

pub(crate) struct DynamicReceiverPlace {
    pub(crate) root: SymbolHandle,
    pub(crate) leaf: SymbolHandle,
    pub(crate) path: Vec<Identifier>,
}

pub(crate) fn dynamic_receiver_place(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<DynamicReceiverPlace> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(name) => {
            let path = program
                .expression_table
                .name_path_members(name.members)
                .to_vec();
            if path.is_empty() {
                return None;
            }
            Some(DynamicReceiverPlace {
                root: if name.head_symbol.is_valid() {
                    name.head_symbol
                } else {
                    name.symbol
                },
                leaf: name.symbol,
                path,
            })
        }
        ExpressionNode::Member(member) => {
            let mut place = dynamic_receiver_place(program, member.receiver)?;
            place.leaf = member.member_symbol;
            place.path.push(member.member.clone());
            Some(place)
        }
        _ => None,
    }
}

pub(crate) fn stored_dynamic_receiver<'facts>(
    program: &TypedTrees,
    facts: &'facts CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    call_site: &crate::semantic::calls::CallSite<'_>,
) -> Option<&'facts checked_trees::DynamicDescriptorStorageFact> {
    let crate::semantic::calls::CallSite::Expression { call, .. } = call_site else {
        return None;
    };
    let place = dynamic_receiver_place(program, call.receiver)?;
    facts.dynamic_conformances.stored_receiver(
        machine,
        state,
        place.root,
        &place.path,
        statement_index,
    )
}

pub(crate) fn local_receiver_symbol(
    program: &TypedTrees,
    call_site: &crate::semantic::calls::CallSite<'_>,
) -> Option<SymbolHandle> {
    match call_site {
        crate::semantic::calls::CallSite::Expression { call, .. } => {
            let ExpressionNode::Name(path) = program.expression_table.expression(call.receiver)
            else {
                return None;
            };
            let [_name] = program.expression_table.name_path_members(path.members) else {
                return None;
            };
            Some(path.symbol)
        }
        crate::semantic::calls::CallSite::Statement(call) => {
            let [_name] = program.statement_table.name_path_members(call.receiver) else {
                return None;
            };
            Some(call.receiver_symbol)
        }
        crate::semantic::calls::CallSite::TransitionNamed { .. } => None,
    }
}
