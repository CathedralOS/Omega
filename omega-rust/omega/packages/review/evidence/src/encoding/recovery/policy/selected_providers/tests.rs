use super::{Error, PackagePolicyRecoveryLimits, SELECTED_PROVIDER_POLICY_MAGIC, fixtures};
use crate::record::*;

pub(super) fn recover(bytes: &[u8]) -> Result<PackagePolicySelectedProviders, Error> {
    PackagePolicySelectedProviders::recover_canonical(bytes, PackagePolicyRecoveryLimits::default())
}

#[test]
fn complete_provider_policy_round_trips_without_nested_receipt_envelopes() {
    for policy in [fixtures::empty(), fixtures::complete()] {
        let bytes = policy.canonical_bytes().unwrap();
        assert_eq!(recover(&bytes).unwrap(), policy);
        crate::encoding::encode::text_test_support::component(
            crate::encoding::encode::text_test_support::Component::SelectedProviders(&policy),
        );
        assert_eq!(recover(&bytes).unwrap().canonical_bytes().unwrap(), bytes);
        for magic in [
            crate::encoding::CALLING_POLICY_MAGIC,
            crate::encoding::CONFORMANCE_POLICY_MAGIC,
            crate::encoding::EXTERNAL_SUPPLY_POLICY_MAGIC,
        ] {
            assert!(!bytes.windows(magic.len()).any(|window| window == magic));
        }
    }
}

#[test]
fn all_provider_prefixes_versions_and_trailing_bytes_reject() {
    let bytes = fixtures::complete().canonical_bytes().unwrap();
    for end in 0..bytes.len() {
        assert!(recover(&bytes[..end]).is_err(), "prefix {end}");
    }
    let mut changed = bytes.clone();
    changed[0] ^= 1;
    assert_eq!(recover(&changed), Err(Error::UnsupportedVersion));
    let mut changed = bytes.clone();
    changed[SELECTED_PROVIDER_POLICY_MAGIC.len()] = 255;
    assert_eq!(recover(&changed), Err(Error::UnsupportedVersion));
    let mut changed = bytes;
    changed.push(0);
    assert_eq!(recover(&changed), Err(Error::TrailingBytes));
}

#[test]
fn provider_family_plan_links_and_selector_order_reject_drift() {
    let original = fixtures::complete();
    let mut cases = Vec::new();
    let mut changed = original.clone();
    changed.families[0].coordinates[0].plan_index = 1;
    cases.push(changed);
    let mut changed = original.clone();
    changed.families[0].coordinates[0].plan_index = 99;
    cases.push(changed);
    let mut changed = original.clone();
    changed.plans.reverse();
    cases.push(changed);
    let mut changed = original.clone();
    changed.plans[0].grants.reverse();
    cases.push(changed);
    let mut changed = original.clone();
    changed.plans[0]
        .grants
        .push(PackageReviewProviderGrantSelectorKind::ProviderSlot);
    cases.push(changed);
    let mut changed = original.clone();
    changed.plans[0].rows.clear();
    cases.push(changed);
    let mut changed = original.clone();
    changed.plans[1].methods[0]
        .calling
        .as_mut()
        .unwrap()
        .requirement = fixtures::nominal("Detached");
    cases.push(changed);
    let mut changed = original.clone();
    changed.plans[0].rows[0]
        .installation_reach
        .as_mut()
        .unwrap()
        .resolved
        .push(fixtures::nominal("Outside"));
    cases.push(changed);
    let mut changed = original.clone();
    changed.plans[0].rows[0].requirement_lifetime_partition = vec![1];
    cases.push(changed);
    for (index, policy) in cases.iter().enumerate() {
        assert!(policy.canonical_bytes().is_err(), "drift case {index}");
    }
    let bytes = original.canonical_bytes().unwrap();
    let mut changed = bytes;
    let length = changed.len();
    changed[length - 4..].copy_from_slice(&99u32.to_le_bytes());
    assert_eq!(
        recover(&changed),
        Err(Error::InvalidValue),
        "decoded family index must rejoin its plan"
    );
}

#[test]
fn service_policy_changes_and_missing_intrinsic_classification_remain_visible() {
    let original = fixtures::complete();
    let bytes = original.canonical_bytes().unwrap();
    let mut cases = Vec::new();
    let mut changed = original.clone();
    changed.plans[0].methods[0].may_block = true;
    cases.push(changed);
    let mut changed = original.clone();
    changed.plans[0].methods[0].may_suspend = false;
    cases.push(changed);
    let mut changed = original.clone();
    changed.plans[0].methods[0].entry_claims[0].domain = "OtherAuthority".into();
    cases.push(changed);
    let mut changed = original.clone();
    changed.plans[0].methods[0].entry_claims[0]
        .effective_carry
        .cpu = psi_language_semantics::CarryCpu::Origin;
    cases.push(changed);
    let mut changed = original.clone();
    changed.plans[0].methods[0].termination_premises[0].profile = "OtherProgress".into();
    changed.plans[0].methods[0].authority.progress_premises[0].profile =
        fixtures::nominal("OtherProgress");
    cases.push(changed);
    let mut changed = original.clone();
    changed.plans[0].grants.clear();
    cases.push(changed);
    let mut changed = original.clone();
    changed.families[0].authority = PackageReviewProviderSelectionAuthority::TargetDefault;
    cases.push(changed);
    for policy in cases {
        let changed = policy.canonical_bytes().unwrap();
        assert_ne!(changed, bytes);
        assert_eq!(recover(&changed).unwrap(), policy);
    }
}
