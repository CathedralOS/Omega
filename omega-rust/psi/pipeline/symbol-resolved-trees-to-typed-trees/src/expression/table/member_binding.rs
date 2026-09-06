//! Bind missing indexed members through the receiver's declared element type.

use super::lowerer::ExpressionTableLowerer;
use crate::call_results::peel;
use crate::lowerer::{exact_field_symbol, exact_top_level_data_symbol};
use symbol_resolved_trees as resolved;
use symbols::{SymbolHandle, SymbolKind};

use resolved::data::{DataField, DataMember};
use resolved::expression::{ExpressionHandle, ExpressionNode, TableMemberExpression};
use resolved::types::TypeReference;

impl ExpressionTableLowerer<'_, '_, '_> {
    pub(super) fn indexed_member_symbol(&self, member: &TableMemberExpression) -> SymbolHandle {
        // A nonzero selection already has an owner. Preserve it, including a
        // conflicting/stale selection, for the existing validation contract.
        if member.member_symbol.is_valid()
            || member.case_variant.is_some()
            || !matches!(
                self.source.expression(member.receiver),
                ExpressionNode::Indexed(_)
            )
        {
            return member.member_symbol;
        }
        self.program
            .and_then(|program| {
                let receiver = declared_type(program, self.source, member.receiver, 0)?;
                declared_field(program, receiver, member).map(|field| field.symbol)
            })
            .unwrap_or(member.member_symbol)
    }
}

fn declared_type<'program>(
    program: &'program resolved::SymbolResolvedTrees,
    expressions: &resolved::expression::ExpressionTable,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<&'program TypeReference> {
    if !expression.is_valid() || depth >= 128 {
        return None;
    }
    match expressions.expression(expression) {
        ExpressionNode::Name(path) => declared_symbol_type(program, path.symbol),
        ExpressionNode::Borrow(borrow) => {
            declared_type(program, expressions, borrow.target, depth + 1)
        }
        ExpressionNode::Indexed(indexed) => {
            if !indexed.index.is_valid()
                || matches!(
                    expressions.expression(indexed.index),
                    ExpressionNode::Range(_)
                )
            {
                return None;
            }
            let collection = declared_type(program, expressions, indexed.collection, depth + 1)?;
            // As in computed receiver typing, each [] consumes exactly one
            // array/slice layer; wrapper peeling never consumes an element.
            let element = match peel(program, collection) {
                TypeReference::FixedArray(array) => array.element_type,
                TypeReference::Slice(slice) => slice.element_type,
                _ => return None,
            };
            element
                .is_valid()
                .then(|| program.child_type_reference(element))
        }
        ExpressionNode::Member(member) => {
            // Explicit self may retain its machine identity rather than a
            // nominal value type. Its inherited field slot still has an exact
            // declared type; require that slot to belong to this receiver.
            if let ExpressionNode::Name(path) = expressions.expression(member.receiver)
                && path.is_self_value
                && path.symbol.is_valid()
                && member.member_symbol.is_valid()
                && program.symbols.get(path.symbol).kind == SymbolKind::Machine
                && program.symbols.get(member.member_symbol).parent == path.symbol
                && program.symbols.name(member.member_symbol) == member.member.as_str()
            {
                return declared_symbol_type(program, member.member_symbol);
            }
            let receiver = declared_type(program, expressions, member.receiver, depth + 1)?;
            Some(&declared_field(program, receiver, member)?.type_reference)
        }
        _ => None,
    }
}

fn declared_field<'program>(
    program: &'program resolved::SymbolResolvedTrees,
    receiver: &TypeReference,
    member: &TableMemberExpression,
) -> Option<&'program DataField> {
    if member.case_variant.is_some() {
        return None;
    }
    let symbol = match peel(program, receiver) {
        TypeReference::Named { symbol, .. } | TypeReference::SelfType { symbol } => *symbol,
        _ => return None,
    };
    let definition = program.data_definitions.iter().find(|definition| {
        definition.symbol == symbol && exact_top_level_data_symbol(program, definition)
    })?;
    let mut fields = program
        .data_members(definition.members)
        .iter()
        .filter_map(|node| {
            let DataMember::Field(field) = node else {
                return None;
            };
            (field.name.as_str() == member.member.as_str()
                && exact_field_symbol(program, definition.symbol, field))
            .then_some(field)
        });
    let field = fields.next()?;
    if fields.next().is_some()
        || (member.member_symbol.is_valid() && member.member_symbol != field.symbol)
    {
        return None;
    }
    Some(field)
}

fn declared_symbol_type(
    program: &resolved::SymbolResolvedTrees,
    symbol: SymbolHandle,
) -> Option<&TypeReference> {
    if !symbol.is_valid() {
        return None;
    }
    let declaration = program.symbols.get(symbol);
    match declaration.kind {
        SymbolKind::Parameter | SymbolKind::Local => {
            let state = program
                .tables
                .declarations
                .machine_states
                .iter()
                .find_map(|(_, state)| (state.symbol == declaration.parent).then_some(state))?;
            program
                .state_parameters(state.parameters)
                .iter()
                .find(|parameter| parameter.symbol == symbol)
                .map(|parameter| &parameter.type_reference)
                .or_else(|| {
                    program
                        .state_statements(state.statements)
                        .iter()
                        .find_map(|statement| {
                            let resolved::statement::Statement::LocalData(local) = statement else {
                                return None;
                            };
                            (local.symbol == symbol).then_some(&local.type_reference)
                        })
                })
        }
        SymbolKind::Field => {
            for definition in &program.data_definitions {
                if let Some(reference) = program.data_members(definition.members).iter().find_map(
                    |member| match member {
                        DataMember::Field(field)
                            if field.symbol == symbol
                                && exact_field_symbol(program, definition.symbol, field) =>
                        {
                            Some(&field.type_reference)
                        }
                        _ => None,
                    },
                ) {
                    return Some(reference);
                }
            }
            // Attached machines retain inherited field symbols in declaration
            // order. Rejoin that exact slot, as the checked place resolver does.
            let machine = program
                .machines
                .iter()
                .find(|machine| machine.symbol == declaration.parent)?;
            let owner = program.data_definitions.iter().find(|definition| {
                definition.symbol == machine.attached_data_symbol
                    && exact_top_level_data_symbol(program, definition)
            })?;
            let ordinal = program
                .symbols
                .child_handles(machine.symbol)?
                .filter(|candidate| program.symbols.get(*candidate).kind == SymbolKind::Field)
                .position(|candidate| candidate == symbol)?;
            let field = program
                .data_members(owner.members)
                .iter()
                .filter_map(|member| match member {
                    DataMember::Field(field) => Some(field),
                    _ => None,
                })
                .nth(ordinal)?;
            (exact_field_symbol(program, owner.symbol, field)
                && program.symbols.name(symbol) == field.name.as_str())
            .then_some(&field.type_reference)
        }
        _ => None,
    }
}
