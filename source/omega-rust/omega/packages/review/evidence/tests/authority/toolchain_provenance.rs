use crate::support::*;

#[test]
fn dangerous_hardware_authorities_require_exact_toolchain_provenance() {
    let Some(target) = host_target_name() else {
        return;
    };

    let canonical = TempPackage::new();
    canonical.write(
        "main.omg",
        r#"use omega::language::core::interrupt;
use omega::language::core::extent;

pub machine exercise_hardware()
reaches MachineControl + PortIo + InterruptMaskControl + InterruptEntry + ExtentRootProvider
{
}
"#,
    );
    canonical.write(
        "build.omg",
        r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let canonical_checked = compile_to_checked_with_packages(
        &canonical.0.join("main.omg"),
        Some(target),
        package_inputs(&canonical.0),
    )
    .expect("canonical hardware-authority fixture should check");
    let canonical_review = project_checked_package_review(&canonical_checked)
        .expect("canonical hardware-authority review should close");
    let classes = canonical_review
        .dangerous_authorities()
        .iter()
        .map(|authority| authority.class())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        classes,
        std::collections::BTreeSet::from([
            PackageReviewDangerousAuthorityClass::MachineControl,
            PackageReviewDangerousAuthorityClass::PortIo,
            PackageReviewDangerousAuthorityClass::InterruptControl,
            PackageReviewDangerousAuthorityClass::InterruptEntry,
            PackageReviewDangerousAuthorityClass::RootMemory,
        ])
    );
    assert!(
        canonical_review
            .dangerous_authorities()
            .iter()
            .all(|authority| matches!(
                authority.service().owner(),
                PackageReviewNominalOwner::ToolchainSource(_)
            ))
    );
    let hardware = canonical_review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "exercise_hardware")
        .expect("hardware callable review");
    assert!(matches!(
        hardware.checked_service_reach(),
        PackageReviewCheckedServiceReach::CheckedBody {
            realized,
            concrete,
        } if realized.is_empty() && concrete.is_empty()
    ));
    let slack_classes = canonical_review
        .dangerous_authority_slack()
        .iter()
        .map(|slack| slack.class())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(slack_classes, classes);
    assert!(
        canonical_review
            .dangerous_authority_slack()
            .iter()
            .all(|slack| {
                slack.callable().path() == "exercise_hardware"
                    && matches!(
                        slack.service().owner(),
                        PackageReviewNominalOwner::ToolchainSource(_)
                    )
            })
    );
    let slack_rows = canonical_review
        .canonical_rows()
        .expect("hardware canonical rows")
        .into_iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::DangerousAuthoritySlack)
        .collect::<Vec<_>>();
    assert_eq!(slack_rows.len(), 5);
    assert!(slack_rows.iter().all(|row| {
        row.risk() == PackageReviewCanonicalRowRisk::AuditRecommended
            && row.source().authored_locations().is_some_and(|locations| {
                locations.iter().any(|location| {
                    location.role() == PackageReviewSourceLocationRole::AuthorityDeclaration
                }) && locations.iter().any(|location| {
                    location.role() == PackageReviewSourceLocationRole::AuthorityExposure
                })
            })
    }));

    let lookalike = TempPackage::new();
    lookalike.write(
        "main.omg",
        r#"pub boundary trait MachineControl {}
pub boundary trait PortIo {}
pub boundary trait InterruptMaskControl {}
pub boundary trait InterruptEntry {}
pub boundary trait ExtentRootProvider {}

pub machine exercise_hardware()
reaches MachineControl + PortIo + InterruptMaskControl + InterruptEntry + ExtentRootProvider
{
}
"#,
    );
    lookalike.write(
        "build.omg",
        r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let lookalike_checked = compile_to_checked_with_packages(
        &lookalike.0.join("main.omg"),
        Some(target),
        package_inputs(&lookalike.0),
    )
    .expect("package-owned hardware lookalikes should check as ordinary source");
    let lookalike_review = project_checked_package_review(&lookalike_checked)
        .expect("package-owned hardware-lookalike review should close");
    assert!(
        lookalike_review.dangerous_authorities().is_empty(),
        "package-controlled hardware names must not mint compiler-owned risk classes"
    );
    assert!(
        lookalike_review.dangerous_authority_slack().is_empty(),
        "package-controlled hardware names must not mint compiler-owned slack classes"
    );
}

