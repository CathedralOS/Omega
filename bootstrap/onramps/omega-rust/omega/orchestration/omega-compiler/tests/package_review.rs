use omega_compiler::{
    BuildObservationClass, PACKAGE_REVIEW_ENCODING_VERSION, PACKAGE_REVIEW_ROW_ENCODING_VERSION,
    PackageCompilationInputs, PackageDependencyBinding, PackageReviewArithmeticDomain,
    PackageReviewCallableRole, PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk,
    PackageReviewCastForm, PackageReviewCheckedServiceReach, PackageReviewContractBinaryOperator,
    PackageReviewContractExpression, PackageReviewContractFact, PackageReviewContractKind,
    PackageReviewCrashInterface, PackageReviewCrashRouteGuard,
    PackageReviewDangerousAuthorityClass, PackageReviewDataMember,
    PackageReviewDomainClassification, PackageReviewDomainEstablishmentKind,
    PackageReviewNominalOwner, PackageReviewPropositionBinderKind,
    PackageReviewPropositionBinderValue, PackageReviewPropositionEvidence,
    PackageReviewRepresentationAbiCommitment, PackageReviewRepresentationMechanism,
    PackageReviewSemanticDependencyExposure, PackageReviewSemanticDependencyKind,
    PackageReviewSourceLocationOwner, PackageReviewSourceLocationRole,
    PackageReviewSynchronousInvocation, PackageReviewSyntheticSourceKind, PackageSourceBinding,
    compile_to_checked_with_packages, project_checked_package_review,
};
use psi_core::PackageKeyIdentity;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempPackage(PathBuf);

impl TempPackage {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-package-review-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create package review fixture");
        Self(path)
    }

    fn write(&self, path: impl AsRef<Path>, source: &str) {
        fs::write(self.0.join(path), source).expect("write package review fixture source");
    }
}

impl Drop for TempPackage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn package_identity() -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([41; 32]).expect("nonzero package identity")
}

fn package_inputs(root: &Path) -> PackageCompilationInputs {
    PackageCompilationInputs::new(
        package_identity(),
        vec![PackageSourceBinding::new(
            package_identity(),
            "review-fixture",
            root.to_owned(),
        )],
        Vec::new(),
    )
    .expect("single-package review graph should validate")
}

#[test]
fn package_source_consumption_commitment_binds_loaded_bytes_not_cache_location() {
    let Some(target) = host_target_name() else {
        return;
    };
    let source = "pub data Token { value: i64; }\n";
    let changed_source = "// source-only change\npub data Token { value: i64; }\n";
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
"#;

    let first = TempPackage::new();
    first.write("main.omg", source);
    first.write("build.omg", build);
    let first_checked = compile_to_checked_with_packages(
        &first.0.join("main.omg"),
        Some(target),
        package_inputs(&first.0),
    )
    .expect("first package source should check");
    let first_commitment = first_checked
        .source_consumption_commitment()
        .expect("package-aware compilation must retain source consumption");
    assert_ne!(first_commitment.digest(), [0; 32]);
    first_checked
        .verify_current_source_consumption()
        .expect("unchanged loaded source should verify");

    let relocated = TempPackage::new();
    relocated.write("main.omg", source);
    relocated.write("build.omg", build);
    let relocated_checked = compile_to_checked_with_packages(
        &relocated.0.join("main.omg"),
        Some(target),
        package_inputs(&relocated.0),
    )
    .expect("relocated package source should check");
    assert_eq!(
        first_commitment,
        relocated_checked
            .source_consumption_commitment()
            .expect("relocated package source commitment"),
        "absolute cache location and source-id assignment are not source identity"
    );

    let changed = TempPackage::new();
    changed.write("main.omg", changed_source);
    changed.write("build.omg", build);
    let changed_checked = compile_to_checked_with_packages(
        &changed.0.join("main.omg"),
        Some(target),
        package_inputs(&changed.0),
    )
    .expect("source-only changed package should check");
    assert_ne!(
        first_commitment,
        changed_checked
            .source_consumption_commitment()
            .expect("changed package source commitment")
    );
    let first_review =
        project_checked_package_review(&first_checked).expect("first package review");
    let changed_review =
        project_checked_package_review(&changed_checked).expect("changed package review");
    assert_eq!(
        first_review
            .canonical_review_bytes()
            .expect("first package review bytes"),
        changed_review
            .canonical_review_bytes()
            .expect("changed package review bytes"),
        "source consumption and normalized capability/API comparison remain separate identities"
    );
    let first_rows = first_review.canonical_rows().expect("first canonical rows");
    let changed_rows = changed_review
        .canonical_rows()
        .expect("changed-source canonical rows");
    assert_eq!(
        first_rows, changed_rows,
        "source-only changes remain outside every normalized capability/API row"
    );
    let first_data = first_rows
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicData)
        .expect("public data row");
    let changed_data = changed_rows
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicData)
        .expect("changed-source public data row");
    let first_locations = first_data
        .source()
        .authored_locations()
        .expect("public data receives an authored source coordinate");
    let changed_locations = changed_data
        .source()
        .authored_locations()
        .expect("changed public data receives an authored source coordinate");
    assert_eq!(first_locations.len(), 1);
    assert_eq!(changed_locations.len(), 1);
    assert_eq!(first_locations[0].relative_path(), "main.omg");
    assert_eq!(changed_locations[0].relative_path(), "main.omg");
    assert!(changed_locations[0].start_byte() > first_locations[0].start_byte());
    assert!(
        !first_locations[0]
            .relative_path()
            .contains(&first.0.display().to_string())
    );

    first.write("main.omg", changed_source);
    assert!(
        first_checked.verify_current_source_consumption().is_err(),
        "loaded source drift must reject against the retained compiler bytes"
    );

    let graph_root = TempPackage::new();
    graph_root.write("main.omg", source);
    graph_root.write("build.omg", build);
    let dependency = TempPackage::new();
    dependency.write("main.omg", "pub data DependencyToken {}\n");
    dependency.write("build.omg", build);
    let root_identity = package_identity();
    let dependency_identity =
        PackageKeyIdentity::from_digest([42; 32]).expect("dependency package identity");
    let graph_inputs = |alias: &str| {
        PackageCompilationInputs::new(
            root_identity,
            vec![
                PackageSourceBinding::new(root_identity, "graph-root", graph_root.0.clone()),
                PackageSourceBinding::new(
                    dependency_identity,
                    "graph-dependency",
                    dependency.0.clone(),
                ),
            ],
            vec![PackageDependencyBinding::new(
                root_identity,
                alias,
                dependency_identity,
            )],
        )
        .expect("two-package graph should validate")
    };
    let first_graph = compile_to_checked_with_packages(
        &graph_root.0.join("main.omg"),
        Some(target),
        graph_inputs("dependency"),
    )
    .expect("first reconciled graph should check");
    let renamed_graph = compile_to_checked_with_packages(
        &graph_root.0.join("main.omg"),
        Some(target),
        graph_inputs("renamed_dependency"),
    )
    .expect("renamed reconciled graph should check");
    assert_ne!(
        first_graph.source_consumption_commitment(),
        renamed_graph.source_consumption_commitment(),
        "requester-local dependency bindings must enter compiler-consumption identity even when unused"
    );
}

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
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
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
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
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
    let inputs = PackageCompilationInputs::new(
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
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
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
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
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
fn process_authority_classification_requires_exact_toolchain_console() {
    let Some(target) = host_target_name() else {
        return;
    };

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
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
"#,
    );
    let canonical_checked = compile_to_checked_with_packages(
        &canonical.0.join("main.omg"),
        Some(target),
        package_inputs(&canonical.0),
    )
    .expect("canonical console fixture should check");
    let canonical_review = project_checked_package_review(&canonical_checked)
        .expect("canonical console review should close");
    let [authority] = canonical_review.dangerous_authorities() else {
        panic!("canonical process authority row")
    };
    assert_eq!(
        authority.class(),
        PackageReviewDangerousAuthorityClass::Process
    );
    let PackageReviewNominalOwner::ToolchainSource(source) = authority.service().owner() else {
        panic!("canonical process authority must retain exact toolchain source")
    };
    assert_ne!(source.digest(), [0; 32]);
    assert_eq!(authority.service().path(), "Console");
    assert!(canonical_review.dangerous_authority_slack().is_empty());
    let rows = canonical_review
        .canonical_rows()
        .expect("process review rows");
    let authority_row = rows
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::DangerousAuthority)
        .expect("dangerous process authority canonical row");
    assert_eq!(
        authority_row.risk(),
        PackageReviewCanonicalRowRisk::Blocking
    );
    let locations = authority_row
        .source()
        .authored_locations()
        .expect("dangerous process authority row must retain authored provenance");
    assert!(locations.iter().any(|location| {
        location.role() == PackageReviewSourceLocationRole::AuthorityDeclaration
            && matches!(
                location.owner(),
                PackageReviewSourceLocationOwner::Toolchain(_)
            )
            && location.relative_path() == "console.omg"
    }));
    assert!(locations.iter().any(|location| {
        location.role() == PackageReviewSourceLocationRole::AuthorityExposure
            && matches!(
                location.owner(),
                PackageReviewSourceLocationOwner::Package(_)
            )
            && location.relative_path() == "main.omg"
    }));

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
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
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
fn empty_boundary_body_is_checked_callable_and_remains_directly_invocable() {
    let Some(target) = host_target_name() else {
        return;
    };

    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"use omega::language::std::filesystem_host;

boundary machine adapter() reaches FilesystemHost { }

pub machine caller() reaches FilesystemHost {
    adapter();
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("an explicit empty boundary body remains executable");
    let review =
        project_checked_package_review(&checked).expect("empty boundary body review should close");
    let adapter = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "adapter")
        .expect("adapter review row");
    assert!(matches!(
        adapter.checked_service_reach(),
        PackageReviewCheckedServiceReach::CheckedBody {
            realized,
            concrete,
        } if realized.is_empty() && concrete.is_empty()
    ));
    assert!(review.dangerous_authority_slack().iter().any(|slack| {
        slack.class() == PackageReviewDangerousAuthorityClass::Filesystem
            && slack.callable().path() == "adapter"
    }));
}

