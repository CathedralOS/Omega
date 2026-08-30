use crate::support::*;

#[test]
fn review_projects_named_witness_interfaces_through_transparent_aliases() {
    let Some(target) = host_target_name() else {
        return;
    };
    let direct = TempPackage::new();
    let aliased = TempPackage::new();
    let direct_source = r#"pub trait EvidenceBase<Element> {
    machine inherited(value: Element);
}
pub trait Evidence<Element>: EvidenceBase<Element> {
    machine witness(value: Element);
}
pub proposition carries<Element>(value: Element) evidence Evidence<Element>;
pub machine consume()
requires proof: carries<i32>(1)
{ }
"#;
    let aliased_source = r#"pub trait EvidenceBase<Element> {
    machine inherited(value: Element);
}
pub trait Evidence<Element>: EvidenceBase<Element> {
    machine witness(value: Element);
}
pub proposition carries<Element>(value: Element) evidence Evidence<Element>;
pub proposition forwarded<Item>(value: Item) = carries<Item>(value);
pub machine consume()
requires evidence: forwarded<i32>(1)
{ }
"#;
    let build = r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    direct.write("main.omg", direct_source);
    direct.write("build.omg", build);
    aliased.write("main.omg", aliased_source);
    aliased.write("build.omg", build);

    let compile = |package: &TempPackage| {
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("named witness fixture should check")
    };
    let direct_checked = compile(&direct);
    let direct_review =
        project_checked_package_review(&direct_checked).expect("direct witness review");
    let aliased_review =
        project_checked_package_review(&compile(&aliased)).expect("aliased witness review");
    let forwarded_row = aliased_review
        .canonical_rows()
        .expect("aliased proposition rows")
        .into_iter()
        .find(|row| {
            row.kind() == PackageReviewCanonicalRowKind::PublicProposition
                && row
                    .key_bytes()
                    .windows("forwarded".len())
                    .any(|window| window == b"forwarded")
        })
        .expect("transparent proposition application row");
    let forwarded_formula = forwarded_row
        .source()
        .authored_locations()
        .unwrap()
        .iter()
        .find(|location| location.role() == PackageReviewSourceLocationRole::PropositionFormula)
        .expect("transparent proposition application source");
    let start = usize::try_from(forwarded_formula.start_byte()).unwrap();
    let end = usize::try_from(forwarded_formula.end_byte()).unwrap();
    assert_eq!(&aliased_source[start..end], "carries<Item>(value)");
    let consume = direct_review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("consume"))
        .expect("public consumer");
    let [contract] = consume.contracts() else {
        panic!("one named witness contract")
    };
    let consume_row = direct_review
        .canonical_rows()
        .expect("named witness canonical rows")
        .into_iter()
        .find(|row| {
            row.kind() == PackageReviewCanonicalRowKind::Callable
                && row
                    .key_bytes()
                    .windows("consume".len())
                    .any(|window| window == b"consume")
        })
        .expect("named witness callable row");
    assert!(
        consume_row
            .source()
            .authored_locations()
            .is_some_and(|locations| locations.iter().any(|location| {
                let start = usize::try_from(location.start_byte()).unwrap();
                let end = usize::try_from(location.end_byte()).unwrap();
                location.role() == PackageReviewSourceLocationRole::ContractClause
                    && &direct_source[start..end] == "requires"
            }))
    );
    assert_eq!(
        contract.binding(),
        None,
        "a named requires spelling is a callee-local alias"
    );
    assert_eq!(contract.evidence_lane_position(), Some(0));
    let PackageReviewContractFact::Proposition(application) = contract.fact() else {
        panic!("witness proposition application")
    };
    assert_eq!(application.declaration().path(), "carries");
    let [binder_argument] = application.binder_arguments() else {
        panic!("one witness proposition type argument")
    };
    let PackageReviewPropositionBinderValue::Type(type_identity) = binder_argument.value() else {
        panic!("concrete proposition type argument must use structural type identity")
    };
    assert!(type_identity.canonical().contains("compiler-type"));
    let PackageReviewPropositionEvidence::Witness(interface) = application.evidence() else {
        panic!("witness interface")
    };
    assert_eq!(interface.trait_identity().path(), "Evidence");
    assert_eq!(interface.arguments().len(), 1);
    assert_eq!(interface.requirements().len(), 2);
    assert!(interface.requirements().iter().any(|requirement| {
        requirement.declaring_trait().path() == "Evidence"
            && requirement.requirement().path().contains("witness")
            && requirement.declaring_trait_arguments().len() == 1
    }));
    assert!(interface.requirements().iter().any(|requirement| {
        requirement.declaring_trait().path() == "EvidenceBase"
            && requirement.requirement().path().contains("inherited")
            && requirement.declaring_trait_arguments().len() == 1
    }));
    assert_ne!(
        direct_review
            .canonical_review_bytes()
            .expect("direct witness encoding"),
        aliased_review
            .canonical_review_bytes()
            .expect("aliased witness encoding"),
        "a published transparent alias is a distinct source API row even though contract semantic identity expands through it",
    );
    let direct_contract = direct_review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("consume"))
        .expect("direct public consumer")
        .contracts();
    let aliased_contract = aliased_review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("consume"))
        .expect("aliased public consumer")
        .contracts();
    assert_eq!(
        direct_contract, aliased_contract,
        "transparent alias expansion must preserve the consuming contract's semantic row"
    );

    let mut diagnostic_spoof = compile(&direct);
    let term_handles = diagnostic_spoof
        .facts
        .proof
        .evidence_terms
        .iter()
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    for handle in term_handles {
        let term = diagnostic_spoof.facts.proof.evidence_terms.get_mut(handle);
        term.evidence_type = "spoofed diagnostic evidence".to_owned();
        term.proposition
            .arguments
            .fill("spoofed argument".to_owned());
        for argument in &mut term.proposition.binder_arguments {
            argument.identity = "spoofed binder".to_owned();
        }
        if let Some(interface) = term.evidence_interface.as_mut() {
            interface.arguments.fill("spoofed interface".to_owned());
            for requirement in &mut interface.requirements {
                requirement
                    .declaring_trait_arguments
                    .fill("spoofed requirement".to_owned());
            }
        }
    }
    let spoofed_review = project_checked_package_review(&diagnostic_spoof)
        .expect("diagnostic strings are not review identity");
    assert_eq!(
        direct_review
            .canonical_review_bytes()
            .expect("structural witness encoding"),
        spoofed_review
            .canonical_review_bytes()
            .expect("spoofed diagnostic witness encoding"),
        "checked diagnostic strings must not influence package evidence",
    );
}

