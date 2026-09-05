use super::*;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempTree(PathBuf);

impl TempTree {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-package-compilation-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create temporary package tree");
        Self(path)
    }

    fn package(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir(&path).expect("create package root");
        path
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn identity(marker: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([marker; 32]).expect("nonzero package identity")
}

fn accepted_console_binding(package: PackageKeyIdentity, marker: u8) -> AcceptedSemanticBinding {
    AcceptedSemanticBinding::new(
        AcceptedSemanticBindingRole::ConsoleExitProcessI32,
        package,
        "Console",
        effects::provider_plan::ServiceSchemaDigest::from_digest([marker; 32]),
        effects::provider_plan::ProviderPlanDigest::from_digest([marker.wrapping_add(1); 32]),
    )
    .expect("valid accepted semantic binding")
}

#[test]
fn accepted_semantic_bindings_are_unique_and_closed_over_the_package_graph() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root)],
        Vec::new(),
    )
    .expect("root-only graph");

    let accepted = inputs
        .clone()
        .with_accepted_semantic_bindings(vec![accepted_console_binding(identity(1), 7)])
        .expect("binding to the exact root package should attach");
    assert_eq!(
        accepted
            .accepted_semantic_binding(AcceptedSemanticBindingRole::ConsoleExitProcessI32)
            .expect("accepted Console binding")
            .package(),
        identity(1),
    );

    let duplicate = inputs
        .clone()
        .with_accepted_semantic_bindings(vec![
            accepted_console_binding(identity(1), 7),
            accepted_console_binding(identity(1), 8),
        ])
        .expect_err("one consumer role cannot accept two declarations");
    assert!(matches!(
        duplicate.as_slice(),
        [PackageCompilationInputError::DuplicateSemanticBindingRole {
            role: AcceptedSemanticBindingRole::ConsoleExitProcessI32,
        }]
    ));

    let foreign = inputs
        .with_accepted_semantic_bindings(vec![accepted_console_binding(identity(2), 7)])
        .expect_err("accepted binding cannot name a package outside the closure");
    assert!(matches!(
        foreign.as_slice(),
        [PackageCompilationInputError::ForeignSemanticBindingPackage {
            role: AcceptedSemanticBindingRole::ConsoleExitProcessI32,
            package,
        }] if *package == identity(2)
    ));
}

#[test]
fn accepted_semantic_binding_canonicalizes_explicit_terminal_permissions() {
    let schema = effects::provider_plan::ServiceSchemaDigest::from_digest([7; 32]);
    let permission = |requirement: &str, class| {
        effects::ServiceTerminalAuthorityPermission::new(
            schema,
            requirement,
            effects::TerminalAuthorityDisposition::from_classes([class]),
        )
    };
    let binding = accepted_console_binding(identity(1), 7)
        .with_terminal_authority_permissions(vec![
            permission(
                "Console::write_line#exact",
                effects::TerminalAuthorityClass::ProcessOutput,
            ),
            permission(
                "Console::exit_process#exact",
                effects::TerminalAuthorityClass::ProcessTermination,
            ),
        ])
        .expect("explicit rows share the accepted service schema");

    assert_eq!(binding.terminal_authority_permissions().len(), 2);
    assert_eq!(
        binding.terminal_authority_permissions()[0].requirement_identity(),
        "Console::exit_process#exact"
    );
    assert_eq!(
        binding.terminal_authority_permissions()[1].requirement_identity(),
        "Console::write_line#exact"
    );
}

#[test]
fn filesystem_binding_retains_explicit_portable_facet_permissions() {
    let schema = effects::provider_plan::ServiceSchemaDigest::from_digest([9; 32]);
    let binding = AcceptedSemanticBinding::new_service(
        AcceptedSemanticBindingRole::FilesystemHostService,
        identity(1),
        "FilesystemHost",
        schema,
    )
    .expect("filesystem service binding")
    .with_terminal_authority_permissions(vec![
        effects::ServiceTerminalAuthorityPermission::for_filesystem_facets(
            schema,
            "FilesystemHost::open#exact",
            [
                effects::PortableFilesystemAuthorityFacet::ContentRead,
                effects::PortableFilesystemAuthorityFacet::ContentWrite,
                effects::PortableFilesystemAuthorityFacet::MetadataQuery,
            ],
        ),
    ])
    .expect("explicit filesystem facets share the exact accepted schema");

    let [permission] = binding.terminal_authority_permissions() else {
        panic!("one exact filesystem permission must survive binding custody");
    };
    assert_eq!(
        permission.requirement_identity(),
        "FilesystemHost::open#exact"
    );
    assert_eq!(
        permission.permitted().classes(),
        &[
            effects::TerminalAuthorityClass::FilesystemContentRead,
            effects::TerminalAuthorityClass::FilesystemContentWrite,
            effects::TerminalAuthorityClass::FilesystemMetadataQuery,
        ]
    );
}

