//! Borrowed views and byte-sequence carriers: which borrow shells present as a
//! slice or named view, and the carrier each byte view realizes as.

use super::*;

/// A borrowed `[T]` element view: one reference shell over a non-byte slice.
/// Fixed arrays and vectors own contiguous storage and a slice borrows it,
/// carrying its own extent as a stored runtime length rather than a declared
/// constant, so the view's shape names only its element. Arithmetic-domain
/// and other constraint shells qualify the carrier without changing it. The
/// byte element keeps its established `ByteSequence` carrier, whose length,
/// read and subslice obligations are already reconstructed. The shell's
/// access is reported, not required: a shared `&[T]` and an exclusive
/// `&mut [T]` view share this carrier, and each consumer admits the loan
/// its own custody needs.
pub(crate) fn element_view_shell(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Option<validation::SliceViewShell> {
    validation::slice_view_shell(program, type_reference, substitutions)
        .filter(|view| program.primitive_type_reference(view.element) != Some(PrimitiveType::U8))
}

/// The element of a borrowed `[T]` element view under any access.
pub(crate) fn borrowed_slice_view_element(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Option<TypeReferenceHandle> {
    element_view_shell(program, type_reference, substitutions).map(|view| view.element)
}

/// A `&[T]` declared result type, either view family. The byte element's
/// `BorrowedView` carrier and the non-byte element's identity share the same
/// reference-peeling shape; an owned-view `&mut` result stays with the
/// primitive reference-result custody family instead.
pub(crate) fn borrowed_slice_view(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    structural_access_for_type_reference(program, type_reference)
        == Some(CheckedStructuralAccess::SharedBorrow)
        && (borrowed_slice_view_element(program, type_reference, &[]).is_some()
            || matches!(
                byte_sequence_carrier(program, type_reference, &[]),
                Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. })
            ))
}

/// A `&'a V` declared result type: a shared borrow of a named record or sum —
/// the named-carrier analog of `borrowed_slice_view`. The referent's storage
/// lives in the caller's frame, so the owned-contents custody clause does not
/// apply. `&mut`/`&write` named results stay with their own custody families.
pub(crate) fn borrowed_named_view(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    borrowed_view_referent(
        program,
        type_reference,
        language_semantics::ReferenceAccess::Shared,
    )
    .is_some()
}

/// A `&mut R`/`&mut [T]` exclusive structural view: the exclusive-loan
/// sibling of `borrowed_named_view`/`borrowed_slice_view`. The referent's
/// storage lives in the caller's frame the same way, but writes flow
/// through the loan — `parts` keeps the primitive-referent case.
pub(crate) fn mutable_borrowed_view(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    borrowed_view_referent(
        program,
        type_reference,
        language_semantics::ReferenceAccess::Mutable,
    )
    .is_some()
}

/// The referee a structural view's reference shell lends under exactly
/// `access`, with `Constrained` shells peeled. The accesses admit different
/// referees, spelled directly beneath the shell: a shared `&'a V` view lends
/// a named or generic referent, primitives included; an exclusive view lends
/// a non-primitive named referent or a `[T]` slice, while primitive
/// referents stay with `parts`. Write-only shells lend no structural view.
pub(crate) fn borrowed_view_referent(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    access: language_semantics::ReferenceAccess,
) -> Option<TypeReferenceHandle> {
    let shell = validation::reference_shell(program, type_reference)
        .filter(|shell| shell.access == access)?;
    let referee = program.type_reference_table.type_reference(shell.referee);
    let lends_view = match access {
        language_semantics::ReferenceAccess::Shared => matches!(
            referee,
            TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. }
        ),
        language_semantics::ReferenceAccess::Mutable => {
            program.primitive_type_reference(shell.referee).is_none()
                && matches!(
                    referee,
                    TypeReferenceNode::Named { .. } | TypeReferenceNode::Slice { .. }
                )
        }
        language_semantics::ReferenceAccess::WriteOnly => false,
    };
    lends_view.then_some(shell.referee)
}

