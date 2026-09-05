use super::*;
use crate::record::PackageReviewNominalOwner;

pub(in crate::encoding::recovery::policy) fn fixture() -> PackagePolicyTerminalPermissions {
    let mut method = super::super::selected_providers::service_method_fixture();
    method.parameter_type_identities = method
        .signature
        .parameters
        .iter()
        .map(|parameter| parameter.type_identity.canonical.clone())
        .collect();
    method.result_type_identity = method
        .signature
        .result
        .as_ref()
        .map(|result| result.canonical.clone());
    let service = method.calling.as_ref().unwrap().boundary_trait.clone();
    let PackageReviewNominalOwner::Package(package) = service.owner else {
        panic!("owned fixture")
    };
    let mut sibling = method.clone();
    sibling.name = "unpermitted".into();
    sibling.requirement.path = "Boundary::unpermitted".into();
    sibling.calling = None;
    PackagePolicyTerminalPermissions {
        package,
        target: target::TargetProfile::LinuxX64,
        services: vec![PackagePolicyTerminalService {
            service,
            static_parameters: Vec::new(),
            lifetime_parameter_count: 0,
            permissions: vec![PackagePolicyTerminalPermission {
                requirement: method.requirement.clone(),
                permitted: TerminalAuthorityDisposition::from_classes(TerminalAuthorityClass::ALL),
            }],
            methods: vec![sibling, method],
        }],
    }
}

pub(super) fn recover(bytes: &[u8]) -> Result<PackagePolicyTerminalPermissions, Error> {
    PackagePolicyTerminalPermissions::recover_canonical(
        bytes,
        PackagePolicyRecoveryLimits::default(),
    )
}

#[test]
fn complete_schema_and_nested_calling_survive_without_nested_envelopes() {
    let policy = fixture();
    let bytes = policy.canonical_bytes().unwrap();
    assert_eq!(recover(&bytes).unwrap(), policy);
    crate::encoding::encode::text_test_support::component(
        crate::encoding::encode::text_test_support::Component::TerminalPermissions(&policy),
    );
    assert!(
        !bytes
            .windows(crate::encoding::CALLING_POLICY_MAGIC.len())
            .any(|window| window == crate::encoding::CALLING_POLICY_MAGIC)
    );
    assert_eq!(policy.services[0].methods[0].name, "unpermitted");
    assert!(policy.services[0].methods[1].calling.is_some());
    for target in target::TargetProfile::ALL {
        let mut empty = policy.clone();
        empty.services.clear();
        empty.target = target;
        assert_eq!(recover(&empty.canonical_bytes().unwrap()).unwrap(), empty);
    }
}

#[test]
fn unpermitted_schema_and_declaration_order_changes_remain_observable() {
    let original = fixture();
    let baseline = original.canonical_bytes().unwrap();
    let mut cases = Vec::new();
    let mut changed = original.clone();
    changed.services[0].methods[0].may_block = true;
    cases.push(changed);
    let mut changed = original.clone();
    changed.services[0].methods[0].signature.parameters[0]
        .type_identity
        .canonical = "Other".into();
    changed.services[0].methods[0].parameter_type_identities[0] = "Other".into();
    cases.push(changed);
    let mut changed = original.clone();
    changed.services[0].methods.reverse();
    cases.push(changed);
    for changed in cases {
        assert_eq!(
            changed.services[0].permissions,
            original.services[0].permissions
        );
        let bytes = changed.canonical_bytes().unwrap();
        assert_ne!(bytes, baseline);
        assert_eq!(recover(&bytes).unwrap(), changed);
        crate::encoding::encode::text_test_support::component(
            crate::encoding::encode::text_test_support::Component::TerminalPermissions(&changed),
        );
    }
}

