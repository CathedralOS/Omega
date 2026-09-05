use crate::support::*;

#[test]
fn package_callable_wins_over_compiler_byte_predicate_spelling_in_review() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub machine valid_utf8(value: &[u8]) -> bool { true }
pub domain [u8]::ReviewedBytes
requires
    valid_utf8(self);
"#,
    );
    package.write(
        "build.omg",
        "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x86_64"),
        package_inputs(&package.0),
    )
    .expect("package callable lookalike should check as an ordinary call");
    let review = project_checked_package_review(&checked)
        .expect("package callable lookalike should retain nominal identity");
    let [domain] = review.public_domains() else {
        panic!("one byte-domain row")
    };
    let [
        PackageReviewContractFact::Expression(PackageReviewContractExpression::Call {
            target, ..
        }),
    ] = domain.predicate_facts()
    else {
        panic!("one nominal predicate call")
    };
    let nominal = target
        .nominal()
        .expect("package declaration must remain nominal");
    assert_eq!(nominal.path(), "valid_utf8::entry");
    assert_eq!(
        nominal.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
}

#[test]
fn public_domain_predicate_review_rejects_checked_owner_and_dependency_spoofs() {
    let compile = || {
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
            "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
        );
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x86_64"),
            package_inputs(&package.0),
        )
        .expect("public domain spoof fixture should check")
    };
    let assert_rejects = |checked: &_, expected: &str| {
        let diagnostics = project_checked_package_review(checked)
            .expect_err("spoofed checked domain ownership must reject");
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
        .domain_definition_facts
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("domain ownership record");
    assert!(
        missing_owner
            .facts
            .semantic
            .domain_definition_facts
            .free(owner)
    );
    assert_rejects(&missing_owner, "0 exact checked ownership records");

    let mut duplicate_owner = compile();
    let owner = duplicate_owner
        .facts
        .semantic
        .domain_definition_facts
        .iter()
        .next()
        .map(|(_, record)| record.clone())
        .expect("domain ownership record");
    duplicate_owner
        .facts
        .semantic
        .domain_definition_facts
        .append(owner);
    assert_rejects(&duplicate_owner, "2 exact checked ownership records");

    let mut wrong_origin = compile();
    let semantic_fact = wrong_origin
        .facts
        .semantic
        .domain_definition_facts
        .iter()
        .next()
        .map(|(_, record)| record.semantic_fact)
        .expect("domain semantic fact");
    wrong_origin
        .facts
        .semantic
        .facts
        .get_mut(semantic_fact)
        .origin = facts::FactOrigin::Unknown;
    assert_rejects(&wrong_origin, "0 exact checked definition rows");

    let mut false_evidence = compile();
    let semantic_fact = false_evidence
        .facts
        .semantic
        .domain_definition_facts
        .iter()
        .next()
        .map(|(_, record)| record.semantic_fact)
        .expect("domain semantic fact");
    false_evidence
        .facts
        .semantic
        .facts
        .get_mut(semantic_fact)
        .evidence
        .receipt_identity = 1;
    assert_rejects(&false_evidence, "0 exact checked definition rows");

    let mut missing_dependency = compile();
    let owner = missing_dependency
        .facts
        .semantic
        .domain_definition_facts
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("domain ownership record");
    missing_dependency
        .facts
        .semantic
        .domain_definition_facts
        .get_mut(owner)
        .dependencies
        .clear();
    assert_rejects(&missing_dependency, "0 exact checked dependency records");

    let mut duplicate_dependency = compile();
    let owner = duplicate_dependency
        .facts
        .semantic
        .domain_definition_facts
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("domain ownership record");
    let dependency = duplicate_dependency
        .facts
        .semantic
        .domain_definition_facts
        .get(owner)
        .dependencies[0];
    duplicate_dependency
        .facts
        .semantic
        .domain_definition_facts
        .get_mut(owner)
        .dependencies
        .push(dependency);
    assert_rejects(&duplicate_dependency, "2 exact checked dependency records");

    let mut wrong_member = compile();
    let dependency = wrong_member
        .facts
        .semantic
        .domain_definition_facts
        .iter()
        .next()
        .and_then(|(_, record)| record.dependencies.first().copied())
        .expect("domain member dependency");
    let segment = wrong_member
        .facts
        .semantic
        .places
        .get(dependency.place)
        .segments
        .start();
    wrong_member
        .facts
        .semantic
        .place_segments
        .get_mut(segment)
        .clone_from(&facts::PlaceSegment::Field {
            symbol: symbols::SymbolHandle::invalid(),
        });
    assert_rejects(&wrong_member, "0 exact checked dependency records");
}

#[test]
fn review_projects_public_domain_membership_predicates_and_rejects_private_targets() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Packet { value: u32; }
pub domain Packet::Base;
pub domain Packet::Ready
    requires self in Packet::Base;
"#,
    );
    package.write(
        "build.omg",
        "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x86_64"),
        package_inputs(&package.0),
    )
    .expect("public domain membership fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("public domain membership review should close");
    let ready = review
        .public_domains()
        .iter()
        .find(|domain| domain.identity().path() == "Packet::Ready")
        .expect("ready domain row");
    let [PackageReviewContractFact::Membership { value, domain }] = ready.predicate_facts() else {
        panic!("one exact membership predicate")
    };
    assert_eq!(value, &PackageReviewContractExpression::DomainSubject);
    assert_eq!(domain.path(), "Packet::Base");

    let private = TempPackage::new();
    private.write(
        "main.omg",
        r#"pub data Packet { value: u32; }
domain Packet::Base;
pub domain Packet::Ready
    requires self in Packet::Base;
"#,
    );
    private.write(
        "build.omg",
        "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    );
    let diagnostics = compile_to_checked_with_packages(
        &private.0.join("main.omg"),
        Some("windows_x86_64"),
        package_inputs(&private.0),
    )
    .expect_err("ordinary visibility must reject a private domain in a public predicate");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("public interface selects private domain `Packet::Base`")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn public_domain_predicate_fact_order_is_canonical_but_content_changes_encoding() {
    let first = TempPackage::new();
    let reordered = TempPackage::new();
    let changed = TempPackage::new();
    let source = |facts: &str| {
        format!(
            "pub data Packet {{ value: u32; }}\npub domain Packet::Ready\nrequires\n    {facts}\n"
        )
    };
    first.write("main.omg", &source("self.value == 0; self.value <= 1;"));
    reordered.write("main.omg", &source("self.value <= 1; self.value == 0;"));
    changed.write("main.omg", &source("self.value == 0; self.value <= 2;"));
    let build = "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n";
    first.write("build.omg", build);
    reordered.write("build.omg", build);
    changed.write("build.omg", build);

    let encode = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x86_64"),
            package_inputs(&package.0),
        )
        .expect("multi-fact public domain fixture should check");
        project_checked_package_review(&checked)
            .expect("multi-fact public domain review should close")
            .canonical_review_bytes()
            .expect("multi-fact public domain encoding")
    };
    assert_eq!(encode(&first), encode(&reordered));
    assert_ne!(encode(&first), encode(&changed));
}
