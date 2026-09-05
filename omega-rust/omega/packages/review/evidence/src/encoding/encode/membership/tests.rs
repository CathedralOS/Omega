use super::*;
use crate::record::*;
use omega_target::TargetProfile;

fn package(byte: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([byte; 32]).unwrap()
}
fn frame(label: &str, children: &[&str]) -> String {
    let mut result = String::new();
    crate::record::write_framed_identity(&mut result, label, children.iter().copied()).unwrap();
    result
}
fn owner(byte: u8) -> String {
    format!("package:{}", format!("{byte:02x}").repeat(32))
}
fn foreign_type() -> String {
    format!(
        "nominal(package-owner(32:{}),path(Foreign))",
        "02".repeat(32)
    )
}
fn signature(runtime: &str) -> String {
    frame("signature-type", &[runtime, "named"])
}

fn fixture() -> PackagePolicyBaseline {
    let package = package(1);
    let target = TargetProfile::LinuxX64;
    PackagePolicyBaseline {
        package,
        target,
        public_api: PackagePolicyPublicApi {
            traits: vec![],
            conformances: vec![],
            domains: vec![],
            propositions: vec![],
            consts: vec![PackageReviewConstShape {
                identity: PackageReviewNominalIdentity {
                    owner: PackageReviewNominalOwner::Package(package),
                    path: "Value".into(),
                },
                declared_type: PackageReviewTypeIdentity {
                    canonical: signature(&foreign_type()),
                },
                canonical_value_encoding: "0".into(),
            }],
            operators: vec![],
            data: vec![],
        },
        callables: PackagePolicyCallables {
            package,
            target,
            callables: vec![],
        },
        selected_providers: PackagePolicySelectedProviders {
            package,
            target,
            plans: vec![],
            families: vec![],
        },
        terminal_permissions: PackagePolicyTerminalPermissions {
            package,
            target,
            services: vec![],
        },
        representation: PackagePolicyRepresentation {
            package,
            target: PackageReviewRepresentationTarget {
                profile: PackageReviewRepresentationTargetProfile::LinuxX64,
                architecture: PackageReviewRepresentationArchitecture::X86_64,
                object_format: PackageReviewRepresentationObjectFormat::Elf,
                pointer_size: 8,
                pointer_alignment: 8,
            },
            declarations: vec![],
            producer_availability: vec![],
            selected_availability: vec![],
            demands: vec![],
        },
        external_supplies: vec![],
        dangerous_capabilities: vec![],
        slack_uses: vec![],
        semantic_dependencies: vec![],
        boundary_applications: PackagePolicyBoundaryApplications {
            demands: vec![],
            realizations: vec![],
        },
    }
}

#[test]
fn complete_traversal_checks_typed_owners_without_changing_encoding() {
    let value = fixture();
    let bytes = value.canonical_bytes().unwrap();
    let text = value.canonical_text().unwrap();
    assert_eq!(
        value.validate_package_membership(
            |candidate| candidate == package(1),
            PackagePolicyMembershipLimits::default()
        ),
        Err(PackagePolicyMembershipError::UnknownPackage {
            package: package(2)
        })
    );
    let usage = value
        .validate_package_membership(
            |candidate| [package(1), package(2)].contains(&candidate),
            PackagePolicyMembershipLimits::default(),
        )
        .unwrap();
    assert_eq!(
        usage.owned_bytes(),
        0,
        "borrowed traversal emits no canonical output"
    );
    assert!(usage.identity_nodes() > 2);
    assert_eq!(value.canonical_bytes().unwrap(), bytes);
    assert_eq!(value.canonical_text().unwrap(), text);
    let recovered = PackagePolicyBaseline::recover_canonical(
        &bytes,
        crate::encoding::PackagePolicyRecoveryLimits::default(),
    )
    .unwrap();
    assert_eq!(
        recovered
            .validate_package_membership(|_| true, PackagePolicyMembershipLimits::default())
            .unwrap(),
        usage
    );
}

