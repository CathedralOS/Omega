//! Read the authored public call through executed Build and ordinary Check.
//! The invalid twin changes only its runtime index; setup failure cannot stand
//! in for the fact-sensitive refusal.

use super::fixture_package_inputs;

#[test]
fn selected_requirement_preserves_runtime_index_and_explicit_evidence() {
    let target = "macos_arm64";
    let mut failures = Vec::new();
    for (fixture, accepted) in [
        (
            "pass/facts/selected_value_application_through_target_body_exit",
            true,
        ),
        (
            "fail/facts/selected_value_application_through_target_body_rejected",
            false,
        ),
    ] {
        let source = fixture_package_inputs::repo_root()
            .join("tests/omega")
            .join(fixture)
            .join("main.omg");
        let result = fixture_package_inputs::reviewed_repository_fixture_package_inputs(
            &source,
            Some(target),
        )
        .and_then(|mut prepared| {
            fixture_package_inputs::check_fixture(&mut prepared, target, Default::default())
        });
        match result {
            Ok(_) if accepted => eprintln!("SOURCE_CHECK {fixture}: accepted"),
            Err(diagnostics)
                if !accepted
                    && diagnostics.iter().any(|diagnostic| {
                        diagnostic
                            .message
                            .contains("local `difference` may overflow `u32`")
                    }) =>
            {
                eprintln!("SOURCE_CHECK {fixture}: intended index refusal {diagnostics:?}")
            }
            Ok(_) => failures.push(format!("{fixture}: invalid runtime index accepted")),
            Err(diagnostics) => failures.push(format!("{fixture}: {diagnostics:?}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
