//! Type-directed maximal residual complement for finite structural transfers.
//!
//! Only the complement is validated: a partially touched node must be a
//! record, array, or exactly selected case whose fields are partial-affine
//! leaves; each untouched subtree must be a finite tree of them. Closed alternatives
//! remain whole when every alternative is custody-free; this does not select
//! a case or authorize disposal of resource-bearing case payloads. A touched
//! case instead retains only its selected payload siblings and common fields.
//! Membership and nominal-hook admission remain the caller's responsibility.
//! A moved subtree transfers whole with its own custody, so its shape (references included) is the
//! consumer's concern, not this discard roster's.
use super::{
    BTreeMap, CheckedUnitPartialAffineDiscardPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralFieldType, CheckedUnitStructuralPathSegment,
    CheckedUnitStructuralTypePlan, CheckedUnitStructuralTypeShape,
};
use crate::execution::terminal_unit::cleanup::partial_affine_cleanup::is_partial_affine_field_type;

fn validate_type(
    types: &BTreeMap<String, CheckedUnitStructuralTypePlan>,
    current_type: &str,
    active: &mut Vec<String>,
    complete: &mut Vec<String>,
) -> Option<()> {
    if complete.iter().any(|identity| identity == current_type) {
        return Some(());
    }
    if active.iter().any(|identity| identity == current_type) {
        return None;
    }
    let declaration = types.get(current_type)?;
    active.push(current_type.to_owned());
    match &declaration.shape {
        CheckedUnitStructuralTypeShape::Record { fields, .. } => {
            for (index, field) in fields.iter().enumerate() {
                if field.identity.is_empty()
                    || field.relevance.is_erased()
                    || !is_partial_affine_field_type(&field.field_type)
                    || fields[..index]
                        .iter()
                        .any(|earlier| earlier.identity == field.identity)
                {
                    return None;
                }
                if let CheckedUnitStructuralFieldType::Structural { type_identity } =
                    &field.field_type
                {
                    validate_type(types, type_identity, active, complete)?;
                }
            }
        }
        CheckedUnitStructuralTypeShape::FixedArray {
            element_type_identity,
            length,
        } => {
            // Empty arrays contain no values whose cleanup eligibility can
            // constrain this complement. They retain their own exact shape,
            // just as an empty record does, without visiting an absent element.
            if *length > 0 {
                validate_type(types, element_type_identity, active, complete)?;
            }
        }
        // A residual loan ends its stored descriptor, never its referent.
        CheckedUnitStructuralTypeShape::Reference {
            referent_identity,
            access,
        } if *access != checked_trees::CheckedStructuralAccess::Owned
            && types.contains_key(referent_identity) => {}
        CheckedUnitStructuralTypeShape::Sum { cases } => {
            validate_inline_alternatives(&[], cases)?;
        }
        CheckedUnitStructuralTypeShape::Mixed { fields, cases } => {
            validate_inline_alternatives(fields, cases)?;
        }
        _ => return None,
    }
    active.pop();
    complete.push(current_type.to_owned());
    Some(())
}

/// No selected-case authority is needed to discard a complete inline value.
/// Apply the existing custody-free field rule to every closed alternative;
/// structural payloads retain their separate case-dependent cleanup contract.
fn validate_inline_alternatives(
    common: &[checked_trees::CheckedUnitStructuralFieldPlan],
    cases: &[checked_trees::CheckedUnitStructuralCasePlan],
) -> Option<()> {
    let fields_are_inline = |fields: &[checked_trees::CheckedUnitStructuralFieldPlan]| {
        fields.iter().enumerate().all(|(position, field)| {
            !field.identity.is_empty()
                && !field.relevance.is_erased()
                && is_partial_affine_field_type(&field.field_type)
                && !matches!(
                    field.field_type,
                    CheckedUnitStructuralFieldType::Structural { .. }
                )
                && !fields[..position]
                    .iter()
                    .any(|earlier| earlier.identity == field.identity)
        })
    };
    (!cases.is_empty()
        && fields_are_inline(common)
        && cases.iter().enumerate().all(|(position, case)| {
            !case.identity.is_empty()
                && !cases[..position]
                    .iter()
                    .any(|earlier| earlier.identity == case.identity)
                && fields_are_inline(&case.fields)
        }))
    .then_some(())
}

