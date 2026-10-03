//! Carrier presentation shared by ordinary calls, boundary calls and state edges.
//!
//! This owner decides representation compatibility only. Call admission keeps
//! source access, multiplicity, predicate obligations and live-loan custody.
use crate::execution::terminal_unit::types::{
    base_type_identity, base_type_identity_with_substitutions, byte_sequence_carrier,
    byte_sequence_type_identity, substituted_formal_type,
};
use crate::execution::terminal_unit::{SymbolHandle, TypeReferenceNode, TypedTrees};

/// Whether caller storage may be presented at an ordinary or boundary
/// parameter. The identity rule is the transitive lane's: the
/// projected type carries the parameter's exact normalized identity.
///
/// A bounded byte owner may also supply a borrowed byte-view parameter. The
/// call keeps its exact storage place and projection: lowering derives a
/// descriptor over that storage's live extent. This is the same representation
/// relation for ordinary helpers and boundary requirements, independent of
/// whether the call returns a scalar or Unit. Equal element types alone do not
/// establish it; both types must classify as byte-sequence carriers. Source
/// access and predicate custody must independently admit the call, including
/// preservation of a mutable owner's invariants. Whole parameters and
/// projections share this relation.
pub(crate) fn is_admitted(
    program: &TypedTrees,
    projected_type: typed_trees::types::TypeReferenceHandle,
    parameter_type: typed_trees::types::TypeReferenceHandle,
    target_identity: &str,
    substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
) -> bool {
    if base_type_identity(program, projected_type, &[])
        .is_some_and(|identity| identity == target_identity)
    {
        return true;
    }
    let formal = substituted_formal_type(program, parameter_type, substitutions);
    match (
        byte_sequence_carrier(program, projected_type, &[]),
        byte_sequence_carrier(program, formal, substitutions),
    ) {
        // A fixed byte reference preserves its pointer carrier identity,
        // while caller storage names the exact array referent. Equal base
        // identity proves the element and declared extent; the formal's
        // carrier identity remains authoritative at the receiving boundary.
        (
            None | Some(checked_trees::CheckedByteSequenceCarrier::BoundedView { .. }),
            Some(checked_trees::CheckedByteSequenceCarrier::BoundedView { .. }),
        ) => {
            byte_sequence_type_identity(program, formal, &[], substitutions)
                .is_some_and(|identity| identity == target_identity)
                && base_type_identity(program, projected_type, &[]).is_some_and(|identity| {
                    base_type_identity_with_substitutions(program, formal, &[], substitutions)
                        .is_some_and(|expected| identity == expected)
                })
        }
        (
            Some(checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. }),
            Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. }),
        ) => true,
        // A stored shared view field presents its own descriptor at a shared
        // view formal: the callee reborrows the stored view's backing for the
        // call, not the record. An exclusive view on either side would
        // duplicate or widen a loan, so both must be shared.
        (
            Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. }),
            Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. }),
        ) => {
            shared_view_reference(program, projected_type)
                && crate::execution::terminal_unit::types::structural_access_for_type_reference(
                    program, formal,
                ) == Some(checked_trees::CheckedStructuralAccess::SharedBorrow)
        }
        _ => false,
    }
}

/// Whether a declared field type is a shared `&[u8]` view, through any
/// domain constraint shells.
fn shared_view_reference(
    program: &TypedTrees,
    mut type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Reference { access, .. } => {
                return *access == language_core::ReferenceAccess::Shared;
            }
            _ => return false,
        }
    }
}
