//! Checked provider requirements projected into Terminal attachment roots.
//!
//! Roots witness exact field-to-requirement specialization, not lexical self
//! storage. Calls through an ordinary context retain the same field root only
//! after source replay joins that parameter's nominal carrier and Fused receipt.
//! Per-occurrence replay precedes complete-set validation; matching boundary
//! names cannot justify a sibling field or a different context type.

use super::super::{
    BoundaryMachineId, CheckedUnitStructuralFieldType, CheckedUnitStructuralTypePlan,
    CheckedUnitStructuralTypeShape, StructuralTypeDeclaration,
};
use super::{
    LoweringError, StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeId,
    StructuralTypeShape, allocate_dense, place_id, unsupported,
};
mod source;
#[cfg(test)]
mod tests;
pub(super) use source::validate_call_source;

pub(super) fn validate_provider_attachment_requirements(
    checked: &checked_trees::CheckedTrees,
    attachment: &CheckedUnitStructuralTypePlan,
    requirements: &[checked_trees::CheckedProviderAttachmentRequirementPlan],
    called_boundaries: &[symbols::SymbolHandle],
) -> Result<(), LoweringError> {
    let fields = match &attachment.shape {
        CheckedUnitStructuralTypeShape::Record { fields } => fields.as_slice(),
        _ => &[],
    };
    let provider_fields = fields
        .iter()
        .filter(|field| {
            matches!(
                field.field_type,
                CheckedUnitStructuralFieldType::ProviderBacked { .. }
                    | CheckedUnitStructuralFieldType::FusedServiceBacked { .. }
            )
        })
        .count();
    // Each requirement names the provider field it is called through, so one
    // attachment may hold several provider fields (a console and a checked
    // adapter side by side); the requirement set must still cover exactly the
    // signature-directed boundary calls the body makes.
    if provider_fields == 0 && !requirements.is_empty() {
        return unsupported(
            "provider-backed attachment field lacks one complete specialization requirement set",
        );
    }
    if provider_fields > 0 {
        // Canonical order is field declaration order, then boundary: rows
        // must be strictly increasing, which also rejects a repeated row.
        let keys = requirements
            .iter()
            .map(|requirement| {
                let position = fields
                    .iter()
                    .position(|field| field.identity == requirement.field_identity)
                    .ok_or(LoweringError::Unsupported(
                        "provider-backed attachment requirement names an unknown field",
                    ))?;
                Ok((
                    position,
                    requirement.boundary.arena_index(),
                    requirement.boundary.generation(),
                ))
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        if keys.windows(2).any(|pair| pair[0] >= pair[1]) {
            return unsupported(
                "provider-backed attachment requirements are not in canonical field and boundary order",
            );
        }
        // Requirement ownership, not equality of machine/state handles,
        // determines attachment custody. Legacy operators use one symbol for
        // both coordinates but own no service receiver. Resolve the authored
        // boundary-trait signatures once here for all Unit plan families.
        let mut called = called_boundaries
            .iter()
            .copied()
            .filter(|boundary| {
                checked.traits().iter().any(|definition| {
                    definition.is_boundary
                        && checked
                            .trait_machine_signatures(definition)
                            .iter()
                            .any(|signature| signature.symbol == *boundary)
                })
            })
            .collect::<Vec<_>>();
        called.sort_by_key(|boundary| (boundary.arena_index(), boundary.generation()));
        called.dedup();
        let mut specialized = requirements
            .iter()
            .map(|requirement| requirement.boundary)
            .collect::<Vec<_>>();
        specialized.sort_by_key(|boundary| (boundary.arena_index(), boundary.generation()));
        if specialized.windows(2).any(|pair| pair[0] == pair[1]) || called != specialized {
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
    structural_types: &[StructuralTypeDeclaration],
    requirements: &[checked_trees::CheckedProviderAttachmentRequirementPlan],
    boundaries: &[(symbols::SymbolHandle, BoundaryMachineId)],
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
            // A retained binding keeps its semantic field path even though
            // Fused realization has no payload. This specialization root is
            // evidence, not a substitute argument or a new establishment.
            if !terminal_semantics::fused_service_field_matches(
                structural_types,
                field,
                &requirement.provider_type_identity,
            ) {
                return unsupported(
                    "provider-backed attachment requirement disagrees with its exact carrier",
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
