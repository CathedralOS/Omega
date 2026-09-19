use crate::support::*;
use compiler::CheckedCompileRequest;
use package_evidence::record::PackagePolicyCallableRole;

#[test]
fn public_domain_shape_changes_change_comparison_encoding() {
    let first = TempPackage::new();
    let second = TempPackage::new();
    first.write(
        "main.omg",
        "pub data Packet { value: u32; }\npub domain Packet::Ready;\n",
    );
    second.write(
        "main.omg",
        "pub data Packet { value: u32; }\npub domain Packet::Prepared;\n",
    );
    let build = r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    first.write("build.omg", build);
    second.write("build.omg", build);

    let encode = |package: &TempPackage| {
        let checked = compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
        })
        .expect("public-domain fixture should check");
        project_checked_package_review(&checked)
            .expect("public-domain review should close")
            .canonical_review_bytes()
            .expect("public-domain encoding")
    };

    assert_ne!(encode(&first), encode(&second));
}

#[test]
fn public_domain_semantic_roles_project_from_exact_typed_identity() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub domain i32::Degrees;
pub domain i32::Radians;

pub operator + add(
    left: i32 in Degrees,
    right: i32 in Degrees
) -> i32 in Degrees;
"#,
    );
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
    })
    .expect("public semantic-role fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("exact typed semantic roles should project");
    let degrees = review
        .public_domains()
        .iter()
        .find(|domain| domain.identity().path() == "i32::Degrees")
        .expect("public Degrees domain row");
    assert_eq!(
        degrees.semantic_roles(),
        &[PackageReviewDomainSemanticRole::DenotationDimension]
    );
    let radians = review
        .public_domains()
        .iter()
        .find(|domain| domain.identity().path() == "i32::Radians")
        .expect("public Radians domain row");
    assert!(radians.semantic_roles().is_empty());
    assert!(review.public_operators().iter().any(|operator| {
        operator.coordinate().identity().path().ends_with("::add")
            || operator.coordinate().identity().path() == "add"
    }));

    let mut role_removed = checked.clone();
    role_removed
        .typed
        .domain_definitions
        .for_each_mut(|_, domain| {
            if domain.name.as_str() == "i32::Degrees" {
                domain.semantic_roles.denotation_dimension = None;
            }
        });
    let role_removed = project_checked_package_review(&role_removed)
        .expect("an absent semantic role remains a coherent distinct declaration");
    assert_ne!(
        review.canonical_review_bytes().unwrap(),
        role_removed.canonical_review_bytes().unwrap(),
        "semantic-role presence must change canonical package-review identity"
    );

    let wrong_identity = checked
        .typed
        .domain_definitions
        .iter()
        .find(|(_, domain)| domain.name.as_str() == "i32::Radians")
        .expect("typed Radians declaration")
        .1
        .semantic_id;
    let mut spoofed = checked.clone();
    spoofed.typed.domain_definitions.for_each_mut(|_, domain| {
        if domain.name.as_str() == "i32::Degrees" {
            domain.semantic_roles.denotation_dimension = Some(wrong_identity);
        }
    });
    let diagnostics = project_checked_package_review(&spoofed)
        .expect_err("a semantic role pointing at another typed domain must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("semantic role does not name its exact typed semantic identity")
    }));
}

#[test]
fn public_domain_generic_binders_are_alpha_normalized() {
    let first = TempPackage::new();
    let second = TempPackage::new();
    first.write(
        "main.omg",
        r#"pub data Unit { code: u32; }
pub domain<Carrier, const Index: Unit> Carrier::Tagged<Index>;
"#,
    );
    second.write(
        "main.omg",
        r#"pub data Unit { code: u32; }
pub domain<Value, const Tag: Unit> Value::Tagged<Tag>;
"#,
    );
    let build = r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    first.write("build.omg", build);
    second.write("build.omg", build);

    let encode = |package: &TempPackage| {
        let checked = compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
        })
        .expect("generic public-domain fixture should check");
        project_checked_package_review(&checked)
            .expect("generic public-domain review should close")
            .canonical_review_bytes()
            .expect("generic public-domain encoding")
    };

    assert_eq!(encode(&first), encode(&second));
}