#[test]
fn named_evidence_lane_order_changes_canonical_review_identity() {
    let Some(target) = host_target_name() else {
        return;
    };
    let first = TempPackage::new();
    let second = TempPackage::new();
    let prefix = r#"pub trait Evidence {}
pub proposition left_fact() evidence Evidence;
pub proposition right_fact() evidence Evidence;
"#;
    first.write(
        "main.omg",
        &format!(
            "{prefix}pub machine consume()\nrequires left: left_fact()\nrequires right: right_fact()\n{{ }}\n"
        ),
    );
    second.write(
        "main.omg",
        &format!(
            "{prefix}pub machine consume()\nrequires right: right_fact()\nrequires left: left_fact()\n{{ }}\n"
        ),
    );
    let build = r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    first.write("build.omg", build);
    second.write("build.omg", build);
    let encode = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("named evidence lane fixture should check");
        project_checked_package_review(&checked)
            .expect("named evidence lane review")
            .canonical_review_bytes()
            .expect("named evidence lane encoding")
    };
    assert_ne!(
        encode(&first),
        encode(&second),
        "reordering positional erased proof inputs must change package evidence",
    );
}

#[test]
fn review_projects_proof_static_evidence_members_by_lane_and_requirement() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let renamed = TempPackage::new();
    let source = |binding: &str| {
        format!(
            r#"pub trait EvidenceBase<Element> {{
    machine modulus() -> Element;
}}
pub trait Evidence<Element>: EvidenceBase<Element> {{
}}
pub proposition holds<Element>() evidence Evidence<Element>;
pub proposition selected<machine Witness>();
pub machine caller()
requires {binding}: holds<i32>()
requires selected<{binding}.modulus>()
{{ }}
"#
        )
    };
    original.write("main.omg", &source("proof"));
    renamed.write("main.omg", &source("evidence"));
    let build = r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    original.write("build.omg", build);
    renamed.write("build.omg", build);
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("proof-static projection fixture should check");
        project_checked_package_review(&checked).expect("proof-static projection review")
    };
    let original = project(&original);
    let renamed = project(&renamed);
    let caller = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("caller"))
        .expect("public caller");
    let selected = caller
        .contracts()
        .iter()
        .find_map(|contract| {
            let PackageReviewContractFact::Proposition(application) = contract.fact() else {
                return None;
            };
            (application.declaration().path() == "selected").then_some(application)
        })
        .expect("selected proposition row");
    let holds = caller
        .contracts()
        .iter()
        .find_map(|contract| {
            let PackageReviewContractFact::Proposition(application) = contract.fact() else {
                return None;
            };
            (application.declaration().path() == "holds").then_some(application)
        })
        .expect("source witness proposition row");
    let [argument] = selected.binder_arguments() else {
        panic!("one projected static machine argument")
    };
    let PackageReviewPropositionBinderValue::EvidenceProjection {
        source_kind,
        source_lane_position,
        declaring_trait,
        declaring_trait_arguments,
        requirement,
    } = argument.value()
    else {
        panic!("exact proof-static evidence projection")
    };
    assert_eq!(*source_kind, PackageReviewContractKind::Requires);
    assert_eq!(*source_lane_position, 0);
    assert_eq!(declaring_trait.path(), "EvidenceBase");
    assert!(requirement.path().contains("modulus"));
    let PackageReviewPropositionEvidence::Witness(source_interface) = holds.evidence() else {
        panic!("source witness interface")
    };
    let source_requirement = source_interface
        .requirements()
        .iter()
        .find(|candidate| candidate.requirement() == requirement)
        .expect("inherited source requirement");
    assert_eq!(
        declaring_trait_arguments,
        source_requirement.declaring_trait_arguments(),
        "the projection must retain the exact inherited requirement template anchored by the source lane",
    );
    assert_eq!(
        original
            .canonical_review_bytes()
            .expect("original proof-static encoding"),
        renamed
            .canonical_review_bytes()
            .expect("renamed proof-static encoding"),
        "renaming the local evidence term must not alter its lane-based package identity",
    );
}
