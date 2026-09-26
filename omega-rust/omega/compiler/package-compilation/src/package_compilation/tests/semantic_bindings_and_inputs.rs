use super::{
    TempTree, accepted_console_binding, generated_bundle, generated_source, identity,
    three_package_generated_inputs,
};
use crate::package_compilation::{
    AcceptedSemanticBinding, AcceptedSemanticBindingRole, BuildDeclarationKind,
    PackageCompilationInputError, PackageCompilationInputs, PackageCompilationTargetInputs,
    PackageDependencyBinding, PackageSourceBinding, PathBuf,
};
use std::fs;

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
    assert!(std::sync::Arc::ptr_eq(
        &with_binding.source_inputs(),
        &expected
    ));

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
    assert!(std::sync::Arc::ptr_eq(
        &with_generated.source_inputs(),
        &expected
    ));
    let expected_inputs = with_generated.clone();
    let (source, target) = with_generated.into_parts();
    assert!(std::sync::Arc::ptr_eq(&source, &expected));
    let rejoined = PackageCompilationInputs::from_parts(source, target)
        .expect("split generated inputs rejoin the same source graph");
    assert_eq!(rejoined, expected_inputs);
    assert!(std::sync::Arc::ptr_eq(&rejoined.source_inputs(), &expected));
}

#[test]
fn target_inputs_share_sources_without_sharing_target_attachments() {
    let tree = TempTree::new();
    let inputs = three_package_generated_inputs(&tree);
    let shared_source = inputs.source_inputs();
    let for_target = |target, marker| {
        let bundles = [identity(2), identity(3)]
            .into_iter()
            .map(|package| generated_bundle(&inputs, package, target, marker, vec![]))
            .collect();
        inputs
            .clone()
            .with_complete_dependency_generated_sources(bundles)
            .expect("complete exact-target generated inputs")
    };
    let windows = for_target(target::TargetProfile::WindowsX64, 8)
        .with_accepted_semantic_bindings(vec![accepted_console_binding(identity(2), 7)])
        .expect("Windows binding belongs to shared source graph");
    let linux = for_target(target::TargetProfile::LinuxX64, 9);
    assert!(std::sync::Arc::ptr_eq(
        &windows.source_inputs(),
        &shared_source
    ));
    assert!(std::sync::Arc::ptr_eq(
        &linux.source_inputs(),
        &shared_source
    ));
    assert_eq!(windows.accepted_semantic_bindings().count(), 1);
    assert_eq!(linux.accepted_semantic_bindings().count(), 0);
    windows
        .validate_dependency_generated_source_target(Some(target::TargetProfile::WindowsX64))
        .expect("Windows attachments retain exact target");
    linux
        .validate_dependency_generated_source_target(Some(target::TargetProfile::LinuxX64))
        .expect("Linux attachments retain exact target");
    let expected = windows.clone();
    let (source, target) = windows.into_parts();
    assert_eq!(
        PackageCompilationInputs::from_parts(source, target).expect("both maps rejoin"),
        expected
    );
    let empty = PackageCompilationInputs::from_parts(
        shared_source,
        PackageCompilationTargetInputs::default(),
    )
    .expect("no target attachments is a valid initial state even with dependencies");
    assert_eq!(empty, inputs);
}

#[test]
fn rejoining_target_inputs_rejects_foreign_semantic_packages() {
    let tree = TempTree::new();
    let original = three_package_generated_inputs(&tree)
        .with_accepted_semantic_bindings(vec![accepted_console_binding(identity(2), 7)])
        .expect("binding belongs to original graph");
    let (_, target) = original.into_parts();
    let other = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(
            identity(1),
            "root",
            tree.package("other"),
        )],
        vec![],
    )
    .expect("different closed source graph");
    let errors = PackageCompilationInputs::from_parts(other.source_inputs(), target)
        .expect_err("binding cannot cross into a graph without its package");
    assert!(errors.iter().any(|error| matches!(error,
        PackageCompilationInputError::ForeignSemanticBindingPackage { package, .. }
            if *package == identity(2)
    )));
}

#[test]
fn rejoining_generated_inputs_revalidates_closure_and_completeness() {
    let tree = TempTree::new();
    let original = three_package_generated_inputs(&tree);
    let bundles = [identity(2), identity(3)]
        .into_iter()
        .map(|package| {
            generated_bundle(
                &original,
                package,
                target::TargetProfile::LinuxX64,
                9,
                vec![],
            )
        })
        .collect();
    let (_, target) = original
        .with_complete_dependency_generated_sources(bundles)
        .expect("original complete bundles")
        .into_parts();
    let changed_graph = PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", tree.package("new-root")),
            PackageSourceBinding::new(identity(2), "middle", tree.package("new-middle")),
            PackageSourceBinding::new(identity(3), "leaf", tree.package("new-leaf")),
            PackageSourceBinding::new(identity(4), "extra", tree.package("extra")),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(1), "leaf", identity(3)),
            PackageDependencyBinding::new(identity(1), "extra", identity(4)),
        ],
    )
    .expect("new closed graph changes middle's closure and adds a dependency");
    let errors = PackageCompilationInputs::from_parts(changed_graph.source_inputs(), target)
        .expect_err("generated custody cannot cross into changed source graph");
    assert!(errors.iter().any(|error| matches!(error,
        PackageCompilationInputError::GeneratedSourceBundleClosureMismatch { package }
            if *package == identity(2)
    )));
    assert!(errors.iter().any(|error| matches!(error,
        PackageCompilationInputError::MissingGeneratedSourceBundle { package }
            if *package == identity(4)
    )));
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
            "renamed_root",
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
fn compiler_captured_metadata_rejects_same_length_drift_at_input_and_evidence_boundaries() {
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
    let inputs = PackageCompilationInputs::new_package(identity(1), vec![binding.clone()], vec![])
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

    PackageCompilationInputs::new_package(identity(1), vec![binding], vec![])
        .expect_err("input construction must reject a stale compiler-captured snapshot");
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
