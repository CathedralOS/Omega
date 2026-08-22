use super::{
    Reg64, append_mov_r15_imm64, append_runtime_value_operand, append_store_r10_to_r15,
    append_store_rax_to_r15, runtime_value_operand_width,
};
use omega_target_operations::{RuntimeValueOperandHandle, RuntimeValueOperandSource};
use psi_diagnostics::Diagnostic;

pub fn machine_halt_width() -> usize {
    1
}

/// The x86 `hlt` instruction (`asm { hlt }`): a single 0xF4 opcode that halts
/// the CPU until the next interrupt. Position-independent, no relocation.
pub fn encode_machine_halt_bytes() -> [u8; 1] {
    [0xf4]
}

pub const fn memory_fence_width() -> usize {
    3
}

/// Exact x86 SSE2 fence encodings: `0f ae /5`, `/7`, and `/6` for load,
/// store, and full ordering respectively.
pub const fn encode_memory_fence_bytes(
    kind: psi_language_core::inline_assembly::AsmFenceKind,
) -> [u8; 3] {
    use psi_language_core::inline_assembly::AsmFenceKind;
    match kind {
        AsmFenceKind::Load => [0x0f, 0xae, 0xe8],
        AsmFenceKind::Store => [0x0f, 0xae, 0xf8],
        AsmFenceKind::Full => [0x0f, 0xae, 0xf0],
    }
}

pub const fn interrupt_control_width() -> usize {
    1
}

/// Exact x86 interrupt-flag encodings: CLI clears RFLAGS.IF and STI sets it,
/// with STI's architectural one-instruction recognition delay represented in
/// the catalog contract.
pub const fn encode_interrupt_control_bytes(
    kind: psi_language_core::inline_assembly::AsmInterruptControlKind,
) -> [u8; 1] {
    use psi_language_core::inline_assembly::AsmInterruptControlKind;
    match kind {
        AsmInterruptControlKind::Disable => [0xfa],
        AsmInterruptControlKind::Enable => [0xfb],
    }
}

pub const fn lidt_from_r10_width() -> usize {
    4
}

/// Checked `lidt [r10]`: R10 points at the 10-byte x86-64 descriptor admitted
/// by the inline-assembly contract.
pub const fn encode_lidt_from_r10_bytes() -> [u8; 4] {
    [0x41, 0x0f, 0x01, 0x1a]
}

/// Exact register read by the checked `lidt [r10]` encoding.
pub fn lidt_from_r10_clobbers() -> omega_calling_conventions::RegisterSet {
    omega_calling_conventions::RegisterSet::new([
        omega_calling_conventions::MachineRegister::X86R10,
    ])
}

// --- RFLAGS snapshot/restore -------------------------------------------------

/// Byte offset of the destination-region `mov r15, imm64` inside a flags
/// snapshot sequence. The relocation targets its immediate at offset +2.
pub const FLAGS_SNAPSHOT_DESTINATION_BASE_OFFSET: usize = 3;
const FLAGS_SNAPSHOT_DESTINATION_STORE_WIDTH: usize = 10 + 7;

pub const fn flags_snapshot_width() -> usize {
    1 + 2 + FLAGS_SNAPSHOT_DESTINATION_STORE_WIDTH
}

/// `asm { pushfq <dest> }` as a stack-balanced value operation:
/// `pushfq; pop r10; mov r15,<dest-base>; mov [r15+disp32],r10`.
pub fn encode_flags_snapshot(dest_byte_offset: usize) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(flags_snapshot_width());
    bytes.push(0x9c); // pushfq
    bytes.extend([0x41, 0x5a]); // pop r10
    append_mov_r15_imm64(&mut bytes, 0); // destination region base (relocated)
    append_store_r10_to_r15(&mut bytes, dest_byte_offset)?;
    debug_assert_eq!(bytes.len(), flags_snapshot_width());
    Ok(bytes)
}

pub fn flags_restore_width(
    source: &impl RuntimeValueOperandSource,
    operand: RuntimeValueOperandHandle,
) -> usize {
    runtime_value_operand_width(source, operand) + 3
}

