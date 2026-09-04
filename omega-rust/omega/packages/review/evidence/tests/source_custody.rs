mod support;

use support::*;

#[test]
fn canonical_row_sorting_keeps_exact_declaration_sources_paired() {
    let Some(target) = host_target_name() else {
        return;
    };
    let source = "pub data Zed {}\npub data Alpha {}\n";
    let package = TempPackage::new();
    package.write("main.omg", source);
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("out-of-source-order declarations should check");
    let review = project_checked_package_review(&checked).expect("package review should close");
    let canonical_rows = review.canonical_rows().expect("canonical review rows");
    let data_rows = canonical_rows
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::PublicData)
        .collect::<Vec<_>>();
    assert_eq!(review.public_data().len(), 2);
    assert_eq!(data_rows.len(), 2);
    for row in data_rows {
        let [location] = row
            .source()
            .authored_locations()
            .expect("public data declaration source")
        else {
            panic!("one exact declaration source")
        };
        let start = usize::try_from(location.start_byte()).expect("source start fits usize");
        let end = usize::try_from(location.end_byte()).expect("source end fits usize");
        let declaration_name = &source[start..end];
        assert!(matches!(declaration_name, "Alpha" | "Zed"));
        assert!(
            row.key_bytes()
                .windows(declaration_name.len())
                .any(|window| window == declaration_name.as_bytes()),
            "the canonical row key and retained source must name the same exact declaration"
        );
    }
}

#[test]
fn callable_review_sources_join_all_authored_checked_body_call_forms() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    let source = r#"machine make() -> u64 { 1u64 }
machine consume(value: u64) { }
pub machine api() {
    consume(make());
    transition { _ -> done() }

    state done() { }
}
"#;
    package.write("main.omg", source);
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("checked body-call source fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("checked body-call sources should join package review");
    let api = review
        .canonical_rows()
        .expect("body-call canonical rows")
        .into_iter()
        .find(|row| {
            row.kind() == PackageReviewCanonicalRowKind::Callable
                && row
                    .key_bytes()
                    .windows("api".len())
                    .any(|window| window == b"api")
        })
        .expect("public api callable row");
    let mut body_call_text = api
        .source()
        .authored_locations()
        .expect("public api declaration and body calls")
        .iter()
        .filter(|location| location.role() == PackageReviewSourceLocationRole::BodyCall)
        .map(|location| {
            let start = usize::try_from(location.start_byte()).unwrap();
            let end = usize::try_from(location.end_byte()).unwrap();
            source[start..end].to_owned()
        })
        .collect::<Vec<_>>();
    body_call_text.sort();
    assert!(body_call_text.iter().any(|text| text == "consume"));
    assert!(body_call_text.iter().any(|text| text == "make"));
    assert!(body_call_text.iter().any(|text| text == "done"));
    let recovered = decode_package_review_canonical_row(
        &encode_package_review_canonical_row(&api).expect("encode body-call source row"),
    )
    .expect("recover body-call source row");
    assert!(
        recovered
            .source()
            .authored_locations()
            .is_some_and(|locations| locations
                .iter()
                .any(|location| { location.role() == PackageReviewSourceLocationRole::BodyCall }))
    );
}

