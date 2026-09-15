use super::{
    admit_uart, field_key, primitive_request_snapshot, uart_extent_with_lineage,
    uart_placement_plan, uart_reach, uart_resource_profile,
};
use crate::{
    DeviceRevisionPredicateId, PlacementAdmissionId, PlacementPlanId, ResourceProfileReceiptId,
    RuntimeDeviceRevisionEvidence, RuntimeDeviceRevisionObservationId,
    SchemaCorrespondenceProviderId, SchemaCorrespondenceSourceId, SchemaDeviceCorrespondenceGrant,
    StableDeviceInstanceId, admit_placement, bind_schema_correspondence_to_placement, place,
};
use extents::LoanPolarity;

#[test]
fn provider_correspondence_admits_against_exact_plan_and_profile_without_storage_join() {
    let plan = uart_placement_plan();
    let extent = uart_extent_with_lineage(0x7180, 12, 236);
    let loan = extent.loan(0, 12).expect("shared UART loan");
    let profile = uart_resource_profile(&loan, &uart_reach());
    let provider = SchemaCorrespondenceProviderId::from_normalized_identity(237)
        .expect("correspondence provider");
    let device = StableDeviceInstanceId::from_normalized_identity(238).expect("stable device");
    let revision = RuntimeDeviceRevisionEvidence::from_admitted_provider(
        RuntimeDeviceRevisionObservationId::from_normalized_identity(239)
            .expect("revision observation"),
        DeviceRevisionPredicateId::from_normalized_identity(240).expect("revision predicate"),
        provider,
        device,
        profile.receipt(),
        3,
    );
    let grant = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        provider,
        device,
        SchemaCorrespondenceSourceId::from_normalized_identity(241).expect("datasheet provenance"),
        &plan,
        profile.receipt(),
        Some(revision),
    )
    .expect("provider correspondence grant");

    let mut colliding = plan.clone();
    colliding.layout.schema_report_fingerprint ^= 1;
    assert_eq!(colliding.identity(), plan.identity());
    assert_ne!(colliding.layout(), plan.layout());
    let rejection = grant
        .admit(&colliding, &profile)
        .expect_err("compact placement identity cannot substitute exact plan structure");
    assert!(
        rejection
            .diagnostic()
            .0
            .contains("exact validated placement")
    );
    let (grant, _) = rejection.into_parts();

    let admitted = grant
        .admit(&plan, &profile)
        .expect("exact plan/profile correspondence admission");
    assert_eq!(admitted.placement(), plan.identity());
    assert_eq!(admitted.profile_receipt(), profile.receipt());
    assert_eq!(admitted.provider(), provider);
    assert_eq!(admitted.device(), device);
    assert_eq!(
        admitted
            .revision()
            .expect("runtime revision evidence")
            .observed_revision(),
        3
    );
}

