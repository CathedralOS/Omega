use super::{make_tree_owner_writable, temp_root, write_package};
use crate::declarations::dependencies::read::{DependencyProjectionError, DependencySourceRequest};
use crate::declarations::project::BuildDeclarationError;
use crate::discovery::{ResolvePackageSourceError, resolve_external_local_package_source};
use omega_package_source::{ExternalSourceContext, LocalSourceLimits};

#[test]
fn declaration_failure_does_not_fall_back_to_repository_name() {
    let root = temp_root("missing-declaration");
    let cache = temp_root("missing-declaration-cache");
    std::fs::create_dir_all(&root).expect("create source");
    std::fs::write(root.join("main.omg"), "machine Main::main() {}\n").expect("write source");

    let error = resolve_external_local_package_source(
        &root,
        &cache,
        LocalSourceLimits::default(),
        ExternalSourceContext::derive(b"consumer-lock"),
    )
    .expect_err("missing declaration must reject");

    assert!(matches!(
        error,
        ResolvePackageSourceError::Declaration(BuildDeclarationError::MissingBuildFile { .. })
    ));

    let _ = std::fs::remove_dir_all(&root);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn application_role_cannot_be_bound_as_a_package_source() {
    let root = temp_root("application-role");
    let cache = temp_root("application-role-cache");
    std::fs::create_dir_all(&root).expect("create source");
    std::fs::write(
        root.join("build.omg"),
        "machine build(builder: &mut Build) {\n    builder.application(\"artifact-root\");\n}\n",
    )
    .expect("write application declaration");
    std::fs::write(root.join("main.omg"), "machine Main::main() {}\n").expect("write source");

    let error = resolve_external_local_package_source(
        &root,
        &cache,
        LocalSourceLimits::default(),
        ExternalSourceContext::derive(b"consumer-lock"),
    )
    .expect_err("an application must not become an importable package");

    assert!(matches!(
        error,
        ResolvePackageSourceError::Declaration(
            BuildDeclarationError::ExpectedPackageDeclaration {
                found: crate::declarations::project::BuildDeclarationKind::Application
            }
        )
    ));

    let _ = std::fs::remove_dir_all(&root);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn source_custody_projects_only_canonical_dependency_rows() {
    let root = temp_root("dependencies");
    let cache = temp_root("dependencies-cache");
    write_package(&root, "application");
    std::fs::write(
        root.join("build.omg"),
        r#"
            machine build(builder: &mut Build) {
                builder.package("application");
                builder.depend(Source::Path { location: "../local-library" });
                builder.depend(Source::Git {
                    repository: "https://github.com/CathedralOS/arithmetic-kernels.git",
                    revision: "main"
                });
            }
            "#,
    )
    .expect("write dependency projection");

    let resolved = resolve_external_local_package_source(
        &root,
        &cache,
        LocalSourceLimits::default(),
        ExternalSourceContext::derive(b"consumer-lock"),
    )
    .expect("resolve package and dependency projection");

    assert_eq!(
        resolved.dependency_requests(),
        [
            DependencySourceRequest::Path {
                explicit_alias: None,
                location: "../local-library".to_owned(),
            },
            DependencySourceRequest::Git {
                explicit_alias: None,
                repository: "https://github.com/CathedralOS/arithmetic-kernels.git".to_owned(),
                revision: "main".to_owned(),
                selection: crate::declarations::PackageSelection::Root,
            },
        ]
    );

    let _ = std::fs::remove_dir_all(&root);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn source_custody_rejects_hidden_dependency_requests() {
    let root = temp_root("hidden-dependency");
    let cache = temp_root("hidden-dependency-cache");
    write_package(&root, "application");
    std::fs::write(
        root.join("build.omg"),
        r#"
            machine helper(builder: &mut Build) {
                builder.depend(Source::Path { location: "../hidden" });
            }
            machine build(builder: &mut Build) {
                builder.package("application");
                helper(builder);
            }
            "#,
    )
    .expect("write hidden dependency");

    let error = resolve_external_local_package_source(
        &root,
        &cache,
        LocalSourceLimits::default(),
        ExternalSourceContext::derive(b"consumer-lock"),
    )
    .expect_err("hidden dependency request must reject");
    assert!(matches!(
        error,
        ResolvePackageSourceError::DependencyProjection(
            DependencyProjectionError::UnsupportedDependencyShape
        )
    ));

    let _ = std::fs::remove_dir_all(&root);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}
