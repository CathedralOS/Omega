mod support;

use support::*;

#[test]
fn public_data_and_numbered_wire_shape_changes_change_comparison_encoding() {
    let first = TempPackage::new();
    let second = TempPackage::new();
    first.write(
        "main.omg",
        "pub data Packet [copy] { #1 value: u32; }\ndata Private { ignored: u32; }\n",
    );
    second.write(
        "main.omg",
        "pub data Packet [copy] { #1 value: u64; }\ndata Private { changed: i64; }\n",
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
        .expect("public-shape fixture should check");
        project_checked_package_review(&checked)
            .expect("public-shape review should close")
            .canonical_review_bytes()
            .expect("public-shape encoding")
    };

    assert_ne!(encode(&first), encode(&second));
}

#[test]
fn public_quotient_identity_binds_carrier_and_relation_but_not_proof_implementation() {
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    let compile = |carrier: &str, relation: &str, evidence: &str, reverse_relation: bool| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &public_quotient_source(carrier, relation, evidence, reverse_relation),
        );
        package.write("build.omg", build);
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("public quotient fixture should check");
        project_checked_package_review(&checked).expect("public quotient review should close")
    };
    let original = compile("Representative", "equivalent", "FirstEvidence", false);
    let different_evidence = compile("Representative", "equivalent", "SecondEvidence", false);
    let different_carrier = compile(
        "AlternateRepresentative",
        "equivalent",
        "FirstEvidence",
        false,
    );
    let different_relation = compile("Representative", "same_bucket", "FirstEvidence", false);
    let different_relation_body = compile("Representative", "equivalent", "FirstEvidence", true);

    let quotient = |review: &omega_package_review::CheckedPackageReviewProjection| {
        review
            .public_data()
            .iter()
            .find(|shape| shape.identity().path() == "EquivalenceClass")
            .cloned()
            .expect("public quotient row")
    };
    let original_quotient = quotient(&original);
    let PackageReviewDataKind::Quotient { carrier, relation } = original_quotient.kind() else {
        panic!("EquivalenceClass must project as quotient data")
    };
    assert!(!carrier.canonical().is_empty());
    assert_eq!(relation.path(), "equivalent");
    assert_eq!(
        original_quotient,
        quotient(&different_evidence),
        "switching one valid equivalence proof implementation must not change public quotient identity"
    );
    assert_ne!(original_quotient, quotient(&different_carrier));
    assert_ne!(original_quotient, quotient(&different_relation));
    assert_eq!(
        original_quotient,
        quotient(&different_relation_body),
        "the relation declaration row, rather than a duplicate body, belongs in the quotient row"
    );
    assert_ne!(
        original
            .canonical_review_bytes()
            .expect("original quotient review bytes"),
        different_relation_body
            .canonical_review_bytes()
            .expect("changed relation review bytes"),
        "the public proposition row must bind a changed relation body"
    );
}

#[test]
fn public_quotient_review_rederives_formation_instead_of_trusting_typed_metadata() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        &public_quotient_source("Representative", "equivalent", "Evidence", false),
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let mut checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("public quotient fixture should check");
    let evidence_symbol = checked
        .conformances()
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|alias| alias.as_str() == "Evidence")
        })
        .map(|conformance| conformance.symbol)
        .expect("selected quotient evidence conformance");
    assert!(checked.authored_declaration_selections().iter().any(|selection| {
        selection.kind()
            == psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Conformance
            && selection.exposure()
                == psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation
            && matches!(
                selection.target(),
                psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target)
                    if target.selected_symbol() == evidence_symbol
            )
    }));
    checked
        .typed
        .tables
        .data_definitions
        .for_each_mut(|_, definition| {
            if definition.name.as_str() == "EquivalenceClass" {
                definition
                    .quotient
                    .as_mut()
                    .expect("quotient metadata")
                    .relation_symbol = psi_symbols::SymbolHandle::invalid();
            }
        });

    let diagnostics = project_checked_package_review(&checked)
        .expect_err("malformed retained quotient metadata must fail independent formation");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("must resolve to one exact proposition family")),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn public_quotient_package_compilation_requires_a_public_relation() {
    let package = TempPackage::new();
    let source = public_quotient_source("Representative", "equivalent", "Evidence", false)
        .replacen("pub proposition equivalent", "proposition equivalent", 1);
    package.write("main.omg", &source);
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let diagnostics = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect_err("a public quotient cannot omit its relation semantics from review");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("public interface selects private proposition `equivalent`")),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

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

