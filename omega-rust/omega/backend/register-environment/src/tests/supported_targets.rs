use isa_aarch64::{
    AARCH64_ADD_I64, AARCH64_ADD_I64_IMMEDIATE, AARCH64_COPY_I64, AARCH64_SUBTRACT_I64,
};
use isa_x86_64::{
    X86_64_ADD_I64, X86_64_ADD_I64_IMMEDIATE, X86_64_COPY_I64, X86_64_FRAME_ADDRESS, X86_64_LOAD64,
    X86_64_MICROSOFT_CALL_UNIT, X86_64_STORE64, X86_64_SUBTRACT_I64,
};
use target::{Architecture, NativeTarget, ObjectFormat};

use super::super::*;

#[test]
fn every_supported_native_target_builds_a_matching_closed_environment() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        assert_eq!(environment.target(), target);
        assert_eq!(
            environment.physical().model().architecture,
            target.architecture
        );
        assert_eq!(
            environment.constraints().architecture(),
            target.architecture
        );
        assert_eq!(
            environment.constraints().catalog().required,
            environment
                .constraints()
                .catalog()
                .constraints
                .iter()
                .map(|constraint| constraint.key)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            environment.identity(),
            baseline_target_register_environment(target)
                .unwrap()
                .identity()
        );
        let (expected_copy, expected_add, expected_add_immediate, expected_subtract) =
            match target.architecture {
                Architecture::X86_64 => (
                    X86_64_COPY_I64,
                    X86_64_ADD_I64,
                    X86_64_ADD_I64_IMMEDIATE,
                    X86_64_SUBTRACT_I64,
                ),
                Architecture::Aarch64 => (
                    AARCH64_COPY_I64,
                    AARCH64_ADD_I64,
                    AARCH64_ADD_I64_IMMEDIATE,
                    AARCH64_SUBTRACT_I64,
                ),
            };
        assert_eq!(environment.selected_keys().copy_i64, expected_copy);
        assert_eq!(environment.selected_keys().add_i64, expected_add);
        assert_eq!(
            environment.selected_keys().add_i64_immediate,
            expected_add_immediate
        );
        assert_eq!(environment.selected_keys().subtract_i64, expected_subtract);
        assert_eq!(
            environment.allocation_constraint_keys().copy_i64,
            expected_copy
        );
        assert_eq!(
            environment.allocation_constraint_keys().add_i64,
            expected_add
        );
        assert_eq!(
            environment.allocation_constraint_keys().add_i64_immediate,
            expected_add_immediate
        );
        assert_eq!(
            environment.allocation_constraint_keys().subtract_i64,
            expected_subtract
        );
        assert!(environment.constraint(expected_copy).is_some());
        assert!(environment.constraint(expected_add).is_some());
        assert!(environment.constraint(expected_add_immediate).is_some());
        assert!(environment.constraint(expected_subtract).is_some());
        let microsoft = matches!(
            (target.architecture, target.object_format),
            (Architecture::X86_64, ObjectFormat::Coff)
        );
        for (selected_key, allocation_key, key, operands) in [
            (
                environment.selected_keys().load64,
                environment.allocation_constraint_keys().load64,
                X86_64_LOAD64,
                2,
            ),
            (
                environment.selected_keys().store64,
                environment.allocation_constraint_keys().store64,
                X86_64_STORE64,
                1,
            ),
            (
                environment.selected_keys().frame_address,
                environment.allocation_constraint_keys().frame_address,
                X86_64_FRAME_ADDRESS,
                1,
            ),
            (
                environment.selected_keys().call_unit,
                environment.allocation_constraint_keys().call_unit,
                X86_64_MICROSOFT_CALL_UNIT,
                2,
            ),
        ] {
            let expected = microsoft.then_some(key);
            assert_eq!(selected_key, expected);
            assert_eq!(allocation_key, expected);
            if let Some(key) = expected {
                assert_eq!(
                    environment
                        .constraint(key)
                        .expect("applicable ordinary ABI primitive")
                        .operands
                        .len(),
                    operands
                );
            }
        }
    }
}