#[test]
fn carried_transitive_types_project_exact_package_qualified_dependency_rows() {
    let Some(target) = host_target_name() else {
        return;
    };
    let root = TempPackage::new();
    let middle = TempPackage::new();
    let leaf = TempPackage::new();
    root.write(
        "main.omg",
        "use middle::middle;\nmachine relay() { consume(make()); }\n",
    );
    root.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    middle.write(
        "middle.omg",
        r#"use leaf::leaf;
pub machine make() -> Token { Token { value: 7u64 } }
pub machine consume(value: Token) {}
"#,
    );
    leaf.write("leaf.omg", "pub data Token { value: u64; }\n");

    let root_identity = package_identity();
    let middle_identity =
        PackageKeyIdentity::from_digest([42; 32]).expect("middle package identity");
    let leaf_identity = PackageKeyIdentity::from_digest([43; 32]).expect("leaf package identity");
    let inputs = PackageCompilationInputs::new_package(
        root_identity,
        vec![
            PackageSourceBinding::new(root_identity, "root", root.0.clone()),
            PackageSourceBinding::new(middle_identity, "middle", middle.0.clone()),
            PackageSourceBinding::new(leaf_identity, "leaf", leaf.0.clone()),
        ],
        vec![
            PackageDependencyBinding::new(root_identity, "middle", middle_identity),
            PackageDependencyBinding::new(middle_identity, "leaf", leaf_identity),
        ],
    )
    .expect("transitive package graph should validate");
    let checked = compile_to_checked_with_packages(&root.0.join("main.omg"), Some(target), inputs)
        .expect("carried transitive type should check without direct leaf authority");
    let review = project_checked_package_review(&checked).expect("semantic dependency review");

    for kind in [
        PackageReviewSemanticDependencyKind::NominalIdentity,
        PackageReviewSemanticDependencyKind::Layout,
        PackageReviewSemanticDependencyKind::OwnershipBehavior,
    ] {
        let dependency = review
            .semantic_dependencies()
            .iter()
            .find(|dependency| {
                dependency.consumer().path() == "relay"
                    && dependency.dependency().path() == "Token"
                    && dependency.dependency().owner()
                        == PackageReviewNominalOwner::Package(leaf_identity)
                    && dependency.exposure()
                        == PackageReviewSemanticDependencyExposure::PrivateImplementation
                    && dependency.kind() == kind
            })
            .unwrap_or_else(|| panic!("missing leaf-owned {kind:?} row"));
        assert_eq!(
            dependency.consumer().owner(),
            PackageReviewNominalOwner::Package(root_identity)
        );
    }

    let canonical = review
        .canonical_rows()
        .expect("canonical semantic dependency rows");
    let rows = canonical
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::SemanticDependency)
        .filter(|row| {
            row.source().authored_locations().is_some_and(|locations| {
                locations.iter().any(|location| {
                    location.role()
                        == PackageReviewSourceLocationRole::SemanticDependencyDeclaration
                        && location.owner()
                            == PackageReviewSourceLocationOwner::Package(leaf_identity)
                        && location.relative_path() == "leaf.omg"
                })
            })
        })
        .collect::<Vec<_>>();
    assert!(rows.len() >= 3);
    for row in rows {
        assert_eq!(row.risk(), PackageReviewCanonicalRowRisk::Blocking);
        let locations = row
            .source()
            .authored_locations()
            .expect("semantic dependency row source");
        assert!(locations.iter().any(|location| {
            location.role() == PackageReviewSourceLocationRole::SemanticDependencyConsumer
                && location.owner() == PackageReviewSourceLocationOwner::Package(root_identity)
                && location.relative_path() == "main.omg"
        }));
        assert!(locations.iter().any(|location| {
            location.role() == PackageReviewSourceLocationRole::SemanticDependencyDeclaration
                && location.owner() == PackageReviewSourceLocationOwner::Package(leaf_identity)
                && location.relative_path() == "leaf.omg"
        }));
    }
}

