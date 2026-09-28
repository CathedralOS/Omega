//! Independent finite partial-affine partition replay from retained types and moves.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    StructuralAffineDiscard, StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

pub(crate) fn exact_partial_cleanup_partition(
    declarations: &[StructuralTypeDeclaration],
    root_type: StructuralTypeId,
    moved: &[(&[StructuralPathSegment], StructuralTypeId)],
    residuals: &[&StructuralAffineDiscard],
) -> bool {
    if declarations.is_empty()
        || moved.is_empty()
        || declarations.windows(2).any(|pair| pair[0].id >= pair[1].id)
        || moved.iter().enumerate().any(|(index, (path, _))| {
            path.is_empty()
                || moved[..index]
                    .iter()
                    .any(|(earlier, _)| path.starts_with(earlier) || earlier.starts_with(path))
        })
    {
        return false;
    }
    let mut by_id = BTreeMap::new();
    let mut identities = BTreeSet::new();
    for declaration in declarations {
        if declaration.identity.is_empty()
            || !identities.insert(declaration.identity.as_str())
            || by_id.insert(declaration.id, declaration).is_some()
        {
            return false;
        }
    }
    if finite_cleanup_type(
        root_type,
        &by_id,
        &mut BTreeSet::new(),
        &mut BTreeSet::new(),
    )
    .is_none()
    {
        return false;
    }
    let mut expected = Vec::new();
    if append_expected_residuals(
        root_type,
        &[],
        moved,
        &by_id,
        residuals.len(),
        &mut expected,
    )
    .is_none()
        || expected.len() != residuals.len()
    {
        return false;
    }
    residuals
        .iter()
        .zip(expected)
        .all(|(actual, (path, structural_type))| {
            actual.path == path && actual.structural_type == structural_type
        })
}

fn finite_cleanup_type(
    structural_type: StructuralTypeId,
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    active: &mut BTreeSet<StructuralTypeId>,
    complete: &mut BTreeSet<StructuralTypeId>,
) -> Option<()> {
    if complete.contains(&structural_type) {
        return Some(());
    }
    if !active.insert(structural_type) {
        return None;
    }
    match &declarations.get(&structural_type)?.shape {
        StructuralTypeShape::Record { fields } => {
            let mut identities = BTreeSet::new();
            if fields.windows(2).any(|pair| pair[0].id >= pair[1].id) {
                return None;
            }
            for field in fields {
                if field.relevance.is_erased()
                    || field.identity.is_empty()
                    || !identities.insert(field.identity.as_str())
                {
                    return None;
                }
                match field.field_type {
                    StructuralFieldType::Structural(nested) => {
                        finite_cleanup_type(nested, declarations, active, complete)?;
                    }
                    StructuralFieldType::Scalar(_)
                    | StructuralFieldType::IeeeFloat(_)
                    | StructuralFieldType::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                    ) => {}
                    _ => return None,
                }
            }
        }
        StructuralTypeShape::FixedArray { element, length } if *length != 0 => {
            finite_cleanup_type(*element, declarations, active, complete)?;
        }
        _ => return None,
    }
    active.remove(&structural_type);
    complete.insert(structural_type);
    Some(())
}

fn append_expected_residuals(
    structural_type: StructuralTypeId,
    prefix: &[StructuralPathSegment],
    moved: &[(&[StructuralPathSegment], StructuralTypeId)],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    limit: usize,
    output: &mut Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>,
) -> Option<()> {
    if moved.is_empty() {
        if output.len() == limit {
            return None;
        }
        output.push((prefix.to_vec(), structural_type));
        return Some(());
    }
    if moved.iter().any(|(path, _)| path.is_empty()) {
        return (moved.len() == 1 && moved[0].1 == structural_type).then_some(());
    }
    let remaining = limit.checked_sub(output.len())?;
    let mut visit = |segment: StructuralPathSegment, child: StructuralTypeId| {
        let nested = moved
            .iter()
            .filter_map(|(path, leaf)| {
                (path.first() == Some(&segment)).then_some((&path[1..], *leaf))
            })
            .collect::<Vec<_>>();
        let mut path = prefix.to_vec();
        path.push(segment);
        append_expected_residuals(child, &path, &nested, declarations, limit, output)
    };
    match &declarations.get(&structural_type)?.shape {
        StructuralTypeShape::Record { fields } => {
            if moved.iter().any(|(path, _)| {
                !fields.iter().any(|field| {
                    matches!(&path[0], StructuralPathSegment::Field(identity)
                        if identity == &field.identity)
                        && matches!(field.field_type, StructuralFieldType::Structural(_))
                })
            }) {
                return None;
            }
            for field in fields.iter().rev() {
                if let StructuralFieldType::Structural(child) = field.field_type {
                    visit(StructuralPathSegment::Field(field.identity.clone()), child)?;
                }
            }
        }
        StructuralTypeShape::FixedArray { element, length } => {
            let touched = moved
                .iter()
                .map(|(path, _)| match path[0] {
                    StructuralPathSegment::FixedIndex(index) if index < *length => Some(index),
                    _ => None,
                })
                .collect::<Option<BTreeSet<_>>>()?;
            // Every untouched child contributes one maximal residual. Check this
            // lower bound before enumerating a dimension supplied by evidence.
            let untouched = length.checked_sub(u64::try_from(touched.len()).ok()?)?;
            if untouched > u64::try_from(remaining).ok()? {
                return None;
            }
            for index in (0..*length).rev() {
                visit(StructuralPathSegment::FixedIndex(index), *element)?;
            }
        }
        _ => return None,
    }
    Some(())
}
