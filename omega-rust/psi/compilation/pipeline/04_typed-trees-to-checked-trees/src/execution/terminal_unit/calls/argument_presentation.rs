//! Carrier presentation shared by ordinary calls, boundary calls and state edges.
//!
//! This owner decides representation compatibility only. Call admission keeps
//! source access, multiplicity, predicate obligations and live-loan custody.
use crate::execution::terminal_unit::types::{
    base_type_identity, byte_sequence_carrier, lent_storage_identity, substituted_formal_type,
};
use crate::execution::terminal_unit::{SymbolHandle, TypeReferenceNode};

/// A representation relation, not authority to lend the selected storage.
/// Array views additionally need initialized, unqualified backing and an
/// admitted projection at the consuming operation.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArgumentPresentation {
    SameIdentity,
    SharedScalarObservation,
    ByteSequence,
    FixedArraySlice,
}

/// Classify caller storage's presentation at an ordinary or boundary
/// parameter. The identity rule is the transitive lane's: the
/// projected type carries the parameter's exact normalized identity.
///
/// A bounded byte owner may also supply a borrowed byte-view parameter. The
/// call keeps its exact storage place and projection: lowering derives a
/// descriptor over that storage's live extent. This is the same representation
/// relation for ordinary helpers and boundary requirements, independent of
/// whether the call returns a scalar or Unit. Equal element types alone do not
/// establish it. Raw fixed arrays instead retain their exact element type and
/// literal extent while lending a slice descriptor. Source
/// access and predicate custody must independently admit the call, including
/// preservation of a mutable owner's invariants. Whole parameters and
/// projections share this relation.
pub(crate) fn classify(
    program: &dyn ::validation::CheckingInput,
    projected_type: typed_trees::types::TypeReferenceHandle,
    parameter_type: typed_trees::types::TypeReferenceHandle,
    target_identity: typed_trees::type_identity::TypeIdentityId,
    substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
) -> Option<ArgumentPresentation> {
    if validation::stored_structural_type_identity(program, projected_type, &[], &[])
        == target_identity
        || base_type_identity(program, projected_type, &[], &[])
            .is_some_and(|identity| identity == target_identity)
    {
        return Some(ArgumentPresentation::SameIdentity);
    }
    let formal = substituted_formal_type(program, parameter_type, substitutions);
    // A shared scalar formal observes the field's carrier without owning its
    // receiving predicate. Keep the exact storage type for mutable/write-only
    // loans; only an unrestricted shared formal may forget a value restriction.
    if let Some(reference) = validation::reference_shell(program, formal)
        && reference.access == language_core::ReferenceAccess::Shared
        && validation::is_plain_scalar_storage(program, reference.referee)
        && let Some(primitive) = program.primitive_type_reference(projected_type)
        && program.primitive_type_reference(reference.referee) == Some(primitive)
        && base_type_identity(program, formal, &[], substitutions) == Some(target_identity)
    {
        return Some(ArgumentPresentation::SharedScalarObservation);
    }
    if super::boundary_admission::fixed_array_slice_view_is_admitted(
        program,
        projected_type,
        formal,
    ) && base_type_identity(program, formal, &[], substitutions)
        .is_some_and(|identity| identity == target_identity)
    {
        return Some(ArgumentPresentation::FixedArraySlice);
    }
    let admitted = match (
        byte_sequence_carrier(program, projected_type, &[]),
        byte_sequence_carrier(program, formal, substitutions),
    ) {
        // A fixed byte reference preserves its pointer carrier identity,
        // while caller storage names the exact array referent. Equal lent
        // storage proves the element and declared extent; the formal's
        // carrier identity remains authoritative at the receiving boundary.
        (
            None | Some(checked_trees::CheckedByteSequenceCarrier::BoundedView { .. }),
            Some(checked_trees::CheckedByteSequenceCarrier::BoundedView { .. }),
        ) => {
            base_type_identity(program, formal, &[], substitutions)
                .is_some_and(|identity| identity == target_identity)
                && lent_storage_identity(program, projected_type, &[]).is_some_and(|identity| {
                    lent_storage_identity(program, formal, substitutions)
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
    };
    admitted.then_some(ArgumentPresentation::ByteSequence)
}

/// Whether a declared field type is a shared `&[u8]` view, through any
/// domain constraint shells.
fn shared_view_reference(
    program: &dyn ::validation::CheckingInput,
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