#[test]
fn public_domain_classification_and_establishment_routes_are_exact_review_rows() {
    let classified = TempPackage::new();
    let routed = TempPackage::new();
    classified.write(
        "main.omg",
        r#"pub data SchedulerHandle { id: u64; }
pub domain SchedulerHandle::WeakFair
satisfies ProgressProfile
established by SchedulerAdmission::grant;
pub boundary trait SchedulerAdmission {
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
}
"#,
    );
    routed.write(
        "main.omg",
        r#"pub data SchedulerHandle { id: u64; }
pub domain SchedulerHandle::WeakFair
established by SchedulerAdmission::grant;
pub boundary trait SchedulerAdmission {
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
}
"#,
    );
    let build = r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    classified.write("build.omg", build);
    routed.write("build.omg", build);

    let compile = |package: &TempPackage| {
        let checked = compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
        })
        .expect("routed public-domain fixture should check");
        project_checked_package_review(&checked).expect("routed public-domain review should close")
    };
    let classified_review = compile(&classified);
    let [domain] = classified_review.public_domains() else {
        panic!("one classified public domain row")
    };
    assert_eq!(
        domain.classification(),
        Some(PackageReviewDomainClassification::ProgressProfile)
    );
    let [route] = domain.establishment_routes() else {
        panic!("one exact establishment route")
    };
    assert_eq!(
        route.kind(),
        PackageReviewDomainEstablishmentKind::BoundaryRequirement
    );
    assert_eq!(route.trait_identity().unwrap().path(), "SchedulerAdmission");
    assert!(
        route
            .requirement_identity()
            .unwrap()
            .path()
            .starts_with("named-callable(")
    );
    assert!(
        route
            .requirement_identity()
            .unwrap()
            .path()
            .contains("SchedulerAdmission::grant")
    );

    assert_ne!(
        classified_review
            .canonical_review_bytes()
            .expect("classified public-domain encoding"),
        compile(&routed)
            .canonical_review_bytes()
            .expect("unclassified routed public-domain encoding")
    );
}

#[test]
fn public_domain_establishment_route_order_is_canonical() {
    let first = TempPackage::new();
    let second = TempPackage::new();
    let source = |routes: &str| {
        format!(
            r#"pub data SchedulerHandle {{ id: u64; }}
pub domain SchedulerHandle::Scheduled
established by {routes};
pub boundary trait PrimaryAdmission {{
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in Scheduled;
}}
pub boundary trait BackupAdmission {{
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in Scheduled;
}}
"#
        )
    };
    first.write(
        "main.omg",
        &source("PrimaryAdmission::grant, BackupAdmission::grant"),
    );
    second.write(
        "main.omg",
        &source("BackupAdmission::grant, PrimaryAdmission::grant"),
    );
    let build = r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    first.write("build.omg", build);
    second.write("build.omg", build);

    let encode = |package: &TempPackage| {
        let checked = compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
        })
        .expect("multi-route public-domain fixture should check");
        project_checked_package_review(&checked)
            .expect("multi-route public-domain review should close")
            .canonical_review_bytes()
            .expect("multi-route public-domain encoding")
    };

    assert_eq!(encode(&first), encode(&second));
}

#[test]
fn public_domain_aliases_flatten_to_canonical_package_qualified_atoms() {
    let first = TempPackage::new();
    let second = TempPackage::new();
    first.write(
        "main.omg",
        r#"pub data Socket { descriptor: u64; }
pub domain Socket::Connected;
pub domain Socket::Authenticated;
pub domain Socket::Trusted = Socket::Authenticated;
pub domain Socket::Usable = Socket::Connected & Socket::Trusted;
pub domain u64::Portable = Carry::Portable;
"#,
    );
    second.write(
        "main.omg",
        r#"pub data Socket { descriptor: u64; }
pub domain Socket::Connected;
pub domain Socket::Authenticated;
pub domain Socket::Trusted = Socket::Authenticated;
pub domain Socket::Usable = Socket::Trusted & Socket::Connected;
pub domain u64::Portable = Carry::Portable;
"#,
    );
    let build = r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    first.write("build.omg", build);
    second.write("build.omg", build);

    let compile = |package: &TempPackage| {
        let checked = compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
        })
        .expect("public-domain alias fixture should check");
        project_checked_package_review(&checked).expect("public-domain alias review should close")
    };
    let first_review = compile(&first);
    let usable = first_review
        .public_domains()
        .iter()
        .find(|domain| domain.identity().path() == "Socket::Usable")
        .expect("usable alias row");
    let usable_atoms = usable.alias_expansion().expect("usable alias expansion");
    assert_eq!(
        usable_atoms
            .iter()
            .map(|atom| match atom {
                PackageReviewDomainAliasAtom::Declared(identity) => identity.path(),
                PackageReviewDomainAliasAtom::Carry(_) => panic!("ordinary domain became carry"),
            })
            .collect::<Vec<_>>(),
        ["Socket::Authenticated", "Socket::Connected"]
    );
    assert!(usable_atoms.iter().all(|atom| {
        matches!(
            atom,
            PackageReviewDomainAliasAtom::Declared(identity)
                if identity.owner() == PackageReviewNominalOwner::Package(package_identity())
        )
    }));

    let portable = first_review
        .public_domains()
        .iter()
        .find(|domain| domain.identity().path() == "u64::Portable")
        .expect("portable alias row");
    let portable_atoms = portable
        .alias_expansion()
        .expect("portable alias expansion");
    assert_eq!(
        portable_atoms,
        &language_semantics::CarryPermission::ALL.map(PackageReviewDomainAliasAtom::Carry)
    );

    assert_eq!(
        first_review
            .canonical_review_bytes()
            .expect("first alias encoding"),
        compile(&second)
            .canonical_review_bytes()
            .expect("reordered alias encoding")
    );
}

