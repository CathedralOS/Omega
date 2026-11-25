use crate::register_model::*;
use ::register_model::TargetRegisterEnvironmentConstraintKeys;
use selected_instructions::SelectedConstraintKeys;
use target::{Architecture, NativeTarget, ObjectFormat};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64SelectedAbi {
    Aapcs64,
    /// AAPCS64 with x18 reserved for the platform, as on Apple (Darwin) and
    /// Windows. Calls preserve x18 and the allocator never assigns it.
    Darwin,
}

pub fn aarch64_selected_abi(target: NativeTarget) -> Option<Aarch64SelectedAbi> {
    match (target.architecture, target.object_format) {
        (Architecture::Aarch64, ObjectFormat::Elf) => Some(Aarch64SelectedAbi::Aapcs64),
        (Architecture::Aarch64, ObjectFormat::MachO | ObjectFormat::Coff) => {
            Some(Aarch64SelectedAbi::Darwin)
        }
        (Architecture::X86_64, ObjectFormat::Elf)
        | (Architecture::X86_64, ObjectFormat::MachO)
        | (Architecture::X86_64, ObjectFormat::Coff) => None,
    }
}

pub fn aarch64_selected_constraint_keys(target: NativeTarget) -> Option<SelectedConstraintKeys> {
    let abi = aarch64_selected_abi(target)?;
    let darwin = abi == Aarch64SelectedAbi::Darwin;
    let return_i64 = match abi {
        Aarch64SelectedAbi::Aapcs64 => AARCH64_AAPCS64_RETURN,
        Aarch64SelectedAbi::Darwin => AARCH64_DARWIN_RETURN,
    };
    let return_unit = match abi {
        Aarch64SelectedAbi::Aapcs64 => AARCH64_AAPCS64_RETURN_UNIT,
        Aarch64SelectedAbi::Darwin => AARCH64_DARWIN_RETURN_UNIT,
    };
    Some({
        let rows = TargetRegisterEnvironmentConstraintKeys {
            port_write: None,
            system_register_read: Some(crate::AARCH64_SYSTEM_REGISTER_READ),
            system_register_write: Some(crate::AARCH64_SYSTEM_REGISTER_WRITE),
            global_descriptor_table: None,
            task_register_activate: None,
            task_register_read: None,
            installed_interrupt_cpu_check: None,
            installed_interrupt_task_commit: None,
            installed_interrupt_table_publication: None,
            code_segment_reload: None,
            data_segment_reload: None,
            segment_selector_read: None,
            control_register_read: None,
            control_register_write: None,
            port_register_read: None,
            port_register_write: None,
            model_specific_read: None,
            model_specific_write: None,
            processor_halt: None,
            interrupt_control: None,
            memory_fence: Some(crate::AARCH64_MEMORY_FENCE),
            atomic_fence: Some(crate::AARCH64_ATOMIC_FENCE),
            access_read: Some(crate::AARCH64_ACCESSOR_READ),
            access_write: Some(crate::AARCH64_ACCESSOR_WRITE),
            atomic_load: Some(crate::AARCH64_ATOMIC_LOAD),
            atomic_store: Some(crate::AARCH64_ATOMIC_STORE),
            atomic_update: Some(crate::AARCH64_ATOMIC_UPDATE),
            atomic_bitwise_update: Some(crate::AARCH64_ATOMIC_UPDATE),
            atomic_compare_exchange: Some(crate::AARCH64_ATOMIC_COMPARE_EXCHANGE),
            atomic_compare_exchange_once: Some(crate::AARCH64_ATOMIC_COMPARE_EXCHANGE_ONCE),
            crash: crate::AARCH64_CRASH,
            privileged_crash: None,
            copy_bytes: Some(crate::AARCH64_COPY_BYTES),
            zero_bytes: Some(crate::AARCH64_ZERO_BYTES),
            scan_byte_sequence: Some(crate::AARCH64_SCAN_BYTE_SEQUENCE),
            load_encoded_scalar: Some(crate::AARCH64_LOAD_ENCODED_SCALAR),
            store_encoded_scalar: Some(crate::AARCH64_STORE_ENCODED_SCALAR),
            compare_byte_sequences: Some(crate::AARCH64_COMPARE_BYTE_SEQUENCES),
            save_floating_control: Some(crate::AARCH64_SAVE_FLOATING_CONTROL),
            restore_floating_control: Some(crate::AARCH64_RESTORE_FLOATING_CONTROL),
            load_packed: Some(crate::AARCH64_LOAD_PACKED),
            store_packed: Some(crate::AARCH64_STORE_PACKED),
            hosted_read_byte: if target == NativeTarget::linux_arm64() {
                Some(crate::AARCH64_HOSTED_READ_BYTE)
            } else if target == NativeTarget::macos_arm64() {
                Some(crate::AARCH64_DARWIN_HOSTED_READ_BYTE)
            } else {
                None
            },
            hosted_exit_process_i32: if target == NativeTarget::linux_arm64() {
                Some(crate::AARCH64_HOSTED_EXIT_PROCESS_I32)
            } else if target == NativeTarget::macos_arm64() {
                Some(crate::AARCH64_DARWIN_HOSTED_EXIT_PROCESS_I32)
            } else {
                None
            },
            hosted_write_byte_i32: if target == NativeTarget::linux_arm64() {
                Some(crate::AARCH64_HOSTED_WRITE_BYTE_I32)
            } else if target == NativeTarget::macos_arm64() {
                Some(crate::AARCH64_DARWIN_HOSTED_WRITE_BYTE_I32)
            } else {
                None
            },
            load64: Some(crate::AARCH64_LOAD64),
            load8: Some(crate::AARCH64_LOAD8),
            load16: Some(crate::AARCH64_LOAD16),
            load32: Some(crate::AARCH64_LOAD32),
            load8_indexed: Some(crate::AARCH64_LOAD8_INDEXED),
            scaled_indexed_load: None,
            store: Some(crate::AARCH64_STORE),
            address_offset: Some(crate::AARCH64_ADDRESS_OFFSET),
            store64: Some(crate::AARCH64_STORE64),
            frame_address: Some(crate::AARCH64_FRAME_ADDRESS),
            byte_literal_image_address: Some(crate::AARCH64_BYTE_LITERAL_IMAGE_ADDRESS),
            call_unit_mixed: match abi {
                Aarch64SelectedAbi::Aapcs64 => crate::aarch64_aapcs64_mixed_unit_call_keys(),
                Aarch64SelectedAbi::Darwin => crate::aarch64_darwin_mixed_unit_call_keys(),
            },
            call_unit: match abi {
                Aarch64SelectedAbi::Aapcs64 => crate::aarch64_aapcs64_register_unit_call_keys(),
                Aarch64SelectedAbi::Darwin => crate::aarch64_darwin_register_unit_call_keys(),
            },
            call_scalar: (match abi {
                Aarch64SelectedAbi::Aapcs64 => aarch64_aapcs64_register_call_keys(),
                Aarch64SelectedAbi::Darwin => crate::aarch64_darwin_register_call_keys(),
            })
            .into_iter()
            .chain(crate::aarch64_float_scalar_call_keys(darwin))
            .collect(),
            call_normalized_foreign: (match abi {
                Aarch64SelectedAbi::Aapcs64 => {
                    crate::aarch64_aapcs64_normalized_foreign_call_keys()
                }
                Aarch64SelectedAbi::Darwin => crate::aarch64_darwin_normalized_foreign_call_keys(),
            })
            .into_iter()
            .chain(
                (target.object_format == ObjectFormat::Elf)
                    .then(crate::aarch64_linux_system_call_keys)
                    .into_iter()
                    .flatten(),
            )
            .collect(),
            materialize_i64: AARCH64_MATERIALIZE_I64,
            materialize_boolean: crate::AARCH64_MATERIALIZE_BOOLEAN,
            call_aggregate: crate::aarch64_register_aggregate_call_keys(darwin)
                .into_iter()
                .chain(crate::aarch64_mixed_aggregate_call_keys(darwin))
                .chain(crate::aarch64_indirect_aggregate_call_keys(darwin))
                .collect(),
            return_aggregate: crate::aarch64_register_aggregate_return_keys(darwin),
            copy_i64: AARCH64_COPY_I64,
            float32_to_bits: Some(crate::AARCH64_FLOAT32_TO_BITS),
            float64_to_bits: Some(crate::AARCH64_FLOAT64_TO_BITS),
            ieee_float_arithmetic: Some(crate::AARCH64_IEEE_FLOAT_ARITHMETIC),
            ieee_float_directed_binary: Some(crate::AARCH64_IEEE_FLOAT_ARITHMETIC),
            ieee_float_directed_ternary: Some(crate::AARCH64_IEEE_FLOAT_TERNARY),
            ieee_float_directed_unary: Some(crate::AARCH64_IEEE_FLOAT_UNARY),
            ieee_float_unary: Some(crate::AARCH64_IEEE_FLOAT_UNARY),
            ieee_float_conversion_control: Some(crate::AARCH64_IEEE_FLOAT_UNARY),
            ieee_float_ternary: Some(crate::AARCH64_IEEE_FLOAT_TERNARY),
            float_result_trapping: Some(crate::AARCH64_FLOAT_RESULT_TRAPPING),
            float_result_saturating: Some(crate::AARCH64_FLOAT_RESULT_SATURATING),
            bits_to_float32: Some(crate::AARCH64_BITS_TO_FLOAT32),
            bits_to_float64: Some(crate::AARCH64_BITS_TO_FLOAT64),
            add_i64: AARCH64_ADD_I64,
            subtract_i64: AARCH64_SUBTRACT_I64,
            multiply_i64: crate::AARCH64_MULTIPLY_I64,
            saturating_subtract_unsigned:
                crate::register_model::AARCH64_SATURATING_SUBTRACT_UNSIGNED,
            saturating_add_u64: crate::register_model::AARCH64_SATURATING_ADD_U64,
            divide_u64: crate::register_model::AARCH64_DIVIDE_U64,
            remainder_u64: crate::register_model::AARCH64_REMAINDER_U64,
            remainder_i64: crate::register_model::AARCH64_REMAINDER_I64,
            divide_i64: crate::register_model::AARCH64_DIVIDE_I64,
            shift_i64: crate::register_model::AARCH64_SHIFT_I64,
            saturating_add_clamped: crate::register_model::AARCH64_SATURATING_ADD_CLAMPED,
            saturating_subtract_clamped: crate::register_model::AARCH64_SATURATING_SUBTRACT_CLAMPED,
            saturating_divide_signed: crate::register_model::AARCH64_SATURATING_DIVIDE_SIGNED,
            saturating_multiply_clamped: crate::register_model::AARCH64_SATURATING_MULTIPLY_CLAMPED,
            saturating_multiply_u64: crate::register_model::AARCH64_SATURATING_MULTIPLY_U64,
            trapping_binary: crate::register_model::AARCH64_TRAPPING_BINARY,
            trapping_fixed_pair: crate::register_model::AARCH64_TRAPPING_FIXED_PAIR,
            trapping_shift: crate::register_model::AARCH64_TRAPPING_SHIFT,
            trapping_convert: crate::register_model::AARCH64_TRAPPING_CONVERT,
            add_i64_immediate: AARCH64_ADD_I64_IMMEDIATE,
            subtract_i64_immediate: AARCH64_SUBTRACT_I64_IMMEDIATE,
            compare_i64_zero: AARCH64_COMPARE_I64_ZERO,
            compare_i64: AARCH64_COMPARE_I64,
            compare_i64_immediate: AARCH64_COMPARE_I64_IMMEDIATE,
            conditional_branch: AARCH64_CONDITIONAL_BRANCH,
            jump: crate::AARCH64_JUMP,
            return_float: crate::aarch64_float_scalar_return_keys(darwin),
            return_i64,
            return_unit,
        };
        let mut completed = SelectedConstraintKeys {
            rows,
            invoke_no_normal: Vec::new(),
        };
        completed.invoke_no_normal = completed.ordinary_call_constraints();
        completed
    })
}
