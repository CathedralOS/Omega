//! The identity a structural type plan registers under.
//!
//! Checking publishes one structural type plan per identity for a whole
//! Terminal module, and every later consumer — call and state-edge admission
//! in checking, lowering's rejoin of authored locals, parameters and results —
//! finds that plan again by recomputing the identity from an authored type.
//! They agree only when they look through the same shells, so this module owns
//! that rule rather than each consumer peeling its own:
//!
//! - A loan's outer access binds on the parameter or operand that uses a
//!   type. Executable storage erases that presentation once, retaining any
//!   inner reference as pointer data. Logical source projection instead looks
//!   through all root presentation shells and records actual loads separately.
//! - A qualification restricts values without changing their storage and is
//!   looked through unless its declaration owns a storage restriction. A
//!   primitive interval or scalar predicate keeps its qualification so its
//!   receiving stores and fresh reads can name the exact restriction. Value
//!   domain membership such as Finite remains in the separate custody channel.
//!   Predicate-free scalar tags use that same separation. Keeping their
//!   qualification in storage identity would create multiple unrestricted
//!   scalar declarations and make an inline field's carrier ambiguous. The
//!   declaration-backed tag judgment is shared with signature custody; every
//!   non-tag constraint still retains its full receiving declaration.
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

/// The logical backing beneath all root loan presentation and qualification
/// shells. Source paths use this geometry; executable signature/storage
/// registration instead uses [`structural_storage_type`].
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
                if program
                    .primitive_type_reference(reference)
                    .is_some_and(|primitive| {
                        program
                            .type_reference_table
                            .constraints(*constraints)
                            .iter()
                            .any(|constraint| {
                                use typed_trees::types::TypeConstraintNode;
                                match constraint {
                                    TypeConstraintNode::Range { .. }
                                    | TypeConstraintNode::Named(_) => true,
                                    TypeConstraintNode::Domain(domain) => domain.subject
                                        != typed_trees::types::DomainConstraintSubject::Value(
                                            language_semantics::value_domain::ValueDomain::Finite,
                                        )
                                        && !crate::proof_contracts::domains::scalar_tag_constraint(
                                            program, domain, primitive,
                                        ),
                                    _ => false,
                                }
                            })
                    }) =>
            {
                return reference;
            }
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

/// The storage an executable occurrence lends. Its outer borrow is the
/// parameter's access edge; a further reference is stored pointer data and
/// keeps its own shape, access and referent. The logical source projection
/// query above deliberately looks through that remaining presentation.
/// Constraint and specialization normalization uses the common shell owner.
pub fn structural_storage_type(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> TypeReferenceHandle {
    let Some(outer) = crate::value_custody::reference_shells::peel_carrier_shells(
        program,
        type_reference,
        substitutions,
    ) else {
        return TypeReferenceHandle::invalid();
    };
    if let TypeReferenceNode::Reference { referee, .. } =
        program.type_reference_table.type_reference(outer)
    {
        let Some(stored) = crate::value_custody::reference_shells::peel_carrier_shells(
            program,
            *referee,
            substitutions,
        ) else {
            return TypeReferenceHandle::invalid();
        };
        if matches!(
            program.type_reference_table.type_reference(stored),
            TypeReferenceNode::Reference { .. }
        ) {
            // Validate only the finite reference chain, without expanding a
            // nominal pointee. A cycle cannot acquire a physical identity.
            let mut selected = stored;
            let mut remaining = program.type_reference_table.type_reference_count();
            while let TypeReferenceNode::Reference { referee, .. } =
                program.type_reference_table.type_reference(selected)
            {
                let Some(next) = remaining.checked_sub(1) else {
                    return TypeReferenceHandle::invalid();
                };
                remaining = next;
                let Some(next) = crate::value_custody::reference_shells::peel_carrier_shells(
                    program,
                    *referee,
                    substitutions,
                ) else {
                    return TypeReferenceHandle::invalid();
                };
                selected = next;
            }
            return stored;
        }
    }
    structural_identity_type(program, type_reference, substitutions)
}

/// Source projections name the logical backing after all root presentation
/// shells. This geometry identity grants no reference load or access.
pub fn logical_structural_type_identity(
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

/// The identity a structural plan for `type_reference` registers under when
/// the plan describes the storage a value lends or owns: the identity of
/// [`structural_storage_type`] under the same binders and substitutions.
pub fn structural_type_identity(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> typed_trees::type_identity::TypeIdentityId {
    let storage = structural_storage_type(program, type_reference, substitutions);
    if !storage.is_valid() {
        return Default::default();
    }
    program.storage_type_identity(TypeIdentityRequest {
        binders,
        substitutions,
        ..TypeIdentityRequest::ordinary(storage)
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
    if !structural_storage_type(program, type_reference, substitutions).is_valid() {
        return Default::default();
    }
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