#[test]
fn package_review_rejects_impossible_supply_body_combinations() {
    let Some(target) = host_target_name() else {
        return;
    };

    let package = TempPackage::new();
    package.write("main.omg", "pub machine api() { }\n");
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("ordinary package should check");

    let mut missing_body = checked.clone();
    missing_body
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "api")
        .expect("api machine")
        .body_is_present = false;
    let diagnostics = project_checked_package_review(&missing_body)
        .expect_err("checked supply without a body must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("classified as checked supply but has no retained body")
    }));

    let mut bodyful_accepted = checked;
    let api = bodyful_accepted
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "api")
        .expect("api machine");
    api.supply_mode = psi_language_semantics::MachineSupplyMode::Accepted;
    let diagnostics = project_checked_package_review(&bodyful_accepted)
        .expect_err("bodyless supply with a body must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("has bodyless supply but retains a body")
    }));
}

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
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
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
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
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
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
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
    assert_eq!(row.abi(), PackageReviewRepresentationAbiCommitment::Unbound);
    assert_eq!(
        row.mechanism(),
        PackageReviewRepresentationMechanism::Unbound
    );
    assert!(
        review.public_data().is_empty(),
        "ordinary public API projection remains visibility-scoped"
    );

    let control = TempPackage::new();
    control.write("main.omg", "data InternalToken { }\n");
    control.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
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
fn review_projects_root_boundary_and_build_authority() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"boundary machine host_ping() reaches <= Host;
boundary trait Host { machine ping(); }
machine ping_leaf() satisfies Host::ping via Binding::VtableSlot(1);
data Receipt [linear] { code: i32; }
pub data Packet [copy] { #1 value: u32; }
pub domain Packet::Ready;
domain Packet::Private;
data PrivatePacket { hidden: u32; }
machine helper()
crashes Abort
{
    crash Abort;
}
pub machine public_api() { }
machine private_api() { }
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build)
crashes Abort
{
    helper();
    let receipt: Receipt = Receipt { code: 1 };
    crash Abort;
}
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("package fixture should check");
    let observations = checked
        .build_observation_summary()
        .expect("selected build machine publishes build observation evidence");
    assert_eq!(observations.ceiling(), BuildObservationClass::Hermetic);
    assert_eq!(observations.realized(), BuildObservationClass::Hermetic);
    let review = project_checked_package_review(&checked).expect("review projection should close");
    let encoded = review
        .canonical_review_bytes()
        .expect("review projection should have a canonical comparison encoding");
    let magic = b"OMEGA-PACKAGE-REVIEW\0";
    assert!(encoded.starts_with(magic));
    assert_eq!(
        &encoded[magic.len()..magic.len() + 2],
        &PACKAGE_REVIEW_ENCODING_VERSION.to_le_bytes(),
    );
    assert_eq!(
        encoded,
        review
            .canonical_review_bytes()
            .expect("repeated encoding must be deterministic")
    );
    let rows = review
        .canonical_rows()
        .expect("review projection should have canonical comparison rows");
    assert_eq!(
        rows,
        review
            .canonical_rows()
            .expect("repeated row encoding must be deterministic")
    );
    assert!(rows.windows(2).all(|pair| {
        (pair[0].kind(), pair[0].key_bytes()) < (pair[1].kind(), pair[1].key_bytes())
    }));
    assert!(
        rows.iter()
            .any(|row| row.kind() == PackageReviewCanonicalRowKind::ProjectionHeader)
    );
    assert!(
        rows.iter()
            .any(|row| row.kind() == PackageReviewCanonicalRowKind::Callable)
    );
    let row_magic = b"OMEGA-PACKAGE-REVIEW-ROW\0";
    for row in &rows {
        assert!(row.canonical_bytes().starts_with(row_magic));
        assert_eq!(
            &row.canonical_bytes()[row_magic.len()..row_magic.len() + 2],
            &PACKAGE_REVIEW_ROW_ENCODING_VERSION.to_le_bytes()
        );
    }

    assert_eq!(review.package(), package_identity());
    assert_eq!(
        review.target().target_name(),
        target,
        "review identity must retain the deployment profile, not only its native ABI",
    );
    assert_eq!(PACKAGE_REVIEW_ENCODING_VERSION, 35);
    assert_eq!(PACKAGE_REVIEW_ROW_ENCODING_VERSION, 1);
    let [ready] = review.public_domains() else {
        panic!("one package-owned public domain row")
    };
    assert_eq!(ready.identity().path(), "Packet::Ready");
    assert!(ready.type_parameters().is_empty());
    assert!(!ready.target_type().canonical().is_empty());
    assert!(ready.index_arguments().is_empty());
    let [packet] = review.public_data() else {
        panic!("one package-owned public data row")
    };
    assert_eq!(packet.identity().path(), "Packet");
    assert_eq!(packet.lifetime_parameter_count(), 0);
    assert_eq!(packet.members().len(), 1);
    let PackageReviewDataMember::Field(value) = &packet.members()[0] else {
        panic!("Packet value field")
    };
    assert_eq!(value.identity(), Some(1));
    assert_eq!(value.name(), "value");
    assert!(!value.type_identity().canonical().is_empty());
    assert_eq!(review.callables().len(), 3);
    let boundary = review
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Boundary)
        .expect("boundary row");
    assert_eq!(boundary.identity().path(), "host_ping");
    assert_eq!(boundary.lifetime_parameter_count(), 0);
    assert!(boundary.type_parameters().is_empty());
    assert!(boundary.parameters().is_empty());
    assert!(!boundary.return_type().canonical().is_empty());
    assert_eq!(
        boundary.identity().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    let [declared] = boundary
        .declared_service_reach()
        .expect("installation-bound declaration retains its upper bound")
    else {
        panic!("one declared upper-bound service")
    };
    assert_eq!(declared.path(), "Host");
    assert_eq!(
        declared.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(
        boundary.checked_service_reach(),
        &PackageReviewCheckedServiceReach::NoCheckedBody
    );
    assert!(boundary.capability_flows().is_empty());
    assert_eq!(boundary.declared_synchronous_invocations(), Some(&[][..]));
    assert!(boundary.realized_synchronous_invocations().is_empty());
    let [installation] = boundary.unresolved_installation_reaches() else {
        panic!("one normalized installation-bound reach row")
    };
    assert_eq!(
        installation.requirement().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert!(installation.requirement().path().contains("host_ping"));
    let [upper_bound] = installation.upper_bound() else {
        panic!("one normalized installation upper-bound service")
    };
    assert_eq!(
        upper_bound.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(upper_bound.path(), "Host");

    let build = review
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Build)
        .expect("build row");
    assert_eq!(build.identity().path(), "build");
    let [builder] = build.parameters() else {
        panic!("build entry retains its builder parameter")
    };
    assert_eq!(builder.name(), "builder");
    assert!(builder.is_mutable());
    assert!(!builder.is_const());
    assert!(!builder.is_self());
    assert!(!builder.type_identity().canonical().is_empty());
    assert_eq!(build.declared_service_reach(), None);
    assert_eq!(build.declared_synchronous_invocations(), None);

    let public = review
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Public)
        .expect("ordinary public callable row");
    assert_eq!(public.identity().path(), "public_api");
    assert_eq!(
        public.identity().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(public.declared_service_reach(), Some(&[][..]));
    assert_eq!(public.declared_synchronous_invocations(), Some(&[][..]));
    assert!(matches!(
        public.checked_service_reach(),
        PackageReviewCheckedServiceReach::CheckedBody {
            realized,
            concrete,
        } if realized.is_empty() && concrete.is_empty()
    ));
    assert!(public.realized_synchronous_invocations().is_empty());
    assert_eq!(
        public.checked_crash().interface(),
        PackageReviewCrashInterface::PublishedCeiling
    );
    assert!(
        review
            .callables()
            .iter()
            .all(|callable| callable.identity().path() != "private_api")
    );
    let crash = build.checked_crash();
    assert_eq!(
        crash.interface(),
        PackageReviewCrashInterface::PublishedCeiling
    );
    let [published_crash] = crash.published() else {
        panic!("one normalized published crash route")
    };
    assert_eq!(
        published_crash.cause(),
        psi_checked_trees::CrashCause::Abort
    );
    assert_eq!(
        published_crash.alternative_guards(),
        [PackageReviewCrashRouteGuard::Truth]
    );
    let [crash_site] = crash.checked_sites() else {
        panic!("one normalized checked crash site")
    };
    assert_eq!(
        crash_site.state().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(crash_site.cause(), psi_checked_trees::CrashCause::Abort);
    assert_eq!(crash_site.guard_covering_buckets(), [1]);
    assert!(!crash_site.frontier_lower_bound().is_empty());
    assert!(
        crash_site
            .frontier_lower_bound()
            .iter()
            .all(|claim| claim.machine().owner()
                == PackageReviewNominalOwner::Package(package_identity())
                && claim.state().owner() == PackageReviewNominalOwner::Package(package_identity()))
    );
    let [crash_call] = crash.checked_calls() else {
        panic!("one normalized checked crash call")
    };
    assert_eq!(
        crash_call.state().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(
        crash_call.target_machine().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(crash_call.target_machine().path(), "helper");
    assert_eq!(
        crash_call.target_state().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );

    let [provider] = review.selected_providers() else {
        panic!("one selected provider review row")
    };
    assert_eq!(provider.realizing_package(), Some(package_identity()));
    assert_eq!(provider.provider_type_package(), None);
    assert_eq!(provider.service_schema(), "Host");
    assert_eq!(
        provider.schema().trait_package_identity,
        Some(package_identity())
    );
    assert_eq!(
        provider.schema().methods[0].requirement_owner_package_identity,
        Some(package_identity())
    );
    assert_eq!(provider.rows().len(), 1);
    assert!(matches!(
        provider.rows()[0].binding,
        omega_effects::provider_plan::ProviderBinding::VtableSlot { index: 1 }
    ));
    let provider_row = rows
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::SelectedProviderSet)
        .expect("selected provider canonical row");
    assert_eq!(
        provider_row.source().compiler_derivations(),
        [
            PackageReviewSyntheticSourceKind::UniqueCoveringProviderSelection,
            PackageReviewSyntheticSourceKind::FreeExternalProviderType,
        ]
    );
    let provider_locations = provider_row
        .source()
        .authored_locations()
        .expect("implicit provider still retains authored schema and realization provenance");
    assert!(provider_locations.iter().any(|location| {
        location.role() == PackageReviewSourceLocationRole::ProviderSchemaDeclaration
            && location.relative_path() == "main.omg"
    }));
    assert!(provider_locations.iter().any(|location| {
        location.role() == PackageReviewSourceLocationRole::ProviderRealization
            && location.relative_path() == "main.omg"
    }));
    assert!(!provider_locations.iter().any(|location| matches!(
        location.role(),
        PackageReviewSourceLocationRole::ProviderSelection
            | PackageReviewSourceLocationRole::ProviderTypeDeclaration
    )));
}

#[test]
fn review_projects_exact_accepted_boundary_contracts() {
    let Some(target) = host_target_name() else {
        return;
    };
    let compile_claim = |value: u8| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!("boundary machine trusted_zero() -> u64\nensures result == {value};\n"),
        );
        package.write(
            "build.omg",
            r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
"#,
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("accepted boundary claim should check");
        project_checked_package_review(&checked).expect("accepted boundary contract review")
    };

    let zero = compile_claim(0);
    let boundary = zero
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Boundary)
        .expect("boundary callable row");
    assert_eq!(
        boundary.supply(),
        psi_language_semantics::MachineSupplyMode::Accepted,
        "a bodyless boundary guarantee must remain an explicit trust-bearing accepted claim",
    );
    assert_eq!(
        boundary.checked_service_reach(),
        &PackageReviewCheckedServiceReach::NoCheckedBody
    );
    assert!(zero.dangerous_authority_slack().is_empty());
    let [contract] = boundary.contracts() else {
        panic!("one exact accepted contract row")
    };
    assert_eq!(contract.kind(), PackageReviewContractKind::Ensures);
    assert_eq!(contract.binding(), None);
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        operator,
        left,
        right,
    }) = contract.fact()
    else {
        panic!("exact equality expression")
    };
    assert_eq!(*operator, PackageReviewContractBinaryOperator::Equal);
    assert_eq!(**left, PackageReviewContractExpression::Result);
    assert_eq!(
        **right,
        PackageReviewContractExpression::Integer("0".to_owned())
    );
    let zero_rows = zero.canonical_rows().expect("zero claim rows");
    let accepted_claims = zero_rows
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::AcceptedClaim)
        .collect::<Vec<_>>();
    let [accepted_claim] = accepted_claims.as_slice() else {
        panic!("one explicit accepted-claim row")
    };
    assert_eq!(
        accepted_claim.risk(),
        PackageReviewCanonicalRowRisk::Blocking
    );
    assert!(
        accepted_claim
            .key_bytes()
            .windows("trusted_zero".len())
            .any(|window| window == b"trusted_zero")
    );
    let [claim_location] = accepted_claim
        .source()
        .authored_locations()
        .expect("accepted claim declaration source")
    else {
        panic!("one accepted claim declaration location")
    };
    assert_eq!(claim_location.relative_path(), "main.omg");
    assert_eq!(
        claim_location.role(),
        PackageReviewSourceLocationRole::Declaration
    );

    let one = compile_claim(1);
    let one_rows = one.canonical_rows().expect("one claim rows");
    let one_claim = one_rows
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::AcceptedClaim)
        .expect("changed accepted claim row");
    assert_ne!(
        accepted_claim.canonical_bytes(),
        one_claim.canonical_bytes(),
        "changing an accepted guarantee must change its trust-bearing row",
    );
    assert_ne!(
        zero.canonical_review_bytes().expect("zero claim encoding"),
        one.canonical_review_bytes().expect("one claim encoding"),
        "changing an accepted guarantee must change exact review evidence",
    );
}