#[test]
fn accepted_semantic_binding_rejects_permission_schema_and_key_substitution() {
    let schema = effects::provider_plan::ServiceSchemaDigest::from_digest([7; 32]);
    let other_schema = effects::provider_plan::ServiceSchemaDigest::from_digest([8; 32]);
    let disposition = effects::TerminalAuthorityDisposition::from_classes([
        effects::TerminalAuthorityClass::ProcessTermination,
    ]);

    assert_eq!(
        accepted_console_binding(identity(1), 7)
            .with_terminal_authority_permissions(vec![
                effects::ServiceTerminalAuthorityPermission::new(
                    other_schema,
                    "Console::exit_process#exact",
                    disposition.clone(),
                ),
            ])
            .expect_err("permission cannot substitute another service schema"),
        "terminal-authority permission names a different service schema"
    );

    for requirement in ["", "Console::exit\nprocess"] {
        assert_eq!(
            accepted_console_binding(identity(1), 7)
                .with_terminal_authority_permissions(vec![
                    effects::ServiceTerminalAuthorityPermission::new(
                        schema,
                        requirement,
                        disposition.clone(),
                    ),
                ])
                .expect_err("invalid exact requirement identity must reject"),
            "terminal-authority permission has an invalid requirement identity"
        );
    }

    let duplicate = effects::ServiceTerminalAuthorityPermission::new(
        schema,
        "Console::exit_process#exact",
        disposition,
    );
    assert_eq!(
        accepted_console_binding(identity(1), 7)
            .with_terminal_authority_permissions(vec![duplicate.clone(), duplicate])
            .expect_err("permission key cannot be duplicated"),
        "terminal-authority permissions repeat an exact requirement"
    );
}

#[test]
fn compiler_inputs_retain_application_root_role_and_reject_workspace_role() {
    let tree = TempTree::new();
    let root = tree.package("application");
    let binding = PackageSourceBinding::new(identity(1), "application", root.clone());
    let inputs = PackageCompilationInputs::new(
        identity(1),
        BuildDeclarationKind::Application,
        vec![binding.clone()],
        vec![],
    )
    .expect("application root enters compiler handoff");

    assert_eq!(inputs.root_role(), BuildDeclarationKind::Application);
    assert_eq!(
        inputs.dependency_closure().root_role(),
        BuildDeclarationKind::Application
    );

    let errors = PackageCompilationInputs::new(
        identity(1),
        BuildDeclarationKind::Workspace,
        vec![binding],
        vec![],
    )
    .expect_err("workspace catalog cannot be a compilation root");
    assert!(matches!(
        errors.as_slice(),
        [PackageCompilationInputError::InvalidRootRole {
            role: BuildDeclarationKind::Workspace
        }]
    ));
}

#[test]
fn source_input_projection_excludes_exact_target_child_inputs() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let dependency = tree.package("dependency");
    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root),
            PackageSourceBinding::new(identity(2), "dependency", dependency),
        ],
        vec![PackageDependencyBinding::new(
            identity(1),
            "dependency",
            identity(2),
        )],
    )
    .expect("source graph should close");
    let expected = inputs.source_inputs();

    let with_binding = inputs
        .clone()
        .with_accepted_semantic_bindings(vec![accepted_console_binding(identity(2), 7)])
        .expect("exact-child semantic binding should attach");
    assert_eq!(with_binding.source_inputs(), expected);

    let generated = generated_bundle(
        &inputs,
        identity(2),
        target::TargetProfile::WindowsX64,
        9,
        vec![generated_source(
            b"generated.omg",
            b"pub machine generated() -> u64 { 1 }\n",
        )],
    );
    let with_generated = inputs
        .with_complete_dependency_generated_sources(vec![generated])
        .expect("exact-child generated source should attach");
    assert_eq!(with_generated.source_inputs(), expected);
}

