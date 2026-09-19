//! Ordinary fixture production retains package acceptance without asking the
//! offered program to authorize its own admission into a receiving environment.

use super::{
    CanaryCompileProduct, CanaryCompileSpec, CompileReport, compile_native_canary_without_output,
    compile_rooted_backend_canary_without_output, native_hosted_target, pass_canary,
    production_compile, reviewed_repository_fixture_package_inputs, unique_no_output_build_dir,
};

fn assert_no_receiver_admission(report: &CompileReport) {
    let artifact = report
        .retained_native_artifact()
        .expect("ordinary native production retains its checked artifact");
    assert_eq!(
        artifact.terminal_authority_permission_policy_identity(),
        None
    );
    let leaves = artifact.terminal_authority_closure_review().leaves();
    assert!(
        !leaves.is_empty(),
        "Console must retain its physical mechanisms"
    );
    assert!(
        leaves.iter().all(|leaf| leaf.permitted().is_none()),
        "package acceptance cannot manufacture a receiver-admission verdict",
    );
}

#[test]
fn ordinary_native_helpers_do_not_invent_receiver_admission() {
    let canary = pass_canary("host/runtime_console_byte_literal_exit");
    let inputs = reviewed_repository_fixture_package_inputs(
        &canary.join("main.omg"),
        Some(native_hosted_target()),
    )
    .expect("fixture package review")
    .expect("Console retains ordinary package inputs");
    assert!(
        inputs
            .accepted_semantic_bindings()
            .flat_map(|binding| binding.terminal_authority_permissions())
            .next()
            .is_some(),
        "this control must exercise nonempty project acceptance",
    );
    for compile_fixture in [
        compile_native_canary_without_output,
        compile_rooted_backend_canary_without_output,
    ] {
        let report = compile_fixture(&canary).expect("ordinary native fixture production");
        assert_no_receiver_admission(&report);
    }
}

#[test]
fn published_native_product_does_not_invent_receiver_admission() {
    let canary = pass_canary("host/runtime_console_byte_literal_exit");
    let scratch = unique_no_output_build_dir();
    let report = production_compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(scratch.clone()),
        target_name: Some(native_hosted_target().into()),
        product: CanaryCompileProduct::NativeArtifact,
    })
    .expect("ordinary production retains a native executable");
    assert_no_receiver_admission(&report);
    let report = report
        .publish_retained_native_artifact(&scratch)
        .expect("publish the same checked native artifact");
    let output = std::process::Command::new(
        report
            .checked_native_executable_path()
            .expect("published executable receipt"),
    )
    .output()
    .expect("execute the published Console program on the matching host");
    assert_eq!(output.status.code(), Some(70));
    assert_eq!(output.stdout, b"7\n");
    std::fs::remove_dir_all(scratch).expect("remove this test's scratch project");
}