/// Count first, traversing only touched array children. Thus a huge dimension
/// cannot trigger enumeration before host-size and output-allocation checks.
fn visit(
    types: &BTreeMap<String, CheckedUnitStructuralTypePlan>,
    source: &CheckedUnitStructuralArgumentSourcePlan,
    current_type: &str,
    moved_paths: &[(&[CheckedUnitStructuralPathSegment], &str)],
    boundaries: &[(&[CheckedUnitStructuralPathSegment], &str)],
    prefix: &mut Vec<CheckedUnitStructuralPathSegment>,
    residuals: &mut Vec<CheckedUnitPartialAffineDiscardPlan>,
    emit: bool,
) -> Option<usize> {
    if moved_paths.is_empty() && boundaries.is_empty() {
        // Only the complement is discarded here, so only the complement must
        // be a finite record/array tree of cleanup-free leaves. A moved
        // subtree leaves whole with its own custody and is never inspected.
        validate_type(types, current_type, &mut Vec::new(), &mut Vec::new())?;
        if emit {
            residuals.push(CheckedUnitPartialAffineDiscardPlan {
                source: source.clone(),
                path: prefix.clone(),
                type_identity: current_type.to_owned(),
            });
        }
        return Some(1);
    }
    if moved_paths.iter().any(|(path, _)| path.is_empty()) {
        return (moved_paths.len() == 1 && moved_paths[0].1 == current_type).then_some(0);
    }
    let nominal_boundary = boundaries.iter().any(|(path, _)| path.is_empty());
    if nominal_boundary
        && (!moved_paths.is_empty()
            || boundaries
                .iter()
                .any(|(path, identity)| path.is_empty() && *identity != current_type))
    {
        return None;
    }
    // A nominal boundary is the owner's hook, not a cut that suppresses its
    // structural children. The hook receives the intact owner first; only its
    // normal return permits recursive reverse-declaration cleanup. Keep that
    // ordering in this same traversal rather than flattening hooks into a set
    // of disjoint leaves or disposing the whole receiver twice.
    if nominal_boundary && emit {
        residuals.push(CheckedUnitPartialAffineDiscardPlan {
            source: source.clone(),
            path: prefix.clone(),
            type_identity: current_type.to_owned(),
        });
    }
    let declaration = types.get(current_type)?;
    let mut count = usize::from(nominal_boundary);
    match &declaration.shape {
        CheckedUnitStructuralTypeShape::Record { fields, .. } => {
            let child_boundaries = boundaries
                .iter()
                .copied()
                .filter(|(path, _)| !path.is_empty())
                .collect::<Vec<_>>();
            count = count.checked_add(visit_fields(
                types,
                source,
                fields,
                moved_paths,
                &child_boundaries,
                prefix,
                residuals,
                emit,
            )?)?;
        }
        CheckedUnitStructuralTypeShape::FixedArray {
            element_type_identity,
            length,
        } => {
            let mut touched = Vec::new();
            for (path, _) in moved_paths {
                let CheckedUnitStructuralPathSegment::FixedIndex(index) = path[0] else {
                    return None;
                };
                if index >= *length {
                    return None;
                }
                touched.push(index);
            }
            for (path, _) in boundaries {
                let [CheckedUnitStructuralPathSegment::FixedIndex(index), ..] = *path else {
                    return None;
                };
                if *index >= *length {
                    return None;
                }
                touched.push(*index);
            }
            touched.sort_unstable();
            touched.dedup();
            let untouched = usize::try_from(*length).ok()?.checked_sub(touched.len())?;
            if !emit {
                count = count.checked_add(untouched)?;
            }
            // Enumeration is needed only when writing the already-sized output.
            let indexes = (0..if emit { *length } else { 0 })
                .rev()
                .chain(touched.into_iter().rev().filter(|_| !emit));
            for index in indexes {
                let matching = moved_paths.iter().filter_map(|(path, moved_type)| {
                    matches!(path[0], CheckedUnitStructuralPathSegment::FixedIndex(candidate) if candidate == index)
                        .then_some((&path[1..], *moved_type))
                }).collect::<Vec<_>>();
                let child_boundaries = boundaries.iter().filter_map(|(path, boundary_type)| {
                    let (segment, remaining) = path.split_first()?;
                    matches!(segment, CheckedUnitStructuralPathSegment::FixedIndex(candidate) if *candidate == index)
                        .then_some((remaining, *boundary_type))
                }).collect::<Vec<_>>();
                prefix.push(CheckedUnitStructuralPathSegment::FixedIndex(index));
                count = count.checked_add(visit(
                    types,
                    source,
                    element_type_identity,
                    &matching,
                    &child_boundaries,
                    prefix,
                    residuals,
                    emit,
                )?)?;
                prefix.pop();
            }
        }
        CheckedUnitStructuralTypeShape::Sum { cases }
        | CheckedUnitStructuralTypeShape::Mixed { cases, .. } => {
            let selected_case =
                moved_paths
                    .iter()
                    .chain(boundaries)
                    .find_map(|(path, _)| match path.first() {
                        Some(CheckedUnitStructuralPathSegment::Case(identity)) => Some(identity),
                        _ => None,
                    })?;
            let selected = cases.iter().find(|case| &case.identity == selected_case)?;
            let common = match &declaration.shape {
                CheckedUnitStructuralTypeShape::Mixed { fields, .. } => fields.as_slice(),
                _ => &[],
            };
            if moved_paths
                .iter()
                .chain(boundaries)
                .any(|(path, _)| match path.first() {
                    Some(CheckedUnitStructuralPathSegment::Case(identity)) => {
                        identity != selected_case
                    }
                    Some(CheckedUnitStructuralPathSegment::Field(_)) => !matches!(
                        declaration.shape,
                        CheckedUnitStructuralTypeShape::Mixed { .. }
                    ),
                    None => !nominal_boundary,
                    _ => true,
                })
            {
                return None;
            }
            let payload_moves = moved_paths
                .iter()
                .filter(|(path, _)| {
                    matches!(
                        path.first(),
                        Some(CheckedUnitStructuralPathSegment::Case(_))
                    )
                })
                .map(|(path, identity)| (&path[1..], *identity))
                .collect::<Vec<_>>();
            let payload_boundaries = boundaries
                .iter()
                .filter(|(path, _)| {
                    matches!(
                        path.first(),
                        Some(CheckedUnitStructuralPathSegment::Case(_))
                    )
                })
                .map(|(path, identity)| (&path[1..], *identity))
                .collect::<Vec<_>>();
            prefix.push(CheckedUnitStructuralPathSegment::Case(
                selected_case.clone(),
            ));
            count = count.checked_add(visit_fields(
                types,
                source,
                &selected.fields,
                &payload_moves,
                &payload_boundaries,
                prefix,
                residuals,
                emit,
            )?)?;
            prefix.pop();
            let common_moves = moved_paths
                .iter()
                .copied()
                .filter(|(path, _)| {
                    matches!(
                        path.first(),
                        Some(CheckedUnitStructuralPathSegment::Field(_))
                    )
                })
                .collect::<Vec<_>>();
            let common_boundaries = boundaries
                .iter()
                .copied()
                .filter(|(path, _)| {
                    matches!(
                        path.first(),
                        Some(CheckedUnitStructuralPathSegment::Field(_))
                    )
                })
                .collect::<Vec<_>>();
            count = count.checked_add(visit_fields(
                types,
                source,
                common,
                &common_moves,
                &common_boundaries,
                prefix,
                residuals,
                emit,
            )?)?;
        }
        _ => return None,
    }
    Some(count)
}

