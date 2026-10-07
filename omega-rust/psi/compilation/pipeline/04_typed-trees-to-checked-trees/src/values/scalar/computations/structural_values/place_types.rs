//! Place-rooted leaf types and copy/borrow custody for structural
//! establishment. A canonical place's borrowability and the leaf type a
//! projected path selects are decided here, ahead of value production.

use super::super::{
    ExpressionHandle, ExpressionNode, StatementNode, SymbolHandle, TypeReferenceNode, TypedTrees,
};
use typed_trees::types::TypeReferenceHandle;

/// The declared type of a shared borrow's exact target without state scope: a
/// whole local's declared type, or a projected leaf resolved through its
/// root's declaration. `canonical_place_from_expression` keeps the root and
/// every segment exact.
pub(super) fn borrowed_place_leaf_type(
    program: &TypedTrees,
    target: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let leaf = match program.expression_table.expression(target) {
        ExpressionNode::Name(path)
            if path.symbol.is_valid()
                && path.head_symbol == path.symbol
                && path.members.count() == 1 =>
        {
            symbol_declared_type(program, path.symbol)
        }
        ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
            projected_leaf_type(program, target)
        }
        _ => None,
    }?;
    // Borrowing a stored descriptor captures its referent with the requested
    // access; it does not borrow the descriptor's own storage as an &&T.
    match program.type_reference_table.type_reference(leaf) {
        TypeReferenceNode::Reference {
            referee, access, ..
        } if *access != language_semantics::ReferenceAccess::WriteOnly => Some(*referee),
        TypeReferenceNode::Reference { .. } => None,
        _ => Some(leaf),
    }
}

/// The declared type of a field/fixed-index projection leaf, resolved through
/// the authored root's own declaration: a whole local's declared type, or a
/// producing call's declared return type. Only exact field and literal-index
/// segments keep a statically checkable identity.

/// The declared type of a field/fixed-index projection leaf, resolved through
/// the authored root's own declaration: a whole local's declared type, or a
/// producing call's declared return type. Only exact field and literal-index
/// segments keep a statically checkable identity.
pub(super) fn projected_leaf_type(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    if !matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
    ) {
        return None;
    }
    let place = crate::flow::canonical_place_from_expression(program, expression)?;
    if place.segments.is_empty()
        || !place.segments.iter().all(|segment| {
            matches!(
                segment,
                facts::PlaceSegment::Field { .. } | facts::PlaceSegment::FixedIndex { .. }
            )
        })
    {
        return None;
    }
    let root = match place.root {
        facts::PlaceRoot::Symbol(symbol) => symbol_declared_type(program, symbol)?,
        facts::PlaceRoot::Expression(expression) => {
            match program.expression_table.expression(expression) {
                ExpressionNode::Call(call) => crate::flow::call_target_return_type(
                    program,
                    crate::flow::call_target_symbol(program, call),
                )?,
                _ => return None,
            }
        }
        _ => return None,
    };
    crate::flow::project_type_reference_from_segments(program, root, &place.segments)
}

/// The place shape a shared-borrow arm target must canonicalize to: a named
/// root reached through record fields and literal fixed indexes only. That is
/// exactly the carrier `borrowed_place` can rebuild into a `SharedBorrow`
/// source -- a computed root cannot be re-derived at replay, and a dynamic
/// index or range segment has no statically checkable ordinal.

/// The place shape a shared-borrow arm target must canonicalize to: a named
/// root reached through record fields and literal fixed indexes only. That is
/// exactly the carrier `borrowed_place` can rebuild into a `SharedBorrow`
/// source -- a computed root cannot be re-derived at replay, and a dynamic
/// index or range segment has no statically checkable ordinal.
pub(in crate::values::scalar::computations) fn canonical_place_is_borrowable(
    place: &crate::flow::CanonicalPlace,
) -> bool {
    matches!(place.root, facts::PlaceRoot::Symbol(_))
        && place.segments.iter().all(|segment| {
            matches!(
                segment,
                facts::PlaceSegment::Field { .. } | facts::PlaceSegment::FixedIndex { .. }
            )
        })
}

/// `canonical_place_is_borrowable` widened with a runtime `Index` segment
/// for the copied-leaf lane: `self.pointer_scan_nodes[position]` has no
/// static ordinal, but the argument-path mint records the selector as a
/// `RuntimeIndex` coordinate the terminal verifier re-proves against the
/// enclosing array's extent. The `&place` borrow and scalar-case lanes keep
/// the static shape — a loan or a case read replays the place, not a runtime
/// element.

