use crate::support::*;

fn assert_external_policy_round_trip(
    supply: &omega_package_evidence::record::PackageReviewExternalExecutableSupply,
) {
    let policy = supply.policy_projection();
    assert_eq!(policy.callable(), supply.callable());
    assert_eq!(policy.signature(), supply.signature());
    assert_eq!(policy.requirement(), supply.requirement());
    let bytes = policy
        .canonical_bytes()
        .expect("encode checked evaluated binding policy");
    let recovered =
        omega_package_evidence::record::PackagePolicyExternalExecutableSupply::recover_canonical(
            &bytes,
            omega_package_evidence::encoding::PackagePolicyRecoveryLimits::default(),
        )
        .expect("recover checked binding policy without source or native replay");
    assert_eq!(recovered, policy);
    assert_eq!(
        recovered
            .canonical_bytes()
            .expect("re-encode checked binding policy"),
        bytes
    );
}

const SOURCE: &str = r#"use omega::language::core::external_binding;

pub boundary trait ExternalSurface {
    machine named();
    machine ordinal();
}

pub windows_x86_64 machine named_binding() -> Binding<12, 11, 0> {
    Binding::DllImport {
        import: DllImport::PeByName {
            library: "kernel32.dll",
            export: "ExitProcess",
        },
    }
}

windows_x86_64 machine ordinal_binding() -> Binding<10, 0, 0> {
    Binding::DllImport {
        import: DllImport::PeByOrdinal {
            library: "user32.dll",
            ordinal: 7,
        },
    }
}

pub machine named_leaf()
    satisfies ExternalSurface::named
    via named_binding();
machine ordinal_leaf()
    satisfies ExternalSurface::ordinal
    via ordinal_binding();
"#;

const BUILD: &str = r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;

fn checked_fixture() -> CheckedCompilation {
    let package = TempPackage::new();
    package.write("main.omg", SOURCE);
    package.write("build.omg", BUILD);
    compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x86_64"),
        package_inputs(&package.0),
    )
    .unwrap_or_else(|diagnostics| {
        panic!(
            "ordinary evaluated-via package should check:\n{}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    })
}

#[test]
fn review_projects_all_package_owned_evaluated_via_leaves_with_exact_receipts() {
    let checked = checked_fixture();
    assert_eq!(checked.evaluated_via_bindings().rows().len(), 2);

    let review = project_checked_package_review(&checked)
        .expect("ordinary evaluated-via supply should project exactly");
    assert_eq!(review.external_executable_supply().len(), 2);

    let named = review
        .external_executable_supply()
        .iter()
        .find(|supply| supply.callable().path() == "named_leaf")
        .expect("public named-import leaf");
    let ordinal = review
        .external_executable_supply()
        .iter()
        .find(|supply| supply.callable().path() == "ordinal_leaf")
        .expect("private unselected ordinal-import leaf");
    assert!(
        review
            .callables()
            .iter()
            .any(|callable| callable.identity() == named.callable())
    );
    assert!(
        !review
            .callables()
            .iter()
            .any(|callable| callable.identity() == ordinal.callable())
    );

    let PackageReviewExternalBinding::NormalizedImport(named_import) = named.binding() else {
        panic!("named leaf must retain ordinary evaluated import identity")
    };
    assert_eq!(
        named_import.target(),
        "omega.target-profile.v1:windows_x86_64"
    );
    assert_eq!(
        named_import.locator(),
        &PackageReviewForeignLocator::PeByName {
            library: b"kernel32.dll".to_vec(),
            export: b"ExitProcess".to_vec(),
        }
    );
    assert_eq!(named_import.producer().path(), "named_binding");
    assert_eq!(named_import.producer_package(), Some(package_identity()));
    assert!(!named_import.producer_callable_identity().is_empty());
    assert_ne!(named_import.producer_closure_digest(), [0; 32]);
    assert_ne!(named_import.evaluation_digest(), [0; 32]);
    assert_ne!(named_import.materialization_digest(), [0; 32]);
    assert_ne!(named_import.receipt_identity_digest(), [0; 32]);
    assert_eq!(
        named_import.receipt_locator_identity_digest(),
        named_import.locator_identity_digest()
    );
    assert!(named_import.evaluator_semantics_marker() > 0);
    assert!(named_import.materializer_schema_version() > 0);
    assert!(named_import.evaluation_usage().fuel_ceiling() > 0);

    let PackageReviewExternalBinding::NormalizedImport(ordinal_import) = ordinal.binding() else {
        panic!("ordinal leaf must retain ordinary evaluated import identity")
    };
    assert_eq!(
        ordinal_import.locator(),
        &PackageReviewForeignLocator::PeByOrdinal {
            library: b"user32.dll".to_vec(),
            ordinal: 7,
        }
    );
    assert_eq!(ordinal_import.producer().path(), "ordinal_binding");
    assert_external_policy_round_trip(named);
    assert_external_policy_round_trip(ordinal);

    let rows = review
        .canonical_rows()
        .expect("evaluated-via canonical rows");
    let supply_rows = rows
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::ExternalExecutableSupply)
        .collect::<Vec<_>>();
    assert_eq!(supply_rows.len(), 2);
    assert!(supply_rows.iter().all(|row| {
        row.risk() == PackageReviewCanonicalRowRisk::OpaqueBlocking
            && row.source().authored_locations().is_some_and(|locations| {
                locations.iter().any(|location| {
                    location.role() == PackageReviewSourceLocationRole::ExternalBinding
                        && location.relative_path() == "main.omg"
                })
            })
    }));
    assert_ne!(
        supply_rows[0].canonical_bytes(),
        supply_rows[1].canonical_bytes()
    );
    for row in supply_rows {
        let encoded = encode_package_review_canonical_row(row)
            .expect("evaluated-via canonical row should encode");
        let decoded = decode_package_review_canonical_row(&encoded)
            .expect("evaluated-via canonical row should recover");
        assert_eq!(decoded.key_bytes(), row.key_bytes());
        assert_eq!(decoded.canonical_bytes(), row.canonical_bytes());
    }
}

#[test]
fn review_rejects_typed_via_mutation_after_evaluation() {
    let mut checked = checked_fixture();
    let conformance_span = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "named_leaf")
        .expect("named external leaf")
        .satisfies;
    checked
        .typed
        .machine_trait_conformances
        .span_mut_or_empty(conformance_span)[0]
        .via_expression = psi_typed_trees::expression::ExpressionHandle::invalid();

    let diagnostics = project_checked_package_review(&checked)
        .expect_err("retained evaluated table must not outlive mutated typed custody");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .to_string()
            .contains("evaluated `via` binding table retains 2 rows for 1 exact typed expressions")
    }));
}

