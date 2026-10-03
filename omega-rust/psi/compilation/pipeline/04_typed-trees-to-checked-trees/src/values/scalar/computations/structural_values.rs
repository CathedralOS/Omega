//! Fresh structural establishment shares the scalar operand evaluation owner.
//! Reference leaves are classified before referent-oriented normalization:
//! their value is borrowed-storage custody, never a scalar pointer snapshot.
//! Record and case literals share the structural-construction contents rule:
//! exact interval-qualified scalar members use the existing bounded carrier
//! and per-member establishment proof, not a constructor-specific exception.
//! Closed primitive arrays select their scalar-array producer at the value
//! boundary, independent of whether a record, case, or other value consumes it.
//! The array occurrence owns its row-major leaf roots and independent replay.
//! Copy permission belongs to the selected leaf, not its root: a readable
//! owned or borrowed root may supply an unrestricted child without moving it.
//! Exact source/path replay and ordinary liveness remain downstream checks.
use super::{
    Builder, CheckedScalarComputationRoot, CheckedScalarExpressionPlans,
    CheckedScalarExpressionRole, ExpressionHandle, ExpressionNode, PrimitiveType, StatementNode,
    SymbolHandle, TypeReferenceNode, TypedTrees,
};
use checked_trees::{
    CheckedStructuralDispatchArm, CheckedStructuralValue, CheckedStructuralValueHandle,
    CheckedStructuralValueKind, CheckedStructuralValuePlans,
};
use typed_trees::expression::MatchPattern;
use typed_trees::types::TypeReferenceHandle;

/// Lifetime arguments erase without specializing the record's field types.
/// Construction and whole-place moves must agree on that runtime carrier;
/// type arguments still require substitution and remain outside this lookup.
fn closed_data_symbol(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } if program
            .type_reference_table
            .type_reference_handles(*arguments)
            .is_empty() =>
        {
            Some(*base_symbol)
        }
        _ => None,
    }
}

// Classify the destination before inserting scalar operand roots. A scalar or
// array Match must not leave partial structural plans when a later arm fails.
// A match arm may also select an owned child projected by exact field or
// fixed-index path; its structural node is a Projection over the root place,
// which the owned-selection transfer evidence then checks path-for-path.
/// A case literal carrying non-scalar payload registers its construction root
/// the same way a record literal does: each authored field keeps its own
/// scalar or nested structural value under the selected case.
pub(super) fn is_structural_case_value(
    program: &TypedTrees,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
) -> bool {
    if !matches!(
        program.expression_table.expression(expression),
        ExpressionNode::StructLiteral(_)
    ) {
        return false;
    }
    let Some(constructor) = validation::structural_case_constructor(program, expression, expected)
    else {
        return false;
    };
    let Some(reference) = validation::unwrapped_type_reference(program, expected) else {
        return false;
    };
    program.normalized_type_identity(constructor.type_reference)
        == program.normalized_type_identity(reference)
}

pub(super) fn is_record_value(
    program: &TypedTrees,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
) -> bool {
    if !matches!(
        program.expression_table.expression(expression),
        ExpressionNode::StructLiteral(_) | ExpressionNode::Match(_)
    ) {
        return false;
    }
    let shared_referent = shared_record_reference(program, expected);
    let Some(reference) = validation::unwrapped_type_reference(program, expected) else {
        return false;
    };
    // A `&T` selection result plans its borrowed arms against the referent
    // record; owned results keep their own carrier.
    let reference = shared_referent.unwrap_or(reference);
    // A primitive referent declares no record to resolve, so there is no field
    // roster to check and no constructor arm it could ever admit: its only
    // admitted arm is the shared borrow of an exact place checked below. The
    // invalid record symbol makes that explicit rather than implied -- no
    // authored `StructLiteral` can carry it.
    let primitive_referent =
        shared_referent.is_some() && program.primitive_type_reference(reference).is_some();
    let mut record_symbol = SymbolHandle::invalid();
    if !primitive_referent {
        let Some(record) = program.data_definitions().iter().find(|record| {
            closed_data_symbol(program, reference) == Some(record.symbol)
                || validation::member_type_substitutions(program, record, reference).is_some()
        }) else {
            return false;
        };
        let symbol = record.symbol;
        if program
            .data_members(record)
            .iter()
            .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_)))
        {
            return false;
        }
        record_symbol = symbol;
    }
    let symbol = &record_symbol;
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        match program.expression_table.expression(expression) {
            ExpressionNode::StructLiteral(literal)
                if literal.case_symbol.is_none() && literal.type_symbol == *symbol => {}
            ExpressionNode::Name(_)
                if validation::affine_owned_value_source(program, expression, expected)
                    .is_some() => {}
            ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
                if shared_referent.is_none()
                    && projected_leaf_type(program, expression).is_some_and(|reference| {
                        program.normalized_type_identity(reference)
                            == program.normalized_type_identity(expected)
                            && program.type_multiplicity(reference)
                                == language_semantics::Multiplicity::Affine
                    }) => {}
            // A shared-borrow arm does not move its referent's owner: the
            // canonical place is rebuilt with exact state scope during value
            // planning and replayed again at lowering.
            ExpressionNode::Borrow(borrow)
                if shared_referent.is_some_and(|referent| {
                    borrow.access == language_semantics::ReferenceAccess::Shared
                        && crate::flow::canonical_place_from_expression(program, borrow.target)
                            .is_some_and(|place| canonical_place_is_borrowable(&place))
                        && borrowed_place_leaf_type(program, borrow.target).is_some_and(|leaf| {
                            program.normalized_type_identity(leaf)
                                == program.normalized_type_identity(referent)
                        })
                }) => {}
            ExpressionNode::Call(call) => {
                // A call's structural product is a fresh owned arm value: the
                // builder admits it under the same exact return-type and
                // plain-custody rule it checks when constructing the node.
                let Some(returned) = crate::flow::call_target_return_type(
                    program,
                    crate::flow::call_target_symbol(program, call),
                ) else {
                    return false;
                };
                if program.normalized_type_identity(returned)
                    != program.normalized_type_identity(expected)
                    || !(validation::has_linear_owned_contents(program, returned)
                        || validation::reference_result_custody::is_reference_record(
                            program, returned,
                        ))
                {
                    return false;
                }
            }
            ExpressionNode::Match(dispatch) => {
                let arms = program.expression_table.match_arms(dispatch.arms);
                if arms.is_empty() {
                    return false;
                }
                pending.extend(arms.iter().map(|arm| arm.value));
            }
            _ => return false,
        }
    }
    true
}