#[test]
fn public_exact_machine_issuer_routes_capture_checked_wrappers() {
    for (declaration, issuer, route) in [
        ("", "issue", "issue"),
        ("", "issue", "issuance::issue"),
        (
            "pub data Factory {}",
            "Factory::issue",
            "issuance::Factory::issue",
        ),
    ] {
        let package = TempPackage::new();
        package.write("main.omg", &format!(
            "module issuance;\n{declaration}\npub domain u64::Issued requires self > 0; established by {route};\n\
             pub machine {issuer}() -> u64 in Issued {{ 7 }}\n\
             pub machine forward() -> u64 in Issued {{ {issuer}() }}\n"
        ));
        package.write(
            "build.omg",
            "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
        );
        let checked = compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
        })
        .expect("exact issuer and forwarding wrapper should check");
        let review = project_checked_package_review(&checked)
            .expect("public exact-machine issuer route should capture");
        let [domain] = review.public_domains() else {
            panic!("one public domain")
        };
        let [route] = domain.establishment_routes() else {
            panic!("one issuer route")
        };
        assert_eq!(
            route.kind(),
            PackageReviewDomainEstablishmentKind::ExactMachine
        );
        assert!(route.trait_identity().is_none());
        assert!(route.requirement_identity().is_none());
        let identity = route.machine_identity().expect("concrete issuer identity");
        assert_eq!(
            identity.owner(),
            PackageReviewNominalOwner::Package(package_identity())
        );
        assert!(
            identity.path().starts_with("named-callable("),
            "{}",
            identity.path()
        );
        assert!(identity.path().contains(issuer), "{}", identity.path());
        assert!(!identity.path().contains("forward"));
        let policy = package_evidence::project_checked_package_policy(
            &checked,
            review.target(),
            package_identity(),
        )
        .expect("exact issuer route participates in complete inert policy");
        let bytes = policy.canonical_bytes().unwrap();
        let recovered = package_evidence::record::PackagePolicyBaseline::recover_canonical(
            &bytes,
            package_evidence::encoding::PackagePolicyRecoveryLimits::default(),
        )
        .unwrap();
        assert_eq!(recovered, policy);
        let text = policy.canonical_text().unwrap();
        assert!(text.contains("exact_machine"));
        assert_eq!(
            package_evidence::record::PackagePolicyBaseline::recover_text(
                &text,
                package_evidence::encoding::PackagePolicyTextRecoveryLimits::default()
            )
            .unwrap(),
            policy
        );
        let wrong_label = text.replacen("machine_identity", "requirement_identity", 1);
        assert!(
            package_evidence::record::PackagePolicyBaseline::recover_text(
                &wrong_label,
                package_evidence::encoding::PackagePolicyTextRecoveryLimits::default()
            )
            .is_err()
        );
        let rows = review.canonical_rows().unwrap();
        let ledger = ordinary_package_obligation_ledger_from_compiler_rows(
            checked.custody.dependency_closure().cloned().unwrap(),
            &rows,
        )
        .unwrap();
        let recovered_ledger = decode_ordinary_package_obligation_ledger(
            &encode_ordinary_package_obligation_ledger(&ledger).unwrap(),
        )
        .unwrap();
        validate_ordinary_package_obligation_ledger(&recovered_ledger, &checked).unwrap();
        // The envelope carries inert bytes. A plausible substituted issuer
        // remains decodable but must fail independent local reconstruction.
        for wrong_owner in [false, true] {
            let changed_rows = rows
                .iter()
                .map(|row| {
                    let mut bytes = encode_package_review_canonical_row(row).unwrap();
                    if row.kind() == PackageReviewCanonicalRowKind::PublicDomain {
                        let positions = bytes
                            .windows(identity.path().len())
                            .enumerate()
                            .filter_map(|(index, value)| {
                                (value == identity.path().as_bytes()).then_some(index)
                            })
                            .collect::<Vec<_>>();
                        let [position] = positions.as_slice() else {
                            panic!("one exact route identity in domain payload")
                        };
                        if wrong_owner {
                            // The exact same callable spelling under another
                            // package owner must not authorize the original row.
                            bytes[position - 8 - 32] ^= 0x40;
                        } else {
                            let offset = identity.path().find("issue").unwrap();
                            bytes[position + offset..position + offset + 5]
                                .copy_from_slice(b"other");
                        }
                    }
                    decode_package_review_canonical_row(&bytes).unwrap()
                })
                .collect::<Vec<_>>();
            let supplied = recover_ordinary_package_obligation_ledger(
                checked.custody.dependency_closure().cloned().unwrap(),
                &changed_rows,
            )
            .unwrap();
            assert!(
                validate_ordinary_package_obligation_ledger(&supplied, &checked).is_err(),
                "a supplied wrong issuer identity must fail local reconstruction"
            );
        }
    }
}

