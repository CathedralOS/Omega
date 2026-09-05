use super::{
    Error,
    tests::{fixture, recover},
};
use crate::record::*;

pub(super) fn generic_fixture() -> PackagePolicyTerminalPermissions {
    let mut policy = fixture();
    let service = &mut policy.services[0];
    let machine = service.methods[0].signature.static_parameters[0].clone();
    let properties = machine.bounds;
    service.static_parameters = vec![
        PackagePolicyTypeParameter {
            kind: PackagePolicyTypeParameterKind::Type,
            bounds: properties,
        },
        PackagePolicyTypeParameter {
            kind: PackagePolicyTypeParameterKind::Const(PackageReviewTypeIdentity {
                canonical: "u64".into(),
            }),
            bounds: properties,
        },
        machine,
        PackagePolicyTypeParameter {
            kind: PackagePolicyTypeParameterKind::Proposition(
                PackageReviewPropositionParameterSignature {
                    parameters: vec![PackageReviewPropositionParameterValue {
                        type_identity: PackageReviewTypeIdentity {
                            canonical: "u64".into(),
                        },
                    }],
                },
            ),
            bounds: properties,
        },
    ];
    service.lifetime_parameter_count = 2;
    for method in &mut service.methods {
        method.calling = None;
        method.signature.schema_lifetime_parameter_count = 2;
        method.signature.schema_arguments = (0..4)
            .map(|ordinal| PackageReviewTypeIdentity {
                canonical: {
                    let mut identity = String::new();
                    crate::record::package::write_service_parameter_identity(
                        &mut identity,
                        ordinal,
                    )
                    .unwrap();
                    identity
                },
            })
            .collect();
        method.signature.requirement_lifetime_arguments = vec![1, 0];
    }
    policy
}

#[test]
fn complete_generic_service_telescope_and_contract_kinds_roundtrip() {
    let original = generic_fixture();
    assert_eq!(
        original.services[0].methods[0].signature.schema_arguments[0].canonical,
        "14:signature-type32:named(name(service-parameter:0))5:named"
    );
    let baseline = original.canonical_bytes().unwrap();
    assert_eq!(recover(&baseline).unwrap(), original);
    let mut changed = original.clone();
    changed.services[0].static_parameters[0].bounds.multiplicity =
        language_semantics::Multiplicity::Linear;
    let bytes = changed.canonical_bytes().unwrap();
    assert_ne!(bytes, baseline);
    assert_eq!(recover(&bytes).unwrap(), changed);
    let mut changed = original.clone();
    changed.services[0].static_parameters[1].kind =
        PackagePolicyTypeParameterKind::Const(PackageReviewTypeIdentity {
            canonical: "u32".into(),
        });
    let bytes = changed.canonical_bytes().unwrap();
    assert_ne!(bytes, baseline);
    assert_eq!(recover(&bytes).unwrap(), changed);
    let mut changed = original;
    changed.services[0].lifetime_parameter_count = 3;
    assert!(changed.canonical_bytes().is_err());
}

#[test]
fn root_binder_arguments_reject_substitution_reordering_and_method_namespace() {
    let original = generic_fixture();
    let mut cases = Vec::new();
    let mut changed = original.clone();
    let service = &mut changed.services[0];
    let method = &mut service.methods[1];
    let mut calling = fixture().services[0].methods[1].calling.take().unwrap();
    calling.boundary_arguments = method.signature.schema_arguments.clone();
    calling.boundary_lifetime_parameter_count = method.signature.schema_lifetime_parameter_count;
    calling.requirement_arguments = method.signature.requirement_arguments.clone();
    calling.requirement_lifetime_arguments =
        method.signature.requirement_lifetime_arguments.clone();
    calling.requirement_lifetime_parameter_count =
        method.signature.requirement_lifetime_parameter_count;
    method.calling = Some(calling);
    assert!(
        method
            .validate_service_structure(&service.service, changed.target)
            .is_ok()
    );
    cases.push(changed);
    let mut changed = original.clone();
    changed.services[0].static_parameters.pop();
    cases.push(changed);
    let mut changed = original.clone();
    changed.services[0].methods[0]
        .signature
        .schema_arguments
        .swap(0, 1);
    cases.push(changed);
    let mut changed = original.clone();
    changed.services[0].methods[0].signature.schema_arguments[0]
        .canonical
        .push('x');
    cases.push(changed);
    let mut changed = original.clone();
    changed.services[0].methods[0].signature.schema_arguments[0].canonical =
        "14:signature-type29:named(name(type-parameter:0))5:named".into();
    cases.push(changed);
    let mut changed = original.clone();
    changed.services[0].methods[0].parameter_type_identities[0] = "AuthoredName".into();
    cases.push(changed);
    let mut changed = original.clone();
    let method = &mut changed.services[0].methods[0];
    method.has_result = true;
    method.signature.result = Some(PackageReviewTypeIdentity {
        canonical: "u64".into(),
    });
    method.result_type_identity = Some("AuthoredResultName".into());
    cases.push(changed);
    for policy in cases {
        assert!(policy.canonical_bytes().is_err());
    }
    let mut bytes = original.canonical_bytes().unwrap();
    let coordinate = b"service-parameter:0";
    let position = bytes
        .windows(coordinate.len())
        .position(|window| window == coordinate)
        .unwrap();
    bytes[position + coordinate.len() - 1] = b'1';
    assert_eq!(recover(&bytes), Err(Error::InvalidValue));
    let mut maximum = String::new();
    crate::record::package::write_service_parameter_identity(&mut maximum, usize::MAX).unwrap();
    assert!(maximum.contains(&usize::MAX.to_string()));
}

#[test]
fn generic_service_truncations_and_static_parameter_tags_fail_closed() {
    let bytes = generic_fixture().canonical_bytes().unwrap();
    for end in 0..bytes.len() {
        assert!(recover(&bytes[..end]).is_err(), "generic prefix {end}");
    }
    let service_name = b"Boundary";
    let position = bytes
        .windows(service_name.len())
        .position(|window| window == service_name)
        .unwrap()
        + service_name.len();
    let mut changed = bytes;
    changed[position + 8] = 255;
    assert_eq!(recover(&changed), Err(Error::InvalidTag));
}