/// A `let view: &T = &place` establishment admits the same shared-borrow
/// carrier a selection arm does, at statement scope: the referent's owner is
/// never moved, and `borrowed_place` rebuilds the authored target's canonical
/// root and path into a `SharedBorrow` source. This is the driver's
/// pre-classification only -- `borrowed_place` re-checks every condition when
/// constructing the node, and it does no plan mutation before that admission,
/// so a rejected borrow still leaves no partial structural plan.
pub(super) fn is_shared_borrow_value(
    program: &TypedTrees,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
) -> bool {
    let ExpressionNode::Borrow(borrow) = program.expression_table.expression(expression) else {
        return false;
    };
    borrow.access == language_semantics::ReferenceAccess::Shared
        && shared_record_reference(program, expected).is_some_and(|referent| {
            crate::flow::canonical_place_from_expression(program, borrow.target)
                .is_some_and(|place| canonical_place_is_borrowable(&place))
                && borrowed_place_leaf_type(program, borrow.target).is_some_and(|leaf| {
                    program.normalized_type_identity(leaf)
                        == program.normalized_type_identity(referent)
                })
        })
}

/// A LocalData initializer whose declared type is a borrowed `&[T]` view. The
/// value itself is adjudicated by `slice_view_place`, which rejoins
/// the checked loan the view holds; this only decides that the destination is
/// a view at all, so the ordinary owned classifications keep every other local.
pub(super) fn is_borrowed_slice_view_value(
    program: &TypedTrees,
    expected: TypeReferenceHandle,
) -> bool {
    crate::execution::terminal_unit::types::borrowed_slice_view_element(program, expected, &[])
        .is_some()
}

/// A LocalData initializer copying one element of a borrowed view
/// (`let chosen: Entry = tail[index]`): a single-element index over a bare
/// name whose declared type is a view of exactly the copyable record the
/// local declares. The builder then admits the view root, the selector and
/// the element's scalar fields.
pub(super) fn is_view_element_copy(
    program: &TypedTrees,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
) -> bool {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        return false;
    };
    let ExpressionNode::Name(name) = program.expression_table.expression(indexed.collection) else {
        return false;
    };
    !matches!(
        program.expression_table.expression(indexed.index),
        ExpressionNode::Range(_)
    ) && copied_place_type(program, expected)
        && !crate::execution::terminal_unit::types::borrowed_slice_view(program, expected)
        && symbol_declared_type(program, name.symbol)
            .and_then(|declared| {
                crate::execution::terminal_unit::types::borrowed_slice_view_element(
                    program,
                    declared,
                    &[],
                )
            })
            .is_some_and(|element| {
                program.normalized_type_identity(element)
                    == program.normalized_type_identity(expected)
            })
}

/// Whether every domain-qualified shared view member (`&[u8] in Utf8`) of
/// the record `reference` is initialized by a quoted literal in `literal`.
/// Construction cannot establish such a member's predicates from another
/// view, so those records stay outside the constructible shapes; a quoted
/// member instead becomes a `ByteLiteralView` whose octets carry the
/// evidence, and which only a decomposing member store consumes.
fn qualified_views_are_quoted(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    literal: &typed_trees::expression::TableStructLiteral,
) -> bool {
    let Some(symbol) = closed_data_symbol(program, reference) else {
        return false;
    };
    let Some(data) = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == symbol)
    else {
        return false;
    };
    let authored = program.expression_table.struct_fields(literal.fields);
    program.data_members(data).iter().all(|member| {
        let typed_trees::data::DataMember::Field(field) = member else {
            return true;
        };
        if crate::facts::field_domain::domain_constraint_symbols(program, field.type_reference)
            .is_empty()
            || !matches!(
                crate::execution::terminal_unit::types::byte_sequence_carrier(
                    program,
                    field.type_reference,
                    &[],
                ),
                Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. })
            )
        {
            return true;
        }
        authored.iter().any(|initializer| {
            initializer.field_symbol == field.symbol
                && matches!(
                    program.expression_table.expression(initializer.value),
                    ExpressionNode::String(_)
                )
        })
    })
}

/// A view-copied record field's carrier: a bare primitive, or a primitive
/// under exactly one arithmetic policy. A policy changes operations, not the
/// stored bits, so the element read and the fresh record carry the same
/// scalar; a range or any other qualification needs its own evidence and
/// keeps the copy declined.
fn plain_or_policy_scalar_field(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { .. } => true,
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            matches!(
                program.type_reference_table.constraint_span(*constraints),
                Some([typed_trees::types::TypeConstraintNode::ArithmeticDomain(_)])
            ) && matches!(
                program.type_reference_table.type_reference(*base_type),
                TypeReferenceNode::Named { .. }
            )
        }
        _ => false,
    }
}

/// A LocalData initializer or assignment value whose declared type is a fixed
/// array of structural elements spelled as a literal. Primitive arrays keep
/// their scalar-element route; structural elements each register their own
/// value node, the same way record fields do inside a constructor.
pub(super) fn is_fixed_array_value(
    program: &TypedTrees,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
) -> bool {
    matches!(
        program.expression_table.expression(expression),
        ExpressionNode::ArrayLiteral(_)
    ) && validation::unwrapped_type_reference(program, expected).is_some_and(|reference| {
        matches!(
            program.type_reference_table.type_reference(reference),
            typed_trees::types::TypeReferenceNode::FixedArray { .. }
        )
    }) && !validation::is_closed_primitive_array_type(program, expected)
}

/// The referent of a shared-borrow result type (`&T` where `T` is a named
/// record, or a primitive, that the structural pipeline can carry). `None` for
/// owned results and for borrows the structural pipeline cannot carry. The
/// carrier rule is linear-tolerant: a shared borrow observes the referent
/// without moving it, so a linear declaration's claim never reaches this join
/// and the referent's own owner keeps the whole discharge obligation.
///
/// A primitive referent resolves to no data declaration, so the record rule
/// cannot name its shape. `add_type` registers it as a `PrimitiveScalar`
/// structural carrier exactly as it does for a borrowed primitive elsewhere,
/// and the borrow still denotes the original storage rather than a snapshot.
fn shared_record_reference(
    program: &TypedTrees,
    expected: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    let TypeReferenceNode::Reference {
        referee, access, ..
    } = program.type_reference_table.type_reference(expected)
    else {
        return None;
    };
    if *access != language_semantics::ReferenceAccess::Shared
        || !validation::has_linear_owned_contents(program, *referee)
    {
        return None;
    }
    if program.primitive_type_reference(*referee).is_some() {
        return Some(*referee);
    }
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(*referee)
    else {
        return None;
    };
    let record = program
        .data_definitions()
        .iter()
        .find(|record| record.symbol == *symbol)?;
    (!program
        .data_members(record)
        .iter()
        .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_))))
    .then_some(*referee)
}