#[test]
fn public_exact_machine_issuer_order_is_canonical_and_predicates_remain_obligations() {
    let mut encodings = Vec::new();
    for routes in [
        "issuance::first, issuance::second",
        "issuance::second, issuance::first",
    ] {
        let package = TempPackage::new();
        package.write("main.omg", &format!(
            "module issuance; pub domain u64::Issued requires self > 0; established by {routes};\n\
             pub machine first() -> u64 in Issued {{ 7 }}\n\
             pub machine second() -> u64 in Issued {{ 9 }}\n\
             pub machine forward() -> u64 in Issued {{ first() }}"
        ));
        package.write(
            "build.omg",
            "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }",
        );
        let checked = compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
        })
        .unwrap();
        encodings.push(
            project_checked_package_review(&checked)
                .unwrap()
                .canonical_review_bytes()
                .unwrap(),
        );
    }
    assert_eq!(encodings[0], encodings[1]);

    let invalid = TempPackage::new();
    invalid.write("main.omg", "module issuance; pub domain u64::Issued requires self > 0; established by issuance::issue; pub machine issue() -> u64 in Issued { 0 }");
    invalid.write(
        "build.omg",
        "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }",
    );
    let diagnostics = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&invalid.0)),
        ..CheckedCompileRequest::new(&invalid.0.join("main.omg"), Some("windows_x86_64"))
    })
    .expect_err("issuer authorization cannot prove a false predicate");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove scalar result domain")),
        "{diagnostics:?}"
    );
}

#[test]
fn public_domain_private_machine_catalog_preserves_wrapper_and_review_identity() {
    let package = TempPackage::new();
    package.write("main.omg", "module issuance; pub domain u64::Issued requires self > 0; established by issue; machine issue() -> u64 in Issued { 7 } pub machine forward() -> u64 in Issued { issue() }");
    package.write(
        "build.omg",
        "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }",
    );
    let checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
    })
    .expect("public domain may authorize its private checked issuer");
    let review = project_checked_package_review(&checked)
        .expect("private catalog remains exact review metadata");
    let [domain] = review.public_domains() else {
        panic!("one domain")
    };
    let [route] = domain.establishment_routes() else {
        panic!("one private issuer")
    };
    let machine = route.machine_identity().unwrap();
    assert_eq!(
        machine.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    let issuer = checked
        .machines()
        .iter()
        .find(|declaration| declaration.name.as_str() == "issue")
        .unwrap();
    assert_eq!(
        machine.path(),
        checked
            .normalized_machine_overload_identity(issuer)
            .unwrap()
            .identity()
    );
    assert!(
        review
            .callables()
            .iter()
            .any(|callable| callable.identity().path().contains("forward"))
    );
    assert!(
        !review
            .callables()
            .iter()
            .any(|callable| callable.identity().path()
                == checked.symbols.display_path(issuer.symbol, "::"))
    );
    assert!(!machine.path().contains("forward"));
    let rows = review.canonical_rows().unwrap();
    let ledger = ordinary_package_obligation_ledger_from_compiler_rows(
        checked.custody.dependency_closure().cloned().unwrap(),
        &rows,
    )
    .unwrap();
    let decoded = decode_ordinary_package_obligation_ledger(
        &encode_ordinary_package_obligation_ledger(&ledger).unwrap(),
    )
    .unwrap();
    validate_ordinary_package_obligation_ledger(&decoded, &checked).unwrap();
}

