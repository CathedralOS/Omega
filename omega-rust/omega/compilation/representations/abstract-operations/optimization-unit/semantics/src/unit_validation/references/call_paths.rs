//! Nonescaping call paths join exact typed crossings to existing loan custody.
//! No path walk mints a permission or broadens escaping reference formation.

use super::{
    PsiOptimizationFunction, StructuralTypeId, access_covers, leaf_type, reference_source_type,
    structural_source_contract,
};
use std::collections::BTreeMap;

/// Type and attenuated access of one nonescaping static reference path.
/// This does not extend escaping reference formation or result correspondence.
pub(crate) fn call_source_type(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    source: &terminal_psi::StructuralArgument,
) -> Option<StructuralTypeId> {
    if !source
        .path
        .contains(&terminal_psi::StructuralPathSegment::Referent)
    {
        return reference_source_type(function, types, source);
    }
    if source.access == terminal_psi::StructuralAccess::Owned {
        return None;
    }
    let signature = structural_source_contract(function, source.place)?;
    if signature.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || !signature.is_unqualified()
        || signature.has_result_evidence()
    {
        return None;
    }
    let mut current = signature.structural_type;
    let mut authority = signature.access;
    let mut prefix_start = 0;
    for (position, segment) in source.path.iter().enumerate() {
        match segment {
            terminal_psi::StructuralPathSegment::Field(_)
            | terminal_psi::StructuralPathSegment::FixedIndex(_) => {}
            terminal_psi::StructuralPathSegment::Referent => {
                current = leaf_type(types, current, &source.path[prefix_start..position])?;
                let (referent, access) =
                    types
                        .get(&current)
                        .and_then(|declaration| match declaration.shape {
                            terminal_psi::StructuralTypeShape::Reference { referent, access } => {
                                Some((referent, access))
                            }
                            _ => None,
                        })?;
                // Reading a pointer word requires readable storage, even when
                // the final call requests write-only access to its referent.
                if authority == terminal_psi::StructuralAccess::WriteOnlyBorrow {
                    return None;
                }
                authority = if access_covers(authority, access) {
                    access
                } else if access_covers(access, authority) {
                    authority
                } else {
                    return None;
                };
                current = referent;
                prefix_start = position + 1;
            }
            _ => return None,
        }
    }
    if !access_covers(authority, source.access) {
        return None;
    }
    leaf_type(types, current, &source.path[prefix_start..])
}
