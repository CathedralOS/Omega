//! Type-directed maximal residual subtrees after projected owned transfers.

use semantic_vocabulary::StructuralTypeId;
use std::collections::BTreeMap;
use terminal_psi::{
    ByteSequenceCarrier, StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

pub(in crate::lowering) fn expected_maximal_residual_subtrees(
    root_type: StructuralTypeId,
    moved: &[(Vec<StructuralPathSegment>, StructuralTypeId)],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    max_residuals: usize,
) -> Option<Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>> {
    if moved.is_empty()
        || moved.iter().enumerate().any(|(index, (path, _))| {
            !is_partial_cleanup_path(path)
                || moved[..index]
                    .iter()
                    .any(|(earlier, _)| path.starts_with(earlier) || earlier.starts_with(path))
        })
    {
        return None;
    }
    let borrowed = moved
        .iter()
        .map(|(path, structural_type)| (path.as_slice(), *structural_type))
        .collect::<Vec<_>>();
    let mut residuals = Vec::new();
    append(
        root_type,
        &mut Vec::new(),
        &borrowed,
        declarations,
        max_residuals,
        &mut residuals,
    )?;
    Some(residuals)
}

pub(in crate::lowering) fn is_partial_cleanup_path(path: &[StructuralPathSegment]) -> bool {
    !path.is_empty()
        && path.iter().all(|segment| match segment {
            StructuralPathSegment::Field(identity) => !identity.is_empty(),
            StructuralPathSegment::FixedIndex(_) => true,
        })
}

fn append(
    structural_type: StructuralTypeId,
    prefix: &mut Vec<StructuralPathSegment>,
    moved: &[(&[StructuralPathSegment], StructuralTypeId)],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    max_residuals: usize,
    residuals: &mut Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>,
) -> Option<()> {
    let declaration = declarations.get(&structural_type)?;
    let mut matched = 0;
    match &declaration.shape {
        StructuralTypeShape::Record { fields } => {
            if fields.iter().any(|field| {
                field.relevance.is_erased()
                    || !matches!(
                        field.field_type,
                        StructuralFieldType::Structural(_)
                            | StructuralFieldType::Scalar(_)
                            | StructuralFieldType::IeeeFloat(_)
                            | StructuralFieldType::ByteSequence(
                                ByteSequenceCarrier::BoundedOwned { .. }
                            )
                    )
            }) {
                return None;
            }
            for field in fields.iter().rev() {
                let segment = StructuralPathSegment::Field(field.identity.clone());
                if let StructuralFieldType::Structural(child_type) = field.field_type {
                    matched += child(
                        child_type,
                        segment,
                        prefix,
                        moved,
                        declarations,
                        max_residuals,
                        residuals,
                    )?;
                } else if moved.iter().any(|(path, _)| path.first() == Some(&segment)) {
                    return None;
                }
            }
        }
        StructuralTypeShape::FixedArray { element, length } if *length > 0 => {
            // Every untouched child costs one output row; reject oversized
            // evidence before walking a potentially forged array dimension.
            if u128::from(*length) > (max_residuals - residuals.len()) as u128 + moved.len() as u128
            {
                return None;
            }
            for index in (0..*length).rev() {
                matched += child(
                    *element,
                    StructuralPathSegment::FixedIndex(index),
                    prefix,
                    moved,
                    declarations,
                    max_residuals,
                    residuals,
                )?;
            }
        }
        _ => return None,
    }
    (matched == moved.len()).then_some(())
}

fn child(
    structural_type: StructuralTypeId,
    segment: StructuralPathSegment,
    prefix: &mut Vec<StructuralPathSegment>,
    moved: &[(&[StructuralPathSegment], StructuralTypeId)],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    max_residuals: usize,
    residuals: &mut Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>,
) -> Option<usize> {
    let matching = moved
        .iter()
        .filter_map(|(path, leaf)| {
            let (first, rest) = path.split_first()?;
            (first == &segment).then_some((rest, *leaf))
        })
        .collect::<Vec<_>>();
    prefix.push(segment);
    if matching.is_empty() {
        if residuals.len() == max_residuals {
            return None;
        }
        residuals.push((prefix.clone(), structural_type));
    } else if matching.iter().all(|(path, _)| !path.is_empty()) {
        append(
            structural_type,
            prefix,
            &matching,
            declarations,
            max_residuals,
            residuals,
        )?;
    } else if matching.len() != 1 || matching[0].1 != structural_type {
        return None;
    }
    prefix.pop();
    Some(matching.len())
}
