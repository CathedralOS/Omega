//! Array results that once lacked physical transports now select on every
//! target: packed fragments cover odd byte sizes and indirect destinations
//! carry the rest, so a missing transport would still reject rather than
//! substitute an empty identity — each row asserts the transport exists.

use super::{NativeTarget, target_plan};
#[test]
fn indirect_result_fragments_have_transports_on_every_target() {
    for (shape, value) in [
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
                .is_ok(),
                "{shape} on {target:?}"
            );
        }
    }
}
