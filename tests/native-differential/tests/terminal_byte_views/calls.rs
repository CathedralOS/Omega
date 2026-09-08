//! Mixed scalar/reference calls retain native frames and resolved call occurrences.

use machine_emission::StagedOptimizedFixedFrameTextSection;
use native_realization::{compiler_baseline_request_v1, optimize_artifact_sections};
use optimization_core::OptimizationSelections;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{MachineId, OperationId};
use target::NativeTarget;
use terminal_psi::TerminalModule;

use super::fixtures::{
    byte_view_conditional_read_call_module, byte_view_read_call_module, byte_view_read_proof,
};

pub(super) fn stage_call_text(
    target: NativeTarget,
    module: &TerminalModule,
) -> StagedOptimizedFixedFrameTextSection {
    let proof = byte_view_read_proof(module);
    stage_call_text_with_proof(target, module, &proof)
}

pub(super) fn stage_call_text_with_proof(
    target: NativeTarget,
    module: &TerminalModule,
    proof: &terminal_verifier::ProofBundle,
) -> StagedOptimizedFixedFrameTextSection {
    stage_call_text_with_settlements(target, module, proof, &[])
}

pub(super) fn stage_call_text_with_settlements(
    target: NativeTarget,
    module: &TerminalModule,
    proof: &terminal_verifier::ProofBundle,
    settlements: &[abstract_operations_to_target_operations::AdmittedBoundarySettlement<'_>],
) -> StagedOptimizedFixedFrameTextSection {
    let semantic = terminal_codec::encode_module(module).unwrap();
    let proof = terminal_codec::encode_proof_bundle(proof).unwrap();
    let selections = OptimizationSelections::new([]).unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        compiler_baseline_request_v1(&selections),
    )
    .expect("guarded helper calls verify before native projection");
    let physical =
        native_realization::stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target,
            settlements,
        )
        .expect("mixed byte-view calls reach the complete physical pipeline");
    let fragments = machine_emission::stage_optimized_function_fragment_emission(
        physical.into_function_fragment_emission_source(),
    )
    .unwrap();
    let framed = machine_emission::stage_function_fragment_frame_application(fragments).unwrap();
    let placed = machine_emission::stage_optimized_fixed_frame_text_section(framed).unwrap();
    machine_emission::validate_optimized_fixed_frame_text_section(&placed).unwrap();
    placed
}

fn stage_read_calls(
    target: NativeTarget,
    module: &TerminalModule,
) -> StagedOptimizedFixedFrameTextSection {
    let placed = stage_call_text(target, module);
    let text = placed.text_section();
    assert_eq!(
        text.functions.len(),
        2,
        "the reader remains a separate function"
    );
    let calls = text
        .resolved_internal_machine_calls
        .iter()
        .map(|call| (call.caller, call.operation, call.callee))
        .collect::<Vec<_>>();
    assert_eq!(
        calls,
        [105, 107].map(|operation| (
            module.entry,
            OperationId::new(operation).unwrap(),
            MachineId::new(1).unwrap(),
        )),
        "both authored calls remain physically resolved, not substituted inline"
    );
    placed
}

#[test]
fn byte_view_read_call_cross_lowers_on_hosted_targets() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let placed = stage_read_calls(target, &byte_view_read_call_module());
        assert!(!placed.text_section().bytes.is_empty());
    }
}

#[test]
fn byte_view_read_call_executes_with_surviving_index_and_descriptor() {
    #[cfg(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64"),
    ))]
    {
        let placed = stage_read_calls(NativeTarget::host(), &byte_view_read_call_module());
        let text = placed.text_section();
        let entry = text
            .functions
            .iter()
            .find(|function| function.machine == MachineId::new(100).unwrap())
            .unwrap();
        super::native_function::assert_c_text(
            &text.bytes,
            entry.section_offset.try_into().unwrap(),
            r#"
            #include <stdint.h>
            #include <stddef.h>
            #include <unistd.h>
            struct ByteView { const uint8_t *bytes; uint64_t length; };
            _Static_assert(sizeof(struct ByteView) == 16, "descriptor size");
            _Static_assert(offsetof(struct ByteView, length) == 8, "length offset");
            extern uint64_t omega_entry(uint64_t, const struct ByteView *);
            int main(void) {
                alarm(10);
                const uint8_t raw[] = { 0xff, 0x00, 0x80, 0x41 };
                const uint8_t other[] = { 0x17, 0xfe };
                struct ByteView view = { NULL, 0 };
                if (omega_entry(0, &view) != 256) return 1;
                if (omega_entry(UINT64_MAX, &view) != 256) return 2;
                view.bytes = raw; view.length = sizeof(raw);
                for (uint64_t position = 0; position < sizeof(raw); ++position)
                    if (omega_entry(position, &view) != raw[position]) return 3;
                if (omega_entry(sizeof(raw), &view) != 256) return 4;
                if (omega_entry(UINT64_MAX, &view) != 256) return 5;
                if (view.bytes != raw || view.length != sizeof(raw)) return 6;
                view.bytes = other; view.length = sizeof(other);
                if (omega_entry(0, &view) != 0x17) return 7;
                if (omega_entry(1, &view) != 0xfe) return 8;
                view.length = 1;
                if (omega_entry(1, &view) != 256) return 9;
                view.bytes = NULL; view.length = 0;
                if (omega_entry(0, &view) != 256) return 10;
                if (view.bytes != NULL || view.length != 0) return 11;
                return 0;
            }
        "#,
        );
    }
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64"),
    )))]
    eprintln!(
        "SKIP mixed byte-view call execution: Linux x86-64/AArch64 and macOS AArch64 cc hosts supported; Windows runtime route unavailable"
    );
}

#[test]
fn conditional_byte_view_read_calls_cross_lower_on_hosted_targets() {
    let module = byte_view_conditional_read_call_module();
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let placed = stage_read_calls(target, &module);
        assert!(!placed.text_section().bytes.is_empty());
    }
}

#[test]
fn conditional_byte_view_read_calls_execute_only_the_nonzero_arm() {
    #[cfg(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64"),
    ))]
    {
        let module = byte_view_conditional_read_call_module();
        let placed = stage_read_calls(NativeTarget::host(), &module);
        let text = placed.text_section();
        let entry = text
            .functions
            .iter()
            .find(|function| function.machine == module.entry)
            .unwrap();
        super::native_function::assert_c_text(
            &text.bytes,
            entry.section_offset.try_into().unwrap(),
            r#"
            #include <stdint.h>
            #include <stddef.h>
            #include <unistd.h>
            struct ByteView { const uint8_t *bytes; uint64_t length; };
            extern uint64_t omega_entry(uint64_t, const struct ByteView *);
            int main(void) {
                alarm(10);
                const uint8_t raw[] = { 0xff, 0x80, 0x00 };
                struct ByteView view = { raw, sizeof(raw) };
                if (omega_entry(0, &view) != 257) return 1;
                if (omega_entry(1, &view) != 0x80) return 2;
                if (omega_entry(2, &view) != 0x00) return 3;
                if (omega_entry(sizeof(raw), &view) != 256) return 4;
                if (omega_entry(UINT64_MAX, &view) != 256) return 5;
                view.bytes = NULL; view.length = 0;
                if (omega_entry(0, &view) != 257) return 6;
                if (omega_entry(1, &view) != 256) return 7;
                return 0;
            }
        "#,
        );
    }
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64"),
    )))]
    eprintln!(
        "SKIP conditional byte-view call execution: Linux/macOS cc hosts supported; Windows runtime route unavailable"
    );
}
