//! Nonescaping observations and calls join typed crossings to existing custody.
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
    // A dynamic observation does not broaden static structural call actuals
    // or permit a new escaping reference formation.
    if source
        .path
        .iter()
        .any(|segment| segment.runtime_index().is_some())
    {
        return None;
    }
    let (current, prefix_start) = access_referent_tail(function, types, source)?;
    if source.access != terminal_psi::StructuralAccess::Owned
        && let Some(endpoint) = terminal_semantics::borrowed_primitive_projection_declaration(
            types.values().copied(),
            current,
            &source.path[prefix_start..],
            None,
        )
    {
        return Some(endpoint);
    }
    leaf_type(types, current, &source.path[prefix_start..])
}

/// Authenticate each pointer word and attenuate access at its declared ceiling.
/// Consumers own their endpoint grammar and bounds; the live ownership walk
/// separately requires every possible stored descriptor selected at run time.
pub(crate) fn access_referent_tail(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    source: &terminal_psi::StructuralArgument,
) -> Option<(StructuralTypeId, usize)> {
    if source.access == terminal_psi::StructuralAccess::Owned {
        return None;
    }
    let signature = structural_source_contract(function, source.place)?;
    if signature.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || signature.has_result_evidence()
    {
        return None;
    }
    project_referent_tail(
        types,
        signature.structural_type,
        signature.backing_access(types.values().copied()),
        source,
    )
}

/// Project a declared carrier under its already authenticated access ceiling.
/// Callers retain responsibility for qualifications and result evidence; this
/// shared walk only attenuates permission at each explicit reference crossing.
pub(crate) fn project_referent_tail(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    mut current: StructuralTypeId,
    mut authority: terminal_psi::StructuralAccess,
    source: &terminal_psi::StructuralArgument,
) -> Option<(StructuralTypeId, usize)> {
    if source.access == terminal_psi::StructuralAccess::Owned {
        return None;
    }
    let mut prefix_start = 0;
    for (position, segment) in source.path.iter().enumerate() {
        match segment {
            terminal_psi::StructuralPathSegment::Field(_)
            | terminal_psi::StructuralPathSegment::FixedIndex(_)
            | terminal_psi::StructuralPathSegment::RuntimeIndex { .. } => {}
            terminal_psi::StructuralPathSegment::Referent => {
                current = access_path_type(types, current, &source.path[prefix_start..position])?;
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
    Some((current, prefix_start))
}

/// A descriptor region has one declared type even when its exact element is
/// selected at run time. Static field resolution retains its existing identity
/// checks; this query supplies no live custody or selector-bound evidence.
pub(crate) fn access_path_type(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    mut current: StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
) -> Option<StructuralTypeId> {
    for segment in path {
        current = match segment {
            terminal_psi::StructuralPathSegment::RuntimeIndex { .. } => {
                match types.get(&current)?.shape {
                    terminal_psi::StructuralTypeShape::FixedArray { element, .. }
                    | terminal_psi::StructuralTypeShape::ElementView { element, .. } => element,
                    _ => return None,
                }
            }
            _ => leaf_type(types, current, std::slice::from_ref(segment))?,
        };
    }
    Some(current)
}
