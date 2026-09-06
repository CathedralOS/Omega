use super::*;

/// Replay the exact specialization which replaces one relevant opaque Record
/// field with a canonical boundary-specific provider-root roster. These roots
/// are retained specialization witnesses, not direct boundary/Unit-call
/// structural arguments.
pub(crate) fn validate_provider_attachment_specialization(
    function: &PsiOptimizationFunction,
    boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    let provider_roots = function
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::ProviderAttachment {
                attachment,
                field,
                boundary,
            } => Some((place.id, attachment, field, boundary)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let provider_fields = function
        .attachment
        .and_then(|attachment| types.get(&attachment))
        .and_then(|attachment| match &attachment.shape {
            terminal_psi::StructuralTypeShape::Record { fields } => Some(
                fields
                    .iter()
                    .filter(|field| {
                        !field.relevance.is_erased()
                            && matches!(
                                field.field_type,
                                terminal_psi::StructuralFieldType::Erased { .. }
                            )
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .unwrap_or_default();
    if provider_fields.is_empty() && provider_roots.is_empty() {
        return Ok(());
    }

    let invalid = || {
        OptimizationUnitValidationError::InvalidProviderAttachmentSpecialization(function.machine)
    };
    let [provider_field] = provider_fields.as_slice() else {
        return Err(invalid());
    };
    let Some(attachment) = function.attachment else {
        return Err(invalid());
    };
    let self_parameters = function
        .structural_parameters
        .iter()
        .filter(|parameter| parameter.is_self)
        .collect::<Vec<_>>();
    let invalid_self = match self_parameters.as_slice() {
        [] => false,
        [parameter] => parameter.position != 0 || parameter.structural_type != attachment,
        _ => true,
    };
    if provider_roots.is_empty()
        || invalid_self
        || provider_roots.windows(2).any(|pair| pair[0].3 >= pair[1].3)
    {
        return Err(invalid());
    }

    let mut specialized_boundaries = BTreeSet::new();
    let provider_places = provider_roots
        .iter()
        .map(|(place, ..)| *place)
        .collect::<BTreeSet<_>>();
    for (_, root_attachment, field, boundary) in &provider_roots {
        let Some(boundary_declaration) = boundary_machines.get(boundary) else {
            return Err(invalid());
        };
        if *root_attachment != attachment
            || *field != provider_field.id
            || boundary_declaration.attachment.is_some()
            || !specialized_boundaries.insert(*boundary)
        {
            return Err(invalid());
        }
    }

    let mut called_boundaries = BTreeSet::new();
    for operation in function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .map(|node| &node.operation)
    {
        match operation {
            O::BoundaryCall {
                boundary,
                structural_arguments,
                ..
            } => {
                called_boundaries.insert(*boundary);
                if structural_arguments
                    .iter()
                    .any(|argument| provider_places.contains(&argument.place))
                {
                    return Err(invalid());
                }
            }
            O::CallUnit {
                structural_arguments,
                ..
            } if structural_arguments
                .iter()
                .any(|argument| provider_places.contains(&argument.place)) =>
            {
                return Err(invalid());
            }
            _ => {}
        }
    }
    if called_boundaries != specialized_boundaries {
        return Err(invalid());
    }
    Ok(())
}
