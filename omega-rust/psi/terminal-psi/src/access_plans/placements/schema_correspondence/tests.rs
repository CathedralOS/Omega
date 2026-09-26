//! Schema correspondence tests.

use super::{
    DeviceRevisionPredicateId, ResourceProfileReceiptId, RuntimeDeviceRevisionEvidence,
    RuntimeDeviceRevisionObservationId, SchemaCorrespondenceProviderId,
    SchemaCorrespondenceSourceId, SchemaDeviceCorrespondenceGrant,
    SchemaDeviceCorrespondenceReceiptContext, StableDeviceInstanceId, ValidatedPlacementPlan,
    admit_schema_device_correspondence,
};
use crate::access_plans::{AccessPlan, BoundaryReach, PlacementPlan, validate_placement_plan};
use crate::layout_plans::LayoutPlanReport;

fn provider_id(identity: u64) -> SchemaCorrespondenceProviderId {
    SchemaCorrespondenceProviderId::from_normalized_identity(identity).expect("provider")
}

fn device_id(identity: u64) -> StableDeviceInstanceId {
    StableDeviceInstanceId::from_normalized_identity(identity).expect("device")
}

fn source(identity: u64) -> SchemaCorrespondenceSourceId {
    SchemaCorrespondenceSourceId::from_normalized_identity(identity).expect("source")
}

fn receipt_id(identity: u64) -> ResourceProfileReceiptId {
    ResourceProfileReceiptId::from_normalized_identity(identity).expect("profile receipt")
}

fn test_placement(schema_report_fingerprint: u64) -> ValidatedPlacementPlan {
    let layout = LayoutPlanReport {
        schema_report_fingerprint,
        entries: Vec::new(),
        offsets: Some(Vec::new()),
        size: Some(0),
        align: 1,
    };
    let access = AccessPlan::inaccessible(&layout).expect("empty inaccessible access plan");
    validate_placement_plan(PlacementPlan {
        layout,
        access,
        reach: BoundaryReach::default(),
    })
    .expect("empty validated placement")
}

fn revision(
    provider: SchemaCorrespondenceProviderId,
    device: StableDeviceInstanceId,
    receipt: ResourceProfileReceiptId,
) -> RuntimeDeviceRevisionEvidence {
    RuntimeDeviceRevisionEvidence::from_admitted_provider(
        RuntimeDeviceRevisionObservationId::from_normalized_identity(41).expect("observation"),
        DeviceRevisionPredicateId::from_normalized_identity(42).expect("predicate"),
        provider,
        device,
        receipt,
        0x17,
    )
}

#[test]
fn correspondence_admission_retains_separate_provider_device_and_revision_evidence() {
    let provider = provider_id(7);
    let device = device_id(8);
    let receipt = receipt_id(9);
    let placement = test_placement(11);
    let grant = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        provider,
        device,
        source(10),
        &placement,
        receipt,
        Some(revision(provider, device, receipt)),
    )
    .expect("provider correspondence grant");

    let admitted = admit_schema_device_correspondence(grant, &placement, receipt)
        .expect("exact placement/profile join");
    assert_eq!(admitted.provider(), provider);
    assert_eq!(admitted.device(), device);
    assert_eq!(admitted.source().normalized_identity(), 10);
    assert_eq!(admitted.placement_plan(), &placement);
    assert_eq!(admitted.profile_receipt(), receipt);
    let revision = admitted.revision().expect("revision evidence");
    assert_eq!(revision.observed_revision(), 0x17);
    assert_eq!(revision.provider(), provider);
    assert_eq!(revision.device(), device);
    assert_eq!(revision.profile_receipt(), receipt);
}

#[test]
fn revision_and_admission_drift_return_exact_grants_for_retry() {
    let provider = provider_id(17);
    let device = device_id(18);
    let receipt = receipt_id(19);
    let placement = test_placement(21);
    let revision = revision(provider_id(99), device, receipt);
    let revision_observation = revision.observation();
    let rejection = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        provider,
        device,
        source(20),
        &placement,
        receipt,
        Some(revision),
    )
    .expect_err("foreign-provider revision evidence must reject");
    assert!(rejection.diagnostic().0.contains("stable device instance"));
    let (revision, _) = rejection.into_parts();
    assert_eq!(
        revision.expect("returned revision evidence").observation(),
        revision_observation
    );

    let grant = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        provider,
        device,
        source(20),
        &placement,
        receipt,
        None,
    )
    .expect("unconditional provider correspondence grant");
    let drifted_placement = test_placement(22);
    let rejection = admit_schema_device_correspondence(grant, &drifted_placement, receipt)
        .expect_err("placement drift must reject");
    assert!(rejection.diagnostic().0.contains("validated placement"));
    let (grant, _) = rejection.into_parts();

    let rejection = admit_schema_device_correspondence(grant, &placement, receipt_id(23))
        .expect_err("profile-grant drift must reject");
    assert!(rejection.diagnostic().0.contains("resource-profile grant"));
    let (grant, _) = rejection.into_parts();

    let admitted = admit_schema_device_correspondence(grant, &placement, receipt)
        .expect("returned provider grant supports corrected retry");
    assert_eq!(admitted.provider(), provider);
    assert_eq!(admitted.device(), device);
    assert!(admitted.revision().is_none());
}

