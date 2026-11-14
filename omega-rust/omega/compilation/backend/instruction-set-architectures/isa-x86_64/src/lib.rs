/*
We provide the x86-64 physical register model, register-constraint catalog,
machine-effect catalog, frame and MXCSR custody protocols, hosted syscall
sequences, post-handoff writer lowering, and
selected-form instruction encoders and validators for the native realization
pipeline. Every interface in this crate consumes only normalized target,
register-home, and terminal-installation facts, with zero dependency on
source-shaped Psi representations.
*/

/*
We pair every x86-64 byte producer with an independent structural decoder and
validator inside the same domain boundary so that variable-length x86-64
instruction streams, REX/VEX prefixes, ModRM/SIB addressing modes, and signed
8-bit/32-bit displacements are verified from actual emitted bytes rather than
trusted from producer claims. Sharing the register-constraint keys, machine
effect declarations, and realization tables across selection, register
allocation, branch relaxation, and binary emission guarantees end-to-end
agreement on register reads, clobbers, flags, memory footprints, and traps.
*/

/*
Our domain modules serve the x86-64 backend across System V AMD64, Microsoft
x64, Linux, macOS, and UEFI targets:
- `register_model` and `preservation_storage`: physical x86-64 registers,
  System V and Microsoft x64 call/operation constraint keys, and callee-saved
  preservation catalogs.
- `machine_effects`: per-alternative encoded effect declarations and size bounds.
- `frame_protocol` and `ieee_float`: fixed-frame prologue/epilogue save/restore,
  4 KiB stack-commit probing, aligned frame addressing, exact IEEE scalar bit
  materialization, MXCSR custody, and canonical scalar FMA3 decoding.
- `selected_form_encoding`: encoding, structural decoding, and validation for
  all x86-64 selected instruction forms, branches, call templates, atomics, and
  installed interrupt/table sequences.
- `generated_entry`, `hosted_kernel`, and `post_handoff_writer`: generated entry
  replay, import-free Linux and Darwin syscall leaves, and post-handoff writer
  lowering.
*/

pub(crate) use optimization_core::codec_bytes;

pub(crate) fn encoder_resolved(
    minimum_bytes: u16,
    maximum_bytes: Option<u16>,
) -> selected_instructions::MachineSizeKnowledge {
    selected_instructions::MachineSizeKnowledge::EncoderResolved {
        minimum_bytes,
        maximum_bytes,
    }
}
mod model_specific_state {
    use calling_conventions::{MachineState, MachineStateSet, RegisterSet, StateEffectsEvidence};
    use selected_instructions::RegisterOperation;

    pub fn x86_64_model_specific_register_state(
        operation: RegisterOperation,
        selector: Option<u32>,
    ) -> Result<StateEffectsEvidence, &'static str> {
        match operation {
            RegisterOperation::ModelSpecificRead => match selector {
                Some(0xc000_0080 | 0x277) => Ok(StateEffectsEvidence::new(
                    RegisterSet::default(),
                    RegisterSet::default(),
                    MachineStateSet::new([MachineState::ControlState]),
                    MachineStateSet::empty(),
                )),
                Some(_) => Err("MSR read selector has no target-defined architectural footprint"),
                None => Err("MSR read selector is not an exact retained u32 constant"),
            },
            RegisterOperation::ModelSpecificWrite => {
                Err("MSR write has no target-defined architectural footprint")
            }
            _ => Err("MSR architectural footprint requested for another operation"),
        }
    }
}
pub use model_specific_state::x86_64_model_specific_register_state;
mod frame_protocol;
mod generated_entry;
pub use generated_entry::decode_x86_64_generated_entry;
pub const ORDINARY_DIRECT_CALL_BYTES: [u8; 5] = [0xe8, 0, 0, 0, 0];
pub const ORDINARY_RETURN_BYTES: [u8; 1] = [0xc3];
pub const INTERRUPT_RETURN_BYTES: [u8; 2] = [0x48, 0xcf];
pub const CLEAR_DIRECTION_FLAG_BYTES: [u8; 1] = [0xfc];
pub const PRIVILEGED_STOP_BYTES: [u8; 4] = [0xfa, 0xf4, 0xeb, 0xfd];
mod hosted_kernel;
mod ieee_float;
mod import_thunk {
    use calling_conventions::{MachineState, MachineStateSet, RegisterSet, StateFootprintEvidence};

