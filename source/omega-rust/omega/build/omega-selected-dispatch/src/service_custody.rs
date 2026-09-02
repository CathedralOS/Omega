//! Exact Fused routed-service custody at the checked-to-Terminal boundary.

use omega_provider_planning::CompositionMode;
use omega_provider_planning::plans::SelectedProviderReviewProvenance;
use psi_checked_trees::{
    CheckedFusedServiceErasureReceipt, CheckedTrees, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralFieldPlan, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralTypeShape,
};
use psi_diagnostics::Diagnostic;
use psi_typed_trees::data::{DataDefinition, DataField, DataMember};

/// Rejoin every checked `Service<R> in Bound` erasure to its exact typed
/// source field or direct owned parameter and owner-controlled Fused
/// selected-provider plan. This runs
/// immediately before Terminal production, where erasure becomes
/// irreversible.
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
    reject_unsupported_parameter_receipts(checked, &mut diagnostics);
    validate_unit_machine_parameters(checked, selected, &mut diagnostics);
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn validate_unit_machine_parameters(
    checked: &CheckedTrees,
    selected: &[SelectedProviderReviewProvenance],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for plan in &checked.facts.flow.terminal_unit_effects.machines {
        let typed_machines = checked
            .machines()
            .iter()
            .filter(|machine| machine.symbol == plan.machine)
            .collect::<Vec<_>>();
        let [machine] = typed_machines.as_slice() else {
            if plan
                .structural_parameters
                .iter()
                .any(|parameter| parameter.fused_service_erasure.is_some())
            {
                diagnostics.push(Diagnostic::error(format!(
                    "checked Unit machine {:?} with a fused Service parameter rejoins {} typed machines; expected one",
                    plan.machine,
                    typed_machines.len(),
                )));
            }
            continue;
        };
        let typed_states = checked
            .machine_states(machine)
            .iter()
            .filter(|state| state.symbol == plan.state)
            .collect::<Vec<_>>();
        let [state] = typed_states.as_slice() else {
            if plan
                .structural_parameters
                .iter()
                .any(|parameter| parameter.fused_service_erasure.is_some())
            {
                diagnostics.push(Diagnostic::error(format!(
                    "checked Unit machine `{}` fused Service plan rejoins {} typed states; expected one",
                    machine.name,
                    typed_states.len(),
                )));
            }
            continue;
        };
        let source_parameters = checked.state_parameters(state);

        for (position, source_parameter) in source_parameters.iter().enumerate() {
            let classification = psi_typed_trees::service::classify_exact_bound_service_carrier(
                checked,
                source_parameter.type_reference,
            );
            let Ok(Some(carrier)) = classification else {
                if let Err(reason) = classification {
                    diagnostics.push(Diagnostic::error(format!(
                        "typed parameter `{}::{}` has an invalid routed Service carrier at Terminal custody: {reason}",
                        machine.name, source_parameter.name,
                    )));
                }
                for checked_parameter in plan.structural_parameters.iter().filter(|parameter| {
                    parameter.position == u32::try_from(position).unwrap_or(u32::MAX)
                        && parameter.fused_service_erasure.is_some()
                }) {
                    let receipt = checked_parameter
                        .fused_service_erasure
                        .as_ref()
                        .expect("filtered above");
                    diagnostics.push(Diagnostic::error(format!(
                        "checked parameter receipt {:?} fabricates a fused Service erasure for non-Service parameter `{}::{}`",
                        receipt.source_parameter, machine.name, source_parameter.name,
                    )));
                }
                continue;
            };

            let checked_matches = plan
                .structural_parameters
                .iter()
                .filter(|parameter| usize::try_from(parameter.position).ok() == Some(position))
                .collect::<Vec<_>>();
            let [checked_parameter] = checked_matches.as_slice() else {
                diagnostics.push(Diagnostic::error(format!(
                    "typed routed Service parameter `{}::{}` rejoins {} checked structural parameters; expected one",
                    machine.name,
                    source_parameter.name,
                    checked_matches.len(),
                )));
                continue;
            };
            let Some(receipt) = checked_parameter.fused_service_erasure.as_ref() else {
                diagnostics.push(Diagnostic::error(format!(
                    "typed routed Service parameter `{}::{}` lost its exact Fused erasure settlement",
                    machine.name, source_parameter.name,
                )));
                continue;
            };
            validate_parameter_receipt(
                checked,
                selected,
                machine,
                state,
                source_parameter,
                position,
                carrier.requirement,
                checked_parameter,
                receipt,
                plan,
                diagnostics,
            );
        }

        for checked_parameter in &plan.structural_parameters {
            let Some(receipt) = &checked_parameter.fused_service_erasure else {
                continue;
            };
            let source_matches = source_parameters
                .get(usize::try_from(checked_parameter.position).unwrap_or(usize::MAX))
                .filter(|parameter| parameter.symbol == receipt.source_parameter)
                .into_iter()
                .filter(|parameter| {
                    psi_typed_trees::service::exact_bound_service_requirement(
                        checked,
                        parameter.type_reference,
                    ) == Some(receipt.requirement)
                })
                .count();
            if source_matches != 1 {
                diagnostics.push(Diagnostic::error(format!(
                    "checked fused Service parameter {:?} rejoins {source_matches} exact typed parameters; expected one",
                    receipt.source_parameter,
                )));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_parameter_receipt(
    checked: &CheckedTrees,
    selected: &[SelectedProviderReviewProvenance],
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    source: &psi_typed_trees::signature::StateParameter,
    position: usize,
    requirement: psi_symbols::SymbolHandle,
    parameter: &CheckedUnitStructuralParameterPlan,
    receipt: &psi_checked_trees::CheckedFusedServiceParameterReceipt,
    plan: &psi_checked_trees::CheckedUnitEffectMachinePlan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let label = format!("{}::{}", machine.name, source.name);
    if source.is_self
        || source.is_const
        || source.is_mutable
        || parameter.is_self
        || parameter.multiplicity != psi_language_semantics::Multiplicity::Affine
        || parameter.access != psi_checked_trees::CheckedStructuralAccess::Owned
        || usize::try_from(parameter.position).ok() != Some(position)
    {
        diagnostics.push(Diagnostic::error(format!(
            "checked routed Service parameter `{label}` is not the exact direct owned affine source parameter",
        )));
        return;
    }
    if receipt.source_parameter != source.symbol {
        diagnostics.push(Diagnostic::error(format!(
            "checked routed Service parameter `{label}` substituted its exact typed parameter symbol",
        )));
        return;
    }
    let source_carrier_identity = checked
        .normalized_type_identity(source.type_reference)
        .into_string();
    if receipt.carrier_type_identity != source_carrier_identity {
        diagnostics.push(Diagnostic::error(format!(
            "checked routed Service parameter `{label}` substituted its normalized carrier identity",
        )));
        return;
    }
    let (base_identity, mut source_qualifications) =
        service_parameter_base_and_qualifications(checked, source.type_reference);
    source_qualifications.sort_by_key(|domain| domain.0);
    source_qualifications.dedup();
    if parameter.type_identity != base_identity
        || parameter.qualifications != source_qualifications
        || source_qualifications.len() != 1
    {
        diagnostics.push(Diagnostic::error(format!(
            "checked routed Service parameter `{label}` substituted its structural base or exact Bound qualification",
        )));
        return;
    }
    if receipt.requirement != requirement {
        diagnostics.push(Diagnostic::error(format!(
            "checked routed Service parameter `{label}` substituted its boundary requirement",
        )));
        return;
    }
    let Some(authorization) = checked.fused_service_erasure(requirement) else {
        diagnostics.push(Diagnostic::error(format!(
            "checked routed Service parameter `{label}` lacks compiler-owned Fused erasure authority",
        )));
        return;
    };
    if authorization.provider_plan_digest != receipt.provider_plan_digest {
        diagnostics.push(Diagnostic::error(format!(
            "checked routed Service parameter `{label}` substituted its selected-provider-plan digest",
        )));
        return;
    }
    let Some(requirement_definition) = checked
        .traits()
        .iter()
        .find(|definition| definition.symbol == requirement)
    else {
        diagnostics.push(Diagnostic::error(format!(
            "checked routed Service parameter `{label}` lost its exact boundary requirement",
        )));
        return;
    };
    let Some(schema) =
        omega_effects::provider_plan::ServiceSchema::from_typed(checked, requirement_definition)
    else {
        diagnostics.push(Diagnostic::error(format!(
            "checked routed Service parameter `{label}` cannot reconstruct its boundary schema",
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
            "checked routed Service parameter `{label}` rejoins {matching} exact Fused selected-provider plans; expected one",
        )));
        return;
    }

    let Some(flow_state) = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .map(|(_, flow)| flow)
        .find(|flow| flow.machine_symbol == machine.symbol && flow.state_symbol == state.symbol)
    else {
        diagnostics.push(Diagnostic::error(format!(
            "checked routed Service parameter `{label}` has no exact flow-state custody",
        )));
        return;
    };
    let calls = checked
        .facts
        .flow
        .control
        .calls
        .span_or_empty(flow_state.calls)
        .iter()
        .filter(|call| {
            call.has_receiver
                && call.receiver_symbol == source.symbol
                && checked
                    .trait_machine_signatures(requirement_definition)
                    .iter()
                    .any(|signature| signature.symbol == call.target_symbol)
        })
        .collect::<Vec<_>>();
    let [call] = calls.as_slice() else {
        diagnostics.push(Diagnostic::error(format!(
            "checked routed Service parameter `{label}` reaches {} exact direct boundary calls; the first rung requires one",
            calls.len(),
        )));
        return;
    };
    let operation_matches = plan
        .operations
        .iter()
        .filter(|operation| match operation {
            CheckedUnitEffectOperationPlan::BoundaryCall {
                coordinate,
                target_state,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                coordinate,
                target_state,
                ..
            } => {
                usize::try_from(coordinate.statement_index).ok() == Some(call.statement_index)
                    && usize::try_from(coordinate.call_ordinal).ok() == Some(call.call_ordinal)
                    && *target_state == call.target_symbol
            }
            _ => false,
        })
        .count();
    if operation_matches != 1 {
        diagnostics.push(Diagnostic::error(format!(
            "checked routed Service parameter `{label}` rejoins {operation_matches} exact checked boundary operations; expected one",
        )));
    }
}

fn service_parameter_base_and_qualifications(
    checked: &CheckedTrees,
    mut type_reference: psi_typed_trees::types::TypeReferenceHandle,
) -> (String, Vec<psi_language_semantics::SemanticDomainId>) {
    let mut qualifications = Vec::new();
    while let psi_typed_trees::types::TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = checked.type_reference_table.type_reference(type_reference)
    {
        qualifications.extend(
            checked
                .type_reference_table
                .constraints(*constraints)
                .iter()
                .filter_map(|constraint| match constraint {
                    psi_typed_trees::types::TypeConstraintNode::Domain(domain) => {
                        Some(domain.semantic_id)
                    }
                    _ => None,
                }),
        );
        type_reference = *base_type;
    }
    (
        checked
            .normalized_type_identity(type_reference)
            .into_string(),
        qualifications,
    )
}

fn reject_unsupported_parameter_receipts(
    checked: &CheckedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut reject = |family: &str, parameters: &[CheckedUnitStructuralParameterPlan]| {
        for parameter in parameters {
            if parameter.fused_service_erasure.is_some() {
                diagnostics.push(Diagnostic::error(format!(
                    "checked {family} fabricates a fused Service parameter receipt outside the first direct Unit-machine rung",
                )));
            }
        }
    };
    let flow = &checked.facts.flow;
    for plan in &flow.terminal_unit_effects.boundary_machines {
        reject("boundary machine", &plan.structural_parameters);
    }
    for machine in &flow.terminal_unit_effects.composed_machines {
        for state in &machine.states {
            reject("composed Unit state", &state.structural_parameters);
        }
    }
    for plan in &flow.terminal_partial_affine_unit_cleanups.machines {
        reject(
            "partial affine cleanup machine",
            &plan.machine.structural_parameters,
        );
    }
    for plan in &flow.terminal_nominal_affine_unit_cleanups.machines {
        reject(
            "nominal affine cleanup machine",
            &plan.machine.structural_parameters,
        );
    }
    for machine in &flow.terminal_structural_unit_controls.machines {
        for state in &machine.states {
            reject("structural control state", &state.structural_parameters);
        }
    }
    for machine in &flow.terminal_structural_scalar_returns.machines {
        reject("structural scalar return", &machine.structural_parameters);
    }
    for machine in &flow
        .terminal_structural_scalar_returns
        .selected_operator_machines
    {
        reject(
            "selected-operator structural return",
            &machine.structural_parameters,
        );
    }
    for machine in &flow
        .terminal_structural_scalar_returns
        .trait_operator_machines
    {
        reject(
            "trait-operator structural return",
            &machine.structural_parameters,
        );
    }
    for machine in &flow.terminal_boundary_scalar_returns.boundary_machines {
        reject(
            "scalar-return boundary machine",
            &machine.structural_parameters,
        );
    }
    for machine in &flow.terminal_boundary_scalar_returns.machines {
        reject("boundary scalar return", &machine.structural_parameters);
    }
    for machine in &flow.terminal_structural_returns.machines {
        reject("structural return", &machine.structural_parameters);
    }
    for machine in &flow.terminal_structural_call_returns.machines {
        reject("structural call return", &machine.structural_parameters);
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
