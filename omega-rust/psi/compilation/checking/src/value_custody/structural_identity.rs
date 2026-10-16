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
//!   lends, including a reference to a fixed byte array.
//! - A qualification restricts values without changing their storage and is
//!   looked through unless its declaration owns a storage restriction. A
//!   primitive interval or scalar predicate keeps its qualification so its
//!   receiving stores and fresh reads can name the exact restriction. Value
//!   domain membership such as Finite remains in the separate custody channel.
//!   Fixed arrays retain their exact carrier identity under qualification.
//! - A type parameter bound in `substitutions` stands for its actual.
//!
//! A stored loan that is neither a byte carrier nor an element view keeps its
//! descriptor as a `Reference` shape, registered under its whole reference
//! type; `stored_structural_type_identity` answers for that family as well.

use crate::value_custody::reference_shells::{reference_shell, slice_view_shell};
use symbols::SymbolHandle;
use typed_trees::type_identity::TypeIdentityRequest;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// The type node whose identity a structural plan for `type_reference`
/// registers under: the storage beneath its loan and qualification shells,
/// as described in the module comment.
pub fn structural_identity_type(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> TypeReferenceHandle {
    let mut reference = type_reference;
    // A walk longer than the table has nodes has revisited one.
    for _ in 0..=program.type_reference_table.type_reference_count() {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. } => reference = *referee,
            TypeReferenceNode::Constrained { constraints, .. }
                if (program.primitive_type_reference(reference).is_some()
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
        && crate::stored_view_shell(program, type_reference, substitutions).is_none()
}

/// The identity of the plan a stored value of `type_reference` occupies — a
/// field, an element, a local or a result holding it. A `Reference` shape is
/// registered under its whole reference type. Stored slice descriptors do the
/// same so their own access survives containment in an array or record;
/// aggregate multiplicity cannot distinguish shared and exclusive leaves.
/// Other values use [`structural_type_identity`].
pub fn stored_structural_type_identity(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> typed_trees::type_identity::TypeIdentityId {
    if stores_reference_shape(program, type_reference, substitutions)
        || slice_view_shell(program, type_reference, substitutions).is_some()
    {
        return program.storage_type_identity(TypeIdentityRequest {
            binders,
            substitutions,
            ..TypeIdentityRequest::ordinary(type_reference)
        });
    }
    structural_type_identity(program, type_reference, binders, substitutions)
}