/// Walk one declaration-ordered namespace. Every move and hook must resolve
/// to its exact structural child; scalar width never substitutes for custody.
fn visit_fields(
    types: &BTreeMap<String, CheckedUnitStructuralTypePlan>,
    source: &CheckedUnitStructuralArgumentSourcePlan,
    fields: &[checked_trees::CheckedUnitStructuralFieldPlan],
    moved_paths: &[(&[CheckedUnitStructuralPathSegment], &str)],
    boundaries: &[(&[CheckedUnitStructuralPathSegment], &str)],
    prefix: &mut Vec<CheckedUnitStructuralPathSegment>,
    residuals: &mut Vec<CheckedUnitPartialAffineDiscardPlan>,
    emit: bool,
) -> Option<usize> {
    if fields.iter().enumerate().any(|(position, field)| {
        field.identity.is_empty()
            || field.relevance.is_erased()
            || !is_partial_affine_field_type(&field.field_type)
            || fields[..position]
                .iter()
                .any(|earlier| earlier.identity == field.identity)
    }) {
        return None;
    }
    let mut count = 0_usize;
    let mut matched = 0_usize;
    let mut matched_boundaries = 0_usize;
    for field in fields.iter().rev() {
        let matching = moved_paths.iter().filter_map(|(path, identity)| {
            let (segment, remaining) = path.split_first()?;
            matches!(segment, CheckedUnitStructuralPathSegment::Field(candidate) if candidate == &field.identity)
                .then_some((remaining, *identity))
        }).collect::<Vec<_>>();
        let child_boundaries = boundaries.iter().filter_map(|(path, identity)| {
            let (segment, remaining) = path.split_first()?;
            matches!(segment, CheckedUnitStructuralPathSegment::Field(candidate) if candidate == &field.identity)
                .then_some((remaining, *identity))
        }).collect::<Vec<_>>();
        matched = matched.checked_add(matching.len())?;
        matched_boundaries = matched_boundaries.checked_add(child_boundaries.len())?;
        let CheckedUnitStructuralFieldType::Structural { type_identity } = &field.field_type else {
            if !matching.is_empty() || !child_boundaries.is_empty() {
                return None;
            }
            continue;
        };
        prefix.push(CheckedUnitStructuralPathSegment::Field(
            field.identity.clone(),
        ));
        count = count.checked_add(visit(
            types,
            source,
            type_identity,
            &matching,
            &child_boundaries,
            prefix,
            residuals,
            emit,
        )?)?;
        prefix.pop();
    }
    (matched == moved_paths.len() && matched_boundaries == boundaries.len()).then_some(count)
}

