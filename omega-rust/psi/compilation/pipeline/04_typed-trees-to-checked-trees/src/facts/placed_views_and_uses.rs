//! Checked placed view inputs and nominal machine use facts.

use typed_trees::TypedTrees;

pub(crate) fn build_checked_placed_view_inputs(
    program: &TypedTrees,
) -> Vec<checked_trees::CheckedPlacedViewInput> {
    let mut inputs = Vec::new();
    for machine in program.machines() {
        if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
            || !machine.body_is_present
            || !machine.lifetime_parameters.is_empty()
            || !program.machine_type_parameters(machine).is_empty()
        {
            continue;
        }
        for state in program.machine_states(machine) {
            for (position, parameter) in program.state_parameters(state).iter().enumerate() {
                let typed_trees::types::TypeReferenceNode::Reference {
                    referee,
                    access,
                    lifetime: _,
                } = program
                    .type_reference_table
                    .type_reference(parameter.type_reference)
                else {
                    continue;
                };
                let Some(view) = program.placed_view_plan_for_type_reference(*referee) else {
                    continue;
                };
                inputs.push(checked_trees::CheckedPlacedViewInput {
                    machine: machine.symbol,
                    state: state.symbol,
                    position: u32::try_from(position)
                        .expect("state parameter count must fit checked input position"),
                    parameter: parameter.symbol,
                    reference_access: *access,
                    binding_is_const: parameter.is_const,
                    binding_is_mutable: parameter.is_mutable,
                    view: view.data_symbol,
                    policy: view.policy_symbol,
                    policy_plan_machine: view.policy_plan_machine_symbol,
                    schema: view.schema_symbol,
                    placement: view.placement.clone(),
                });
            }
        }
    }
    inputs
}

pub(crate) fn build_nominal_machine_use_facts(
    program: &TypedTrees,
    nominal_machine_uses: Vec<validation::ValidatedNominalMachineUse>,
    contract_plans: &checked_trees::MachineContractPlans,
) -> Result<checked_trees::NominalMachineUseFacts, Vec<diagnostics::Diagnostic>> {
    let mut checked = Vec::with_capacity(nominal_machine_uses.len());
    for nominal_use in nominal_machine_uses {
        let Some(published) = contract_plans.crash_capsule(
            nominal_use.satisfaction_trait,
            nominal_use.satisfaction_requirement,
        ) else {
            return Err(vec![diagnostics::Diagnostic::error(
                "admitted nominal machine use is missing its published requirement contract identity",
            )]);
        };
        let Some(actual) = contract_plans.for_machine(nominal_use.selected_machine) else {
            return Err(vec![diagnostics::Diagnostic::error(
                "admitted nominal machine use is missing its selected machine contract identity",
            )]);
        };
        let Some(actual_envelope) = contract_plans.realized_envelope(nominal_use.selected_machine)
        else {
            return Err(vec![diagnostics::Diagnostic::error(
                "admitted nominal machine use is missing its realized contract envelope",
            )]);
        };
        let published_fingerprint = published.target_contract_report_fingerprint();
        let published_commitment = published.target_contract_commitment();
        let actual_fingerprint = actual.report_fingerprint;
        if published_commitment.is_zero()
            || actual.commitment.is_zero()
            || actual_envelope.contract_report_fingerprint != actual_fingerprint
            || actual_envelope.contract_commitment != actual.commitment
        {
            return Err(vec![diagnostics::Diagnostic::error(
                "admitted nominal machine use retained an empty contract-envelope identity",
            )]);
        }
        let callback_placement = match program.boundary_calling_plan_identity(
            nominal_use.satisfaction_trait,
            nominal_use.satisfaction_requirement,
        ) {
            Some(boundary_calling_plan_identity) => {
                let Some(resource_envelope) = contract_plans
                    .resource_envelope(nominal_use.selected_machine, nominal_use.selected_entry)
                else {
                    return Err(vec![diagnostics::Diagnostic::error(
                        "admitted nominal callback use is missing its exact checked entry resource envelope",
                    )]);
                };
                let resource_receipt =
                    checked_trees::CheckedCallbackResourceReceipt::try_from_entry_envelope(
                        resource_envelope,
                    )
                    .map_err(|error| {
                        vec![diagnostics::Diagnostic::error(format!(
                            "admitted nominal callback resource receipt failed checked replay: {error}"
                        ))]
                })?;
                Some(checked_trees::CheckedCallbackPlacementIdentity {
                    boundary_calling_plan_report_fingerprint: boundary_calling_plan_identity
                        .report_fingerprint,
                    boundary_calling_plan_commitment: boundary_calling_plan_identity.commitment,
                    resource_receipt,
                })
            }
            None => None,
        };
        if callback_placement
            .is_some_and(|placement| placement.boundary_calling_plan_commitment.is_zero())
        {
            return Err(vec![diagnostics::Diagnostic::error(
                "admitted nominal callback use is missing its evaluated boundary calling-plan identity",
            )]);
        }
        checked.push(checked_trees::CheckedNominalMachineUse {
            site: match nominal_use.site {
                validation::ValidatedNominalMachineUseSite::Statement(handle) => {
                    checked_trees::NominalMachineUseSite::Statement(handle)
                }
                validation::ValidatedNominalMachineUseSite::Expression(handle) => {
                    checked_trees::NominalMachineUseSite::Expression(handle)
                }
            },
            registration_operation: nominal_use.registration_operation,
            static_machine_ordinal: nominal_use.static_machine_ordinal,
            selected_machine: nominal_use.selected_machine,
            selected_entry: nominal_use.selected_entry,
            satisfaction_trait: nominal_use.satisfaction_trait,
            satisfaction_requirement: nominal_use.satisfaction_requirement,
            canonical_requirement_overload: nominal_use.canonical_requirement_overload,
            published_requirement_envelope: checked_trees::CheckedMachineContractEnvelopeIdentity {
                contract_report_fingerprint: published_fingerprint,
                contract_commitment: published_commitment,
            },
            selected_actual_envelope: checked_trees::CheckedMachineContractEnvelopeIdentity {
                contract_report_fingerprint: actual_fingerprint,
                contract_commitment: actual.commitment,
            },
            callback_placement,
            refinement: checked_trees::CheckedMachineContractRefinement {
                published_requirement_report_fingerprint: published_fingerprint,
                published_requirement_commitment: published_commitment,
                selected_actual_report_fingerprint: actual_fingerprint,
                selected_actual_commitment: actual.commitment,
            },
        });
    }
    checked_trees::NominalMachineUseFacts::try_with_uses(checked)
        .map_err(|message| vec![diagnostics::Diagnostic::error(message)])
}
