//! Declared member types retain the selected case instead of flattening it away.

use super::{declared_place_type_raw, unwrapped_type_reference};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[cfg(test)]
mod tests;

pub(super) fn contains_case_projection(
    program: &TypedTrees,
    mut expression: ExpressionHandle,
) -> bool {
    loop {
        expression = match program.expression_table.expression(expression) {
            ExpressionNode::Member(member) if member.case_variant.is_some() => return true,
            ExpressionNode::Member(member) => member.receiver,
            ExpressionNode::Borrow(borrow) => borrow.target,
            _ => return false,
        };
    }
}

pub(super) fn declared_case_projection_type(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let member = match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(borrow) => {
            return declared_place_type_raw(program, machine, state, borrow.target);
        }
        ExpressionNode::Member(member) => member,
        _ => return None,
    };
    let receiver_symbol = match program.expression_table.expression(member.receiver) {
        ExpressionNode::Name(name)
            if name.symbol == machine.symbol
                && matches!(
                    program.expression_table.name_path_members(name.members),
                    [only] if only.as_str() == "self"
                ) =>
        {
            machine.attached_data_symbol
        }
        _ => {
            let receiver_type = declared_place_type_raw(program, machine, state, member.receiver)?;
            let receiver_type = unwrapped_type_reference(program, receiver_type)?;
            match program.type_reference_table.type_reference(receiver_type) {
                TypeReferenceNode::Named { symbol, .. } => *symbol,
                TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
                _ => return None,
            }
        }
    };
    let data = program
        .data_definitions()
        .iter()
        .find(|definition| receiver_symbol.is_valid() && definition.symbol == receiver_symbol)?;
    // Ordinary member symbols may identify synthesized accessors. Payload
    // symbols, when present, identify the field under this exact case.
    let field_symbol = if member.case_variant.is_some() {
        member.member_symbol
    } else {
        SymbolHandle::invalid()
    };
    exact_data_member_field(
        program,
        data,
        field_symbol,
        member.member.as_str(),
        member.case_variant.as_ref().map(|variant| variant.as_str()),
    )
    .map(|field| field.type_reference)
}

/// Select one field under the receiver's nominal declaration and, for a
/// payload, its exact case. A missing payload symbol may be recovered from
/// that declaration; a conflicting retained symbol cannot be replaced.
pub(crate) fn exact_data_member_field<'program>(
    program: &'program TypedTrees,
    data: &'program typed_trees::data::DataDefinition,
    member_symbol: SymbolHandle,
    member_name: &str,
    case_variant: Option<&str>,
) -> Option<&'program typed_trees::data::DataField> {
    if let Some(case_variant) = case_variant {
        let mut matches = program.data_members(data).iter().filter_map(|member| {
            let typed_trees::data::DataMember::Variant(variant) = member else {
                return None;
            };
            (variant.name.as_str() == case_variant).then_some(variant)
        });
        let variant = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        let mut fields = program.data_payload_fields(variant).iter().filter(|field| {
            field.name.as_str() == member_name
                && (!member_symbol.is_valid() || field.symbol == member_symbol)
                && field.symbol.is_valid()
                && field.type_reference.is_valid()
        });
        let field = fields.next()?;
        return fields.next().is_none().then_some(field);
    }

    let mut fields = program.data_members(data).iter().filter_map(|member| {
        let typed_trees::data::DataMember::Field(field) = member else {
            return None;
        };
        (field.name.as_str() == member_name
            && (!member_symbol.is_valid() || field.symbol == member_symbol)
            && field.symbol.is_valid()
            && field.type_reference.is_valid())
        .then_some(field)
    });
    let field = fields.next()?;
    fields.next().is_none().then_some(field)
}
