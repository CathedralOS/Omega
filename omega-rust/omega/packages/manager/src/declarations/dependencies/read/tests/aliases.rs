use super::PackageFixture;
use crate::declarations::dependencies::read::{
    DependencyAliasError, ProjectedDependencies, extract_build_dependency_projection,
};
use crate::declarations::{AliasName, PackageName};

fn projection(source: &str) -> ProjectedDependencies {
    let fixture = PackageFixture::with_source(source);
    extract_build_dependency_projection(&fixture.root)
        .expect("project flat dependencies")
        .into_parts()
        .1
}

fn package_names(names: &[&str]) -> Vec<PackageName> {
    names
        .iter()
        .map(|name| PackageName::parse(*name).expect("valid package name"))
        .collect()
}

#[test]
fn accepts_unique_aliases_over_the_whole_authored_set() {
    let dependencies = projection(
        r#"
        machine build(builder: &mut Build) {
            builder.package("unique-aliases");
            builder.depend(Source::Path { location: "../portable" });
            builder.depend_as("native_api", Source::Path { location: "../native" });
        }
        "#,
    );

    dependencies
        .validate_aliases(&package_names(&["portable-api", "platform-api"]))
        .expect("the flat dependency set has unique aliases");
    assert_eq!(dependencies.authored_dependencies().len(), 2);
}

#[test]
fn rejects_explicit_alias_reuse_anywhere_in_the_set() {
    let dependencies = projection(
        r#"
        machine build(builder: &mut Build) {
            builder.package("explicit-alias-conflict");
            builder.depend_as("shared", Source::Path { location: "../first" });
            builder.depend_as("shared", Source::Path { location: "../second" });
        }
        "#,
    );

    assert_eq!(
        dependencies.validate_aliases(&package_names(&["first", "second"])),
        Err(DependencyAliasError::DuplicateAlias {
            alias: AliasName::parse("shared").unwrap(),
            first_occurrence: 0,
            conflicting_occurrence: 1,
        })
    );
}

#[test]
fn rejects_default_alias_collisions_after_package_selection() {
    let dependencies = projection(
        r#"
        machine build(builder: &mut Build) {
            builder.package("default-alias-conflict");
            builder.depend(Source::Path { location: "../first" });
            builder.depend(Source::Path { location: "../second" });
        }
        "#,
    );

    assert!(matches!(
        dependencies.validate_aliases(&package_names(&["same-name", "same-name"])),
        Err(DependencyAliasError::DuplicateAlias {
            first_occurrence: 0,
            conflicting_occurrence: 1,
            ref alias,
        }) if alias.as_str() == "same_name"
    ));
}

#[test]
fn rejects_an_incomplete_selected_package_roster() {
    let dependencies = projection(
        r#"
        machine build(builder: &mut Build) {
            builder.package("alias-roster");
            builder.depend(Source::Path { location: "../first" });
            builder.depend(Source::Path { location: "../second" });
        }
        "#,
    );

    assert_eq!(
        dependencies.validate_aliases(&package_names(&["first"])),
        Err(DependencyAliasError::SelectedPackageCountMismatch {
            dependency_occurrences: 2,
            selected_packages: 1,
        })
    );
}
