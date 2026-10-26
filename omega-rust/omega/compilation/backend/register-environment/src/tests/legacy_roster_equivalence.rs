//! Temporary equivalence harness: the roster copy this change deletes against
//! the ISA-owner roster that replaces it. Deleted before the commit.
#![allow(unused_imports, dead_code, clippy::all)]

use isa_aarch64::{
    AARCH64_AAPCS64_CALL, AARCH64_AAPCS64_RETURN, AARCH64_AAPCS64_RETURN_UNIT, AARCH64_ADD_I64,
    AARCH64_ADD_I64_IMMEDIATE, AARCH64_COMPARE_I64_ZERO, AARCH64_CONDITIONAL_BRANCH,
    AARCH64_COPY_I64, AARCH64_DARWIN_CALL, AARCH64_DARWIN_RETURN, AARCH64_DARWIN_RETURN_UNIT,
    AARCH64_MATERIALIZE_I64, AARCH64_SUBTRACT_I64, AARCH64_SUBTRACT_I64_IMMEDIATE,
    aarch64_physical_register_model, aarch64_register_constraint_catalog,
};
use isa_x86_64::{
    X86_64_ADD_I64, X86_64_ADD_I64_IMMEDIATE, X86_64_COMPARE_I64_ZERO, X86_64_CONDITIONAL_BRANCH,
    X86_64_COPY_I64, X86_64_MATERIALIZE_I64, X86_64_MICROSOFT_CALL, X86_64_MICROSOFT_RETURN,
    X86_64_MICROSOFT_RETURN_UNIT, X86_64_SUBTRACT_I64, X86_64_SUBTRACT_I64_IMMEDIATE,
    X86_64_SYSTEM_V_CALL, X86_64_SYSTEM_V_RETURN, X86_64_SYSTEM_V_RETURN_UNIT,
    x86_64_physical_register_model, x86_64_register_constraint_catalog,
};
use selected_instructions::SelectedConstraintKeys;
use target::{Architecture, NativeTarget, ObjectFormat};

/// The x86-64 targets whose hosted byte and exit leaves enter the kernel
/// directly: Linux and Intel macOS. Both use `syscall` with the same argument
/// and clobber registers, so they share one constraint row per leaf.
fn x86_64_hosted_syscall_target(target: NativeTarget) -> bool {
    target == NativeTarget::linux_x64() || target == NativeTarget::macos_x64()
}

