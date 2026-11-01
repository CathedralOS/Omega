//! Checked provider requirements projected into Terminal attachment roots.
//!
//! Roots witness exact field-to-requirement specialization, not lexical self
//! storage. Stage 05 builds the requirement set
//! (`execution/terminal_unit/providers.rs`): one row for each boundary call
//! whose receiver, lexical self or a parameter-rooted Fused binding, routes
//! through a provider field, refusing the plan when a receiver claims none.
//! This module only lowers each requirement to its field root.

use super::super::{BoundaryMachineId, StructuralTypeDeclaration};
use super::{
    LoweringError, StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeId,
    StructuralTypeShape, allocate_dense, place_id, unsupported,
};

pub(crate) fn lower_provider_attachment_places(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &checked_trees::CheckedTrees,
    attachment: StructuralTypeId,
    declaration: &StructuralTypeDeclaration,
    structural_types: &[StructuralTypeDeclaration],
    requirements: &[checked_trees::CheckedProviderAttachmentRequirementPlan],
    boundaries: &[(symbols::SymbolHandle, BoundaryMachineId)],
    next_place: &mut u64,
) -> Result<Vec<StructuralPlaceDeclaration>, LoweringError> {
    let fields = match &declaration.shape {
        StructuralTypeShape::Record { fields, .. } => Some(fields.as_slice()),
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
            let field = terminal_psi::field_by_identity(
                &fields.expect("provider requirements require record attachment"),
                identities.intern(&requirement.field_identity.spelling()),
            )
            .ok_or(LoweringError::Unsupported(
                "provider-backed attachment requirement names an unknown field",
            ))?;
            // A retained binding keeps its semantic field path even though
            // Fused realization has no payload. This specialization root is
            // evidence, not a substitute argument or a new establishment.
            if !terminal_semantics::fused_service_field_matches(
                structural_types,
                field,
                identities.intern(
                    &checked
                        .type_identity_text(requirement.provider_type_identity)
                        .unwrap_or_default(),
                ),
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