/// The `type_identity` an anonymous-result plan mints for a borrowed-view
/// result — the carrier's own identity with borrow and constraint shells
/// peeled, exactly as `ShapeCollector::add_type` registers it.
pub(crate) fn borrowed_view_result_identity(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<String> {
    if let Some(referee) = borrowed_view_referent(
        program,
        type_reference,
        language_semantics::ReferenceAccess::Shared,
    ) {
        return Some(
            program
                .type_identity(TypeIdentityRequest::ordinary(referee))
                .into_string(),
        );
    }
    borrowed_slice_view(program, type_reference)
        .then(|| borrowed_slice_view_type_identity(program, type_reference, &[], &[]))
}

/// A named record whose fields are each either plain owned contents or a
/// shared borrowed view — the record owns its scalars while each `&` field
/// keeps its own statically named loan.
pub(crate) fn record_with_owned_or_shared_view_fields(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    validation::has_owned_or_shared_view_fields(program, type_reference)
}

/// The view's own identity is the borrowed `[T]` carrier, with the reference
/// and constraint shells peeled exactly as the borrowed byte view peels them:
/// borrow access belongs to the plan carrying the view, not to its shape.
pub(crate) fn borrowed_slice_view_type_identity(
    program: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> String {
    while let TypeReferenceNode::Reference { referee, .. }
    | TypeReferenceNode::Constrained {
        base_type: referee, ..
    } = program.type_reference_table.type_reference(type_reference)
    {
        type_reference = *referee;
    }
    program
        .type_identity(TypeIdentityRequest {
            binders,
            substitutions,
            ..TypeIdentityRequest::ordinary(type_reference)
        })
        .into_string()
}

pub(crate) fn byte_sequence_carrier(
    program: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Option<checked_trees::CheckedByteSequenceCarrier> {
    let mut borrowed = None;
    let mut has_domain = false;
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Named { symbol, .. } => {
                let Some((_, replacement)) = substitutions
                    .iter()
                    .rev()
                    .find(|(parameter, _)| parameter == symbol)
                else {
                    break;
                };
                // A substitution can bind a parameter to its own reference;
                // rewriting it again only loops.
                if *replacement == type_reference {
                    break;
                }
                type_reference = *replacement;
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                has_domain |= program
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .any(|constraint| matches!(constraint, TypeConstraintNode::Domain(_)));
                type_reference = *base_type;
            }
            // The reference's access is the carrier's access: a `&mut [u8]`
            // field and a `&[u8]` one are distinguishable here and nowhere
            // downstream, so it is recorded rather than discarded.
            TypeReferenceNode::Reference {
                referee, access, ..
            } if borrowed.is_none() => {
                borrowed = Some(*access);
                type_reference = *referee;
            }
            _ => break,
        }
    }

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Slice { element_type }
            if borrowed.is_some()
                && program.primitive_type_reference(*element_type) == Some(PrimitiveType::U8) =>
        {
            Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView {
                access: Some(borrowed?.into()),
            })
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: typed_trees::types::FixedArrayLength::Literal(capacity),
        } if program.primitive_type_reference(*element_type) == Some(PrimitiveType::U8) => {
            match borrowed {
                // A domain-qualified fixed byte carrier owns a live extent. A
                // reference loans that same owner; access remains on the formal.
                Some(_) if has_domain => {
                    Some(checked_trees::CheckedByteSequenceCarrier::BoundedOwned {
                        capacity: u64::try_from(*capacity).ok()?,
                    })
                }
                // A borrowed reference to a fixed array keeps the referent in
                // the caller's frame; its extent is the declaration's literal
                // length rather than a stored descriptor field.
                Some(access) => Some(checked_trees::CheckedByteSequenceCarrier::BoundedView {
                    access: Some(access.into()),
                    capacity: u64::try_from(*capacity).ok()?,
                }),
                None if has_domain => {
                    Some(checked_trees::CheckedByteSequenceCarrier::BoundedOwned {
                        capacity: u64::try_from(*capacity).ok()?,
                    })
                }
                None => None,
            }
        }
        _ => None,
    }
}
