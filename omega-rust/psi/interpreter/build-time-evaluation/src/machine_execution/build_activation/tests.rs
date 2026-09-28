use super::admit_toolchain_build_activation;
use source::{SourceMap, SourceOrigin};
use typed_trees::TypedTrees;

const PRELUDE: &str = "
    data X86DeploymentFeatures { case Baseline; case AvxFma3; }
    data Build { x86_deployment_features: X86DeploymentFeatures; }
";

fn program(prelude: &str, origin: SourceOrigin, path: &str, body: &str) -> TypedTrees {
    let mut sources = SourceMap::default();
    let prelude_id = sources
        .add_with_metadata(path.into(), prelude.into(), ".".into(), None, origin)
        .source_id;
    let body_id = sources.add("build.omg".into(), body.into()).source_id;
    crate::front_end::typed_program_from_source_map(
        sources,
        &[(prelude_id, prelude), (body_id, body)],
    )
}

fn toolchain_program(body: &str) -> TypedTrees {
    program(PRELUDE, SourceOrigin::Toolchain, "<build-prelude>", body)
}

#[test]
fn admitted_build_can_update_its_configuration_field() {
    let typed = toolchain_program(
        "machine build(builder: &mut Build) {
            builder.x86_deployment_features = X86DeploymentFeatures::AvxFma3;
        }",
    );
    assert!(admit_toolchain_build_activation(&typed).unwrap());
}

#[test]
fn nominal_lookalikes_do_not_receive_toolchain_activation() {
    for (origin, path) in [
        (SourceOrigin::User, "<build-prelude>"),
        (SourceOrigin::Toolchain, "authored_build.omg"),
    ] {
        let typed = program(
            PRELUDE,
            origin,
            path,
            "machine build(builder: &mut Build) {}",
        );
        assert!(!admit_toolchain_build_activation(&typed).unwrap());
    }
}

#[test]
fn deployment_field_requires_the_exact_toolchain_type() {
    let typed = program(
        "data Build { x86_deployment_features: X86DeploymentFeatures; }",
        SourceOrigin::Toolchain,
        "<build-prelude>",
        "data X86DeploymentFeatures { case Baseline; }
         machine build(builder: &mut Build) {}",
    );
    let errors = admit_toolchain_build_activation(&typed).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "toolchain Build.x86_deployment_features must have the exact toolchain X86DeploymentFeatures type",
    );
}

#[test]
fn missing_deployment_field_rejects_during_admission() {
    let typed = program(
        "data Build { unrelated: bool; }",
        SourceOrigin::Toolchain,
        "<build-prelude>",
        "machine build(builder: &mut Build) {}",
    );
    let errors = admit_toolchain_build_activation(&typed).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert!(
        errors[0]
            .message
            .contains("declares 0 `x86_deployment_features` fields")
    );
}

#[test]
fn build_storage_and_local_copy_reject_without_executing_a_machine() {
    for (body, expected) in [
        (
            "data Container { activation: Build; }
             machine build(builder: &mut Build) {}",
            "cannot be stored in field `activation`",
        ),
        (
            "machine build(builder: &mut Build) { let copied: Build; }",
            "cannot be copied into local `copied`",
        ),
        (
            "machine build(builder: &mut Build) {
                let forged: Build = Build {
                    x86_deployment_features: X86DeploymentFeatures::Baseline
                };
             }",
            "source cannot construct the compiler-owned Build activation",
        ),
    ] {
        let typed = toolchain_program(body);
        let errors = admit_toolchain_build_activation(&typed).unwrap_err();
        assert!(
            errors.iter().any(|error| error.message.contains(expected)),
            "{errors:?}"
        );
    }
}
