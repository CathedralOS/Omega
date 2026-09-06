//! Independently reconstruct the maximal live record/array path complement.

use super::*;

pub(super) fn partial_affine_residuals(
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    root_type: StructuralTypeId,
    moved_paths: &BTreeSet<Vec<StructuralPathSegment>>,
    max_residuals: usize,
) -> Option<Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>> {
    if moved_paths.is_empty()
        || moved_paths.iter().any(|path| {
            path.is_empty()
                || crate::unit_validation::resolve_structural_path(
                    structural_types,
                    root_type,
                    path,
                )
                .is_none()
        })
        || moved_paths.iter().enumerate().any(|(index, path)| {
            moved_paths
                .iter()
                .enumerate()
                .any(|(other_index, other)| index != other_index && path.starts_with(other))
        })
    {
        return None;
    }
    let moved_paths = moved_paths.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let mut residuals = Vec::new();
    collect_partial_affine_residuals(
        structural_types,
        root_type,
        &moved_paths,
        &mut Vec::new(),
        max_residuals,
        &mut residuals,
    )?;
    Some(residuals)
}

fn collect_partial_affine_residuals(
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
    moved_paths: &[&[StructuralPathSegment]],
    prefix: &mut Vec<StructuralPathSegment>,
    max_residuals: usize,
    residuals: &mut Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>,
) -> Option<()> {
    match &structural_types.get(&structural_type)?.shape {
        StructuralTypeShape::Record { fields } => {
            if fields.is_empty()
                || fields.iter().any(|field| {
                    field.relevance.is_erased()
                        || !matches!(
                            field.field_type,
                            StructuralFieldType::Structural(_)
                                | StructuralFieldType::Scalar(_)
                                | StructuralFieldType::IeeeFloat(_)
                                | StructuralFieldType::ByteSequence(
                                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                                )
                        )
                })
            {
                return None;
            }
            for field in fields.iter().rev() {
                if let StructuralFieldType::Structural(child_type) = field.field_type {
                    collect_child_residuals(
                        structural_types,
                        child_type,
                        StructuralPathSegment::Field(field.identity.clone()),
                        moved_paths,
                        prefix,
                        max_residuals,
                        residuals,
                    )?;
                }
            }
        }
        StructuralTypeShape::FixedArray { element, length } if *length != 0 => {
            // Every untouched child costs one supplied cleanup. Count that
            // lower bound before enumerating children, including for an empty
            // complement, so forged dimensions cannot create unbounded work.
            let remaining = max_residuals.checked_sub(residuals.len())?;
            if u128::from(*length) > remaining as u128 + moved_paths.len() as u128 {
                return None;
            }
            for index in (0..*length).rev() {
                collect_child_residuals(
                    structural_types,
                    *element,
                    StructuralPathSegment::FixedIndex(index),
                    moved_paths,
                    prefix,
                    max_residuals,
                    residuals,
                )?;
            }
        }
        _ => return None,
    }
    Some(())
}

fn collect_child_residuals(
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    child_type: StructuralTypeId,
    segment: StructuralPathSegment,
    moved_paths: &[&[StructuralPathSegment]],
    prefix: &mut Vec<StructuralPathSegment>,
    max_residuals: usize,
    residuals: &mut Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>,
) -> Option<()> {
    let descendants = moved_paths
        .iter()
        .filter_map(|path| {
            let (head, tail) = path.split_first()?;
            (head == &segment).then_some(tail)
        })
        .collect::<Vec<_>>();
    prefix.push(segment);
    if descendants.is_empty() {
        if residuals.len() == max_residuals {
            return None;
        }
        residuals.push((prefix.clone(), child_type));
    } else if descendants.iter().all(|path| !path.is_empty()) {
        collect_partial_affine_residuals(
            structural_types,
            child_type,
            &descendants,
            prefix,
            max_residuals,
            residuals,
        )?;
    } else if descendants.len() != 1 {
        return None;
    }
    prefix.pop();
    Some(())
}
