//! Declared call-result types select typed method candidates. These queries do
//! not prove a runtime receiver origin, effects, or reference access legality.

use symbol_resolved_trees as resolved;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};

#[cfg(test)]
mod tests;

pub(crate) fn declared_call_state(
    program: &resolved::SymbolResolvedTrees,
    target: SymbolHandle,
) -> Option<&resolved::state::State> {
    if !target.is_valid() || program.symbols.get(target).kind != SymbolKind::State {
        return None;
    }
    program
        .tables
        .declarations
        .machine_states
        .iter()
        .find(|(_, state)| state.symbol == target)
        .map(|(_, state)| state)
}

pub(crate) fn peel<'program>(
    program: &'program resolved::SymbolResolvedTrees,
    mut reference: &'program resolved::types::TypeReference,
) -> &'program resolved::types::TypeReference {
    loop {
        reference = match reference {
            resolved::types::TypeReference::Reference(reference) => {
                program.child_type_reference(reference.referee)
            }
            resolved::types::TypeReference::Constrained(reference) => {
                program.child_type_reference(reference.base_type)
            }
            _ => return reference,
        };
    }
}

pub(crate) fn computed_receiver_method_target(
    program: &resolved::SymbolResolvedTrees,
    expressions: &ExpressionTable,
    mut receiver: ExpressionHandle,
    target: &resolved::name::DiagnosticName,
) -> SymbolHandle {
    let mut resolve = || {
        let mut projections = Vec::new();
        let call = loop {
            receiver = match expressions.expression(receiver) {
                node @ ExpressionNode::Member(member) => {
                    projections.push(node);
                    member.receiver
                }
                node @ ExpressionNode::Indexed(indexed) => {
                    projections.push(node);
                    indexed.collection
                }
                ExpressionNode::Borrow(borrow) => borrow.target,
                ExpressionNode::Call(call) => break call,
                _ => return None,
            };
        };
        let state = declared_call_state(program, call.target_symbol)?;
        let owner = program.symbols.get(state.symbol).parent;
        let producer = program
            .machines
            .iter()
            .find(|machine| machine.symbol == owner)?;
        if call.receiver.is_valid() != producer.attached_data.is_some() {
            return None;
        }
        let mut reference = state.return_type.as_ref()?;
        for projection in projections.into_iter().rev() {
            reference = match projection {
                ExpressionNode::Indexed(_) => {
                    let element = match peel(program, reference) {
                        resolved::types::TypeReference::FixedArray(array) => array.element_type,
                        resolved::types::TypeReference::Slice(slice) => slice.element_type,
                        _ => return None,
                    };
                    if !element.is_valid() {
                        return None;
                    }
                    program.child_type_reference(element)
                }
                ExpressionNode::Member(member) => {
                    let resolved::types::TypeReference::Named { symbol, .. } =
                        peel(program, reference)
                    else {
                        return None;
                    };
                    let definition = program
                        .data_definitions
                        .iter()
                        .find(|definition| definition.symbol == *symbol && symbol.is_valid())?;
                    let mut fields = program
                        .data_members(definition.storage.members)
                        .iter()
                        .filter_map(|node| match node {
                            resolved::data::DataMember::Field(field)
                                if field.name.as_str() == member.member.as_str() =>
                            {
                                Some(field)
                            }
                            _ => None,
                        });
                    let field = fields.next()?;
                    if fields.next().is_some()
                        || (member.member_symbol.is_valid() && member.member_symbol != field.symbol)
                    {
                        return None;
                    }
                    &field.type_reference
                }
                _ => return None,
            };
        }
        let resolved::types::TypeReference::Named { symbol: owner, .. } = peel(program, reference)
        else {
            return None;
        };
        if !owner.is_valid() || program.symbols.get(*owner).kind != SymbolKind::Data {
            return None;
        }
        let qualified = format!("{}::{}", program.symbols.name(*owner), target.as_str());
        let machine_symbol = program
            .symbols
            .find_top_level_by_name_and_kinds_from_source(
                &qualified,
                &[SymbolKind::Machine],
                target.source_span(),
            )?;
        let machine = program.machines.iter().find(|machine| {
            machine.symbol == machine_symbol && machine.attached_data_symbol == *owner
        })?;
        let mut states = program
            .machine_state_handles(machine.states)
            .iter()
            .map(|handle| program.machine_state(*handle))
            .filter(|state| state.name.as_str() == target.as_str());
        let state = states.next()?;
        (states.next().is_none() && state.symbol.is_valid()).then_some(state.symbol)
    };
    resolve().unwrap_or_else(SymbolHandle::invalid)
}