/// `canonical_place_is_borrowable` widened with a runtime `Index` segment
/// for the copied-leaf lane: `self.pointer_scan_nodes[position]` has no
/// static ordinal, but the argument-path mint records the selector as a
/// `RuntimeIndex` coordinate the terminal verifier re-proves against the
/// enclosing array's extent. The `&place` borrow and scalar-case lanes keep
/// the static shape — a loan or a case read replays the place, not a runtime
/// element.
pub(super) fn canonical_place_is_copyable(place: &crate::flow::CanonicalPlace) -> bool {
    matches!(place.root, facts::PlaceRoot::Symbol(_))
        && place.segments.iter().all(|segment| {
            matches!(
                segment,
                facts::PlaceSegment::Field { .. }
                    | facts::PlaceSegment::FixedIndex { .. }
                    | facts::PlaceSegment::Index { .. }
            )
        })
}

pub(super) fn symbol_declared_type(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            if let Some(parameter) = program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.symbol == symbol)
            {
                return Some(parameter.type_reference);
            }
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let StatementNode::LocalData(local) = statement
                    && local.symbol == symbol
                {
                    return Some(local.type_reference);
                }
            }
        }
    }
    None
}

/// A copyable payloadless case-sum leaf: a named `data` whose members are all
/// variants carrying no payload fields, under `Unrestricted` multiplicity.
/// Membership observation can reconstruct such a leaf without reading any
/// storage contents, which is exactly what a `self.bitness`-shape read needs;
/// anything with payload storage or affine custody needs move semantics.

/// A copyable payloadless case-sum leaf: a named `data` whose members are all
/// variants carrying no payload fields, under `Unrestricted` multiplicity.
/// Membership observation can reconstruct such a leaf without reading any
/// storage contents, which is exactly what a `self.bitness`-shape read needs;
/// anything with payload storage or affine custody needs move semantics.
pub(super) fn scalar_case_place_type(program: &TypedTrees, expected: TypeReferenceHandle) -> bool {
    let Some(reference) = validation::unwrapped_type_reference(program, expected) else {
        return false;
    };
    if program.type_multiplicity(reference) != language_semantics::Multiplicity::Unrestricted {
        return false;
    }
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        return false;
    };
    let Some(data) = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == *symbol)
    else {
        return false;
    };
    let members = program.data_members(data);
    !members.is_empty()
        && members.iter().all(|member| match member {
            typed_trees::data::DataMember::Variant(variant) => {
                program.data_payload_fields(variant).is_empty()
            }
            _ => false,
        })
}

/// `self.bitness`-shape admission: a member/fixed-index projection rooted at
/// shared-borrowed storage whose leaf is a copyable payloadless case sum. The
/// value keeps the exact canonical path so lowering can observe the held case
/// and re-establish it -- never moving borrowed storage.

/// `self.bitness`-shape admission: a member/fixed-index projection rooted at
/// shared-borrowed storage whose leaf is a copyable payloadless case sum. The
/// value keeps the exact canonical path so lowering can observe the held case
/// and re-establish it -- never moving borrowed storage.
pub(in crate::values::scalar::computations) fn is_scalar_case_place_value(
    program: &TypedTrees,
    state: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
) -> bool {
    if !scalar_case_place_type(program, expected) {
        return false;
    }
    let Some(place) = crate::flow::canonical_place_from_expression_in_state(
        program,
        state,
        statement_index,
        expression,
    ) else {
        return false;
    };
    if place.segments.is_empty() || !canonical_place_is_borrowable(&place) {
        return false;
    }
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return false;
    };
    let Some(root) = symbol_declared_type(program, symbol) else {
        return false;
    };
    if !matches!(
        program.type_reference_table.type_reference(root),
        TypeReferenceNode::Reference {
            access: language_semantics::ReferenceAccess::Shared,
            ..
        }
    ) {
        return false;
    }
    crate::flow::canonical_place_type_reference(program, state, statement_index, &place)
        .is_some_and(|leaf| {
            program.normalized_type_identity(leaf) == program.normalized_type_identity(expected)
        })
}

/// A copyable `Unrestricted` leaf of any structural shape: a named `data`
/// under `Unrestricted` multiplicity. Copying its contents into a fresh
/// owned place reproduces the read a `self.scan_compare_type`-shape member
/// access needs where moving or case-fan-out cannot apply; anything affine
/// still needs move semantics.