#[test]
fn claim_free_boundary_supply_does_not_collapse_into_an_accepted_claim() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write("main.omg", "boundary machine host_ping();\n");
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("claim-free boundary fixture should check");
    let review =
        project_checked_package_review(&checked).expect("claim-free boundary review should close");
    let boundary = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "host_ping")
        .expect("claim-free boundary row");
    assert_eq!(
        boundary.supply(),
        psi_language_semantics::MachineSupplyMode::Boundary
    );
    assert!(boundary.contracts().is_empty());
    assert_eq!(
        boundary.checked_service_reach(),
        &PackageReviewCheckedServiceReach::NoCheckedBody
    );
    assert!(review.dangerous_authority_slack().is_empty());
    assert!(
        review
            .canonical_rows()
            .expect("claim-free boundary rows")
            .iter()
            .all(|row| row.kind() != PackageReviewCanonicalRowKind::AcceptedClaim),
        "claim-free boundary supply must not emit accepted-claim evidence"
    );
}

#[test]
fn review_projects_exact_public_domain_membership_contracts() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub domain u64::Trusted;

pub machine consume(value: u64)
requires value in u64::Trusted
{ }
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("checked public-domain membership requirement should check");
    let review = project_checked_package_review(&checked).expect("membership contract review");
    let callable = review
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Public)
        .expect("public callable row");
    let [contract] = callable.contracts() else {
        panic!("one exact membership contract")
    };
    let PackageReviewContractFact::Membership { value, domain } = contract.fact() else {
        panic!("exact membership row")
    };
    assert_eq!(*value, PackageReviewContractExpression::Parameter(0));
    assert_eq!(domain.path(), "u64::Trusted");
    assert_eq!(
        domain.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );

    let hidden = TempPackage::new();
    hidden.write(
        "main.omg",
        r#"domain u64::Hidden;
pub machine consume(value: u64)
requires value in u64::Hidden
{ }
"#,
    );
    hidden.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &hidden.0.join("main.omg"),
        Some(target),
        package_inputs(&hidden.0),
    )
    .expect("private-domain contract fixture should check before package review");
    let diagnostics = project_checked_package_review(&checked)
        .expect_err("a public callable must not expose a private domain");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("exposes non-public domain `u64::Hidden`")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn review_projects_structural_propositions_and_alpha_normalizes_their_binders() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let renamed = TempPackage::new();
    original.write(
        "main.omg",
        r#"proposition equivalent<Element>(left: Element, right: Element);
pub machine compare<Value>(left: Value, right: Value)
requires equivalent<Value>(left, right)
{ }
"#,
    );
    renamed.write(
        "main.omg",
        r#"proposition equivalent<Item>(left: Item, right: Item);
pub machine compare<Compared>(left: Compared, right: Compared)
requires equivalent<Compared>(left, right)
{ }
"#,
    );
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
"#;
    original.write("build.omg", build);
    renamed.write("build.omg", build);

    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("generic proposition fixture should check");
        project_checked_package_review(&checked).expect("generic proposition review")
    };
    let original = project(&original);
    let renamed = project(&renamed);
    let compare = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("compare"))
        .expect("public comparison callable");
    let [contract] = compare.contracts() else {
        panic!("one proposition contract")
    };
    assert_eq!(contract.evidence_lane_position(), None);
    let PackageReviewContractFact::Proposition(application) = contract.fact() else {
        panic!("exact proposition application")
    };
    assert_eq!(application.declaration().path(), "equivalent");
    let [binder] = application.binders() else {
        panic!("one proposition binder")
    };
    assert_eq!(binder.kind(), &PackageReviewPropositionBinderKind::Type);
    let [argument] = application.binder_arguments() else {
        panic!("one proposition binder argument")
    };
    assert_eq!(
        argument.value(),
        &PackageReviewPropositionBinderValue::GenericBinder(0)
    );
    assert_eq!(application.parameter_types().len(), 2);
    assert_eq!(
        application.arguments(),
        [
            PackageReviewContractExpression::Parameter(0),
            PackageReviewContractExpression::Parameter(1),
        ]
    );
    assert_eq!(
        application.evidence(),
        &PackageReviewPropositionEvidence::FactOnly
    );
    assert_eq!(
        original
            .canonical_review_bytes()
            .expect("original encoding"),
        renamed.canonical_review_bytes().expect("renamed encoding"),
        "renaming callable and proposition binders must not alter package evidence",
    );
}

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
proposition carries<Element>(value: Element) evidence Evidence<Element>;
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
proposition carries<Element>(value: Element) evidence Evidence<Element>;
proposition forwarded<Item>(value: Item) = carries<Item>(value);
pub machine consume()
requires evidence: forwarded<i32>(1)
{ }
"#;
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
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
    let consume = direct_review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("consume"))
        .expect("public consumer");
    let [contract] = consume.contracts() else {
        panic!("one named witness contract")
    };
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
    assert_eq!(
        direct_review
            .canonical_review_bytes()
            .expect("direct witness encoding"),
        aliased_review
            .canonical_review_bytes()
            .expect("aliased witness encoding"),
        "a transparent proposition alias and local requires-binding rename must not mint package identity",
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
proposition left_fact() evidence Evidence;
proposition right_fact() evidence Evidence;
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
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
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
proposition holds<Element>() evidence Evidence<Element>;
proposition selected<machine Witness>();
pub machine caller()
requires {binding}: holds<i32>()
requires selected<{binding}.modulus>()
{{ }}
"#
        )
    };
    original.write("main.omg", &source("proof"));
    renamed.write("main.omg", &source("evidence"));
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
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