#[test]
fn public_trait_shape_retains_boundary_parent_and_alpha_normalized_requirements() {
    let first = TempPackage::new();
    let second = TempPackage::new();
    let first_source = r#"pub trait Parent<Element> {
    operator < compare(left: Element, right: Element) -> bool;
}
pub boundary trait Service<Element>: Parent<Element> {
    machine Self::exchange(&mut self, item: Element) -> Element;
}
"#;
    first.write("main.omg", first_source);
    second.write(
        "main.omg",
        r#"pub trait Parent<Value> {
    operator < compare(left: Value, right: Value) -> bool;
}
pub boundary trait Service<Value>: Parent<Value> {
    machine Self::exchange(&mut self, item: Value) -> Value;
}
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
        .expect("public-trait fixture should check");
        project_checked_package_review(&checked).expect("public-trait review should close")
    };
    let first_review = compile(&first);
    let parent_shape = first_review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Parent")
        .expect("parent trait row");
    let [compare] = parent_shape.requirements() else {
        panic!("one fixed-operator requirement")
    };
    assert_eq!(
        compare.spelling(),
        Some(psi_language_core::OperatorSpelling::Less)
    );
    let service = first_review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Service")
        .expect("service trait row");
    assert!(service.is_boundary());
    assert_eq!(service.type_parameters().len(), 1);
    let [parent] = service.parents() else {
        panic!("one exact parent edge")
    };
    assert_eq!(
        parent.kind(),
        psi_typed_trees::trait_definition::TraitCompositionKind::Policy
    );
    assert_eq!(parent.identity().path(), "Parent");
    assert_eq!(parent.arguments().len(), 1);
    let canonical_rows = first_review
        .canonical_rows()
        .expect("public-trait canonical rows");
    let service_row = canonical_rows
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::PublicTrait)
        .find(|row| {
            row.source().authored_locations().is_some_and(|locations| {
                locations.iter().any(|location| {
                    let start = usize::try_from(location.start_byte()).unwrap();
                    let end = usize::try_from(location.end_byte()).unwrap();
                    location.role() == PackageReviewSourceLocationRole::Declaration
                        && &first_source[start..end] == "Service"
                })
            })
        })
        .expect("service canonical row");
    let locations = service_row
        .source()
        .authored_locations()
        .expect("service declaration and parent source");
    assert!(locations.iter().any(|location| {
        let start = usize::try_from(location.start_byte()).unwrap();
        let end = usize::try_from(location.end_byte()).unwrap();
        location.role() == PackageReviewSourceLocationRole::TraitParent
            && &first_source[start..end] == "Parent"
    }));
    let recovered_service_row = decode_package_review_canonical_row(
        &encode_package_review_canonical_row(service_row).expect("encode service review row"),
    )
    .expect("recover service review row");
    assert!(
        recovered_service_row
            .source()
            .authored_locations()
            .is_some_and(|locations| locations.iter().any(|location| {
                let start = usize::try_from(location.start_byte()).unwrap();
                let end = usize::try_from(location.end_byte()).unwrap();
                location.role() == PackageReviewSourceLocationRole::TraitParent
                    && &first_source[start..end] == "Parent"
            }))
    );
    let [exchange] = service.requirements() else {
        panic!("one exact requirement row")
    };
    assert!(exchange.identity().path().starts_with("named-callable("));
    assert!(exchange.identity().path().contains("Service::exchange"));
    assert!(exchange.spelling().is_none());
    assert!(exchange.type_parameters().is_empty());
    let [receiver, item] = exchange.parameters() else {
        panic!("receiver and item parameters")
    };
    assert!(receiver.is_self());
    assert!(receiver.is_mutable());
    assert!(!receiver.is_const());
    assert!(receiver.type_identity().canonical().contains("trait-self"));
    assert_eq!(item.name(), "item");
    assert!(!item.is_self());
    assert_eq!(
        item.type_identity().canonical(),
        exchange.return_type().canonical()
    );

    assert_eq!(
        first_review
            .canonical_review_bytes()
            .expect("first public-trait encoding"),
        compile(&second)
            .canonical_review_bytes()
            .expect("renamed-binder public-trait encoding")
    );
}