/// A copyable `Unrestricted` leaf of any structural shape: a named `data`
/// under `Unrestricted` multiplicity. Copying its contents into a fresh
/// owned place reproduces the read a `self.scan_compare_type`-shape member
/// access needs where moving or case-fan-out cannot apply; anything affine
/// still needs move semantics.
pub(super) fn copied_place_type(program: &TypedTrees, expected: TypeReferenceHandle) -> bool {
    // Stored loan descriptors carry affine custody even when source-level
    // shared access is unrestricted. Transfer their existing Place/Projection;
    // copying the descriptor as a referent would mint a different loan.
    if validation::reference_result_custody::stored_loan_parts(program, expected).is_some() {
        return false;
    }
    // A stored `&'a [T]` view leaf copies the view value out of the carrier
    // the same way an `Unrestricted` named leaf does — the referent's loan
    // stays exactly where the field's `&` put it. Its bare `Slice` rung
    // unwraps to `Affine`, so the shared-borrow view's own multiplicity is
    // what decides admission.
    if crate::execution::terminal_unit::types::borrowed_slice_view(program, expected)
        && program.type_multiplicity(expected) == language_semantics::Multiplicity::Unrestricted
    {
        return true;
    }
    let Some(reference) = validation::unwrapped_type_reference(program, expected) else {
        return false;
    };
    if program.type_multiplicity(reference) != language_semantics::Multiplicity::Unrestricted {
        return false;
    }
    // A fixed primitive array is a copyable structural value too. Materialize
    // the existing copied-place operation before an owned call argument so
    // reading a field through `&self`/`&mut self` never moves it out of the
    // borrowed receiver. The destination keeps the exact array type/extent.
    if validation::is_closed_primitive_array_type(program, reference) {
        return true;
    }
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        return false;
    };
    program
        .data_definitions()
        .iter()
        .any(|data| data.symbol == *symbol)
}

/// A member/index projection rooted at readable owned or borrowed storage
/// whose leaf is a copyable `Unrestricted` named type or fixed primitive array.
/// It shares the scalar-case read's root/path contract, but retains contents
/// that cannot be reconstructed by case fan-out. A bare
/// whole-root name is the degenerate path: an owned root's `Unrestricted`
/// contents copy out whole, no loan involved.
/// Retain that snapshot at the operand's evaluation point; passing an earlier
/// constructor's result would confuse mutable storage with its initial value.

/// A member/index projection rooted at readable owned or borrowed storage
/// whose leaf is a copyable `Unrestricted` named type or fixed primitive array.
/// It shares the scalar-case read's root/path contract, but retains contents
/// that cannot be reconstructed by case fan-out. A bare
/// whole-root name is the degenerate path: an owned root's `Unrestricted`
/// contents copy out whole, no loan involved.
/// Retain that snapshot at the operand's evaluation point; passing an earlier
/// constructor's result would confuse mutable storage with its initial value.
pub(in crate::values::scalar::computations) fn needs_copied_place_root(
    program: &TypedTrees,
    state: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
) -> bool {
    if !copied_place_type(program, expected)
        || is_scalar_case_place_value(program, state, statement_index, expression, expected)
    {
        return false;
    }
    let Some(place) = crate::flow::canonical_place_from_expression_in_state(
        program,
        state,
        statement_index,
        expression,
    ) else {
        return false;
    };
    if !canonical_place_is_copyable(&place) {
        return false;
    }
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return false;
    };
    let Some(root) = symbol_declared_type(program, symbol) else {
        return false;
    };
    if place.segments.is_empty() {
        // A whole owned root reads its declared contents directly: any
        // non-view reference carrier stays on the borrowed-leaf lane below,
        // while an owned `[copy]` root copies into a fresh result with an
        // empty path. A bare `&`-view name is the degenerate copied leaf —
        // the view itself is the `Unrestricted` leaf the copy transfers.
        let root_is_view =
            matches!(
                program.type_reference_table.type_reference(root),
                TypeReferenceNode::Reference {
                    access: language_semantics::ReferenceAccess::Shared,
                    ..
                }
            ) && (crate::execution::terminal_unit::types::borrowed_slice_view(program, root)
                || crate::execution::terminal_unit::types::borrowed_named_view(program, root));
        if matches!(
            program.type_reference_table.type_reference(root),
            TypeReferenceNode::Reference { .. }
        ) && !root_is_view
        {
            return false;
        }
    } else if let TypeReferenceNode::Reference { access, .. } =
        program.type_reference_table.type_reference(root)
        && !matches!(
            access,
            language_semantics::ReferenceAccess::Shared
                | language_semantics::ReferenceAccess::Mutable
        )
    {
        // A readable owned root can supply its copyable child just as a
        // readable loan can. Neither consumes the root, even when it is
        // affine; a write-only loan still grants no observation authority.
        return false;
    }
    crate::flow::canonical_place_type_reference(program, state, statement_index, &place)
        .is_some_and(|leaf| validation::declared_carriers_match(program, leaf, expected))
}

/// An omitted runtime field's zero value is admitted only for a
/// `[scalar; N]` leaf: a literal-length fixed array whose element resolves
/// to one plain integer primitive. Anything else — borrowed carriers,
/// bounded integers, nested composites — has no authored zero spelling to
/// synthesize, so the field must stay authored.

/// The terminal expression carrying a projection's storage: the whole local
/// name for an owned place source, or the producing call for a structural
/// product source.
pub(super) fn projection_root_expression(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let mut cursor = expression;
    loop {
        match program.expression_table.expression(cursor) {
            ExpressionNode::Member(member) => cursor = member.receiver,
            ExpressionNode::Indexed(indexed) => cursor = indexed.collection,
            ExpressionNode::Name(_) | ExpressionNode::Call(_) => return Some(cursor),
            _ => return None,
        }
    }
}
