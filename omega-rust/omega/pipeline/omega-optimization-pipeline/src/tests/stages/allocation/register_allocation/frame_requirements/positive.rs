use crate::tests::*;

use super::fixture::{EXACT_USAGE, exact_budget, spill_source, stage};

#[test]
fn exact_target_matrix_retains_requirements_and_is_deterministic() {
    for (target, abi, red_zone) in [
        (
            NativeTarget::linux_x64(),
            FrameAbiPreservationConvention::SystemVAMD64,
            128,
        ),
        (
            NativeTarget::windows_x64(),
            FrameAbiPreservationConvention::MicrosoftX64,
            0,
        ),
        (
            NativeTarget::uefi_x64(),
            FrameAbiPreservationConvention::MicrosoftX64,
            0,
        ),
        (
            NativeTarget::linux_arm64(),
            FrameAbiPreservationConvention::Aapcs64,
            0,
        ),
        (
            NativeTarget::macos_arm64(),
            FrameAbiPreservationConvention::DarwinAapcs64,
            0,
        ),
    ] {
        let source = spill_source(target);
        let environment = baseline_target_register_environment(target).unwrap();
        let first = stage(&source, &environment, exact_budget()).unwrap();
        let repeated = stage(&source, &environment, exact_budget()).unwrap();
        assert_eq!(first, repeated);
        assert_eq!(
            first.receipt().abstract_spill_access_constraints(),
            source.receipt().identity()
        );
        assert_eq!(
            first.receipt().register_environment(),
            environment.identity()
        );
        assert_eq!(first.receipt().target(), target);
        assert_eq!(first.receipt().usage(), EXACT_USAGE);
        assert_eq!(first.receipt().function_count(), 1);
        assert_eq!(first.receipt().spill_bearing_function_count(), 1);
        assert_eq!(first.receipt().max_abstract_spill_area_bytes(), 16);
        assert_eq!(first.receipt().max_abstract_spill_area_alignment(), 8);
        assert_eq!(
            first.receipt().identity(),
            non_authoritative_spill_frame_requirement_identity(first.plan()),
        );
        let function = first.plan().functions[0];
        assert_eq!(function.abstract_spill_area_bytes, 16);
        assert_eq!(function.abstract_spill_area_alignment, 8);
        assert_eq!(function.abi_preservation_convention, abi);
        assert_eq!(function.abi_stack_alignment, 16);
        assert_eq!(function.abi_red_zone_capacity_bytes, red_zone);
        let replayed = validate_non_authoritative_spill_frame_requirements(
            &source,
            &environment,
            first.plan().clone(),
        )
        .unwrap();
        assert_eq!(replayed, first);
    }
}

#[test]
fn independent_zero_access_rows_retain_neutral_alignment_without_inventing_a_frame() {
    let machine = psi_core::MachineId::new(41_991).unwrap();
    let direct = derive_zero_access_requirement_for_test(machine);
    let replayed = replay_zero_access_requirement_for_test(machine);
    assert_eq!(direct, replayed);
    assert_eq!(direct.abstract_spill_area_bytes, 0);
    assert_eq!(direct.abstract_spill_area_alignment, 1);
    assert_eq!(
        direct.abi_preservation_convention,
        FrameAbiPreservationConvention::SystemVAMD64
    );
    assert_eq!(direct.abi_stack_alignment, 16);
    assert_eq!(direct.abi_red_zone_capacity_bytes, 128);
}