#[test]
fn public_domain_private_catalogs_retain_requirement_and_attached_identities() {
    for (source, kind) in [
        (
            "data Catalog {} pub domain u64::Issued requires self > 0; established by Catalog::issue; machine Catalog::issue() -> u64 in Issued { 7 } pub machine forward() -> u64 in Issued { Catalog::issue() }",
            PackageReviewDomainEstablishmentKind::ExactMachine,
        ),
        (
            "trait Catalog { machine issue() -> u64 in Issued; } pub domain u64::Issued requires self > 0; established by Catalog::issue; machine issue()->u64 in Issued satisfies Catalog::issue {7} pub machine forward()->u64 in Issued {issue()}",
            PackageReviewDomainEstablishmentKind::CheckedRequirement,
        ),
        (
            "boundary trait Catalog { machine issue() -> u64 in Issued; } pub domain u64::Issued requires self > 0; established by Catalog::issue;",
            PackageReviewDomainEstablishmentKind::BoundaryRequirement,
        ),
    ] {
        let package = TempPackage::new();
        package.write("main.omg", source);
        package.write(
            "build.omg",
            "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }",
        );
        let checked = compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
        })
        .unwrap();
        let review = project_checked_package_review(&checked).unwrap();
        let [domain] = review.public_domains() else {
            panic!("one catalog")
        };
        let [route] = domain.establishment_routes() else {
            panic!("one issuer")
        };
        assert_eq!(route.kind(), kind);
        assert!(
            review
                .public_traits()
                .iter()
                .all(|definition| !definition.identity().path().contains("Catalog"))
        );
        assert!(
            review
                .public_data()
                .iter()
                .all(|definition| !definition.identity().path().contains("Catalog"))
        );
        let rows = review.canonical_rows().unwrap();
        let ledger = ordinary_package_obligation_ledger_from_compiler_rows(
            checked.custody.dependency_closure().cloned().unwrap(),
            &rows,
        )
        .unwrap();
        validate_ordinary_package_obligation_ledger(&ledger, &checked).unwrap();
    }
}

#[test]
fn private_boundary_issuer_remains_an_explicit_review_assumption() {
    let package = TempPackage::new();
    // Catalog identity and an explicit assumption are review data. They do
    // not constitute the authorized boundary-requirement receipt needed to
    // establish routed membership at an invocation.
    let source = "pub domain u64::Issued requires self > 0; established by issue; boundary machine issue() -> u64 in Issued ensures result > 0;";
    package.write("main.omg", source);
    package.write(
        "build.omg",
        "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }",
    );
    let checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
    })
    .unwrap();
    let review = project_checked_package_review(&checked).unwrap();
    assert_eq!(
        review.public_domains()[0].establishment_routes()[0].kind(),
        PackageReviewDomainEstablishmentKind::ExactMachine
    );
    let policy = package_evidence::project_checked_package_policy(
        &checked,
        review.target(),
        package_identity(),
    )
    .unwrap();
    let claim = policy
        .callables()
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("issue"))
        .unwrap();
    assert_eq!(claim.role(), PackagePolicyCallableRole::PrivateAssumption);
    assert_eq!(claim.supply(), PackageReviewCallableSupply::AdmissionClaim);
    assert!(!claim.contracts().is_empty());
    assert!(
        review
            .public_traits()
            .iter()
            .all(|definition| !definition.identity().path().contains("Catalog"))
    );
    package.write(
        "main.omg",
        &format!("{source} pub machine forward() -> u64 in Issued {{ issue() }}"),
    );
    let diagnostics = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
    })
    .expect_err("a concrete admission claim is not a boundary-requirement receipt");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot establish call-result qualification")),
        "{diagnostics:?}"
    );
}
