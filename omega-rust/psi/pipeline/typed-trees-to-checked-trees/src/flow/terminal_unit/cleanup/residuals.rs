//! Type-directed maximal residual complement for finite records and arrays.

use super::*;

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
        CheckedUnitStructuralTypeShape::Record { fields } => {
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
        } if *length > 0 => {
            validate_type(types, element_type_identity, active, complete)?;
        }
        _ => return None,
    }
    active.pop();
    complete.push(current_type.to_owned());
    Some(())
}

/// Count first, traversing only touched array children. Thus a huge dimension
/// cannot trigger enumeration before host-size and output-allocation checks.
fn visit(
    types: &BTreeMap<String, CheckedUnitStructuralTypePlan>,
    source: &CheckedUnitStructuralArgumentSourcePlan,
    current_type: &str,
    moved_paths: &[(&[CheckedUnitStructuralPathSegment], &str)],
    prefix: &mut Vec<CheckedUnitStructuralPathSegment>,
    residuals: &mut Vec<CheckedUnitPartialAffineDiscardPlan>,
    emit: bool,
) -> Option<usize> {
    if moved_paths.is_empty() {
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
    let declaration = types.get(current_type)?;
    let mut count = 0_usize;
    match &declaration.shape {
        CheckedUnitStructuralTypeShape::Record { fields } => {
            let mut matched = 0_usize;
            for field in fields.iter().rev() {
                let matching = moved_paths.iter().filter_map(|(path, moved_type)| {
                    matches!(&path[0], CheckedUnitStructuralPathSegment::Field(identity) if identity == &field.identity)
                        .then_some((&path[1..], *moved_type))
                }).collect::<Vec<_>>();
                matched = matched.checked_add(matching.len())?;
                let CheckedUnitStructuralFieldType::Structural { type_identity } =
                    &field.field_type
                else {
                    if !matching.is_empty() {
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
                    prefix,
                    residuals,
                    emit,
                )?)?;
                prefix.pop();
            }
            if matched != moved_paths.len() {
                return None;
            }
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
            touched.sort_unstable();
            touched.dedup();
            let untouched = usize::try_from(*length).ok()?.checked_sub(touched.len())?;
            if !emit {
                count = untouched;
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
                prefix.push(CheckedUnitStructuralPathSegment::FixedIndex(index));
                count = count.checked_add(visit(
                    types,
                    source,
                    element_type_identity,
                    &matching,
                    prefix,
                    residuals,
                    emit,
                )?)?;
                prefix.pop();
            }
        }
        _ => return None,
    }
    Some(count)
}

pub(super) fn reconstruct(
    types: &BTreeMap<String, CheckedUnitStructuralTypePlan>,
    source: &CheckedUnitStructuralArgumentSourcePlan,
    root_type: &str,
    moved_paths: &[(&[CheckedUnitStructuralPathSegment], &str)],
    max_residuals: usize,
) -> Option<Vec<CheckedUnitPartialAffineDiscardPlan>> {
    if moved_paths.is_empty()
        || moved_paths.iter().enumerate().any(|(index, (path, _))| {
            path.is_empty()
                || moved_paths[..index]
                    .iter()
                    .any(|(earlier, _)| path.starts_with(earlier) || earlier.starts_with(path))
        })
    {
        return None;
    }
    validate_type(types, root_type, &mut Vec::new(), &mut Vec::new())?;
    let mut residuals = Vec::new();
    let count = visit(
        types,
        source,
        root_type,
        moved_paths,
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
        &mut Vec::new(),
        &mut residuals,
        true,
    )?;
    (emitted == count && residuals.len() == count).then_some(residuals)
}
