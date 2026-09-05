//! Indexed method candidates follow declared collection element types. Index
//! values, bounds, effects, and reference access remain later-stage obligations.

use arena::Arena;
use symbol_resolved_trees::data::DataMember;
use symbol_resolved_trees::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, TableCallExpression,
};
use symbol_resolved_trees::signature::StateParameter;
use symbol_resolved_trees::statement::Statement;
use symbol_resolved_trees::types::TypeReference;
use symbols::{SymbolHandle, SymbolKind, SymbolTable};

use super::super::lookup::call_target_for_attached_data;
use super::super::scope::MachineScope;

#[cfg(test)]
mod tests;

pub(super) fn contains_index(table: &ExpressionTable, mut expression: ExpressionHandle) -> bool {
    loop {
        expression = match table.expression(expression) {
            ExpressionNode::Indexed(_) => return true,
            ExpressionNode::Member(member) => member.receiver,
            ExpressionNode::Borrow(borrow) => borrow.target,
            _ => return false,
        };
    }
}

enum ReceiverType<'program> {
    Declared(&'program TypeReference),
    Data(SymbolHandle),
}

fn peel<'program>(
    mut reference: &'program TypeReference,
    children: &'program Arena<TypeReference>,
) -> &'program TypeReference {
    loop {
        reference = match reference {
            TypeReference::Reference(reference) => children.get(reference.referee),
            TypeReference::Constrained(reference) => children.get(reference.base_type),
            _ => return reference,
        };
    }
}

impl ReceiverType<'_> {
    fn nominal(&self, children: &Arena<TypeReference>) -> Option<SymbolHandle> {
        match self {
            Self::Data(symbol) => Some(*symbol),
            Self::Declared(reference) => match peel(reference, children) {
                TypeReference::Named { symbol, .. } => Some(*symbol),
                _ => None,
            },
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn call_target(
    machine: &MachineScope<'_>,
    parameters: &[StateParameter],
    state_symbol: SymbolHandle,
    call: &TableCallExpression,
    table: &ExpressionTable,
    children: &Arena<TypeReference>,
    symbols: &SymbolTable,
) -> SymbolHandle {
    let resolve = || {
        let mut projections = Vec::new();
        let mut expression = call.receiver;
        let name = loop {
            expression = match table.expression(expression) {
                node @ ExpressionNode::Indexed(indexed) => {
                    projections.push(node);
                    indexed.collection
                }
                node @ ExpressionNode::Member(member) => {
                    projections.push(node);
                    member.receiver
                }
                ExpressionNode::Borrow(borrow) => borrow.target,
                ExpressionNode::Name(name) => break name,
                _ => return None,
            };
        };
        let [spelling] = table.name_path_members(name.members) else {
            return None;
        };
        let declaration = symbols.get(name.symbol);
        if !name.symbol.is_valid() || name.head_symbol != name.symbol {
            return None;
        }
        let mut receiver = if spelling.as_str() == "self"
            && name.symbol == machine.symbol
            && declaration.kind == SymbolKind::Machine
        {
            ReceiverType::Data(machine.attached_data_symbol)
        } else {
            if declaration.parent != state_symbol || symbols.name(name.symbol) != spelling.as_str()
            {
                return None;
            }
            let reference = match declaration.kind {
                SymbolKind::Parameter => {
                    &parameters
                        .iter()
                        .find(|parameter| parameter.symbol == name.symbol)?
                        .type_reference
                }
                SymbolKind::Local => {
                    machine
                        .prior_statements
                        .iter()
                        .find_map(|statement| match statement {
                            Statement::LocalData(local) if local.symbol == name.symbol => {
                                Some(&local.type_reference)
                            }
                            _ => None,
                        })?
                }
                _ => return None,
            };
            ReceiverType::Declared(reference)
        };
        for projection in projections.into_iter().rev() {
            receiver = match projection {
                ExpressionNode::Indexed(_) => {
                    let ReceiverType::Declared(reference) = receiver else {
                        return None;
                    };
                    let element = match peel(reference, children) {
                        TypeReference::FixedArray(array) => array.element_type,
                        TypeReference::Slice(slice) => slice.element_type,
                        _ => return None,
                    };
                    if !element.is_valid() {
                        return None;
                    }
                    ReceiverType::Declared(children.get(element))
                }
                ExpressionNode::Member(member) => {
                    let owner = receiver.nominal(children)?;
                    let definition = machine
                        .data_definitions
                        .iter()
                        .find(|definition| definition.symbol == owner && owner.is_valid())?;
                    let mut fields = machine
                        .data_members
                        .span_or_empty(definition.storage.members)
                        .iter()
                        .filter_map(|member_node| match member_node {
                            DataMember::Field(field) if field.name == member.member => Some(field),
                            _ => None,
                        });
                    let field = fields.next()?;
                    if fields.next().is_some() {
                        return None;
                    }
                    ReceiverType::Declared(&field.type_reference)
                }
                _ => return None,
            };
        }
        let owner = receiver.nominal(children)?;
        (owner.is_valid() && symbols.get(owner).kind == SymbolKind::Data).then(|| {
            call_target_for_attached_data(
                symbols,
                symbols.name(owner),
                call.target.as_str(),
                call.target.source_span(),
            )
        })
    };
    resolve().unwrap_or_else(SymbolHandle::invalid)
}