#[test]
fn every_terminal_class_and_explicit_empty_differ_from_absence() {
    let original = fixture();
    for classes in
        std::iter::once(Vec::new()).chain(TerminalAuthorityClass::ALL.map(|class| vec![class]))
    {
        let mut policy = original.clone();
        policy.services[0].permissions[0].permitted =
            TerminalAuthorityDisposition::from_classes(classes.clone());
        let bytes = policy.canonical_bytes().unwrap();
        let recovered = recover(&bytes).unwrap();
        assert_eq!(
            recovered.services[0].permissions[0].permitted.classes(),
            classes
        );
        assert_eq!(recovered, policy);
        crate::encoding::encode::text_test_support::component(
            crate::encoding::encode::text_test_support::Component::TerminalPermissions(&policy),
        );
    }
    let mut explicit = original.clone();
    explicit.services[0].permissions[0].permitted = TerminalAuthorityDisposition::from_classes([]);
    let mut absent = explicit.clone();
    absent.services.clear();
    assert_ne!(
        explicit.canonical_bytes().unwrap(),
        absent.canonical_bytes().unwrap()
    );
    explicit.services[0].permissions.clear();
    assert!(
        explicit.canonical_bytes().is_err(),
        "empty service rows are not absence"
    );
}

#[test]
fn every_truncation_unknown_version_and_trailing_bytes_reject() {
    let bytes = fixture().canonical_bytes().unwrap();
    for end in 0..bytes.len() {
        assert!(recover(&bytes[..end]).is_err(), "prefix {end}");
    }
    let mut changed = bytes.clone();
    changed[TERMINAL_PERMISSION_POLICY_MAGIC.len()] = 255;
    assert_eq!(recover(&changed), Err(Error::UnsupportedVersion));
    let mut changed = bytes;
    changed.push(0);
    assert_eq!(recover(&changed), Err(Error::TrailingBytes));
}

#[test]
fn unknown_repeated_and_unsorted_terminal_classes_reject_before_normalization() {
    let bytes = fixture().canonical_bytes().unwrap();
    let start = bytes.len() - TerminalAuthorityClass::ALL.len();
    for (offset, tag, error) in [
        (0, 255, Error::InvalidTag),
        (1, 0, Error::NonCanonicalEncoding),
        (0, 2, Error::NonCanonicalEncoding),
    ] {
        let mut changed = bytes.clone();
        changed[start + offset] = tag;
        assert_eq!(recover(&changed), Err(error));
    }
    let mut changed = bytes;
    changed[start - 8..start].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(recover(&changed), Err(Error::ElementLimitExceeded));
}

#[test]
fn permission_membership_identity_target_and_canonical_sets_reject_drift() {
    let original = fixture();
    let mut cases = Vec::new();
    let mut changed = original.clone();
    changed.services[0].permissions[0].requirement.path = "Missing".into();
    cases.push(changed);
    let mut changed = original.clone();
    changed.services[0].service.path = "Other".into();
    cases.push(changed);
    let mut changed = original.clone();
    changed.target = target::TargetProfile::WindowsX64;
    cases.push(changed);
    let mut changed = original.clone();
    changed.services[0].methods[1].requirement_owner.path = "Other".into();
    cases.push(changed);
    let mut changed = original.clone();
    changed.services[0].methods[0] = changed.services[0].methods[1].clone();
    cases.push(changed);
    let mut changed = original.clone();
    changed.services.push(changed.services[0].clone());
    cases.push(changed);
    let mut changed = original.clone();
    let repeated = changed.services[0].permissions[0].clone();
    changed.services[0].permissions.push(repeated);
    cases.push(changed);
    for policy in cases {
        assert!(policy.canonical_bytes().is_err());
    }

    let mut two = original;
    let mut permission = two.services[0].permissions[0].clone();
    permission.requirement = two.services[0].methods[0].requirement.clone();
    two.services[0].permissions.push(permission);
    assert!(two.canonical_bytes().is_ok());
    two.services[0].permissions.reverse();
    assert!(two.canonical_bytes().is_err());

    let mut services = fixture();
    let mut second = services.services[0].clone();
    second.service.path = "OtherBoundary".into();
    for method in &mut second.methods {
        method.calling = None;
    }
    services.services.push(second);
    assert!(services.canonical_bytes().is_ok());
    services.services.reverse();
    assert!(services.canonical_bytes().is_err());

    let mut bytes = fixture().canonical_bytes().unwrap();
    let requirement = b"Boundary::call";
    let position = bytes
        .windows(requirement.len())
        .rposition(|window| window == requirement)
        .unwrap();
    bytes[position..position + requirement.len()].copy_from_slice(b"Boundary::fake");
    assert_eq!(recover(&bytes), Err(Error::InvalidValue));
}