/// `asm { popfq <source> }` as a stack-balanced value operation:
/// `load r10,<source>; push r10; popfq`.
pub fn encode_flags_restore(
    source: &impl RuntimeValueOperandSource,
    operand: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(flags_restore_width(source, operand));
    append_runtime_value_operand(source, &mut bytes, Reg64::R10, operand)?;
    bytes.extend([0x41, 0x52]); // push r10
    bytes.push(0x9d); // popfq
    debug_assert_eq!(bytes.len(), flags_restore_width(source, operand));
    Ok(bytes)
}

// --- model-specific registers (`rdmsr` / `wrmsr`) --------------------------

const MSR_READ_RESULT_COMBINE_WIDTH: usize = 3 + 4 + 3;
const MSR_DESTINATION_STORE_WIDTH: usize = 10 + 7;
pub const MSR_WRITE_INDEX_STASH_WIDTH: usize = 2;

pub fn msr_read_destination_base_offset(
    source: &impl RuntimeValueOperandSource,
    index: RuntimeValueOperandHandle,
) -> usize {
    runtime_value_operand_width(source, index)
        + 3 // mov ecx, r10d
        + 2 // rdmsr
        + MSR_READ_RESULT_COMBINE_WIDTH
}

pub fn msr_read_width(
    source: &impl RuntimeValueOperandSource,
    index: RuntimeValueOperandHandle,
) -> usize {
    msr_read_destination_base_offset(source, index) + MSR_DESTINATION_STORE_WIDTH
}

/// `asm { rdmsr <dest>, <index> }`: load ECX, execute RDMSR, combine the
/// architectural EDX:EAX result into one u64, and store it to `dest`.
pub fn encode_msr_read(
    source: &impl RuntimeValueOperandSource,
    index: RuntimeValueOperandHandle,
    dest_byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(msr_read_width(source, index));
    append_runtime_value_operand(source, &mut bytes, Reg64::R10, index)?;
    bytes.extend([0x44, 0x89, 0xd1]); // mov ecx, r10d
    bytes.extend([0x0f, 0x32]); // rdmsr -> edx:eax
    bytes.extend([0x41, 0x89, 0xc2]); // mov r10d, eax (zero extends)
    bytes.extend([0x48, 0xc1, 0xe2, 0x20]); // shl rdx, 32
    bytes.extend([0x49, 0x09, 0xd2]); // or r10, rdx
    append_mov_r15_imm64(&mut bytes, 0); // destination region base (relocated)
    append_store_r10_to_r15(&mut bytes, dest_byte_offset)?;
    debug_assert_eq!(bytes.len(), msr_read_width(source, index));
    Ok(bytes)
}

pub fn msr_write_width(
    source: &impl RuntimeValueOperandSource,
    index: RuntimeValueOperandHandle,
    value: RuntimeValueOperandHandle,
) -> usize {
    runtime_value_operand_width(source, index)
        + MSR_WRITE_INDEX_STASH_WIDTH // push r10
        + runtime_value_operand_width(source, value)
        + 2 // pop r10
        + 3 // mov ecx, r10d
        + 3 // mov eax, r11d
        + 3 // mov rdx, r11
        + 4 // shr rdx, 32
        + 2 // wrmsr
}

/// `asm { wrmsr <index>, <value> }`: preserve the index while evaluating the
/// value, split the u64 into EDX:EAX, then execute WRMSR. The temporary stack
/// use is balanced inside the realized sequence.
pub fn encode_msr_write(
    source: &impl RuntimeValueOperandSource,
    index: RuntimeValueOperandHandle,
    value: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(msr_write_width(source, index, value));
    append_runtime_value_operand(source, &mut bytes, Reg64::R10, index)?;
    bytes.extend([0x41, 0x52]); // push r10
    append_runtime_value_operand(source, &mut bytes, Reg64::R11, value)?;
    bytes.extend([0x41, 0x5a]); // pop r10
    bytes.extend([0x44, 0x89, 0xd1]); // mov ecx, r10d
    bytes.extend([0x44, 0x89, 0xd8]); // mov eax, r11d
    bytes.extend([0x4c, 0x89, 0xda]); // mov rdx, r11
    bytes.extend([0x48, 0xc1, 0xea, 0x20]); // shr rdx, 32
    bytes.extend([0x0f, 0x30]); // wrmsr
    debug_assert_eq!(bytes.len(), msr_write_width(source, index, value));
    Ok(bytes)
}