#[test]
fn representation_tcb_retains_private_opaque_data_as_unbound() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write("main.omg", "boundary data InternalToken;\n");
    package.write(
        "build.omg",
        r#"target windows_x86_64 { }
target linux_x86_64 { }
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
    .expect("opaque representation fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("opaque representation review should close");
    let [row] = review.representation_tcb() else {
        panic!("one private representation-TCB row")
    };
    assert_eq!(row.declaration().path(), "InternalToken");
    assert_eq!(
        row.declaration().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(row.kind(), &PackageReviewRepresentationTcbKind::Unbound);
    assert!(
        review.public_data().is_empty(),
        "ordinary public API projection remains visibility-scoped"
    );

    let control = TempPackage::new();
    control.write("main.omg", "data InternalToken { }\n");
    control.write(
        "build.omg",
        r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let control_checked = compile_to_checked_with_packages(
        &control.0.join("main.omg"),
        Some(target),
        package_inputs(&control.0),
    )
    .expect("ordinary private representation fixture should check");
    let control_review = project_checked_package_review(&control_checked)
        .expect("ordinary private representation review should close");
    assert!(control_review.public_data().is_empty());
    assert!(control_review.representation_tcb().is_empty());
    assert_ne!(
        review
            .canonical_review_bytes()
            .expect("opaque review encoding"),
        control_review
            .canonical_review_bytes()
            .expect("ordinary review encoding"),
        "a private opaque representation-TCB row must enter comparison identity"
    );
}

#[test]
fn representation_tcb_publishes_public_producer_availability_without_selecting_it() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"use omega::language::core::representation;

pub boundary data PublicToken;
pub data PublicCarrier {
    value: u64;
}

pub PublicTokenRepresentation:
    PublicCarrier satisfies OpaqueRepresentation<PublicToken>;
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x86_64 { }
target linux_x86_64 { }
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
    .expect("public opaque-representation availability fixture should check");
    assert!(
        checked.opaque_representation_selections().is_empty(),
        "producer availability accepts no consumer selection"
    );
    assert!(
        checked
            .boundary_calling_plan_realizations()
            .iter()
            .all(|realization| realization
                .materialized_signature
                .opaque_representation_uses()
                .is_empty())
    );

    let review = project_checked_package_review(&checked)
        .expect("public opaque-representation availability review should close");
    let rows = review
        .representation_tcb()
        .iter()
        .filter(|row| row.declaration().path() == "PublicToken")
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .any(|row| row.kind() == &PackageReviewRepresentationTcbKind::Unbound)
    );
    let availability = rows
        .iter()
        .find_map(|row| match row.kind() {
            PackageReviewRepresentationTcbKind::ProducerAvailability {
                conformance,
                carrier,
            } => Some((conformance, carrier)),
            PackageReviewRepresentationTcbKind::Unbound
            | PackageReviewRepresentationTcbKind::SelectedCopyReceipt { .. } => None,
        })
        .expect("one producer-availability row");
    assert_eq!(availability.0.path(), "PublicTokenRepresentation");
    assert_eq!(availability.1.path(), "PublicCarrier");
    assert!(review.public_conformances().iter().any(|conformance| {
        conformance.identity() == availability.0
            && matches!(
                conformance.subject(),
                PackageReviewConformanceSubject::Nominal(carrier) if carrier == availability.1
            )
    }));
    assert_eq!(
        review
            .canonical_rows()
            .expect("representation availability canonical rows")
            .iter()
            .filter(|row| row.kind() == PackageReviewCanonicalRowKind::RepresentationTcb)
            .count(),
        2,
        "unbound policy and producer availability require distinct canonical keys"
    );
}

#[test]
fn copyable_opaque_retains_its_exact_selected_property_receipt() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"use omega::language::core::representation;

pub boundary data CopyToken [copy];
pub data CopyLeaf [copy] { value: u64; }
pub data CopyPayload [copy] { case Empty; case Value(value: CopyLeaf); }
pub data CopyCarrier [copy] { payloads: [CopyPayload; 2]; }
pub CopyTokenRepresentation:
    CopyCarrier satisfies OpaqueRepresentation<CopyToken>;
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) {
    builder.package("review-fixture");
    builder.select_representation<CopyToken, CopyTokenRepresentation>();
}
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("selected copyable opaque fixture should check");
    let [selection] = checked.opaque_representation_selections() else {
        panic!("one selected copyable opaque representation")
    };
    assert_eq!(
        selection.copy_disposition(),
        omega_representation_planning::OpaqueRepresentationCopyDisposition::CheckedSemanticCopy
    );
    assert!(
        checked
            .boundary_calling_plan_realizations()
            .iter()
            .all(|realization| realization
                .materialized_signature
                .opaque_representation_uses()
                .is_empty()),
        "an unused property receipt is not a D26 consumer demand"
    );

    let review = project_checked_package_review(&checked)
        .expect("selected copy receipt should project canonically");
    let rows = review
        .representation_tcb()
        .iter()
        .filter(|row| row.declaration().path() == "CopyToken")
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 3);
    let receipt = rows
        .iter()
        .find_map(|row| match row.kind() {
            PackageReviewRepresentationTcbKind::SelectedCopyReceipt {
                conformance,
                carrier,
                representation_schema_version,
                origin,
                lifecycle,
                copy_disposition,
                conformance_application_commitment,
                selected_application_commitment,
            } => Some((
                conformance,
                carrier,
                representation_schema_version,
                origin,
                lifecycle,
                copy_disposition,
                conformance_application_commitment,
                selected_application_commitment,
            )),
            PackageReviewRepresentationTcbKind::Unbound
            | PackageReviewRepresentationTcbKind::ProducerAvailability { .. } => None,
        })
        .expect("one selected opaque copy receipt row");
    assert_eq!(receipt.0.path(), "CopyTokenRepresentation");
    assert_eq!(receipt.1.path(), "CopyCarrier");
    assert_eq!(*receipt.2, selection.schema_version());
    assert_eq!(
        *receipt.3,
        PackageReviewOpaqueRepresentationApplicationOrigin::NamedConformance
    );
    assert_eq!(
        *receipt.4,
        PackageReviewOpaqueRepresentationLifecycleDisposition::Inert
    );
    assert_eq!(
        *receipt.5,
        PackageReviewOpaqueRepresentationCopyDisposition::CheckedSemanticCopy
    );
    assert_eq!(*receipt.6, selection.application().commitment.as_bytes());
    assert_eq!(*receipt.7, selection.selected_application_commitment());

    let canonical = review
        .canonical_rows()
        .expect("copy receipt canonical rows");
    let representation_rows = canonical
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::RepresentationTcb)
        .collect::<Vec<_>>();
    assert_eq!(representation_rows.len(), 3);
    let receipt_row = representation_rows
        .iter()
        .find(|row| {
            row.source().authored_locations().is_some_and(|locations| {
                locations.iter().any(|location| {
                    location.role() == PackageReviewSourceLocationRole::RepresentationSelection
                })
            })
        })
        .expect("copy receipt row retains exact build selection source");
    assert_eq!(
        receipt_row.risk(),
        PackageReviewCanonicalRowRisk::AuditRecommended
    );
    let envelope = encode_package_review_canonical_row(receipt_row)
        .expect("selected copy receipt canonical row should encode");
    let recovered = decode_package_review_canonical_row(&envelope)
        .expect("selected copy receipt canonical row should recover");
    assert_eq!(
        recovered.kind(),
        PackageReviewCanonicalRowKind::RepresentationTcb
    );
    assert!(
        recovered
            .source()
            .authored_locations()
            .is_some_and(|locations| {
                locations.iter().any(|location| {
                    location.role() == PackageReviewSourceLocationRole::RepresentationSelection
                })
            })
    );
}

