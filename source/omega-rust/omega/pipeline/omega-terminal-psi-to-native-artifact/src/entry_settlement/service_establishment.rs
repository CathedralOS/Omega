//! Independent native-settlement replay for selected Fused application roots.

use super::{NativeProgramEntrySettlement, NativeProgramEntrySettlementError};

pub(super) fn validate_terminal_rows(
    module: &psi_terminal::TerminalModule,
    settlement: NativeProgramEntrySettlement<'_>,
) -> Result<(), NativeProgramEntrySettlementError> {
    let rows = settlement.fused_service_establishments();
    if rows.is_empty() {
        return Ok(());
    }
    let Some(receiver_identity) = settlement.source().receiver().normalized_type_identity() else {
        return Err(NativeProgramEntrySettlementError::FusedServiceEstablishmentDrift);
    };
    let entry_machines = module
        .machines
        .iter()
        .filter(|machine| machine.id == module.entry)
        .collect::<Vec<_>>();
    let [entry] = entry_machines.as_slice() else {
        return Err(NativeProgramEntrySettlementError::FusedServiceEstablishmentDrift);
    };
    let Some(attachment) = entry.attachment else {
        return Err(NativeProgramEntrySettlementError::FusedServiceEstablishmentDrift);
    };
    let attachment_types = module
        .structural_types
        .iter()
        .filter(|declaration| declaration.id == attachment)
        .collect::<Vec<_>>();
    let [attachment_type] = attachment_types.as_slice() else {
        return Err(NativeProgramEntrySettlementError::FusedServiceEstablishmentDrift);
    };
    let psi_terminal::StructuralTypeShape::Record { fields } = &attachment_type.shape else {
        return Err(NativeProgramEntrySettlementError::FusedServiceEstablishmentDrift);
    };
    if attachment_type.identity != receiver_identity
        || attachment_type.identity != rows[0].attachment_type_identity()
        || rows.iter().any(|row| {
            row.source_signature_identity() != settlement.source().identity()
                || row.target_slot() != settlement.source().target_slot()
                || row.receiver_type_identity() != receiver_identity
                || row.attachment_type_identity() != attachment_type.identity
        })
    {
        return Err(NativeProgramEntrySettlementError::FusedServiceEstablishmentDrift);
    }
    for row in rows {
        let matching_fields = fields
            .iter()
            .filter(|field| field.identity == row.field_identity())
            .collect::<Vec<_>>();
        let [field] = matching_fields.as_slice() else {
            return Err(NativeProgramEntrySettlementError::FusedServiceEstablishmentDrift);
        };
        if !matches!(
            &field.field_type,
            psi_terminal::StructuralFieldType::Erased { type_identity }
                if type_identity == row.carrier_type_identity()
        ) {
            return Err(NativeProgramEntrySettlementError::FusedServiceEstablishmentDrift);
        }
    }
    Ok(())
}

