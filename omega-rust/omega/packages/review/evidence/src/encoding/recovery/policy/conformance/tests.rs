use super::*;
use crate::record::{
    PackageReviewNominalIdentity, PackageReviewNominalOwner, PackageReviewTypeIdentity,
};
use psi_core::PackageKeyIdentity;

fn nominal(path: &str) -> PackageReviewNominalIdentity {
    PackageReviewNominalIdentity {
        owner: PackageReviewNominalOwner::Package(
            PackageKeyIdentity::from_digest([7; 32]).expect("nonzero owner"),
        ),
        path: path.to_owned(),
    }
}

fn value_type(identity: &str) -> PackageReviewTypeIdentity {
    PackageReviewTypeIdentity {
        canonical: identity.to_owned(),
    }
}

fn application() -> PackagePolicyClosedConformanceApplication {
    PackagePolicyClosedConformanceApplication {
        declaration: nominal("Chosen"),
        lifetime_arguments: vec![0, 1, 0],
        type_arguments: vec![value_type("type-first"), value_type("type-second")],
        const_arguments: vec![
            PackagePolicyConformanceConstArgument::Evaluated {
                parameter_carrier: value_type("parameter-carrier"),
                declared_carrier: value_type("declared-carrier"),
                canonical_value_encoding: "structured-value".into(),
            },
            PackagePolicyConformanceConstArgument::CallerBinder {
                parameter_carrier: value_type("binder-parameter-carrier"),
                binder: nominal("Caller::Count"),
                binder_carrier: value_type("binder-carrier"),
            },
        ],
        machine_arguments: vec![nominal("Callback::first"), nominal("Callback::second")],
        subject: Some(value_type("subject-type")),
        trait_identity: nominal("TargetTrait"),
        trait_lifetime_arguments: vec![1, 0],
        trait_arguments: vec![value_type("trait-first"), value_type("trait-second")],
        rows: vec![PackagePolicyConformanceRow {
            declaring_trait: nominal("InheritedTrait"),
            requirement: nominal("InheritedTrait::invoke"),
            realization_machine: nominal("Adapter"),
            realization_state: nominal("Adapter::invoke"),
        }],
    }
}

fn recover(bytes: &[u8]) -> Result<PackagePolicyClosedConformanceApplication, Error> {
    PackagePolicyClosedConformanceApplication::recover_canonical(
        bytes,
        PackagePolicyRecoveryLimits::default(),
    )
}

fn bytes(application: &PackagePolicyClosedConformanceApplication) -> Vec<u8> {
    application
        .canonical_bytes()
        .expect("encode inert application")
}

#[test]
fn complete_application_and_subjectless_application_round_trip() {
    let mut original = application();
    for subject in [original.subject.clone(), None] {
        original.subject = subject;
        let encoded = bytes(&original);
        let restored = recover(&encoded).expect("recover complete application");
        assert_eq!(restored, original);
        assert_eq!(bytes(&restored), encoded);
    }
}

#[test]
fn every_semantic_coordinate_changes_application_bytes() {
    let original = application();
    let encoded = bytes(&original);
    let mutations: &[fn(&mut PackagePolicyClosedConformanceApplication)] = &[
        |value| value.declaration = nominal("Other"),
        |value| value.declaration.owner = nominal_owner(8),
        |value| value.lifetime_arguments.swap(0, 1),
        |value| value.type_arguments.swap(0, 1),
        |value| value.const_arguments.swap(0, 1),
        |value| {
            if let PackagePolicyConformanceConstArgument::Evaluated {
                parameter_carrier, ..
            } = &mut value.const_arguments[0]
            {
                *parameter_carrier = value_type("changed");
            }
        },
        |value| {
            if let PackagePolicyConformanceConstArgument::Evaluated {
                declared_carrier, ..
            } = &mut value.const_arguments[0]
            {
                *declared_carrier = value_type("changed");
            }
        },
        |value| {
            if let PackagePolicyConformanceConstArgument::Evaluated {
                canonical_value_encoding,
                ..
            } = &mut value.const_arguments[0]
            {
                *canonical_value_encoding = "changed".into();
            }
        },
        |value| {
            if let PackagePolicyConformanceConstArgument::CallerBinder {
                parameter_carrier, ..
            } = &mut value.const_arguments[1]
            {
                *parameter_carrier = value_type("changed");
            }
        },
        |value| {
            if let PackagePolicyConformanceConstArgument::CallerBinder { binder, .. } =
                &mut value.const_arguments[1]
            {
                *binder = nominal("Other::Count");
            }
        },
        |value| {
            if let PackagePolicyConformanceConstArgument::CallerBinder { binder_carrier, .. } =
                &mut value.const_arguments[1]
            {
                *binder_carrier = value_type("changed");
            }
        },
        |value| value.machine_arguments.swap(0, 1),
        |value| value.subject = None,
        |value| value.trait_identity = nominal("OtherTrait"),
        |value| value.trait_lifetime_arguments.swap(0, 1),
        |value| value.trait_arguments.swap(0, 1),
        |value| value.rows[0].declaring_trait = nominal("OtherTrait"),
        |value| value.rows[0].requirement = nominal("OtherTrait::invoke"),
        |value| value.rows[0].realization_machine = nominal("OtherAdapter"),
        |value| value.rows[0].realization_state = nominal("OtherAdapter::invoke"),
    ];
    for mutation in mutations {
        let mut changed = original.clone();
        mutation(&mut changed);
        let changed_bytes = bytes(&changed);
        assert_ne!(changed_bytes, encoded);
        assert_eq!(
            recover(&changed_bytes).expect("recover changed policy"),
            changed
        );
    }
}