// --- control registers -----------------------------------------------------

pub const CONTROL_REGISTER_READ_DESTINATION_BASE_OFFSET: usize = 4;
const CONTROL_REGISTER_DESTINATION_STORE_WIDTH: usize = 10 + 7;

pub const fn control_register_read_width() -> usize {
    4 + CONTROL_REGISTER_DESTINATION_STORE_WIDTH
}

const fn control_register_modrm(
    register: psi_language_core::inline_assembly::AsmControlRegister,
) -> u8 {
    use psi_language_core::inline_assembly::AsmControlRegister;
    match register {
        AsmControlRegister::Cr0 => 0xc2,
        AsmControlRegister::Cr2 => 0xd2,
        AsmControlRegister::Cr3 => 0xda,
        AsmControlRegister::Cr4 => 0xe2,
    }
}

/// Read CR0/CR2/CR3/CR4 into R10, then store the exact u64 value to `dest`.
pub fn encode_control_register_read(
    register: psi_language_core::inline_assembly::AsmControlRegister,
    dest_byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(control_register_read_width());
    bytes.extend([0x41, 0x0f, 0x20, control_register_modrm(register)]);
    append_mov_r15_imm64(&mut bytes, 0);
    append_store_r10_to_r15(&mut bytes, dest_byte_offset)?;
    debug_assert_eq!(bytes.len(), control_register_read_width());
    Ok(bytes)
}

pub fn control_register_write_width(
    source: &impl RuntimeValueOperandSource,
    operand: RuntimeValueOperandHandle,
) -> usize {
    runtime_value_operand_width(source, operand) + 4
}

/// Load a u64 source into R10 and write it to CR0/CR3/CR4.
pub fn encode_control_register_write(
    source: &impl RuntimeValueOperandSource,
    register: psi_language_core::inline_assembly::AsmControlRegister,
    operand: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(control_register_write_width(source, operand));
    append_runtime_value_operand(source, &mut bytes, Reg64::R10, operand)?;
    bytes.extend([0x41, 0x0f, 0x22, control_register_modrm(register)]);
    debug_assert_eq!(bytes.len(), control_register_write_width(source, operand));
    Ok(bytes)
}

// --- port I/O (`asm { out .. }` / `asm { in .. }`) --------------------------
//
// The port operand loads into DX and the byte operand into AL by REUSING the
// generic runtime-value operand loader (into R10/R11, which handles immediate/
// storage/pointee/indexed forms and relocates storage reads), then a 3-byte
// register move parks the result in DX/AL for the one-byte `out`/`in`. Keeping
// the operand loader means storage operands relocate through the same
// machinery as every other runtime-value read (see omega-relocations); the
// relative offsets below MUST match the encoders (a drift silently relocates
// the wrong bytes and faults at runtime).

/// Width of the `mov edx, r10d` / `mov eax, r11d` register park after an
/// operand load (3 bytes each).
pub const PORT_OPERAND_REGISTER_MOVE_WIDTH: usize = 3;

pub fn port_write_width(
    source: &impl RuntimeValueOperandSource,
    port: RuntimeValueOperandHandle,
    value: RuntimeValueOperandHandle,
) -> usize {
    runtime_value_operand_width(source, port)
        + PORT_OPERAND_REGISTER_MOVE_WIDTH
        + runtime_value_operand_width(source, value)
        + PORT_OPERAND_REGISTER_MOVE_WIDTH
        + 1 // out dx, al
}

/// `asm { out <port>, <value> }` -> `out dx, al`. Loads `port` into DX and the
/// byte `value` into AL, then emits 0xEE.
pub fn encode_port_write(
    source: &impl RuntimeValueOperandSource,
    port: RuntimeValueOperandHandle,
    value: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    if let (Some(port), Some(value)) = (
        source
            .immediate_integer(port)
            .and_then(|value| u16::try_from(value).ok()),
        source
            .immediate_integer(value)
            .and_then(|value| u8::try_from(value).ok()),
    ) {
        return Ok(omega_x86_encoding::encode_immediate_port_write(port, value).to_vec());
    }
    let mut bytes = Vec::with_capacity(port_write_width(source, port, value));
    append_runtime_value_operand(source, &mut bytes, Reg64::R10, port)?;
    bytes.extend([0x44, 0x89, 0xd2]); // mov edx, r10d  -> DX = port
    append_runtime_value_operand(source, &mut bytes, Reg64::R11, value)?;
    bytes.extend([0x44, 0x89, 0xd8]); // mov eax, r11d  -> AL = value
    bytes.push(0xee); // out dx, al
    debug_assert_eq!(bytes.len(), port_write_width(source, port, value));
    Ok(bytes)
}