#[test]
fn source_input_projection_binds_names_roots_metadata_and_edges() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let first_dependency = tree.package("first-dependency");
    let second_dependency = tree.package("second-dependency");
    fs::write(root.join("main.omg"), b"machine main() {}\n").expect("write root source");

    let inputs = |root_name: &str,
                  root_path: PathBuf,
                  dependency_path: PathBuf,
                  alias: &str,
                  with_metadata: bool| {
        let root_binding = PackageSourceBinding::new(identity(1), root_name, root_path);
        let root_binding = if with_metadata {
            root_binding
                .with_canonical_source_metadata()
                .expect("capture root source metadata")
        } else {
            root_binding
        };
        PackageCompilationInputs::new_package(
            identity(1),
            vec![
                root_binding,
                PackageSourceBinding::new(identity(2), "dependency", dependency_path),
            ],
            vec![PackageDependencyBinding::new(
                identity(1),
                alias,
                identity(2),
            )],
        )
        .expect("source graph should close")
        .source_inputs()
    };

    let baseline = inputs(
        "root",
        root.clone(),
        first_dependency.clone(),
        "dependency",
        false,
    );
    assert_ne!(
        baseline,
        inputs(
            "renamed-root",
            root.clone(),
            first_dependency.clone(),
            "dependency",
            false,
        ),
        "canonical names participate in self-import routing",
    );
    assert_ne!(
        baseline,
        inputs("root", root.clone(), second_dependency, "dependency", false,),
        "physical source roots belong to the shared checkpoint",
    );
    assert_ne!(
        baseline,
        inputs(
            "root",
            root.clone(),
            first_dependency.clone(),
            "renamed_dependency",
            false,
        ),
        "requester-local dependency edges drive import routing",
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(root.join("main.omg"), fs::Permissions::from_mode(0o444))
            .expect("seal root source");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o555)).expect("seal root directory");
    }
    assert_ne!(
        baseline,
        inputs("root", root, first_dependency, "dependency", true,),
        "canonical build-visible source metadata must not cross checkpoints",
    );
}

#[test]
fn compiler_captured_canonical_metadata_rejects_late_same_length_content_drift() {
    let tree = TempTree::new();
    let root = tree.package("root");
    fs::write(root.join("undeclared.omg"), b"source").expect("write undeclared source");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            root.join("undeclared.omg"),
            fs::Permissions::from_mode(0o444),
        )
        .expect("seal source file");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o555)).expect("seal source root");
    }
    let binding = PackageSourceBinding::new(identity(1), "root", root.clone())
        .with_canonical_source_metadata()
        .expect("compiler captures canonical metadata");
    let inputs = PackageCompilationInputs::new_package(identity(1), vec![binding], vec![])
        .expect("input construction independently recaptures canonical metadata");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            root.join("undeclared.omg"),
            fs::Permissions::from_mode(0o644),
        )
        .expect("temporarily unseal source file");
    }
    fs::write(root.join("undeclared.omg"), b"change").expect("replace equal-length bytes");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            root.join("undeclared.omg"),
            fs::Permissions::from_mode(0o444),
        )
        .expect("reseal source file");
    }

    let diagnostics = inputs
        .validate_canonical_source_metadata()
        .expect_err("late content drift must invalidate compiler-captured metadata");
    assert!(
        diagnostics[0]
            .message
            .contains("changed before compiler evidence was issued")
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(tree.0.join("root"), fs::Permissions::from_mode(0o755))
            .expect("unseal source root for cleanup");
    }
}

#[test]
fn dependency_metadata_indexes_are_rejected_instead_of_aggregated() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let dependency = tree.package("dependency");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o555)).expect("seal root");
        fs::set_permissions(&dependency, fs::Permissions::from_mode(0o555))
            .expect("seal dependency");
    }
    let dependency = PackageSourceBinding::new(identity(2), "dependency", dependency)
        .with_canonical_source_metadata()
        .expect("capture dependency metadata");

    let errors = PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root),
            dependency,
        ],
        vec![PackageDependencyBinding::new(
            identity(1),
            "dependency",
            identity(2),
        )],
    )
    .expect_err("only the current build root may retain a metadata index");
    assert!(errors.iter().any(|error| matches!(
        error,
        PackageCompilationInputError::InvalidSourceRoot { reason, .. }
            if reason.contains("only the current root package")
    )));
}

