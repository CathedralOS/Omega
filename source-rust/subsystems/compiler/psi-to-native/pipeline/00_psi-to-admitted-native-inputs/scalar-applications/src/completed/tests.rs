use super::*;
use psi_target_layouts::{LayoutTargetFacts, terminal_target_context};
use std::sync::Arc;
use terminal_codec::CanonicalTerminalArtifact;
const TARGET: &str = "linux_x86_64";

fn artifact(label: &str) -> Arc<CanonicalTerminalArtifact> {
    let mut module = terminal_psi::TerminalModule::declarations();
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    identities.intern(label);
    module.identities = identities.build();
    let optimization =
        terminal_codec::build_identity_optimization_execution_record(&module).unwrap();
    Arc::new(CanonicalTerminalArtifact::from_parts(module, optimization, None).unwrap())
}
fn product(artifact: &Arc<CanonicalTerminalArtifact>) -> ProductScalarCompletion {
    let completed = build_scalar_evaluation::prepare_scalar_applications(
        artifact,
        Some(TARGET),
        None,
        &[],
        None,
        &mut Vec::new(),
        |_| {
            Ok(build_scalar_evaluation::ScalarScopeEvaluation {
                application_roots: None,
                targets: Some(terminal_target_context(artifact.view(), TARGET)),
                providers: psi_provider_selections::admit_provider_installation(
                    artifact.view(),
                    &[])
                .unwrap(),
                address_model: Ok(None),
                carrier_layout: None,
                layout_target: Some(LayoutTargetFacts {
                    profile: TARGET.into(),
                    pointer_size: 8,
                    pointer_alignment: 8,
                    case_tag_size: 4,
                    case_tag_alignment: 4,
                    little_endian: true,
                }),
                import_realizations: None,
            })
        },
    )
    .unwrap();
    ProductScalarCompletion {
        completed,
        proposals: Vec::new(),
    }
}

#[test]
fn preserves_the_difference_between_absent_demand_and_an_empty_roster() {
    let program = artifact("first");
    let mut accounting = Vec::new();
    assert!(
        product(&program)
            .extend(&program, TARGET, &[], None, &mut accounting)
            .unwrap()
            .is_none()
    );
    assert!(
        product(&program)
            .extend(&program, TARGET, &[], Some(&[]), &mut accounting)
            .unwrap()
            .is_some()
    );
    assert!(accounting.is_empty());
}

#[test]
fn absent_demand_still_checks_the_artifact_and_target() {
    let program = artifact("first");
    let second = artifact("other");
    let mut accounting = Vec::new();
    for (input, target) in [(&second, TARGET), (&program, "other")] {
        let error = product(&program)
            .extend(input, target, &[], None, &mut accounting)
            .unwrap_err();
        assert!(error.contains("changed its artifact or target"), "{error}");
    }
}