#[test]
fn review_rejects_unrepresented_callable_contract_expressions() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"machine chosen(value: u64) -> u64 { value }
machine apply<machine Selected>(value: u64) -> u64
where machine Selected(value: u64) -> u64
{
    Selected(value)
}
boundary machine trusted_zero() -> u64
ensures result == apply<chosen>(0);
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("effect-free static contract call should check");
    let diagnostics = project_checked_package_review(&checked)
        .expect_err("unrepresented static call arguments must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("static arguments not yet represented")
    }));
}

#[test]
fn review_projects_contract_member_paths_with_exact_receivers_and_fields() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let changed = TempPackage::new();
    let source = |left_receiver: &str, right_receiver: &str| {
        format!(
            r#"pub data Pair [copy] {{
    left: i32;
    right: i32;
}}
pub machine compare(first: Pair, second: Pair)
requires {left_receiver}.left == {right_receiver}.right
{{ }}
"#
        )
    };
    original.write("main.omg", &source("first", "second"));
    changed.write("main.omg", &source("second", "first"));
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
"#;
    original.write("build.omg", build);
    changed.write("build.omg", build);
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("member-path contract fixture should check");
        project_checked_package_review(&checked).expect("member-path package review")
    };
    let original = project(&original);
    let changed = project(&changed);
    let compare = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "compare")
        .expect("public comparison callable");
    let [contract] = compare.contracts() else {
        panic!("one member-path contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        left,
        right,
        ..
    }) = contract.fact()
    else {
        panic!("binary member-path contract")
    };
    let PackageReviewContractExpression::Member {
        receiver: left_receiver,
        member: left_member,
        case_variant: left_variant,
    } = left.as_ref()
    else {
        panic!("left member path")
    };
    let PackageReviewContractExpression::Member {
        receiver: right_receiver,
        member: right_member,
        case_variant: right_variant,
    } = right.as_ref()
    else {
        panic!("right member path")
    };
    assert_eq!(
        left_receiver.as_ref(),
        &PackageReviewContractExpression::Parameter(0)
    );
    assert_eq!(
        right_receiver.as_ref(),
        &PackageReviewContractExpression::Parameter(1)
    );
    assert_eq!(left_member.path(), "Pair::left");
    assert_eq!(right_member.path(), "Pair::right");
    assert!(left_variant.is_none());
    assert!(right_variant.is_none());
    assert_eq!(
        left_member.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_ne!(
        original
            .canonical_review_bytes()
            .expect("original member-path encoding"),
        changed
            .canonical_review_bytes()
            .expect("changed member-path encoding"),
        "changing only the receiver coordinates must change package review identity",
    );
}

#[test]
fn review_projects_contract_casts_without_diagnostic_spelling() {
    let Some(target) = host_target_name() else {
        return;
    };
    let u16_cast = TempPackage::new();
    let u32_cast = TempPackage::new();
    let source = |target_type: &str| {
        format!(
            r#"pub machine compare(value: u8)
requires (value as {target_type}) == 1
{{ }}
"#
        )
    };
    u16_cast.write("main.omg", &source("u16"));
    u32_cast.write("main.omg", &source("u32"));
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
"#;
    u16_cast.write("build.omg", build);
    u32_cast.write("build.omg", build);
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("exact widening cast contract should check");
        project_checked_package_review(&checked).expect("cast contract package review")
    };
    let u16_cast = project(&u16_cast);
    let u32_cast = project(&u32_cast);
    let compare = u16_cast
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "compare")
        .expect("public comparison callable");
    let [contract] = compare.contracts() else {
        panic!("one cast contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        left, ..
    }) = contract.fact()
    else {
        panic!("binary cast contract")
    };
    let PackageReviewContractExpression::Cast {
        value,
        target,
        arithmetic_domain,
        semantic_domain,
        semantic_domain_arguments,
        form,
    } = left.as_ref()
    else {
        panic!("structural cast expression")
    };
    assert_eq!(
        value.as_ref(),
        &PackageReviewContractExpression::Parameter(0)
    );
    assert!(target.canonical().contains("u16"));
    assert_eq!(*arithmetic_domain, PackageReviewArithmeticDomain::Exact);
    assert!(semantic_domain.is_none());
    assert!(semantic_domain_arguments.is_empty());
    assert_eq!(*form, PackageReviewCastForm::Value);
    assert_ne!(
        u16_cast
            .canonical_review_bytes()
            .expect("u16 cast encoding"),
        u32_cast
            .canonical_review_bytes()
            .expect("u32 cast encoding"),
        "changing the exact cast target must change package review identity",
    );
}

#[test]
fn review_casts_retain_public_semantic_domains_and_reject_private_exposure() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
"#;
    let public = TempPackage::new();
    public.write(
        "main.omg",
        r#"pub domain u16::Tagged;
pub machine compare(value: u8)
requires (value as u16 in Tagged) == 1
{ }
"#,
    );
    public.write("build.omg", build);
    let checked = compile_to_checked_with_packages(
        &public.0.join("main.omg"),
        Some(target),
        package_inputs(&public.0),
    )
    .expect("public semantic-domain cast contract should check");
    let review = project_checked_package_review(&checked).expect("public domain cast review");
    let compare = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "compare")
        .expect("public comparison callable");
    let [contract] = compare.contracts() else {
        panic!("one public-domain cast contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        left, ..
    }) = contract.fact()
    else {
        panic!("binary public-domain cast contract")
    };
    let PackageReviewContractExpression::Cast {
        semantic_domain: Some(domain),
        ..
    } = left.as_ref()
    else {
        panic!("semantic domain cast identity")
    };
    assert_eq!(domain.path(), "u16::Tagged");
    assert_eq!(
        domain.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );

    let private = TempPackage::new();
    private.write(
        "main.omg",
        r#"domain u16::Hidden;
pub machine compare(value: u8)
requires (value as u16 in Hidden) == 1
{ }
"#,
    );
    private.write("build.omg", build);
    let checked = compile_to_checked_with_packages(
        &private.0.join("main.omg"),
        Some(target),
        package_inputs(&private.0),
    )
    .expect("private semantic-domain cast contract should check before package review");
    let diagnostics = project_checked_package_review(&checked)
        .expect_err("a public contract must not expose a private semantic domain");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("exposes non-public semantic domain")
    }));
}

#[test]
fn public_callable_signatures_are_exact_and_lifetime_alpha_normalized() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let renamed = TempPackage::new();
    let changed = TempPackage::new();
    original.write(
        "main.omg",
        r#"pub machine borrow<'source, 'temporary>(
    source: &'source [u8],
    temporary: &'temporary [u8]
) -> &'source [u8] { source }
pub machine identity<Element [copy]>(value: Element) -> Element { value }
"#,
    );
    renamed.write(
        "main.omg",
        r#"pub machine borrow<'origin, 'scratch>(
    source: &'origin [u8],
    temporary: &'scratch [u8]
) -> &'origin [u8] { source }
pub machine identity<Value [copy]>(value: Value) -> Value { value }
"#,
    );
    changed.write(
        "main.omg",
        r#"pub machine borrow<'source, 'temporary>(
    source: &'source [u8],
    temporary: &'temporary [u8]
) -> &'temporary [u8] { temporary }
pub machine identity<Element [copy]>(value: Element) -> Element { value }
"#,
    );
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
"#;
    original.write("build.omg", build);
    renamed.write("build.omg", build);
    changed.write("build.omg", build);

    let review = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("public callable signature fixture should check");
        project_checked_package_review(&checked).expect("callable signature review should close")
    };
    let original = review(&original);
    let renamed = review(&renamed);
    let changed = review(&changed);
    let borrow = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("borrow"))
        .expect("borrow callable row");
    assert_eq!(borrow.lifetime_parameter_count(), 2);
    assert_eq!(borrow.parameters().len(), 2);
    assert!(borrow.type_parameters().is_empty());
    assert!(!borrow.return_type().canonical().is_empty());
    let identity = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("identity"))
        .expect("generic identity callable row");
    assert_eq!(identity.type_parameters().len(), 1);
    assert_eq!(identity.parameters().len(), 1);

    assert_eq!(
        original
            .canonical_review_bytes()
            .expect("original encoding"),
        renamed.canonical_review_bytes().expect("renamed encoding"),
        "renaming lifetime and type binders must not alter canonical review evidence",
    );
    assert_ne!(
        original
            .canonical_review_bytes()
            .expect("original encoding"),
        changed.canonical_review_bytes().expect("changed encoding"),
        "changing the result's borrow relationship must alter canonical review evidence",
    );
}