/// The declared type of a shared borrow's exact target without state scope: a
/// whole local's declared type, or a projected leaf resolved through its
/// root's declaration. `canonical_place_from_expression` keeps the root and
/// every segment exact.
fn borrowed_place_leaf_type(
    program: &TypedTrees,
    target: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    match program.expression_table.expression(target) {
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
    }
}

/// The declared type of a field/fixed-index projection leaf, resolved through
/// the authored root's own declaration: a whole local's declared type, or a
/// producing call's declared return type. Only exact field and literal-index
/// segments keep a statically checkable identity.
fn projected_leaf_type(
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
pub(super) fn canonical_place_is_borrowable(place: &crate::flow::CanonicalPlace) -> bool {
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
fn canonical_place_is_copyable(place: &crate::flow::CanonicalPlace) -> bool {
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

fn symbol_declared_type(program: &TypedTrees, symbol: SymbolHandle) -> Option<TypeReferenceHandle> {
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
fn scalar_case_place_type(program: &TypedTrees, expected: TypeReferenceHandle) -> bool {
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
pub(super) fn is_scalar_case_place_value(
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
fn copied_place_type(program: &TypedTrees, expected: TypeReferenceHandle) -> bool {
    // A stored `&'a [T]` view leaf copies the view value out of the carrier
    // the same way an `Unrestricted` named leaf does — the referent's loan
    // stays exactly where the field's `&` put it. Its bare `Slice` rung
    // unwraps to `Affine`, so the shared-borrow view's own multiplicity is
    // what decides admission.
    if (crate::execution::terminal_unit::types::borrowed_slice_view(program, expected)
        || crate::execution::terminal_unit::types::borrowed_named_view(program, expected))
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
pub(super) fn needs_copied_place_root(
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
fn zeroed_scalar_array_parts(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<(PrimitiveType, u64)> {
    let reference = validation::unwrapped_type_reference(program, type_reference)?;
    let TypeReferenceNode::FixedArray {
        element_type,
        length: typed_trees::types::FixedArrayLength::Literal(length),
    } = program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    let primitive_type = program.primitive_type_reference(*element_type)?;
    if !matches!(
        primitive_type,
        PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
    ) {
        return None;
    }
    Some((primitive_type, u64::try_from(*length).ok()?))
}

/// The checked literal-zero computation one synthesized zeroed field
/// element contributes: a landed `0` in the declared element carrier.
fn zeroed_integer_literal(
    primitive_type: PrimitiveType,
) -> Option<checked_trees::CheckedScalarExpression> {
    let landed_type = match primitive_type {
        PrimitiveType::I8 => numerics::literals::LandedIntegerType::I8,
        PrimitiveType::I16 => numerics::literals::LandedIntegerType::I16,
        PrimitiveType::I32 => numerics::literals::LandedIntegerType::I32,
        PrimitiveType::I64 => numerics::literals::LandedIntegerType::I64,
        PrimitiveType::U8 => numerics::literals::LandedIntegerType::U8,
        PrimitiveType::U16 => numerics::literals::LandedIntegerType::U16,
        PrimitiveType::U32 => numerics::literals::LandedIntegerType::U32,
        PrimitiveType::U64 => numerics::literals::LandedIntegerType::U64,
        _ => return None,
    };
    Some(checked_trees::CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_parts(
            false,
            numerics::literals::IntegerRadix::Decimal,
            "0",
        )
        .ok()?
        .with_landing(numerics::literals::IntegerLanding {
            landed_type,
            domain: numerics::arithmetic::ArithmeticDomain::Exact,
        }),
    })
}

/// The terminal expression carrying a projection's storage: the whole local
/// name for an owned place source, or the producing call for a structural
/// product source.
fn projection_root_expression(
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

impl Builder<'_, '_> {
    pub(super) fn structural_value(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
        values: &mut CheckedStructuralValuePlans,
        pure: &CheckedScalarExpressionPlans,
    ) -> Option<CheckedStructuralValueHandle> {
        let kind = if matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::String(_)
        ) {
            let argument = crate::execution::terminal_unit::calls::byte_sequence_literal_argument(
                self.program,
                expected,
                expression,
            )?;
            if argument.access != checked_trees::CheckedStructuralAccess::Owned {
                return None;
            }
            CheckedStructuralValueKind::Place(argument)
        } else if validation::reference_result_custody::stored_loan_parts(self.program, expected)
            .is_some_and(|(_, access)| {
                access == checked_trees::CheckedStructuralAccess::MutableBorrow
            })
        {
            let state = crate::lookup::machine_by_symbol(self.program, self.machine).and_then(
                |machine| {
                    self.program
                        .machine_states(machine)
                        .iter()
                        .find(|state| state.symbol == self.state)
                },
            )?;
            CheckedStructuralValueKind::Reference {
                source: validation::reference_result_custody::initializer_source(
                    self.program,
                    state,
                    expression,
                    expected,
                )?,
            }
        } else if matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Name(_)
        ) && validation::scalar_case_constructor(self.program, expression).is_none()
            && let Some(argument) = self.owned_record_place(expression, expected)
            && matches!(
                argument.source,
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { .. }
            )
        {
            // A whole owned parameter has no producer in this body: reading it
            // transfers the caller's value, which an unrestricted carrier
            // copies. Only a local's later read takes the copy route below,
            // because the local's initializer need not be its current value.
            CheckedStructuralValueKind::Place(argument)
        } else if let Some(view) = self
            .slice_view_place(expression, expected)
            .or_else(|| self.subslice_view_place(expression, expected))
        {
            // A view's authored producer is spelled as a call but establishes
            // no call result: it lends storage that already exists. Classify
            // it before the ordinary call-result branch, which would otherwise
            // demand a returning target for it.
            view
        } else if let ExpressionNode::Call(call) =
            self.program.expression_table.expression(expression)
        {
            let call_target = crate::flow::call_target_symbol(self.program, call);
            let returned = crate::flow::call_target_return_type(self.program, call_target)?;
            if self.program.normalized_type_identity(returned)
                != self.program.normalized_type_identity(expected)
                || !(validation::has_linear_owned_contents(self.program, returned)
                    || validation::reference_result_custody::is_reference_record(
                        self.program,
                        returned,
                    ))
            {
                return None;
            }
            let (source_call, call_ordinal) = self.call_ordinal(expression, call_target)?;
            self.record_call_arguments(
                pure,
                values,
                u32::try_from(self.statement_index).ok()?,
                call_ordinal,
                call_target,
                self.program
                    .expression_table
                    .expression_handles(call.arguments),
            );
            CheckedStructuralValueKind::Call { source_call }
        } else if let Some(copy) = self.scalar_case_place(expression, expected) {
            copy
        } else if let Some(copy) = self.view_element_copy(expression, expected) {
            copy
        } else if let Some(copied) = self.copied_place(expression, expected) {
            copied
        } else if matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Name(_)
        ) && validation::scalar_case_constructor(self.program, expression).is_none()
            && let Some(argument) = self.owned_record_place(expression, expected)
        {
            // Reading unrestricted storage snapshots its current contents at
            // this evaluation point. Try the copy routes before whole-place
            // custody: a mutable copyable local also has an owned shape, but
            // its original producer is not the value of a later read.
            CheckedStructuralValueKind::Place(argument)
        } else if let Some(projection) =
            self.projected_selection_place(expression, expected, values, pure)
        {
            projection
        } else if let Some(constructor) = self.case_construction(expression) {
            if self
                .program
                .normalized_type_identity(constructor.type_reference)
                != self.program.normalized_type_identity(expected)
            {
                return None;
            }
            for (field_ordinal, field) in self
                .plans
                .case_fields
                .span(constructor.fields)?
                .iter()
                .enumerate()
            {
                self.plans.roots.append(CheckedScalarComputationRoot {
                    machine: self.machine,
                    state: self.state,
                    statement_ordinal: u32::try_from(self.statement_index).ok()?,
                    role: CheckedScalarExpressionRole::StructuralValueField {
                        expression,
                        field_ordinal: u32::try_from(field_ordinal).ok()?,
                    },
                    root: field.value,
                });
            }
            CheckedStructuralValueKind::Case(constructor)
        } else if let ExpressionNode::StructLiteral(literal) =
            self.program.expression_table.expression(expression)
            && literal.case_symbol.is_some()
        {
            self.structural_case_value(expression, expected, values, pure)?
        } else if let ExpressionNode::StructLiteral(literal) =
            self.program.expression_table.expression(expression)
            && literal.case_symbol.is_none()
        {
            self.record_value(expression, expected, values, pure)?
        } else if let ExpressionNode::ArrayLiteral(literal) =
            self.program.expression_table.expression(expression)
        {
            if validation::is_closed_primitive_array_type(self.program, expected) {
                self.scalar_array_value(expression, expected)?
            } else {
                self.fixed_array_value(expected, *literal, values, pure)?
            }
        } else if let Some(symbol) =
            validation::scalar_case_value_source(self.program, expression, expected)
        {
            CheckedStructuralValueKind::Place(checked_trees::CheckedUnitStructuralArgumentPlan {
                source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol,
                },
                path: Vec::new(),
                type_identity: self
                    .program
                    .normalized_type_identity(expected)
                    .into_string(),
                access: checked_trees::CheckedStructuralAccess::Owned,
            })
        } else if let Some(borrowed) = self.borrowed_place(expression, expected) {
            borrowed
        } else {
            let ExpressionNode::Match(dispatch) =
                self.program.expression_table.expression(expression).clone()
            else {
                return None;
            };
            let machine = crate::lookup::machine_by_symbol(self.program, self.machine)?;
            let state = self
                .program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == self.state)?;
            let subject_type = validation::expression_result_type_reference(
                self.program,
                machine,
                state,
                dispatch.subject,
            )
            .and_then(|reference| validation::unwrapped_type_reference(self.program, reference))
            .and_then(|reference| self.program.primitive_type_reference(reference))
            .or_else(|| validation::match_subject_primitive_type(self.program, &dispatch))?;
            let subject = self.expression(dispatch.subject, subject_type)?;
            self.plans.nodes.get_mut(subject).authored_root = dispatch.subject;
            self.plans.roots.append(CheckedScalarComputationRoot {
                machine: self.machine,
                state: self.state,
                statement_ordinal: u32::try_from(self.statement_index).ok()?,
                role: CheckedScalarExpressionRole::StructuralValueSubject { expression },
                root: subject,
            });
            let authored = self
                .program
                .expression_table
                .match_arms(dispatch.arms)
                .to_vec();
            let mut arms = Vec::new();
            let mut covered = false;
            let mut boolean_coverage = [false; 2];
            for (ordinal, arm) in authored.iter().enumerate() {
                // First-match semantics make a repeated literal Boolean arm
                // unreachable. It cannot transfer an owner or evaluate fields.
                if subject_type == PrimitiveType::Bool
                    && let MatchPattern::Value(pattern) = arm.pattern
                    && let ExpressionNode::Boolean(value) =
                        self.program.expression_table.expression(pattern)
                    && boolean_coverage[usize::from(*value)]
                {
                    continue;
                }
                let source_arm = arena::Handle::from_parts(
                    dispatch
                        .arms
                        .start()
                        .arena_index()
                        .checked_add(u32::try_from(ordinal).ok()?)?,
                    dispatch.arms.start().generation(),
                );
                let equality_use = if matches!(arm.pattern, MatchPattern::Value(_))
                    && matches!(subject_type, PrimitiveType::F32 | PrimitiveType::F64)
                {
                    self.comparison_use(
                        expression,
                        checked_trees::CheckedOperatorOccurrence::MatchEquality { source_arm },
                    )?
                } else {
                    arena::Handle::invalid()
                };
                let pattern = match arm.pattern {
                    MatchPattern::Wildcard => {
                        covered = true;
                        checked_trees::CheckedScalarDispatchPattern::Wildcard
                    }
                    MatchPattern::Value(pattern) => {
                        if subject_type == PrimitiveType::Bool
                            && let ExpressionNode::Boolean(value) =
                                self.program.expression_table.expression(pattern)
                        {
                            boolean_coverage[usize::from(*value)] = true;
                            covered = boolean_coverage.iter().all(|value| *value);
                        }
                        let computation = self.expression(pattern, subject_type)?;
                        self.plans.nodes.get_mut(computation).authored_root = pattern;
                        self.plans.roots.append(CheckedScalarComputationRoot {
                            machine: self.machine,
                            state: self.state,
                            statement_ordinal: u32::try_from(self.statement_index).ok()?,
                            role: CheckedScalarExpressionRole::StructuralValuePattern {
                                source_arm,
                            },
                            root: computation,
                        });
                        checked_trees::CheckedScalarDispatchPattern::Value(computation)
                    }
                };
                let value = self.structural_value(arm.value, expected, values, pure)?;
                arms.push(CheckedStructuralDispatchArm {
                    source_arm,
                    equality_use,
                    pattern,
                    value,
                });
                if covered {
                    break;
                }
            }
            if !covered {
                return None;
            }
            CheckedStructuralValueKind::Dispatch {
                subject,
                arms: values.dispatch_arms.insert_many(arms),
            }
        };
        Some(
            values
                .nodes
                .append(CheckedStructuralValue { expression, kind }),
        )
    }
    /// One copyable payloadless case leaf projected out of shared-borrowed
    /// storage: `self.bitness` reads which case the borrowed record currently
    /// holds. The root and path become a `SharedBorrow` `Place` source exactly
    /// as `borrowed_place` builds for `&place`, so lowering observes the held
    /// case and establishes the same constructor fresh rather than moving a
    /// child out of the loan. Owned roots still move their whole leaf through
    /// the owned arms.
    fn scalar_case_place(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueKind> {
        if !is_scalar_case_place_value(
            self.program,
            self.state,
            self.statement_index,
            expression,
            expected,
        ) {
            return None;
        }
        let (source, leaf) = self.borrowed_leaf_argument(expression, expected)?;
        Some(CheckedStructuralValueKind::ScalarCasePlace { source, leaf })
    }

    /// One `[copy]` record element copied out of a borrowed view: the same
    /// element selection an element read takes, then one read per scalar
    /// field of the element in declaration order.
    fn view_element_copy(
        &self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueKind> {
        if !is_view_element_copy(self.program, expression, expected) {
            return None;
        }
        let selection = super::super::scalar_lowering::selected_element(
            self.program,
            self.operators,
            expression,
            self.parameters,
            self.authored_parameters,
            self.parameter_types,
            self.locals,
            self.exact_integer_casts,
        )?;
        if !selection.through_view
            || !selection.path.is_empty()
            || self
                .program
                .normalized_type_identity(selection.element_type)
                != self.program.normalized_type_identity(expected)
        {
            return None;
        }
        let TypeReferenceNode::Named { symbol, .. } = self
            .program
            .type_reference_table
            .type_reference(validation::unwrapped_type_reference(
                self.program,
                expected,
            )?)
        else {
            return None;
        };
        let data = self
            .program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == *symbol)?;
        let mut reads = Vec::new();
        for member in self.program.data_members(data) {
            let typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            let primitive_type = self
                .program
                .primitive_type_reference(field.type_reference)?;
            if field.relevance.is_erased()
                || primitive_type == PrimitiveType::Addr
                || !plain_or_policy_scalar_field(self.program, field.type_reference)
            {
                return None;
            }
            reads.push(
                checked_trees::CheckedScalarExpression::StructuralParameterIndexedRead {
                    root: selection.root,
                    path: Vec::new(),
                    index: Box::new(selection.index.clone()),
                    inner_indexes: Vec::new(),
                    element_path: vec![
                        checked_trees::CheckedStructuralPredicatePathSegment::Field(
                            field.path_identity(),
                        ),
                    ],
                    primitive_type,
                },
            );
        }
        (!reads.is_empty()).then_some(CheckedStructuralValueKind::ViewElementCopy { reads })
    }

    /// Copy one `Unrestricted` leaf from readable storage into a fresh owned
    /// place. The source root and its ownership or loan remain unchanged.
    fn copied_place(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueKind> {
        if !needs_copied_place_root(
            self.program,
            self.state,
            self.statement_index,
            expression,
            expected,
        ) {
            return None;
        }
        let (source, leaf) = self.borrowed_leaf_argument(expression, expected)?;
        Some(CheckedStructuralValueKind::CopiedStructuralPlace { source, leaf })
    }

    /// The `SharedBorrow` argument plan a borrowed leaf read produces: the
    /// named root as a parameter/local/`self` source plus the canonical
    /// member path, with `type_identity` naming the projected leaf.
    fn borrowed_leaf_argument(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<(
        checked_trees::CheckedUnitStructuralArgumentPlan,
        TypeReferenceHandle,
    )> {
        let place = crate::flow::canonical_place_from_expression_in_state(
            self.program,
            self.state,
            self.statement_index,
            expression,
        )?;
        let facts::PlaceRoot::Symbol(symbol) = place.root else {
            return None;
        };
        // Value planning precedes the final structural parameter roster. Its
        // dense positions change when a borrowed receiver is retained, so an
        // authored leaf must keep its symbol until binding resolves that roster.
        // StructuralLocal already represents both live locals and named formals;
        // independent source replay still checks the exact authored root.
        let symbol = self
            .authored_parameters
            .iter()
            .find(|parameter| {
                parameter.is_self && (parameter.symbol == symbol || self.machine == symbol)
            })
            .map_or(symbol, |parameter| parameter.symbol);
        let source =
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol };
        // The borrowed lane admits a runtime `Index` segment: the selector
        // resolves to a checked `RuntimeIndex` coordinate and terminal
        // verification re-proves `0 <= index < extent` against the enclosing
        // array's literal extent.
        let (projected, path) = crate::execution::terminal_unit::calls::projected_borrowed_path(
            self.program,
            self.machine,
            self.state,
            self.statement_index,
            &place,
        )?;
        if !validation::declared_carriers_match(self.program, projected, expected) {
            return None;
        }
        // The structural-type registry names a `&[T]` leaf by its borrowed
        // view carrier — the same identity `add_type` mints for the result —
        // so a view leaf records its unwrapped referee's identity. Arithmetic
        // policy likewise changes operations, not storage: owned leaves name
        // the policy-free carrier registered by ShapeCollector::add_type.
        // Keep `projected` below and the carrier comparison above qualified
        // by the declared leaf, never its text.
        let type_identity =
            if crate::execution::terminal_unit::types::borrowed_slice_view(self.program, projected)
            {
                validation::unwrapped_type_reference(self.program, projected).map(|unwrapped| {
                    self.program
                        .normalized_type_identity(unwrapped)
                        .into_string()
                })
            } else {
                None
            }
            .unwrap_or_else(|| {
                self.program
                    .normalized_type_identity(validation::peel_arithmetic_policy(
                        self.program,
                        projected,
                    ))
                    .into_string()
            });
        Some((
            checked_trees::CheckedUnitStructuralArgumentPlan {
                source,
                path,
                type_identity,
                access: checked_trees::CheckedStructuralAccess::SharedBorrow,
            },
            projected,
        ))
    }

    /// One selected projected child of an existing owner. The owned-selection
    /// transfer is the authoritative admission: its recorded canonical path
    /// must equal the authored place, and the projected type must be the
    /// enclosing result type. The root source is built as a whole `Place` (or
    /// `Call` product) node carrying the exact root identity.
    fn projected_selection_place(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
        values: &mut CheckedStructuralValuePlans,
        pure: &CheckedScalarExpressionPlans,
    ) -> Option<CheckedStructuralValueKind> {
        if !matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
        ) {
            return None;
        }
        // Owned-selection receipts are recorded only after value plans are
        // built, so this projection derives the same canonical place
        // independently. The multiplicity checker's recorded transfer path is
        // compared against this path downstream, not trusted here.
        let place = crate::flow::canonical_place_from_expression_in_state(
            self.program,
            self.state,
            self.statement_index,
            expression,
        )?;
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
        let (projected, path) = crate::execution::terminal_unit::calls::projected_argument_path(
            self.program,
            self.state,
            self.statement_index,
            &place,
        )?;
        if self.program.normalized_type_identity(projected)
            != self.program.normalized_type_identity(expected)
        {
            return None;
        }
        let root_expression = projection_root_expression(self.program, expression)?;
        let root_reference = crate::flow::canonical_place_type_reference(
            self.program,
            self.state,
            self.statement_index,
            &crate::flow::CanonicalPlace {
                root: place.root,
                segments: Vec::new(),
            },
        )?;
        let source = self.structural_value(root_expression, root_reference, values, pure)?;
        Some(CheckedStructuralValueKind::Projection {
            source,
            path,
            type_identity: self
                .program
                .normalized_type_identity(projected)
                .into_string(),
        })
    }

    /// One shared borrow of an exact place selected as a `&T` result. The
    /// referent stays owned by its established root; the authored target's
    /// canonical root and path become a `SharedBorrow` source so lowering can
    /// rejoin borrowed custody at the selection's block parameter instead of
    /// moving a child. `shared_record_reference` pins the referent to a
    /// carrier the pipeline can build -- a record, or a primitive whose
    /// structural shape is its own scalar -- and the target must be the exact
    /// place semantics admitted: a named root under record fields and literal
    /// fixed indexes, whose projected leaf type has to equal the declared
    /// referent.
    fn borrowed_place(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueKind> {
        let ExpressionNode::Borrow(borrow) = self.program.expression_table.expression(expression)
        else {
            return None;
        };
        if borrow.access != language_semantics::ReferenceAccess::Shared {
            return None;
        }
        let referent = shared_record_reference(self.program, expected)?;
        let place = crate::flow::canonical_place_from_expression_in_state(
            self.program,
            self.state,
            self.statement_index,
            borrow.target,
        )?;
        if !canonical_place_is_borrowable(&place) {
            return None;
        }
        let facts::PlaceRoot::Symbol(symbol) = place.root else {
            return None;
        };
        // The root's dense structural position is its signature ordinal when
        // the name resolves to a carried parameter, exactly as
        // `owned_record_place` classifies owned arms; anything else is an
        // operation-sequence local keyed by symbol.
        let source = if let Some((index, _)) = self
            .authored_parameters
            .iter()
            .filter(|parameter| {
                !parameter.is_const
                    && !parameter.relevance.is_erased()
                    && self
                        .program
                        .primitive_type_reference(parameter.type_reference)
                        .is_none()
            })
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == symbol)
        {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(index).ok()?,
            }
        } else {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
        };
        let (projected, path) = crate::execution::terminal_unit::calls::projected_argument_path(
            self.program,
            self.state,
            self.statement_index,
            &place,
        )?;
        if self.program.normalized_type_identity(projected)
            != self.program.normalized_type_identity(referent)
        {
            return None;
        }
        Some(CheckedStructuralValueKind::Reference {
            source: checked_trees::CheckedUnitStructuralArgumentPlan {
                source,
                path,
                type_identity: self
                    .program
                    .normalized_type_identity(projected)
                    .into_string(),
                access: checked_trees::CheckedStructuralAccess::SharedBorrow,
            },
        })
    }

    fn owned_record_place(
        &self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<checked_trees::CheckedUnitStructuralArgumentPlan> {
        let ExpressionNode::Name(name) = self.program.expression_table.expression(expression)
        else {
            return None;
        };
        if !name.symbol.is_valid()
            || name.head_symbol != name.symbol
            || name.members.count() != 1
            // The same finite owned walls as plain storage, but a `[linear]`
            // member rides as a claim the selection transfer names.
            || !validation::has_linear_owned_contents(self.program, expected)
        {
            return None;
        }
        let symbol = closed_data_symbol(self.program, expected)?;
        let data = self
            .program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == symbol)?;
        if self
            .program
            .data_members(data)
            .iter()
            .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_)))
        {
            return None;
        }
        let owner = crate::lookup::machine_by_symbol(self.program, self.machine)?;
        let state = self
            .program
            .machine_states(owner)
            .iter()
            .find(|state| state.symbol == self.state)?;
        let (source, reference) = if let Some((index, parameter)) = self
            .authored_parameters
            .iter()
            .filter(|parameter| {
                // An `[erased]` binding owns no structural position either.
                !parameter.is_const
                    && !parameter.relevance.is_erased()
                    && self
                        .program
                        .primitive_type_reference(parameter.type_reference)
                        .is_none()
            })
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == name.symbol)
        {
            (
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: u32::try_from(index).ok()?,
                },
                parameter.type_reference,
            )
        } else {
            let (_, local) = self
                .program
                .state_local_declaration(state, name.symbol)
                .filter(|&(ordinal, local)| {
                    ordinal < self.statement_index && local.initial_value.is_valid()
                })?;
            (
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol: local.symbol,
                },
                local.type_reference,
            )
        };
        if self.program.normalized_type_identity(reference)
            != self.program.normalized_type_identity(expected)
            || !validation::has_linear_owned_contents(self.program, reference)
            || closed_data_symbol(self.program, reference) != Some(symbol)
        {
            return None;
        }
        Some(checked_trees::CheckedUnitStructuralArgumentPlan {
            source,
            path: Vec::new(),
            type_identity: self
                .program
                .normalized_type_identity(reference)
                .into_string(),
            access: checked_trees::CheckedStructuralAccess::Owned,
        })
    }

    /// Element-wise array literal establishment: each element registers as
    /// its own structural value under the declared element type, exactly as
    /// record fields do inside a constructor.
    fn fixed_array_value(
        &mut self,
        expected: TypeReferenceHandle,
        literal: arena::HandleSpan<ExpressionHandle>,
        values: &mut CheckedStructuralValuePlans,
        pure: &CheckedScalarExpressionPlans,
    ) -> Option<CheckedStructuralValueKind> {
        if validation::is_closed_primitive_array_type(self.program, expected) {
            return None;
        }
        let reference = validation::unwrapped_type_reference(self.program, expected)?;
        let TypeReferenceNode::FixedArray { element_type, .. } =
            self.program.type_reference_table.type_reference(reference)
        else {
            return None;
        };
        let mut elements = Vec::new();
        for element in self.program.expression_table.expression_handles(literal) {
            elements.push(self.structural_value(*element, *element_type, values, pure)?);
        }
        Some(CheckedStructuralValueKind::FixedArray { elements })
    }

    /// A closed primitive array supplied as one member of a structural value.
    /// The ordinary scalar-array producer flattens it to its row-major leaves;
    /// each leaf evaluates in authored order as its own scalar computation,
    /// keyed by this array expression the way record fields key by their
    /// literal. Lowering establishes the whole roster as one scalar array, so
    /// the array contributes no element custody of its own. Constant-array
    /// projections stay with their indexing-selection owner.
    fn scalar_array_value(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueKind> {
        if !matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::ArrayLiteral(_)
        ) {
            return None;
        }
        let array =
            validation::scalar_array_elements(self.program, self.machine, expression, expected)?;
        if !array.projections.is_empty() {
            return None;
        }
        let statement_ordinal = u32::try_from(self.statement_index).ok()?;
        let mut elements = Vec::with_capacity(array.elements.len());
        for (element_ordinal, (leaf, primitive_type)) in array.elements.into_iter().enumerate() {
            let root = self.expression(leaf, primitive_type)?;
            self.plans.nodes.get_mut(root).authored_root = leaf;
            self.plans.roots.append(CheckedScalarComputationRoot {
                machine: self.machine,
                state: self.state,
                statement_ordinal,
                role: CheckedScalarExpressionRole::StructuralArrayElement {
                    expression,
                    element_ordinal: u32::try_from(element_ordinal).ok()?,
                },
                root,
            });
            elements.push(root);
        }
        Some(CheckedStructuralValueKind::ScalarArray { elements })
    }

    fn record_value(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
        values: &mut CheckedStructuralValuePlans,
        pure: &CheckedScalarExpressionPlans,
    ) -> Option<CheckedStructuralValueKind> {
        let ExpressionNode::StructLiteral(literal) =
            self.program.expression_table.expression(expression).clone()
        else {
            return None;
        };
        let plain_contents = |reference| {
            validation::has_plain_owned_contents_with_numeric_constraints(self.program, reference)
                || validation::has_structural_construction_contents(self.program, reference)
                || validation::has_linear_owned_contents(self.program, reference)
                || validation::reference_result_custody::is_reference_record(
                    self.program,
                    reference,
                )
                || crate::execution::terminal_unit::types::record_with_owned_or_shared_view_fields(
                    self.program,
                    reference,
                )
                || (validation::has_owned_or_shared_view_fields(self.program, reference)
                    && qualified_views_are_quoted(self.program, reference, &literal))
        };
        // A custody domain on the declared type (`Region in Owned`) qualifies
        // the binding, not the record's storage: the shape question is about
        // the underlying record. Domain-qualified storage that is not a plain
        // owned shape still refuses on the unwrapped check.
        if !plain_contents(expected)
            && !validation::unwrapped_type_reference(self.program, expected)
                .is_some_and(|unwrapped| unwrapped != expected && plain_contents(unwrapped))
        {
            return None;
        }
        let reference = validation::unwrapped_type_reference(self.program, expected)?;
        let Some(data) = self.program.data_definitions().iter().find(|data| {
            validation::member_type_substitutions(self.program, data, reference).is_some()
        }) else {
            return None;
        };
        if literal.case_name.is_some() || literal.type_symbol != data.symbol {
            return None;
        }
        let substitutions = validation::member_type_substitutions(self.program, data, reference)?;
        let program = self.program;
        let member_type = |field: &typed_trees::data::DataField| {
            validation::substituted_member_reference(program, field.type_reference, &substitutions)
        };
        let declared = self.program.data_members(data);
        if declared
            .iter()
            .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_)))
        {
            return None;
        }
        let authored = self
            .program
            .expression_table
            .struct_fields(literal.fields)
            .to_vec();
        let relevant = declared
            .iter()
            .filter_map(|member| match member {
                typed_trees::data::DataMember::Field(field) if !field.relevance.is_erased() => {
                    Some(field)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let erased = declared
            .iter()
            .filter_map(|member| match member {
                typed_trees::data::DataMember::Field(field) if field.relevance.is_erased() => {
                    Some(field)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        // An ordinary runtime field may be omitted only when its zero value
        // satisfies the declared carrier — today that means a `[scalar; N]`
        // leaf whose element is a plain integer primitive. Every authored
        // initializer must still name a declared member exactly once.
        let omitted = relevant
            .iter()
            .filter(|field| {
                authored
                    .iter()
                    .all(|initializer| initializer.field_symbol != field.symbol)
            })
            .copied()
            .collect::<Vec<_>>();
        if authored.len() != relevant.len() + erased.len()
            && (authored.len() > relevant.len() + erased.len()
                || authored.iter().any(|initializer| {
                    declared.iter().all(|member| match member {
                        typed_trees::data::DataMember::Field(field) => {
                            field.symbol != initializer.field_symbol
                        }
                        _ => true,
                    })
                })
                || omitted.iter().any(|field| {
                    zeroed_scalar_array_parts(self.program, member_type(field)).is_none()
                        && !validation::zeroed_structural_parts(self.program, member_type(field))
                }))
        {
            return None;
        }
        let mut fields = Vec::new();
        for (ordinal, initializer) in authored.into_iter().enumerate() {
            let erased_field = erased
                .iter()
                .find(|field| field.symbol == initializer.field_symbol);
            let field = if let Some(field) = erased_field {
                *field
            } else {
                *relevant
                    .iter()
                    .find(|field| field.symbol == initializer.field_symbol)?
            };
            if erased_field.is_some() {
                // The erased member stays in the checked record's field list —
                // it carries semantic content but no runtime storage, so the
                // checked value keeps its exact initializer for interpretation
                // while emission skips it below the lowered representations.
                let value = checked_trees::CheckedStructuralRecordFieldValue::Structural(
                    self.structural_value(initializer.value, member_type(field), values, pure)?,
                );
                fields.push(checked_trees::CheckedStructuralRecordField {
                    field: field.symbol,
                    expression: initializer.value,
                    type_reference: member_type(field),
                    value,
                });
                continue;
            }
            if fields
                .iter()
                .any(|prior: &checked_trees::CheckedStructuralRecordField| {
                    prior.field == field.symbol
                })
            {
                return None;
            }
            // A quoted literal for a shared byte view member lends immutable
            // image storage: there is nothing to construct, so the member is
            // its octets. Only a record literal assigned through an exclusive
            // root consumes it, as one view field store per such member; the
            // literal must already grant every declared domain of the view.
            if let ExpressionNode::String(bytes) =
                self.program.expression_table.expression(initializer.value)
                && matches!(
                    crate::execution::terminal_unit::types::byte_sequence_carrier(
                        self.program,
                        member_type(field),
                        &[],
                    ),
                    Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView {
                        access: Some(checked_trees::CheckedStructuralAccess::SharedBorrow),
                    })
                )
            {
                if !crate::facts::field_domain::string_literal_grants_view_domains(
                    self.program,
                    member_type(field),
                    initializer.value,
                ) {
                    return None;
                }
                let node = values.nodes.append(CheckedStructuralValue {
                    expression: initializer.value,
                    kind: CheckedStructuralValueKind::ByteLiteralView {
                        bytes: bytes.to_vec(),
                    },
                });
                fields.push(checked_trees::CheckedStructuralRecordField {
                    field: field.symbol,
                    expression: initializer.value,
                    type_reference: member_type(field),
                    value: checked_trees::CheckedStructuralRecordFieldValue::Structural(node),
                });
                continue;
            }
            let declared = member_type(field);
            let reference = validation::unwrapped_type_reference(self.program, declared)?;
            let value =
                if validation::reference_result_custody::parts(self.program, declared).is_some() {
                    checked_trees::CheckedStructuralRecordFieldValue::Structural(
                        self.structural_value(initializer.value, declared, values, pure)?,
                    )
                } else if let Some(primitive) = self.program.primitive_type_reference(reference) {
                    let root = self.expression(initializer.value, primitive)?;
                    self.plans.nodes.get_mut(root).authored_root = initializer.value;
                    self.plans.roots.append(CheckedScalarComputationRoot {
                        machine: self.machine,
                        state: self.state,
                        statement_ordinal: u32::try_from(self.statement_index).ok()?,
                        role: CheckedScalarExpressionRole::RecordField {
                            expression,
                            field_ordinal: u32::try_from(ordinal).ok()?,
                        },
                        root,
                    });
                    checked_trees::CheckedStructuralRecordFieldValue::Scalar(root)
                } else {
                    checked_trees::CheckedStructuralRecordFieldValue::Structural(
                        self.structural_value(initializer.value, declared, values, pure)?,
                    )
                };
            fields.push(checked_trees::CheckedStructuralRecordField {
                field: field.symbol,
                expression: initializer.value,
                type_reference: declared,
                value,
            });
        }
        for field in omitted {
            let declared = member_type(&field);
            if zeroed_scalar_array_parts(self.program, declared).is_none() {
                // Structural members carry no element roster: the emitted
                // EstablishZeroedStructural owns the whole shape's zeros.
                let node = values.nodes.append(CheckedStructuralValue {
                    expression,
                    kind: CheckedStructuralValueKind::ZeroedStructural {},
                });
                fields.push(checked_trees::CheckedStructuralRecordField {
                    field: field.symbol,
                    expression,
                    type_reference: declared,
                    value: checked_trees::CheckedStructuralRecordFieldValue::Structural(node),
                });
                continue;
            }
            let (primitive_type, element_count) =
                zeroed_scalar_array_parts(self.program, declared)?;
            let zero = zeroed_integer_literal(primitive_type)?;
            let element = self
                .plans
                .nodes
                .append(checked_trees::CheckedScalarComputation {
                    authored_root: ExpressionHandle::invalid(),
                    value_source: ExpressionHandle::invalid(),
                    primitive_type,
                    kind: checked_trees::CheckedScalarComputationKind::Value(zero),
                });
            // This literal has no authored scalar occurrence. The zeroed array
            // owns its node; structural replay checks the declared carrier,
            // element count and literal zero before emission.
            let node = values.nodes.append(CheckedStructuralValue {
                expression,
                kind: CheckedStructuralValueKind::ZeroedScalarArray {
                    element,
                    element_count,
                },
            });
            fields.push(checked_trees::CheckedStructuralRecordField {
                field: field.symbol,
                expression,
                type_reference: declared,
                value: checked_trees::CheckedStructuralRecordFieldValue::Structural(node),
            });
        }
        Some(CheckedStructuralValueKind::Record {
            data_symbol: literal.type_symbol,
            fields: values.record_fields.insert_many(fields),
        })
    }

    /// A case literal composes its payload fields exactly like a record's:
    /// authored initializers pair against the selected case's declarations in
    /// authored order, and each operand is a scalar computation or a nested
    /// structural value. The case tag stays with the construction, so the
    /// value keeps its discriminated identity a record cannot spell.
    fn structural_case_value(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
        values: &mut CheckedStructuralValuePlans,
        pure: &CheckedScalarExpressionPlans,
    ) -> Option<CheckedStructuralValueKind> {
        let constructor =
            validation::structural_case_constructor(self.program, expression, expected)?;
        if self
            .program
            .normalized_type_identity(constructor.type_reference)
            != self.program.normalized_type_identity(expected)
        {
            return None;
        }
        let mut fields = Vec::with_capacity(constructor.fields.len());
        for (ordinal, (symbol, value_expression, declared)) in
            constructor.fields.into_iter().enumerate()
        {
            let reference = validation::unwrapped_type_reference(self.program, declared)?;
            let value =
                if validation::reference_result_custody::parts(self.program, declared).is_some() {
                    checked_trees::CheckedStructuralRecordFieldValue::Structural(
                        self.structural_value(value_expression, declared, values, pure)?,
                    )
                } else if let Some(primitive) = self.program.primitive_type_reference(reference) {
                    let root = self.expression(value_expression, primitive)?;
                    self.plans.nodes.get_mut(root).authored_root = value_expression;
                    self.plans.roots.append(CheckedScalarComputationRoot {
                        machine: self.machine,
                        state: self.state,
                        statement_ordinal: u32::try_from(self.statement_index).ok()?,
                        role: CheckedScalarExpressionRole::StructuralValueField {
                            expression,
                            field_ordinal: u32::try_from(ordinal).ok()?,
                        },
                        root,
                    });
                    checked_trees::CheckedStructuralRecordFieldValue::Scalar(root)
                } else {
                    checked_trees::CheckedStructuralRecordFieldValue::Structural(
                        self.structural_value(value_expression, declared, values, pure)?,
                    )
                };
            fields.push(checked_trees::CheckedStructuralRecordField {
                field: symbol,
                expression: value_expression,
                type_reference: declared,
                value,
            });
        }
        let ExpressionNode::StructLiteral(literal) =
            self.program.expression_table.expression(expression)
        else {
            return None;
        };
        Some(CheckedStructuralValueKind::StructuralCase {
            data_symbol: literal.type_symbol,
            case: constructor.case,
            fields: values.record_fields.insert_many(fields),
        })
    }
}
