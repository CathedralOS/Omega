//! Maximal residual-subtree reconstruction after partial structural moves.

use std::collections::{BTreeMap, BTreeSet};

use psi_core::StructuralTypeId;
use psi_terminal::{
    ByteSequenceCarrier, StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

pub(in crate::lowering) fn expected_maximal_residual_subtrees(
    root_type: StructuralTypeId,
    moved: &[(Vec<StructuralPathSegment>, StructuralTypeId)],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Option<Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>> {
    if moved.is_empty() {
        return None;
    }
    if moved
        .iter()
        .all(|(path, _)| matches!(path.as_slice(), [StructuralPathSegment::FixedIndex(_)]))
    {
        let declaration = declarations.get(&root_type).copied()?;
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
            .filter_map(|(path, _)| match path.as_slice() {
                [StructuralPathSegment::FixedIndex(index)] if *index < length => Some(*index),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        if moved_indexes.len() != moved.len() {
            return None;
        }
        let residuals = (0..length)
            .rev()
            .filter(|index| !moved_indexes.contains(index))
            .map(|index| (vec![StructuralPathSegment::FixedIndex(index)], element))
            .collect::<Vec<_>>();
        return (!residuals.is_empty()).then_some(residuals);
    }
    if moved.iter().all(|(path, _)| {
        matches!(
            path.as_slice(),
            [
                StructuralPathSegment::FixedIndex(_),
                StructuralPathSegment::FixedIndex(_)
            ]
        )
    }) {
        let StructuralTypeShape::FixedArray { element, length: 2 } =
            declarations.get(&root_type)?.shape
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
            .filter_map(|(path, _)| match path.as_slice() {
                [
                    StructuralPathSegment::FixedIndex(outer @ (0 | 1)),
                    StructuralPathSegment::FixedIndex(inner),
                ] if *inner < inner_length => Some((*outer, *inner)),
                _ => None,
            })
            .collect::<BTreeMap<_, _>>();
        if moved_by_outer.len() != 2 {
            return None;
        }
        let mut residuals = Vec::with_capacity(usize::try_from(2 * (inner_length - 1)).ok()?);
        for outer in (0_u64..2).rev() {
            let moved_inner = *moved_by_outer.get(&outer)?;
            for inner in (0_u64..inner_length).rev() {
                if inner != moved_inner {
                    residuals.push((
                        vec![
                            StructuralPathSegment::FixedIndex(outer),
                            StructuralPathSegment::FixedIndex(inner),
                        ],
                        leaf,
                    ));
                }
            }
        }
        return Some(residuals);
    }
    let borrowed = moved
        .iter()
        .map(|(path, structural_type)| (path.as_slice(), *structural_type))
        .collect::<Vec<_>>();
    let mut residuals = Vec::new();
    append_maximal_residual_subtrees(root_type, &[], &borrowed, declarations, &mut residuals)?;
    (!residuals.is_empty()).then_some(residuals)
}

pub(in crate::lowering) fn is_partial_cleanup_path(path: &[StructuralPathSegment]) -> bool {
    (!path.is_empty()
        && path.iter().all(
            |segment| matches!(segment, StructuralPathSegment::Field(identity) if !identity.is_empty()),
        )) || matches!(
        path,
        [StructuralPathSegment::FixedIndex(0..=3)]
            | [
                StructuralPathSegment::FixedIndex(0 | 1),
                StructuralPathSegment::FixedIndex(0..=14),
            ]
    )
}

fn append_maximal_residual_subtrees(
    structural_type: StructuralTypeId,
    prefix: &[StructuralPathSegment],
    moved: &[(&[StructuralPathSegment], StructuralTypeId)],
    declarations: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    residuals: &mut Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>,
) -> Option<()> {
    let declaration = declarations.get(&structural_type).copied()?;
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
                            ByteSequenceCarrier::BoundedOwned { .. }
                        )
                )
        })
        || moved
            .iter()
            .any(|(path, _)| !matches!(path.first(), Some(StructuralPathSegment::Field(_))))
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
            residuals.push((field_path, field_type));
            continue;
        }
        let whole_moves = matching
            .iter()
            .filter(|(path, _)| path.len() == 1)
            .collect::<Vec<_>>();
        if !whole_moves.is_empty() {
            if whole_moves.len() != 1 || matching.len() != 1 || whole_moves[0].1 != field_type {
                return None;
            }
            continue;
        }
        let nested = matching
            .iter()
            .map(|(path, leaf_type)| (&path[1..], *leaf_type))
            .collect::<Vec<_>>();
        append_maximal_residual_subtrees(
            field_type,
            &field_path,
            &nested,
            declarations,
            residuals,
        )?;
    }
    (matched == moved.len()).then_some(())
}
