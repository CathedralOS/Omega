//! Scalar host caller for the optimizer corpus.

use abstract_operations_to_target_operations::lower_optimized_to_target_operations;
use native_realization::{compiler_baseline_request_v1, optimize_artifact_sections};
use optimization_core::OptimizationSelections;
use proof_admission::AdmissionProfile;
use selected_form_encoding_to_resolved_layout::StagedOptimizedResolvedSelectedFormLayout;
use target::NativeTarget;

use super::psi::CorpusArtifact;

#[path = "../common/native_function.rs"]
mod native_function;

pub(super) fn assert_u64_result(layout: &StagedOptimizedResolvedSelectedFormLayout, expected: u64) {
    native_function::assert_c_driver(
        layout,
        &format!(
            "#include <stdint.h>\nextern uint64_t omega_entry(uint8_t);\nint main(void) {{ return omega_entry(0) == {expected}ULL && omega_entry(1) == {expected}ULL ? 0 : 1; }}\n"
        ),
    );
}

pub(super) fn assert_bool_result(
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    expected: bool,
) {
    let expected_literal = if expected { "true" } else { "false" };
    native_function::assert_c_driver(
        layout,
        &format!(
            "#include <stdbool.h>\n#include <stdint.h>\nextern bool omega_entry(uint8_t);\nint main(void) {{ return omega_entry(0) == {expected_literal} && omega_entry(1) == {expected_literal} ? 0 : 1; }}\n"
        ),
    );
}

/// The atomic lane's arms may disagree: `omega_entry(0)` must equal
/// `when_false` and `omega_entry(1)` must equal `when_true`. Atomic
/// establishment results own frame-local aggregate storage, so the executed
/// text comes from the production fixed-frame fragment path (physical pipeline
/// -> fragment emission -> frame application -> placed text) rather than raw
/// layout rows, which carry no prologue/epilogue.
pub(super) fn assert_bool_result_arms_atomic(
    artifact: &CorpusArtifact,
    when_false: bool,
    when_true: bool,
) {
    let optimized = optimize_artifact_sections(
        &artifact.semantic,
        &artifact.proof,
        &AdmissionProfile::default(),
        compiler_baseline_request_v1(&OptimizationSelections::new([]).unwrap()),
    )
    .expect("atomic host-native artifact should admit baseline optimization");
    let post_terminal = optimized.selections().project_post_terminal();
    let target = lower_optimized_to_target_operations(optimized, NativeTarget::host())
        .expect("atomic host-native artifact should lower to target operations");
    let physical = native_realization::stage_optimized_verified_physical_pipeline(
        target,
        post_terminal.selections(),
    )
    .expect("atomic host-native artifact should complete the physical pipeline");
    let fragments = machine_emission::stage_optimized_function_fragment_emission(
        physical.into_function_fragment_emission_source(),
    )
    .expect("atomic host-native artifact should emit function fragments");
    let framed = machine_emission::stage_function_fragment_frame_application(fragments)
        .expect("atomic host-native artifact should apply its frame protocol");
    let text = machine_emission::stage_optimized_fixed_frame_text_section(framed)
        .expect("atomic host-native artifact should place its fixed-frame text");
    machine_emission::validate_optimized_fixed_frame_text_section(&text)
        .expect("atomic host-native text section should replay");
    let section = text.text_section();
    let false_literal = if when_false { "true" } else { "false" };
    let true_literal = if when_true { "true" } else { "false" };
    native_function::assert_c_text(
        &section.bytes,
        usize::try_from(section.semantic_entry_offset).unwrap(),
        &format!(
            "#include <stdbool.h>\n#include <stdint.h>\nextern bool omega_entry(uint8_t);\nint main(void) {{ return omega_entry(0) == {false_literal} && omega_entry(1) == {true_literal} ? 0 : 1; }}\n"
        ),
    );
}
