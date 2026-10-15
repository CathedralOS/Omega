//! Place-rooted leaf types and copy/borrow custody for structural
//! establishment. A canonical place's borrowability and the leaf type a
//! projected path selects are decided here, ahead of value production.
//! Source use multiplicity is not stored-carrier custody: a reference array
//! can be reusable source syntax while its descriptors must transfer or retire.
//! Copy selection uses the same custody query as result publication.

use super::super::{ExpressionHandle, ExpressionNode, SymbolHandle, TypeReferenceNode};
use typed_trees::types::TypeReferenceHandle;

/// The declared type of a shared borrow's exact target without state scope: a
/// whole local's declared type, or a projected leaf resolved through its
/// root's declaration. `canonical_place_from_expression` keeps the root and
/// every segment exact.
pub(super) fn borrowed_place_leaf_type(
    program: &dyn ::validation::CheckingInput,
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
pub(super) fn projected_leaf_type(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    if !matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
    ) {
        return None;
    }
    let place = validation::canonical_place_from_expression(program, expression)?;
    if !place.has_only_positional_segments() {
        return None;
    }
    let root = match place.root {
        facts::PlaceRoot::Symbol(symbol) => symbol_declared_type(program, symbol)?,
        facts::PlaceRoot::Expression(expression) => {
            match program.expression_table.expression(expression) {
                ExpressionNode::Call(call) => validation::call_target_return_type(
                    program,
                    validation::call_target_symbol(program, call),
                )?,
                _ => return None,
            }
        }
        _ => return None,
    };
    validation::project_type_reference_from_segments(program, root, &place.segments)
}

/// The place shape a shared-borrow arm target must canonicalize to: a named
/// root reached through record fields and literal fixed indexes only. This is
/// the shape static type queries can resolve without an evaluation owner.
/// Computed roots and dynamic selectors instead retain their producers in
/// the structural value graph through the common reference-source capture.
pub(in crate::execution::scalar_computations) fn canonical_place_is_borrowable(
    place: &validation::CanonicalPlace,
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
pub(super) fn canonical_place_is_copyable(place: &validation::CanonicalPlace) -> bool {
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
    program: &dyn ::validation::CheckingInput,
    symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            if let Some(parameter) = program.state_parameter_by_symbol(state, symbol) {
                return Some(parameter.type_reference);
            }
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let Some(local) = statement.as_local_data()
                    && local.symbol == symbol
                {
                    return Some(local.type_reference);
                }
            }
        }
    }
    None
}

/// A copyable `Unrestricted` leaf of any structural shape, payload-free case
/// sums included: a named `data` under `Unrestricted` multiplicity. Copying
/// its contents into a fresh owned place reproduces the read a
/// `self.scan_compare_type`-shape member access needs without moving borrowed
/// storage; anything affine still needs move semantics.
pub(super) fn copied_place_type(
    program: &dyn ::validation::CheckingInput,
    expected: TypeReferenceHandle,
) -> bool {
    // Copy the stored value, not its referent. A shared descriptor has its own
    // disposal occurrence but preserves the captured backing and immediate
    // parents; consuming the source would contradict shared multiplicity.
    // Exclusive descriptors still require an owned transfer.
    if let Some((_, access)) =
        validation::reference_result_custody::stored_loan_parts(program, expected)
    {
        return access == checked_trees::CheckedStructuralAccess::SharedBorrow;
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
    // Source reference uses are unrestricted, but stored descriptors own a
    // loan lifetime. Arrays inherit that custody recursively. Copy selection
    // must use the same carrier multiplicity as result publication, otherwise
    // projecting an exclusive descriptor from an array copies its permission.
    // The shared-descriptor observation above is the explicit exception.
    if validation::reference_result_custody::result_multiplicity(program, reference)
        != language_semantics::Multiplicity::Unrestricted
    {
        return false;
    }
    // Copying established array storage preserves its element qualifications;
    // unlike literal construction, it does not need an unqualified scalar
    // initializer route. Multiplicity above decides whether contents copy.
    if matches!(
        program.type_reference_table.type_reference(reference),
        TypeReferenceNode::FixedArray { .. }
    ) {
        return true;
    }
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        return false;
    };
    program.data_definition_by_symbol(*symbol).is_some()
}

/// The value presented by an authored place. A borrowed formal presents its
/// referent, while a local owns its stored descriptor. Do not apply referent
/// normalization to a descriptor copy or a view's captured collection extent.
pub(super) fn readable_place_type(
    program: &dyn ::validation::CheckingInput,
    state: SymbolHandle,
    statement_index: usize,
    place: &validation::CanonicalPlace,
    expected: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    let declared =
        validation::canonical_place_type_reference(program, state, statement_index, place)?;
    if validation::declared_carriers_match(program, declared, expected) {
        return Some(declared);
    }
    let state = program.state_by_symbol(state)?;
    let symbol = place.root.as_symbol()?;
    if !place.segments.is_empty() || program.state_parameter_by_symbol(state, symbol).is_none() {
        return None;
    }
    let shell = validation::reference_shell(program, declared)?;
    if shell.access == language_semantics::ReferenceAccess::WriteOnly {
        return None;
    }
    validation::declared_carriers_match(program, shell.referee, expected).then_some(shell.referee)
}

/// A member/index projection rooted at readable owned or borrowed storage
/// whose leaf is a copyable `Unrestricted` structural type.
/// It shares the scalar-case read's root/path contract, but retains contents
/// that cannot be reconstructed by case fan-out. A bare
/// whole-root name is the degenerate path: an owned root's `Unrestricted`
/// contents copy out whole, no loan involved.
/// Retain that snapshot at the operand's evaluation point; passing an earlier
/// constructor's result would confuse mutable storage with its initial value.
pub(in crate::execution::scalar_computations) fn needs_copied_place_root(
    program: &dyn ::validation::CheckingInput,
    state: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
) -> bool {
    if !copied_place_type(program, expected) {
        return false;
    }
    let Some(place) = validation::canonical_place_from_expression_in_state(
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
    let Some(symbol) = place.root.as_symbol() else {
        return false;
    };
    let Some(root) = symbol_declared_type(program, symbol) else {
        return false;
    };
    if !place.segments.is_empty()
        && validation::reference_shell(program, root)
            .is_some_and(|shell| shell.access == language_semantics::ReferenceAccess::WriteOnly)
    {
        // A readable owned root can supply its copyable child just as a
        // readable loan can. Neither consumes the root, even when it is
        // affine; a write-only loan still grants no observation authority.
        return false;
    }
    readable_place_type(program, state, statement_index, &place, expected)
        .is_some_and(|leaf| copied_place_type(program, leaf))
}

/// The terminal expression carrying a projection's storage: the whole local
/// name or anonymous producer. Selecting storage does not restrict how its
/// owner was produced; ordinary structural value planning admits that owner.
pub(super) fn projection_root_expression(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> ExpressionHandle {
    let mut cursor = expression;
    loop {
        match program.expression_table.expression(cursor) {
            ExpressionNode::Member(member) => cursor = member.receiver,
            ExpressionNode::Indexed(indexed) => cursor = indexed.collection,
            _ => return cursor,
        }
    }
}
