use semantic_vocabulary::{StructuralCaseId, StructuralTypeId};
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

/// Tests typed coverage of a receiving region, not allocation or access
/// authority. A live sibling or missing ancestor cannot be erased by a
/// store into this region. Callers retain their own home and loan checks.
pub fn structural_region_is_vacant(
    types: &[StructuralTypeDeclaration],
    root: StructuralTypeId,
    missing: &BTreeSet<Vec<StructuralPathSegment>>,
    region: &[StructuralPathSegment],
    admitted_case: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
) -> bool {
    let Some(selected) = crate::runtime_structural_path_tip(types.iter(), root, region) else {
        return false;
    };
    if missing.iter().any(|absent| {
        crate::structural_paths_may_overlap(absent, region) && !absent.starts_with(region)
    }) {
        return false;
    }
    if missing.contains(region) {
        return true;
    }
    let children = missing
        .iter()
        .filter_map(|absent| absent.strip_prefix(region).map(<[_]>::to_vec))
        .collect();
    // The counting pass proves an empty typed complement before any
    // output is allocated or untouched array elements are enumerated.
    partition(types, selected.id, &children, None, 0, &|path, case| {
        let mut selected = region.to_vec();
        selected.extend_from_slice(path);
        admitted_case(&selected, case)
    })
    .is_some_and(|residuals| residuals.is_empty())
}

pub fn partial_affine_residuals(
    types: &[StructuralTypeDeclaration],
    root_type: StructuralTypeId,
    moved_paths: &BTreeSet<Vec<StructuralPathSegment>>,
    max_residuals: usize,
) -> Option<Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>> {
    partition(
        types,
        root_type,
        moved_paths,
        None,
        max_residuals,
        &|_, _| false,
    )
}

/// Keeps only live residuals within the requested regions. Reconverging
/// arrivals use the successor's vacancies as regions and their own moves
/// as exclusions. The existing typed walk retains declaration order and
/// proves the output bound before enumerating array elements.
pub fn partial_affine_residuals_in_regions(
    types: &[StructuralTypeDeclaration],
    root_type: StructuralTypeId,
    moved_paths: &BTreeSet<Vec<StructuralPathSegment>>,
    regions: &BTreeSet<Vec<StructuralPathSegment>>,
    max_residuals: usize,
) -> Option<Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>> {
    partition(
        types,
        root_type,
        moved_paths,
        Some(regions),
        max_residuals,
        &|_, _| false,
    )
}

fn partition(
    types: &[StructuralTypeDeclaration],
    root_type: StructuralTypeId,
    moved_paths: &BTreeSet<Vec<StructuralPathSegment>>,
    regions: Option<&BTreeSet<Vec<StructuralPathSegment>>>,
    max_residuals: usize,
    admitted_case: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
) -> Option<Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>> {
    find_structural_type(types, root_type)?;
    let valid = |paths: &BTreeSet<Vec<StructuralPathSegment>>, allow_root| {
        paths.iter().all(|path| {
            allow_root && path.is_empty() || is_partial_affine_path(types, root_type, path)
        }) && !paths.iter().enumerate().any(|(index, path)| {
            paths
                .iter()
                .enumerate()
                .any(|(other_index, other)| index != other_index && path.starts_with(other))
        })
    };
    if moved_paths.is_empty() && regions.is_none()
        || !valid(moved_paths, regions.is_some())
        || regions.is_some_and(|paths| !valid(paths, true))
    {
        return None;
    }
    if moved_paths.iter().any(Vec::is_empty) || regions.is_some_and(BTreeSet::is_empty) {
        return Some(Vec::new());
    }
    let moved = moved_paths.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let regions = regions.map(|paths| paths.iter().map(Vec::as_slice).collect::<Vec<_>>());
    let regions = regions
        .as_deref()
        .filter(|paths| !paths.iter().any(|path| path.is_empty()));
    if moved.is_empty() && regions.is_none() {
        return (max_residuals != 0).then(|| vec![(Vec::new(), root_type)]);
    }
    let mut residuals = Vec::new();
    let count = visit(
        types,
        root_type,
        &moved,
        regions,
        &mut Vec::new(),
        max_residuals,
        &mut residuals,
        false,
        admitted_case,
    )?;
    if count > max_residuals {
        return None;
    }
    residuals.try_reserve_exact(count).ok()?;
    let emitted = visit(
        types,
        root_type,
        &moved,
        regions,
        &mut Vec::new(),
        max_residuals,
        &mut residuals,
        true,
        admitted_case,
    )?;
    (emitted == count && residuals.len() == count).then_some(residuals)
}

