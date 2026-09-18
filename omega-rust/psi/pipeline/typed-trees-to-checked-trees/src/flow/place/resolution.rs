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
/// literal's type), or a half-open window taken from a collection. This
/// mirrors the typed-tree lowerer's `SubjectPosition`
/// and the checker-side partition replay: a `Box<Context>` leaf keeps its
/// `T` argument bound so a member whose declared type is `T` resumes the
/// walk at `Context` instead of stopping at an opaque leaf.
///
/// `Sliced` carries the window's element reference rather than the window's
/// own type: no slice type-reference node exists to mint for it, and the
/// distinction is load-bearing — an index hop resumes at the element while
/// a member demand resolves nothing, because a slice declares no fields.
///
/// Position equality is identity equality on the stored evidence — the same
/// reference handle, the same declaration, or the same element reference —
/// not type equality: two positions naming equal types through different
/// stored rows do not join, because neither is evidence for the other.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum MemberPosition {
    Reference(typed_trees::types::TypeReferenceHandle),
    Declaration(SymbolHandle),
    Sliced(typed_trees::types::TypeReferenceHandle),
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
        // A window's leaf is the slice, which owns no declaration identity:
        // only a further index hop names the element a member could live on.
        MemberPosition::Sliced(_) => SymbolHandle::invalid(),
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
        // An atomic expression's value is its operand's evaluation (the
        // checked interpreter lowers the node the same way), so the position
        // is the operand's position rather than an opaque leaf.
        ExpressionNode::Atomic(atomic) => expression_type_position(program, atomic.value),
        ExpressionNode::Call(call) => {
            super::super::calls::call_target_return_type(program, call.target_symbol)
                .map(MemberPosition::Reference)
        }
        ExpressionNode::Name(path) => {
            let symbol = first_valid_name_path_symbol(path, &program.expression_table)?;
            symbol_type_position(program, symbol)
        }
        ExpressionNode::Indexed(indexed) => {
            // The index hop lands on an element, not on the collection itself:
            // replay the same segment canonicalization pushes for this
            // expression so `values[i].item` resumes at `Box<Context>` with
            // its generic argument still bound. A collection whose position
            // does not project to an element keeps no position rather than
            // minting the collection's own for the element.
            let segment = super::index_place_segment(program, indexed.index);
            match expression_type_position(program, indexed.collection) {
                Some(MemberPosition::Reference(reference)) => match segment {
                    // A range hop yields a window over the same element —
                    // `values[0..2]` is a slice of `Box<Context>`, not one
                    // element — so the position becomes the window and only
                    // a further index hop resumes at the element.
                    facts::PlaceSegment::FixedRange { .. } => {
                        super::super::collection_element_type_reference(program, reference)
                            .map(MemberPosition::Sliced)
                    }
                    _ => super::super::project_type_reference_from_segments(
                        program,
                        reference,
                        &[segment],
                    )
                    .map(MemberPosition::Reference),
                },
                Some(MemberPosition::Sliced(element)) => match segment {
                    // Re-windowing a window keeps the same element; an index
                    // hop names one element and resumes at its position.
                    facts::PlaceSegment::FixedRange { .. } => Some(MemberPosition::Sliced(element)),
                    _ => Some(MemberPosition::Reference(element)),
                },
                position => position,
            }
        }
        ExpressionNode::Member(member) => {
            let symbol = effective_member_symbol(program, member.receiver, member);
            member_type_position(program, member.receiver, symbol)
        }
        ExpressionNode::StructLiteral(literal) => literal
            .type_symbol
            .is_valid()
            .then_some(MemberPosition::Declaration(literal.type_symbol)),
        // A cast stores its complete normalized result qualification on the
        // node, so the leaf's position is that reference outright. An
        // unresolved stored result (zero means unnormalized) keeps none.
        ExpressionNode::Cast(cast) => cast
            .result_type
            .is_valid()
            .then_some(MemberPosition::Reference(cast.result_type)),
        // The proof-only zero value of `T` is a `T`; the node stores the
        // exact reference, so its position is that reference outright.
        ExpressionNode::ZeroValue(reference) => reference
            .is_valid()
            .then_some(MemberPosition::Reference(*reference)),
        ExpressionNode::ArrayLiteral(elements) => array_literal_position(
            program,
            program.expression_table.expression_handles(*elements),
        ),
        ExpressionNode::Match(dispatch) => match_result_position(program, dispatch),
        // The remaining leaves produce scalar values — literals, unary and
        // binary results, range operands — whose types own no declaration
        // identity to resume at, so they keep no position rather than
        // borrowing a same-shaped row.
        _ => None,
    }
}

/// An array literal is a fixed collection over one element type: the
/// position is `Sliced` over that element — an index hop resumes at the
/// element, a range keeps the same window, and a member demand resolves
/// nothing because an array declares no fields. Every element must agree on
/// one exact `Reference` position; an opaque element or a different stored
/// handle keeps the literal unproven rather than naming one element's type
/// for the whole collection. Elements whose positions are not references —
/// a declaration leaf, another window — carry no element handle the window
/// could reuse, so the literal keeps no position there either.
fn array_literal_position(
    program: &typed_trees::TypedTrees,
    elements: &[ExpressionHandle],
) -> Option<MemberPosition> {
    let mut positions = elements
        .iter()
        .map(|element| expression_type_position(program, *element));
    let Some(Some(first)) = positions.next() else {
        return None;
    };
    let MemberPosition::Reference(element) = first else {
        return None;
    };
    positions
        .all(|position| position == Some(first))
        .then_some(MemberPosition::Sliced(element))
}

/// A match's value is whichever arm produces it, so every arm must agree on
/// one exact position — the same stored reference, declaration, or window
/// element. An opaque arm or a different stored row keeps the dispatch
/// unproven rather than letting one arm's leaf stand in for the others';
/// an arm spelled through a distinct handle to an equal type is that same
/// disagreement, not a join.
fn match_result_position(
    program: &typed_trees::TypedTrees,
    dispatch: &typed_trees::expression::TableMatchExpression,
) -> Option<MemberPosition> {
    let mut positions = program
        .expression_table
        .match_arms(dispatch.arms)
        .iter()
        .map(|arm| expression_type_position(program, arm.value));
    let Some(Some(first)) = positions.next() else {
        return None;
    };
    positions
        .all(|position| position == Some(first))
        .then_some(first)
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

/// A case-qualified member hop resolves inside the selected variant only:
/// payload spellings repeat across cases, so the demanded field is the one
/// the named case declares — never the first same-named field a sibling
/// variant carries. This is the place-route counterpart of the selected-case
/// lookup `effective_member_symbol` performs when the receiver's type is
/// known; a variant that is absent or declares no such payload field yields
/// no symbol rather than borrowing a same-shaped row.
pub(super) fn resolve_case_member_symbol_from_type_symbol(
    program: &typed_trees::TypedTrees,
    type_symbol: SymbolHandle,
    case_name: &str,
    member_name: &str,
) -> Option<SymbolHandle> {
    let declaration = program
        .data_definitions()
        .iter()
        .find(|row| row.symbol == type_symbol)?;
    let variant = program
        .data_members(declaration)
        .iter()
        .find_map(|row| match row {
            typed_trees::data::DataMember::Variant(variant)
                if variant.name.as_str() == case_name =>
            {
                Some(variant)
            }
            _ => None,
        })?;
    program
        .data_payload_fields(variant)
        .iter()
        .find(|field| field.name.as_str() == member_name)
        .map(|field| field.symbol)
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
