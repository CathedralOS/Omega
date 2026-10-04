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

/// The place one dynamic dispatch reads its descriptor from: the operand at
/// the dispatched requirement's subject position.
///
/// A requirement names its subject as an ordinary parameter, so the authored
/// call supplies the descriptor as that position's argument
/// (`Shape::code(erased)`). The qualifier a trait-qualified call carries names
/// the trait, not storage, so the subject is located by POSITION in the
/// requirement -- the same rule `dynamic_subject_position` states for the
/// surface itself. Only the subject operand is supported: a requirement with
/// further parameters supplies no descriptor place here.
pub(crate) fn dispatch_subject_place(
    program: &TypedTrees,
    call_site: &crate::semantic::calls::CallSite<'_>,
    target_symbol: SymbolHandle,
) -> Option<DynamicReceiverPlace> {
    let (_, _, position) = program.dynamic_requirement_subject(target_symbol)?;
    let arguments = crate::semantic::calls::call_site_argument_expressions(program, call_site);
    if arguments.len() != position.checked_add(1)? {
        return None;
    }
    dynamic_receiver_place(program, *arguments.get(position)?)
}

pub(crate) fn stored_dynamic_receiver<'facts>(
    program: &TypedTrees,
    facts: &'facts CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    call_site: &crate::semantic::calls::CallSite<'_>,
    target_symbol: SymbolHandle,
) -> Option<&'facts checked_trees::DynamicDescriptorStorageFact> {
    let place = dispatch_subject_place(program, call_site, target_symbol)?;
    facts.dynamic_conformances.stored_receiver(
        machine,
        state,
        place.root,
        &place.path,
        statement_index,
    )
}
