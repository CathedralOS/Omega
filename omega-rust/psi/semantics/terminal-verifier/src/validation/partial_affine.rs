//! Reconstruct maximal residual subtrees from finite record/array move paths.

use super::*;

/// Resolve custody from its real owner, independently of any projected use.
/// Result declarations alone do not establish an affine value or its producer.
pub(super) fn partial_affine_root_type(
    machine: &TerminalMachine,
    place: PlaceId,
) -> Option<StructuralTypeId> {
    if machine
        .entry_claims
        .iter()
        .any(|claim| claim.input == place)
        || machine
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == place)
    {
        return None;
    }
    if let Some(parameter) = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == place)
    {
        return (parameter.multiplicity == StructuralMultiplicity::Affine
            && parameter.access == StructuralAccess::Owned
            && parameter.qualifications.is_empty()
            && parameter.projected_qualifications.is_empty())
        .then_some(parameter.structural_type);
    }
    let declaration = machine
        .structural_places
        .iter()
        .find(|declaration| declaration.id == place)?;
    let StructuralPlaceKind::OperationResult {
        producer,
        structural_type,
    } = declaration.kind
    else {
        return None;
    };
    let operation = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == producer)?;
    if !matches!(
        operation.kind,
        OperationKind::CallStructuralWithScalarArguments { .. }
            | OperationKind::BoundaryCall { .. }
    ) {
        return None;
    }
    let result = operation.result.structural()?;
    (result.place == place
        && result.structural_type == structural_type
        && result.multiplicity == StructuralMultiplicity::Affine
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty())
    .then_some(structural_type)
}

pub(super) fn is_partial_affine_path(
    module: &TerminalModule,
    root_type: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> bool {
    !path.is_empty() && resolve_structural_path(module, root_type, path).is_some()
}

/// The supplied cleanup bounds output work. An enormous array with a short
/// forged cleanup cannot make verification enumerate its absent residuals.
pub(super) fn partial_affine_residuals(
    module: &TerminalModule,
    root_type: StructuralTypeId,
    moved_paths: &BTreeSet<Vec<StructuralPathSegment>>,
    max_residuals: usize,
) -> Option<Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>> {
    if moved_paths.is_empty()
        || moved_paths
            .iter()
            .any(|path| !is_partial_affine_path(module, root_type, path))
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
    visit(
        module,
        root_type,
        &moved,
        &mut Vec::new(),
        max_residuals,
        &mut residuals,
    )?;
    Some(residuals)
}

fn visit(
    module: &TerminalModule,
    current: StructuralTypeId,
    moved: &[&[StructuralPathSegment]],
    prefix: &mut Vec<StructuralPathSegment>,
    max_residuals: usize,
    residuals: &mut Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>,
) -> Option<()> {
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == current)?;
    match &declaration.shape {
        StructuralTypeShape::Record { fields } => {
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
            for field in fields.iter().rev() {
                if let StructuralFieldType::Structural(child_type) = field.field_type {
                    child(
                        module,
                        child_type,
                        StructuralPathSegment::Field(field.identity.clone()),
                        moved,
                        prefix,
                        max_residuals,
                        residuals,
                    )?;
                }
            }
        }
        StructuralTypeShape::FixedArray { element, length } if *length != 0 => {
            // Each untouched child needs one residual. At most one child per
            // moved path can avoid that cost; reject before iterating a range
            // too large to fit the supplied evidence.
            if u128::from(*length) > (max_residuals - residuals.len()) as u128 + moved.len() as u128
            {
                return None;
            }
            for index in (0..*length).rev() {
                child(
                    module,
                    *element,
                    StructuralPathSegment::FixedIndex(index),
                    moved,
                    prefix,
                    max_residuals,
                    residuals,
                )?;
            }
        }
        _ => return None,
    }
    Some(())
}

fn child(
    module: &TerminalModule,
    child_type: StructuralTypeId,
    segment: StructuralPathSegment,
    moved: &[&[StructuralPathSegment]],
    prefix: &mut Vec<StructuralPathSegment>,
    max_residuals: usize,
    residuals: &mut Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>,
) -> Option<()> {
    let descendants = moved
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
        visit(
            module,
            child_type,
            &descendants,
            prefix,
            max_residuals,
            residuals,
        )?;
    } else if descendants.len() != 1 {
        return None;
    }
    prefix.pop();
    Some(())
}
