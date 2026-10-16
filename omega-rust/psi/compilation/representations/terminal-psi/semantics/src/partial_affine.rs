//! Type-directed maximal residual geometry for finite structural transfers.
//!
//! This query grants no custody or disposal authority. Its caller authenticates
//! the root, moved operands, and cleanup-free residual types independently.
//! The supplied output bound limits enumeration, including large fixed arrays.
use semantic_vocabulary::StructuralTypeId;
use std::collections::BTreeSet;
use terminal_psi::{
    StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
    find_structural_case, find_structural_type,
};

fn is_partial_affine_path(
    types: &[StructuralTypeDeclaration],
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> bool {
    !path.is_empty()
        && path.iter().all(|segment| {
            matches!(
                segment,
                StructuralPathSegment::Field(_)
                    | StructuralPathSegment::FixedIndex(_)
                    | StructuralPathSegment::Case(_)
            )
        })
        && crate::runtime_structural_path_tip(types.iter(), root, path).is_some()
}

/// The supplied cleanup bounds output work. An enormous array with a short
/// forged cleanup cannot make verification enumerate its absent residuals.
pub fn partial_affine_residuals(
    types: &[StructuralTypeDeclaration],
    root_type: StructuralTypeId,
    moved_paths: &BTreeSet<Vec<StructuralPathSegment>>,
    max_residuals: usize,
) -> Option<Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>> {
    if moved_paths.is_empty()
        || moved_paths
            .iter()
            .any(|path| !is_partial_affine_path(types, root_type, path))
        || moved_paths.iter().enumerate().any(|(index, path)| {
            moved_paths
                .iter()
                .enumerate()
                .any(|(other_index, other)| index != other_index && path.starts_with(other))
        })
    {
        return None;
    }
    let moved = moved_paths.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let mut residuals = Vec::new();
    let count = visit(
        types,
        root_type,
        &moved,
        &mut Vec::new(),
        max_residuals,
        &mut residuals,
        false,
    )?;
    if count > max_residuals {
        return None;
    }
    residuals.try_reserve_exact(count).ok()?;
    let emitted = visit(
        types,
        root_type,
        &moved,
        &mut Vec::new(),
        max_residuals,
        &mut residuals,
        true,
    )?;
    (emitted == count && residuals.len() == count).then_some(residuals)
}

fn visit(
    types: &[StructuralTypeDeclaration],
    current: StructuralTypeId,
    moved: &[&[StructuralPathSegment]],
    prefix: &mut Vec<StructuralPathSegment>,
    max_residuals: usize,
    residuals: &mut Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>,
    emit: bool,
) -> Option<usize> {
    let declaration = find_structural_type(types, current)?;
    let mut count = 0usize;
    match &declaration.shape {
        StructuralTypeShape::Record { fields, .. } => {
            // Byte predicates constrain the payload, not its disposal. An
            // inline bounded owner remains cleanup-free; a borrowed view does
            // not gain that status merely by carrying the same predicate.
            if fields.is_empty() {
                return None;
            }
            count = fields_residuals(types, fields, moved, prefix, max_residuals, residuals, emit)?;
        }
        StructuralTypeShape::FixedArray { element, length } if *length != 0 => {
            let mut touched = moved
                .iter()
                .map(|path| match path.first()? {
                    StructuralPathSegment::FixedIndex(index) if index < length => Some(*index),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?;
            touched.sort_unstable();
            touched.dedup();
            if !emit {
                count = usize::try_from(*length).ok()?.checked_sub(touched.len())?;
            }
            let indexes = (0..if emit { *length } else { 0 })
                .rev()
                .chain(touched.into_iter().rev().filter(|_| !emit));
            for index in indexes {
                count = count.checked_add(child(
                    types,
                    *element,
                    StructuralPathSegment::FixedIndex(index),
                    moved,
                    prefix,
                    max_residuals,
                    residuals,
                    emit,
                )?)?;
                if count > max_residuals {
                    return None;
                }
            }
        }
        StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
            // A Case step names a selected namespace, not a child
            // value. Only that case has live payload siblings. Never enumerate
            // inactive alternatives or merge different selected cases.
            let selected_case = moved.iter().find_map(|path| match path.first() {
                Some(StructuralPathSegment::Case(case)) => Some(*case),
                _ => None,
            })?;
            let selected = find_structural_case(cases, selected_case)?;
            if moved.iter().any(|path| match path.first() {
                Some(StructuralPathSegment::Case(case)) => *case != selected_case,
                Some(StructuralPathSegment::Field(_)) => {
                    declaration.shape.direct_fields().is_none()
                }
                _ => true,
            }) {
                return None;
            }
            let payload_moves = moved
                .iter()
                .filter_map(|path| {
                    matches!(path.first(), Some(StructuralPathSegment::Case(_)))
                        .then_some(&path[1..])
                })
                .collect::<Vec<_>>();
            prefix.push(StructuralPathSegment::Case(selected_case));
            count = fields_residuals(
                types,
                &selected.fields,
                &payload_moves,
                prefix,
                max_residuals,
                residuals,
                emit,
            )?;
            prefix.pop();
            if let Some(common) = declaration.shape.direct_fields() {
                let common_moves = moved
                    .iter()
                    .copied()
                    .filter(|path| matches!(path.first(), Some(StructuralPathSegment::Field(_))))
                    .collect::<Vec<_>>();
                count = count.checked_add(fields_residuals(
                    types,
                    common,
                    &common_moves,
                    prefix,
                    max_residuals,
                    residuals,
                    emit,
                )?)?;
            }
        }
        _ => return None,
    }
    (count <= max_residuals).then_some(count)
}

fn fields_residuals(
    types: &[StructuralTypeDeclaration],
    fields: &[terminal_psi::StructuralFieldDeclaration],
    moved: &[&[StructuralPathSegment]],
    prefix: &mut Vec<StructuralPathSegment>,
    max_residuals: usize,
    residuals: &mut Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>,
    emit: bool,
) -> Option<usize> {
    // Inline descriptors carry disposal just like structural children. Their
    // declaration describes the descriptor, never the borrowed backing. Scalar
    // restrictions and inline owned bytes introduce no disposal row.
    let mut count = 0_usize;
    for field in fields.iter().rev() {
        if field.relevance.is_erased() {
            return None;
        }
        let child_type = match &field.field_type {
            StructuralFieldType::Structural(child) => Some(*child),
            scalar if scalar.scalar_type().is_some() => None,
            bytes => match bytes.byte_sequence_carrier()? {
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. } => None,
                carrier => Some(crate::byte_carrier_field_declaration(types.iter(), carrier)?.id),
            },
        };
        if let Some(child_type) = child_type {
            count = count.checked_add(child(
                types,
                child_type,
                StructuralPathSegment::Field(field.identity.clone()),
                moved,
                prefix,
                max_residuals,
                residuals,
                emit,
            )?)?;
        }
    }
    (count <= max_residuals).then_some(count)
}

fn child(
    types: &[StructuralTypeDeclaration],
    child_type: StructuralTypeId,
    segment: StructuralPathSegment,
    moved: &[&[StructuralPathSegment]],
    prefix: &mut Vec<StructuralPathSegment>,
    max_residuals: usize,
    residuals: &mut Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>,
    emit: bool,
) -> Option<usize> {
    let descendants = moved
        .iter()
        .filter_map(|path| {
            let (head, tail) = path.split_first()?;
            (head == &segment).then_some(tail)
        })
        .collect::<Vec<_>>();
    prefix.push(segment);
    let count = if descendants.is_empty() {
        if emit {
            if residuals.len() == max_residuals {
                return None;
            }
            residuals.push((prefix.clone(), child_type));
        }
        1
    } else if descendants.iter().all(|path| !path.is_empty()) {
        visit(
            types,
            child_type,
            &descendants,
            prefix,
            max_residuals,
            residuals,
            emit,
        )?
    } else if descendants.len() != 1 {
        return None;
    } else {
        0
    };
    prefix.pop();
    Some(count)
}