#[test]
fn public_lifetime_contracts_are_alpha_normalized_and_relationship_sensitive() {
    let first = TempPackage::new();
    let renamed = TempPackage::new();
    let changed = TempPackage::new();
    first.write(
        "main.omg",
        r#"pub data View<'left, 'right> {
    first: &'left [u8];
    second: &'right [u8];
}
pub trait Parent<'source> {
    machine borrow<'temporary>(source: &'source [u8], temporary: &'temporary [u8]) -> &'source [u8];
}
pub trait Child<'child>: Parent<'child> { }
"#,
    );
    renamed.write(
        "main.omg",
        r#"pub data View<'primary, 'secondary> {
    first: &'primary [u8];
    second: &'secondary [u8];
}
pub trait Parent<'origin> {
    machine borrow<'scratch>(source: &'origin [u8], temporary: &'scratch [u8]) -> &'origin [u8];
}
pub trait Child<'region>: Parent<'region> { }
"#,
    );
    changed.write(
        "main.omg",
        r#"pub data View<'left, 'right> {
    first: &'left [u8];
    second: &'left [u8];
}
pub trait Parent<'source> {
    machine borrow<'temporary>(source: &'source [u8], temporary: &'temporary [u8]) -> &'temporary [u8];
}
pub trait Child<'child>: Parent<'child> { }
"#,
    );
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    first.write("build.omg", build);
    renamed.write("build.omg", build);
    changed.write("build.omg", build);

    let compile = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("public lifetime fixture should check");
        project_checked_package_review(&checked).expect("public lifetime review should close")
    };
    let first_review = compile(&first);
    let view = first_review
        .public_data()
        .iter()
        .find(|shape| shape.identity().path() == "View")
        .expect("view data row");
    assert_eq!(view.lifetime_parameter_count(), 2);
    let [
        PackageReviewDataMember::Field(first_field),
        PackageReviewDataMember::Field(second_field),
    ] = view.members()
    else {
        panic!("two view fields")
    };
    assert_ne!(
        first_field.type_identity().canonical(),
        second_field.type_identity().canonical()
    );

    let parent = first_review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Parent")
        .expect("parent trait row");
    assert_eq!(parent.lifetime_parameter_count(), 1);
    let [borrow] = parent.requirements() else {
        panic!("borrow requirement")
    };
    assert_eq!(borrow.lifetime_parameter_count(), 1);
    let [source, temporary] = borrow.parameters() else {
        panic!("borrow parameters")
    };
    assert_ne!(
        source.type_identity().canonical(),
        temporary.type_identity().canonical()
    );
    assert_eq!(
        source.type_identity().canonical(),
        borrow.return_type().canonical()
    );

    let child = first_review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Child")
        .expect("child trait row");
    let [parent_edge] = child.parents() else {
        panic!("parent edge")
    };
    assert_eq!(parent_edge.lifetime_arguments(), &[0]);

    let first_bytes = first_review
        .canonical_review_bytes()
        .expect("first lifetime encoding");
    assert_eq!(
        first_bytes,
        compile(&renamed)
            .canonical_review_bytes()
            .expect("renamed lifetime encoding")
    );
    assert_ne!(
        first_bytes,
        compile(&changed)
            .canonical_review_bytes()
            .expect("changed lifetime encoding")
    );
}

