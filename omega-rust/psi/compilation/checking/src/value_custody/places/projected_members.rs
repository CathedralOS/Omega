//! Declared member types retain the selected case instead of flattening it away.
//!
//! Prepared applications own substituted field types, but their concrete storage
//! symbols can differ from the template selectors retained by source expressions.
//! Join those selectors through the receiver's exact declaration correspondence
//! before querying a field. Symbolic views keep their template symbols; neither
//! route substitutes another declaration's same-named field or supplies membership.

use super::declared_place_type_raw;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::TypeReferenceHandle;

/// Read a member's applied type without reopening the generic declaration.
/// Closed storage may translate a retained template selector; an open view
/// already uses the template's exact field namespace. Payload selection stays
/// qualified by its case and ordinary checking still owes case membership.
pub fn applied_data_member_field<'program>(
    program: &'program dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    member: &typed_trees::expression::TableMemberExpression,
) -> Option<&'program typed_trees::data::DataField> {
    applied_data_field(
        program,
        reference,
        member.member_symbol,
        member.member.as_str(),
        member.case_variant.as_ref().map(|variant| variant.as_str()),
    )
}

pub(super) fn applied_data_field<'program>(
    program: &'program dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    retained: SymbolHandle,
    member_name: &str,
    case_variant: Option<&str>,
) -> Option<&'program typed_trees::data::DataField> {
    let reference = program
        .type_reference_table
        .peeled_type_reference(reference);
    let closed = program.closed_data_definition(reference);
    let data = match closed {
        Some(data) => data,
        None => program
            .data_application_definition(reference)
            .or_else(|| super::data_definition_for_type(program, reference))?,
    };
    let selected = if closed.is_some() && retained.is_valid() {
        program
            .closed_data_constructor_field(reference, data.symbol, retained)?
            .symbol
    } else {
        retained
    };
    exact_data_member_field(program, data, selected, member_name, case_variant)
}

pub(super) fn contains_case_projection(
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    if let Some(borrow) = program.expression_table.borrow_expression(expression) {
        return declared_place_type_raw(program, machine, state, borrow.target);
    }
    declared_case_projection_field(program, machine, state, expression)
        .map(|field| field.type_reference)
}

/// The exact payload/common field a case-tagged member projection resolves to.
/// Same receiver/declaration walk as the type query; returning the field keeps
/// the declaration symbol (an operand identity) alongside its declared type.
pub fn declared_case_projection_field<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> Option<&'program typed_trees::data::DataField> {
    let member = match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => member,
        _ => return None,
    };
    let receiver_symbol = match program.expression_table.expression(member.receiver) {
        ExpressionNode::Name(name)
            if name.symbol == machine.symbol
                && matches!(
                    program.expression_table.name_path_members(name.members),
                    [only] if only.is_self_receiver()
                ) =>
        {
            machine.attached_data_symbol
        }
        _ => {
            let receiver_type = declared_place_type_raw(program, machine, state, member.receiver)?;
            return applied_data_member_field(program, receiver_type, member);
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
}

/// Select one field under the receiver's nominal declaration and, for a
/// payload, its exact case. A missing payload symbol may be recovered from
/// that declaration; a conflicting retained symbol cannot be replaced.
pub fn exact_data_member_field<'program>(
    program: &'program dyn crate::CheckingInput,
    data: &'program typed_trees::data::DataDefinition,
    member_symbol: SymbolHandle,
    member_name: &str,
    case_variant: Option<&str>,
) -> Option<&'program typed_trees::data::DataField> {
    if let Some(case_variant) = case_variant {
        let mut matches = program.data_members(data).iter().filter_map(|member| {
            let variant = member.as_variant()?;
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

    // An authored payload read can omit its case qualifier when ordinary
    // checking establishes membership. Recover its declaration, not that
    // membership: a retained selector must match exactly, and an unresolved
    // selector must identify one field under this receiver. Repeated payload
    // names cannot choose a case by declaration order.
    let mut selected = None;
    for member in program.data_members(data) {
        let fields = match member {
            typed_trees::data::DataMember::Field(field) => std::slice::from_ref(field),
            typed_trees::data::DataMember::Variant(variant) => program.data_payload_fields(variant),
        };
        for field in fields {
            if field.name.as_str() != member_name
                || (member_symbol.is_valid() && field.symbol != member_symbol)
                || !field.symbol.is_valid()
                || !field.type_reference.is_valid()
            {
                continue;
            }
            if selected.is_some() {
                return None;
            }
            selected = Some(field);
        }
    }
    selected
}
