pub(crate) fn validate_paired_calling_plans(
    source: &omega_program_entry_plan::SelectedProgramEntrySourceSignature,
    semantic: &omega_calling_conventions::BoundaryEntryPlan,
    storage: &omega_program_entry_plan::SelectedProgramStorageEntryPlan,
) -> Result<(), String> {
    let slot = source.target_slot();
    let (Some(expected_semantic), Some(expected_physical)) = (
        slot.semantic_calling_convention,
        slot.physical_calling_convention,
    ) else {
        return Err(
            "selected ProgramEntry has an incomplete two-surface calling declaration".into(),
        );
    };
    let expected_policy = |convention| match convention {
        omega_target::ProgramEntryCallingConvention::MicrosoftX64 => {
            omega_calling_conventions::CallingPolicy::MicrosoftX64
        }
    };
    if storage.target_slot() != slot || semantic.call.policy != expected_policy(expected_semantic) {
        return Err(
            "selected ProgramEntry semantic calling plan drifted from its target slot".into(),
        );
    }
    let signature = omega_calling_conventions::CallSignature {
        parameters: source
            .visible_parameters()
            .iter()
            .map(|parameter| parameter.value_shape())
            .collect(),
        result: None,
    };
    let validated_semantic =
        omega_calling_conventions::validate_boundary_entry_plan(semantic.clone(), &signature)
            .map_err(|error| format!("selected ProgramEntry semantic plan is invalid: {error}"))?;
    let matching_methods = storage
        .schema()
        .methods
        .iter()
        .filter(|method| method.requirement_identity == storage.requirement_identity())
        .collect::<Vec<_>>();
    let [method] = matching_methods.as_slice() else {
        return Err(
            "selected ProgramEntry storage plan lost its unique semantic requirement".into(),
        );
    };
    if method.calling_plan_report_fingerprint
        != Some(validated_semantic.contract_report_fingerprint())
        || method.calling_plan_commitment.map(|value| value.as_bytes())
            != Some(validated_semantic.contract_commitment_digest())
        || method.parameter_type_identities
            != source
                .visible_parameters()
                .iter()
                .map(|parameter| parameter.normalized_type_identity().to_owned())
                .collect::<Vec<_>>()
        || method.has_result
        || method.result_type_identity.is_some()
    {
        return Err(
            "selected ProgramEntry semantic plan is not paired with its source signature".into(),
        );
    }
    let physical = storage
        .physical_contract()
        .ok_or_else(|| "selected ProgramEntry lost its physical calling contract".to_owned())?;
    let physical_plan = physical.boundary_entry_plan();
    let physical_signature = omega_calling_conventions::CallSignature {
        parameters: physical_plan
            .call
            .parameters
            .iter()
            .map(|placement| placement.shape)
            .collect(),
        result: physical_plan
            .call
            .result
            .as_ref()
            .map(|placement| placement.shape),
    };
    let validated_physical = omega_calling_conventions::validate_boundary_entry_plan(
        physical_plan.clone(),
        &physical_signature,
    )
    .map_err(|error| format!("selected ProgramEntry physical plan is invalid: {error}"))?;
    if physical.target_slot() != slot
        || physical.requirement_identity() != slot.physical_arrival_requirement.unwrap_or_default()
        || physical_plan.call.policy != expected_policy(expected_physical)
        || validated_physical.contract_report_fingerprint()
            != physical.calling_plan_report_fingerprint()
    {
        return Err("selected ProgramEntry physical plan drifted from its target contract".into());
    }
    Ok(())
}