#[test]
fn dependency_owned_opaque_copy_receipt_belongs_to_the_selecting_package() {
    let Some(target) = host_target_name() else {
        return;
    };
    let root = TempPackage::new();
    let dependency = TempPackage::new();
    root.write("main.omg", "machine root() {}\n");
    root.write(
        "build.omg",
        r#"use carrier::types;
target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) {
    builder.package("review-fixture");
    builder.select_representation<CopyToken, CopyTokenRepresentation>();
}
"#,
    );
    dependency.write(
        "types.omg",
        r#"use omega::language::core::representation;

pub boundary data CopyToken [copy];
pub data CopyCarrier [copy] { value: u64; }
pub CopyTokenRepresentation:
    CopyCarrier satisfies OpaqueRepresentation<CopyToken>;
"#,
    );

    let root_identity = package_identity();
    let dependency_identity =
        PackageKeyIdentity::from_digest([42; 32]).expect("dependency package identity");
    let inputs = PackageCompilationInputs::new_package(
        root_identity,
        vec![
            PackageSourceBinding::new(root_identity, "review-fixture", root.0.clone()),
            PackageSourceBinding::new(dependency_identity, "carrier-package", dependency.0.clone()),
        ],
        vec![PackageDependencyBinding::new(
            root_identity,
            "carrier",
            dependency_identity,
        )],
    )
    .expect("root and representation dependency graph should validate");
    let checked = compile_to_checked_with_packages(&root.0.join("main.omg"), Some(target), inputs)
        .expect("root should select a dependency-owned copyable opaque");
    let review = project_checked_package_review(&checked)
        .expect("the selecting package should retain the copy receipt");
    let receipt = review
        .representation_tcb()
        .iter()
        .find(|row| {
            row.declaration().owner() == PackageReviewNominalOwner::Package(dependency_identity)
                && matches!(
                    row.kind(),
                    PackageReviewRepresentationTcbKind::SelectedCopyReceipt { .. }
                )
        })
        .expect("dependency-owned opaque copy receipt in root review");
    assert_eq!(receipt.declaration().path(), "CopyToken");

    let canonical = review
        .canonical_rows()
        .expect("cross-package copy receipt canonical rows");
    let receipt_source = canonical
        .iter()
        .find(|row| {
            row.kind() == PackageReviewCanonicalRowKind::RepresentationTcb
                && row.source().authored_locations().is_some_and(|locations| {
                    locations.iter().any(|location| {
                        location.role() == PackageReviewSourceLocationRole::RepresentationSelection
                            && location.owner()
                                == PackageReviewSourceLocationOwner::Package(root_identity)
                    })
                })
        })
        .expect("root-owned selecting source for dependency representation receipt");
    assert!(
        receipt_source
            .source()
            .authored_locations()
            .is_some_and(|locations| locations.iter().any(|location| {
                location.role() == PackageReviewSourceLocationRole::Declaration
                    && location.owner()
                        == PackageReviewSourceLocationOwner::Package(dependency_identity)
            }))
    );
}
