use super::PackageFixture;
use crate::declarations::PackageName;
use crate::declarations::dependencies::read::{
    ActiveDependencyAliasError, ActiveDependencyAliasScope, ProjectedDependencies,
    extract_build_dependency_projection,
};
use omega_target::TargetProfile;

fn projection(source: &str) -> ProjectedDependencies {
    let fixture = PackageFixture::with_source(source);
    extract_build_dependency_projection(&fixture.root)
        .expect("project dependency columns")
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
fn allows_one_alias_in_mutually_exclusive_exact_profile_columns() {
    let dependencies = projection(
        r#"
        machine build(builder: &mut Build) {
            builder.package("profile-alias-reuse");
            transition builder.target {
                TargetProfile::WindowsX86_64 -> windows(builder)
                TargetProfile::LinuxX86_64 -> linux(builder)
            }

            state windows(builder: &mut Build) {
                builder.depend_as("native_api", Source::Path { location: "../windows" });
            }
            state linux(builder: &mut Build) {
                builder.depend_as("native_api", Source::Path { location: "../linux" });
            }
        }
        "#,
    );

    dependencies
        .validate_active_aliases(TargetProfile::WindowsX64, &package_names(&["windows-api"]))
        .expect("Windows column may use the local alias");
    dependencies
        .validate_active_aliases(TargetProfile::LinuxX64, &package_names(&["linux-api"]))
        .expect("mutually exclusive columns may reuse one local alias");
    assert_eq!(dependencies.authored_dependencies().len(), 2);
    assert_eq!(dependencies.by_profile().len(), 2);
}

#[test]
fn rejects_a_common_alias_reused_in_an_exact_profile_column() {
    let dependencies = projection(
        r#"
        machine build(builder: &mut Build) {
            builder.package("common-profile-conflict");
            builder.depend_as("native_api", Source::Path { location: "../portable" });
            transition builder.target {
                TargetProfile::WindowsX86_64 -> windows(builder)
            }

            state windows(builder: &mut Build) {
                builder.depend_as("native_api", Source::Path { location: "../windows" });
            }
        }
        "#,
    );

    assert_eq!(
        dependencies.validate_active_aliases(
            TargetProfile::WindowsX64,
            &package_names(&["portable-api", "windows-api"]),
        ),
        Err(ActiveDependencyAliasError::DuplicateAlias {
            scope: ActiveDependencyAliasScope::Profile(TargetProfile::WindowsX64),
            alias: crate::declarations::AliasName::parse("native_api").unwrap(),
            first_occurrence: 0,
            conflicting_occurrence: 1,
        })
    );
}

#[test]
fn rejects_default_aliases_that_collide_after_package_selection() {
    let dependencies = projection(
        r#"
        machine build(builder: &mut Build) {
            builder.package("resolved-alias-conflict");
            transition builder.target {
                TargetProfile::LinuxX86_64 -> linux(builder)
            }

            state linux(builder: &mut Build) {
                builder.depend(Source::Path { location: "../first" });
                builder.depend(Source::Path { location: "../second" });
            }
        }
        "#,
    );

    let error = dependencies
        .validate_active_aliases(
            TargetProfile::LinuxX64,
            &package_names(&["same-name", "same-name"]),
        )
        .expect_err("selected package names produce a duplicate default alias");
    assert!(matches!(
        error,
        ActiveDependencyAliasError::DuplicateAlias {
            scope: ActiveDependencyAliasScope::Profile(TargetProfile::LinuxX64),
            first_occurrence: 0,
            conflicting_occurrence: 1,
            ref alias,
        } if alias.as_str() == "same_name"
    ));
}

#[test]
fn rejects_alias_reuse_within_the_common_column() {
    let dependencies = projection(
        r#"
        machine build(builder: &mut Build) {
            builder.package("common-alias-conflict");
            builder.depend_as("shared", Source::Path { location: "../first" });
            builder.depend_as("shared", Source::Path { location: "../second" });
        }
        "#,
    );

    let error = dependencies
        .validate_active_aliases(
            TargetProfile::CrossPlatformCli,
            &package_names(&["first", "second"]),
        )
        .expect_err("one common active set cannot bind an alias twice");
    assert!(matches!(
        error,
        ActiveDependencyAliasError::DuplicateAlias {
            scope: ActiveDependencyAliasScope::Common,
            first_occurrence: 0,
            conflicting_occurrence: 1,
            ref alias,
        } if alias.as_str() == "shared"
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
        dependencies
            .validate_active_aliases(TargetProfile::CrossPlatformCli, &package_names(&["first"]),),
        Err(ActiveDependencyAliasError::ResolvedPackageCountMismatch {
            active_occurrences: 2,
            selected_packages: 1,
        })
    );
}
