use super::*;

pub(super) fn partial_affine_residuals(
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    root_type: StructuralTypeId,
    moved_paths: &BTreeSet<Vec<StructuralPathSegment>>,
) -> Option<Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>> {
    if moved_paths.is_empty()
        || moved_paths
            .iter()
            .any(|path| !is_bounded_partial_affine_path(structural_types, root_type, path))
    {
        return None;
    }
    if moved_paths
        .iter()
        .all(|path| matches!(path.as_slice(), [StructuralPathSegment::FixedIndex(_)]))
    {
        let StructuralTypeShape::FixedArray { element, length } =
            structural_types.get(&root_type)?.shape
        else {
            return None;
        };
        if !matches!(
            structural_types
                .get(&element)
                .map(|declaration| &declaration.shape),
            Some(StructuralTypeShape::Record { .. })
        ) {
            return None;
        }
        if !matches!((length, moved_paths.len()), (2, 1) | (3, 1 | 2) | (4, 2)) {
            return None;
        }
        let moved = moved_paths
            .iter()
            .filter_map(|path| match path.as_slice() {
                [StructuralPathSegment::FixedIndex(index)] if *index < length => Some(*index),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        if moved.len() != moved_paths.len() {
            return None;
        }
        return Some(
            (0_u64..length)
                .rev()
                .filter(|index| !moved.contains(index))
                .map(|index| (vec![StructuralPathSegment::FixedIndex(index)], element))
                .collect(),
        );
    }
    if moved_paths.iter().all(|path| {
        matches!(
            path.as_slice(),
            [
                StructuralPathSegment::FixedIndex(_),
                StructuralPathSegment::FixedIndex(_)
            ]
        )
    }) {
        let StructuralTypeShape::FixedArray { element, length: 2 } =
            structural_types.get(&root_type)?.shape
        else {
            return None;
        };
        let StructuralTypeShape::FixedArray {
            element: leaf,
            length: inner_length @ (3..=16),
        } = structural_types.get(&element)?.shape
        else {
            return None;
        };
        if moved_paths.len() != 2
            || !matches!(
                structural_types
                    .get(&leaf)
                    .map(|declaration| &declaration.shape),
                Some(StructuralTypeShape::Record { .. })
            )
        {
            return None;
        }
        let moved = moved_paths
            .iter()
            .filter_map(|path| match path.as_slice() {
                [
                    StructuralPathSegment::FixedIndex(outer @ (0 | 1)),
                    StructuralPathSegment::FixedIndex(inner),
                ] if *inner < inner_length => Some((*outer, *inner)),
                _ => None,
            })
            .collect::<BTreeMap<_, _>>();
        if moved.len() != 2 {
            return None;
        }
        let mut residuals = Vec::with_capacity(usize::try_from(2 * (inner_length - 1)).ok()?);
        for outer in (0_u64..2).rev() {
            let moved_inner = *moved.get(&outer)?;
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
    if moved_paths.iter().enumerate().any(|(index, path)| {
        moved_paths
            .iter()
            .enumerate()
            .any(|(other_index, other)| index != other_index && path.starts_with(other))
    }) {
        return None;
    }
    let moved_paths = moved_paths.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let mut residuals = Vec::new();
    collect_partial_affine_residuals(
        structural_types,
        root_type,
        &moved_paths,
        &mut Vec::new(),
        &mut residuals,
    )?;
    Some(residuals)
}

pub(super) fn is_bounded_partial_affine_path(
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    root_type: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> bool {
    matches!(path, [StructuralPathSegment::Field(_), ..])
        || (matches!(path, [StructuralPathSegment::FixedIndex(_)])
            && structural_types.get(&root_type).is_some_and(|declaration| {
                matches!(
                    (&declaration.shape, path),
                    (
                        StructuralTypeShape::FixedArray { length: 2, .. },
                        [StructuralPathSegment::FixedIndex(0 | 1)]
                    ) | (
                        StructuralTypeShape::FixedArray { length: 3, .. },
                        [StructuralPathSegment::FixedIndex(0..=2)]
                    ) | (
                        StructuralTypeShape::FixedArray { length: 4, .. },
                        [StructuralPathSegment::FixedIndex(0..=3)]
                    )
                )
            }))
        || (matches!(path, [StructuralPathSegment::FixedIndex(_), StructuralPathSegment::FixedIndex(_)])
            && structural_types.get(&root_type).is_some_and(|declaration| {
                let StructuralTypeShape::FixedArray { length: 2, element } = declaration.shape else {
                    return false;
                };
                let Some(inner) = structural_types.get(&element) else {
                    return false;
                };
                let StructuralTypeShape::FixedArray { length: inner_length @ (3..=16), .. } = inner.shape else {
                    return false;
                };
                matches!(path, [StructuralPathSegment::FixedIndex(outer), StructuralPathSegment::FixedIndex(index)] if *outer < 2 && *index < inner_length)
            }))
}

pub(super) fn collect_partial_affine_residuals(
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
    moved_paths: &[&[StructuralPathSegment]],
    prefix: &mut Vec<StructuralPathSegment>,
    residuals: &mut Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>,
) -> Option<()> {
    let StructuralTypeShape::Record { fields } = &structural_types.get(&structural_type)?.shape
    else {
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
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                        )
                )
        })
    {
        return None;
    }
    let mut matched = 0_usize;
    for field in fields.iter().rev() {
        prefix.push(StructuralPathSegment::Field(field.identity.clone()));
        let descendants = moved_paths
            .iter()
            .filter_map(|path| match path {
                [StructuralPathSegment::Field(identity), remaining @ ..]
                    if *identity == field.identity =>
                {
                    Some(remaining)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        matched += descendants.len();
        let StructuralFieldType::Structural(field_type) = field.field_type else {
            if !descendants.is_empty() {
                return None;
            }
            prefix.pop();
            continue;
        };
        if descendants.is_empty() {
            residuals.push((prefix.clone(), field_type));
        } else if descendants.iter().all(|path| !path.is_empty()) {
            collect_partial_affine_residuals(
                structural_types,
                field_type,
                &descendants,
                prefix,
                residuals,
            )?;
        } else if descendants.len() != 1 {
            return None;
        }
        prefix.pop();
    }
    (matched == moved_paths.len()).then_some(())
}