fn generated_bundle(
    inputs: &PackageCompilationInputs,
    package: PackageKeyIdentity,
    target: target::TargetProfile,
    commitment_marker: u8,
    sources: Vec<PackageGeneratedSource>,
) -> PackageGeneratedSourceBundle {
    PackageGeneratedSourceBundle::from_checked(
        package,
        target,
        inputs.dependency_closure_for(package),
        PackageSourceConsumptionCommitment::for_test([commitment_marker; 32]),
        sources,
    )
}

fn generated_source(relative_path: &[u8], bytes: &[u8]) -> PackageGeneratedSource {
    let tree = build_output::replayed_single_ordinary_file(relative_path, bytes)
        .expect("test source must form a canonical retained output tree");
    build_output::select_included_sources(&tree, &[relative_path.to_vec()])
        .expect("test source must be explicitly included")
        .pop()
        .expect("one included test source must be retained")
}

fn three_package_generated_inputs(tree: &TempTree) -> PackageCompilationInputs {
    PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", tree.package("root")),
            PackageSourceBinding::new(identity(2), "middle", tree.package("middle")),
            PackageSourceBinding::new(identity(3), "leaf", tree.package("leaf")),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ],
    )
    .expect("generated-source test graph should close")
}

#[test]
fn requester_local_aliases_may_name_different_targets() {
    let tree = TempTree::new();
    let packages = (1..=4)
        .map(|marker| {
            PackageSourceBinding::new(
                identity(marker),
                format!("package-{marker}"),
                tree.package(&marker.to_string()),
            )
        })
        .collect();
    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        packages,
        vec![
            PackageDependencyBinding::new(identity(1), "shared", identity(2)),
            PackageDependencyBinding::new(identity(2), "shared", identity(3)),
            PackageDependencyBinding::new(identity(3), "leaf", identity(4)),
        ],
    )
    .expect("requester-local aliases should reconcile");

    assert_eq!(
        inputs.dependency_target(identity(1), "shared"),
        Some(identity(2))
    );
    assert_eq!(
        inputs.dependency_target(identity(2), "shared"),
        Some(identity(3))
    );
    assert_eq!(inputs.package_name(identity(1)), Some("package-1"));
    assert!(inputs.allows_declaration_selection(identity(1), identity(1)));
    assert!(inputs.allows_declaration_selection(identity(1), identity(2)));
    assert!(!inputs.allows_declaration_selection(identity(1), identity(3)));
    assert!(inputs.allows_declaration_selection(identity(2), identity(3)));
    assert!(
        inputs
            .package_label(identity(1))
            .starts_with("`package-1` (")
    );
}

#[test]
fn noncanonical_package_names_reject_at_compiler_handoff() {
    let tree = TempTree::new();
    let errors = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(
            identity(1),
            "not_canonical",
            tree.package("root"),
        )],
        Vec::new(),
    )
    .expect_err("compiler inputs must independently reject noncanonical package names");

    assert!(errors.iter().any(|error| matches!(
        error,
        PackageCompilationInputError::InvalidPackageName { identity: found, name }
            if *found == identity(1) && name == "not_canonical"
    )));
}

#[test]
fn duplicate_aliases_and_unreachable_rows_reject() {
    let tree = TempTree::new();
    let errors = PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", tree.package("root")),
            PackageSourceBinding::new(identity(2), "first", tree.package("first")),
            PackageSourceBinding::new(identity(3), "second", tree.package("second")),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "dep", identity(2)),
            PackageDependencyBinding::new(identity(1), "dep", identity(2)),
        ],
    )
    .expect_err("duplicate alias and unreachable package must reject");

    assert!(errors.iter().any(|error| matches!(
        error,
        PackageCompilationInputError::DuplicateAlias { alias, .. } if alias == "dep"
    )));
    assert!(errors.iter().any(|error| matches!(
        error,
        PackageCompilationInputError::UnreachablePackage { identity: found }
            if *found == identity(3)
    )));
}

