use super::{device_claim, device_coordinates, device_requirement, device_scope_occurrence};
use crate::{
    DeviceOperation, DeviceOperationProviderPlanId, DeviceOrderingScopeId,
    ProviderAssertedDeviceOperationClaim, structurally_close_device_operation_requirements,
};

#[test]
fn device_operation_requirements_close_all_five_non_fence_families_exactly() {
    let operations = [
        DeviceOperation::DmaPublication,
        DeviceOperation::DeviceAcquisition,
        DeviceOperation::CacheMaintenance,
        DeviceOperation::MmioNotification,
        DeviceOperation::PostedWriteCompletion,
    ];
    let requirements = operations
        .into_iter()
        .enumerate()
        .map(|(index, operation)| device_requirement(830 + index as u64, operation, 0, 820, 821))
        .collect::<Vec<_>>();
    let evidence = requirements
        .iter()
        .rev()
        .map(|requirement| device_claim(822, requirement, 900))
        .collect();

    let closed = structurally_close_device_operation_requirements(requirements, evidence)
        .expect("every emitted device operation has an exact provider assertion");

    closed
        .validate_structure()
        .expect("sealed device-operation closure replays");
    assert_eq!(
        closed
            .rows()
            .iter()
            .map(|row| row.requirement().operation())
            .collect::<Vec<_>>(),
        operations
    );
    assert!(closed.rows().iter().all(|row| {
        row.provider_plan().normalized_identity() == 822
            && row
                .requirement()
                .correspondence()
                .device()
                .normalized_identity()
                == 818
            && row.scope_occurrence().scope_capability()
                == DeviceOrderingScopeId::from_normalized_identity(821).unwrap()
    }));
}

#[test]
fn device_operation_coordinates_carry_the_role_and_its_places() {
    // The coordinate variant is the role: a mismatched role/coordinate pair
    // is unrepresentable, and each role names the spec's data, descriptor,
    // doorbell, request, completion, or maintained places instead of one
    // uniform range.
    for operation in [
        DeviceOperation::DmaPublication,
        DeviceOperation::DeviceAcquisition,
        DeviceOperation::CacheMaintenance,
        DeviceOperation::MmioNotification,
        DeviceOperation::PostedWriteCompletion,
    ] {
        let coordinates = device_coordinates(operation, 0);
        assert_eq!(coordinates.operation(), operation);
    }

    let notification = device_coordinates(DeviceOperation::MmioNotification, 0);
    let other_place = device_coordinates(DeviceOperation::MmioNotification, 0x40);
    let other_role = device_coordinates(DeviceOperation::PostedWriteCompletion, 0);
    assert_ne!(notification, other_place);
    assert_ne!(notification, other_role);
}

#[test]
fn device_operation_claim_binds_the_issued_scope_occurrence() {
    let requirement = device_requirement(860, DeviceOperation::MmioNotification, 0, 831, 832);
    let plan = DeviceOperationProviderPlanId::from_normalized_identity(833).expect("provider plan");

    let foreign_scope = device_scope_occurrence(834, 901);
    let rejection = ProviderAssertedDeviceOperationClaim::from_provider_assertion(
        plan,
        &requirement,
        foreign_scope,
    )
    .expect_err("an occurrence issued for a different scope cannot cover this demand");
    assert!(rejection.0.contains("does not cover the demanded scope"));

    let issued = device_scope_occurrence(832, 902);
    let claim =
        ProviderAssertedDeviceOperationClaim::from_provider_assertion(plan, &requirement, issued)
            .expect("provider-issued occurrence of the demanded scope covers it");
    let closed = structurally_close_device_operation_requirements(vec![requirement], vec![claim])
        .expect("claim bound to its issued scope occurrence closes");
    let row = &closed.rows()[0];
    // The occurrence is opaque: a consumer may carry it but cannot construct,
    // inspect, or compare its identity — only its scope capability is visible.
    assert_eq!(
        row.scope_occurrence().scope_capability(),
        DeviceOrderingScopeId::from_normalized_identity(832).unwrap()
    );
}

#[test]
fn device_operation_structural_closure_rejects_drift_and_returns_retry_custody() {
    let exact = device_requirement(840, DeviceOperation::DmaPublication, 0, 823, 824);
    for drifted in [
        device_requirement(840, DeviceOperation::DeviceAcquisition, 0, 823, 824),
        device_requirement(840, DeviceOperation::DmaPublication, 0x80, 823, 824),
        device_requirement(840, DeviceOperation::DmaPublication, 0, 825, 824),
        device_requirement(840, DeviceOperation::DmaPublication, 0, 823, 826),
    ] {
        let evidence = device_claim(827, &drifted, 903);
        let error =
            structurally_close_device_operation_requirements(vec![exact.clone()], vec![evidence])
                .expect_err("compact identity cannot cover structural drift");
        assert!(error.diagnostic().0.contains("structurally drifted"));
        let (returned_requirements, returned_evidence) = error.into_parts();
        assert_eq!(returned_requirements, vec![exact.clone()]);
        assert_eq!(returned_evidence.len(), 1);
        assert_eq!(returned_evidence[0].requirement(), &drifted);
        assert_eq!(
            returned_evidence[0].provider_plan().normalized_identity(),
            827
        );

        let repaired = device_claim(827, &returned_requirements[0], 904);
        let _closed =
            structurally_close_device_operation_requirements(returned_requirements, vec![repaired])
                .expect("returned demand supports corrected retry");
    }
}

#[test]
fn device_operation_structural_closure_requires_exact_one_to_one_rows() {
    let first = device_requirement(850, DeviceOperation::CacheMaintenance, 0, 828, 829);
    let second = device_requirement(851, DeviceOperation::CacheMaintenance, 0, 828, 829);

    let missing = structurally_close_device_operation_requirements(
        vec![first.clone(), second.clone()],
        vec![device_claim(830, &first, 905)],
    )
    .expect_err("each equal-looking occurrence needs its own evidence");
    assert!(missing.diagnostic().0.contains("no provider claim"));
    let (returned_requirements, returned_evidence) = missing.into_parts();
    assert_eq!(returned_requirements, vec![first.clone(), second.clone()]);
    assert_eq!(returned_evidence.len(), 1);
    assert_eq!(returned_evidence[0].requirement(), &first);

    let duplicate_requirement = structurally_close_device_operation_requirements(
        vec![first.clone(), first.clone()],
        vec![device_claim(830, &first, 906)],
    )
    .expect_err("duplicate emitted identities reject");
    assert!(
        duplicate_requirement
            .diagnostic()
            .0
            .contains("emitted more than once")
    );

    let duplicate_evidence = structurally_close_device_operation_requirements(
        vec![first.clone()],
        vec![
            device_claim(830, &first, 907),
            device_claim(830, &first, 908),
        ],
    )
    .expect_err("duplicate provider assertions reject");
    assert!(
        duplicate_evidence
            .diagnostic()
            .0
            .contains("duplicate provider claims")
    );

    let extra = structurally_close_device_operation_requirements(
        vec![first.clone()],
        vec![
            device_claim(830, &first, 909),
            device_claim(830, &second, 910),
        ],
    )
    .expect_err("evidence for an un-emitted occurrence rejects");
    assert!(extra.diagnostic().0.contains("un-emitted"));

    let _empty = structurally_close_device_operation_requirements(Vec::new(), Vec::new())
        .expect("an empty emitted set is exactly closed by empty evidence");
}