#[test]
fn semantic_dependency_projection_rejects_retained_evidence_drift() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Token { value: u64; }
pub machine make() -> Token { Token { value: 7u64 } }
"#,
    );
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("semantic dependency evidence fixture should check");
    assert!(
        !checked.facts.flow.semantic_dependencies.rows.is_empty(),
        "fixture must carry at least one semantic dependency"
    );
    project_checked_package_review(&checked).expect("unaltered retained evidence should rederive");

    let mut missing = checked.clone();
    missing.facts.flow.semantic_dependencies.rows.pop();
    assert_semantic_dependency_rederivation_rejects(&missing, "missing row");

    let mut duplicate = checked.clone();
    let duplicated_row = duplicate.facts.flow.semantic_dependencies.rows[0];
    duplicate
        .facts
        .flow
        .semantic_dependencies
        .rows
        .push(duplicated_row);
    assert_semantic_dependency_rederivation_rejects(&duplicate, "duplicate row");

    let mut reordered = checked.clone();
    assert!(
        reordered.facts.flow.semantic_dependencies.rows.len() >= 2,
        "fixture must carry enough rows to test canonical ordering"
    );
    reordered.facts.flow.semantic_dependencies.rows.swap(0, 1);
    assert_semantic_dependency_rederivation_rejects(&reordered, "reordered rows");

    let mut altered = checked;
    let row = &mut altered.facts.flow.semantic_dependencies.rows[0];
    row.exposure = match row.exposure {
        psi_checked_trees::CheckedSemanticDependencyExposure::PrivateImplementation => {
            psi_checked_trees::CheckedSemanticDependencyExposure::PublicInterface
        }
        psi_checked_trees::CheckedSemanticDependencyExposure::PublicInterface => {
            psi_checked_trees::CheckedSemanticDependencyExposure::PrivateImplementation
        }
    };
    assert_semantic_dependency_rederivation_rejects(&altered, "altered row");
}

fn assert_semantic_dependency_rederivation_rejects(
    checked: &omega_compiler::CheckedCompilation,
    mutation: &str,
) {
    let diagnostics = match project_checked_package_review(checked) {
        Err(diagnostics) => diagnostics,
        Ok(_) => panic!("{mutation} should reject retained semantic-dependency drift"),
    };
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "retained checked semantic-dependency evidence does not equal compiler rederivation",
        )
    }));
}

#[test]
fn dangerous_authority_classification_requires_exact_accepted_binding() {
    let Some(target) = host_target_name() else {
        return;
    };

    let canonical = TempPackage::new();
    canonical.write(
        "main.omg",
        r#"pub boundary trait FilesystemHost {
    machine write(descriptor: i32, bytes: &[u8]) -> i64;
}

pub data Journal { files: FilesystemHost; written: i64; }

pub machine Journal::append(&mut self, descriptor: i32, bytes: &[u8])
reaches FilesystemHost
invokes FilesystemHost;
{
    self.written = self.files.write(descriptor, bytes);
}
"#,
    );
    canonical.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let candidate_checked = compile_to_checked_with_packages(
        &canonical.0.join("main.omg"),
        Some(target),
        package_inputs(&canonical.0),
    )
    .expect("filesystem candidate should check without accepted authority");
    let candidate_review = project_checked_package_review(&candidate_checked)
        .expect("unaccepted filesystem candidate review should close");
    assert!(
        candidate_review.dangerous_authorities().is_empty(),
        "a readable package-owned name alone cannot mint filesystem authority",
    );
    let accepted = candidate_checked
        .candidate_service_binding(
            AcceptedSemanticBindingRole::FilesystemHostService,
            package_identity(),
            "FilesystemHost",
        )
        .expect("derive exact accepted filesystem binding");
    let canonical_checked = compile_to_checked_with_packages(
        &canonical.0.join("main.omg"),
        Some(target),
        package_inputs(&canonical.0)
            .with_accepted_semantic_bindings(vec![accepted])
            .expect("binding names the exact fixture package"),
    )
    .expect("accepted filesystem fixture should check");
    let canonical_review = project_checked_package_review(&canonical_checked)
        .expect("canonical filesystem review should close");
    let [authority] = canonical_review.dangerous_authorities() else {
        panic!("canonical filesystem authority row")
    };
    assert_eq!(
        authority.class(),
        PackageReviewDangerousAuthorityClass::Filesystem
    );
    assert_eq!(
        authority.service().owner(),
        PackageReviewNominalOwner::Package(package_identity()),
    );
    assert_eq!(authority.service().path(), "FilesystemHost");
    assert!(canonical_review.dangerous_authority_slack().is_empty());
    let rows = canonical_review
        .canonical_rows()
        .expect("filesystem review rows");
    let authority_row = rows
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::DangerousAuthority)
        .expect("dangerous authority canonical row");
    let locations = authority_row
        .source()
        .authored_locations()
        .expect("dangerous authority row must retain authored provenance");
    assert!(locations.iter().any(|location| {
        location.role() == PackageReviewSourceLocationRole::AuthorityDeclaration
            && matches!(
                location.owner(),
                PackageReviewSourceLocationOwner::Package(_)
            )
    }));
    assert!(locations.iter().any(|location| {
        location.role() == PackageReviewSourceLocationRole::AuthorityExposure
            && matches!(
                location.owner(),
                PackageReviewSourceLocationOwner::Package(_)
            )
            && location.relative_path() == "main.omg"
    }));
    assert!(locations.iter().all(|location| {
        !location
            .relative_path()
            .contains(canonical.0.to_string_lossy().as_ref())
    }));
}