#[test]
fn review_projects_exact_public_callable_conformances_and_rejects_unrepresented_forms() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
"#;
    let satisfying = TempPackage::new();
    satisfying.write(
        "main.omg",
        r#"pub trait Handler<Element> { machine handle(value: Element) -> Element; }
pub machine handle(value: u32) -> u32 satisfies Handler<u32>::handle { value }
"#,
    );
    satisfying.write("build.omg", build);
    let checked = compile_to_checked_with_packages(
        &satisfying.0.join("main.omg"),
        Some(target),
        package_inputs(&satisfying.0),
    )
    .expect("public satisfier fixture should check");
    let review = project_checked_package_review(&checked).expect("public conformance review");
    let handle = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("handle"))
        .expect("public handle row");
    let [conformance] = handle.conformances() else {
        panic!("one exact public callable conformance")
    };
    assert_eq!(conformance.trait_identity().path(), "Handler");
    assert!(conformance.requirement_identity().path().contains("handle"));
    assert_eq!(conformance.arguments().len(), 1);
    assert_eq!(conformance.alias(), None);

    let hidden = TempPackage::new();
    hidden.write(
        "main.omg",
        r#"trait Hidden { machine handle(); }
pub machine handle() satisfies Hidden::handle { }
"#,
    );
    hidden.write("build.omg", build);
    let checked = compile_to_checked_with_packages(
        &hidden.0.join("main.omg"),
        Some(target),
        package_inputs(&hidden.0),
    )
    .expect("private-trait satisfier fixture should check");
    let diagnostics = project_checked_package_review(&checked).unwrap_err();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("realizes non-public trait `Hidden`")
    }));

    let generic = TempPackage::new();
    generic.write(
        "main.omg",
        r#"pub machine register<machine Selected>()
where machine Selected();
{ }
"#,
    );
    generic.write("build.omg", build);
    let checked = compile_to_checked_with_packages(
        &generic.0.join("main.omg"),
        Some(target),
        package_inputs(&generic.0),
    )
    .expect("public static-machine fixture should check");
    let diagnostics = project_checked_package_review(&checked).unwrap_err();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("conformance bounds not yet represented")
            || diagnostic
                .message
                .contains("static machine or proposition parameter not yet represented")
    }));
}

#[test]
fn review_projects_alpha_normalized_public_conformance_binders() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
"#;
    let original = TempPackage::new();
    original.write(
        "main.omg",
        r#"pub trait Ranked<Metric> { }
pub trait Alternate<Metric> { }
pub trait Ordering<Element, Other, Evidence: Element satisfies Ranked<u32>> { }
pub machine identity<Element, Other, Evidence: Element satisfies Ranked<u32>>(value: Element) -> Element {
    value
}
"#,
    );
    original.write("build.omg", build);
    let renamed = TempPackage::new();
    renamed.write(
        "main.omg",
        r#"pub trait Ranked<Measure> { }
pub trait Alternate<Measure> { }
pub trait Ordering<Value, Unused, OrderingEvidence: Value satisfies Ranked<u32>> { }
pub machine identity<Value, Unused, IdentityEvidence: Value satisfies Ranked<u32>>(value: Value) -> Value {
    value
}
"#,
    );
    renamed.write("build.omg", build);
    let changed = TempPackage::new();
    changed.write(
        "main.omg",
        r#"pub trait Ranked<Metric> { }
pub trait Alternate<Metric> { }
pub trait Ordering<Element, Other, Evidence: Element satisfies Alternate<u32>> { }
pub machine identity<Element, Other, Evidence: Element satisfies Alternate<u32>>(value: Element) -> Element {
    value
}
"#,
    );
    changed.write("build.omg", build);
    let changed_subject = TempPackage::new();
    changed_subject.write(
        "main.omg",
        r#"pub trait Ranked<Metric> { }
pub trait Alternate<Metric> { }
pub trait Ordering<Element, Other, Evidence: Other satisfies Ranked<u32>> { }
pub machine identity<Element, Other, Evidence: Other satisfies Ranked<u32>>(value: Element) -> Element {
    value
}
"#,
    );
    changed_subject.write("build.omg", build);
    let changed_argument = TempPackage::new();
    changed_argument.write(
        "main.omg",
        r#"pub trait Ranked<Metric> { }
pub trait Alternate<Metric> { }
pub trait Ordering<Element, Other, Evidence: Element satisfies Ranked<u64>> { }
pub machine identity<Element, Other, Evidence: Element satisfies Ranked<u64>>(value: Element) -> Element {
    value
}
"#,
    );
    changed_argument.write("build.omg", build);

    let review = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("generic conformance-binder fixture should check");
        project_checked_package_review(&checked)
            .expect("generic conformance-binder review should close")
    };
    let original = review(&original);
    let renamed = review(&renamed);
    let changed = review(&changed);
    let changed_subject = review(&changed_subject);
    let changed_argument = review(&changed_argument);

    let ordering = original
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Ordering")
        .expect("public Ordering row");
    let [trait_bound] = ordering.conformance_bounds() else {
        panic!("one exact trait conformance binder")
    };
    assert_eq!(trait_bound.binder_ordinal(), Some(0));
    assert_eq!(trait_bound.subject_parameter(), 0);
    assert_eq!(trait_bound.trait_identity().path(), "Ranked");
    assert_eq!(trait_bound.arguments().len(), 1);

    let identity = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("identity"))
        .expect("public identity row");
    let [callable_bound] = identity.conformance_bounds() else {
        panic!("one exact callable conformance binder")
    };
    assert_eq!(callable_bound.binder_ordinal(), Some(0));
    assert_eq!(callable_bound.subject_parameter(), 0);
    assert_eq!(callable_bound.trait_identity().path(), "Ranked");

    assert_eq!(
        original.canonical_review_bytes().unwrap(),
        renamed.canonical_review_bytes().unwrap(),
        "renaming conformance, type, and lifetime-free evidence binders must not change review identity"
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed.canonical_review_bytes().unwrap(),
        "changing the exact conformance trait must change review identity"
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_subject.canonical_review_bytes().unwrap(),
        "changing the conformance subject parameter must change review identity"
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_argument.canonical_review_bytes().unwrap(),
        "changing a conformance trait argument must change review identity"
    );
}

#[test]
fn review_projects_binder_free_conformance_requirements_without_fabricating_evidence() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait Ranked { }
pub trait Constraint<Element>
where Element satisfies Ranked
{ }
pub machine identity<Element>(value: Element) -> Element
where Element satisfies Ranked
{
    value
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("unbound conformance-requirement fixture should check before review");
    let review = project_checked_package_review(&checked)
        .expect("binder-free conformance requirement must project exactly");
    let identity = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("identity"))
        .expect("public identity row");
    let [bound] = identity.conformance_bounds() else {
        panic!("one exact binder-free conformance requirement")
    };
    assert_eq!(bound.binder_ordinal(), None);
    assert_eq!(bound.subject_parameter(), 0);
    assert_eq!(bound.trait_identity().path(), "Ranked");
    let constraint = review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Constraint")
        .expect("public Constraint row");
    let [trait_bound] = constraint.conformance_bounds() else {
        panic!("one exact trait binder-free conformance requirement")
    };
    assert_eq!(trait_bound.binder_ordinal(), None);
    assert_eq!(trait_bound.subject_parameter(), 0);
    assert_eq!(trait_bound.trait_identity().path(), "Ranked");
    assert!(!review.canonical_review_bytes().unwrap().is_empty());
}

#[test]
fn review_projects_exact_selected_conformance_carrier_trait_and_arguments() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait Marker<Tag> { }
pub data Tag { }
pub data Good { }
Primary: Good satisfies Marker<Tag>;
pub machine accept<Element>(value: &Element)
where Element satisfies Good::Primary
{ }
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("selected-conformance fixture should check before review");
    let review = project_checked_package_review(&checked)
        .expect("exact non-generic selected conformance should project");
    let accept = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("accept"))
        .expect("public accept row");
    let [bound] = accept.conformance_bounds() else {
        panic!("one exact selected conformance requirement")
    };
    assert_eq!(bound.binder_ordinal(), None);
    assert_eq!(bound.subject_parameter(), 0);
    assert_eq!(
        bound
            .selected_conformance()
            .expect("selected conformance")
            .path(),
        "Primary"
    );
    assert_eq!(
        bound.selected_carrier().expect("selected carrier").path(),
        "Good"
    );
    assert!(bound.selected_carrier_arguments().is_empty());
    assert_eq!(bound.trait_identity().path(), "Marker");
    assert_eq!(bound.arguments().len(), 1);
    assert!(bound.arguments()[0].canonical().contains("Tag"));
    assert!(!review.canonical_review_bytes().unwrap().is_empty());
}

