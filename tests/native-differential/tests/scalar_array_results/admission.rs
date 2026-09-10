//! Missing physical transports reject without replacing values with empty identities.
use super::*;

#[test]
fn zero_odd_and_indirect_result_fragments_remain_explicit_limits() {
    for (shape, value) in [
        ("[u8; 0]", "[]"),
        ("[u8; 3]", "[1u8, 2u8, 3u8]"),
        ("[u64; 3]", "[1u64, 2u64, 3u64]"),
    ] {
        let source = format!("machine selected() -> {shape} {{ {value} }}");
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
            let compiled = target_plan(&source, "selected", target).unwrap();
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            assert!(
                target_operations_to_selected_instructions::stage_optimized_instruction_selection(
                    compiled,
                    environment
                )
                .is_err(),
                "{shape} on {target:?}"
            );
        }
    }
}
