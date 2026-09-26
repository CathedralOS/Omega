use super::{
    extent_rights, field_key, stable_word_placement, stable_word_profile, uart_extent_with_lineage,
    uart_placement_plan, uart_reach, uart_resource_profile_data,
};
use crate::{
    AdmittedResourceProfile, AdmittedSchemaDeviceCorrespondence, ExternalPrimitiveOperation,
    ObservationModel, OwnedPlacementAdmission, PlacedOccurrenceId, PlacementAdmissionId,
    ResourceProfileGrant, ResourceProfileReceiptId, SchemaCorrespondenceProviderId,
    SchemaCorrespondenceSourceId, SchemaDeviceCorrespondenceGrant, StableDeviceInstanceId,
    ValidatedPlacementPlan, admit_owned_placement, adopt_owned_external,
};

fn owned_external_profile(extent: &extents::Extent, receipt: u64) -> AdmittedResourceProfile {
    ResourceProfileGrant::from_admitted_provider(
        ResourceProfileReceiptId::from_normalized_identity(receipt)
            .expect("external profile receipt"),
        extent,
        extent_rights(&[3]),
        uart_reach(),
    )
    .expect("external resource-profile grant")
    .admit(uart_resource_profile_data(extent.length(), &uart_reach()))
    .expect("admitted external resource profile")
}

fn device_correspondence(
    plan: &ValidatedPlacementPlan,
    profile: &AdmittedResourceProfile,
) -> AdmittedSchemaDeviceCorrespondence {
    SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        SchemaCorrespondenceProviderId::from_normalized_identity(703)
            .expect("correspondence provider"),
        StableDeviceInstanceId::from_normalized_identity(704).expect("stable device"),
        SchemaCorrespondenceSourceId::from_normalized_identity(705).expect("correspondence source"),
        plan,
        profile.receipt(),
        None,
    )
    .expect("correspondence grant")
    .admit(plan, profile)
    .expect("admitted correspondence")
}

fn owned_external_admission(
    plan: &ValidatedPlacementPlan,
    base: u64,
    admission_identity: u64,
) -> (OwnedPlacementAdmission, AdmittedSchemaDeviceCorrespondence) {
    let extent = uart_extent_with_lineage(base, 12, 701);
    let profile = owned_external_profile(&extent, 702);
    let correspondence = device_correspondence(plan, &profile);
    let admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(admission_identity)
            .expect("admission identity"),
        extent,
        plan,
        &profile,
    )
    .expect("owned External admission");
    (admission, correspondence)
}

#[test]
fn owned_external_adoption_establishes_projects_and_retires_corresponded_placement() {
    let plan = uart_placement_plan();
    let (admission, correspondence) = owned_external_admission(&plan, 0xd000, 706);

    let dormant = adopt_owned_external(admission, correspondence)
        .expect("provider-corresponded External adoption");
    assert_eq!(dormant.admission().normalized_identity(), 706);
    assert_eq!(dormant.placement_plan().identity(), plan.identity());
    assert_eq!(dormant.profile_receipt().normalized_identity(), 702);
    assert_eq!(dormant.extent().base(), 0xd000);
    assert_eq!(dormant.correspondence().device().normalized_identity(), 704);

    let established = dormant
        .view(PlacedOccurrenceId::from_normalized_identity(707).expect("occurrence"))
        .expect("owned External view");
    assert_eq!(established.occurrence().normalized_identity(), 707);
    assert_eq!(established.extent().base(), 0xd000);

    let projection = established
        .project(field_key(plan.access(), "status"))
        .expect("External status projection");
    assert_eq!(projection.observation(), ObservationModel::External);
    assert_eq!(projection.resident_claim(), None);
    assert_eq!(
        projection
            .placed_occurrence()
            .map(|occurrence| occurrence.normalized_identity()),
        Some(707)
    );
    assert!(projection.correspondence().is_some());
    let read = projection
        .read()
        .expect("repeatable External read")
        .into_primitive_request()
        .into_external_primitive_access()
        .expect("corresponded External read specialization");
    assert_eq!(read.operation(), ExternalPrimitiveOperation::Read);
    assert_eq!(read.primitive_address(), 0xd000);

    let dormant = established.retire().expect("External retirement");
    let (admission, _correspondence) = dormant.into_parts();
    let _extent = admission.withdraw();
}

#[test]
fn owned_external_adoption_rejects_drifted_placement_binding() {
    let plan = uart_placement_plan();
    let drifted_plan = stable_word_placement();
    let (admission, _correspondence) = owned_external_admission(&plan, 0xd100, 710);
    // A correspondence admitted against a different placement cannot bind
    // this admission's exact plan identity.
    let drift_extent = uart_extent_with_lineage(0xd180, 4, 711);
    let drift_correspondence =
        device_correspondence(&drifted_plan, &owned_external_profile(&drift_extent, 702));
    let error = adopt_owned_external(admission, drift_correspondence)
        .expect_err("drifted placement must reject");
    assert!(
        error
            .diagnostic()
            .0
            .contains("exact plan and resource-profile receipt")
    );
    let (admission, _correspondence, _diagnostic) = error.into_parts();
    assert_eq!(admission.identity.normalized_identity(), 710);
    let _extent = admission.withdraw();
}

#[test]
fn owned_external_adoption_rejects_stable_observation_roster() {
    let plan = stable_word_placement();
    let extent = uart_extent_with_lineage(0xd200, 4, 712);
    let profile = stable_word_profile(&extent);
    let correspondence = device_correspondence(&plan, &profile);
    let admission = admit_owned_placement(
        PlacementAdmissionId::from_normalized_identity(713).expect("admission identity"),
        extent,
        &plan,
        &profile,
    )
    .expect("owned Stable admission");
    let error = adopt_owned_external(admission, correspondence)
        .expect_err("Stable-observation roster cannot adopt the External route");
    assert!(error.diagnostic().0.contains("External"));
    let (admission, _correspondence, _diagnostic) = error.into_parts();
    let _extent = admission.withdraw();
}