pub(super) fn legacy_selected_constraint_keys(
    target: NativeTarget,
) -> Option<SelectedConstraintKeys> {
    match (target.architecture, target.object_format) {
        // Intel macOS shares the System V AMD64 call roster with Linux; only
        // the hosted syscall encodings differ, and those select by exact target.
        (Architecture::X86_64, ObjectFormat::Elf | ObjectFormat::MachO) => {
            Some(SelectedConstraintKeys {
                port_write: Some(isa_x86_64::X86_64_PORT_WRITE),
                system_register_read: Some(isa_x86_64::X86_64_FLAGS_SNAPSHOT),
                system_register_write: Some(isa_x86_64::X86_64_FLAGS_RESTORE),
                control_register_read: Some(isa_x86_64::X86_64_CONTROL_REGISTER_READ),
                control_register_write: Some(isa_x86_64::X86_64_CONTROL_REGISTER_WRITE),
                port_register_read: Some(isa_x86_64::X86_64_PORT_REGISTER_READ),
                port_register_write: Some(isa_x86_64::X86_64_PORT_REGISTER_WRITE),
                model_specific_read: Some(isa_x86_64::X86_64_MODEL_SPECIFIC_READ),
                model_specific_write: Some(isa_x86_64::X86_64_MODEL_SPECIFIC_WRITE),
                processor_halt: Some(isa_x86_64::X86_64_PROCESSOR_HALT),
                interrupt_control: Some(isa_x86_64::X86_64_INTERRUPT_CONTROL),
                memory_fence: Some(isa_x86_64::X86_64_MEMORY_FENCE),
                atomic_fence: Some(isa_x86_64::X86_64_ATOMIC_FENCE),
                placed_read: Some(isa_x86_64::X86_64_PLACED_READ),
                placed_write: Some(isa_x86_64::X86_64_PLACED_WRITE),
                atomic_load: Some(isa_x86_64::X86_64_ATOMIC_LOAD),
                atomic_store: Some(isa_x86_64::X86_64_ATOMIC_STORE),
                atomic_update: Some(isa_x86_64::X86_64_ATOMIC_UPDATE),
                atomic_bitwise_update: Some(isa_x86_64::X86_64_ATOMIC_BITWISE_UPDATE),
                atomic_compare_exchange: Some(isa_x86_64::X86_64_ATOMIC_COMPARE_EXCHANGE),
                crash: isa_x86_64::X86_64_CRASH,
                call_aggregate: isa_x86_64::x86_64_system_v_aggregate_call_keys()
                    .into_iter()
                    .chain(isa_x86_64::x86_64_system_v_mixed_aggregate_call_keys())
                    .chain(isa_x86_64::x86_64_indirect_aggregate_call_keys(false))
                    .collect(),
                return_aggregate: isa_x86_64::x86_64_system_v_aggregate_return_keys(),
                hosted_read_byte: x86_64_hosted_syscall_target(target)
                    .then_some(isa_x86_64::X86_64_HOSTED_READ_BYTE),
                hosted_write_byte_i32: x86_64_hosted_syscall_target(target)
                    .then_some(isa_x86_64::X86_64_HOSTED_WRITE_BYTE_I32),
                hosted_exit_process_i32: x86_64_hosted_syscall_target(target)
                    .then_some(isa_x86_64::X86_64_HOSTED_EXIT_PROCESS_I32),
                load64: Some(isa_x86_64::X86_64_LOAD64),
                load8: Some(isa_x86_64::X86_64_LOAD8),
                load16: Some(isa_x86_64::X86_64_LOAD16),
                load32: Some(isa_x86_64::X86_64_LOAD32),
                load_packed: Some(isa_x86_64::X86_64_LOAD_PACKED),
                store_packed: Some(isa_x86_64::X86_64_STORE_PACKED),
                load8_indexed: Some(isa_x86_64::X86_64_LOAD8_INDEXED),
                copy_bytes: Some(isa_x86_64::X86_64_COPY_BYTES),
                zero_bytes: Some(isa_x86_64::X86_64_ZERO_BYTES),
                scan_byte_sequence: Some(isa_x86_64::X86_64_SCAN_BYTE_SEQUENCE),
                load_encoded_scalar: Some(isa_x86_64::X86_64_LOAD_ENCODED_SCALAR),
                store_encoded_scalar: Some(isa_x86_64::X86_64_STORE_ENCODED_SCALAR),
                compare_byte_sequences: Some(isa_x86_64::X86_64_COMPARE_BYTE_SEQUENCES),
                save_floating_control: Some(isa_x86_64::X86_64_SAVE_FLOATING_CONTROL),
                restore_floating_control: Some(isa_x86_64::X86_64_RESTORE_FLOATING_CONTROL),
                store: Some(isa_x86_64::X86_64_STORE),
                address_offset: Some(isa_x86_64::X86_64_ADDRESS_OFFSET),
                store64: Some(isa_x86_64::X86_64_STORE64),
                frame_address: Some(isa_x86_64::X86_64_FRAME_ADDRESS),
                byte_literal_image_address: Some(isa_x86_64::X86_64_BYTE_LITERAL_IMAGE_ADDRESS),
                call_unit: isa_x86_64::x86_64_system_v_register_unit_call_keys(),
                call_unit_mixed: isa_x86_64::x86_64_system_v_mixed_unit_call_keys(),
                call_scalar: isa_x86_64::x86_64_system_v_register_call_keys()
                    .into_iter()
                    .chain(isa_x86_64::x86_64_float_scalar_call_keys(false))
                    .collect(),
                call_normalized_foreign: isa_x86_64::x86_64_system_v_normalized_foreign_call_keys()
                    .into_iter()
                    .chain(
                        (target.object_format == ObjectFormat::Elf)
                            .then(isa_x86_64::x86_64_linux_system_call_keys)
                            .into_iter()
                            .flatten(),
                    )
                    .collect(),
                materialize_i64: X86_64_MATERIALIZE_I64,
                materialize_boolean: isa_x86_64::X86_64_MATERIALIZE_BOOLEAN,
                copy_i64: X86_64_COPY_I64,
                float32_to_bits: Some(isa_x86_64::X86_64_FLOAT32_TO_BITS),
                float64_to_bits: Some(isa_x86_64::X86_64_FLOAT64_TO_BITS),
                bits_to_float32: Some(isa_x86_64::X86_64_BITS_TO_FLOAT32),
                bits_to_float64: Some(isa_x86_64::X86_64_BITS_TO_FLOAT64),
                ieee_float_arithmetic: Some(isa_x86_64::X86_64_IEEE_FLOAT_ARITHMETIC),
                ieee_float_directed_binary: Some(isa_x86_64::X86_64_IEEE_FLOAT_DIRECTED_BINARY),
                ieee_float_directed_unary: Some(isa_x86_64::X86_64_IEEE_FLOAT_DIRECTED_UNARY),
                ieee_float_unary: Some(isa_x86_64::X86_64_IEEE_FLOAT_UNARY),
                ieee_float_conversion_control: Some(
                    isa_x86_64::X86_64_IEEE_FLOAT_CONVERSION_CONTROL,
                ),
                ieee_float_ternary: Some(isa_x86_64::X86_64_IEEE_FLOAT_TERNARY),
                add_i64: X86_64_ADD_I64,
                add_i64_immediate: X86_64_ADD_I64_IMMEDIATE,
                subtract_i64: X86_64_SUBTRACT_I64,
                multiply_i64: isa_x86_64::X86_64_MULTIPLY_I64,
                saturating_subtract_unsigned: isa_x86_64::X86_64_SATURATING_SUBTRACT_UNSIGNED,
                saturating_add_u64: isa_x86_64::X86_64_SATURATING_ADD_U64,
                divide_u64: isa_x86_64::X86_64_DIVIDE_U64,
                remainder_u64: isa_x86_64::X86_64_REMAINDER_U64,
                remainder_i64: isa_x86_64::X86_64_REMAINDER_I64,
                divide_i64: isa_x86_64::X86_64_DIVIDE_I64,
                shift_i64: isa_x86_64::X86_64_SHIFT_I64,
                saturating_add_clamped: isa_x86_64::X86_64_SATURATING_ADD_CLAMPED,
                saturating_subtract_clamped: isa_x86_64::X86_64_SATURATING_SUBTRACT_CLAMPED,
                saturating_divide_signed: isa_x86_64::X86_64_SATURATING_DIVIDE_SIGNED,
                saturating_multiply_clamped: isa_x86_64::X86_64_SATURATING_MULTIPLY_CLAMPED,
                saturating_multiply_u64: isa_x86_64::X86_64_SATURATING_MULTIPLY_U64,
                trapping_binary: isa_x86_64::X86_64_TRAPPING_BINARY,
                trapping_fixed_pair: isa_x86_64::X86_64_TRAPPING_FIXED_PAIR,
                trapping_shift: isa_x86_64::X86_64_TRAPPING_SHIFT,
                trapping_convert: isa_x86_64::X86_64_TRAPPING_CONVERT,
                float_result_trapping: Some(isa_x86_64::X86_64_FLOAT_RESULT_TRAPPING),
                float_result_saturating: Some(isa_x86_64::X86_64_FLOAT_RESULT_SATURATING),
                subtract_i64_immediate: X86_64_SUBTRACT_I64_IMMEDIATE,
                compare_i64_zero: X86_64_COMPARE_I64_ZERO,
                compare_i64: isa_x86_64::X86_64_COMPARE_I64,
                compare_i64_immediate: isa_x86_64::X86_64_COMPARE_I64_IMMEDIATE,
                conditional_branch: X86_64_CONDITIONAL_BRANCH,
                jump: isa_x86_64::X86_64_JUMP,
                return_i64: X86_64_SYSTEM_V_RETURN,
                return_float: isa_x86_64::x86_64_float_scalar_return_keys(false),
                return_unit: X86_64_SYSTEM_V_RETURN_UNIT,
            })
        }
        (Architecture::X86_64, ObjectFormat::Coff) => Some(SelectedConstraintKeys {
            port_write: Some(isa_x86_64::X86_64_PORT_WRITE),
            system_register_read: Some(isa_x86_64::X86_64_FLAGS_SNAPSHOT),
            system_register_write: Some(isa_x86_64::X86_64_FLAGS_RESTORE),
            control_register_read: Some(isa_x86_64::X86_64_CONTROL_REGISTER_READ),
            control_register_write: Some(isa_x86_64::X86_64_CONTROL_REGISTER_WRITE),
            // These ISA declarations do not grant execution authority. Omitting
            // them here disagrees with the effect catalog even for ordinary calls.
            port_register_read: Some(isa_x86_64::X86_64_PORT_REGISTER_READ),
            port_register_write: Some(isa_x86_64::X86_64_PORT_REGISTER_WRITE),
            model_specific_read: Some(isa_x86_64::X86_64_MODEL_SPECIFIC_READ),
            model_specific_write: Some(isa_x86_64::X86_64_MODEL_SPECIFIC_WRITE),
            processor_halt: Some(isa_x86_64::X86_64_PROCESSOR_HALT),
            interrupt_control: Some(isa_x86_64::X86_64_INTERRUPT_CONTROL),
            memory_fence: Some(isa_x86_64::X86_64_MEMORY_FENCE),
            atomic_fence: Some(isa_x86_64::X86_64_ATOMIC_FENCE),
            placed_read: Some(isa_x86_64::X86_64_PLACED_READ),
            placed_write: Some(isa_x86_64::X86_64_PLACED_WRITE),
            atomic_load: Some(isa_x86_64::X86_64_ATOMIC_LOAD),
            atomic_store: Some(isa_x86_64::X86_64_ATOMIC_STORE),
            atomic_update: Some(isa_x86_64::X86_64_ATOMIC_UPDATE),
            atomic_bitwise_update: Some(isa_x86_64::X86_64_ATOMIC_BITWISE_UPDATE),
            atomic_compare_exchange: Some(isa_x86_64::X86_64_ATOMIC_COMPARE_EXCHANGE),
            crash: isa_x86_64::X86_64_CRASH,
            call_aggregate: isa_x86_64::x86_64_microsoft_aggregate_call_keys()
                .into_iter()
                .chain(isa_x86_64::x86_64_microsoft_mixed_aggregate_call_keys())
                .chain(isa_x86_64::x86_64_indirect_aggregate_call_keys(true))
                .collect(),
            return_aggregate: isa_x86_64::x86_64_microsoft_aggregate_return_keys(),
            hosted_read_byte: None,
            hosted_write_byte_i32: None,
            hosted_exit_process_i32: None,
            load64: Some(isa_x86_64::X86_64_LOAD64),
            load8: Some(isa_x86_64::X86_64_LOAD8),
            load16: Some(isa_x86_64::X86_64_LOAD16),
            load32: Some(isa_x86_64::X86_64_LOAD32),
            load_packed: Some(isa_x86_64::X86_64_LOAD_PACKED),
            store_packed: Some(isa_x86_64::X86_64_STORE_PACKED),
            load8_indexed: Some(isa_x86_64::X86_64_LOAD8_INDEXED),
            copy_bytes: Some(isa_x86_64::X86_64_COPY_BYTES),
            zero_bytes: Some(isa_x86_64::X86_64_ZERO_BYTES),
            scan_byte_sequence: Some(isa_x86_64::X86_64_SCAN_BYTE_SEQUENCE),
            load_encoded_scalar: Some(isa_x86_64::X86_64_LOAD_ENCODED_SCALAR),
            store_encoded_scalar: Some(isa_x86_64::X86_64_STORE_ENCODED_SCALAR),
            compare_byte_sequences: Some(isa_x86_64::X86_64_COMPARE_BYTE_SEQUENCES),
            save_floating_control: Some(isa_x86_64::X86_64_SAVE_FLOATING_CONTROL),
            restore_floating_control: Some(isa_x86_64::X86_64_RESTORE_FLOATING_CONTROL),
            store: Some(isa_x86_64::X86_64_STORE),
            address_offset: Some(isa_x86_64::X86_64_ADDRESS_OFFSET),
            store64: Some(isa_x86_64::X86_64_STORE64),
            frame_address: Some(isa_x86_64::X86_64_FRAME_ADDRESS),
            byte_literal_image_address: Some(isa_x86_64::X86_64_BYTE_LITERAL_IMAGE_ADDRESS),
            call_unit: isa_x86_64::x86_64_microsoft_register_unit_call_keys(),
            call_unit_mixed: isa_x86_64::x86_64_microsoft_mixed_unit_call_keys(),
            call_scalar: isa_x86_64::x86_64_microsoft_register_call_keys()
                .into_iter()
                .chain(isa_x86_64::x86_64_float_scalar_call_keys(true))
                .collect(),
            call_normalized_foreign: isa_x86_64::x86_64_microsoft_normalized_foreign_call_keys(),
            materialize_i64: X86_64_MATERIALIZE_I64,
            materialize_boolean: isa_x86_64::X86_64_MATERIALIZE_BOOLEAN,
            copy_i64: X86_64_COPY_I64,
            float32_to_bits: Some(isa_x86_64::X86_64_FLOAT32_TO_BITS),
            float64_to_bits: Some(isa_x86_64::X86_64_FLOAT64_TO_BITS),
            bits_to_float32: Some(isa_x86_64::X86_64_BITS_TO_FLOAT32),
            bits_to_float64: Some(isa_x86_64::X86_64_BITS_TO_FLOAT64),
            ieee_float_arithmetic: Some(isa_x86_64::X86_64_IEEE_FLOAT_ARITHMETIC),
            ieee_float_directed_binary: Some(isa_x86_64::X86_64_IEEE_FLOAT_DIRECTED_BINARY),
            ieee_float_directed_unary: Some(isa_x86_64::X86_64_IEEE_FLOAT_DIRECTED_UNARY),
            ieee_float_unary: Some(isa_x86_64::X86_64_IEEE_FLOAT_UNARY),
            ieee_float_conversion_control: Some(isa_x86_64::X86_64_IEEE_FLOAT_CONVERSION_CONTROL),
            ieee_float_ternary: Some(isa_x86_64::X86_64_IEEE_FLOAT_TERNARY),
            add_i64: X86_64_ADD_I64,
            add_i64_immediate: X86_64_ADD_I64_IMMEDIATE,
            subtract_i64: X86_64_SUBTRACT_I64,
            multiply_i64: isa_x86_64::X86_64_MULTIPLY_I64,
            saturating_subtract_unsigned: isa_x86_64::X86_64_SATURATING_SUBTRACT_UNSIGNED,
            saturating_add_u64: isa_x86_64::X86_64_SATURATING_ADD_U64,
            divide_u64: isa_x86_64::X86_64_DIVIDE_U64,
            remainder_u64: isa_x86_64::X86_64_REMAINDER_U64,
            remainder_i64: isa_x86_64::X86_64_REMAINDER_I64,
            divide_i64: isa_x86_64::X86_64_DIVIDE_I64,
            shift_i64: isa_x86_64::X86_64_SHIFT_I64,
            saturating_add_clamped: isa_x86_64::X86_64_SATURATING_ADD_CLAMPED,
            saturating_subtract_clamped: isa_x86_64::X86_64_SATURATING_SUBTRACT_CLAMPED,
            saturating_divide_signed: isa_x86_64::X86_64_SATURATING_DIVIDE_SIGNED,
            saturating_multiply_clamped: isa_x86_64::X86_64_SATURATING_MULTIPLY_CLAMPED,
            saturating_multiply_u64: isa_x86_64::X86_64_SATURATING_MULTIPLY_U64,
            trapping_binary: isa_x86_64::X86_64_TRAPPING_BINARY,
            trapping_fixed_pair: isa_x86_64::X86_64_TRAPPING_FIXED_PAIR,
            trapping_shift: isa_x86_64::X86_64_TRAPPING_SHIFT,
            trapping_convert: isa_x86_64::X86_64_TRAPPING_CONVERT,
            float_result_trapping: Some(isa_x86_64::X86_64_FLOAT_RESULT_TRAPPING),
            float_result_saturating: Some(isa_x86_64::X86_64_FLOAT_RESULT_SATURATING),
            subtract_i64_immediate: X86_64_SUBTRACT_I64_IMMEDIATE,
            compare_i64_zero: X86_64_COMPARE_I64_ZERO,
            compare_i64: isa_x86_64::X86_64_COMPARE_I64,
            compare_i64_immediate: isa_x86_64::X86_64_COMPARE_I64_IMMEDIATE,
            conditional_branch: X86_64_CONDITIONAL_BRANCH,
            jump: isa_x86_64::X86_64_JUMP,
            return_i64: X86_64_MICROSOFT_RETURN,
            return_float: isa_x86_64::x86_64_float_scalar_return_keys(true),
            return_unit: X86_64_MICROSOFT_RETURN_UNIT,
        }),
        (Architecture::Aarch64, ObjectFormat::Elf | ObjectFormat::MachO) => {
            Some(legacy_aarch64_selected_constraint_keys(target))
        }
        _ => None,
    }
}

