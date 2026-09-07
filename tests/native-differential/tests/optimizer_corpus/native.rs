//! Scalar host caller for the optimizer corpus.

use selected_form_encoding_to_resolved_layout::StagedOptimizedResolvedSelectedFormLayout;

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
