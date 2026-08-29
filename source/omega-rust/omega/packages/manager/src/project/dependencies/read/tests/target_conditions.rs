use super::PackageFixture;
use crate::project::dependencies::read::{
    DependencyPathTaint, DependencyProjectionError, ProjectedDependencies,
    TARGET_DEPENDENCY_CONDITION_SCHEMA_VERSION, extract_build_dependency_projection,
};
use omega_target::TargetProfile;

fn projected(source: &str) -> ProjectedDependencies {
    let fixture = PackageFixture::with_source(source);
    extract_build_dependency_projection(&fixture.root)
        .expect("project build state graph")
        .into_parts()
        .1
}

fn path_locations<'a>(
    dependencies: impl IntoIterator<Item = &'a super::DependencySourceRequest>,
) -> Vec<&'a str> {
    dependencies
        .into_iter()
        .map(|dependency| match dependency {
            super::DependencySourceRequest::Path { location, .. } => location.as_str(),
            super::DependencySourceRequest::Git { .. } => panic!("expected path dependency"),
        })
        .collect()
}

fn profile_locations(projection: &ProjectedDependencies, profile: TargetProfile) -> Vec<&str> {
    let column_indices = projection
        .by_profile()
        .iter()
        .find(|column| column.profile() == profile)
        .map(|column| column.occurrence_indices())
        .unwrap_or_default();
    path_locations(
        column_indices
            .iter()
            .map(|index| &projection.authored_dependencies()[*index]),
    )
}

#[test]
fn projects_anonymous_unconditional_transition_as_common() {
    let projection = projected(
        r#"
        machine build(builder: &mut Build) {
            builder.package("unconditional-projection");
            transition { _ -> dependencies(builder) }

            state dependencies(builder: &mut Build) {
                builder.depend(Source::Path { location: "../common" });
            }
        }
        "#,
    );

    assert_eq!(path_locations(projection.common()), ["../common"]);
    assert!(projection.by_profile().is_empty());
    assert!(
        projection
            .condition_schema()
            .referenced_profile_identities()
            .is_empty()
    );
}

#[test]
fn projects_common_and_exact_target_columns_without_flattening() {
    let source = r#"
        machine build(builder: &mut Build) {
            builder.package("target-columns");
            builder.depend(Source::Path { location: "../portable" });
            transition builder.target {
                TargetProfile::WindowsX86_64 -> windows(builder)
                TargetProfile::LinuxX86_64 -> linux(builder)
            }

            state windows(builder: &mut Build) {
                builder.depend(Source::Path { location: "../windows" });
            }
            state linux(builder: &mut Build) {
                builder.depend(Source::Path { location: "../linux" });
            }
        }
    "#;
    let projection = projected(source);

    assert_eq!(path_locations(projection.common()), ["../portable"]);
    assert_eq!(
        profile_locations(&projection, TargetProfile::LinuxX64),
        ["../linux"]
    );
    assert_eq!(
        profile_locations(&projection, TargetProfile::WindowsX64),
        ["../windows"]
    );
    assert_eq!(
        path_locations(projection.for_profile(TargetProfile::WindowsX64)),
        ["../portable", "../windows"]
    );
    assert_eq!(
        projection.condition_schema().version(),
        TARGET_DEPENDENCY_CONDITION_SCHEMA_VERSION
    );
    assert_eq!(
        projection
            .condition_schema()
            .referenced_profile_identities(),
        [
            TargetProfile::LinuxX64.identity(),
            TargetProfile::WindowsX64.identity()
        ]
    );

    let fixture = PackageFixture::with_source(source);
    assert_eq!(
        fixture.extract().unwrap_err(),
        DependencyProjectionError::TargetConditionedResolutionUnavailable,
        "a profile-less caller must not flatten exact columns"
    );
}