#[test]
fn public_trait_lifetime_declarations_validate_before_review() {
    for (source, expected) in [
        (
            "pub trait Parent<'left, 'right> { }\npub trait Child<'child>: Parent<'child> { }\n",
            "expects 2 lifetime argument(s), got 1",
        ),
        (
            "pub trait Parent<'source> { }\npub trait Child<'child>: Parent<'ghost> { }\n",
            "uses undeclared lifetime argument `'ghost'",
        ),
        (
            "pub trait Parent<'source> { machine borrow<'source>(value: &'source [u8]) -> &'source [u8]; }\n",
            "redeclares inherited lifetime `'source'",
        ),
    ] {
        let package = TempPackage::new();
        package.write("main.omg", source);
        package.write(
            "build.omg",
            "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
        );
        let diagnostics = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect_err("invalid parent lifetime application must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "missing `{expected}` in {diagnostics:#?}"
        );
    }
}

#[test]
fn review_projects_public_data_invariants_from_exact_checked_rows() {
    let compile = |facts: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"pub data Ledger
where
{facts}
{{
    len: u32;
    count: u32;
}}
"#
            ),
        );
        package.write(
            "build.omg",
            "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("public data invariant fixture should check");
        project_checked_package_review(&checked)
            .expect("review should project the checked public data invariant")
    };

    let review = compile("    count <= len,");
    let [data] = review.public_data() else {
        panic!("one public data row")
    };
    let [
        PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
            meaning,
            operator,
            left,
            right,
        }),
    ] = data.invariants()
    else {
        panic!("one binary data invariant")
    };
    assert_eq!(meaning, &PackageReviewContractOperatorMeaning::Builtin);
    assert_eq!(*operator, PackageReviewContractBinaryOperator::LessOrEqual);
    for (expression, expected) in [
        (left.as_ref(), "Ledger::count"),
        (right.as_ref(), "Ledger::len"),
    ] {
        let PackageReviewContractExpression::Member {
            receiver,
            member,
            case_variant,
        } = expression
        else {
            panic!("data-subject field")
        };
        assert_eq!(
            receiver.as_ref(),
            &PackageReviewContractExpression::DomainSubject
        );
        assert_eq!(member.path(), expected);
        assert!(case_variant.is_none());
    }
    assert_ne!(
        review.canonical_review_bytes().unwrap(),
        compile("    count < len,")
            .canonical_review_bytes()
            .unwrap(),
        "changing a public data invariant must change canonical package identity"
    );
    assert_eq!(
        review.canonical_review_bytes().unwrap(),
        compile("    count <= len,\n    count <= len,")
            .canonical_review_bytes()
            .unwrap(),
        "duplicate invariant observations must normalize to one canonical fact"
    );
    assert_eq!(
        compile("    count <= len,\n    count <= 8,")
            .canonical_review_bytes()
            .unwrap(),
        compile("    count <= 8,\n    count <= len,")
            .canonical_review_bytes()
            .unwrap(),
        "authored invariant order must not change canonical package identity"
    );
}

#[test]
fn public_data_invariants_keep_generic_binders_distinct_from_fields() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Buffer<const N: u64>
where N <= 8,
{
    used: u64;
}
"#,
    );
    package.write(
        "build.omg",
        "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("generic data invariant should check");
    let review = project_checked_package_review(&checked)
        .expect("generic data invariant should retain its binder identity");
    let [data] = review.public_data() else {
        panic!("one public data row")
    };
    let [
        PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
            left, ..
        }),
    ] = data.invariants()
    else {
        panic!("one generic data invariant")
    };
    assert_eq!(
        left.as_ref(),
        &PackageReviewContractExpression::GenericBinder(0)
    );
}

#[test]
fn public_data_membership_invariants_keep_exact_field_and_domain_identity() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub domain u32::Small
requires self <= 8;

