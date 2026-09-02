//! Checked provider requirements projected into Terminal attachment roots.

use super::*;

pub(super) fn validate_provider_attachment_requirements(
    attachment: &CheckedUnitStructuralTypePlan,
    requirements: &[psi_checked_trees::CheckedProviderAttachmentRequirementPlan],
    called_boundaries: &[psi_symbols::SymbolHandle],
) -> Result<(), LoweringError> {
    let provider_fields = match &attachment.shape {
        CheckedUnitStructuralTypeShape::Record { fields } => fields
            .iter()
            .filter(|field| {
                matches!(
                    field.field_type,
                    CheckedUnitStructuralFieldType::ProviderBacked { .. }
                        | CheckedUnitStructuralFieldType::FusedServiceBacked { .. }
                )
            })
            .count(),
        _ => 0,
    };
    if (provider_fields == 0) != requirements.is_empty() || provider_fields > 1 {
        return unsupported(
            "provider-backed attachment field lacks one complete specialization requirement set",
        );
    }
    if provider_fields == 1 {
        let mut called = called_boundaries.to_vec();
        called.sort_by_key(|boundary| (boundary.arena_index(), boundary.generation()));
        called.dedup();
        let specialized = requirements
            .iter()
            .map(|requirement| requirement.boundary)
            .collect::<Vec<_>>();
        if called != specialized {
            return unsupported(
                "provider-backed attachment requirements do not exactly cover boundary calls",
            );
        }
    }
    Ok(())
}

pub(crate) fn lower_provider_attachment_places(
    attachment: StructuralTypeId,
    declaration: &StructuralTypeDeclaration,
    requirements: &[psi_checked_trees::CheckedProviderAttachmentRequirementPlan],
    boundaries: &[(psi_symbols::SymbolHandle, BoundaryMachineId)],
    next_place: &mut u64,
) -> Result<Vec<StructuralPlaceDeclaration>, LoweringError> {
    let fields = match &declaration.shape {
        StructuralTypeShape::Record { fields } => Some(fields.as_slice()),
        _ => None,
    };
    if !requirements.is_empty() && fields.is_none() {
        return unsupported(
            "provider-backed attachment specialization requires a record attachment",
        );
    }
    let mut roots = requirements
        .iter()
        .map(|requirement| {
            let field = fields
                .expect("provider requirements require record attachment")
                .iter()
                .find(|field| field.identity == requirement.field_identity)
                .ok_or(LoweringError::Unsupported(
                    "provider-backed attachment requirement names an unknown field",
                ))?;
            if field.relevance.is_erased()
                || !matches!(&field.field_type,
                    StructuralFieldType::Erased { type_identity }
                        if type_identity == &requirement.provider_type_identity)
            {
                return unsupported(
                    "provider-backed attachment requirement disagrees with its erased carrier",
                );
            }
            let boundary = boundaries
                .iter()
                .find_map(|(symbol, boundary)| {
                    (*symbol == requirement.boundary).then_some(*boundary)
                })
                .ok_or(LoweringError::Unsupported(
                    "provider-backed attachment requirement names an unlowered boundary",
                ))?;
            Ok((field.id, boundary))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    roots.sort_by_key(|(_, boundary)| *boundary);
    if roots.windows(2).any(|pair| pair[0] == pair[1]) {
        return unsupported("provider-backed attachment requirements contain duplicates");
    }
    roots
        .into_iter()
        .map(|(field, boundary)| {
            Ok(StructuralPlaceDeclaration {
                id: place_id(allocate_dense(next_place)?),
                kind: StructuralPlaceKind::ProviderAttachment {
                    attachment,
                    field,
                    boundary,
                },
            })
        })
        .collect()
}
