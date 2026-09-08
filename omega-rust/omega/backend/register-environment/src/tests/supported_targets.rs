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
        let unit_keys = &environment.selected_keys().call_unit;
        let expected_write = match (target.architecture, target.object_format) {
            (Architecture::X86_64, ObjectFormat::Elf) => {
                Some(isa_x86_64::X86_64_HOSTED_WRITE_BYTE_I32)
            }
            (Architecture::Aarch64, ObjectFormat::Elf) => {
                Some(isa_aarch64::AARCH64_HOSTED_WRITE_BYTE_I32)
            }
            (Architecture::Aarch64, ObjectFormat::MachO) => {
                Some(isa_aarch64::AARCH64_DARWIN_HOSTED_WRITE_BYTE_I32)
            }
            _ => None,
        };
        assert_eq!(
            environment.selected_keys().hosted_write_byte_i32,
            expected_write
        );
        assert_eq!(
            environment
                .allocation_constraint_keys()
                .hosted_write_byte_i32,
            expected_write
        );
        if let Some(key) = expected_write {
            let row = environment.constraint(key).unwrap();
            assert_eq!(row.operands.len(), 1);
            assert_eq!(
                row.operands[0].access,
                register_model::RegisterOperandAccess::Use
            );
            assert!(!row.clobbers.is_empty());
        }
        assert_eq!(
            unit_keys,
            &environment.allocation_constraint_keys().call_unit
        );
        assert_eq!(unit_keys.len(), environment.selected_keys().call_i64.len());
        if microsoft {
            assert_eq!(unit_keys[2], X86_64_MICROSOFT_CALL_UNIT);
        }
        for (arity, key) in unit_keys.iter().enumerate() {
            let row = environment.constraint(*key).unwrap();
            assert_eq!(row.operands.len(), arity);
            assert!(
                row.operands
                    .iter()
                    .all(|operand| operand.access == register_model::RegisterOperandAccess::Use)
            );
        }
        let load = match target.architecture {
            Architecture::X86_64 => X86_64_LOAD64,
            Architecture::Aarch64 => isa_aarch64::AARCH64_LOAD64,
        };
        assert_eq!(environment.selected_keys().load64, Some(load));
        assert_eq!(environment.allocation_constraint_keys().load64, Some(load));
        assert_eq!(environment.constraint(load).unwrap().operands.len(), 2);
        let indexed_load = match target.architecture {
            Architecture::X86_64 => isa_x86_64::X86_64_LOAD8_INDEXED,
            Architecture::Aarch64 => isa_aarch64::AARCH64_LOAD8_INDEXED,
        };
        assert_eq!(
            environment.selected_keys().load8_indexed,
            Some(indexed_load)
        );
        assert_eq!(
            environment.allocation_constraint_keys().load8_indexed,
            Some(indexed_load)
        );
        assert_eq!(
            environment.constraint(indexed_load).unwrap().operands.len(),
            3
        );
        for (selected_key, allocation_key, key, operands) in [
            (
                environment.selected_keys().store,
                environment.allocation_constraint_keys().store,
                match target.architecture {
                    Architecture::X86_64 => isa_x86_64::X86_64_STORE,
                    Architecture::Aarch64 => isa_aarch64::AARCH64_STORE,
                },
                2,
            ),
            (
                environment.selected_keys().address_offset,
                environment.allocation_constraint_keys().address_offset,
                match target.architecture {
                    Architecture::X86_64 => isa_x86_64::X86_64_ADDRESS_OFFSET,
                    Architecture::Aarch64 => isa_aarch64::AARCH64_ADDRESS_OFFSET,
                },
                2,
            ),
            (
                environment.selected_keys().store64,
                environment.allocation_constraint_keys().store64,
                match target.architecture {
                    Architecture::X86_64 => X86_64_STORE64,
                    Architecture::Aarch64 => isa_aarch64::AARCH64_STORE64,
                },
                1,
            ),
            (
                environment.selected_keys().frame_address,
                environment.allocation_constraint_keys().frame_address,
                match target.architecture {
                    Architecture::X86_64 => X86_64_FRAME_ADDRESS,
                    Architecture::Aarch64 => isa_aarch64::AARCH64_FRAME_ADDRESS,
                },
                1,
            ),
        ] {
            let expected = Some(key);
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
