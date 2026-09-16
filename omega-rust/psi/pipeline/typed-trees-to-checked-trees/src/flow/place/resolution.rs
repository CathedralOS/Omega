//! Resolve expression projections to declaration identities used by canonical places.
//! Case payload spelling is not unique within a data declaration. Reconstruct
//! qualified fields inside the selected case so borrow and Terminal consumers
//! see the same Case/Field path; never repair a conflicting retained identity.

use crate::lookup::{first_valid_name_path_symbol, machine_by_symbol};
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use symbols::SymbolHandle;

#[cfg(test)]
mod tests;

/// Where a demanded member path currently points: a stored type reference
/// that still retains the reaching generic application's arguments, or a
/// declaration reached without one (an attached `self` datum, a struct
/// literal's type). This mirrors the typed-tree lowerer's `SubjectPosition`
/// and the checker-side partition replay: a `Box<Context>` leaf keeps its
/// `T` argument bound so a member whose declared type is `T` resumes the
/// walk at `Context` instead of stopping at an opaque leaf.
#[derive(Clone, Copy)]
pub(super) enum MemberPosition {
    Reference(typed_trees::types::TypeReferenceHandle),
    Declaration(SymbolHandle),
}

/// The declaration symbol a position's leaf names. Substituted arguments are
/// already folded into a `Reference` leaf by
/// `super::super::project_type_reference_from_segments`, so this is the same
/// terminal `type_symbol` mapping callers have always observed.
pub(super) fn position_leaf_symbol(
    program: &typed_trees::TypedTrees,
    position: MemberPosition,
) -> SymbolHandle {
    match position {
        MemberPosition::Reference(reference) => program.type_reference_table.type_symbol(reference),
        MemberPosition::Declaration(symbol) => symbol,
    }
}

/// One member hop through a position: the member's declared type reference
/// with the receiver's reaching generic application still bound, so
/// `Box<Context>::item` resumes at `Context`. Falls back to the member's own
/// declared position when the receiver does not replay (a declaration
/// position, an opaque leaf, or a member outside the replayed declaration) —
/// the exact contract the bare-symbol walk had.
fn member_type_position(
    program: &typed_trees::TypedTrees,
    receiver: ExpressionHandle,
    member_symbol: SymbolHandle,
) -> Option<MemberPosition> {
    if let Some(MemberPosition::Reference(reference)) = expression_type_position(program, receiver)
        && let Some(projected) = super::super::project_type_reference_from_segments(
            program,
            reference,
            &[facts::PlaceSegment::Field {
                symbol: member_symbol,
            }],
        )
    {
        return Some(MemberPosition::Reference(projected));
    }
    symbol_type_position(program, member_symbol)
}

/// The position a resolved symbol's declared type points at. Unlike
/// `symbol_type_symbol`, this retains the type reference so a generic
/// application's arguments remain bound for the next member hop.
pub(super) fn symbol_type_position(
    program: &typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<MemberPosition> {
    if !symbol.is_valid() {
        return None;
    }

    for machine in program.machines() {
        // Bare attached fields retain an inherited machine symbol. Resolve its
        // exact declaration type before walking child fields; missing child
        // identities must not make overlapping receiver loans appear disjoint.
        if program.symbols.get(symbol).parent == machine.symbol
            && let Some(field) = validation::exact_attached_field(
                program,
                machine,
                symbol,
                program.symbols.name(symbol),
            )
        {
            return Some(MemberPosition::Reference(field.type_reference));
        }
        if machine.symbol == symbol {
            // The retained attached application keeps the owner's generic
            // arguments bound; a nongeneric attachment names its data
            // declaration directly.
            if machine.attached_data_application.is_valid() {
                return Some(MemberPosition::Reference(machine.attached_data_application));
            }
            if let Some(attached_data) = machine.attached_data.as_deref()
                && let Some(data) = program
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.name.as_str() == attached_data)
            {
                return Some(MemberPosition::Declaration(data.symbol));
            }
        }
        for state in program.machine_states(machine) {
            for parameter in program.state_parameters(state) {
                if parameter.symbol == symbol {
                    return Some(MemberPosition::Reference(parameter.type_reference));
                }
            }
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let typed_trees::statement::StatementNode::LocalData(local_data) = statement
                    && local_data.symbol == symbol
                {
                    return Some(MemberPosition::Reference(local_data.type_reference));
                }
            }
        }
        for owned in program.machine_owned_data(machine) {
            if owned.symbol == symbol {
                return Some(MemberPosition::Reference(owned.type_reference));
            }
        }
    }

    // Trait requirements are checked state signatures rather than executable
    // machine states. Their parameter symbols still own exact declared types
    // and must participate in member-place resolution for requirement
    // contracts.
    for definition in program.traits() {
        for signature in program.trait_machine_signatures(definition) {
            for parameter in program.state_signature_parameters(signature) {
                if parameter.symbol == symbol {
                    return Some(MemberPosition::Reference(parameter.type_reference));
                }
            }
        }
    }

    // Operator contracts are declaration-owned proof expressions rather than
    // executable state bodies. Their parameters nevertheless have ordinary
    // typed member-place semantics and must resolve through their exact
    // operator declaration.
    for operator in program.operators().iter().chain(
        program
            .domain_definitions()
            .iter()
            .flat_map(|domain| program.domain_operators(domain)),
    ) {
        for parameter in program.operator_parameters(operator) {
            if parameter.symbol == symbol {
                return Some(MemberPosition::Reference(parameter.type_reference));
            }
        }
    }

    for data in program.data_definitions() {
        for member in program.data_members(data) {
            match member {
                typed_trees::data::DataMember::Field(field) if field.symbol == symbol => {
                    return Some(MemberPosition::Reference(field.type_reference));
                }
                typed_trees::data::DataMember::Variant(variant) => {
                    if let Some(field) = program
                        .data_payload_fields(variant)
                        .iter()
                        .find(|field| field.symbol == symbol)
                    {
                        return Some(MemberPosition::Reference(field.type_reference));
                    }
                }
                _ => {}
            }
        }
    }

    None
}