#[test]
fn review_keeps_ordinary_syscall_receipt_distinct_from_legacy_syscall() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"use omega::language::core::external_binding;

pub boundary trait Process {
    machine exit(code: i32);
}

pub linux_x86_64 machine exit_binding() -> Binding<0, 0, 0> {
    Binding::Syscall { number: 60 }
}

pub machine exit_leaf(code: i32)
    satisfies Process::exit
    via exit_binding();
"#,
    );
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("linux_x86_64"),
        package_inputs(&package.0),
    )
    .unwrap_or_else(|diagnostics| panic!("ordinary syscall should check: {diagnostics:#?}"));
    let review = project_checked_package_review(&checked)
        .expect("ordinary syscall supply should project exactly");
    let [supply] = review.external_executable_supply() else {
        panic!("one reviewed ordinary syscall leaf expected");
    };
    let PackageReviewExternalBinding::NormalizedSyscall(syscall) = supply.binding() else {
        panic!("ordinary syscall must not project as a legacy syscall");
    };
    assert_eq!(syscall.target(), "omega.target-profile.v1:linux_x86_64");
    assert_eq!(syscall.number(), 60);
    assert_eq!(syscall.producer().path(), "exit_binding");
    assert_eq!(syscall.producer_package(), Some(package_identity()));
    assert_ne!(syscall.producer_closure_digest(), [0; 32]);
    assert_ne!(syscall.evaluation_digest(), [0; 32]);
    assert_ne!(syscall.materialization_digest(), [0; 32]);
    assert_ne!(syscall.receipt_identity_digest(), [0; 32]);
    assert_eq!(
        syscall.receipt_binding_identity_digest(),
        syscall.binding_identity_digest()
    );
    assert_external_policy_round_trip(supply);

    let rows = review.canonical_rows().expect("ordinary syscall rows");
    let row = rows
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::ExternalExecutableSupply)
        .expect("ordinary syscall canonical row");
    let encoded = encode_package_review_canonical_row(row).expect("encode ordinary syscall row");
    let decoded = decode_package_review_canonical_row(&encoded).expect("recover syscall row");
    assert_eq!(decoded.canonical_bytes(), row.canonical_bytes());
}