#[test]
fn public_machine_visibility_survives_checked_compilation_and_strict_empty_contracts() {
    let package = TempPackage::new();
    package.write("main.omg", "pub machine Package::entry() { }\n");
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("public machine should check");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Package::entry")
        .expect("checked public machine");
    assert!(machine.is_public);
    assert_eq!(
        machine.supply_mode,
        psi_language_semantics::MachineSupplyMode::CheckedBody
    );

    let service = checked
        .facts
        .service_reaches
        .for_machine(machine.symbol)
        .expect("checked service contract");
    assert!(matches!(
        service.interface,
        psi_language_semantics::ServiceReachInterface::PublishedCeiling(_)
    ));
    let invocation = checked
        .facts
        .synchronous_invocations
        .for_machine(machine.symbol)
        .expect("checked invocation contract");
    assert_eq!(
        invocation.interface,
        psi_language_semantics::SynchronousInvocationInterface::PublishedCeiling
    );
    assert!(matches!(
        checked
            .facts
            .suspensions
            .for_machine(machine.symbol)
            .expect("checked suspension contract")
            .interface,
        psi_language_semantics::SuspensionInterface::PublishedMaySuspend(false)
    ));
    assert!(matches!(
        checked
            .facts
            .blocking
            .for_machine(machine.symbol)
            .expect("checked blocking contract")
            .interface,
        psi_language_semantics::BlockingInterface::PublishedMayBlock(false)
    ));
    assert_eq!(
        checked
            .facts
            .contract_plans
            .for_machine(machine.symbol)
            .expect("checked contract")
            .crash
            .interface(),
        psi_checked_trees::CrashInterface::PublishedCeiling
    );
}

#[test]
fn public_machine_cannot_hide_realized_reach_invocation_or_operational_effects() {
    let cases = [
        (
            "invocation",
            r#"boundary trait Handler { machine handle(); }
pub machine public_api(handler: &mut Handler) { handler.handle(); }
"#,
            &["omits `invokes handler;`"][..],
        ),
        (
            "operational",
            r#"boundary trait Waiting { machine wait() reaches Waiting suspends; blocks; }
pub machine public_api(waiting: &mut Waiting)
reaches Waiting
invokes waiting;
{
    suspend block waiting.wait();
}
"#,
            &["omits `suspends;`", "omits `blocks;`"][..],
        ),
        (
            "crash",
            r#"pub machine public_api() { crash Abort; }
"#,
            &["crash"][..],
        ),
    ];

    for (label, source, expected_messages) in cases {
        let package = TempPackage::new();
        package.write("main.omg", source);
        let diagnostics = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            None,
            package_inputs(&package.0),
        )
        .unwrap_err();
        for expected in expected_messages {
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(expected)),
                "{label} omission should mention `{expected}`: {diagnostics:#?}"
            );
        }
    }
}

#[test]
fn exact_synchronous_invocations_change_comparison_encoding() {
    let quiet = TempPackage::new();
    let invoking = TempPackage::new();
    quiet.write(
        "main.omg",
        r#"boundary trait Handler { machine handle(); }
boundary trait Host { machine ping() reaches Host; }
pub machine dispatch(handler: &mut Handler)
reaches Host
invokes handler;
invokes Host;
{ }
"#,
    );
    invoking.write(
        "main.omg",
        r#"boundary trait Handler { machine handle(); }
boundary trait Host { machine ping() reaches Host; }
pub machine dispatch(handler: &mut Handler)
invokes handler;
invokes Host;
{
    handler.handle();
    Host::ping();
}
"#,
    );
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#;
    quiet.write("build.omg", build);
    invoking.write("build.omg", build);

    let compile = |package: &TempPackage| {
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("invocation comparison fixture should check")
    };
    let quiet = project_checked_package_review(&compile(&quiet)).expect("quiet review");
    let invoking = project_checked_package_review(&compile(&invoking)).expect("invoking review");
    let dispatch = invoking
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Public)
        .expect("public dispatch row");
    let quiet_dispatch = quiet
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Public)
        .expect("quiet public dispatch row");
    let declared = dispatch
        .declared_synchronous_invocations()
        .expect("published invocation ceiling");
    assert_eq!(declared.len(), 2);
    assert_eq!(
        declared[0],
        PackageReviewSynchronousInvocation::Parameter(0)
    );
    let PackageReviewSynchronousInvocation::Service(service) = &declared[1] else {
        panic!("second exact invocation should be a service identity")
    };
    assert_eq!(service.path(), "Host");
    assert_eq!(
        service.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(
        quiet_dispatch.declared_synchronous_invocations(),
        Some(declared)
    );
    assert!(quiet_dispatch.realized_synchronous_invocations().is_empty());
    assert_eq!(quiet_dispatch.contracts(), dispatch.contracts());
    assert_eq!(dispatch.realized_synchronous_invocations(), declared);
    assert_ne!(
        quiet.canonical_review_bytes().expect("quiet encoding"),
        invoking
            .canonical_review_bytes()
            .expect("invoking encoding")
    );
}

#[test]
fn review_rejects_target_free_and_standalone_checked_programs() {
    let package = TempPackage::new();
    package.write("main.omg", "machine local() { }\n");

    let target_free = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        None,
        package_inputs(&package.0),
    )
    .expect("target-free package fixture should check");
    let diagnostics = project_checked_package_review(&target_free)
        .expect_err("review must require an explicit target");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires one explicit target selection")
    }));

    let standalone = omega_compiler::compile_to_checked(&package.0.join("main.omg"), None)
        .expect("standalone fixture should check");
    let diagnostics = project_checked_package_review(&standalone)
        .expect_err("review must require package-aware compilation");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires package-aware checked compilation")
    }));
}

#[test]
fn review_distinguishes_profiles_that_share_a_native_target() {
    let package = TempPackage::new();
    package.write("main.omg", "machine local() { }\n");
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target uefi_x64 { }
machine build(builder: &mut Build) { }
"#,
    );

    let windows = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("Windows review fixture should check");
    let uefi = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("uefi_x64"),
        package_inputs(&package.0),
    )
    .expect("UEFI review fixture should check");

    assert_eq!(
        windows.selected_native_target(),
        uefi.selected_native_target()
    );
    let windows = project_checked_package_review(&windows).expect("Windows review projection");
    let uefi = project_checked_package_review(&uefi).expect("UEFI review projection");
    assert_eq!(windows.target(), omega_target::TargetProfile::WindowsX64);
    assert_eq!(uefi.target(), omega_target::TargetProfile::UefiX64);
    assert_ne!(windows.target(), uefi.target());
    assert_ne!(
        windows.canonical_review_bytes().expect("Windows encoding"),
        uefi.canonical_review_bytes().expect("UEFI encoding"),
    );
}

#[test]
fn review_encoding_ignores_unreviewed_arena_insertion_order() {
    let first = TempPackage::new();
    let second = TempPackage::new();
    first.write("main.omg", "boundary machine host_ping();\n");
    second.write(
        "main.omg",
        "machine unrelated() { }\nboundary machine host_ping();\n",
    );
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#;
    first.write("build.omg", build);
    second.write("build.omg", build);

    let compile = |package: &TempPackage| {
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("arena-order fixture should check")
    };
    let first = project_checked_package_review(&compile(&first))
        .expect("first arena-order review")
        .canonical_review_bytes()
        .expect("first arena-order encoding");
    let second = project_checked_package_review(&compile(&second))
        .expect("second arena-order review")
        .canonical_review_bytes()
        .expect("second arena-order encoding");

    assert_eq!(first, second);
}

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
machine build(builder: &mut Build) { }
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
machine build(builder: &mut Build) { }
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
machine build(builder: &mut Build) { }
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
machine build(builder: &mut Build) { }
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
    assert_eq!(
        route.requirement_identity().path(),
        "SchedulerAdmission::grant"
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
machine build(builder: &mut Build) { }
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
machine build(builder: &mut Build) { }
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
            .map(|atom| atom.path())
            .collect::<Vec<_>>(),
        ["Socket::Authenticated", "Socket::Connected"]
    );
    assert!(usable_atoms.iter().all(|atom| {
        matches!(
            atom.owner(),
            PackageReviewNominalOwner::Package(identity) if identity == package_identity()
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
    assert_eq!(portable_atoms.len(), 4);
    assert!(portable_atoms.iter().all(|atom| {
        matches!(atom.owner(), PackageReviewNominalOwner::ToolchainUnbound)
            && atom.path().starts_with("Carry::")
    }));

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
    first.write(
        "main.omg",
        r#"pub trait Parent<Element> {
    operator < compare(left: Element, right: Element) -> bool;
}
pub boundary trait Service<Element>: Parent<Element> {
    machine Self::exchange(&mut self, item: Element) -> Element;
}
"#,
    );
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
machine build(builder: &mut Build) { }
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
    let [exchange] = service.requirements() else {
        panic!("one exact requirement row")
    };
    assert_eq!(exchange.identity().path(), "Service::exchange");
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
machine build(builder: &mut Build) { }
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
            "target windows_x64 { }\nmachine build(builder: &mut Build) { }\n",
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
fn review_rejects_public_data_semantics_that_lack_canonical_rows() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Ledger
where
    count <= len,
{
    len: u32;
    count: u32;
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("public data fact fixture should check");
    let diagnostics = project_checked_package_review(&checked)
        .expect_err("review must not silently omit public data facts");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("uses proof facts not yet represented by package review")
    }));
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
machine build(builder: &mut Build) { }
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
            operator,
            left,
            right,
        }),
    ] = domain.predicate_facts()
    else {
        panic!("one binary domain predicate fact")
    };
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
            "target windows_x64 { }\nmachine build(builder: &mut Build) { }\n",
        );
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
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
        .origin = psi_facts::FactOrigin::Unknown;
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
        .clone_from(&psi_facts::PlaceSegment::Field {
            symbol: psi_symbols::SymbolHandle::invalid(),
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
        "target windows_x64 { }\nmachine build(builder: &mut Build) { }\n",
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
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
        "target windows_x64 { }\nmachine build(builder: &mut Build) { }\n",
    );
    let checked = compile_to_checked_with_packages(
        &private.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&private.0),
    )
    .expect("private predicate target fixture should check");
    let diagnostics = project_checked_package_review(&checked)
        .expect_err("public predicate must not expose a package-private domain");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("exposes non-public domain") })
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
    let build = "target windows_x64 { }\nmachine build(builder: &mut Build) { }\n";
    first.write("build.omg", build);
    reordered.write("build.omg", build);
    changed.write("build.omg", build);

    let encode = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
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

