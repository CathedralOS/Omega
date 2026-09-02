//! Exact Fused routed-service custody at the checked-to-Terminal boundary.

mod parameters;

use omega_provider_planning::CompositionMode;
use omega_provider_planning::plans::SelectedProviderReviewProvenance;
use psi_checked_trees::{
    CheckedFusedServiceErasureReceipt, CheckedTrees, CheckedUnitStructuralFieldPlan,
    CheckedUnitStructuralFieldType, CheckedUnitStructuralTypeShape,
};
use psi_diagnostics::Diagnostic;
use psi_typed_trees::data::{DataDefinition, DataField, DataMember};

/// Rejoin every checked `Service<R> in Bound` erasure to its exact typed
/// source field or direct owned parameter and owner-controlled Fused
/// selected-provider plan. This runs immediately before Terminal production,
/// where erasure becomes irreversible.
pub fn validate_fused_service_terminal_custody(
    checked: &CheckedTrees,
    selected: &[SelectedProviderReviewProvenance],
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    for plan in &checked.facts.flow.terminal_unit_effects.structural_types {
        let CheckedUnitStructuralTypeShape::Record { fields } = &plan.shape else {
            if structural_shape_contains_fused_service(&plan.shape) {
                diagnostics.push(Diagnostic::error(format!(
                    "checked structural type `{}` carries a fused Service erasure outside its exact record field",
                    plan.identity,
                )));
            }
            continue;
        };
        let mut owner_symbols = checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter()
            .filter(|machine| machine.attachment_type_identity == plan.identity)
            .map(|machine| machine.machine)
            .chain(
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .composed_machines
                    .iter()
                    .filter(|machine| machine.attachment_type_identity == plan.identity)
                    .map(|machine| machine.machine),
            )
            .filter_map(|machine_symbol| {
                checked
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == machine_symbol)
                    .map(|machine| machine.attached_data_symbol)
            })
            .collect::<Vec<_>>();
        owner_symbols.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
        owner_symbols.dedup();
        let owners = owner_symbols
            .iter()
            .filter_map(|owner| {
                checked
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.symbol == *owner)
            })
            .collect::<Vec<_>>();
        let [owner] = owners.as_slice() else {
            if fields.iter().any(|field| {
                matches!(
                    field.field_type,
                    CheckedUnitStructuralFieldType::FusedServiceBacked { .. }
                )
            }) {
                diagnostics.push(Diagnostic::error(format!(
                    "checked structural type `{}` cannot rejoin its fused Service fields to one exact typed owner",
                    plan.identity,
                )));
            }
            continue;
        };
        validate_record_fields(checked, selected, owner, fields, &mut diagnostics);
    }
    parameters::validate(checked, selected, &mut diagnostics);
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn validate_record_fields(
    checked: &CheckedTrees,
    selected: &[SelectedProviderReviewProvenance],
    owner: &DataDefinition,
    checked_fields: &[CheckedUnitStructuralFieldPlan],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for member in checked.data_members(owner) {
        let DataMember::Field(source_field) = member else {
            continue;
        };
        let source_identity = data_field_identity(source_field);
        let checked_matches = checked_fields
            .iter()
            .filter(|field| field.identity == source_identity)
            .collect::<Vec<_>>();
        let classification = psi_typed_trees::service::classify_exact_bound_service_carrier(
            checked,
            source_field.type_reference,
        );
        let Ok(Some(carrier)) = classification else {
            if let Err(reason) = classification {
                diagnostics.push(Diagnostic::error(format!(
                    "typed field `{}::{}` has an invalid routed Service carrier at Terminal custody: {reason}",
                    owner.name, source_field.name,
                )));
            }
            for field in checked_matches {
                if matches!(
                    field.field_type,
                    CheckedUnitStructuralFieldType::FusedServiceBacked { .. }
                ) {
                    diagnostics.push(Diagnostic::error(format!(
                        "checked field `{}::{}` fabricates a fused Service erasure for a non-Service source field",
                        owner.name, source_field.name,
                    )));
                }
            }
            continue;
        };
        let [checked_field] = checked_matches.as_slice() else {
            diagnostics.push(Diagnostic::error(format!(
                "typed routed Service field `{}::{}` rejoins {} checked structural fields; expected one",
                owner.name,
                source_field.name,
                checked_matches.len(),
            )));
            continue;
        };
        let CheckedUnitStructuralFieldType::FusedServiceBacked {
            provider_type_identity,
            erasure,
        } = &checked_field.field_type
        else {
            diagnostics.push(Diagnostic::error(format!(
                "typed routed Service field `{}::{}` lost its exact Fused erasure settlement",
                owner.name, source_field.name,
            )));
            continue;
        };
        let source_type_identity = checked
            .normalized_type_identity(source_field.type_reference)
            .into_string();
        if provider_type_identity != &source_type_identity {
            diagnostics.push(Diagnostic::error(format!(
                "checked routed Service field `{}::{}` substituted its normalized carrier identity",
                owner.name, source_field.name,
            )));
            continue;
        }
        validate_receipt(
            checked,
            selected,
            owner,
            source_field,
            carrier.requirement,
            *erasure,
            diagnostics,
        );
    }

    for checked_field in checked_fields {
        if !matches!(
            checked_field.field_type,
            CheckedUnitStructuralFieldType::FusedServiceBacked { .. }
        ) {
            continue;
        }
        let source_matches = checked
            .data_members(owner)
            .iter()
            .filter_map(|member| match member {
                DataMember::Field(field)
                    if data_field_identity(field) == checked_field.identity =>
                {
                    Some(field)
                }
                _ => None,
            })
            .count();
        if source_matches != 1 {
            diagnostics.push(Diagnostic::error(format!(
                "checked fused Service field `{}::{}` rejoins {source_matches} typed source fields; expected one",
                owner.name, checked_field.identity,
            )));
        }
    }
}