#[test]
fn correspondence_binding_replays_placement_and_returns_both_inputs_for_retry() {
    let plan = uart_placement_plan();
    let extent = uart_extent_with_lineage(0x7190, 12, 242);
    let loan = extent.loan(0, 12).expect("shared UART loan");
    let profile = uart_resource_profile(&loan, &uart_reach());
    let admission_id =
        PlacementAdmissionId::from_normalized_identity(243).expect("placement admission");
    let mut admission =
        admit_placement(admission_id, loan, &plan, &profile).expect("borrowed placement admission");
    let provider = SchemaCorrespondenceProviderId::from_normalized_identity(244)
        .expect("correspondence provider");
    let device = StableDeviceInstanceId::from_normalized_identity(245).expect("stable device");
    let grant = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        provider,
        device,
        SchemaCorrespondenceSourceId::from_normalized_identity(246).expect("datasheet provenance"),
        &plan,
        profile.receipt(),
        None,
    )
    .expect("provider correspondence grant");
    let correspondence = grant
        .admit(&plan, &profile)
        .expect("schema correspondence admission");

    admission.placement_plan.layout.schema_report_fingerprint ^= 1;
    assert_eq!(admission.placement_plan.identity(), plan.identity());
    let rejection = bind_schema_correspondence_to_placement(admission, correspondence)
        .expect_err("same compact identity cannot hide placement structure drift");
    assert!(rejection.diagnostic().0.contains("exact plan"));
    let (mut admission, mut correspondence, _) = rejection.into_parts();
    admission.placement_plan.layout.schema_report_fingerprint =
        plan.layout().schema_report_fingerprint;

    correspondence.replace_placement_for_test(PlacementPlanId(plan.identity().0 ^ 1));
    let rejection = bind_schema_correspondence_to_placement(admission, correspondence)
        .expect_err("placement identity drift must reject");
    assert!(rejection.diagnostic().0.contains("exact plan"));
    let (mut admission, mut correspondence, _) = rejection.into_parts();
    assert_eq!(admission.identity(), admission_id);
    correspondence.replace_placement_for_test(plan.identity());

    admission.profile_receipt =
        ResourceProfileReceiptId::from_normalized_identity(999).expect("drifted receipt");
    let rejection = bind_schema_correspondence_to_placement(admission, correspondence)
        .expect_err("admission receipt drift must reject");
    assert!(rejection.diagnostic().0.contains("exact plan"));
    let (mut admission, correspondence, _) = rejection.into_parts();
    admission.profile_receipt = profile.receipt();

    let bound = bind_schema_correspondence_to_placement(admission, correspondence)
        .expect("repaired inputs remain valid for retry");
    assert_eq!(bound.admission(), admission_id);
    assert_eq!(bound.correspondence().provider(), provider);
    assert_eq!(bound.correspondence().device(), device);
    let (loan, correspondence) = bound.withdraw();
    assert_eq!(loan.base(), 0x7190);
    assert_eq!(loan.length(), 12);
    assert_eq!(correspondence.placement(), plan.identity());
}

#[test]
fn corresponded_view_establishment_replays_both_inputs_and_preserves_retry() {
    let plan = uart_placement_plan();
    let extent = uart_extent_with_lineage(0x71a0, 12, 247);
    let loan = extent.loan(0, 12).expect("shared UART loan");
    let profile = uart_resource_profile(&loan, &uart_reach());
    let admission_id =
        PlacementAdmissionId::from_normalized_identity(248).expect("placement admission");
    let admission =
        admit_placement(admission_id, loan, &plan, &profile).expect("borrowed placement admission");
    let provider = SchemaCorrespondenceProviderId::from_normalized_identity(249)
        .expect("correspondence provider");
    let device = StableDeviceInstanceId::from_normalized_identity(250).expect("stable device");
    let grant = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        provider,
        device,
        SchemaCorrespondenceSourceId::from_normalized_identity(251).expect("datasheet provenance"),
        &plan,
        profile.receipt(),
        None,
    )
    .expect("provider correspondence grant");
    let correspondence = grant
        .admit(&plan, &profile)
        .expect("schema correspondence admission");
    let mut bound = bind_schema_correspondence_to_placement(admission, correspondence)
        .expect("correspondence placement binding");

    bound.replace_correspondence_placement_for_test(PlacementPlanId(plan.identity().0 ^ 1));
    let rejection = bound
        .establish_view()
        .expect_err("establishment must independently replay correspondence");
    assert!(rejection.diagnostic().0.contains("exact plan"));
    let (mut bound, _) = rejection.into_parts();
    assert_eq!(bound.admission(), admission_id);
    bound.replace_correspondence_placement_for_test(plan.identity());

    let view = bound
        .establish_view()
        .expect("repaired bound carrier remains valid for retry");
    assert_eq!(view.admission(), admission_id);
    assert_eq!(view.base(), 0x71a0);
    assert_eq!(view.length(), 12);
    assert_eq!(view.correspondence().provider(), provider);
    assert_eq!(view.correspondence().device(), device);
    assert_eq!(view.correspondence().placement(), plan.identity());
}

