use omega_selected_instructions::{MachineLatencyKnowledge, MachineSizeKnowledge};
use omega_target::{Architecture, NativeTarget, ObjectFormat};

use super::{
    NonAuthoritativeLatencyCost, NonAuthoritativeMachineSizeCost, TargetCostModelVersion,
    target_cost_model,
};

#[test]
fn identity_is_deterministic_and_covers_the_complete_native_target() {
    let linux_x64 = target_cost_model(NativeTarget::linux_x64());
    assert_eq!(linux_x64, target_cost_model(NativeTarget::linux_x64()));
    assert_eq!(
        linux_x64.version(),
        TargetCostModelVersion::MachineKnowledgeV1
    );
    assert_eq!(linux_x64.target(), NativeTarget::linux_x64());
    assert_ne!(linux_x64.identity().bytes(), [0; 32]);

    for distinct in [
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget {
            architecture: Architecture::X86_64,
            object_format: ObjectFormat::Elf,
            pointer_size: 4,
            pointer_alignment: 4,
        },
    ] {
        assert_ne!(
            linux_x64.identity(),
            target_cost_model(distinct).identity(),
            "target distinction {distinct:?} must remain in model identity",
        );
    }
}

#[test]
fn exact_size_remains_exact_and_latency_remains_unavailable() {
    let model = target_cost_model(NativeTarget::linux_x64());
    let observation = model.observe(
        MachineSizeKnowledge::ExactBytes(3),
        MachineLatencyKnowledge::StableBaselineUnavailable,
    );

    assert_eq!(observation.model(), model.identity());
    assert_eq!(
        observation.size(),
        NonAuthoritativeMachineSizeCost::ExactBytes(3)
    );
    assert_eq!(observation.size().minimum_bytes(), 3);
    assert_eq!(observation.size().maximum_bytes(), Some(3));
    assert_eq!(observation.size().exact_bytes(), Some(3));
    assert_eq!(
        observation.latency(),
        NonAuthoritativeLatencyCost::Unavailable
    );
}

#[test]
fn encoder_resolved_bounds_are_not_promoted_to_exact_costs() {
    let model = target_cost_model(NativeTarget::linux_arm64());
    for (maximum_bytes, expected_maximum) in [(Some(16), Some(16)), (None, None)] {
        let observation = model.observe(
            MachineSizeKnowledge::EncoderResolved {
                minimum_bytes: 4,
                maximum_bytes,
            },
            MachineLatencyKnowledge::StableBaselineUnavailable,
        );

        assert_eq!(
            observation.size(),
            NonAuthoritativeMachineSizeCost::EncoderResolved {
                minimum_bytes: 4,
                maximum_bytes,
            }
        );
        assert_eq!(observation.size().minimum_bytes(), 4);
        assert_eq!(observation.size().maximum_bytes(), expected_maximum);
        assert_eq!(observation.size().exact_bytes(), None);
    }
}
