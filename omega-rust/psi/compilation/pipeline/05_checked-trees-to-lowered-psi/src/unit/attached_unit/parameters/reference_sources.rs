//! Reference provenance uses an ending `Referent` marker for the borrowed
//! leaf, including a borrow formed at non-reference storage. Ordinary call
//! projections instead cross an actual reference carrier at that segment.

use super::{
    LoweringError, StructuralArgument, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, lower_structural_arguments, lower_typed_argument_path,
    structural_access_can_supply, terminal_access, unsupported,
};
use crate::expression_preparation::bindings::StructuralLocalBinding;
use checked_trees::{
    CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralPathSegment,
};

/// Resolve the original source home and typed prefix through the ordinary
/// argument owner, then retain the reference-source leaf marker. Formation,
/// access and escape permission remain with reference custody validation.
pub(crate) fn lower_reference_source(
    source: &CheckedUnitStructuralArgumentPlan,
    structural_types: &[StructuralTypeDeclaration],
    parameters: &[StructuralParameterDeclaration],
    trivial_affine_locals: &[StructuralPlaceDeclaration],
    structural_results: &[(Option<StructuralPlaceDeclaration>, bool)],
    primitive_locals: &[super::super::primitive_locals::PrimitiveLocal],
    structural_locals: &[StructuralLocalBinding],
) -> Result<StructuralArgument, LoweringError> {
    if source.access == CheckedStructuralAccess::Owned {
        return unsupported("reference source cannot transfer owned custody");
    }
    let mut prefix = source.clone();
    let has_leaf_marker = matches!(
        prefix.path.last(),
        Some(CheckedUnitStructuralPathSegment::Referent)
    );
    if has_leaf_marker {
        prefix.path.pop();
    }
    let mut argument = if let Some(root) = prefix.structural_storage_root() {
        // The operation sequence has already materialized this local or
        // expression. Borrow its published descriptor slot and type; the
        // initializer's old backing path is no longer its storage identity.
        let mut bindings = structural_locals
            .iter()
            .filter(|binding| binding.root == root);
        let binding = bindings.next().ok_or(LoweringError::Unsupported(
            "reference source has no established structural storage",
        ))?;
        let access = terminal_access(prefix.access);
        if bindings.next().is_some()
            || !binding.argument.path.is_empty()
            || !structural_access_can_supply(binding.argument.access, access)
        {
            return unsupported("reference source changes its established storage custody");
        }
        StructuralArgument {
            place: binding.argument.place,
            path: lower_typed_argument_path(
                &prefix.path,
                binding.structural_type,
                structural_types,
                None,
                &mut std::iter::empty(),
                None,
            )?,
            access,
        }
    } else {
        let arguments = lower_structural_arguments(
            std::slice::from_ref(&prefix),
            structural_types,
            parameters,
            trivial_affine_locals,
            structural_results,
            &[],
            &[],
            primitive_locals,
            None,
            &mut std::iter::empty(),
            None,
        )?;
        let [argument] = <[StructuralArgument; 1]>::try_from(arguments)
            .map_err(|_| LoweringError::Unsupported("reference source is not one argument"))?;
        argument
    };
    if has_leaf_marker {
        argument
            .path
            .push(terminal_psi::StructuralPathSegment::Referent);
    }
    Ok(argument)
}

/// Normalize exact caller-relative origins under each returned reference leaf.
/// Multiple completion alternatives share one output path without losing their
/// distinct source places, projections, or access permissions.
pub(crate) fn lower_reference_result_sources(
    references: &[checked_trees::CheckedReferenceResultSourcePlan],
    structural_types: &[StructuralTypeDeclaration],
    parameters: &[StructuralParameterDeclaration],
) -> Result<Vec<terminal_psi::StructuralReferenceResultSource>, LoweringError> {
    let mut references = references
        .iter()
        .map(|reference| {
            let source = lower_reference_source(
                &reference.source,
                structural_types,
                parameters,
                &[],
                &[],
                &[],
                &[],
            )?;
            Ok(terminal_psi::StructuralReferenceResultSource {
                path: super::lower_structural_path(&reference.path)?,
                sources: vec![source],
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    references.sort_by(|left, right| left.path.cmp(&right.path));
    let mut grouped: Vec<terminal_psi::StructuralReferenceResultSource> =
        Vec::with_capacity(references.len());
    for reference in references {
        if let Some(group) = grouped.last_mut()
            && group.path == reference.path
        {
            group.sources.extend(reference.sources);
        } else {
            grouped.push(reference);
        }
    }
    for group in &mut grouped {
        group.sources.sort();
        group.sources.dedup();
    }
    Ok(grouped)
}