#[test]
fn overlapping_roots_and_cycles_reject() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let nested = root.join("nested");
    fs::create_dir(&nested).expect("create nested package");
    let errors = PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root),
            PackageSourceBinding::new(identity(2), "nested", nested),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "child", identity(2)),
            PackageDependencyBinding::new(identity(2), "parent", identity(1)),
        ],
    )
    .expect_err("overlap and cycle must reject");

    assert!(errors.iter().any(|error| matches!(
        error,
        PackageCompilationInputError::OverlappingSourceRoots { .. }
    )));
    assert!(
        errors
            .iter()
            .any(|error| matches!(error, PackageCompilationInputError::DependencyCycle { .. }))
    );
}

#[test]
fn canonical_path_free_closure_recovery_rejects_open_unreachable_and_cyclic_graphs() {
    let packages = vec![identity(1), identity(2)];
    let unreachable = PackageDependencyClosure::from_canonical_parts(
        identity(1),
        BuildDeclarationKind::Package,
        packages.clone(),
        Vec::new(),
    )
    .expect_err("unreachable path-free closure package must reject");
    assert!(unreachable.contains("unreachable"));

    let open = PackageDependencyClosure::from_canonical_parts(
        identity(1),
        BuildDeclarationKind::Package,
        packages.clone(),
        vec![PackageDependencyBinding::new(
            identity(1),
            "dependency",
            identity(3),
        )],
    )
    .expect_err("open path-free closure edge must reject");
    assert!(open.contains("open edge"));

    let cyclic = PackageDependencyClosure::from_canonical_parts(
        identity(1),
        BuildDeclarationKind::Package,
        packages,
        vec![
            PackageDependencyBinding::new(identity(1), "dependency", identity(2)),
            PackageDependencyBinding::new(identity(2), "root", identity(1)),
        ],
    )
    .expect_err("cyclic path-free closure must reject");
    assert!(cyclic.contains("cycle"));
}

#[test]
fn complete_generated_source_bundles_bind_owner_closure_target_and_bytes() {
    let tree = TempTree::new();
    let inputs = three_package_generated_inputs(&tree);
    let generated = generated_source(
        b"generated_api.omg",
        b"pub machine generated_value() -> u64 { 17 }\n",
    );
    let generated_digest = generated.digest();
    let middle = generated_bundle(
        &inputs,
        identity(2),
        target::TargetProfile::WindowsX64,
        12,
        vec![generated],
    );
    let leaf = generated_bundle(
        &inputs,
        identity(3),
        target::TargetProfile::WindowsX64,
        13,
        Vec::new(),
    );

    let inputs = inputs
        .with_complete_dependency_generated_sources(vec![leaf, middle])
        .expect("one exact bundle per dependency should attach");
    inputs
        .validate_dependency_generated_source_target(Some(target::TargetProfile::WindowsX64))
        .expect("matching generated-source targets should validate");
    let logical = inputs
        .generated_source_import_path(identity(2), &[PathBuf::from("generated_api.omg")])
        .expect("compiler-issued generated path should remain canonical")
        .expect("generated module should resolve from retained custody");
    assert_eq!(
        logical,
        inputs
            .package_root(identity(2))
            .unwrap()
            .join(".omega/generated/generated_api.omg")
    );
    let retained = inputs
        .generated_source_at_logical_path(&logical)
        .expect("logical generated path should recover retained bytes");
    assert_eq!(retained.relative_path(), b"generated_api.omg");
    assert_eq!(
        retained.bytes(),
        b"pub machine generated_value() -> u64 { 17 }\n"
    );
    assert_eq!(retained.digest(), generated_digest);
}