#[test]
fn corresponded_view_retirement_replays_both_authorities_and_returns_exact_inputs() {
    let plan = uart_placement_plan();
    let extent = uart_extent_with_lineage(0x71a8, 12, 259);
    let origin = extent.origin();
    let lineage = extent.lineage_root();
    let loan = extent.loan(0, 12).expect("shared UART loan");
    let profile = uart_resource_profile(&loan, &uart_reach());
    let admission = admit_placement(
        PlacementAdmissionId::from_normalized_identity(260).expect("placement admission"),
        loan,
        &plan,
        &profile,
    )
    .expect("borrowed placement admission");
    let provider = SchemaCorrespondenceProviderId::from_normalized_identity(261)
        .expect("correspondence provider");
    let device =
        StableDeviceInstanceId::from_normalized_identity(262).expect("stable device instance");
    let correspondence = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        provider,
        device,
        SchemaCorrespondenceSourceId::from_normalized_identity(263).expect("datasheet provenance"),
        &plan,
        profile.receipt(),
        None,
    )
    .expect("provider correspondence grant")
    .admit(&plan, &profile)
    .expect("schema correspondence admission");
    let mut view = bind_schema_correspondence_to_placement(admission, correspondence)
        .expect("correspondence placement binding")
        .establish_view()
        .expect("corresponded view establishment");

    let drifted_receipt =
        ResourceProfileReceiptId::from_normalized_identity(264).expect("drifted receipt");
    view.replace_view_profile_receipt_for_test(drifted_receipt);
    view.replace_correspondence_profile_receipt_for_test(drifted_receipt);
    let rejection = view
        .retire()
        .expect_err("coordinated copied receipt drift must reject retirement");
    assert!(
        rejection
            .diagnostic()
            .0
            .contains("admitted resource-profile receipt")
    );
    let (mut view, _) = rejection.into_parts();
    view.replace_view_profile_receipt_for_test(profile.receipt());
    view.replace_correspondence_profile_receipt_for_test(profile.receipt());

    view.replace_correspondence_placement_for_test(PlacementPlanId(plan.identity().0 ^ 1));
    let rejection = view
        .retire()
        .expect_err("physical correspondence drift must reject retirement");
    assert!(rejection.diagnostic().0.contains("exact placement"));
    let (mut view, _) = rejection.into_parts();
    view.replace_correspondence_placement_for_test(plan.identity());

    let (loan, correspondence) = view
        .retire()
        .expect("repaired view remains valid for retirement retry");
    assert_eq!(loan.origin(), origin);
    assert_eq!(loan.lineage_root(), lineage);
    assert_eq!(loan.base(), 0x71a8);
    assert_eq!(loan.length(), 12);
    assert_eq!(loan.polarity(), LoanPolarity::Shared);
    assert_eq!(correspondence.provider(), provider);
    assert_eq!(correspondence.device(), device);
    assert_eq!(correspondence.placement(), plan.identity());
    assert_eq!(correspondence.profile_receipt(), profile.receipt());
}

