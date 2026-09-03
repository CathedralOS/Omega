//! Exact partial-affine cleanup partition replay.
//!
//! This module reconstructs the residual field partition implied by moved
//! structural paths and compares it with retained cleanup evidence. It does not
//! choose moved paths, cleanup operations, or aggregate layouts.

use psi_core::StructuralTypeId;
use psi_terminal::{
    StructuralAffineDiscard, StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

pub(crate) fn exact_partial_cleanup_partition(
    declarations: &[StructuralTypeDeclaration],
    root_type: StructuralTypeId,
    moved: &[(&[StructuralPathSegment], StructuralTypeId)],
    residuals: &[&StructuralAffineDiscard],
) -> bool {
    if declarations.is_empty() || moved.is_empty() || residuals.is_empty() {
        return false;
    }
    let mut by_id = std::collections::BTreeMap::new();
    let mut identities = std::collections::BTreeSet::new();
    for declaration in declarations {
        if declaration.identity.is_empty()
            || !identities.insert(declaration.identity.as_str())
            || by_id.insert(declaration.id, declaration).is_some()
        {
            return false;
        }
    }
    if declarations.windows(2).any(|pair| pair[0].id >= pair[1].id) {
        return false;
    }
    let mut expected = Vec::new();
    if append_expected_partial_residuals(root_type, &[], moved, &by_id, &mut expected).is_none()
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

fn append_expected_partial_residuals(
    structural_type: StructuralTypeId,
    prefix: &[StructuralPathSegment],
    moved: &[(&[StructuralPathSegment], StructuralTypeId)],
    declarations: &std::collections::BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    output: &mut Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>,
) -> Option<()> {
    if prefix.is_empty()
        && moved.iter().all(|(path, _)| {
            matches!(
                path,
                [
                    StructuralPathSegment::FixedIndex(_),
                    StructuralPathSegment::FixedIndex(_)
                ]
            )
        })
    {
        let StructuralTypeShape::FixedArray { element, length: 2 } =
            declarations.get(&structural_type)?.shape
        else {
            return None;
        };
        let StructuralTypeShape::FixedArray {
            element: leaf,
            length: inner_length @ (3..=16),
        } = declarations.get(&element)?.shape
        else {
            return None;
        };
        if moved.len() != 2
            || moved.iter().any(|(_, moved_type)| *moved_type != leaf)
            || !matches!(
                declarations
                    .get(&leaf)
                    .map(|declaration| &declaration.shape),
                Some(StructuralTypeShape::Record { .. })
            )
        {
            return None;
        }
        let moved_by_outer = moved
            .iter()
            .filter_map(|(path, _)| match path {
                [
                    StructuralPathSegment::FixedIndex(outer @ (0 | 1)),
                    StructuralPathSegment::FixedIndex(inner),
                ] if *inner < inner_length => Some((*outer, *inner)),
                _ => None,
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        if moved_by_outer.len() != 2 {
            return None;
        }
        for outer in (0_u64..2).rev() {
            let moved_inner = *moved_by_outer.get(&outer)?;
            for inner in (0_u64..inner_length).rev() {
                if inner != moved_inner {
                    output.push((
                        vec![
                            StructuralPathSegment::FixedIndex(outer),
                            StructuralPathSegment::FixedIndex(inner),
                        ],
                        leaf,
                    ));
                }
            }
        }
        return Some(());
    }
    if prefix.is_empty()
        && moved
            .iter()
            .all(|(path, _)| matches!(path, [StructuralPathSegment::FixedIndex(_)]))
    {
        let declaration = declarations.get(&structural_type)?;
        let StructuralTypeShape::FixedArray { element, length } = declaration.shape else {
            return None;
        };
        if !matches!((length, moved.len()), (2, 1) | (3, 1 | 2) | (4, 2))
            || moved.iter().any(|(_, moved_type)| *moved_type != element)
            || !matches!(
                declarations
                    .get(&element)
                    .map(|declaration| &declaration.shape),
                Some(StructuralTypeShape::Record { .. })
            )
        {
            return None;
        }
        let moved_indexes = moved
            .iter()
            .filter_map(|(path, _)| match path {
                [StructuralPathSegment::FixedIndex(index)] if *index < length => Some(*index),
                _ => None,
            })
            .collect::<std::collections::BTreeSet<_>>();
        if moved_indexes.len() != moved.len() {
            return None;
        }
        let residuals = (0..length)
            .rev()
            .filter(|index| !moved_indexes.contains(index))
            .collect::<Vec<_>>();
        if residuals.is_empty() {
            return None;
        }
        output.extend(
            residuals
                .into_iter()
                .map(|residual| (vec![StructuralPathSegment::FixedIndex(residual)], element)),
        );
        return Some(());
    }
    let declaration = declarations.get(&structural_type)?;
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return None;
    };
    if fields.is_empty()
        || fields.iter().any(|field| {
            field.relevance.is_erased()
                || !matches!(
                    field.field_type,
                    StructuralFieldType::Structural(_)
                        | StructuralFieldType::Scalar(_)
                        | StructuralFieldType::IeeeFloat(_)
                        | StructuralFieldType::ByteSequence(
                            psi_terminal::ByteSequenceCarrier::BoundedOwned { .. }
                        )
                )
        })
    {
        return None;
    }
    let mut matched = 0_usize;
    for field in fields.iter().rev() {
        let matching = moved
            .iter()
            .filter(|(path, _)| {
                matches!(path.first(), Some(StructuralPathSegment::Field(identity))
                    if identity == &field.identity)
            })
            .copied()
            .collect::<Vec<_>>();
        matched += matching.len();
        let mut field_path = prefix.to_vec();
        field_path.push(StructuralPathSegment::Field(field.identity.clone()));
        let StructuralFieldType::Structural(field_type) = field.field_type else {
            if !matching.is_empty() {
                return None;
            }
            continue;
        };
        if matching.is_empty() {
            output.push((field_path, field_type));
            continue;
        }
        let whole = matching
            .iter()
            .filter(|(path, _)| path.len() == 1)
            .collect::<Vec<_>>();
        if !whole.is_empty() {
            if whole.len() != 1 || matching.len() != 1 || whole[0].1 != field_type {
                return None;
            }
            continue;
        }
        let nested = matching
            .iter()
            .map(|(path, leaf)| (&path[1..], *leaf))
            .collect::<Vec<_>>();
        append_expected_partial_residuals(field_type, &field_path, &nested, declarations, output)?;
    }
    (matched == moved.len()).then_some(())
}
