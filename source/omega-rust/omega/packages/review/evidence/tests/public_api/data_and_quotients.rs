use crate::support::*;

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

    let quotient = |review: &omega_package_evidence::record::CheckedPackageReviewProjection| {
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