/// The `mov r15,imm64` (10) + `mov [r15+disp32], al` (7) tail that stores the
/// `in` result byte to a destination place.
const PORT_READ_DESTINATION_STORE_WIDTH: usize = 10 + 7;

pub fn port_read_width(
    source: &impl RuntimeValueOperandSource,
    port: RuntimeValueOperandHandle,
) -> usize {
    runtime_value_operand_width(source, port)
        + PORT_OPERAND_REGISTER_MOVE_WIDTH
        + 1 // in al, dx
        + PORT_READ_DESTINATION_STORE_WIDTH
}

/// `asm { in <dest>, <port> }` -> `in al, dx`. Loads `port` into DX, reads the
/// byte into AL (0xEC), then stores AL to the destination place. The
/// `mov r15,imm64=0` is relocated to `dest`'s storage region by
/// omega-relocations, exactly like any storage write.
pub fn encode_port_read(
    source: &impl RuntimeValueOperandSource,
    port: RuntimeValueOperandHandle,
    dest_byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(port_read_width(source, port));
    append_runtime_value_operand(source, &mut bytes, Reg64::R10, port)?;
    bytes.extend([0x44, 0x89, 0xd2]); // mov edx, r10d  -> DX = port
    bytes.push(0xec); // in al, dx  -> AL = port byte
    append_mov_r15_imm64(&mut bytes, 0); // dest region base (relocated)
    append_store_rax_to_r15(&mut bytes, dest_byte_offset, 1)?; // mov [r15+disp32], al
    debug_assert_eq!(bytes.len(), port_read_width(source, port));
    Ok(bytes)
}

#[cfg(test)]
mod machine_control_tests {
    use super::*;
    use crate::{append_load_unsigned_reg_from_r15, unsigned_load_width};
    use omega_target_operations::{RuntimeStorageRegion, StateGuardOperator};

    #[test]
    fn machine_halt_is_a_single_hlt_opcode() {
        // `asm { hlt }` must encode to exactly the one-byte x86 HLT (0xF4),
        // and its width must agree with the emitter (privileged_effects_and_
        // binary_trust brief, machine_control M3 subset).
        assert_eq!(encode_machine_halt_bytes(), [0xf4]);
        assert_eq!(machine_halt_width(), 1);
        assert_eq!(encode_machine_halt_bytes().len(), machine_halt_width());
    }

    #[test]
    fn memory_fences_have_exact_sse2_encodings() {
        use psi_language_core::inline_assembly::AsmFenceKind;

        for (kind, bytes) in [
            (AsmFenceKind::Load, [0x0f, 0xae, 0xe8]),
            (AsmFenceKind::Store, [0x0f, 0xae, 0xf8]),
            (AsmFenceKind::Full, [0x0f, 0xae, 0xf0]),
        ] {
            assert_eq!(encode_memory_fence_bytes(kind), bytes);
            assert_eq!(bytes.len(), memory_fence_width());
        }
    }

    #[test]
    fn interrupt_control_has_exact_cli_sti_encodings() {
        use psi_language_core::inline_assembly::AsmInterruptControlKind;

        assert_eq!(
            encode_interrupt_control_bytes(AsmInterruptControlKind::Disable),
            [0xfa]
        );
        assert_eq!(
            encode_interrupt_control_bytes(AsmInterruptControlKind::Enable),
            [0xfb]
        );
        assert_eq!(interrupt_control_width(), 1);
    }

    #[test]
    fn checked_lidt_uses_the_pinned_r10_encoding() {
        assert_eq!(encode_lidt_from_r10_bytes(), [0x41, 0x0f, 0x01, 0x1a]);
        assert_eq!(encode_lidt_from_r10_bytes().len(), lidt_from_r10_width());
    }

