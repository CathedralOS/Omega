//! Native persistence rejoins canonical external satisfaction without a saved
//! source-classified numeric execution row. Exercise a call and comparisons.

use super::{execute_build, fixture_package_inputs};
use compiler::{
    CheckedCompileRequest, CompilationSession, CompileOptions, CompileRequest,
    RequestedCompileProduct,
};
use std::sync::Arc;
use terminal_psi_to_abstract_operations::provider_admission;

#[test]
fn numeric_native_inputs_reload_from_canonical_numeric_satisfaction() {
    let target = "macos_arm64";
    let fixture = fixture_package_inputs::repo_root()
        .join("tests/omega/pass/float/named_provider_negate_is_nan_exit/main.omg");
    let prepared =
        fixture_package_inputs::reviewed_repository_fixture_package_inputs(&fixture, Some(target))
            .expect("review numeric fixture inputs");
    let mut session = CompilationSession::default();
    let build = execute_build(
        &mut session,
        CheckedCompileRequest {
            package_inputs: prepared.package_inputs.clone(),
            ..CheckedCompileRequest::new(&prepared.root_path, Some(target))
        },
    );
    let checked = session
        .check_package_build(build)
        .expect("checked numeric source");
    let mut request = CompileRequest::new(CompileOptions {
        root_path: prepared.root_path,
        build_dir: None,
        target_name: Some(target.into()),
    })
    .with_requested_product(RequestedCompileProduct::TerminalArtifact);
    if let Some(inputs) = prepared.package_inputs {
        request = request.with_package_inputs(inputs);
    }
    let retained = omega_build::compile_sources(&mut session, request)
        .and_then(compiler::CompileOutcomes::into_single_report)
        .expect("numeric Terminal production")
        .into_retained_terminal_artifact()
        .expect("retained numeric proposal");
    let proposal = retained
        .native_realization_proposal()
        .expect("native proposal");
    let selections = optimization_core::OptimizationRollback::default()
        .settle(&checked.build().config.settings.optimizations);
    let request = checked
        .terminal_native_proposal_request(selections.effective(), &[])
        .expect("source-owned derivation inputs");
    let permissions = provider_admission::terminal_authority_permission_policy_with_rows(
        proposal.package_terminal_authority_permissions().to_vec(),
        proposal
            .package_terminal_authority_permission_identities()
            .clone(),
    )
    .expect("package permission policy");
    let bytes = native_production::retain_native_derivation_inputs(
        proposal.program_entry().checked_entry(),
        proposal.program_entry().physical_adapter().is_some(),
        proposal.target_provider_proposals(),
        &request,
        &permissions,
    )
    .expect("persist native derivation inputs");
    let artifact = Arc::new(
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&retained.artifact().to_bytes())
            .expect("reload canonical Terminal"),
    );
    drop(request);
    drop(checked);
    drop(session);
    let (recovered, recovered_permissions) =
        terminal_psi_to_abstract_operations::recover_native_derivation_inputs(artifact, &bytes)
            .expect("recover native inputs without source");
    assert_eq!(recovered_permissions, permissions);
    assert_eq!(recovered, retained);
    let proposal = recovered.native_realization_proposal().unwrap();
    assert!(
        proposal
            .providers()
            .intrinsic_executions
            .iter()
            .flatten()
            .all(|execution| execution.is_none_or(|execution| !execution.is_numeric()))
    );
    assert!(
        recovered
            .artifact()
            .semantic_module()
            .numeric_realizations
            .iter()
            .any(|candidate| candidate.numeric_intrinsic.is_some())
    );
    proposal
        .prepare_native_callable_execution(
            recovered.artifact(),
            recovered.artifact().semantic_module().entry,
        )
        .expect("reloaded numeric providers settle executable demand");
}