/// The position an expression's type points at, keeping generic application
/// arguments bound through member hops so `b.item` under `b: &Box<Context>`
/// stands on `Context`, not on `Box`'s unbound `T`.
pub(super) fn expression_type_position(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<MemberPosition> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => expression_type_position(program, inner.target),
        ExpressionNode::Call(call) => {
            super::super::calls::call_target_return_type(program, call.target_symbol)
                .map(MemberPosition::Reference)
        }
        ExpressionNode::Name(path) => {
            let symbol = first_valid_name_path_symbol(path, &program.expression_table)?;
            symbol_type_position(program, symbol)
        }
        ExpressionNode::Indexed(indexed) => expression_type_position(program, indexed.collection),
        ExpressionNode::Member(member) => {
            let symbol = effective_member_symbol(program, member.receiver, member);
            member_type_position(program, member.receiver, symbol)
        }
        ExpressionNode::StructLiteral(literal) => literal
            .type_symbol
            .is_valid()
            .then_some(MemberPosition::Declaration(literal.type_symbol)),
        _ => None,
    }
}

pub(crate) fn effective_member_symbol(
    program: &typed_trees::TypedTrees,
    receiver: ExpressionHandle,
    member: &typed_trees::expression::TableMemberExpression,
) -> SymbolHandle {
    // A case-qualified projection must not select the first same-named field
    // in another case. Preserve the selected variant when reconstructing its
    // canonical place; an inconsistent retained symbol is not repairable here.
    if let Some(case_name) = &member.case_variant {
        let selected = expression_type_symbol(program, receiver).and_then(|type_symbol| {
            let declaration = program
                .data_definitions()
                .iter()
                .find(|row| row.symbol == type_symbol)?;
            let variant = program
                .data_members(declaration)
                .iter()
                .find_map(|row| match row {
                    typed_trees::data::DataMember::Variant(variant)
                        if variant.name == *case_name =>
                    {
                        Some(variant)
                    }
                    _ => None,
                })?;
            program
                .data_payload_fields(variant)
                .iter()
                .find(|field| field.name == member.member)
                .map(|field| field.symbol)
        });
        return selected
            .filter(|symbol| !member.member_symbol.is_valid() || member.member_symbol == *symbol)
            .unwrap_or_else(SymbolHandle::invalid);
    }
    if let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(receiver)
        && let Some(field) = program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .find(|field| field.name == member.member)
        && field.field_symbol.is_valid()
    {
        return field.field_symbol;
    }

    if let Some(symbol) =
        resolve_member_symbol_from_receiver(program, receiver, member.member.as_str())
    {
        return symbol;
    }

    if member.member_symbol.is_valid() {
        return member.member_symbol;
    }

    SymbolHandle::invalid()
}

pub(crate) fn resolve_member_symbol_from_type_symbol(
    program: &typed_trees::TypedTrees,
    type_symbol: SymbolHandle,
    member_name: &str,
) -> Option<SymbolHandle> {
    if let Some(data) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == type_symbol)
    {
        for member in program.data_members(data) {
            match member {
                typed_trees::data::DataMember::Field(field)
                    if field.name.as_str() == member_name =>
                {
                    return Some(field.symbol);
                }
                typed_trees::data::DataMember::Variant(variant)
                    if variant.name.as_str() == member_name =>
                {
                    return Some(variant.symbol);
                }
                typed_trees::data::DataMember::Variant(variant) => {
                    if let Some(field) = program
                        .data_payload_fields(variant)
                        .iter()
                        .find(|field| field.name.as_str() == member_name)
                    {
                        return Some(field.symbol);
                    }
                }
                _ => {}
            }
        }
    }

    if let Some(machine) = machine_by_symbol(program, type_symbol) {
        if let Some(attached_data) = machine.attached_data.as_deref()
            && let Some(data) = program
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == attached_data)
        {
            for member in program.data_members(data) {
                match member {
                    typed_trees::data::DataMember::Field(field)
                        if field.name.as_str() == member_name =>
                    {
                        return Some(field.symbol);
                    }
                    typed_trees::data::DataMember::Variant(variant)
                        if variant.name.as_str() == member_name =>
                    {
                        return Some(variant.symbol);
                    }
                    typed_trees::data::DataMember::Variant(variant) => {
                        if let Some(field) = program
                            .data_payload_fields(variant)
                            .iter()
                            .find(|field| field.name.as_str() == member_name)
                        {
                            return Some(field.symbol);
                        }
                    }
                    _ => {}
                }
            }
        }
        for owned in program.machine_owned_data(machine) {
            if owned.name.as_str() == member_name {
                return Some(owned.symbol);
            }
        }
    }

    None
}

fn resolve_member_symbol_from_receiver(
    program: &typed_trees::TypedTrees,
    receiver: ExpressionHandle,
    member_name: &str,
) -> Option<SymbolHandle> {
    let type_symbol = expression_type_symbol(program, receiver)?;
    resolve_member_symbol_from_type_symbol(program, type_symbol, member_name)
}

pub(crate) fn expression_type_symbol(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    expression_type_position(program, expression)
        .map(|position| position_leaf_symbol(program, position))
}

pub(crate) fn symbol_type_symbol(
    program: &typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<SymbolHandle> {
    symbol_type_position(program, symbol).map(|position| position_leaf_symbol(program, position))
}
