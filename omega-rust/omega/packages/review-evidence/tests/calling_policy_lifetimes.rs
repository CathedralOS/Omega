//! Calling lifetime applications remain relative to their containing telescope.

use crate::support;

use compiler::CheckedCompileRequest;
use package_evidence::encoding::PackagePolicyRecoveryLimits;
use package_evidence::{project_checked_calling_policy, record::PackagePolicyCallingPlan};
use support::*;

fn policy(declaration: &str) -> PackagePolicyCallingPlan {
    let fixture = fs::read_to_string(
        repository_root().join("source/library/std/tests/direct_callback_parameter.omg"),
    )
    .unwrap();
    // `use calling` would import the now-public standard `calling` module,
    // colliding with the package-local copy written beside main.omg.
    let prefix = fixture
        .split_once("boundary trait HookProcedure:")
        .unwrap()
        .0
        .replace("use calling;\n", "");
    let package = TempPackage::new();
    package.write(
        "main.omg",
        &format!("{prefix}\n{declaration}")
            .replace("use calling;", "use omega_language_std::calling;"),
    );
    package.write(
        "build.omg",
        "machine build(builder: &mut Build) { Build::package(builder, \"review_fixture\"); }\n",
    );
    let checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs_with_std(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
    })
    .expect("calling lifetime fixture checks");
    let owner = checked
        .traits()
        .iter()
        .find(|owner| owner.name.as_str() == "HookProcedure")
        .unwrap();
    let realization = checked
        .custody
        .providers()
        .boundary_calling_plan_realizations
        .iter()
        .find(|realization| realization.boundary_trait == owner.symbol)
        .unwrap();
    let policy = project_checked_calling_policy(&checked, realization).unwrap();
    let bytes = policy.canonical_bytes().unwrap();
    assert_eq!(
        PackagePolicyCallingPlan::recover_canonical(&bytes, PackagePolicyRecoveryLimits::default())
            .unwrap(),
        policy
    );
    policy
}

#[test]
fn inherited_lifetime_application_keeps_root_ordinal() {
    let declaration = r#"
boundary trait ProcedureBase<'scope> {
    machine call(message: &'scope u64) -> u64;
}
boundary trait HookProcedure<'first, 'second>:
    ProcedureBase<'second> + Calling<HookProcedurePolicy> {}
"#;
    let second = policy(declaration);
    let first = policy(&declaration.replace("ProcedureBase<'second>", "ProcedureBase<'first>"));
    assert_eq!(second.boundary_lifetime_parameter_count(), 2);
    assert_eq!(second.requirement_lifetime_arguments(), &[1]);
    assert_eq!(first.requirement_lifetime_arguments(), &[0]);
    assert_eq!(second.requirement_lifetime_parameter_count(), 0);
    assert_eq!(first.physical(), second.physical());
    assert_ne!(first.semantic_parameters(), second.semantic_parameters());
    assert_ne!(
        first.canonical_bytes().unwrap(),
        second.canonical_bytes().unwrap()
    );
}