#[test]
fn review_projects_callable_domain_predicates_through_exact_checked_selection() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Reading {
    value: i64;
    minimum: i64;
    maximum: i64;
}
pub machine within_calibration(reading: Reading) -> bool {
    reading.value >= reading.minimum && reading.value <= reading.maximum
}
pub domain Reading::Calibrated
requires
    within_calibration(self);
"#,
    );
    package.write(
        "build.omg",
        "target windows_x64 { }\nmachine build(builder: &mut Build) { }\n",
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("callable domain predicate fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("simple checked callable predicates have an exact review row");
    let [domain] = review.public_domains() else {
        panic!("one public domain row")
    };
    let [
        PackageReviewContractFact::Expression(PackageReviewContractExpression::Call {
            receiver,
            target,
            arguments,
        }),
    ] = domain.predicate_facts()
    else {
        panic!("one callable domain predicate")
    };
    assert!(receiver.is_none());
    assert_eq!(target.path(), "within_calibration::entry");
    assert_eq!(arguments, &[PackageReviewContractExpression::DomainSubject]);
}

#[test]
fn callable_domain_predicate_review_rejects_checked_target_spoof() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Reading { value: i64; }
pub machine is_zero(reading: Reading) -> bool { reading.value == 0 }
pub domain Reading::Zero requires is_zero(self);
"#,
    );
    package.write(
        "build.omg",
        "target windows_x64 { }\nmachine build(builder: &mut Build) { }\n",
    );
    let mut checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("callable predicate spoof fixture should check");
    let call_expression = checked
        .expression_table
        .iter_expressions()
        .find_map(|(expression, node)| {
            matches!(node, psi_typed_trees::expression::ExpressionNode::Call(_))
                .then_some(expression)
        })
        .expect("domain predicate call expression");
    let psi_typed_trees::expression::ExpressionNode::Call(call) = checked
        .typed
        .expression_table
        .expression_mut(call_expression)
    else {
        panic!("call expression")
    };
    call.target_symbol = psi_symbols::SymbolHandle::invalid();

    let diagnostics = project_checked_package_review(&checked)
        .expect_err("a typed call target cannot diverge from checked selection custody");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("target disagrees with its exact checked call-selection row")
    }));
}

#[test]
fn public_trait_operational_envelope_is_exact_review_shape() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub boundary trait Console { }
pub boundary trait Worker {
    machine wait(handler: &mut Console)
    reaches <= Console
    invokes handler;
    invokes Console;
    suspends;
    blocks;
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("public trait suspension fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("public trait operational review should close");
    let worker = review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Worker")
        .expect("worker trait row");
    let [wait] = worker.requirements() else {
        panic!("one worker requirement")
    };
    let [console] = wait.service_reach() else {
        panic!("one exact service-reach row")
    };
    assert_eq!(console.path(), "Console");
    assert_eq!(
        console.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert!(wait.service_reach_is_installation_bound());
    assert_eq!(wait.synchronous_invocations().len(), 2);
    assert_eq!(wait.synchronous_invocations()[0].parameter(), Some(0));
    assert_eq!(
        wait.synchronous_invocations()[1]
            .service()
            .expect("service invocation")
            .path(),
        "Console"
    );
    assert!(wait.suspends());
    assert!(wait.blocks());
}

#[test]
fn public_trait_termination_is_parameter_rooted_review_shape() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub boundary trait SchedulerRuntime {
    machine wait(&self, scheduler: SchedulerRuntime)
    requires self in WeakFair
    requires scheduler in WeakFair
    terminates;
}
pub domain SchedulerRuntime::WeakFair
satisfies ProgressProfile
established by SchedulerAdmission::grant;
pub boundary trait SchedulerAdmission {
    machine grant(scheduler: SchedulerRuntime) -> SchedulerRuntime in WeakFair;
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("public trait termination fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("public trait termination review should close");
    let runtime = review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "SchedulerRuntime")
        .expect("scheduler runtime trait row");
    let wait = runtime
        .requirements()
        .iter()
        .find(|requirement| requirement.identity().path() == "SchedulerRuntime::wait")
        .expect("wait requirement row");
    let premises = wait
        .termination()
        .premises()
        .expect("wait must promise termination");
    assert_eq!(premises.len(), 2);
    for premise in premises {
        assert_eq!(premise.profile().path(), "SchedulerRuntime::WeakFair");
        assert_eq!(
            premise.profile().owner(),
            PackageReviewNominalOwner::Package(package_identity())
        );
        assert!(premise.projections().is_empty());
    }
    assert!(premises[0].subject().is_receiver());
    assert_eq!(premises[1].subject().parameter(), Some(0));
}

#[test]
fn public_trait_termination_rejects_a_non_public_progress_profile() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data SchedulerHandle { }
domain SchedulerHandle::WeakFair
satisfies ProgressProfile
established by SchedulerAdmission::grant;
pub boundary trait SchedulerAdmission {
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
}
pub boundary trait SchedulerRuntime {
    machine wait(scheduler: SchedulerHandle)
    requires scheduler in WeakFair
    terminates;
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("private progress profile fixture should check");
    let diagnostics = project_checked_package_review(&checked)
        .expect_err("review must reject a private profile in a public trait contract");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("exposes non-public progress profile")
    }));
}

#[test]
fn review_projects_trait_defaults_and_unnamed_contracts() {
    let default_package = TempPackage::new();
    default_package.write(
        "main.omg",
        r#"pub trait Worker {
    machine wait() { }
}
"#,
    );
    default_package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &default_package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&default_package.0),
    )
    .expect("public trait default fixture should check");
    let default_review = project_checked_package_review(&checked)
        .expect("review should retain a public trait default realization");
    let default_requirement = &default_review.public_traits()[0].requirements()[0];
    assert!(default_requirement.has_default_realization());

    let abstract_package = TempPackage::new();
    abstract_package.write(
        "main.omg",
        r#"pub trait Worker {
    machine wait();
}
"#,
    );
    abstract_package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#,
    );
    let abstract_checked = compile_to_checked_with_packages(
        &abstract_package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&abstract_package.0),
    )
    .expect("abstract public trait fixture should check");
    let abstract_review = project_checked_package_review(&abstract_checked)
        .expect("review should retain an abstract public trait requirement");
    assert!(!abstract_review.public_traits()[0].requirements()[0].has_default_realization());
    assert_ne!(
        default_review.canonical_review_bytes().unwrap(),
        abstract_review.canonical_review_bytes().unwrap(),
    );

    let precondition_package = TempPackage::new();
    precondition_package.write(
        "main.omg",
        r#"pub data SchedulerHandle { }
pub domain SchedulerHandle::WeakFair
satisfies ProgressProfile
established by SchedulerAdmission::grant;
pub boundary trait SchedulerAdmission {
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
}
pub boundary trait SchedulerRuntime {
    machine wait(scheduler: SchedulerHandle)
    requires scheduler in WeakFair;
}
"#,
    );
    precondition_package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &precondition_package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&precondition_package.0),
    )
    .expect("public progress precondition fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("an unnamed trait precondition should project exactly");
    let runtime = review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "SchedulerRuntime")
        .expect("scheduler runtime trait row");
    let [wait] = runtime.requirements() else {
        panic!("one scheduler requirement")
    };
    let [contract] = wait.contracts() else {
        panic!("one exact trait contract")
    };
    assert_eq!(contract.kind(), PackageReviewContractKind::Requires);
    assert_eq!(contract.binding(), None);
    let PackageReviewContractFact::Membership { value, domain } = contract.fact() else {
        panic!("trait precondition must retain exact membership")
    };
    assert_eq!(value, &PackageReviewContractExpression::Parameter(0));
    assert_eq!(domain.path(), "SchedulerHandle::WeakFair");
    assert_eq!(
        domain.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
}

