use super::PackageFixture;
use crate::declarations::dependencies::read::DependencyProjectionError;

#[test]
fn rejects_noncanonical_first_build_parameter() {
    for source in [
        "machine build() {}",
        "machine build(builder: Build) {}",
        "machine build(builder: &Build) {}",
        "machine build(builder: &write Build) {}",
        "machine build(builder: &mut Builder) {}",
        "machine build(build: &mut Build) {}",
    ] {
        let fixture = PackageFixture::with_source(source);
        assert!(matches!(
            fixture.extract(),
            Err(DependencyProjectionError::InvalidBuildParameter)
        ));
    }
}

#[test]
fn rejects_nonordinary_build_machine_forms() {
    for source in [
        "boundary machine build(builder: &mut Build);",
        "machine build<T>(builder: &mut Build) {}",
    ] {
        let fixture = PackageFixture::with_source(source);
        assert!(matches!(
            fixture.extract(),
            Err(DependencyProjectionError::InvalidBuildMachine)
        ));
    }
}

#[test]
fn rejects_authored_dependency_vocabulary() {
    for source in [
        "data Build {} machine build(builder: &mut Build) {}",
        "data Source {} machine build(builder: &mut Build) {}",
        "data PackageSelection {} machine build(builder: &mut Build) {}",
        "domain u64::Source; machine build(builder: &mut Build) {}",
        "trait Build {} machine build(builder: &mut Build) {}",
        "machine Build::depend(source: Source) {} machine build(builder: &mut Build) {}",
        "machine Build::depend_as(alias: &[u8], source: Source) {} machine build(builder: &mut Build) {}",
        "machine Build::depend_when(condition: bool, source: Source) {} machine build(builder: &mut Build) {}",
        "machine Build::depend_as_when(alias: &[u8], condition: bool, source: Source) {} machine build(builder: &mut Build) {}",
    ] {
        let fixture = PackageFixture::with_source(source);
        let result = fixture.extract();
        assert!(
            matches!(
                result,
                Err(DependencyProjectionError::AuthoredToolchainVocabulary { .. })
            ),
            "unexpected projection result for {source:?}: {result:?}"
        );
    }
}

#[test]
fn rejects_nested_helper_and_control_flow_dependency_requests() {
    let helper = PackageFixture::with_source(
        r#"
        machine add_dependency(builder: &mut Build) {
            builder.depend_as("hidden_alias", Source::Path { location: "hidden" });
        }
        machine build(builder: &mut Build) { add_dependency(builder); }
        "#,
    );
    assert!(matches!(
        helper.extract(),
        Err(DependencyProjectionError::UnsupportedDependencyShape)
    ));

    let nested_state = PackageFixture::with_source(
        r#"
        machine build(builder: &mut Build) {
            state later(builder: &mut Build) {
                builder.depend(Source::Path { location: "conditional" });
            }
        }
        "#,
    );
    assert!(matches!(
        nested_state.extract(),
        Err(DependencyProjectionError::UnsupportedDependencyShape)
    ));
}

#[test]
fn rejects_dependency_syntax_without_an_authoritative_build_machine() {
    let fixture = PackageFixture::with_source(
        r#"
        machine helper(builder: &mut Build) {
            builder.depend(Source::Path { location: "hidden" });
        }
        "#,
    );
    assert!(matches!(
        fixture.extract(),
        Err(DependencyProjectionError::UnsupportedDependencyShape)
    ));
}