#[test]
fn generated_source_bundle_omission_duplicate_foreign_root_and_closure_substitution_reject() {
    let tree = TempTree::new();
    let inputs = three_package_generated_inputs(&tree);
    let middle = generated_bundle(
        &inputs,
        identity(2),
        target::TargetProfile::WindowsX64,
        12,
        Vec::new(),
    );
    let leaf = generated_bundle(
        &inputs,
        identity(3),
        target::TargetProfile::WindowsX64,
        13,
        Vec::new(),
    );

    let missing = inputs
        .clone()
        .with_complete_dependency_generated_sources(vec![middle.clone()])
        .expect_err("omitted explicit empty leaf bundle must reject");
    assert!(missing.iter().any(|error| matches!(
        error,
        PackageCompilationInputError::MissingGeneratedSourceBundle { package }
            if *package == identity(3)
    )));

    let duplicate = inputs
        .clone()
        .with_complete_dependency_generated_sources(vec![
            middle.clone(),
            middle.clone(),
            leaf.clone(),
        ])
        .expect_err("duplicate package bundle must reject");
    assert!(duplicate.iter().any(|error| matches!(
        error,
        PackageCompilationInputError::DuplicateGeneratedSourceBundle { package }
            if *package == identity(2)
    )));

    let foreign = PackageGeneratedSourceBundle::from_checked(
        identity(4),
        target::TargetProfile::WindowsX64,
        inputs.dependency_closure_for(identity(3)),
        PackageSourceConsumptionCommitment::for_test([14; 32]),
        Vec::new(),
    );
    let foreign_errors = inputs
        .clone()
        .with_complete_dependency_generated_sources(vec![middle.clone(), leaf.clone(), foreign])
        .expect_err("foreign bundle must reject");
    assert!(foreign_errors.iter().any(|error| matches!(
        error,
        PackageCompilationInputError::ForeignGeneratedSourceBundle { package }
            if *package == identity(4)
    )));

    let root = PackageGeneratedSourceBundle::from_checked(
        identity(1),
        target::TargetProfile::WindowsX64,
        inputs.dependency_closure(),
        PackageSourceConsumptionCommitment::for_test([11; 32]),
        Vec::new(),
    );
    let root_errors = inputs
        .clone()
        .with_complete_dependency_generated_sources(vec![middle.clone(), leaf.clone(), root])
        .expect_err("root self-injection must reject");
    assert!(root_errors.iter().any(|error| matches!(
        error,
        PackageCompilationInputError::RootGeneratedSourceBundle { package }
            if *package == identity(1)
    )));

    let wrong_closure = PackageGeneratedSourceBundle::from_checked(
        identity(2),
        target::TargetProfile::WindowsX64,
        inputs.dependency_closure_for(identity(3)),
        PackageSourceConsumptionCommitment::for_test([12; 32]),
        Vec::new(),
    );
    let closure_errors = inputs
        .with_complete_dependency_generated_sources(vec![wrong_closure, leaf])
        .expect_err("bundle from another producer closure must reject");
    assert!(closure_errors.iter().any(|error| matches!(
        error,
        PackageCompilationInputError::GeneratedSourceBundleClosureMismatch { package }
            if *package == identity(2)
    )));
}

#[test]
fn generated_source_bundle_target_substitution_rejects_before_loading() {
    let tree = TempTree::new();
    let inputs = three_package_generated_inputs(&tree);
    let middle = generated_bundle(
        &inputs,
        identity(2),
        target::TargetProfile::WindowsX64,
        12,
        Vec::new(),
    );
    let leaf = generated_bundle(
        &inputs,
        identity(3),
        target::TargetProfile::WindowsX64,
        13,
        Vec::new(),
    );
    let inputs = inputs
        .with_complete_dependency_generated_sources(vec![middle, leaf])
        .expect("complete generated-source bundles should attach");
    let errors = inputs
        .validate_dependency_generated_source_target(Some(target::TargetProfile::LinuxX64))
        .expect_err("cross-target generated-source substitution must reject");
    assert_eq!(errors.len(), 2);
    assert!(errors.iter().all(|error| matches!(
        error,
        PackageCompilationInputError::GeneratedSourceBundleTargetMismatch {
            bundle_target: target::TargetProfile::WindowsX64,
            selected_target: Some(target::TargetProfile::LinuxX64),
            ..
        }
    )));
}

#[test]
fn missing_and_symlink_source_roots_reject() {
    let tree = TempTree::new();
    let missing = tree.0.join("missing");
    let errors = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", missing)],
        Vec::new(),
    )
    .expect_err("missing source root must reject");
    assert!(errors.iter().any(|error| matches!(
        error,
        PackageCompilationInputError::InvalidSourceRoot { .. }
    )));

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let actual = tree.package("actual");
        let linked = tree.0.join("linked");
        symlink(actual, &linked).expect("create source-root symlink");
        let errors = PackageCompilationInputs::new_package(
            identity(1),
            vec![PackageSourceBinding::new(identity(1), "root", linked)],
            Vec::new(),
        )
        .expect_err("symlink source root must reject");
        assert!(errors.iter().any(|error| matches!(
            error,
            PackageCompilationInputError::InvalidSourceRoot { .. }
        )));
    }
}