    #[test]
    fn flags_snapshot_is_stack_balanced_and_stores_the_full_register() {
        let bytes = encode_flags_snapshot(24).expect("encode RFLAGS snapshot");
        assert_eq!(bytes.len(), flags_snapshot_width());
        assert_eq!(&bytes[0..3], &[0x9c, 0x41, 0x5a]); // pushfq; pop r10
        assert_eq!(&bytes[3..5], &[0x49, 0xbf]); // mov r15, imm64
        assert_eq!(&bytes[5..13], &0u64.to_le_bytes());
        assert_eq!(&bytes[13..16], &[0x4d, 0x89, 0x97]); // [r15+disp32] = r10
        assert_eq!(&bytes[16..20], &24u32.to_le_bytes());
        assert_eq!(FLAGS_SNAPSHOT_DESTINATION_BASE_OFFSET, 3);
    }

    #[test]
    fn unsigned_index_loads_use_exact_zero_extending_widths() {
        let expected_prefixes: &[(usize, &[u8])] = &[
            (1, &[0x45, 0x0f, 0xb6, 0x9f]),
            (2, &[0x45, 0x0f, 0xb7, 0x9f]),
            (4, &[0x45, 0x8b, 0x9f]),
            (8, &[0x4d, 0x8b, 0x9f]),
        ];
        for &(byte_size, prefix) in expected_prefixes {
            let mut bytes = Vec::new();
            append_load_unsigned_reg_from_r15(&mut bytes, Reg64::R11, 24, byte_size)
                .expect("supported index width");
            assert_eq!(&bytes[..prefix.len()], prefix);
            assert_eq!(bytes.len(), unsigned_load_width(byte_size));
        }
    }

    /// A RuntimeValueOperandSource where every handle is an immediate integer
    /// (handle arena index -> value). Immediates emit no relocation, so the
    /// port-encoder byte layout is fully deterministic.
    struct ImmediateOperands(Vec<i64>);
    impl RuntimeValueOperandSource for ImmediateOperands {
        fn immediate_integer(&self, handle: RuntimeValueOperandHandle) -> Option<i64> {
            self.0.get(handle.arena_index() as usize).copied()
        }
        fn storage(
            &self,
            _: RuntimeValueOperandHandle,
        ) -> Option<(RuntimeStorageRegion, usize, usize)> {
            None
        }
        fn bit_field(
            &self,
            _: RuntimeValueOperandHandle,
        ) -> Option<(
            RuntimeStorageRegion,
            usize,
            usize,
            Vec<omega_target_operations::RuntimeBitFieldFragment>,
        )> {
            None
        }
        fn pointee(&self, _: RuntimeValueOperandHandle) -> Option<(usize, usize, usize)> {
            None
        }
        fn frame_indexed(
            &self,
            _: RuntimeValueOperandHandle,
        ) -> Option<(
            usize,
            RuntimeStorageRegion,
            usize,
            usize,
            usize,
            usize,
            usize,
        )> {
            None
        }
        fn frame_base_indexed(
            &self,
            _: RuntimeValueOperandHandle,
        ) -> Option<(usize, usize, usize, usize, usize, usize)> {
            None
        }
        fn frame_fixed_indexed(
            &self,
            _: RuntimeValueOperandHandle,
        ) -> Option<(usize, usize, usize, usize, usize)> {
            None
        }
        fn machine_indexed(
            &self,
            _: RuntimeValueOperandHandle,
        ) -> Option<(
            usize,
            RuntimeStorageRegion,
            usize,
            usize,
            usize,
            usize,
            usize,
        )> {
            None
        }
        fn binary(
            &self,
            _: RuntimeValueOperandHandle,
        ) -> Option<(
            RuntimeValueOperandHandle,
            StateGuardOperator,
            RuntimeValueOperandHandle,
        )> {
            None
        }
        fn binary_is_float(&self, _: RuntimeValueOperandHandle) -> bool {
            false
        }
        fn binary_byte_width(&self, _: RuntimeValueOperandHandle) -> Option<usize> {
            None
        }
        fn binary_arithmetic_domain(
            &self,
            _: RuntimeValueOperandHandle,
        ) -> Option<(psi_numerics::arithmetic::ArithmeticDomain, bool)> {
            None
        }
        fn convert(
            &self,
            _: RuntimeValueOperandHandle,
        ) -> Option<(RuntimeValueOperandHandle, usize, usize, bool, bool, bool)> {
            None
        }
        fn convert_trapping(&self, _: RuntimeValueOperandHandle) -> bool {
            false
        }
        fn convert_saturating(&self, _: RuntimeValueOperandHandle) -> bool {
            false
        }
        fn convert_target_signed(&self, _: RuntimeValueOperandHandle) -> bool {
            false
        }
        fn text_equals(
            &self,
            _: RuntimeValueOperandHandle,
        ) -> Option<(
            RuntimeStorageRegion,
            usize,
            bool,
            RuntimeStorageRegion,
            usize,
            bool,
        )> {
            None
        }
        fn text_equals_literal(
            &self,
            _: RuntimeValueOperandHandle,
        ) -> Option<(RuntimeValueOperandHandle, std::sync::Arc<[u8]>, bool)> {
            None
        }
    }

