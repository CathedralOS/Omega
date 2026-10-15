//! The identity a structural type plan registers under.
//!
//! Checking publishes one structural type plan per identity for a whole
//! Terminal module, and every later consumer — call and state-edge admission
//! in checking, lowering's rejoin of authored locals, parameters and results —
//! finds that plan again by recomputing the identity from an authored type.
//! They agree only when they look through the same shells, so this module owns
//! that rule rather than each consumer peeling its own:
//!
//! - A loan's access binds on the field, parameter or local that uses a type,
//!   not on the type, so a reference shell is looked through to the storage it
//!   lends. A `&[u8; N]` bounded view is the exception: its referent stays in
//!   the caller's frame and the descriptor itself is the carrier, so it keeps
//!   its shell and never names the plain array it borrows.
//! - A qualification restricts values without changing their storage and is
//!   looked through unless its declaration owns a storage restriction. A
//!   primitive interval or scalar predicate keeps its qualification so its
//!   receiving stores and fresh reads can name the exact restriction. Value
//!   domain membership such as Finite remains in the separate custody channel.
//!   A domain over a fixed byte array also turns plain array storage into a
//!   bounded byte owner with a live extent.
//!   That owner keeps its qualification, so a plain `[u8; N]` and a
//!   `[u8; N] in D` never share an identity, and with it one plan, in a module
//!   that holds both — whichever target's bodies happen to name them.
//! - A type parameter bound in `substitutions` stands for its actual.
//!
//! A stored loan that is neither a byte carrier nor an element view keeps its
//! descriptor as a `Reference` shape, registered under its whole reference
//! type; `stored_structural_type_identity` answers for that family as well.

use crate::value_custody::reference_shells::{
    byte_sequence_carrier, reference_shell, slice_view_shell,
};
use checked_trees::CheckedByteSequenceCarrier;
use symbols::SymbolHandle;
use typed_trees::type_identity::TypeIdentityRequest;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// The type node whose identity a structural plan for `type_reference`
/// registers under: the storage beneath its loan and qualification shells,
/// with a bounded view keeping its loan and a bounded byte owner keeping its
/// qualification (see the module comment).
pub fn structural_identity_type(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> TypeReferenceHandle {
    let carrier = byte_sequence_carrier(program, type_reference, substitutions);
    if matches!(
        carrier,
        Some(CheckedByteSequenceCarrier::BoundedView { .. })
    ) {
        return type_reference;
    }
    let keeps_qualification = matches!(
        carrier,
        Some(CheckedByteSequenceCarrier::BoundedOwned { .. })
    );
    let mut reference = type_reference;
    // A walk longer than the table has nodes has revisited one.
    for _ in 0..=program.type_reference_table.type_reference_count() {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. } => reference = *referee,
            TypeReferenceNode::Constrained { constraints, .. }
                if keeps_qualification
                    || (program.primitive_type_reference(reference).is_some()
                        && program.type_reference_table.constraints(*constraints).iter().any(|constraint|
                            matches!(constraint, typed_trees::types::TypeConstraintNode::Range { .. }
                                | typed_trees::types::TypeConstraintNode::Named(_))
                            || matches!(constraint, typed_trees::types::TypeConstraintNode::Domain(domain)
                                if domain.subject != typed_trees::types::DomainConstraintSubject::Value(
                                    language_semantics::value_domain::ValueDomain::Finite)))) => return reference,
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Named { symbol, .. } => {
                match crate::substitutions::actual_for(substitutions, *symbol) {
                    // A substitution can bind a parameter to its own
                    // reference; rewriting it again only loops.
                    Some(actual) if actual != reference => reference = actual,
                    _ => return reference,
                }
            }
            _ => return reference,
        }
    }
    reference
}

/// The identity a structural plan for `type_reference` registers under when
/// the plan describes the storage a value lends or owns: the identity of
/// [`structural_identity_type`] under the same binders and substitutions.
pub fn structural_type_identity(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> typed_trees::type_identity::TypeIdentityId {
    program.storage_type_identity(TypeIdentityRequest {
        binders,
        substitutions,
        ..TypeIdentityRequest::ordinary(structural_identity_type(
            program,
            type_reference,
            substitutions,
        ))
    })
}

/// Whether a stored value of `type_reference` keeps its loan as a `Reference`
/// shape: a borrow that does not already store a byte or `[T]` view descriptor.
/// A loan of a bounded byte owner points at that owner's live storage; it must
/// not inline the owner merely because formal ingress presents its byte shape.
pub fn stores_reference_shape(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> bool {
    reference_shell(program, type_reference).is_some()
        && !matches!(
            byte_sequence_carrier(program, type_reference, substitutions),
            Some(
                CheckedByteSequenceCarrier::BorrowedView { .. }
                    | CheckedByteSequenceCarrier::BoundedView { .. }
            )
        )
        && slice_view_shell(program, type_reference, substitutions).is_none()
}

/// The identity of the plan a stored value of `type_reference` occupies — a
/// field, an element, a local or a result holding it. A `Reference` shape is
/// registered under its whole reference type; every other value under
/// [`structural_type_identity`].
pub fn stored_structural_type_identity(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> typed_trees::type_identity::TypeIdentityId {
    if stores_reference_shape(program, type_reference, substitutions)
        || (slice_view_shell(program, type_reference, substitutions).is_some()
            && byte_sequence_carrier(program, type_reference, substitutions).is_none())
    {
        return program.storage_type_identity(TypeIdentityRequest {
            binders,
            substitutions,
            ..TypeIdentityRequest::ordinary(type_reference)
        });
    }
    structural_type_identity(program, type_reference, binders, substitutions)
}
