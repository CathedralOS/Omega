use isa_aarch64::{
    AARCH64_ADD_I64, AARCH64_ADD_I64_IMMEDIATE, AARCH64_COPY_I64, AARCH64_SUBTRACT_I64,
};
use isa_x86_64::{
    X86_64_ADD_I64, X86_64_ADD_I64_IMMEDIATE, X86_64_COPY_I64,
    X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR, X86_64_SUBTRACT_I64,
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
        let expected_structural_call = matches!(
            (target.architecture, target.object_format),
            (Architecture::X86_64, ObjectFormat::Coff)
        )
        .then_some(X86_64_MICROSOFT_CALL_UNIT_OWNED_INDIRECT_PAIR);
        assert_eq!(
            environment.selected_keys().structural_unit_call,
            expected_structural_call
        );
        assert_eq!(
            environment
                .allocation_constraint_keys()
                .structural_unit_call,
            expected_structural_call
        );
        if let Some(key) = expected_structural_call {
            let row = environment
                .constraint(key)
                .expect("applicable structural Unit call row is catalog-owned");
            assert!(row.operands.is_empty());
        }
    }
}
