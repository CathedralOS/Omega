use super::PackageFixture;
use crate::declarations::dependencies::read::DependencyProjectionError;

#[test]
fn rejects_dependencies_reached_through_target_control_flow() {
    let fixture = PackageFixture::with_source(
        r#"
        machine build(builder: &mut Build) {
            builder.package("target-dependent-graph");
            transition builder.target {
                TargetProfile::WindowsX86_64 -> windows(builder)
            }

            state windows(builder: &mut Build) {
                builder.depend(Source::Path { location: "../windows" });
            }
        }
        "#,
    );

    assert_eq!(
        fixture.extract().unwrap_err(),
        DependencyProjectionError::UnsupportedDependencyShape
    );
}

#[test]
fn rejects_dependencies_reached_through_unconditional_state_control_flow() {
    let fixture = PackageFixture::with_source(
        r#"
        machine build(builder: &mut Build) {
            builder.package("state-dependent-graph");
            transition { _ -> dependencies(builder) }

            state dependencies(builder: &mut Build) {
                builder.depend(Source::Path { location: "../hidden" });
            }
        }
        "#,
    );

    assert_eq!(
        fixture.extract().unwrap_err(),
        DependencyProjectionError::UnsupportedDependencyShape
    );
}

#[test]
fn rejects_retired_conditional_dependency_operations() {
    for statement in [
        r#"builder.depend_when(builder.target, Source::Path { location: "../target" });"#,
        r#"builder.depend_as_when("native", builder.target, Source::Path { location: "../target" });"#,
    ] {
        let fixture = PackageFixture::with_source(&format!(
            "machine build(builder: &mut Build) {{ builder.package(\"conditional-operation\"); {statement} }}"
        ));
        assert_eq!(
            fixture.extract().unwrap_err(),
            DependencyProjectionError::UnsupportedDependencyShape
        );
    }
}