#[test]
fn public_trait_requires_and_ensures_change_comparison_identity() {
    let project = |minimum: u8| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                "pub trait Bounds {{\n    machine clamp(value: u64) -> u64\n    requires value >= {minimum}\n    ensures result >= value;\n}}\n"
            ),
        );
        package.write(
            "build.omg",
            r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#,
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("public trait contract fixture should check");
        project_checked_package_review(&checked).expect("public trait contracts should project")
    };

    let first = project(1);
    let changed = project(2);
    let [bounds] = first.public_traits() else {
        panic!("one public trait")
    };
    let [clamp] = bounds.requirements() else {
        panic!("one public trait requirement")
    };
    assert_eq!(clamp.contracts().len(), 2);
    assert!(
        clamp
            .contracts()
            .iter()
            .any(|contract| contract.kind() == PackageReviewContractKind::Requires)
    );
    assert!(
        clamp
            .contracts()
            .iter()
            .any(|contract| contract.kind() == PackageReviewContractKind::Ensures)
    );
    assert_ne!(
        first.canonical_review_bytes().unwrap(),
        changed.canonical_review_bytes().unwrap(),
        "changing a public trait contract must change comparison identity"
    );
}

#[test]
fn public_trait_named_witness_contracts_retain_exact_lanes_and_selector_identity() {
    let project = |requires_binding: &str, ensures_binding: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"pub trait Evidence {{
    machine witness();
}}
proposition ready() evidence Evidence;
pub trait Worker {{
    machine relay(value: i32) -> i32
    requires {requires_binding}: ready()
    ensures {ensures_binding}: ready()
    {{
        {ensures_binding} = {requires_binding};
    }}
}}
"#,
            ),
        );
        package.write(
            "build.omg",
            r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#,
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("named public-trait witness fixture should check");
        project_checked_package_review(&checked)
            .expect("named public-trait witness contracts should project")
    };

    let original = project("input_proof", "output_proof");
    let renamed_requires = project("renamed_local_input", "output_proof");
    let renamed_ensures = project("input_proof", "renamed_public_output");
    let worker = original
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Worker")
        .expect("worker trait row");
    let [relay] = worker.requirements() else {
        panic!("one worker requirement")
    };
    assert_eq!(relay.contracts().len(), 2);
    for contract in relay.contracts() {
        assert!(contract.evidence_lane_position().is_some());
        let PackageReviewContractFact::Proposition(application) = contract.fact() else {
            panic!("named witness contract must retain proposition identity")
        };
        assert_eq!(application.declaration().path(), "ready");
        let PackageReviewPropositionEvidence::Witness(interface) = application.evidence() else {
            panic!("named witness contract must retain its evidence interface")
        };
        assert_eq!(interface.trait_identity().path(), "Evidence");
        assert_eq!(interface.requirements().len(), 1);
    }
    let requires = relay
        .contracts()
        .iter()
        .find(|contract| contract.kind() == PackageReviewContractKind::Requires)
        .expect("named requires row");
    let ensures = relay
        .contracts()
        .iter()
        .find(|contract| contract.kind() == PackageReviewContractKind::Ensures)
        .expect("named ensures row");
    assert_eq!(requires.binding(), None, "requires binding is local");
    assert_eq!(ensures.binding(), Some("output_proof"));
    assert_eq!(
        original.canonical_review_bytes().unwrap(),
        renamed_requires.canonical_review_bytes().unwrap(),
        "renaming a local requires evidence alias must preserve review identity",
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        renamed_ensures.canonical_review_bytes().unwrap(),
        "renaming a public ensures selector must change review identity",
    );
}

#[test]
fn public_trait_member_contracts_join_exact_state_signature_places() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Threshold { minimum: u64; }
pub trait Bounds {
    machine accepts(threshold: Threshold, value: u64)
    requires value >= threshold.minimum;
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("public trait member contract should check");
    let review = project_checked_package_review(&checked)
        .expect("public trait member contract should join checked places");
    let [bounds] = review.public_traits() else {
        panic!("one public trait")
    };
    let [accepts] = bounds.requirements() else {
        panic!("one public trait requirement")
    };
    let [contract] = accepts.contracts() else {
        panic!("one public trait contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        right,
        ..
    }) = contract.fact()
    else {
        panic!("one exact binary contract expression")
    };
    let PackageReviewContractExpression::Member {
        receiver, member, ..
    } = right.as_ref()
    else {
        panic!("right operand must retain the exact field place")
    };
    assert_eq!(
        receiver.as_ref(),
        &PackageReviewContractExpression::Parameter(0)
    );
    assert_eq!(member.path(), "Threshold::minimum");
    assert_eq!(
        member.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
}

#[test]
fn public_trait_crash_ceilings_are_exact_canonical_checked_routes() {
    let project = |trap_guard: &str, stop_cause: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                "pub trait Worker {{\n    machine run(flag: bool)\n    crashes Trap {trap_guard};\n    machine stop() crashes {stop_cause};\n    machine idle();\n}}\n"
            ),
        );
        package.write(
            "build.omg",
            r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#,
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("public trait crash ceiling should check");
        project_checked_package_review(&checked)
            .expect("public trait crash ceiling should project from its exact checked capsule")
    };

    let first = project("flag", "Abort");
    let guard_changed = project("!flag", "Abort");
    let cause_changed = project("flag", "Trap");
    let worker = first
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Worker")
        .expect("worker trait row");
    let run = worker
        .requirements()
        .iter()
        .find(|requirement| requirement.identity().path() == "Worker::run")
        .expect("run requirement row");
    let [trap] = run.published_crash() else {
        panic!("one guarded trap route")
    };
    assert_eq!(trap.cause(), psi_checked_trees::CrashCause::Trap);
    let [PackageReviewCrashRouteGuard::Predicate(predicate)] = trap.alternative_guards() else {
        panic!("trap route must retain one canonical predicate guard")
    };
    assert!(!predicate.canonical_bytes().is_empty());
    let stop = worker
        .requirements()
        .iter()
        .find(|requirement| requirement.identity().path() == "Worker::stop")
        .expect("stop requirement row");
    let [abort] = stop.published_crash() else {
        panic!("one unconditional abort route")
    };
    assert_eq!(abort.cause(), psi_checked_trees::CrashCause::Abort);
    assert_eq!(
        abort.alternative_guards(),
        [PackageReviewCrashRouteGuard::Truth]
    );
    let idle = worker
        .requirements()
        .iter()
        .find(|requirement| requirement.identity().path() == "Worker::idle")
        .expect("idle requirement row");
    assert!(idle.published_crash().is_empty());
    assert_ne!(
        first.canonical_review_bytes().unwrap(),
        guard_changed.canonical_review_bytes().unwrap(),
        "changing a crash guard must change package comparison identity"
    );
    assert_ne!(
        first.canonical_review_bytes().unwrap(),
        cause_changed.canonical_review_bytes().unwrap(),
        "changing a crash cause must change package comparison identity"
    );
}

#[test]
fn public_trait_crash_projection_rejects_missing_or_duplicate_checked_capsules() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub trait Worker {
    machine run() crashes Trap;
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("public trait crash fixture should check");

    let mut missing = checked.clone();
    missing.facts.contract_plans.crash_capsules.clear();
    let diagnostics = project_checked_package_review(&missing)
        .expect_err("missing checked crash capsule must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("has no exact checked crash capsule")
    }));

    let mut duplicate = checked;
    let capsule = duplicate.facts.contract_plans.crash_capsules[0].clone();
    duplicate.facts.contract_plans.crash_capsules.push(capsule);
    let diagnostics = project_checked_package_review(&duplicate)
        .expect_err("duplicate checked crash capsules must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("has duplicate checked crash capsules")
    }));
}

#[test]
fn public_trait_contract_calls_use_the_same_checked_projection() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"machine computed_zero() -> u64 { 0 }
pub trait Worker {
    machine wait() -> u64
    ensures result == computed_zero();
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("public trait contract call fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("public trait contract calls use the checked call row");
    let [trait_shape] = review.public_traits() else {
        panic!("one public trait")
    };
    let [requirement] = trait_shape.requirements() else {
        panic!("one trait requirement")
    };
    let [contract] = requirement.contracts() else {
        panic!("one trait requirement contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        right,
        ..
    }) = contract.fact()
    else {
        panic!("binary trait guarantee")
    };
    assert!(matches!(
        right.as_ref(),
        PackageReviewContractExpression::Call { target, .. }
            if target.path() == "computed_zero::entry"
    ));
}

#[test]
fn review_rejects_contract_entailment_stand_downs() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"machine unchecked_claim(a: u64, b: u64)
requires
    min(a, b) >= 1
ensures
    a >= 1
{
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("ordinary checking should retain the out-of-language stand-down");
    let [stand_down] = checked.contract_entailment_stand_downs() else {
        panic!("one exact contract-entailment stand-down")
    };
    assert_eq!(stand_down.contract_index, 1);
    assert_eq!(stand_down.fact_index, 0);
    assert_eq!(
        stand_down.reason,
        psi_validation::ContractEntailmentStandDownReason::OutsideEntailmentLanguage
    );

    let diagnostics = project_checked_package_review(&checked)
        .expect_err("package review must fail closed on an unresolved stand-down");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rejects unresolved contract-entailment stand-down")
    }));
}

fn host_target_name() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Some("windows_x64"),
        ("linux", "x86_64") => Some("linux_x64"),
        ("linux", "aarch64") => Some("linux_arm64"),
        ("macos", "aarch64") => Some("macos_arm64"),
        _ => None,
    }
}