#[test]
fn shared_node_budget_is_lowerable_across_individually_valid_baselines() {
    let value = fixture();
    let usage = value
        .validate_package_membership(|_| true, PackagePolicyMembershipLimits::default())
        .unwrap();
    let exact = PackagePolicyMembershipLimits::new(0, usage.identity_nodes(), 128);
    assert_eq!(
        value.validate_package_membership(|_| true, exact).unwrap(),
        usage
    );
    assert_eq!(
        value.validate_package_membership(
            |_| true,
            PackagePolicyMembershipLimits::new(0, usage.identity_nodes() - 1, 128)
        ),
        Err(PackagePolicyMembershipError::IdentityNodeLimitExceeded)
    );
    let aggregate_nodes = usage.identity_nodes() * 2 - 1;
    let first = value
        .validate_package_membership(
            |_| true,
            PackagePolicyMembershipLimits::new(0, aggregate_nodes, 128),
        )
        .unwrap();
    assert_eq!(
        value.validate_package_membership(
            |_| true,
            PackagePolicyMembershipLimits::new(0, aggregate_nodes - first.identity_nodes(), 128)
        ),
        Err(PackagePolicyMembershipError::IdentityNodeLimitExceeded)
    );
    assert_eq!(
        value.validate_package_membership(
            |_| true,
            PackagePolicyMembershipLimits::new(0, usize::MAX, 0)
        ),
        Err(PackagePolicyMembershipError::NestingLimitExceeded)
    );
}

#[test]
fn callable_callback_and_caller_binder_wrappers_keep_foreign_owner_meaning() {
    let parameter = frame(
        "parameter",
        &[&signature(&foreign_type()), "false:false:false"],
    );
    let result = frame("result-none", &[]);
    let callable = frame(
        "conformance-callable",
        &["run", "overload", "run", "machine", &parameter, &result],
    );
    let callback = frame("callback-static-parameter", &[&callable, "0"]);
    let caller = frame("conformance-caller-binder", &[&owner(2), &callable]);
    for identity in [&callable, &callback, &caller] {
        let mut visitor = visitor::Visitor::new(
            |candidate| candidate == package(1),
            PackagePolicyMembershipLimits::default(),
        );
        assert_eq!(
            visitor.nominal_path(identity),
            Err(PackagePolicyMembershipError::UnknownPackage {
                package: package(2)
            })
        );
        let mut visitor = visitor::Visitor::new(|_| true, PackagePolicyMembershipLimits::default());
        visitor.nominal_path(identity).unwrap();
    }
    let mut visitor = visitor::Visitor::new(|_| false, PackagePolicyMembershipLimits::default());
    visitor
        .nominal_path(&format!("Literal::{}", owner(2)))
        .unwrap();
}

#[test]
fn declared_domain_lifetime_topology_has_independent_package_membership() {
    let declared = frame("declared-domain", &[&owner(2), "Domain"]);
    let constraint = frame(&declared, &["named"]);
    let topology = frame("constrained", &["named", &constraint]);
    let identity = frame(
        "signature-type",
        &["named(name(type-parameter:0))", &topology],
    );
    let mut visitor = visitor::Visitor::new(
        |candidate| candidate == package(1),
        PackagePolicyMembershipLimits::default(),
    );
    assert_eq!(
        visitor.type_identity(&identity),
        Err(PackagePolicyMembershipError::UnknownPackage {
            package: package(2)
        })
    );
    let mut visitor = visitor::Visitor::new(|_| true, PackagePolicyMembershipLimits::default());
    visitor.type_identity(&identity).unwrap();
    let toolchain_domain = frame(
        "declared-domain",
        &[&format!("toolchain-source:{}", "00".repeat(32)), "Domain"],
    );
    let toolchain_constraint = frame(&toolchain_domain, &["named"]);
    let toolchain_topology = frame("constrained", &["named", &toolchain_constraint]);
    let toolchain_identity = frame("signature-type", &["unit", &toolchain_topology]);
    let mut visitor = visitor::Visitor::new(|_| false, PackagePolicyMembershipLimits::default());
    visitor.type_identity(&toolchain_identity).unwrap();
}

#[test]
fn malformed_and_overdeep_wrappers_reject_before_unbounded_descent() {
    let valid = signature(&foreign_type());
    for end in 0..valid.len() {
        let mut visitor = visitor::Visitor::new(|_| true, PackagePolicyMembershipLimits::default());
        assert!(
            visitor.type_identity(&valid[..end]).is_err(),
            "accepted truncated prefix {end}"
        );
    }
    for invalid in [
        format!("{valid}trailing"),
        valid.replacen("14:", "014:", 1),
        "99999999999999999999999999999:signature-type".into(),
    ] {
        let mut visitor = visitor::Visitor::new(|_| true, PackagePolicyMembershipLimits::default());
        assert_eq!(
            visitor.type_identity(&invalid),
            Err(PackagePolicyMembershipError::MalformedIdentity)
        );
    }
    let mut nested = "Root".to_owned();
    for _ in 0..129 {
        nested = frame("callback-static-parameter", &[&nested, "0"]);
    }
    let mut visitor = visitor::Visitor::new(
        |_| true,
        PackagePolicyMembershipLimits::new(usize::MAX, usize::MAX, usize::MAX),
    );
    assert_eq!(
        visitor.nominal_path(&nested),
        Err(PackagePolicyMembershipError::NestingLimitExceeded)
    );
}

