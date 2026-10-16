//! Independently reconstruct the maximal live structural path complement.
//! A case step selects active payload debt; inactive alternatives are never
//! residuals. The caller still authenticates membership and cleanup authority.
use semantic_vocabulary::{StructuralCaseId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
    find_structural_case,
};

pub(super) fn partial_affine_residuals(
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    root_type: StructuralTypeId,
    moved_paths: &BTreeSet<Vec<StructuralPathSegment>>,
    max_residuals: usize,
) -> Option<Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>> {
    selected_affine_residuals(
        structural_types,
        root_type,
        moved_paths,
        max_residuals,
        &|_, _| false,
    )
}

pub(super) fn selected_affine_residuals(
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    root_type: StructuralTypeId,
    moved_paths: &BTreeSet<Vec<StructuralPathSegment>>,
    max_residuals: usize,
    selected: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
) -> Option<Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>> {
    if moved_paths.is_empty()
        || moved_paths.iter().any(|path| {
            path.is_empty()
                || crate::unit_validation::structural_catalog::resolve_structural_path(
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
        selected,
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
    selected: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
    residuals: &mut Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>,
) -> Option<()> {
    match &structural_types.get(&structural_type)?.shape {
        StructuralTypeShape::Record { fields, .. } => {
            if fields.is_empty() {
                return None;
            }
            collect_field_residuals(
                structural_types,
                fields,
                moved_paths,
                prefix,
                max_residuals,
                selected,
                residuals,
            )?;
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
                    selected,
                    residuals,
                )?;
            }
        }
        StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
            let selected_case = moved_paths
                .iter()
                .find_map(|path| match path.first() {
                    Some(StructuralPathSegment::Case(case)) => Some(*case),
                    _ => None,
                })
                .or_else(|| {
                    let mut matches = cases.iter().filter(|case| selected(prefix, case.id));
                    let case = matches.next()?;
                    matches.next().is_none().then_some(case.id)
                })?;
            let selected_case_declaration = find_structural_case(cases, selected_case)?;
            let common = structural_types
                .get(&structural_type)?
                .shape
                .direct_fields();
            if moved_paths.iter().any(|path| match path.first() {
                Some(StructuralPathSegment::Case(case)) => *case != selected_case,
                Some(StructuralPathSegment::Field(_)) => common.is_none(),
                _ => true,
            }) {
                return None;
            }
            let payload_moves = moved_paths
                .iter()
                .filter_map(|path| {
                    matches!(path.first(), Some(StructuralPathSegment::Case(_)))
                        .then_some(&path[1..])
                })
                .collect::<Vec<_>>();
            prefix.push(StructuralPathSegment::Case(selected_case));
            collect_field_residuals(
                structural_types,
                &selected_case_declaration.fields,
                &payload_moves,
                prefix,
                max_residuals,
                selected,
                residuals,
            )?;
            prefix.pop();
            if let Some(common) = common {
                let common_moves = moved_paths
                    .iter()
                    .copied()
                    .filter(|path| matches!(path.first(), Some(StructuralPathSegment::Field(_))))
                    .collect::<Vec<_>>();
                collect_field_residuals(
                    structural_types,
                    common,
                    &common_moves,
                    prefix,
                    max_residuals,
                    selected,
                    residuals,
                )?;
            }
        }
        _ => return None,
    }
    Some(())
}

fn collect_field_residuals(
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    fields: &[terminal_psi::StructuralFieldDeclaration],
    moved_paths: &[&[StructuralPathSegment]],
    prefix: &mut Vec<StructuralPathSegment>,
    max_residuals: usize,
    selected: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
    residuals: &mut Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>,
) -> Option<()> {
    // Reconstruct inline descriptor debt as well as structural children.
    // Scalar restrictions and bounded owned bytes carry no disposal action.
    for field in fields.iter().rev() {
        if field.relevance.is_erased() {
            return None;
        }
        let child_type = match &field.field_type {
            StructuralFieldType::Structural(child) => Some(*child),
            scalar if scalar.scalar_type().is_some() => None,
            bytes => match bytes.byte_sequence_carrier()? {
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. } => None,
                carrier => Some(
                    terminal_semantics::byte_carrier_field_declaration(
                        structural_types.values().copied(),
                        carrier,
                    )?
                    .id,
                ),
            },
        };
        if let Some(child_type) = child_type {
            collect_child_residuals(
                structural_types,
                child_type,
                StructuralPathSegment::Field(field.identity.clone()),
                moved_paths,
                prefix,
                max_residuals,
                selected,
                residuals,
            )?;
        }
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
    selected: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
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
            selected,
            residuals,
        )?;
    } else if descendants.len() != 1 {
        return None;
    }
    prefix.pop();
    Some(())
}