// The caller admits only AArch64 ELF and Mach-O; ABI and exact hosted-target
// gates remain explicit while the common ISA keys are declared once.
fn legacy_aarch64_selected_constraint_keys(target: NativeTarget) -> SelectedConstraintKeys {
    let darwin = target.object_format == ObjectFormat::MachO;
    let hosted = target
        == if darwin {
            NativeTarget::macos_arm64()
        } else {
            NativeTarget::linux_arm64()
        };
    SelectedConstraintKeys {
        port_write: None,
        system_register_read: Some(isa_aarch64::AARCH64_SYSTEM_REGISTER_READ),
        system_register_write: Some(isa_aarch64::AARCH64_SYSTEM_REGISTER_WRITE),
        control_register_read: None,
        control_register_write: None,
        port_register_read: None,
        port_register_write: None,
        model_specific_read: None,
        model_specific_write: None,
        processor_halt: None,
        interrupt_control: None,
        memory_fence: Some(isa_aarch64::AARCH64_MEMORY_FENCE),
        atomic_fence: Some(isa_aarch64::AARCH64_ATOMIC_FENCE),
        placed_read: Some(isa_aarch64::AARCH64_PLACED_READ),
        placed_write: Some(isa_aarch64::AARCH64_PLACED_WRITE),
        atomic_load: (target == NativeTarget::macos_arm64())
            .then_some(isa_aarch64::AARCH64_ATOMIC_LOAD),
        atomic_store: (target == NativeTarget::macos_arm64())
            .then_some(isa_aarch64::AARCH64_ATOMIC_STORE),
        atomic_update: (target == NativeTarget::macos_arm64())
            .then_some(isa_aarch64::AARCH64_ATOMIC_UPDATE),
        atomic_bitwise_update: (target == NativeTarget::macos_arm64())
            .then_some(isa_aarch64::AARCH64_ATOMIC_UPDATE),
        atomic_compare_exchange: (target == NativeTarget::macos_arm64())
            .then_some(isa_aarch64::AARCH64_ATOMIC_COMPARE_EXCHANGE),
        crash: isa_aarch64::AARCH64_CRASH,
        call_aggregate: isa_aarch64::aarch64_register_aggregate_call_keys(darwin)
            .into_iter()
            .chain(isa_aarch64::aarch64_mixed_aggregate_call_keys(darwin))
            .chain(isa_aarch64::aarch64_indirect_aggregate_call_keys(darwin))
            .collect(),
        return_aggregate: isa_aarch64::aarch64_register_aggregate_return_keys(darwin),
        hosted_read_byte: hosted.then_some(if darwin {
            isa_aarch64::AARCH64_DARWIN_HOSTED_READ_BYTE
        } else {
            isa_aarch64::AARCH64_HOSTED_READ_BYTE
        }),
        hosted_write_byte_i32: hosted.then_some(if darwin {
            isa_aarch64::AARCH64_DARWIN_HOSTED_WRITE_BYTE_I32
        } else {
            isa_aarch64::AARCH64_HOSTED_WRITE_BYTE_I32
        }),
        hosted_exit_process_i32: hosted.then_some(if darwin {
            isa_aarch64::AARCH64_DARWIN_HOSTED_EXIT_PROCESS_I32
        } else {
            isa_aarch64::AARCH64_HOSTED_EXIT_PROCESS_I32
        }),
        load64: Some(isa_aarch64::AARCH64_LOAD64),
        load8: Some(isa_aarch64::AARCH64_LOAD8),
        load16: Some(isa_aarch64::AARCH64_LOAD16),
        load32: Some(isa_aarch64::AARCH64_LOAD32),
        load_packed: Some(isa_aarch64::AARCH64_LOAD_PACKED),
        store_packed: Some(isa_aarch64::AARCH64_STORE_PACKED),
        load8_indexed: Some(isa_aarch64::AARCH64_LOAD8_INDEXED),
        copy_bytes: Some(isa_aarch64::AARCH64_COPY_BYTES),
        zero_bytes: Some(isa_aarch64::AARCH64_ZERO_BYTES),
        scan_byte_sequence: Some(isa_aarch64::AARCH64_SCAN_BYTE_SEQUENCE),
        load_encoded_scalar: Some(isa_aarch64::AARCH64_LOAD_ENCODED_SCALAR),
        store_encoded_scalar: Some(isa_aarch64::AARCH64_STORE_ENCODED_SCALAR),
        compare_byte_sequences: Some(isa_aarch64::AARCH64_COMPARE_BYTE_SEQUENCES),
        save_floating_control: Some(isa_aarch64::AARCH64_SAVE_FLOATING_CONTROL),
        restore_floating_control: Some(isa_aarch64::AARCH64_RESTORE_FLOATING_CONTROL),
        store: Some(isa_aarch64::AARCH64_STORE),
        address_offset: Some(isa_aarch64::AARCH64_ADDRESS_OFFSET),
        store64: Some(isa_aarch64::AARCH64_STORE64),
        frame_address: Some(isa_aarch64::AARCH64_FRAME_ADDRESS),
        byte_literal_image_address: Some(isa_aarch64::AARCH64_BYTE_LITERAL_IMAGE_ADDRESS),
        call_unit: if darwin {
            isa_aarch64::aarch64_darwin_register_unit_call_keys()
        } else {
            isa_aarch64::aarch64_aapcs64_register_unit_call_keys()
        },
        call_unit_mixed: if darwin {
            isa_aarch64::aarch64_darwin_mixed_unit_call_keys()
        } else {
            isa_aarch64::aarch64_aapcs64_mixed_unit_call_keys()
        },
        call_scalar: if darwin {
            isa_aarch64::aarch64_darwin_register_call_keys()
        } else {
            isa_aarch64::aarch64_aapcs64_register_call_keys()
        }
        .into_iter()
        .chain(isa_aarch64::aarch64_float_scalar_call_keys(darwin))
        .collect(),
        call_normalized_foreign: if darwin {
            isa_aarch64::aarch64_darwin_normalized_foreign_call_keys()
        } else {
            isa_aarch64::aarch64_aapcs64_normalized_foreign_call_keys()
                .into_iter()
                .chain(isa_aarch64::aarch64_linux_system_call_keys())
                .collect()
        },
        materialize_i64: AARCH64_MATERIALIZE_I64,
        materialize_boolean: isa_aarch64::AARCH64_MATERIALIZE_BOOLEAN,
        copy_i64: AARCH64_COPY_I64,
        float32_to_bits: Some(isa_aarch64::AARCH64_FLOAT32_TO_BITS),
        float64_to_bits: Some(isa_aarch64::AARCH64_FLOAT64_TO_BITS),
        bits_to_float32: Some(isa_aarch64::AARCH64_BITS_TO_FLOAT32),
        bits_to_float64: Some(isa_aarch64::AARCH64_BITS_TO_FLOAT64),
        ieee_float_arithmetic: Some(isa_aarch64::AARCH64_IEEE_FLOAT_ARITHMETIC),
        ieee_float_directed_binary: Some(isa_aarch64::AARCH64_IEEE_FLOAT_ARITHMETIC),
        ieee_float_directed_unary: Some(isa_aarch64::AARCH64_IEEE_FLOAT_UNARY),
        ieee_float_unary: Some(isa_aarch64::AARCH64_IEEE_FLOAT_UNARY),
        ieee_float_conversion_control: Some(isa_aarch64::AARCH64_IEEE_FLOAT_UNARY),
        ieee_float_ternary: Some(isa_aarch64::AARCH64_IEEE_FLOAT_TERNARY),
        add_i64: AARCH64_ADD_I64,
        add_i64_immediate: AARCH64_ADD_I64_IMMEDIATE,
        subtract_i64: AARCH64_SUBTRACT_I64,
        multiply_i64: isa_aarch64::AARCH64_MULTIPLY_I64,
        saturating_subtract_unsigned: isa_aarch64::AARCH64_SATURATING_SUBTRACT_UNSIGNED,
        saturating_add_u64: isa_aarch64::AARCH64_SATURATING_ADD_U64,
        divide_u64: isa_aarch64::AARCH64_DIVIDE_U64,
        remainder_u64: isa_aarch64::AARCH64_REMAINDER_U64,
        remainder_i64: isa_aarch64::AARCH64_REMAINDER_I64,
        divide_i64: isa_aarch64::AARCH64_DIVIDE_I64,
        shift_i64: isa_aarch64::AARCH64_SHIFT_I64,
        saturating_add_clamped: isa_aarch64::AARCH64_SATURATING_ADD_CLAMPED,
        saturating_subtract_clamped: isa_aarch64::AARCH64_SATURATING_SUBTRACT_CLAMPED,
        saturating_divide_signed: isa_aarch64::AARCH64_SATURATING_DIVIDE_SIGNED,
        saturating_multiply_clamped: isa_aarch64::AARCH64_SATURATING_MULTIPLY_CLAMPED,
        saturating_multiply_u64: isa_aarch64::AARCH64_SATURATING_MULTIPLY_U64,
        trapping_binary: isa_aarch64::AARCH64_TRAPPING_BINARY,
        trapping_fixed_pair: isa_aarch64::AARCH64_TRAPPING_FIXED_PAIR,
        trapping_shift: isa_aarch64::AARCH64_TRAPPING_SHIFT,
        trapping_convert: isa_aarch64::AARCH64_TRAPPING_CONVERT,
        float_result_trapping: Some(isa_aarch64::AARCH64_FLOAT_RESULT_TRAPPING),
        float_result_saturating: Some(isa_aarch64::AARCH64_FLOAT_RESULT_SATURATING),
        subtract_i64_immediate: AARCH64_SUBTRACT_I64_IMMEDIATE,
        compare_i64_zero: AARCH64_COMPARE_I64_ZERO,
        compare_i64: isa_aarch64::AARCH64_COMPARE_I64,
        compare_i64_immediate: isa_aarch64::AARCH64_COMPARE_I64_IMMEDIATE,
        conditional_branch: AARCH64_CONDITIONAL_BRANCH,
        jump: isa_aarch64::AARCH64_JUMP,
        return_i64: if darwin {
            AARCH64_DARWIN_RETURN
        } else {
            AARCH64_AAPCS64_RETURN
        },
        return_float: isa_aarch64::aarch64_float_scalar_return_keys(darwin),
        return_unit: if darwin {
            AARCH64_DARWIN_RETURN_UNIT
        } else {
            AARCH64_AAPCS64_RETURN_UNIT
        },
    }
}

/// The roster every declared pair resolved to before the consolidation, versus
/// the roster its ISA owner now declares. One mismatch means this change moved
/// a constraint row.
#[test]
fn deleted_roster_copy_equals_its_isa_owner_for_every_declared_pair() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::macos_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let deleted = legacy_selected_constraint_keys(target);
        let resolved = match target.architecture {
            Architecture::X86_64 => isa_x86_64::x86_64_selected_constraint_keys(target),
            Architecture::Aarch64 => isa_aarch64::aarch64_selected_constraint_keys(target),
        };
        assert_eq!(deleted, resolved, "roster moved for {target:?}");
        if let Some(deleted) = deleted {
            let projected = super::super::catalog::selected_environment_keys(deleted);
            let projected_resolved =
                super::super::catalog::selected_environment_keys(resolved.unwrap());
            assert_eq!(
                projected, projected_resolved,
                "projection moved for {target:?}"
            );
        }
    }
}