#[test]
fn intersects_nested_constraints_merges_shared_states_and_closes_cycles() {
    let projection = projected(
        r#"
        machine build(builder: &mut Build) {
            builder.package("state-fixpoint");
            transition builder.target {
                TargetProfile::WindowsX86_64 -> windows(builder)
                TargetProfile::LinuxX86_64 -> shared(builder)
            }

            state windows(builder: &mut Build) {
                transition builder.target {
                    TargetProfile::WindowsX86_64 -> shared(builder)
                    TargetProfile::LinuxX86_64 -> {}
                }
            }
            state shared(builder: &mut Build) {
                builder.depend(Source::Path { location: "../shared-native" });
                transition builder.target {
                    TargetProfile::WindowsX86_64 -> windows(builder)
                    TargetProfile::LinuxX86_64 -> shared(builder)
                }
            }
        }
        "#,
    );

    assert_eq!(
        profile_locations(&projection, TargetProfile::LinuxX64),
        ["../shared-native"]
    );
    assert_eq!(
        profile_locations(&projection, TargetProfile::WindowsX64),
        ["../shared-native"]
    );
}

#[test]
fn rejects_mixed_exact_and_wildcard_paths_with_arm_provenance() {
    let fixture = PackageFixture::with_source(
        r#"
        machine build(builder: &mut Build) {
            builder.package("mixed-target-paths");
            transition builder.target {
                TargetProfile::WindowsX86_64 -> shared(builder)
                _ -> shared(builder)
            }
            state shared(builder: &mut Build) {
                builder.depend(Source::Path { location: "../shared" });
            }
        }
        "#,
    );

    let error = extract_build_dependency_projection(&fixture.root).unwrap_err();
    assert!(matches!(
        error,
        DependencyProjectionError::MixedDependencyPaths {
            provenance,
            ..
        } if provenance.taint == DependencyPathTaint::WildcardTargetArm
            && provenance.transition.span.end > provenance.transition.span.start
    ));
}

#[test]
fn rejects_runtime_subject_paths_even_when_the_state_receives_builder_authority() {
    let fixture = PackageFixture::with_source(
        r#"
        machine build(builder: &mut Build) {
            builder.package("runtime-subject-path");
            transition builder.freestanding {
                true -> conditional(builder)
                _ -> {}
            }
            state conditional(builder: &mut Build) {
                builder.depend(Source::Path { location: "../conditional" });
            }
        }
        "#,
    );

    let error = extract_build_dependency_projection(&fixture.root).unwrap_err();
    assert!(matches!(
        error,
        DependencyProjectionError::TaintedDependencyPath {
            provenance,
            ..
        } if provenance.taint == DependencyPathTaint::RuntimeSubjectTransition
    ));
}

#[test]
fn rejects_dependencies_behind_contradictory_nested_profiles_as_unreachable() {
    let fixture = PackageFixture::with_source(
        r#"
        machine build(builder: &mut Build) {
            builder.package("unreachable-target-path");
            transition builder.target {
                TargetProfile::WindowsX86_64 -> windows(builder)
            }
            state windows(builder: &mut Build) {
                transition builder.target {
                    TargetProfile::LinuxX86_64 -> impossible(builder)
                }
            }
            state impossible(builder: &mut Build) {
                builder.depend(Source::Path { location: "../impossible" });
            }
        }
        "#,
    );

    assert!(matches!(
        extract_build_dependency_projection(&fixture.root),
        Err(DependencyProjectionError::UnreachableDependency { state, .. })
            if state == "impossible"
    ));
}

#[test]
fn validates_exact_cases_against_the_trusted_target_catalog() {
    let fixture = PackageFixture::with_source(
        r#"
        machine build(builder: &mut Build) {
            builder.package("unknown-target-case");
            transition builder.target {
                TargetProfile::ImaginaryCpu -> dependency(builder)
            }
            state dependency(builder: &mut Build) {
                builder.depend(Source::Path { location: "../unknown" });
            }
        }
        "#,
    );

    assert!(matches!(
        extract_build_dependency_projection(&fixture.root),
        Err(DependencyProjectionError::UnknownTargetProfile { case_name, .. })
            if case_name == "ImaginaryCpu"
    ));
}
