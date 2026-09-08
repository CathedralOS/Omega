//! Mixed scalar/view helpers retain runtime inputs and original reference identity.
use super::*;
#[path = "mixed_helpers/calls.rs"]
mod calls;
#[path = "mixed_helpers/proof.rs"]
mod proof;

#[test]
fn mixed_reordered_repeated_nested_and_result_fed_calls_cross_lower() {
    cross_lower(&calls::composed(), 3);
}

#[test]
fn mixed_conditional_calls_cross_lower() {
    cross_lower(&calls::conditional(), 2);
}

fn cross_lower(module: &TerminalModule, function_count: usize) {
    let proof = proof::for_module(module);
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let layout = stage_byte_view(module, &proof, target);
        assert_eq!(layout.functions().len(), function_count);
    }
}

#[test]
fn mixed_reordered_repeated_nested_and_result_fed_calls_execute() {
    execute(
        &calls::composed(),
        5,
        include_str!("mixed_helpers/composed.c"),
    );
}

#[test]
fn mixed_conditional_calls_execute_only_on_selected_branch() {
    execute(
        &calls::conditional(),
        2,
        include_str!("mixed_helpers/conditional.c"),
    );
}

#[cfg(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "aarch64"),
))]
fn execute(module: &TerminalModule, call_count: usize, driver: &str) {
    let semantic = terminal_codec::encode_module(module).unwrap();
    let proof = terminal_codec::encode_proof_bundle(&proof::for_module(module)).unwrap();
    let selections = OptimizationSelections::new([]).unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        compiler_baseline_request_v1(&selections),
    )
    .expect("canonical mixed helper reaches optimizer");
    let physical =
        native_realization::stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            NativeTarget::host(),
            &[],
        )
        .expect("mixed helper reaches full physical pipeline");
    let fragments = machine_emission::stage_optimized_function_fragment_emission(
        physical.into_function_fragment_emission_source(),
    )
    .unwrap();
    let framed = machine_emission::stage_function_fragment_frame_application(fragments).unwrap();
    let placed = machine_emission::stage_optimized_fixed_frame_text_section(framed).unwrap();
    let text = placed.text_section();
    assert_eq!(
        text.resolved_internal_machine_calls.len(),
        call_count,
        "each authored call has one emitted call site even when its result is reused"
    );
    let entry = text
        .functions
        .iter()
        .find(|function| function.machine == module.entry)
        .unwrap();
    native_function::assert_c_text(
        &text.bytes,
        entry.section_offset.try_into().unwrap(),
        driver,
    );
}

#[cfg(not(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "aarch64"),
)))]
fn execute(_module: &TerminalModule, _call_count: usize, _driver: &str) {
    eprintln!(
        "SKIP mixed helper execution: existing cc harness supports Linux x86-64/AArch64 and macOS AArch64; Windows runtime route unavailable"
    );
}

#[test]
fn mixed_helpers_still_require_the_callee_guard_proof() {
    for module in [calls::composed(), calls::conditional()] {
        let mut missing_reader = proof::for_module(&module);
        missing_reader.evidence.remove(0);
        assert_byte_read_proof_rejected(&module, &missing_reader);
    }
}

#[test]
fn mixed_exact_sums_require_evidence_and_the_selected_operand_guards() {
    for module in [calls::composed(), calls::conditional()] {
        let proof = proof::for_module(&module);
        for evidence in proof.evidence.iter().skip(1) {
            let mut missing = proof.clone();
            missing
                .evidence
                .retain(|entry| entry.obligation != evidence.obligation);
            assert_byte_read_proof_rejected(&module, &missing);
        }
        let mut wrong_guard = module;
        let guard = wrong_guard
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .flat_map(|block| &mut block.operations)
            .find(|operation| operation.id == OperationId::new(1001).unwrap())
            .expect("first sum's left operand guard");
        let OperationKind::IntegerLessOrEqual { left, right } = &mut guard.kind else {
            panic!("exact sum operand bound");
        };
        std::mem::swap(left, right);
        assert_byte_read_proof_rejected(&wrong_guard, &proof);
    }
}
