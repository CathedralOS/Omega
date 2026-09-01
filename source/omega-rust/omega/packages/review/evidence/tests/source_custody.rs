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
        r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
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
fn dangerous_authority_classification_requires_exact_toolchain_provenance() {
    let Some(target) = host_target_name() else {
        return;
    };

    let canonical = TempPackage::new();
    canonical.write(
        "main.omg",
        r#"use omega::language::std::filesystem_host;

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
    .expect("canonical filesystem fixture should check");
    let canonical_review = project_checked_package_review(&canonical_checked)
        .expect("canonical filesystem review should close");
    let [authority] = canonical_review.dangerous_authorities() else {
        panic!("canonical filesystem authority row")
    };
    assert_eq!(
        authority.class(),
        PackageReviewDangerousAuthorityClass::Filesystem
    );
    let PackageReviewNominalOwner::ToolchainSource(source) = authority.service().owner() else {
        panic!("canonical filesystem authority must retain exact toolchain source")
    };
    assert_ne!(source.digest(), [0; 32]);
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
                PackageReviewSourceLocationOwner::Toolchain(_)
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

    let lookalike = TempPackage::new();
    lookalike.write(
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
    .expect("package-owned filesystem lookalike should check as ordinary source");
    let lookalike_review = project_checked_package_review(&lookalike_checked)
        .expect("package-owned filesystem lookalike review should close");
    assert!(
        lookalike_review.dangerous_authorities().is_empty(),
        "package-controlled naming must not mint a compiler-owned risk class"
    );
}

#[test]
fn process_authority_review_retains_closed_exit_and_discloses_unresolved_siblings() {
    // Package review closes the demanded process identity without pretending
    // that unrelated selected Console siblings have closed execution atoms.
    // Terminal realization remains responsible for rejecting an unresolved
    // sibling when a canonical artifact actually demands it.
    let target = "linux_x86_64";

    let canonical = TempPackage::new();
    canonical.write(
        "main.omg",
        r#"use omega::language::std::console;

pub machine terminate(console: Console, return_code: i32)
reaches Console
invokes console;
{
    console.exit_process(return_code);
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
    .expect("canonical console fixture should check");
    let canonical_review = project_checked_package_review(&canonical_checked)
        .expect("unsupported selected siblings remain non-authorizing review evidence");
    let [legacy_process_authority] = canonical_review.dangerous_authorities() else {
        panic!("legacy bundled Console must retain one process-authority row")
    };
    assert_eq!(
        legacy_process_authority.class(),
        PackageReviewDangerousAuthorityClass::Process,
    );
    assert!(matches!(
        legacy_process_authority.service().owner(),
        PackageReviewNominalOwner::ToolchainSource(_),
    ));
    let console = canonical_review
        .selected_providers()
        .iter()
        .find(|provider| provider.service_schema() == "Console")
        .expect("canonical Console provider review");
    assert_eq!(console.rows().len(), console.row_declarations().len());
    let execution_for = |method: &str| {
        let index = console
            .rows()
            .iter()
            .position(|row| row.method == method)
            .unwrap_or_else(|| panic!("selected Console plan retains `{method}`"));
        console.row_declarations()[index].compiler_intrinsic_execution()
    };
    assert_eq!(
        execution_for("exit_process"),
        Some(PackageReviewCompilerIntrinsicExecution::LinuxExitGroupI32),
        "the exact Linux exit execution remains closed",
    );
    for method in ["read_line", "read_byte", "write_byte"] {
        assert_eq!(
            execution_for(method),
            None,
            "unsupported selected Console sibling `{method}` must remain visible but non-authorizing",
        );
    }

    let lookalike = TempPackage::new();
    lookalike.write(
        "main.omg",
        r#"pub boundary trait Console {
    machine exit_process(return_code: i32);
}

pub machine terminate(console: Console, return_code: i32)
reaches Console
invokes console;
{
    console.exit_process(return_code);
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
    .expect("package-owned console lookalike should check as ordinary source");
    let lookalike_review = project_checked_package_review(&lookalike_checked)
        .expect("package-owned console lookalike review should close");
    assert!(
        lookalike_review.dangerous_authorities().is_empty(),
        "package-controlled Console naming must not mint process authority"
    );
}

#[test]
fn accepted_dependency_console_binding_classifies_the_exact_process_authority() {
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
        r#"target linux_x86_64 { }
machine build(builder: &mut Build) {
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
linux_x86_64 machine ConsoleNativeProvider::exit_process(return_code: i32)
    satisfies Console::exit_process
    via Binding::CompilerIntrinsic;
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
    let checked = compile_to_checked_with_packages(
        &root.0.join("main.omg"),
        Some("linux_x86_64"),
        base_inputs()
            .with_accepted_semantic_bindings(vec![accepted])
            .expect("binding package is in the exact closure"),
    )
    .expect("accepted Console dependency should settle exactly");
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
}
