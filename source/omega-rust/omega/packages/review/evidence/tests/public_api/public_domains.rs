use crate::support::*;

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
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    first.write("build.omg", build);
    second.write("build.omg", build);

    let encode = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
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
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
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
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    first.write("build.omg", build);
    second.write("build.omg", build);

    let encode = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
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
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    classified.write("build.omg", build);
    routed.write("build.omg", build);

    let compile = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
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
    assert_eq!(route.trait_identity().path(), "SchedulerAdmission");
    assert!(
        route
            .requirement_identity()
            .path()
            .starts_with("named-callable(")
    );
    assert!(
        route
            .requirement_identity()
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
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    first.write("build.omg", build);
    second.write("build.omg", build);

    let encode = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
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
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    first.write("build.omg", build);
    second.write("build.omg", build);

    let compile = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
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
        &psi_language_semantics::CarryPermission::ALL.map(PackageReviewDomainAliasAtom::Carry)
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