pub data Counter
where count in u32::Small,
{
    count: u32;
}
"#,
    );
    package.write(
        "build.omg",
        "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("data membership invariant should check");
    let review = project_checked_package_review(&checked)
        .expect("data membership invariant should retain exact identities");
    let [data] = review.public_data() else {
        panic!("one public data row")
    };
    let [PackageReviewContractFact::Membership { value, domain }] = data.invariants() else {
        panic!("one membership invariant")
    };
    let PackageReviewContractExpression::Member {
        receiver, member, ..
    } = value
    else {
        panic!("membership value projects the data field")
    };
    assert_eq!(
        receiver.as_ref(),
        &PackageReviewContractExpression::DomainSubject
    );
    assert_eq!(member.path(), "Counter::count");
    assert_eq!(domain.path(), "u32::Small");
}

#[test]
fn public_data_invariant_review_rejects_checked_ownership_spoofs() {
    let compile = || {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            r#"pub data Ledger
where count <= len,
{
    len: u32;
    count: u32;
}
"#,
        );
        package.write(
            "build.omg",
            "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
        );
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("public data ownership fixture should check")
    };
    let assert_rejects = |checked: &_, expected: &str| {
        let diagnostics = project_checked_package_review(checked)
            .expect_err("spoofed checked data ownership must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "missing `{expected}` in {diagnostics:#?}"
        );
    };

    let mut missing_owner = compile();
    let owner = missing_owner
        .facts
        .semantic
        .data_definition_facts
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("data ownership record");
    assert!(
        missing_owner
            .facts
            .semantic
            .data_definition_facts
            .free(owner)
    );
    assert_rejects(&missing_owner, "data invariant evidence");

    let mut duplicate_owner = compile();
    let owner = duplicate_owner
        .facts
        .semantic
        .data_definition_facts
        .iter()
        .next()
        .map(|(_, record)| record.clone())
        .expect("data ownership record");
    duplicate_owner
        .facts
        .semantic
        .data_definition_facts
        .append(owner);
    assert_rejects(&duplicate_owner, "data invariant evidence");

    let mut unrelated_extra_owner = compile();
    let mut owner = unrelated_extra_owner
        .facts
        .semantic
        .data_definition_facts
        .iter()
        .next()
        .map(|(_, record)| record.clone())
        .expect("data ownership record");
    owner.semantic_fact = Default::default();
    unrelated_extra_owner
        .facts
        .semantic
        .data_definition_facts
        .append(owner);
    assert_rejects(&unrelated_extra_owner, "data invariant evidence");

    let mut wrong_origin = compile();
    let semantic_fact = wrong_origin
        .facts
        .semantic
        .data_definition_facts
        .iter()
        .next()
        .map(|(_, record)| record.semantic_fact)
        .expect("data semantic fact");
    wrong_origin
        .facts
        .semantic
        .facts
        .get_mut(semantic_fact)
        .origin = psi_facts::FactOrigin::Unknown;
    assert_rejects(&wrong_origin, "data invariant evidence");

    let mut missing_dependency = compile();
    let owner = missing_dependency
        .facts
        .semantic
        .data_definition_facts
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("data ownership record");
    missing_dependency
        .facts
        .semantic
        .data_definition_facts
        .get_mut(owner)
        .dependencies
        .clear();
    assert_rejects(&missing_dependency, "data invariant evidence");

    let mut extra_dependency = compile();
    let owner = extra_dependency
        .facts
        .semantic
        .data_definition_facts
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("data ownership record");
    let dependency = extra_dependency
        .facts
        .semantic
        .data_definition_facts
        .get(owner)
        .dependencies[0];
    extra_dependency
        .facts
        .semantic
        .data_definition_facts
        .get_mut(owner)
        .dependencies
        .push(dependency);
    assert_rejects(&extra_dependency, "data invariant evidence");

    let mut orphan_semantic_fact = compile();
    let semantic_fact = orphan_semantic_fact
        .facts
        .semantic
        .data_definition_facts
        .iter()
        .next()
        .map(|(_, record)| record.semantic_fact)
        .expect("data semantic fact");
    let fact = *orphan_semantic_fact.facts.semantic.facts.get(semantic_fact);
    orphan_semantic_fact.facts.semantic.facts.append(fact);
    assert_rejects(&orphan_semantic_fact, "data invariant evidence");

    let mut orphan_ref = compile();
    let semantic_fact = orphan_ref
        .facts
        .semantic
        .data_definition_facts
        .iter()
        .next()
        .map(|(_, record)| record.semantic_fact)
        .expect("data semantic fact");
    orphan_ref.facts.semantic.refs.append(psi_facts::FactRef {
        fact: semantic_fact,
    });
    assert_rejects(&orphan_ref, "data invariant evidence");

    let mut dangling_ref = compile();
    dangling_ref.facts.semantic.refs.append(psi_facts::FactRef {
        fact: psi_arena::Handle::from_parts(u32::MAX, 1),
    });
    assert_rejects(&dangling_ref, "data invariant evidence");

    let mut malformed_extra_context = compile();
    malformed_extra_context
        .facts
        .semantic
        .contexts
        .append(psi_facts::FactContext {
            point: psi_facts::ProgramPoint::Global,
            facts: psi_arena::HandleSpan::from_parts(psi_arena::Handle::from_parts(u32::MAX, 1), 1),
        });
    assert_rejects(&malformed_extra_context, "data invariant evidence");

    let mut missing_context = compile();
    let context = missing_context
        .facts
        .semantic
        .contexts
        .iter()
        .find_map(|(handle, context)| {
            matches!(context.point, psi_facts::ProgramPoint::Definition { .. }).then_some(handle)
        })
        .expect("data fact context");
    assert!(missing_context.facts.semantic.contexts.free(context));
    assert_rejects(&missing_context, "data invariant evidence");

    let mut missing_symbol_set = compile();
    let symbol_set = missing_symbol_set
        .facts
        .semantic
        .symbol_sets
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("data symbol fact set");
    assert!(
        missing_symbol_set
            .facts
            .semantic
            .symbol_sets
            .free(symbol_set)
    );
    assert_rejects(&missing_symbol_set, "data invariant evidence");

    let mut malformed_empty_path = {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            r#"pub data Buffer<const N: u64>
where N <= 8,
{
    used: u64;
}
"#,
        );
        package.write(
            "build.omg",
            "target windows_x64 { }\nmachine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
        );
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("generic data ownership fixture should check")
    };
    let binder_place = malformed_empty_path
        .facts
        .semantic
        .data_definition_facts
        .iter()
        .next()
        .map(|(_, record)| record.dependencies[0].place)
        .expect("generic binder dependency");
    assert!(
        malformed_empty_path
            .facts
            .semantic
            .places
            .get(binder_place)
            .segments
            .is_empty()
    );
    malformed_empty_path
        .facts
        .semantic
        .places
        .get_mut(binder_place)
        .segments =
        psi_arena::HandleSpan::from_parts(psi_arena::Handle::from_parts(u32::MAX, 1), 1);
    assert_rejects(&malformed_empty_path, "data invariant evidence");
}

#[test]
fn review_projects_public_domain_predicates_from_exact_checked_rows() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Packet { value: u32; }
pub domain Packet::Ready
    requires self.value == 0;
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("public domain fact fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("review should project the checked public domain predicate");
    let [domain] = review.public_domains() else {
        panic!("one public domain row")
    };
    assert_eq!(
        domain.predicate_body(),
        psi_language_semantics::DomainPredicateBody::Present
    );
    let [
        PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
            meaning,
            operator,
            left,
            right,
        }),
    ] = domain.predicate_facts()
    else {
        panic!("one binary domain predicate fact")
    };
    assert_eq!(meaning, &PackageReviewContractOperatorMeaning::Builtin);
    assert_eq!(*operator, PackageReviewContractBinaryOperator::Equal);
    let PackageReviewContractExpression::Member {
        receiver,
        member,
        case_variant,
    } = left.as_ref()
    else {
        panic!("domain-subject member path")
    };
    assert_eq!(
        receiver.as_ref(),
        &PackageReviewContractExpression::DomainSubject
    );
    assert_eq!(member.path(), "Packet::value");
    assert!(case_variant.is_none());
    assert_eq!(
        right.as_ref(),
        &PackageReviewContractExpression::Integer("0".to_owned())
    );
}
