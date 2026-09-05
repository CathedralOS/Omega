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