#[test]
fn arbitrary_literal_text_is_not_a_package_owner() {
    let mut value = fixture();
    value.public_api.consts[0].declared_type.canonical = "named(name(type-parameter:0))".into();
    value.public_api.consts[0].canonical_value_encoding = foreign_type();
    value
        .validate_package_membership(
            |candidate| candidate == package(1),
            PackagePolicyMembershipLimits::default(),
        )
        .unwrap();
}

#[test]
fn carried_private_nominals_require_only_package_membership_not_public_declarations() {
    let mut value = fixture();
    value.public_api.consts.clear();
    value
        .semantic_dependencies
        .push(PackagePolicySemanticDependency {
            consumer: PackagePolicySemanticDependencyConsumer::PackageImplementation,
            dependency: PackageReviewNominalIdentity {
                owner: PackageReviewNominalOwner::Package(package(2)),
                path: "Private::Layout::CarriedType".into(),
            },
            exposure: PackageReviewSemanticDependencyExposure::PrivateImplementation,
            kind: PackageReviewSemanticDependencyKind::Layout,
        });
    assert_eq!(
        value.validate_package_membership(
            |candidate| candidate == package(1),
            PackagePolicyMembershipLimits::default()
        ),
        Err(PackagePolicyMembershipError::UnknownPackage {
            package: package(2)
        })
    );
    value
        .validate_package_membership(
            |candidate| [package(1), package(2)].contains(&candidate),
            PackagePolicyMembershipLimits::default(),
        )
        .unwrap();
    // Exact toolchain sources are not managed package keys. They never invoke
    // membership, and private spelling does not require another API baseline.
    value.semantic_dependencies[0].dependency.owner =
        PackageReviewNominalOwner::ToolchainSource(PackageReviewToolchainSourceIdentity {
            digest: [3; 32],
        });
    value
        .validate_package_membership(
            |candidate| candidate == package(1),
            PackagePolicyMembershipLimits::default(),
        )
        .unwrap();
    value.semantic_dependencies[0].dependency.owner = PackageReviewNominalOwner::Unresolved;
    assert_eq!(
        value.validate_package_membership(|_| true, PackagePolicyMembershipLimits::default()),
        Err(PackagePolicyMembershipError::InvalidPolicy)
    );
}

#[test]
fn embedded_caller_binders_share_precharged_unescape_storage() {
    let name = frame(
        "conformance-caller-binder",
        &[&owner(2), "Owner::parameter(with, punctuation)"],
    );
    let escaped = name
        .chars()
        .flat_map(|character| {
            if matches!(character, '(' | ')' | ',' | '\\') {
                vec!['\\', character]
            } else {
                vec![character]
            }
        })
        .collect::<String>();
    let runtime = format!("named(name({escaped}))");
    let identity = signature(&runtime);
    let mut visitor = visitor::Visitor::new(|_| true, PackagePolicyMembershipLimits::default());
    visitor.type_identity(&identity).unwrap();
    let usage = visitor.usage();
    assert_eq!(usage.owned_bytes(), name.len());
    for remaining in [name.len() - 1, 0] {
        let mut visitor = visitor::Visitor::new(
            |_| true,
            PackagePolicyMembershipLimits::new(remaining, usage.identity_nodes(), 128),
        );
        assert_eq!(
            visitor.type_identity(&identity),
            Err(PackagePolicyMembershipError::OwnedBytesLimitExceeded)
        );
    }
    let mut visitor = visitor::Visitor::new(
        |_| true,
        PackagePolicyMembershipLimits::new(name.len(), usage.identity_nodes(), 128),
    );
    visitor.type_identity(&identity).unwrap();
    assert_eq!(visitor.usage(), usage);
    let mut visitor = visitor::Visitor::new(
        |candidate| candidate == package(1),
        PackagePolicyMembershipLimits::default(),
    );
    assert_eq!(
        visitor.type_identity(&identity),
        Err(PackagePolicyMembershipError::UnknownPackage {
            package: package(2)
        })
    );
}