fn nominal_owner(byte: u8) -> PackageReviewNominalOwner {
    PackageReviewNominalOwner::Package(
        PackageKeyIdentity::from_digest([byte; 32]).expect("nonzero owner"),
    )
}

#[test]
fn all_truncations_and_trailing_input_reject() {
    let mut encoded = bytes(&application());
    for length in 0..encoded.len() {
        assert!(
            recover(&encoded[..length]).is_err(),
            "accepted truncated length {length}"
        );
    }
    encoded.push(0);
    assert_eq!(recover(&encoded), Err(Error::TrailingBytes));
}

#[test]
fn hostile_versions_owners_tags_utf8_and_counts_reject() {
    let encoded = bytes(&application());
    let header = CONFORMANCE_POLICY_MAGIC.len();
    let mut changed = encoded.clone();
    changed[header] = 2;
    assert_eq!(recover(&changed), Err(Error::UnsupportedVersion));
    let mut changed = encoded.clone();
    changed[header + 2] = 2;
    assert_eq!(recover(&changed), Err(Error::InvalidIdentity));
    let mut changed = encoded.clone();
    changed[header + 3..header + 35].fill(0);
    assert_eq!(recover(&changed), Err(Error::InvalidIdentity));
    let declaration_text = header + 2 + 1 + 32 + 8;
    let mut changed = encoded.clone();
    changed[declaration_text] = 255;
    assert_eq!(recover(&changed), Err(Error::InvalidUtf8));
    let lifetime_count = declaration_text + "Chosen".len();
    let mut changed = encoded.clone();
    changed[lifetime_count..lifetime_count + 8].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(recover(&changed).is_err());
    // The const discriminator precedes the first length-framed carrier.
    let carrier = encoded
        .windows("parameter-carrier".len())
        .position(|part| part == b"parameter-carrier")
        .expect("carrier marker");
    let mut changed = encoded.clone();
    changed[carrier - 9] = 2;
    assert_eq!(recover(&changed), Err(Error::InvalidTag));
}

#[test]
fn caller_limits_bound_bytes_elements_fields_and_owned_storage() {
    let encoded = bytes(&application());
    let limits = [
        PackagePolicyRecoveryLimits::new(
            encoded.len() - 1,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
        ),
        PackagePolicyRecoveryLimits::new(usize::MAX, 1, usize::MAX, usize::MAX, usize::MAX),
        PackagePolicyRecoveryLimits::new(usize::MAX, usize::MAX, 1, usize::MAX, usize::MAX),
        PackagePolicyRecoveryLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX),
    ];
    for limits in limits {
        assert!(
            PackagePolicyClosedConformanceApplication::recover_canonical(&encoded, limits).is_err()
        );
    }
    let generous = PackagePolicyRecoveryLimits::new(
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
    );
    assert_eq!(
        PackagePolicyClosedConformanceApplication::recover_canonical(&encoded, generous)
            .expect("hard-clamped defaults permit small application"),
        application()
    );
}

#[test]
fn writer_obeys_component_byte_and_aggregate_element_ceilings() {
    let mut oversized = application();
    oversized.type_arguments = vec![value_type("too-large"); 65_537];
    assert!(oversized.canonical_bytes().is_err());
    let mut oversized = application();
    oversized.subject = Some(value_type(&"x".repeat(4 * 1024 * 1024)));
    assert!(oversized.canonical_bytes().is_err());
}

#[test]
fn owned_storage_budget_includes_canonical_comparison_scratch() {
    let minimal = PackagePolicyClosedConformanceApplication {
        declaration: nominal("Chosen"),
        lifetime_arguments: Vec::new(),
        type_arguments: Vec::new(),
        const_arguments: Vec::new(),
        machine_arguments: Vec::new(),
        subject: None,
        trait_identity: nominal("TargetTrait"),
        trait_lifetime_arguments: Vec::new(),
        trait_arguments: Vec::new(),
        rows: Vec::new(),
    };
    let encoded = bytes(&minimal);
    let decoded_owned_bytes = "Chosen".len() + "TargetTrait".len();
    let limits = |maximum_owned_bytes| {
        PackagePolicyRecoveryLimits::new(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            maximum_owned_bytes,
            usize::MAX,
        )
    };
    assert_eq!(
        PackagePolicyClosedConformanceApplication::recover_canonical(
            &encoded,
            limits(decoded_owned_bytes + encoded.len() - 1),
        ),
        Err(Error::AllocationLimitExceeded),
    );
    assert_eq!(
        PackagePolicyClosedConformanceApplication::recover_canonical(
            &encoded,
            limits(decoded_owned_bytes + encoded.len()),
        )
        .expect("exact accounted allocation budget"),
        minimal,
    );
}