#[test]
fn admission_replays_revision_binding_and_returns_grant_for_retry() {
    let provider = provider_id(27);
    let device = device_id(28);
    let receipt = receipt_id(29);
    let placement = test_placement(31);
    let mut grant = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        provider,
        device,
        source(30),
        &placement,
        receipt,
        Some(revision(provider, device, receipt)),
    )
    .expect("provider correspondence grant");
    grant.revision.as_mut().expect("revision evidence").device = device_id(99);

    let rejection = admit_schema_device_correspondence(grant, &placement, receipt)
        .expect_err("admission must independently replay revision binding");
    assert!(rejection.diagnostic().0.contains("could not replay"));
    let (mut grant, _) = rejection.into_parts();
    grant
        .revision
        .as_mut()
        .expect("returned revision evidence")
        .device = device;

    let admitted = admit_schema_device_correspondence(grant, &placement, receipt)
        .expect("repaired grant remains valid for retry");
    assert_eq!(admitted.device(), device);
    assert_eq!(
        admitted.revision().expect("revision evidence").device(),
        device
    );
}

#[test]
fn receipt_context_compares_complete_correspondence_structure() {
    let provider = provider_id(37);
    let device = device_id(38);
    let receipt = receipt_id(39);
    let placement = test_placement(40);
    let grant = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        provider,
        device,
        source(41),
        &placement,
        receipt,
        Some(revision(provider, device, receipt)),
    )
    .expect("provider correspondence grant");
    let admitted = admit_schema_device_correspondence(grant, &placement, receipt)
        .expect("exact placement/profile join");
    let context = admitted.receipt_context();

    let assert_drift = |drifted: &SchemaDeviceCorrespondenceReceiptContext| {
        assert_eq!(context.device(), drifted.device());
        assert_eq!(context.placement.identity(), drifted.placement.identity());
        assert_ne!(&context, drifted);
    };

    assert_eq!(context, context.clone());
    assert_eq!(context.provider(), provider);
    assert_eq!(context.device(), device);

    let mut provider_drift = context.clone();
    provider_drift.provider = provider_id(42);
    assert_drift(&provider_drift);

    let mut source_drift = context.clone();
    source_drift.source = source(43);
    assert_drift(&source_drift);

    let mut profile_drift = context.clone();
    profile_drift.profile_receipt = receipt_id(44);
    assert_drift(&profile_drift);

    let mut revision_absent = context.clone();
    revision_absent.revision = None;
    assert_drift(&revision_absent);

    let mut revision_observation_drift = context.clone();
    revision_observation_drift
        .revision
        .as_mut()
        .expect("revision context")
        .observation =
        RuntimeDeviceRevisionObservationId::from_normalized_identity(46).expect("observation");
    assert_drift(&revision_observation_drift);

    let mut revision_predicate_drift = context.clone();
    revision_predicate_drift
        .revision
        .as_mut()
        .expect("revision context")
        .predicate = DeviceRevisionPredicateId::from_normalized_identity(47).expect("predicate");
    assert_drift(&revision_predicate_drift);

    let mut revision_provider_drift = context.clone();
    revision_provider_drift
        .revision
        .as_mut()
        .expect("revision context")
        .provider = provider_id(48);
    assert_drift(&revision_provider_drift);

    let mut revision_device_drift = context.clone();
    revision_device_drift
        .revision
        .as_mut()
        .expect("revision context")
        .device = device_id(49);
    assert_drift(&revision_device_drift);

    let mut revision_profile_drift = context.clone();
    revision_profile_drift
        .revision
        .as_mut()
        .expect("revision context")
        .profile_receipt = receipt_id(50);
    assert_drift(&revision_profile_drift);

    let mut revision_drift = context.clone();
    revision_drift
        .revision
        .as_mut()
        .expect("revision context")
        .observed_revision = 0x18;
    assert_drift(&revision_drift);

    let mut placement_drift = context.clone();
    placement_drift.placement.layout.schema_report_fingerprint = 45;
    assert_drift(&placement_drift);
}
