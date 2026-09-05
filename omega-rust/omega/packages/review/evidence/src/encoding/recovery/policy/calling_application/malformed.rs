use super::*;

#[test]
fn target_and_shape_graph_reject_unknown_closed_tags() {
    let target = [1, 1, 0, 8, 0, 8, 0];
    for index in 0..3 {
        let mut bytes = target;
        bytes[index] = 255;
        let mut reader = Reader::new(&bytes, PackagePolicyRecoveryLimits::default()).unwrap();
        assert_eq!(shapes::target(&mut reader), Err(Error::InvalidTag));
    }
    let mut bytes = 1u64.to_le_bytes().to_vec();
    bytes.extend_from_slice(&[255, 8, 0, 8, 0]);
    let mut reader = Reader::new(&bytes, PackagePolicyRecoveryLimits::default()).unwrap();
    assert_eq!(shapes::graph(&mut reader), Err(Error::InvalidTag));
}

#[test]
fn callback_demand_and_materialization_destinations_reject_unknown_tags() {
    let mut demand = 0u64.to_le_bytes().to_vec(); // no binders
    demand.extend_from_slice(&1u64.to_le_bytes());
    demand.extend_from_slice(&[0; 46]); // minimum complete destination + requirement
    demand[16] = 255;
    let mut reader = Reader::new(&demand, PackagePolicyRecoveryLimits::default()).unwrap();
    assert_eq!(callbacks::decode(&mut reader), Err(Error::InvalidTag));

    let mut materialization = vec![0; 16]; // no binders or demands
    materialization.extend_from_slice(&1u64.to_le_bytes());
    materialization.extend_from_slice(&[0; 9]);
    materialization[28] = 255; // destination discriminator after binder index
    let mut reader = Reader::new(&materialization, PackagePolicyRecoveryLimits::default()).unwrap();
    assert_eq!(callbacks::decode(&mut reader), Err(Error::InvalidTag));
}

#[test]
fn declared_nested_counts_reject_before_unbounded_allocation() {
    for count in [u64::MAX, 65_537] {
        let bytes = count.to_le_bytes();
        let mut reader = Reader::new(&bytes, PackagePolicyRecoveryLimits::default()).unwrap();
        assert!(callbacks::decode(&mut reader).is_err());
        let mut reader = Reader::new(&bytes, PackagePolicyRecoveryLimits::default()).unwrap();
        assert!(shapes::graph(&mut reader).is_err());
    }
    let bytes = 65_536u64.to_le_bytes();
    let mut reader = Reader::new(&bytes, PackagePolicyRecoveryLimits::default()).unwrap();
    assert_eq!(callbacks::decode(&mut reader), Err(Error::UnexpectedEnd));
}

#[test]
fn opaque_selection_lifetimes_cannot_borrow_the_callback_telescope() {
    let mut policy = tests::complete_fixture();
    policy.boundary_lifetime_parameter_count = 1;
    for layout in &mut policy.callbacks.layouts {
        layout.terminal_slot.lifetime_arguments = vec![0];
        layout.terminal_slot.trait_lifetime_arguments = vec![0];
    }
    let valid = policy.canonical_bytes().unwrap();
    assert_eq!(
        PackagePolicyCallingPlan::recover_canonical(&valid, PackagePolicyRecoveryLimits::default())
            .unwrap(),
        policy
    );

    for trait_arguments in [false, true] {
        let mut changed = policy.clone();
        let application = &mut changed.opaque_uses[0].application;
        if trait_arguments {
            application.trait_lifetime_arguments = vec![0];
        } else {
            application.lifetime_arguments = vec![0];
        }
        assert!(changed.canonical_bytes().is_err());
        // Encode malformed inner fields deliberately, bypassing the public
        // writer's structural gate to exercise independent offline recovery.
        let mut encoder =
            crate::encoding::encode::encoder::Encoder::policy_bounded(4 * 1024 * 1024);
        encoder.fixed_bytes(CALLING_POLICY_MAGIC);
        encoder.u16(PACKAGE_CALLING_POLICY_VERSION);
        crate::encoding::encode::calling::encode_application(&mut encoder, &changed).unwrap();
        let bytes = encoder.finish().unwrap();
        assert_eq!(
            PackagePolicyCallingPlan::recover_canonical(
                &bytes,
                PackagePolicyRecoveryLimits::default()
            ),
            Err(Error::InvalidValue)
        );
    }
}