pub(crate) fn validate_for_artifact_and_selected_plans(
    artifact: &psi_terminal_codec::CanonicalTerminalArtifact,
    settlement: NativeProgramEntrySettlement<'_>,
    selected_provider_plans: &omega_effects::SelectedProviderPlanFacts,
) -> Result<(), NativeProgramEntrySettlementError> {
    artifact.validate().map_err(|error| {
        NativeProgramEntrySettlementError::CanonicalArtifactReplay(error.to_string())
    })?;
    let module = psi_terminal_codec::decode_module(artifact.semantic_bytes()).map_err(|error| {
        NativeProgramEntrySettlementError::CanonicalArtifactReplay(error.to_string())
    })?;
    validate_terminal_rows(&module, settlement)?;
    for row in settlement.fused_service_establishments() {
        let matching_plans = selected_provider_plans
            .plans()
            .iter()
            .filter(|plan| {
                omega_program_entry_plan::ProgramEntryFusedServiceEstablishment::requirement_identity_for_schema(&plan.schema)
                    == row.requirement_identity()
                    && plan.schema.identity_digest() == row.service_schema_digest()
                    && plan.identity_digest() == row.selected_provider_plan_digest()
            })
            .count();
        if matching_plans != 1 {
            return Err(NativeProgramEntrySettlementError::FusedServiceEstablishmentDrift);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_for_artifact_and_selected_plans;
    use crate::NativeProgramEntrySettlement;
    use crate::tests::native_realization::entry_settlement::fused_service_custody;
    use omega_effects::provider_plan::{
        ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceSchema,
    };

    #[test]
    fn selected_plan_digest_rejoins_exact_fused_root() {
        let (artifact, _, source, fixture_row) = fused_service_custody();
        let plan = provider_plan();
        let row = omega_program_entry_plan::ProgramEntryFusedServiceEstablishment::new(
            source.identity(),
            source.target_slot(),
            fixture_row.receiver_type_identity().into(),
            fixture_row.attachment_type_identity().into(),
            fixture_row.field_identity().into(),
            fixture_row.carrier_type_identity().into(),
            fixture_row.carrier_base_identity().into(),
            fixture_row.bound_domain_identity().into(),
            omega_program_entry_plan::ProgramEntryFusedServiceEstablishment::requirement_identity_for_schema(&plan.schema),
            plan.schema.identity_digest(),
            plan.identity_digest(),
        )
        .expect("selected-plan-bound Fused root");
        let selected =
            omega_effects::SelectedProviderPlanFacts::from_selected_plans(vec![plan.clone()])
                .expect("selected provider facts");
        validate_for_artifact_and_selected_plans(
            &artifact,
            NativeProgramEntrySettlement::new(&source, None, std::slice::from_ref(&row)),
            &selected,
        )
        .expect("exact selected plan rejoins the Fused root");

        let mut substituted_plan = plan;
        substituted_plan.name.push_str("-substituted");
        let substituted =
            omega_effects::SelectedProviderPlanFacts::from_selected_plans(vec![substituted_plan])
                .expect("well-formed substituted provider facts");
        assert!(
            validate_for_artifact_and_selected_plans(
                &artifact,
                NativeProgramEntrySettlement::new(&source, None, &[row]),
                &substituted,
            )
            .is_err(),
            "a selected provider-plan digest substitution must reject",
        );
    }

    fn provider_plan() -> ProviderPlan {
        let requirement = "EvidenceService::ping";
        ProviderPlan {
            name: "evidence-provider".into(),
            provider_type: "EvidenceProvider".into(),
            provider_type_package_identity: None,
            target: "windows_x64".into(),
            schema: ServiceSchema {
                trait_name: "EvidenceService".into(),
                trait_package_identity: None,
                methods: vec![ServiceMethod {
                    name: "ping".into(),
                    requirement_owner: "EvidenceService".into(),
                    requirement_owner_package_identity: None,
                    requirement_identity: requirement.into(),
                    parameter_count: 0,
                    parameter_type_identities: Vec::new(),
                    entry_claims: Vec::new(),
                    has_result: false,
                    result_type_identity: None,
                    result_claims: Vec::new(),
                    service_reach: vec!["EvidenceService".into()],
                    synchronous_invocations: Vec::new(),
                    may_suspend: false,
                    may_block: false,
                    terminates_guarantee: false,
                    termination_premises: Vec::new(),
                    calling_plan_report_fingerprint: None,
                    calling_plan_commitment: None,
                }],
            },
            rows: vec![ProviderPlanRow {
                method: "ping".into(),
                requirement_identity: requirement.into(),
                requirement_lifetime_partition: Vec::new(),
                binding: ProviderBinding::CheckedAdapter {
                    machine_identity: "EvidenceProvider::ping".into(),
                    machine_package_identity: None,
                },
            }],
            origin_package_identity: None,
            origin_package: "test".into(),
        }
    }
}
