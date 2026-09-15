use super::device_requirement;
use crate::{
    DeviceOperation, DeviceOperationProviderPlanId, ProviderAssertedDeviceOperationClaim,
    structurally_close_device_operation_requirements,
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
        .map(|requirement| {
            ProviderAssertedDeviceOperationClaim::from_provider_assertion(
                DeviceOperationProviderPlanId::from_normalized_identity(822)
                    .expect("device provider plan"),
                requirement,
            )
        })
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
    }));
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
        let evidence = ProviderAssertedDeviceOperationClaim::from_provider_assertion(
            DeviceOperationProviderPlanId::from_normalized_identity(827)
                .expect("device provider plan"),
            &drifted,
        );
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

        let repaired = ProviderAssertedDeviceOperationClaim::from_provider_assertion(
            DeviceOperationProviderPlanId::from_normalized_identity(827)
                .expect("device provider plan"),
            &returned_requirements[0],
        );
        let _closed =
            structurally_close_device_operation_requirements(returned_requirements, vec![repaired])
                .expect("returned demand supports corrected retry");
    }
}

#[test]
fn device_operation_structural_closure_requires_exact_one_to_one_rows() {
    let first = device_requirement(850, DeviceOperation::CacheMaintenance, 0, 828, 829);
    let second = device_requirement(851, DeviceOperation::CacheMaintenance, 0, 828, 829);
    let plan =
        DeviceOperationProviderPlanId::from_normalized_identity(830).expect("device provider plan");

    let missing = structurally_close_device_operation_requirements(
        vec![first.clone(), second.clone()],
        vec![ProviderAssertedDeviceOperationClaim::from_provider_assertion(plan, &first)],
    )
    .expect_err("each equal-looking occurrence needs its own evidence");
    assert!(missing.diagnostic().0.contains("no provider claim"));
    let (returned_requirements, returned_evidence) = missing.into_parts();
    assert_eq!(returned_requirements, vec![first.clone(), second.clone()]);
    assert_eq!(returned_evidence.len(), 1);
    assert_eq!(returned_evidence[0].requirement(), &first);

    let duplicate_requirement = structurally_close_device_operation_requirements(
        vec![first.clone(), first.clone()],
        vec![ProviderAssertedDeviceOperationClaim::from_provider_assertion(plan, &first)],
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
            ProviderAssertedDeviceOperationClaim::from_provider_assertion(plan, &first),
            ProviderAssertedDeviceOperationClaim::from_provider_assertion(plan, &first),
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
            ProviderAssertedDeviceOperationClaim::from_provider_assertion(plan, &first),
            ProviderAssertedDeviceOperationClaim::from_provider_assertion(plan, &second),
        ],
    )
    .expect_err("evidence for an un-emitted occurrence rejects");
    assert!(extra.diagnostic().0.contains("un-emitted"));

    let _empty = structurally_close_device_operation_requirements(Vec::new(), Vec::new())
        .expect("an empty emitted set is exactly closed by empty evidence");
}