fn validate_receipt(
    checked: &CheckedTrees,
    selected: &[SelectedProviderReviewProvenance],
    owner: &DataDefinition,
    field: &DataField,
    requirement: psi_symbols::SymbolHandle,
    receipt: CheckedFusedServiceErasureReceipt,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if receipt.requirement != requirement {
        diagnostics.push(Diagnostic::error(format!(
            "checked routed Service field `{}::{}` substituted its boundary requirement",
            owner.name, field.name,
        )));
        return;
    }
    let Some(authorization) = checked.fused_service_erasure(requirement) else {
        diagnostics.push(Diagnostic::error(format!(
            "checked routed Service field `{}::{}` lacks compiler-owned Fused erasure authority",
            owner.name, field.name,
        )));
        return;
    };
    if authorization.provider_plan_digest != receipt.provider_plan_digest {
        diagnostics.push(Diagnostic::error(format!(
            "checked routed Service field `{}::{}` substituted its selected-provider-plan digest",
            owner.name, field.name,
        )));
        return;
    }
    let Some(requirement_definition) = checked
        .traits()
        .iter()
        .find(|definition| definition.symbol == requirement)
    else {
        diagnostics.push(Diagnostic::error(format!(
            "checked routed Service field `{}::{}` lost its exact boundary requirement",
            owner.name, field.name,
        )));
        return;
    };
    let Some(schema) =
        omega_effects::provider_plan::ServiceSchema::from_typed(checked, requirement_definition)
    else {
        diagnostics.push(Diagnostic::error(format!(
            "checked routed Service field `{}::{}` cannot reconstruct its boundary schema",
            owner.name, field.name,
        )));
        return;
    };
    let matching = selected
        .iter()
        .filter(|candidate| {
            candidate.plan.schema == schema
                && candidate.plan.identity_digest().as_bytes() == &receipt.provider_plan_digest
                && candidate.selected_by.composition_mode() == Ok(CompositionMode::Fused)
        })
        .count();
    if matching != 1 {
        diagnostics.push(Diagnostic::error(format!(
            "checked routed Service field `{}::{}` rejoins {matching} exact Fused selected-provider plans; expected one",
            owner.name, field.name,
        )));
    }
}

fn data_field_identity(field: &DataField) -> String {
    field
        .identity
        .map(|identity| format!("#{identity}"))
        .unwrap_or_else(|| field.name.as_str().to_owned())
}

fn structural_shape_contains_fused_service(shape: &CheckedUnitStructuralTypeShape) -> bool {
    match shape {
        CheckedUnitStructuralTypeShape::Record { fields } => fields.iter().any(|field| {
            matches!(
                field.field_type,
                CheckedUnitStructuralFieldType::FusedServiceBacked { .. }
            )
        }),
        CheckedUnitStructuralTypeShape::Sum { cases } => cases
            .iter()
            .any(|case| case.fields.iter().any(fused_service_field)),
        CheckedUnitStructuralTypeShape::Mixed { fields, cases } => {
            fields.iter().any(fused_service_field)
                || cases
                    .iter()
                    .any(|case| case.fields.iter().any(fused_service_field))
        }
        CheckedUnitStructuralTypeShape::PrimitiveScalar(_)
        | CheckedUnitStructuralTypeShape::ByteSequence(_)
        | CheckedUnitStructuralTypeShape::FixedArray { .. } => false,
    }
}

fn fused_service_field(field: &CheckedUnitStructuralFieldPlan) -> bool {
    matches!(
        field.field_type,
        CheckedUnitStructuralFieldType::FusedServiceBacked { .. }
    )
}