    #[test]
    fn flags_restore_is_stack_balanced_after_operand_load() {
        let source = ImmediateOperands(vec![0x202]);
        let operand = RuntimeValueOperandHandle::from_parts(0, 1);
        let bytes = encode_flags_restore(&source, operand).expect("encode RFLAGS restore");
        assert_eq!(bytes.len(), flags_restore_width(&source, operand));
        assert_eq!(&bytes[0..2], &[0x49, 0xba]); // mov r10, imm64
        assert_eq!(&bytes[2..10], &0x202u64.to_le_bytes());
        assert_eq!(&bytes[10..13], &[0x41, 0x52, 0x9d]); // push r10; popfq
    }

    #[test]
    fn msr_read_combines_edx_eax_and_stores_u64() {
        let source = ImmediateOperands(vec![0xc000_0080]);
        let index = RuntimeValueOperandHandle::from_parts(0, 1);
        let bytes = encode_msr_read(&source, index, 24).expect("encode RDMSR");
        assert_eq!(bytes.len(), msr_read_width(&source, index));
        assert_eq!(&bytes[10..15], &[0x44, 0x89, 0xd1, 0x0f, 0x32]);
        assert_eq!(
            &bytes[15..25],
            &[0x41, 0x89, 0xc2, 0x48, 0xc1, 0xe2, 0x20, 0x49, 0x09, 0xd2]
        );
        assert_eq!(msr_read_destination_base_offset(&source, index), 25);
        assert_eq!(&bytes[25..27], &[0x49, 0xbf]);
        assert_eq!(&bytes[35..38], &[0x4d, 0x89, 0x97]);
        assert_eq!(&bytes[38..42], &24u32.to_le_bytes());
    }

    #[test]
    fn msr_write_preserves_index_and_splits_u64_value() {
        let source = ImmediateOperands(vec![0xc000_0080, 0x1122_3344_5566_7788]);
        let index = RuntimeValueOperandHandle::from_parts(0, 1);
        let value = RuntimeValueOperandHandle::from_parts(1, 1);
        let bytes = encode_msr_write(&source, index, value).expect("encode WRMSR");
        assert_eq!(bytes.len(), msr_write_width(&source, index, value));
        assert_eq!(&bytes[10..12], &[0x41, 0x52]); // push r10 index
        assert_eq!(&bytes[22..24], &[0x41, 0x5a]); // pop r10 index
        assert_eq!(&bytes[24..30], &[0x44, 0x89, 0xd1, 0x44, 0x89, 0xd8]);
        assert_eq!(&bytes[30..37], &[0x4c, 0x89, 0xda, 0x48, 0xc1, 0xea, 0x20]);
        assert_eq!(&bytes[37..39], &[0x0f, 0x30]);
    }

    #[test]
    fn control_register_reads_use_exact_modrm_and_store_u64() {
        use psi_language_core::inline_assembly::AsmControlRegister;

        for (register, modrm) in [
            (AsmControlRegister::Cr0, 0xc2),
            (AsmControlRegister::Cr2, 0xd2),
            (AsmControlRegister::Cr3, 0xda),
            (AsmControlRegister::Cr4, 0xe2),
        ] {
            let bytes = encode_control_register_read(register, 24).expect("encode MOV from CR");
            assert_eq!(bytes.len(), control_register_read_width());
            assert_eq!(&bytes[0..4], &[0x41, 0x0f, 0x20, modrm]);
            assert_eq!(&bytes[4..6], &[0x49, 0xbf]);
            assert_eq!(&bytes[6..14], &0u64.to_le_bytes());
            assert_eq!(&bytes[14..17], &[0x4d, 0x89, 0x97]);
            assert_eq!(&bytes[17..21], &24u32.to_le_bytes());
        }
        assert_eq!(CONTROL_REGISTER_READ_DESTINATION_BASE_OFFSET, 4);
    }

