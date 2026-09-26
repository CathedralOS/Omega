use super::{Error, PackagePolicyRecoveryLimits, REPRESENTATION_POLICY_MAGIC, fixtures};
use crate::record::*;

pub(super) fn recover(bytes: &[u8]) -> Result<PackagePolicyRepresentation, Error> {
    PackagePolicyRepresentation::recover_canonical(bytes, PackagePolicyRecoveryLimits::default())
}

#[test]
fn complete_representation_round_trips_all_families_without_nested_receipts() {
    for policy in [fixtures::empty(), fixtures::complete()] {
        let bytes = policy.canonical_bytes().unwrap();
        assert_eq!(recover(&bytes).unwrap(), policy);
        crate::encoding::encode::text_test_support::component(
            crate::encoding::encode::text_test_support::Component::Representation(&policy),
        );
        assert_eq!(recover(&bytes).unwrap().canonical_bytes().unwrap(), bytes);
        for magic in [
            crate::encoding::CALLING_POLICY_MAGIC,
            crate::encoding::CONFORMANCE_POLICY_MAGIC,
            crate::encoding::PHYSICAL_CALLING_POLICY_MAGIC,
        ] {
            assert!(!bytes.windows(magic.len()).any(|window| window == magic));
        }
    }
    let policy = fixtures::complete();
    assert_eq!(policy.declarations.len(), 2);
    assert_eq!(policy.producer_availability.len(), 1);
    assert_eq!(policy.selected_availability.len(), 2);
    assert_eq!(policy.demands.len(), 1);
    assert_ne!(
        policy.selected_availability[0].copy_disposition,
        policy.selected_availability[1].copy_disposition
    );
    assert_eq!(policy.demands[0].calling.callbacks.layouts.len(), 2);
}

#[test]
fn every_truncation_unknown_envelope_and_trailing_bytes_reject() {
    let bytes = fixtures::complete().canonical_bytes().unwrap();
    for end in 0..bytes.len() {
        assert!(recover(&bytes[..end]).is_err(), "truncated prefix {end}");
    }
    let mut changed = bytes.clone();
    changed[0] ^= 1;
    assert_eq!(recover(&changed), Err(Error::UnsupportedVersion));
    let mut changed = bytes.clone();
    changed[REPRESENTATION_POLICY_MAGIC.len()..][..2].copy_from_slice(&u16::MAX.to_le_bytes());
    assert_eq!(recover(&changed), Err(Error::UnsupportedVersion));
    let mut changed = bytes;
    changed.push(0);
    assert_eq!(recover(&changed), Err(Error::TrailingBytes));
}

#[test]
fn malformed_identity_target_counts_and_collection_order_reject() {
    let mut policy = fixtures::empty();
    policy.declarations = vec![fixtures::nominal("A"), fixtures::nominal("B")];
    let bytes = policy.canonical_bytes().unwrap();
    let package_offset = REPRESENTATION_POLICY_MAGIC.len() + 2;
    let target_offset = package_offset + 32;
    let declarations_count = target_offset + 7;
    for (offset, replacement, expected) in [
        (target_offset, 255, Error::InvalidTag),
        (target_offset + 1, 255, Error::InvalidTag),
        (target_offset + 2, 255, Error::InvalidTag),
        (declarations_count + 8, 255, Error::InvalidIdentity),
    ] {
        let mut changed = bytes.clone();
        changed[offset] = replacement;
        assert_eq!(recover(&changed), Err(expected));
    }
    let mut changed = bytes.clone();
    changed[package_offset..target_offset].fill(0);
    assert_eq!(recover(&changed), Err(Error::InvalidIdentity));
    let mut changed = bytes.clone();
    changed[declarations_count..][..8].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(recover(&changed).is_err());
    let first = declarations_count + 8;
    let width = 1 + 32 + 8 + 1;
    let mut changed = bytes.clone();
    changed[first..first + width].copy_from_slice(&bytes[first + width..first + 2 * width]);
    assert_eq!(
        recover(&changed),
        Err(Error::InvalidValue),
        "duplicate declaration"
    );
    let mut changed = bytes.clone();
    changed[first..first + width].copy_from_slice(&bytes[first + width..first + 2 * width]);
    changed[first + width..first + 2 * width].copy_from_slice(&bytes[first..first + width]);
    assert_eq!(
        recover(&changed),
        Err(Error::InvalidValue),
        "noncanonical declaration order"
    );
}

#[test]
fn writer_rejects_detached_selection_demand_owner_and_target() {
    let original = fixtures::complete();
    let mut cases = Vec::new();
    let mut changed = original.clone();
    changed.selected_availability.clear();
    cases.push(changed);
    let mut changed = original.clone();
    changed.demands[0].opaque = fixtures::nominal("Missing");
    cases.push(changed);
    let mut changed = original.clone();
    changed.demands[0].calling.opaque_uses[0].copy_disposition =
        PackageReviewOpaqueRepresentationCopyDisposition::CheckedSemanticCopy;
    cases.push(changed);
    let mut changed = original.clone();
    changed.selected_availability[0].selection_owner = PackageReviewNominalOwner::Package(
        semantic_vocabulary::PackageKeyIdentity::from_digest([9; 32]).unwrap(),
    );
    cases.push(changed);
    let mut changed = original.clone();
    changed.demands[0].calling.target.profile =
        PackageReviewRepresentationTargetProfile::WindowsX64;
    cases.push(changed);
    let mut changed = original.clone();
    changed.selected_availability.reverse();
    cases.push(changed);
    let mut changed = original.clone();
    changed
        .producer_availability
        .push(changed.producer_availability[0].clone());
    cases.push(changed);
    let mut changed = original.clone();
    changed.demands.push(changed.demands[0].clone());
    cases.push(changed);
    for (index, policy) in cases.iter().enumerate() {
        assert!(
            policy.canonical_bytes().is_err(),
            "detached or noncanonical case {index}"
        );
    }
}

#[test]
fn selected_origin_lifecycle_and_copy_tags_are_closed() {
    let mut policy = fixtures::complete();
    policy.demands.clear();
    let bytes = policy.canonical_bytes().unwrap();
    // The final selection ends in its three disposition tags, followed only
    // by the empty demand count. No implementation receipts occupy this tail.
    let dispositions = bytes.len() - 8 - 3;
    for ordinal in 0..3 {
        let mut changed = bytes.clone();
        changed[dispositions + ordinal] = 255;
        assert_eq!(recover(&changed), Err(Error::InvalidTag));
    }
    let mut changed = bytes.clone();
    changed[dispositions + 2] = 0;
    let decoded = recover(&changed).unwrap();
    assert_eq!(
        decoded.selected_availability[1].copy_disposition,
        PackageReviewOpaqueRepresentationCopyDisposition::PlacementOnly
    );
    assert_ne!(
        decoded, policy,
        "unused copy disposition remains policy meaning"
    );
}
