use super::PackageFixture;
use crate::manifest::declaration::{BuildDeclaration, BuildDeclarationError};
use crate::manifest::dependency_projection::{
    DependencyProjectionError, DependencySourceRequest, extract_build_dependency_projection,
};
use omega_package_source::AliasName;
use std::fs;

#[test]
fn projects_path_and_git_requests_in_authored_order() {
    let fixture = PackageFixture::with_source(
        r#"
        machine build(builder: &mut Build, filesystem: &mut Filesystem) {
            builder.application("dependency-projection-probe");
            builder.depend(Source::Path { location: "../local" });
            builder.depend_as("arithmetic_kernels", Source::Git {
                revision: "0123456789abcdef",
                repository: "ssh://git@github.com/CathedralOS/example.git"
            });
        }
        "#,
    );
    let projection = extract_build_dependency_projection(&fixture.root).unwrap();
    assert!(matches!(
        projection.declaration(),
        BuildDeclaration::Application(application)
            if application.name.as_str() == "dependency-projection-probe"
    ));
    assert_eq!(
        projection.dependencies(),
        vec![
            DependencySourceRequest::Path {
                explicit_alias: None,
                location: "../local".to_owned(),
            },
            DependencySourceRequest::Git {
                explicit_alias: Some(AliasName::parse("arithmetic_kernels").unwrap()),
                repository: "ssh://git@github.com/CathedralOS/example.git".to_owned(),
                revision: "0123456789abcdef".to_owned(),
            },
        ]
    );
}

#[test]
fn absent_build_machine_is_not_an_implicit_project_role() {
    let fixture = PackageFixture::with_source("target windows_x64 { }");
    assert!(matches!(
        fixture.extract(),
        Err(DependencyProjectionError::BuildDeclaration(error))
            if matches!(*error, BuildDeclarationError::MissingBuildDeclaration)
    ));
}

#[test]
fn rejects_missing_unreadable_non_utf8_unlexable_and_unparsable_files() {
    let missing = PackageFixture::empty();
    assert!(matches!(
        missing.extract(),
        Err(DependencyProjectionError::MissingBuildFile { .. })
    ));

    let unreadable = PackageFixture::empty();
    fs::create_dir(
        unreadable
            .root
            .join(super::super::extraction::BUILD_FILE_NAME),
    )
    .expect("create build directory");
    assert!(matches!(
        unreadable.extract(),
        Err(DependencyProjectionError::ReadBuildFile { .. })
    ));

    let invalid_encoding = PackageFixture::empty();
    fs::write(
        invalid_encoding
            .root
            .join(super::super::extraction::BUILD_FILE_NAME),
        [0xff],
    )
    .expect("write bad UTF-8");
    assert!(matches!(
        invalid_encoding.extract(),
        Err(DependencyProjectionError::InvalidBuildFileEncoding { .. })
    ));

    let unlexable = PackageFixture::with_source("machine build(builder: &mut Build) { ` }");
    assert!(matches!(
        unlexable.extract(),
        Err(DependencyProjectionError::Lex { .. })
    ));
    let unparsable = PackageFixture::with_source("machine build(builder: &mut Build) {");
    assert!(matches!(
        unparsable.extract(),
        Err(DependencyProjectionError::Parse { .. })
    ));
}

#[test]
fn rejects_duplicate_and_scoped_build_machines() {
    let duplicate = PackageFixture::with_source(
        "machine build(builder: &mut Build) {} machine build(builder: &mut Build) {}",
    );
    assert!(matches!(
        duplicate.extract(),
        Err(DependencyProjectionError::DuplicateBuildMachines { count: 2 })
    ));

    let scoped = PackageFixture::with_source("machine Owner::build(builder: &mut Build) {}");
    assert!(matches!(
        scoped.extract(),
        Err(DependencyProjectionError::ScopedBuildMachine { .. })
    ));
}