#[test]
fn accepted_dependency_console_permission_retains_exact_policy_and_source_custody() {
    let root = TempPackage::new();
    let console = TempPackage::new();
    let root_package = package_identity();
    let console_package =
        PackageKeyIdentity::from_digest([49; 32]).expect("Console package identity");

    root.write(
        "main.omg",
        r#"use accepted_console::console;

pub machine terminate(console: Console, return_code: i32)
reaches Console
invokes console;
{
    console.exit_process(return_code);
}
"#,
    );
    root.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.package("review-fixture");
    builder.depend_as("accepted_console", Source::Path { location: "../console" });
    builder.select_provider<Console, ConsoleNativeProvider>();
}
"#,
    );
    console.write(
        "console.omg",
        r#"pub boundary trait Console {
    machine exit_process(return_code: i32)
    reaches Console;
}

pub data ConsoleNativeProvider { }
linux_x86_64 boundary machine ConsoleNativeProvider::exit_process(return_code: i32)
    satisfies Console::exit_process;
"#,
    );

    let base_inputs = || {
        PackageCompilationInputs::new_package(
            root_package,
            vec![
                PackageSourceBinding::new(root_package, "review-fixture", root.0.clone()),
                PackageSourceBinding::new(console_package, "accepted-console", console.0.clone()),
            ],
            vec![PackageDependencyBinding::new(
                root_package,
                "accepted_console",
                console_package,
            )],
        )
        .expect("ordinary Console dependency graph")
    };
    let candidate = compile_to_checked_with_packages(
        &root.0.join("main.omg"),
        Some("linux_x86_64"),
        base_inputs(),
    )
    .expect("candidate Console dependency should check before consumer acceptance");
    let (plan, retained) = candidate
        .selected_provider_plans()
        .plans()
        .iter()
        .zip(candidate.selected_provider_provenance())
        .find(|(plan, _)| plan.schema.trait_name == "Console")
        .expect("candidate retains its exact Console plan");
    let accepted = omega_package_compilation::AcceptedSemanticBinding::new(
        omega_package_compilation::AcceptedSemanticBindingRole::ConsoleExitProcessI32,
        console_package,
        candidate
            .typed
            .symbols
            .display_path(retained.provider.schema.symbol(), "::"),
        plan.schema.identity_digest(),
        plan.identity_digest(),
    )
    .expect("exact accepted Console binding");
    let checked_without_permission = compile_to_checked_with_packages(
        &root.0.join("main.omg"),
        Some("linux_x86_64"),
        base_inputs()
            .with_accepted_semantic_bindings(vec![accepted.clone()])
            .expect("binding package is in the exact closure"),
    )
    .expect("accepted Console dependency should settle exactly");
    let review_without_permission = project_checked_package_review(&checked_without_permission)
        .expect("resolved package Console authority should rederive during review");
    assert_eq!(review_without_permission.dangerous_authorities().len(), 1);
    assert!(
        review_without_permission
            .terminal_authority_permissions()
            .is_empty()
    );

    let [exit_method] = plan.schema.methods.as_slice() else {
        panic!("test Console schema has one exact requirement")
    };
    let accepted = accepted
        .with_terminal_authority_permissions(vec![
            omega_effects::ServiceTerminalAuthorityPermission::new(
                plan.schema.identity_digest(),
                exit_method.requirement_identity.clone(),
                omega_effects::TerminalAuthorityDisposition::from_classes([
                    omega_effects::TerminalAuthorityClass::ProcessTermination,
                ]),
            ),
        ])
        .expect("permission schema matches accepted Console schema");
    let checked = compile_to_checked_with_packages(
        &root.0.join("main.omg"),
        Some("linux_x86_64"),
        base_inputs()
            .with_accepted_semantic_bindings(vec![accepted])
            .expect("permission binding package is in the exact closure"),
    )
    .expect("permission-bearing accepted Console dependency should settle exactly");
    let review = project_checked_package_review(&checked)
        .expect("resolved package Console authority should rederive during review");
    let [authority] = review.dangerous_authorities() else {
        panic!("one exact accepted process authority row")
    };
    assert_eq!(
        authority.class(),
        PackageReviewDangerousAuthorityClass::Process,
    );
    assert_eq!(
        authority.service().owner(),
        PackageReviewNominalOwner::Package(console_package),
    );
    assert_eq!(authority.service().path(), "Console");

    let [permission] = review.terminal_authority_permissions() else {
        panic!("one exact Console exit permission row")
    };
    assert_eq!(
        permission.service().owner(),
        PackageReviewNominalOwner::Package(console_package),
    );
    assert_eq!(permission.service().path(), "Console");
    assert_eq!(permission.service_schema(), plan.schema.identity_digest());
    assert_eq!(
        permission.requirement_identity(),
        exit_method.requirement_identity,
    );
    assert_eq!(
        permission.permitted().classes(),
        &[omega_effects::TerminalAuthorityClass::ProcessTermination],
    );

    let permission_row = review
        .canonical_rows()
        .expect("canonical Console permission row")
        .into_iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::TerminalAuthorityPermission)
        .expect("terminal-authority permission is independently framed");
    assert_eq!(
        permission_row.risk(),
        PackageReviewCanonicalRowRisk::Blocking
    );
    assert_eq!(
        permission_row.source().compiler_derivations(),
        &[PackageReviewSyntheticSourceKind::ConsumerTerminalAuthorityPermission],
    );
    let locations = permission_row
        .source()
        .authored_locations()
        .expect("permission retains authored service and requirement custody");
    assert!(locations.iter().any(|location| {
        location.owner() == PackageReviewSourceLocationOwner::Package(console_package)
            && location.relative_path() == "console.omg"
            && location.role() == PackageReviewSourceLocationRole::AuthorityDeclaration
    }));
    assert!(locations.iter().any(|location| {
        location.owner() == PackageReviewSourceLocationOwner::Package(console_package)
            && location.relative_path() == "console.omg"
            && location.role() == PackageReviewSourceLocationRole::ProviderRequirementDeclaration
    }));
    let recovered = decode_package_review_canonical_row(
        &encode_package_review_canonical_row(&permission_row)
            .expect("encode exact permission row with source custody"),
    )
    .expect("recover exact permission row with source custody");
    assert_eq!(
        recovered.kind(),
        PackageReviewCanonicalRowKind::TerminalAuthorityPermission,
    );
    assert_eq!(
        recovered.source().compiler_derivations(),
        &[PackageReviewSyntheticSourceKind::ConsumerTerminalAuthorityPermission],
    );

    let results = reconstruct_ordinary_package_obligation_results(&checked)
        .expect("permission-bearing obligation result reconstruction");
    let [open_permission] = results.open_terminal_authority_permissions() else {
        panic!("one open root-admission permission obligation")
    };
    assert_eq!(
        open_permission.status(),
        OrdinaryPackageObligationStatus::OpenRootAdmission,
    );
    assert_eq!(open_permission.permission(), permission);
}