    use crate::selected_form_encoding::decoding::{DecodedInstruction, decode_all};

    pub const X86_64_IMPORT_THUNK_BYTE_COUNT: usize = 6;

    pub fn x86_64_import_thunk_footprint() -> StateFootprintEvidence {
        StateFootprintEvidence::new(
            RegisterSet::default(),
            MachineStateSet::new([MachineState::InstructionPointer]),
        )
    }

    pub fn decode_x86_64_import_thunk(bytes: &[u8]) -> Option<i32> {
        match decode_all(bytes) {
            Ok(rows) => match rows.as_slice() {
                [DecodedInstruction::JumpIndirectRip { displacement }] => Some(*displacement),
                _ => None,
            },
            Err(_) => None,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{
            X86_64_IMPORT_THUNK_BYTE_COUNT, decode_x86_64_import_thunk,
            x86_64_import_thunk_footprint,
        };
        use calling_conventions::{
            MachineState, MachineStateSet, RegisterSet, StateFootprintEvidence,
        };

        #[test]
        fn decode_returns_the_emitted_displacement() {
            let bytes = [0xff, 0x25, 0x78, 0x56, 0x34, 0x12];
            assert_eq!(bytes.len(), X86_64_IMPORT_THUNK_BYTE_COUNT);
            assert_eq!(decode_x86_64_import_thunk(&bytes), Some(0x1234_5678));
            assert_eq!(
                decode_x86_64_import_thunk(&[0xff, 0x25, 0, 0, 0, 0]),
                Some(0)
            );
        }

        #[test]
        fn decode_rejects_everything_but_the_closed_form() {
            assert_eq!(
                decode_x86_64_import_thunk(&[0xff, 0x24, 0x24, 0, 0, 0]),
                None
            );
            assert_eq!(decode_x86_64_import_thunk(&[0xff, 0x25, 0, 0, 0]), None);
            assert_eq!(
                decode_x86_64_import_thunk(&[0xff, 0x25, 0, 0, 0, 0, 0x90]),
                None
            );
            assert_eq!(decode_x86_64_import_thunk(&[]), None);
            assert_eq!(decode_x86_64_import_thunk(&[0xc3, 0, 0, 0, 0, 0]), None);
        }

        #[test]
        fn footprint_claims_the_instruction_pointer_alone() {
            assert_eq!(
                x86_64_import_thunk_footprint(),
                StateFootprintEvidence::new(
                    RegisterSet::default(),
                    MachineStateSet::new([MachineState::InstructionPointer]),
                )
            );
        }
    }
}
mod machine_effects;
mod post_handoff_writer;
mod preservation_storage;
mod register_model;
pub use register_model::X86_64_COPY_BYTES;
pub use register_model::X86_64_DIVIDE_I64;
pub use register_model::X86_64_DIVIDE_U64;
pub use register_model::X86_64_LOAD8;
pub use register_model::X86_64_LOAD16;
pub use register_model::X86_64_LOAD32;
pub use register_model::X86_64_MATERIALIZE_BOOLEAN;
pub use register_model::X86_64_MULTIPLY_I64;
pub use register_model::X86_64_REMAINDER_I64;
pub use register_model::X86_64_REMAINDER_U64;
pub use register_model::X86_64_SATURATING_ADD_CLAMPED;
pub use register_model::X86_64_SATURATING_ADD_U64;
pub use register_model::X86_64_SATURATING_DIVIDE_SIGNED;
pub use register_model::X86_64_SATURATING_MULTIPLY_CLAMPED;
pub use register_model::X86_64_SATURATING_MULTIPLY_U64;
pub use register_model::X86_64_SATURATING_SUBTRACT_CLAMPED;
pub use register_model::X86_64_SATURATING_SUBTRACT_UNSIGNED;
pub use register_model::X86_64_SHIFT_I64;
pub use register_model::X86_64_ZERO_BYTES;
pub use register_model::x86_64_linux_system_call_keys;
#[cfg(test)]
pub(crate) use register_model::x86_64_required_register_constraints;
pub use register_model::{
    X86_64_ATOMIC_BITWISE_UPDATE, X86_64_ATOMIC_COMPARE_EXCHANGE,
    X86_64_ATOMIC_COMPARE_EXCHANGE_ONCE, X86_64_ATOMIC_FENCE, X86_64_ATOMIC_LOAD,
    X86_64_ATOMIC_STORE, X86_64_ATOMIC_UPDATE, X86_64_CODE_SEGMENT_RELOAD,
    X86_64_CONTROL_REGISTER_READ, X86_64_CONTROL_REGISTER_WRITE, X86_64_DATA_SEGMENT_RELOAD,
    X86_64_FLAGS_RESTORE, X86_64_FLAGS_SNAPSHOT, X86_64_GLOBAL_DESCRIPTOR_TABLE,
    X86_64_INSTALLED_INTERRUPT_CPU_CHECK, X86_64_INSTALLED_INTERRUPT_TABLE_PUBLICATION,
    X86_64_INSTALLED_INTERRUPT_TASK_COMMIT, X86_64_INTERRUPT_CONTROL, X86_64_MEMORY_FENCE,
    X86_64_MODEL_SPECIFIC_READ, X86_64_MODEL_SPECIFIC_WRITE, X86_64_PORT_REGISTER_READ,
    X86_64_PORT_REGISTER_WRITE, X86_64_PORT_WRITE, X86_64_PROCESSOR_HALT,
    X86_64_SEGMENT_SELECTOR_READ, X86_64_TASK_REGISTER_ACTIVATE, X86_64_TASK_REGISTER_READ,
};
pub use register_model::{
    X86_64_BITS_TO_FLOAT32, X86_64_BITS_TO_FLOAT64, X86_64_CRASH, X86_64_FLOAT_RESULT_SATURATING,
    X86_64_FLOAT_RESULT_TRAPPING, X86_64_FLOAT32_TO_BITS, X86_64_FLOAT64_TO_BITS,
    X86_64_HOSTED_READ_BYTE, X86_64_IEEE_FLOAT_ARITHMETIC, X86_64_IEEE_FLOAT_CONVERSION_CONTROL,
    X86_64_IEEE_FLOAT_DIRECTED_BINARY, X86_64_IEEE_FLOAT_DIRECTED_UNARY, X86_64_IEEE_FLOAT_TERNARY,
    X86_64_IEEE_FLOAT_UNARY, X86_64_PRIVILEGED_CRASH,
};
pub use register_model::{X86_64_COMPARE_BYTE_SEQUENCES, X86_64_SCAN_BYTE_SEQUENCE};
pub use register_model::{X86_64_LOAD_ENCODED_SCALAR, X86_64_STORE_ENCODED_SCALAR};
pub use register_model::{
    X86_64_TRAPPING_BINARY, X86_64_TRAPPING_CONVERT, X86_64_TRAPPING_FIXED_PAIR,
    X86_64_TRAPPING_SHIFT,
};
pub use register_model::{
    x86_64_microsoft_mixed_unit_call_keys, x86_64_system_v_mixed_unit_call_keys,
};
mod selected_form_encoding;
pub use register_model::{X86_64_RESTORE_FLOATING_CONTROL, X86_64_SAVE_FLOATING_CONTROL};
pub use selected_form_encoding::system_call::{
    encode_x86_64_selected_system_call_form, validate_x86_64_selected_system_call_form,
};

pub use frame_protocol::{
    X86_64FrameProtocolError, X86_64FrameSlot, X86_64StackProbe,
    encode_system_v_amd64_frame_protocol, encode_x86_64_aligned_frame_address,
    encode_x86_64_frame_address, encode_x86_64_frame_memory, encode_x86_64_integer_constant,
    encode_x86_64_pointer_memory,
};
pub use ieee_float::{DecodedScalarFmaFormat, decode_vfmadd132_scalar};
pub use ieee_float::{
    OMEGA_CANONICAL_MXCSR, encode_binary32_bits_to_xmm, encode_binary64_bits_to_xmm,
    encode_ldmxcsr_rsp_displacement, encode_stmxcsr_rsp_displacement,
    encode_store_mxcsr_constant_rsp_displacement,
};
pub use import_thunk::{
    X86_64_IMPORT_THUNK_BYTE_COUNT, decode_x86_64_import_thunk, x86_64_import_thunk_footprint,
};
pub use machine_effects::{
    X86_64MachineEffectCatalogValidationError, validate_x86_64_machine_effect_catalog,
    x86_64_machine_effect_catalog, x86_64_normalized_foreign_call_declaration,
    x86_64_selected_constraint_keys,
};
pub use post_handoff_writer::{
    encode_generated_post_handoff_writer_bytes,
    generated_post_handoff_writer_additional_machine_state, generated_post_handoff_writer_clobbers,
    generated_post_handoff_writer_width,
};
pub use preservation_storage::x86_64_preservation_storage_catalog;
pub use register_model::X86_64_HOSTED_WRITE_BYTE_I32;
pub use register_model::x86_64_microsoft_normalized_foreign_call_keys;
pub use register_model::x86_64_microsoft_register_call_keys;
pub use register_model::x86_64_system_v_normalized_foreign_call_keys;
pub use register_model::x86_64_system_v_register_call_keys;
pub use register_model::{
    X86_64_ADD_I64, X86_64_ADD_I64_IMMEDIATE, X86_64_COMPARE_I64, X86_64_COMPARE_I64_IMMEDIATE,
    X86_64_COMPARE_I64_ZERO, X86_64_CONDITIONAL_BRANCH, X86_64_COPY_I64, X86_64_JUMP,
    X86_64_MATERIALIZE_I64, X86_64_MICROSOFT_CALL, X86_64_MICROSOFT_RETURN,
    X86_64_MICROSOFT_RETURN_UNIT, X86_64_SUBTRACT_I64, X86_64_SUBTRACT_I64_IMMEDIATE,
    X86_64_SYSTEM_V_CALL, X86_64_SYSTEM_V_RETURN, X86_64_SYSTEM_V_RETURN_UNIT,
    X86_64RegisterConstraintCatalogValidationError, validate_x86_64_register_constraint_catalog,
    validated_x86_64_physical_register_model, x86_64_fixed_register_view,
    x86_64_physical_register_model, x86_64_preservation_convention_for_target,
    x86_64_register_constraint_catalog,
};
pub use register_model::{
    X86_64_LOAD_PACKED, X86_64_STORE_PACKED, x86_64_microsoft_aggregate_call_keys,
    x86_64_microsoft_aggregate_return_keys, x86_64_microsoft_mixed_aggregate_call_keys,
    x86_64_system_v_mixed_aggregate_call_keys,
};
pub(crate) use register_model::{
    canonical_x86_64_physical_register_model_identity, x86_64_physical_register_model_ref,
    x86_64_register_constraint_catalog_for,
};
pub use register_model::{
    x86_64_microsoft_register_unit_call_keys, x86_64_system_v_register_unit_call_keys,
};
pub use register_model::{
    x86_64_system_v_aggregate_call_keys, x86_64_system_v_aggregate_return_keys,
};
pub use selected_form_encoding::hosted_read_byte::{
    decode_x86_64_selected_hosted_read_byte, encode_x86_64_selected_hosted_read_byte_form,
    validate_x86_64_selected_hosted_read_byte_form,
};
pub use selected_form_encoding::hosted_write_byte::decode_x86_64_selected_hosted_write_byte_i32;
pub use selected_form_encoding::materialization::mov_r32_imm32::X86_64MovR32Imm32I64MaterializationError;
pub use selected_form_encoding::materialization::mov_r64_imm32_sign_extended::X86_64MovR64Imm32SignExtendedI64MaterializationError;

pub use hosted_kernel::{
    encode_darwin_read_byte_to_stack, encode_hosted_exit_process_i32_for_target,
};
pub use hosted_kernel::{
    encode_hosted_exit_process_i32, encode_hosted_write_byte_i32_from_r11,
    encode_linux_read_byte_to_stack, encode_linux_write_line_literal,
};
pub(crate) use hosted_kernel::{hosted_exit_syscall_number, hosted_syscall_target};
pub use register_model::X86_64_HOSTED_EXIT_PROCESS_I32;
pub use register_model::x86_64_indirect_aggregate_call_keys;
pub use register_model::{
    X86_64_ADDRESS_OFFSET, X86_64_BYTE_LITERAL_IMAGE_ADDRESS, X86_64_FRAME_ADDRESS,
    X86_64_LOAD8_INDEXED, X86_64_LOAD64, X86_64_MICROSOFT_CALL_UNIT, X86_64_STORE, X86_64_STORE64,
};
pub use register_model::{x86_64_float_scalar_call_keys, x86_64_float_scalar_return_keys};
pub use selected_form_encoding::hosted_exit_process::{
    decode_x86_64_selected_hosted_exit_process_i32,
    encode_x86_64_selected_hosted_exit_process_form,
    validate_x86_64_selected_hosted_exit_process_form,
};
pub use selected_form_encoding::{
    ValidatedX86_64SelectedFormEncoding, X86_64_BYTE_LITERAL_ADDRESS_FIELD_OFFSET,
    X86_64CallbackAddressMaterialization, X86_64CallbackMaterializationDestination,
    X86_64NormalizedForeignCallFixup, X86_64NormalizedForeignCallFixupKind,
    X86_64NormalizedForeignCallFixupState, X86_64NormalizedForeignCallTemplateError,
    X86_64ScalarCallFixup, X86_64ScalarCallFixupKind, X86_64ScalarCallFixupState,
    X86_64ScalarCallTemplateError, X86_64SelectedFormEncodingError,
    encode_x86_64_selected_dynamic_call_template, encode_x86_64_selected_form,
    encode_x86_64_selected_frame_form, encode_x86_64_selected_i64_less_than_branch_form,
    encode_x86_64_selected_jump_form, encode_x86_64_selected_memory_form,
    encode_x86_64_selected_nonzero_branch_form,
    encode_x86_64_selected_normalized_foreign_call_template,
    encode_x86_64_selected_scalar_call_template, encode_x86_64_selected_short_nonzero_branch_form,
    encode_x86_64_selected_u64_less_than_branch_form, validate_x86_64_selected_form_encoding,
    validate_x86_64_selected_frame_form, validate_x86_64_selected_i64_less_than_branch_form,
    validate_x86_64_selected_jump_form, validate_x86_64_selected_memory_form,
    validate_x86_64_selected_nonzero_branch_form,
    validate_x86_64_selected_normalized_foreign_call_template,
    validate_x86_64_selected_scalar_call_template,
    validate_x86_64_selected_short_nonzero_branch_form,
    validate_x86_64_selected_u64_less_than_branch_form,
};
pub use selected_form_encoding::{
    encode_x86_64_selected_hosted_write_byte_form, validate_x86_64_selected_hosted_write_byte_form,
};

pub use register_model::{X86_64_ACCESSOR_READ, X86_64_ACCESSOR_WRITE};
pub use selected_form_encoding::{
    encode_x86_64_selected_table_call_form, validate_x86_64_selected_table_call_form,
};

pub use selected_form_encoding::callback_group_bytes as encode_private_callback_materialization;

pub use selected_form_encoding::register_access::installed_table::{
    X86InstalledCpuStateGeometry, X86InstalledCpuStateRegisters, X86InstalledTableEncodingError,
    X86InstalledTablePublicationRegisters, X86InstalledTaskCommitRegisters,
    encode_x86_64_installed_cpu_state_check, encode_x86_64_installed_table_publication,
    encode_x86_64_installed_task_commit, validate_x86_64_installed_cpu_state_check,
    validate_x86_64_installed_table_publication, validate_x86_64_installed_task_commit,
};