fn visit(
    types: &[StructuralTypeDeclaration],
    current: StructuralTypeId,
    moved: &[&[StructuralPathSegment]],
    regions: Option<&[&[StructuralPathSegment]]>,
    prefix: &mut Vec<StructuralPathSegment>,
    max_residuals: usize,
    residuals: &mut Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>,
    emit: bool,
    admitted_case: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
) -> Option<usize> {
    if regions.is_some_and(<[_]>::is_empty) {
        return Some(0);
    }
    let declaration = find_structural_type(types, current)?;
    let mut count = 0usize;
    match &declaration.shape {
        StructuralTypeShape::Record { fields, .. } => {
            if fields.is_empty() {
                return None;
            }
            count = fields_residuals(
                types,
                fields,
                moved,
                regions,
                prefix,
                max_residuals,
                residuals,
                emit,
                admitted_case,
            )?;
        }
        StructuralTypeShape::FixedArray {
            element,
            length: terminal_psi::ArrayExtent::Literal(length),
        } if *length != 0 => {
            let mut touched = moved
                .iter()
                .map(|path| match path.first()? {
                    StructuralPathSegment::FixedIndex(index) if index < length => Some(*index),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?;
            touched.sort_unstable();
            touched.dedup();
            if !emit && regions.is_none() {
                count = usize::try_from(*length).ok()?.checked_sub(touched.len())?;
            }
            let mut requested = if let Some(regions) = regions {
                regions
                    .iter()
                    .map(|path| match path.first()? {
                        StructuralPathSegment::FixedIndex(index) if index < length => {
                            Some(*index)
                        }
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>()?
            } else {
                touched
            };
            requested.sort_unstable();
            requested.dedup();
            let indexes = (0..if emit && regions.is_none() {
                *length
            } else {
                0
            })
                .rev()
                .chain(
                    requested
                        .into_iter()
                        .rev()
                        .filter(|_| regions.is_some() || !emit),
                );
            for index in indexes {
                count = count.checked_add(child(
                    types,
                    *element,
                    StructuralPathSegment::FixedIndex(index),
                    moved,
                    regions,
                    prefix,
                    max_residuals,
                    residuals,
                    emit,
                    admitted_case,
                )?)?;
                if count > max_residuals {
                    return None;
                }
            }
        }
        StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
            // A common-field cleanup does not observe the case. Retain
            // the payload namespace only when the receiving scope asks
            // for it; a requested payload still needs actual membership.
            let payload_regions = regions.map(|paths| {
                paths
                    .iter()
                    .copied()
                    .filter(|path| matches!(path.first(), Some(StructuralPathSegment::Case(_))))
                    .collect::<Vec<_>>()
            });
            if payload_regions
                .as_ref()
                .is_none_or(|paths| !paths.is_empty())
            {
                let selected_case = moved
                    .iter()
                    .find_map(|path| match path.first() {
                        Some(StructuralPathSegment::Case(case)) => Some(*case),
                        _ => None,
                    })
                    .or_else(|| {
                        let mut matching =
                            cases.iter().filter(|case| admitted_case(prefix, case.id));
                        let case = matching.next()?;
                        matching.next().is_none().then_some(case.id)
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
                let case_regions = region_children(
                    payload_regions.as_deref(),
                    StructuralPathSegment::Case(selected_case),
                );
                count = fields_residuals(
                    types,
                    &selected.fields,
                    &payload_moves,
                    case_regions.as_deref(),
                    prefix,
                    max_residuals,
                    residuals,
                    emit,
                    admitted_case,
                )?;
                prefix.pop();
            }
            if let Some(common) = declaration.shape.direct_fields() {
                let common_moves = moved
                    .iter()
                    .copied()
                    .filter(|path| {
                        matches!(path.first(), Some(StructuralPathSegment::Field(_)))
                    })
                    .collect::<Vec<_>>();
                count = count.checked_add(fields_residuals(
                    types,
                    common,
                    &common_moves,
                    regions,
                    prefix,
                    max_residuals,
                    residuals,
                    emit,
                    admitted_case,
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
    regions: Option<&[&[StructuralPathSegment]]>,
    prefix: &mut Vec<StructuralPathSegment>,
    max_residuals: usize,
    residuals: &mut Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>,
    emit: bool,
    admitted_case: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
) -> Option<usize> {
    let mut count = 0_usize;
    for field in fields.iter().rev() {
        // A scoped cleanup does not inspect presentation or erasure of
        // sibling storage it was not asked to dispose.
        if region_children(regions, StructuralPathSegment::Field(field.identity))
            .as_ref()
            .is_some_and(Vec::is_empty)
        {
            continue;
        }
        if field.relevance.is_erased() {
            return None;
        }
        let child_type = match &field.field_type {
            StructuralFieldType::Structural(child) => Some(*child),
            scalar if scalar.scalar_type().is_some() => None,
            bytes => match bytes.byte_sequence_carrier()? {
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. } => None,
                _ => Some(crate::inline_carrier_declaration(types.iter(), field)?.id),
            },
        };
        if let Some(child_type) = child_type {
            count = count.checked_add(child(
                types,
                child_type,
                StructuralPathSegment::Field(field.identity),
                moved,
                regions,
                prefix,
                max_residuals,
                residuals,
                emit,
                admitted_case,
            )?)?;
        }
    }
    (count <= max_residuals).then_some(count)
}

fn region_children<'a>(
    regions: Option<&[&'a [StructuralPathSegment]]>,
    segment: StructuralPathSegment,
) -> Option<Vec<&'a [StructuralPathSegment]>> {
    let paths = regions?
        .iter()
        .filter_map(|path| {
            let (head, tail) = path.split_first()?;
            (*head == segment).then_some(tail)
        })
        .collect::<Vec<_>>();
    (!paths.iter().any(|path| path.is_empty())).then_some(paths)
}

fn child(
    types: &[StructuralTypeDeclaration],
    child_type: StructuralTypeId,
    segment: StructuralPathSegment,
    moved: &[&[StructuralPathSegment]],
    regions: Option<&[&[StructuralPathSegment]]>,
    prefix: &mut Vec<StructuralPathSegment>,
    max_residuals: usize,
    residuals: &mut Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>,
    emit: bool,
    admitted_case: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
) -> Option<usize> {
    let regions = region_children(regions, segment.clone());
    if regions.as_ref().is_some_and(Vec::is_empty) {
        return Some(0);
    }
    let descendants = moved
        .iter()
        .filter_map(|path| {
            let (head, tail) = path.split_first()?;
            (head == &segment).then_some(tail)
        })
        .collect::<Vec<_>>();
    prefix.push(segment);
    let count = if descendants.is_empty() && regions.is_none() {
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
            regions.as_deref(),
            prefix,
            max_residuals,
            residuals,
            emit,
            admitted_case,
        )?
    } else if descendants.len() != 1 {
        return None;
    } else {
        0
    };
    prefix.pop();
    Some(count)
}
