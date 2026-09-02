//! Exact custody for the first direct owned routed-Service parameter rung.

use omega_provider_planning::CompositionMode;
use omega_provider_planning::plans::SelectedProviderReviewProvenance;
use psi_checked_trees::{
    CheckedStructuralScalarParameterPlan, CheckedTrees, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralParameterPlan,
};
use psi_diagnostics::Diagnostic;

pub(super) fn validate(
    checked: &CheckedTrees,
    selected: &[SelectedProviderReviewProvenance],
    diagnostics: &mut Vec<Diagnostic>,
) {
    reject_unsupported_receipts(checked, diagnostics);
    validate_unit_machines(checked, selected, diagnostics);
}

fn validate_unit_machines(
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
        let carries_receipt = plan
            .structural_parameters
            .iter()
            .any(|parameter| parameter.fused_service_erasure.is_some());
        let mut source_scalar_parameters = Vec::new();
        let mut invalid_scalar_parameter = false;
        for (position, source) in source_parameters.iter().enumerate() {
            let Some(primitive_type) = checked.primitive_type_reference(source.type_reference)
            else {
                continue;
            };
            if source.is_self || source.is_const || source.is_mutable {
                invalid_scalar_parameter = true;
                continue;
            }
            let Ok(source_position) = u32::try_from(position) else {
                invalid_scalar_parameter = true;
                continue;
            };
            source_scalar_parameters.push(CheckedStructuralScalarParameterPlan {
                source_position,
                primitive_type,
            });
        }
        if carries_receipt {
            if invalid_scalar_parameter || plan.scalar_parameters != source_scalar_parameters {
                diagnostics.push(Diagnostic::error(format!(
                    "checked Unit machine `{}` routed Service scalar parameters do not rejoin the exact immutable typed source partition",
                    machine.name,
                )));
            }
        }

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
            validate_receipt(
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
fn validate_receipt(
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
        base_and_qualifications(checked, source.type_reference);
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
        .map(|call| (call.statement_index, call.call_ordinal, call.target_symbol))
        .collect::<Vec<_>>();
    if calls.is_empty() {
        if let Err(reason) =
            validate_single_forward(checked, machine, state, parameter, receipt, plan)
        {
            diagnostics.push(Diagnostic::error(format!(
                "checked routed Service parameter `{label}` is neither an exact direct-call route nor one exact whole-root forwarding hop: {reason}",
            )));
        }
        return;
    }
    let operations = plan
        .operations
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::BoundaryCall {
                coordinate,
                target_state,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                coordinate,
                target_state,
                ..
            } if checked
                .trait_machine_signatures(requirement_definition)
                .iter()
                .any(|signature| signature.symbol == *target_state) =>
            {
                Some((
                    usize::try_from(coordinate.statement_index).ok()?,
                    usize::try_from(coordinate.call_ordinal).ok()?,
                    *target_state,
                ))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if operations != calls {
        diagnostics.push(Diagnostic::error(format!(
            "checked routed Service parameter `{label}` reaches {} exact direct boundary calls but rejoins {} exact checked boundary operations; expected one ordered operation per call",
            calls.len(),
            operations.len(),
        )));
    }
}

fn validate_single_forward(
    checked: &CheckedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    parameter: &CheckedUnitStructuralParameterPlan,
    receipt: &psi_checked_trees::CheckedFusedServiceParameterReceipt,
    plan: &psi_checked_trees::CheckedUnitEffectMachinePlan,
) -> Result<(), &'static str> {
    if machine.attached_data.is_some()
        || plan.attachment_type_identity.is_some()
        || !plan.scalar_parameters.is_empty()
        || plan.structural_parameters.len() != 1
        || parameter.position != 0
        || parameter.multiplicity != psi_language_semantics::Multiplicity::Affine
        || parameter.access != psi_checked_trees::CheckedStructuralAccess::Owned
        || parameter.qualifications.len() != 1
        || plan.operations.len() != 2
        || !matches!(
            plan.operations.last(),
            Some(CheckedUnitEffectOperationPlan::ReturnUnit { .. })
        )
    {
        return Err("the caller widened beyond one free owned Service hop");
    }
    let forwards = plan
        .operations
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::CallUnit {
                coordinate,
                target_machine,
                target_state,
                structural_arguments,
                claim_transfers,
                ..
            } => Some((
                coordinate,
                target_machine,
                target_state,
                structural_arguments,
                claim_transfers,
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [(coordinate, target_machine, target_state, structural_arguments, claim_transfers)] =
        forwards.as_slice()
    else {
        return Err("the caller does not retain exactly one internal call");
    };
    let [argument] = structural_arguments.as_slice() else {
        return Err("the forwarding edge does not retain exactly one argument");
    };
    if !claim_transfers.is_empty()
        || argument.source_parameter_index != 0
        || !argument.path.is_empty()
        || argument.byte_sequence_literal.is_some()
        || argument.type_identity != parameter.type_identity
        || argument.access != psi_checked_trees::CheckedStructuralAccess::Owned
    {
        return Err("the forwarding edge is not one whole-root owned move");
    }
    let targets = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .filter(|candidate| {
            candidate.machine == **target_machine && candidate.state == **target_state
        })
        .collect::<Vec<_>>();
    let [target] = targets.as_slice() else {
        return Err("the forwarding edge has no unique checked target plan");
    };
    let [target_parameter] = target.structural_parameters.as_slice() else {
        return Err("the forwarding target does not retain one exact carrier");
    };
    let Some(target_receipt) = target_parameter.fused_service_erasure.as_ref() else {
        return Err("the forwarding target lost its routed Service receipt");
    };
    if target.attachment_type_identity.is_some()
        || !target.scalar_parameters.is_empty()
        || target_parameter.position != 0
        || target_parameter.type_identity != parameter.type_identity
        || target_parameter.multiplicity != parameter.multiplicity
        || target_parameter.access != parameter.access
        || target_parameter.qualifications != parameter.qualifications
        || target_receipt.carrier_type_identity != receipt.carrier_type_identity
        || target_receipt.requirement != receipt.requirement
        || target_receipt.provider_plan_digest != receipt.provider_plan_digest
    {
        return Err("the forwarding target substituted carrier, domain, requirement, or plan");
    }
    let Some(requirement) = checked
        .traits()
        .iter()
        .find(|definition| definition.is_boundary && definition.symbol == receipt.requirement)
    else {
        return Err("the forwarding route lost its boundary requirement");
    };
    let requirement_states = checked.trait_machine_signatures(requirement);
    let Some((target_return, target_body)) = target.operations.split_last() else {
        return Err("the forwarding target has no checked body");
    };
    if !matches!(
        target_return,
        CheckedUnitEffectOperationPlan::ReturnUnit { .. }
    ) || target_body.is_empty()
        || target_body.iter().any(|operation| {
            !matches!(operation,
                CheckedUnitEffectOperationPlan::BoundaryCall { target_state, .. }
                    if requirement_states.iter().any(|signature| signature.symbol == *target_state))
        })
    {
        return Err("the forwarding target does not terminate in direct requirement calls");
    }

    let flow_states = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .map(|(_, flow)| flow)
        .filter(|flow| flow.machine_symbol == machine.symbol && flow.state_symbol == state.symbol)
        .collect::<Vec<_>>();
    let [flow_state] = flow_states.as_slice() else {
        return Err("the forwarding caller has no unique flow state");
    };
    let source_calls = checked
        .facts
        .flow
        .control
        .calls
        .span_or_empty(flow_state.calls);
    if !matches!(source_calls, [source]
        if source.statement_index == usize::try_from(coordinate.statement_index).unwrap_or(usize::MAX)
            && source.call_ordinal == usize::try_from(coordinate.call_ordinal).unwrap_or(usize::MAX)
            && source.target_symbol == **target_state
            && !source.has_receiver)
    {
        return Err("the forwarding plan does not rejoin one exact internal source call");
    }
    Ok(())
}

fn base_and_qualifications(
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

fn reject_unsupported_receipts(checked: &CheckedTrees, diagnostics: &mut Vec<Diagnostic>) {
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