#[test]
fn corresponded_view_retains_and_replays_evidence_through_primitive_specialization() {
    let plan = uart_placement_plan();
    let extent = uart_extent_with_lineage(0x71b0, 12, 252);
    let loan = extent.loan(0, 12).expect("shared UART loan");
    let profile = uart_resource_profile(&loan, &uart_reach());
    let admission = admit_placement(
        PlacementAdmissionId::from_normalized_identity(253).expect("placement admission"),
        loan,
        &plan,
        &profile,
    )
    .expect("borrowed placement admission");
    let provider = SchemaCorrespondenceProviderId::from_normalized_identity(254)
        .expect("correspondence provider");
    let device = StableDeviceInstanceId::from_normalized_identity(255).expect("stable device");
    let correspondence = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        provider,
        device,
        SchemaCorrespondenceSourceId::from_normalized_identity(256).expect("datasheet provenance"),
        &plan,
        profile.receipt(),
        None,
    )
    .expect("provider correspondence grant")
    .admit(&plan, &profile)
    .expect("schema correspondence admission");
    let mut view = bind_schema_correspondence_to_placement(admission, correspondence)
        .expect("correspondence placement binding")
        .establish_view()
        .expect("corresponded view establishment");
    let status = field_key(plan.access(), "status");

    view.replace_correspondence_placement_for_test(PlacementPlanId(plan.identity().0 ^ 1));
    let rejection = view
        .project(status)
        .expect_err("projection must replay retained correspondence");
    assert!(rejection.0.contains("schema/device correspondence"));
    view.replace_correspondence_placement_for_test(plan.identity());

    let projection = view
        .project(status)
        .expect("repaired correspondence remains available for retry");
    assert_eq!(
        projection
            .correspondence()
            .expect("corresponded projection")
            .provider(),
        provider
    );
    let access = projection.read().expect("External status read");
    assert_eq!(
        access
            .correspondence()
            .expect("corresponded authorized access")
            .device(),
        device
    );
    let request = access.into_primitive_request();
    assert_eq!(
        request
            .correspondence()
            .expect("corresponded primitive request")
            .placement(),
        plan.identity()
    );
    let exact_request = primitive_request_snapshot(&request);
    let external = request
        .into_external_primitive_access()
        .expect("External specialization replays correspondence");
    assert_eq!(
        primitive_request_snapshot(external.primitive_request()),
        exact_request,
        "outward specialization must retain the exact sealed request"
    );
    assert_eq!(
        external
            .correspondence()
            .expect("correspondence reaches outward specialization")
            .device(),
        device
    );
    let alternate_correspondence = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
        SchemaCorrespondenceProviderId::from_normalized_identity(259)
            .expect("alternate correspondence provider"),
        StableDeviceInstanceId::from_normalized_identity(260).expect("alternate stable device"),
        SchemaCorrespondenceSourceId::from_normalized_identity(261)
            .expect("alternate datasheet provenance"),
        &plan,
        profile.receipt(),
        None,
    )
    .expect("alternate provider correspondence grant")
    .admit(&plan, &profile)
    .expect("alternate schema correspondence admission");
    let mut corresponded = external
        .into_corresponded_external_access()
        .expect("provider/device preflight requires retained correspondence");
    assert_eq!(corresponded.correspondence().provider(), provider);
    assert_eq!(
        primitive_request_snapshot(corresponded.external_access().primitive_request()),
        exact_request
    );
    let retained_correspondence =
        corresponded.replace_correspondence_for_test(&alternate_correspondence);
    let rejection = corresponded
        .validate_for_provider_lowering()
        .expect_err("a distinct correspondence carrier cannot replace retained authority");
    assert!(
        rejection
            .0
            .contains("different schema/device correspondence")
    );
    corresponded.replace_correspondence_for_test(retained_correspondence);
    corresponded
        .validate_for_provider_lowering()
        .expect("restoring the exact correspondence carrier permits retry");

    corresponded.replace_request_plan_for_test(PlacementPlanId(plan.identity().0 ^ 1));
    let rejection = corresponded
        .validate_for_provider_lowering()
        .expect_err("provider/device preflight must replay the retained placement");
    assert!(rejection.0.contains("copied plan"));
    assert_eq!(corresponded.correspondence().provider(), provider);
    assert_eq!(
        corresponded.external_access().primitive_request().plan(),
        PlacementPlanId(plan.identity().0 ^ 1),
        "borrowed request inspection reflects the still-retained drifted carrier"
    );
    corresponded.replace_request_plan_for_test(plan.identity());
    corresponded
        .validate_for_provider_lowering()
        .expect("repaired outward carrier remains available for retry");
    assert_eq!(
        primitive_request_snapshot(corresponded.external_access().primitive_request()),
        exact_request
    );
    let request = corresponded.into_external_access().into_primitive_request();
    assert_eq!(
        request
            .correspondence()
            .expect("retained evidence")
            .provider(),
        provider
    );

    let ordinary_extent = uart_extent_with_lineage(0x71c0, 12, 257);
    let ordinary_loan = ordinary_extent.loan(0, 12).expect("ordinary shared loan");
    let ordinary = place(
        admit_uart(258, ordinary_loan, &plan, &uart_reach()).expect("ordinary placement admission"),
    )
    .expect("ordinary view establishment");
    let ordinary_projection = ordinary.project(status).expect("ordinary projection");
    assert!(ordinary_projection.correspondence().is_none());
    let ordinary_request = ordinary_projection
        .read()
        .expect("ordinary External read")
        .into_primitive_request();
    let ordinary_snapshot = primitive_request_snapshot(&ordinary_request);
    let rejection = ordinary_request
        .into_external_primitive_access()
        .expect("ordinary External specialization remains valid")
        .into_corresponded_external_access()
        .expect_err("device/provider preflight must reject correspondence-free storage");
    assert!(rejection.diagnostic().0.contains("requires admitted"));
    let (ordinary_external, _) = rejection.into_parts();
    assert_eq!(
        primitive_request_snapshot(ordinary_external.primitive_request()),
        ordinary_snapshot,
        "rejection must return the exact already-specialized External request"
    );
    ordinary_external
        .validate_for_lowering()
        .expect("returned correspondence-free request remains valid for another consumer");
}
