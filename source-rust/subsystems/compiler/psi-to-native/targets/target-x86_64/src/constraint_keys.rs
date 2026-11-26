use crate::register_model::*;
use ::register_model::TargetRegisterEnvironmentConstraintKeys;
use selected_instructions::SelectedConstraintKeys;
use target::{Architecture, NativeTarget, ObjectFormat};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64SelectedAbi {
    SystemV,
    Microsoft,
}

pub fn x86_64_selected_abi(target: NativeTarget) -> Option<X86_64SelectedAbi> {
    match (target.architecture, target.object_format) {
        (Architecture::X86_64, ObjectFormat::Elf | ObjectFormat::MachO) => {
            Some(X86_64SelectedAbi::SystemV)
        }
        (Architecture::X86_64, ObjectFormat::Coff) => Some(X86_64SelectedAbi::Microsoft),
        (Architecture::Aarch64, ObjectFormat::Elf)
        | (Architecture::Aarch64, ObjectFormat::MachO)
        | (Architecture::Aarch64, ObjectFormat::Coff) => None,
    }
}

pub fn x86_64_selected_constraint_keys(target: NativeTarget) -> Option<SelectedConstraintKeys> {
    let abi = x86_64_selected_abi(target)?;
    let microsoft = abi == X86_64SelectedAbi::Microsoft;
    let return_i64 = match abi {
        X86_64SelectedAbi::SystemV => X86_64_SYSTEM_V_RETURN,
        X86_64SelectedAbi::Microsoft => X86_64_MICROSOFT_RETURN,
    };
    let return_unit = match abi {
        X86_64SelectedAbi::SystemV => X86_64_SYSTEM_V_RETURN_UNIT,
        X86_64SelectedAbi::Microsoft => X86_64_MICROSOFT_RETURN_UNIT,
    };
    Some({
        let rows = TargetRegisterEnvironmentConstraintKeys {
            port_write: Some(crate::X86_64_PORT_WRITE),
            system_register_read: Some(crate::X86_64_FLAGS_SNAPSHOT),
            system_register_write: Some(crate::X86_64_FLAGS_RESTORE),
            global_descriptor_table: Some(crate::X86_64_GLOBAL_DESCRIPTOR_TABLE),
            task_register_activate: Some(crate::X86_64_TASK_REGISTER_ACTIVATE),
            task_register_read: Some(crate::X86_64_TASK_REGISTER_READ),
            installed_interrupt_cpu_check: Some(crate::X86_64_INSTALLED_INTERRUPT_CPU_CHECK),
            installed_interrupt_task_commit: Some(crate::X86_64_INSTALLED_INTERRUPT_TASK_COMMIT),
            installed_interrupt_table_publication: Some(
                crate::X86_64_INSTALLED_INTERRUPT_TABLE_PUBLICATION,
            ),
            code_segment_reload: Some(crate::X86_64_CODE_SEGMENT_RELOAD),
            data_segment_reload: Some(crate::X86_64_DATA_SEGMENT_RELOAD),
            segment_selector_read: Some(crate::X86_64_SEGMENT_SELECTOR_READ),
            control_register_read: Some(crate::X86_64_CONTROL_REGISTER_READ),
            control_register_write: Some(crate::X86_64_CONTROL_REGISTER_WRITE),
            port_register_read: Some(crate::X86_64_PORT_REGISTER_READ),
            port_register_write: Some(crate::X86_64_PORT_REGISTER_WRITE),
            model_specific_read: Some(crate::X86_64_MODEL_SPECIFIC_READ),
            model_specific_write: Some(crate::X86_64_MODEL_SPECIFIC_WRITE),
            processor_halt: Some(crate::X86_64_PROCESSOR_HALT),
            interrupt_control: Some(crate::X86_64_INTERRUPT_CONTROL),
            memory_fence: Some(crate::X86_64_MEMORY_FENCE),
            atomic_fence: Some(crate::X86_64_ATOMIC_FENCE),
            access_read: Some(crate::X86_64_ACCESSOR_READ),
            access_write: Some(crate::X86_64_ACCESSOR_WRITE),
            atomic_load: Some(crate::X86_64_ATOMIC_LOAD),
            atomic_store: Some(crate::X86_64_ATOMIC_STORE),
            atomic_update: Some(crate::X86_64_ATOMIC_UPDATE),
            atomic_bitwise_update: Some(crate::X86_64_ATOMIC_BITWISE_UPDATE),
            atomic_compare_exchange: Some(crate::X86_64_ATOMIC_COMPARE_EXCHANGE),
            atomic_compare_exchange_once: Some(crate::X86_64_ATOMIC_COMPARE_EXCHANGE_ONCE),
            crash: crate::X86_64_CRASH,
            privileged_crash: Some(crate::X86_64_PRIVILEGED_CRASH),
            load_packed: Some(crate::X86_64_LOAD_PACKED),
            store_packed: Some(crate::X86_64_STORE_PACKED),
            call_aggregate: match abi {
                X86_64SelectedAbi::SystemV => {
                    crate::register_model::x86_64_system_v_aggregate_call_keys()
                        .into_iter()
                        .chain(crate::x86_64_system_v_mixed_aggregate_call_keys())
                        .chain(crate::x86_64_indirect_aggregate_call_keys(false))
                        .collect()
                }
                X86_64SelectedAbi::Microsoft => {
                    crate::register_model::x86_64_microsoft_aggregate_call_keys()
                        .into_iter()
                        .chain(crate::x86_64_microsoft_mixed_aggregate_call_keys())
                        .chain(crate::x86_64_indirect_aggregate_call_keys(true))
                        .collect()
                }
            },
            return_aggregate: match abi {
                X86_64SelectedAbi::SystemV => {
                    crate::register_model::x86_64_system_v_aggregate_return_keys()
                }
                X86_64SelectedAbi::Microsoft => {
                    crate::register_model::x86_64_microsoft_aggregate_return_keys()
                }
            },
            hosted_read_byte: crate::hosted_syscall_target(target)
                .then_some(crate::X86_64_HOSTED_READ_BYTE),
            hosted_exit_process_i32: crate::hosted_syscall_target(target)
                .then_some(crate::X86_64_HOSTED_EXIT_PROCESS_I32),
            hosted_write_byte_i32: crate::hosted_syscall_target(target)
                .then_some(crate::X86_64_HOSTED_WRITE_BYTE_I32),
            load64: Some(crate::X86_64_LOAD64),
            load8: Some(crate::X86_64_LOAD8),
            load16: Some(crate::X86_64_LOAD16),
            load32: Some(crate::X86_64_LOAD32),
            load8_indexed: Some(crate::X86_64_LOAD8_INDEXED),
            scaled_indexed_load: Some(crate::X86_64_SCALED_INDEXED_LOAD),
            packed_add_i16x8: Some(crate::X86_64_PACKED_ADD_I16X8),
            packed_sub_i16x8: Some(crate::X86_64_PACKED_SUB_I16X8),
            packed_shuffle_i16x8: Some(crate::X86_64_PACKED_SHUFFLE_I16X8),
            packed_shuffle_general_i16x8: Some(crate::X86_64_PACKED_SHUFFLE_GENERAL_I16X8),
            packed_shift_right_i16x8: Some(crate::X86_64_PACKED_SHIFT_RIGHT_I16X8),
            packed_widen_u8x8_to_i16x8: Some(crate::X86_64_PACKED_WIDEN_U8X8_TO_I16X8),
            packed_narrow_i16x8_to_u8x8_saturating: Some(
                crate::X86_64_PACKED_NARROW_I16X8_TO_U8X8_SATURATING,
            ),
            packed_load_i16x4_zero_extend: Some(crate::X86_64_PACKED_LOAD_I16X4_ZERO_EXTEND),
            packed_widen_u8x4_to_i16x8: Some(crate::X86_64_PACKED_WIDEN_U8X4_TO_I16X8),
            packed_narrow_i16x8_to_u8x4_saturating: Some(
                crate::X86_64_PACKED_NARROW_I16X8_TO_U8X4_SATURATING,
            ),
            packed_load_128: Some(crate::X86_64_PACKED_LOAD_128),
            packed_store_128: Some(crate::X86_64_PACKED_STORE_128),
            packed_load_64_zero_extend: Some(crate::X86_64_PACKED_LOAD_64_ZERO_EXTEND),
            packed_store_64: Some(crate::X86_64_PACKED_STORE_64),
            packed_load_32_zero_extend: Some(crate::X86_64_PACKED_LOAD_32_ZERO_EXTEND),
            packed_store_32: Some(crate::X86_64_PACKED_STORE_32),
            packed_materialize_zero: Some(crate::X86_64_PACKED_MATERIALIZE_ZERO),
            packed_from_gpr64_zero_extend: Some(crate::X86_64_PACKED_FROM_GPR64_ZERO_EXTEND),
            packed_from_gpr64_pair: Some(crate::X86_64_PACKED_FROM_GPR64_PAIR),
            packed_add_i16x8_reg: Some(crate::X86_64_PACKED_ADD_I16X8_REG),
            packed_sub_i16x8_reg: Some(crate::X86_64_PACKED_SUB_I16X8_REG),
            packed_shift_right_i16x8_reg: Some(crate::X86_64_PACKED_SHIFT_RIGHT_I16X8_REG),
            packed_shuffle_i16x8_reg: Some(crate::X86_64_PACKED_SHUFFLE_I16X8_REG),
            packed_widen_u8x8_to_i16x8_reg: Some(crate::X86_64_PACKED_WIDEN_U8X8_TO_I16X8_REG),
            packed_narrow_i16x8_to_u8x8_saturating_reg: Some(
                crate::X86_64_PACKED_NARROW_I16X8_TO_U8X8_SATURATING_REG,
            ),
            packed_add_u8x16: Some(crate::X86_64_PACKED_ADD_U8X16),
            packed_min_i16x8: Some(crate::X86_64_PACKED_MIN_I16X8),
            packed_cmpgt_i16x8: Some(crate::X86_64_PACKED_CMPGT_I16X8),
            packed_bit_and_i16x8: Some(crate::X86_64_PACKED_BIT_AND_I16X8),
            packed_bit_andnot_i16x8: Some(crate::X86_64_PACKED_BIT_ANDNOT_I16X8),
            packed_abs_i16x8: Some(crate::X86_64_PACKED_ABS_I16X8),
            packed_add_u8x16_reg: Some(crate::X86_64_PACKED_ADD_U8X16_REG),
            packed_min_i16x8_reg: Some(crate::X86_64_PACKED_MIN_I16X8_REG),
            packed_cmpgt_i16x8_reg: Some(crate::X86_64_PACKED_CMPGT_I16X8_REG),
            packed_bit_and_i16x8_reg: Some(crate::X86_64_PACKED_BIT_AND_I16X8_REG),
            packed_bit_andnot_i16x8_reg: Some(crate::X86_64_PACKED_BIT_ANDNOT_I16X8_REG),
            packed_abs_i16x8_reg: Some(crate::X86_64_PACKED_ABS_I16X8_REG),
            copy_bytes: Some(crate::X86_64_COPY_BYTES),
            zero_bytes: Some(crate::X86_64_ZERO_BYTES),
            scan_byte_sequence: Some(crate::X86_64_SCAN_BYTE_SEQUENCE),
            load_encoded_scalar: Some(crate::X86_64_LOAD_ENCODED_SCALAR),
            store_encoded_scalar: Some(crate::X86_64_STORE_ENCODED_SCALAR),
            compare_byte_sequences: Some(crate::X86_64_COMPARE_BYTE_SEQUENCES),
            save_floating_control: Some(crate::X86_64_SAVE_FLOATING_CONTROL),
            restore_floating_control: Some(crate::X86_64_RESTORE_FLOATING_CONTROL),
            store: Some(crate::X86_64_STORE),
            address_offset: Some(crate::X86_64_ADDRESS_OFFSET),
            store64: Some(crate::X86_64_STORE64),
            frame_address: Some(crate::X86_64_FRAME_ADDRESS),
            byte_literal_image_address: Some(crate::X86_64_BYTE_LITERAL_IMAGE_ADDRESS),
            call_unit_mixed: match abi {
                X86_64SelectedAbi::SystemV => crate::x86_64_system_v_mixed_unit_call_keys(),
                X86_64SelectedAbi::Microsoft => crate::x86_64_microsoft_mixed_unit_call_keys(),
            },
            call_unit: match abi {
                X86_64SelectedAbi::SystemV => crate::x86_64_system_v_register_unit_call_keys(),
                X86_64SelectedAbi::Microsoft => crate::x86_64_microsoft_register_unit_call_keys(),
            },
            call_scalar: (match abi {
                X86_64SelectedAbi::SystemV => x86_64_system_v_register_call_keys(),
                X86_64SelectedAbi::Microsoft => crate::x86_64_microsoft_register_call_keys(),
            })
            .into_iter()
            .chain(crate::x86_64_float_scalar_call_keys(microsoft))
            .collect(),
            call_normalized_foreign: (match abi {
                X86_64SelectedAbi::SystemV => crate::x86_64_system_v_normalized_foreign_call_keys(),
                X86_64SelectedAbi::Microsoft => {
                    crate::x86_64_microsoft_normalized_foreign_call_keys()
                }
            })
            .into_iter()
            .chain(
                (target.object_format == ObjectFormat::Elf)
                    .then(crate::x86_64_linux_system_call_keys)
                    .into_iter()
                    .flatten(),
            )
            .collect(),
            materialize_i64: X86_64_MATERIALIZE_I64,
            materialize_boolean: crate::X86_64_MATERIALIZE_BOOLEAN,
            copy_i64: X86_64_COPY_I64,
            float32_to_bits: Some(crate::X86_64_FLOAT32_TO_BITS),
            float64_to_bits: Some(crate::X86_64_FLOAT64_TO_BITS),
            bits_to_float32: Some(crate::X86_64_BITS_TO_FLOAT32),
            bits_to_float64: Some(crate::X86_64_BITS_TO_FLOAT64),
            ieee_float_arithmetic: Some(crate::X86_64_IEEE_FLOAT_ARITHMETIC),
            ieee_float_directed_binary: Some(crate::X86_64_IEEE_FLOAT_DIRECTED_BINARY),
            ieee_float_directed_ternary: None,
            ieee_float_directed_unary: Some(crate::X86_64_IEEE_FLOAT_DIRECTED_UNARY),
            ieee_float_unary: Some(crate::X86_64_IEEE_FLOAT_UNARY),
            ieee_float_conversion_control: Some(crate::X86_64_IEEE_FLOAT_CONVERSION_CONTROL),
            ieee_float_ternary: Some(crate::X86_64_IEEE_FLOAT_TERNARY),
            float_result_trapping: Some(crate::X86_64_FLOAT_RESULT_TRAPPING),
            float_result_saturating: Some(crate::X86_64_FLOAT_RESULT_SATURATING),
            add_i64: X86_64_ADD_I64,
            subtract_i64: X86_64_SUBTRACT_I64,
            multiply_i64: crate::X86_64_MULTIPLY_I64,
            saturating_subtract_unsigned:
                crate::register_model::X86_64_SATURATING_SUBTRACT_UNSIGNED,
            saturating_add_u64: crate::register_model::X86_64_SATURATING_ADD_U64,
            divide_u64: crate::register_model::X86_64_DIVIDE_U64,
            remainder_u64: crate::register_model::X86_64_REMAINDER_U64,
            remainder_i64: crate::register_model::X86_64_REMAINDER_I64,
            divide_i64: crate::register_model::X86_64_DIVIDE_I64,
            shift_i64: crate::register_model::X86_64_SHIFT_I64,
            saturating_add_clamped: crate::register_model::X86_64_SATURATING_ADD_CLAMPED,
            saturating_subtract_clamped: crate::register_model::X86_64_SATURATING_SUBTRACT_CLAMPED,
            saturating_divide_signed: crate::register_model::X86_64_SATURATING_DIVIDE_SIGNED,
            saturating_multiply_clamped: crate::register_model::X86_64_SATURATING_MULTIPLY_CLAMPED,
            saturating_multiply_u64: crate::register_model::X86_64_SATURATING_MULTIPLY_U64,
            trapping_binary: crate::X86_64_TRAPPING_BINARY,
            trapping_fixed_pair: crate::X86_64_TRAPPING_FIXED_PAIR,
            trapping_shift: crate::X86_64_TRAPPING_SHIFT,
            trapping_convert: crate::X86_64_TRAPPING_CONVERT,
            add_i64_immediate: X86_64_ADD_I64_IMMEDIATE,
            subtract_i64_immediate: X86_64_SUBTRACT_I64_IMMEDIATE,
            compare_i64_zero: X86_64_COMPARE_I64_ZERO,
            compare_i64: X86_64_COMPARE_I64,
            compare_i64_immediate: X86_64_COMPARE_I64_IMMEDIATE,
            conditional_branch: X86_64_CONDITIONAL_BRANCH,
            jump: crate::X86_64_JUMP,
            return_float: crate::x86_64_float_scalar_return_keys(microsoft),
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
