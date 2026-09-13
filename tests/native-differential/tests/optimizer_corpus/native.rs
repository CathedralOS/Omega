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
