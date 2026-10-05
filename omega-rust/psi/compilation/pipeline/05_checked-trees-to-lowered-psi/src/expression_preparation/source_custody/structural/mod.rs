//! Reconstruct construction, selected ownership and ordered dispatch from authored nodes.
//! Establishing a local place does not necessarily establish a new value origin:
//! whole affine moves preserve the original provenance. Replay traces the authored
//! initializer chain separately from checking each place's establishment and transfer.
//!
//! `validate` rejoins an establishment to its destination, then `validate_value`
//! walks the retained value nodes as a worklist. Records, case
//! payloads and structural selections replay in `record`, `structural_case`
//! and `dispatch`, which queue their structural children back onto the same
//! worklist through `Replay`.

use checked_trees::CheckedTrees;
use checked_trees::expression::ExpressionHandle;
use checked_trees::types::TypeReferenceHandle;

pub(crate) mod byte_literals;
mod dispatch;
mod local_disposition;
mod owned_places;
mod owned_selection;
mod record;
mod shared_borrow;
mod slice_view;
mod structural_case;
mod view_element_copy;

/// The referent of a shared-borrow `&T` result/local type: `T` must be a named
/// record or a primitive, independently of whether a record contains loans.
/// A shared borrow observes the referent instead of moving its contents;
/// contained loans and linear claims stay with the referent's owner and never
/// become owned obligations of this join. A
/// primitive referent has no data declaration to resolve; its structural
/// carrier is the `PrimitiveScalar` shape the type catalog already registers,
/// and the borrow still denotes the original storage rather than a snapshot.
/// `None` for owned types, `&mut` carriers (which keep their own
/// `reference_result_custody` lane), and sums. Shared by source replay and
/// attached-unit registration, so it lives here rather than in a machine
/// producer.
pub(crate) fn shared_borrow_record_referent(
    checked: &CheckedTrees,
    reference: checked_trees::types::TypeReferenceHandle,
) -> Option<checked_trees::types::TypeReferenceHandle> {
    let checked_trees::types::TypeReferenceNode::Reference {
        referee, access, ..
    } = checked.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    if *access != language_semantics::ReferenceAccess::Shared {
        return None;
    }
    if checked.primitive_type_reference(*referee).is_some() {
        return crate::content_classes::linear_owned_contents(checked, *referee)
            .then_some(*referee);
    }
    // `&[T]` names the slice view itself as the borrowed record: element custody
    // belongs to the view, so there are no plain owned contents to require.
    if let checked_trees::types::TypeReferenceNode::Slice { .. } =
        checked.type_reference_table.type_reference(*referee)
    {
        return Some(*referee);
    }
    let checked_trees::types::TypeReferenceNode::Named { symbol, .. } =
        checked.type_reference_table.type_reference(*referee)
    else {
        return None;
    };
    let record = checked
        .data_definitions()
        .iter()
        .find(|record| record.symbol == *symbol)?;
    if checked
        .data_members(record)
        .iter()
        .any(|member| matches!(member, checked_trees::data::DataMember::Variant(_)))
    {
        return None;
    }
    Some(*referee)
}

/// The carrier a mutable slice binding joins. Both ByteSequence and ElementView
/// catalogs identify the slice beneath the reference shell; access and affine
/// descriptor debt are independently retained on its result. Named and primitive
/// referees keep their reference shell, so this is not generic dereferencing.
pub(crate) fn mutable_view_carrier(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    validation::slice_view_shell(&checked.typed, reference, &[])
        .filter(|view| view.access == language_semantics::ReferenceAccess::Mutable)
        .map(|view| view.slice)
}

/// The carrier of an actual shared slice view, including byte slices. This
/// queries source access independently of whether Terminal uses ByteSequence
/// or ElementView storage; mutable and write-only references cannot lend it.
pub(crate) fn shared_slice_view_referent(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    validation::slice_view_shell(&checked.typed, reference, &[])
        .filter(|view| view.access == language_semantics::ReferenceAccess::Shared)
        .map(|view| view.slice)
}

/// The type a view-binding `let` declares. Both authored spellings bind the
/// same slice: `let v: &[u8] = coll[a..b]` names the view directly, while
/// `let v: u8 = coll[a..b]` names only its element — the binding's declared
/// type is then the expression's own slice-view result, exactly as the
/// checker resolved it. A non-view declaration is returned unchanged.
pub(crate) fn declared_view_binding_type(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    state: &checked_trees::state::State,
    declared: TypeReferenceHandle,
    initial_value: ExpressionHandle,
) -> TypeReferenceHandle {
    if validation::reference_shell(&checked.typed, declared).is_some() {
        return declared;
    }
    validation::expression_result_type_reference(&checked.typed, machine, state, initial_value)
        .filter(|result| validation::slice_view_shell(&checked.typed, *result, &[]).is_some())
        .unwrap_or(declared)
}
