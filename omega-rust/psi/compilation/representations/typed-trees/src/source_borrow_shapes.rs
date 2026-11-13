//! Inspect declared reference and slice shells under exact type substitutions.

use crate::types::{TypeReferenceHandle, TypeReferenceNode};
use language_semantics::ReferenceAccess;
use symbols::SymbolHandle;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReferenceShell {
    pub access: ReferenceAccess,
    pub referee: TypeReferenceHandle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SliceViewShell {
    pub access: ReferenceAccess,
    pub slice: TypeReferenceHandle,
    pub element: TypeReferenceHandle,
}

pub fn reference_shell(
    program: &crate::TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<ReferenceShell> {
    reference_shell_with_substitutions(program, reference, &[])
}

pub fn reference_shell_with_substitutions(
    program: &crate::TypedTrees,
    reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Option<ReferenceShell> {
    let shell = peel_carrier_shells(program, reference, substitutions)?;
    let TypeReferenceNode::Reference {
        referee, access, ..
    } = program.type_reference_table.type_reference(shell)
    else {
        return None;
    };
    Some(ReferenceShell {
        access: *access,
        referee: *referee,
    })
}

pub fn slice_view_shell(
    program: &crate::TypedTrees,
    reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Option<SliceViewShell> {
    let shell = peel_carrier_shells(program, reference, substitutions)?;
    let TypeReferenceNode::Reference {
        referee, access, ..
    } = program.type_reference_table.type_reference(shell)
    else {
        return None;
    };
    let slice = peel_carrier_shells(program, *referee, substitutions)?;
    let TypeReferenceNode::Slice { element_type } =
        program.type_reference_table.type_reference(slice)
    else {
        return None;
    };
    Some(SliceViewShell {
        access: *access,
        slice,
        element: *element_type,
    })
}

pub fn peel_carrier_shells(
    program: &crate::TypedTrees,
    mut reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Option<TypeReferenceHandle> {
    for _ in 0..=program.type_reference_table.type_reference_count() {
        if !program.contains_type_reference(reference) {
            return None;
        }
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Named { symbol, .. } => {
                let Some(replacement) = substitutions
                    .iter()
                    .rev()
                    .find(|(formal, _)| formal == symbol)
                    .map(|(_, actual)| *actual)
                else {
                    return Some(reference);
                };
                if replacement == reference {
                    return None;
                }
                reference = replacement;
            }
            _ => return Some(reference),
        }
    }
    None
}

pub fn carrier_beneath_borrows(
    program: &crate::TypedTrees,
    mut reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    if !reference.is_valid() {
        return None;
    }
    for _ in 0..=program.type_reference_table.type_reference_count() {
        if !program.contains_type_reference(reference) {
            return None;
        }
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Reference { referee, .. } => reference = *referee,
            _ => return Some(reference),
        }
    }
    None
}

pub fn stored_view_shell(
    program: &crate::TypedTrees,
    reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Option<ReferenceShell> {
    slice_view_shell(program, reference, substitutions)?;
    reference_shell(program, reference)
}
