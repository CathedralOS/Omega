//! Independent settlement of exact hosted source and checked ProgramEntry custody.

use crate::tests::fixtures::{checked_source::checked, hosted::hosted_custody};
use crate::{
    NativeProgramEntrySettlement, NativeProgramEntrySettlementError,
    validate_native_program_entry_settlement,
};

#[test]
fn independently_settles_exact_hosted_source_and_entry() {
    let (artifact, receipt, source) = hosted_custody();
    let settlement = validate_native_program_entry_settlement(
        &artifact,
        &receipt,
        NativeProgramEntrySettlement::new(&source, None, &[]),
        omega_target::NativeTarget::windows_x64(),
    )
    .expect("independent ProgramEntry settlement");

    assert_eq!(settlement.source(), &source);
    assert_eq!(settlement.checked_entry(), &receipt);
    assert_eq!(
        settlement.target(),
        omega_target::NativeTarget::windows_x64()
    );
    assert!(settlement.semantic_boundary_entry_plan().is_none());
    assert!(settlement.storage_entry().is_none());
    assert!(settlement.fused_service_establishments().is_empty());
}

#[test]
fn independently_settles_exact_fused_service_root() {
    let (artifact, receipt, source, establishment) = fused_service_custody();
    let rows = [establishment.clone()];
    let settlement = validate_native_program_entry_settlement(
        &artifact,
        &receipt,
        NativeProgramEntrySettlement::new(&source, None, &rows),
        omega_target::NativeTarget::windows_x64(),
    )
    .expect("independent Fused root settlement");
    assert_eq!(settlement.fused_service_establishments(), rows);

    let substituted = establishment_for_source(
        &source,
        establishment.attachment_type_identity(),
        "other_service",
        establishment.carrier_type_identity(),
    );
    assert_eq!(
        validate_native_program_entry_settlement(
            &artifact,
            &receipt,
            NativeProgramEntrySettlement::new(&source, None, &[substituted]),
            omega_target::NativeTarget::windows_x64(),
        ),
        Err(NativeProgramEntrySettlementError::FusedServiceEstablishmentDrift),
    );

    for substituted in [
        establishment_for_source(
            &source,
            "other_attachment",
            establishment.field_identity(),
            establishment.carrier_type_identity(),
        ),
        establishment_for_source(
            &source,
            establishment.attachment_type_identity(),
            establishment.field_identity(),
            "other_carrier",
        ),
    ] {
        assert_eq!(
            validate_native_program_entry_settlement(
                &artifact,
                &receipt,
                NativeProgramEntrySettlement::new(&source, None, &[substituted]),
                omega_target::NativeTarget::windows_x64(),
            ),
            Err(NativeProgramEntrySettlementError::FusedServiceEstablishmentDrift),
        );
    }

    assert_eq!(
        validate_native_program_entry_settlement(
            &artifact,
            &receipt,
            NativeProgramEntrySettlement::new(
                &source,
                None,
                &[establishment.clone(), establishment],
            ),
            omega_target::NativeTarget::windows_x64(),
        ),
        Err(NativeProgramEntrySettlementError::FusedServiceEstablishmentDrift),
    );
}

pub(crate) fn fused_service_custody() -> (
    psi_terminal_codec::CanonicalTerminalArtifact,
    psi_checked_trees_to_terminal::CheckedProgramEntryTerminalReceipt,
    omega_program_entry_plan::SelectedProgramEntrySourceSignature,
    omega_program_entry_plan::ProgramEntryFusedServiceEstablishment,
) {
    let checked = checked(
        r#"
            data Evidence {}
            data Main { service [erased]: Evidence; }
            machine Main::launch(&mut self) {}
        "#,
    );
    let selection = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find(|machine| machine.name == "Main::launch")
        .expect("terminal selection");
    let provisional =
        omega_program_entry_plan::SelectedProgramEntrySourceSignature::from_checked_typed_entry(
            omega_target::TargetProfile::WindowsX64.program_entry_slot(),
            selection.machine,
            selection.machine,
            selection.name.clone(),
            "entry".into(),
            "test::Main::launch(&mut self) -> Unit".into(),
            omega_program_entry_plan::ProgramEntrySourceReceiverSignature::ProvisionedMutable {
                normalized_type_identity: "provisional".into(),
            },
            Vec::new(),
        )
        .expect("provisional hosted source signature");
    let provisional_artifact =
        psi_checked_trees_to_terminal::produce_program_entry_terminal_artifact(
            &checked,
            "Main::launch",
            provisional.identity().bytes(),
        )
        .expect("provisional ProgramEntry Terminal artifact");
    let module =
        psi_terminal_codec::decode_module(provisional_artifact.artifact().semantic_bytes())
            .expect("decode provisional Terminal module");
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("unique Terminal entry");
    let attachment = entry.attachment.expect("attached Terminal entry");
    let attachment_type = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == attachment)
        .expect("entry attachment type");
    let psi_terminal::StructuralTypeShape::Record { fields } = &attachment_type.shape else {
        panic!("entry attachment remains a record")
    };
    let field = fields
        .iter()
        .find(|field| field.identity == "service")
        .expect("erased service field");
    let psi_terminal::StructuralFieldType::Erased { type_identity } = &field.field_type else {
        panic!("service fixture field remains erased")
    };
    let source =
        omega_program_entry_plan::SelectedProgramEntrySourceSignature::from_checked_typed_entry(
            omega_target::TargetProfile::WindowsX64.program_entry_slot(),
            selection.machine,
            selection.machine,
            selection.name.clone(),
            "entry".into(),
            "test::Main::launch(&mut self) -> Unit".into(),
            omega_program_entry_plan::ProgramEntrySourceReceiverSignature::ProvisionedMutable {
                normalized_type_identity: attachment_type.identity.clone(),
            },
            Vec::new(),
        )
        .expect("hosted source signature");
    let produced = psi_checked_trees_to_terminal::produce_program_entry_terminal_artifact(
        &checked,
        "Main::launch",
        source.identity().bytes(),
    )
    .expect("ProgramEntry Terminal artifact");
    let establishment =
        establishment_for_source(&source, &attachment_type.identity, "service", type_identity);
    let (artifact, receipt, _) = produced.into_parts();
    (artifact, receipt, source, establishment)
}

fn establishment_for_source(
    source: &omega_program_entry_plan::SelectedProgramEntrySourceSignature,
    attachment_type_identity: &str,
    field_identity: &str,
    carrier_type_identity: &str,
) -> omega_program_entry_plan::ProgramEntryFusedServiceEstablishment {
    omega_program_entry_plan::ProgramEntryFusedServiceEstablishment::new(
        source.identity(),
        source.target_slot(),
        attachment_type_identity.into(),
        attachment_type_identity.into(),
        field_identity.into(),
        carrier_type_identity.into(),
        carrier_type_identity.into(),
        "Bound".into(),
        "Evidence#test".into(),
        omega_effects::provider_plan::ServiceSchemaDigest::from_digest([41; 32]),
        omega_effects::provider_plan::ProviderPlanDigest::from_digest([43; 32]),
    )
    .expect("well-formed Fused root establishment")
}