    #[test]
    fn control_register_writes_use_exact_modrm_after_u64_materialization() {
        use psi_language_core::inline_assembly::AsmControlRegister;

        let source = ImmediateOperands(vec![0x1122_3344_5566_7788]);
        let value = RuntimeValueOperandHandle::from_parts(0, 1);
        for (register, modrm) in [
            (AsmControlRegister::Cr0, 0xc2),
            (AsmControlRegister::Cr3, 0xda),
            (AsmControlRegister::Cr4, 0xe2),
        ] {
            let bytes =
                encode_control_register_write(&source, register, value).expect("encode MOV to CR");
            assert_eq!(bytes.len(), control_register_write_width(&source, value));
            assert_eq!(&bytes[0..2], &[0x49, 0xba]);
            assert_eq!(&bytes[2..10], &0x1122_3344_5566_7788u64.to_le_bytes());
            assert_eq!(&bytes[10..14], &[0x41, 0x0f, 0x22, modrm]);
        }
    }

    #[test]
    fn port_write_immediate_operands_encode_out_dx_al() {
        // `asm { out 0x3F8, 'A' }`: port 0x3F8 -> DX, value 0x41 -> AL, out.
        let source = ImmediateOperands(vec![0x3f8, 0x41]);
        let port = RuntimeValueOperandHandle::from_parts(0, 1);
        let value = RuntimeValueOperandHandle::from_parts(1, 1);
        let bytes = encode_port_write(&source, port, value).expect("encode");
        assert_eq!(bytes.len(), port_write_width(&source, port, value));
        // mov r10, imm64=0x3F8
        assert_eq!(&bytes[0..2], &[0x49, 0xba]);
        assert_eq!(&bytes[2..10], &0x3f8u64.to_le_bytes());
        assert_eq!(&bytes[10..13], &[0x44, 0x89, 0xd2]); // mov edx, r10d
        // mov r11, imm64=0x41
        assert_eq!(&bytes[13..15], &[0x49, 0xbb]);
        assert_eq!(&bytes[15..23], &0x41u64.to_le_bytes());
        assert_eq!(&bytes[23..26], &[0x44, 0x89, 0xd8]); // mov eax, r11d
        assert_eq!(bytes[26], 0xee); // out dx, al
        assert_eq!(bytes.len(), 27);
    }

    #[test]
    fn port_read_immediate_port_encodes_in_al_dx_then_store() {
        // `asm { in status, 0x3FD }` with status at machine+4: port 0x3FD -> DX,
        // in al,dx, then mov [r15+4], al (r15 relocated to the dest region).
        let source = ImmediateOperands(vec![0x3fd]);
        let port = RuntimeValueOperandHandle::from_parts(0, 1);
        let bytes = encode_port_read(&source, port, 4).expect("encode");
        assert_eq!(bytes.len(), port_read_width(&source, port));
        assert_eq!(&bytes[0..2], &[0x49, 0xba]); // mov r10, imm64
        assert_eq!(&bytes[2..10], &0x3fdu64.to_le_bytes());
        assert_eq!(&bytes[10..13], &[0x44, 0x89, 0xd2]); // mov edx, r10d
        assert_eq!(bytes[13], 0xec); // in al, dx
        assert_eq!(&bytes[14..16], &[0x49, 0xbf]); // mov r15, imm64=0 (relocated)
        assert_eq!(&bytes[16..24], &0u64.to_le_bytes());
        assert_eq!(&bytes[24..27], &[0x41, 0x88, 0x87]); // mov [r15+disp32], al
        assert_eq!(&bytes[27..31], &4u32.to_le_bytes()); // disp32 = dest offset
        assert_eq!(bytes.len(), 31);
    }
}