pub(super) fn reconstruct(
    types: &BTreeMap<String, CheckedUnitStructuralTypePlan>,
    source: &CheckedUnitStructuralArgumentSourcePlan,
    root_type: &str,
    moved_paths: &[(&[CheckedUnitStructuralPathSegment], &str)],
    max_residuals: usize,
) -> Option<Vec<CheckedUnitPartialAffineDiscardPlan>> {
    reconstruct_with_boundaries(types, source, root_type, moved_paths, &[], max_residuals)
}

/// Preserve cleanup operation boundaries inside the maximal live complement.
/// A boundary selects an intact receiver with its own hook; it is not a move.
/// Nested boundaries are legal and run parent-first, then in recursive reverse
/// declaration order. Duplicate hooks and hooks overlapping a move are not.
/// The same count-before-enumeration walk serves ordinary no-code residuals.
pub(super) fn reconstruct_with_boundaries(
    types: &BTreeMap<String, CheckedUnitStructuralTypePlan>,
    source: &CheckedUnitStructuralArgumentSourcePlan,
    root_type: &str,
    moved_paths: &[(&[CheckedUnitStructuralPathSegment], &str)],
    boundaries: &[(&[CheckedUnitStructuralPathSegment], &str)],
    max_residuals: usize,
) -> Option<Vec<CheckedUnitPartialAffineDiscardPlan>> {
    if boundaries.iter().enumerate().any(|(position, (path, _))| {
        boundaries[..position]
            .iter()
            .any(|(earlier, _)| path == earlier)
            || moved_paths
                .iter()
                .any(|(moved, _)| path.starts_with(moved) || moved.starts_with(path))
    }) {
        return None;
    }
    if moved_paths.iter().enumerate().any(|(index, (path, _))| {
        path.is_empty()
            || moved_paths[..index]
                .iter()
                .any(|(earlier, _)| path.starts_with(earlier) || earlier.starts_with(path))
    }) {
        return None;
    }
    let mut residuals = Vec::new();
    let count = visit(
        types,
        source,
        root_type,
        moved_paths,
        boundaries,
        &mut Vec::new(),
        &mut residuals,
        false,
    )?;
    if count > max_residuals {
        return None;
    }
    residuals.try_reserve_exact(count).ok()?;
    let emitted = visit(
        types,
        source,
        root_type,
        moved_paths,
        boundaries,
        &mut Vec::new(),
        &mut residuals,
        true,
    )?;
    (emitted == count && residuals.len() == count).then_some(residuals)
}
