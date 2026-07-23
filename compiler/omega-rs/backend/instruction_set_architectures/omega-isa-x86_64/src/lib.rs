mod place_copy;
pub use place_copy::{
    PLACE_COPY_MAX_SITES, PlaceCopySide, PlaceCopySites, copy_places_to_pointee_clobbers,
    encode_copy_places, encode_place_address_write, encode_place_binary_write,
    encode_place_bounded_buffer_literal_append, encode_place_bounded_buffer_source_append,
    encode_place_bounded_buffer_write, encode_place_compare, encode_place_convert_write,
    encode_place_copy, encode_place_copy_shared_base, encode_place_integer_write,
    encode_place_string_write, encode_place_value_compare, place_binary_index_base_positions,
    place_binary_operand_start_width, place_compare_additional_machine_state,
    place_compare_register_writes, place_value_compare_additional_machine_state,
    place_value_compare_register_writes,
};

use omega_calling_conventions::{
    CallPlan, CallSignature, CallingPolicy, EntryControl, HostCapability, HostOperation,
    HostOperationKey, IndirectPointerLocation, MachineRegister, MachineState, MachineStateSet,
    RegisterSet, SystemVEightbyteClass, ValueClass, ValueLocation, ValuePlacement, ValueShape,
    evaluate_call_plan, validate_call_plan,
};
use omega_core::arithmetic::ArithmeticDomain;
use omega_core::diagnostics::Diagnostic;
use omega_target_operations::{
    InstructionOperandLike, RuntimeStorageRegion, RuntimeValueOperandHandle,
    RuntimeValueOperandSource, StateGuardOperator,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64RelocationSiteKind {
    Absolute64,
    Relative32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64RelocationSite {
    pub operand_index: Option<usize>,
    pub byte_offset: usize,
    pub byte_width: usize,
    pub kind: X86_64RelocationSiteKind,
}

/// Bytes reserved by the fixed ordinary x86-64 frame. Saving eight registers
/// keeps the entry stack's modulo-16 alignment unchanged, so the existing
/// SysV and Microsoft x64 outbound-call reservations remain valid.
pub const FUNCTION_FRAME_BYTES: usize = 64;

pub fn function_enter_width() -> usize {
    12
}

/// Preserve the union of the SysV AMD64 and Microsoft x64 nonvolatile GPRs
/// used by generated Omega code: rbx, rbp, rsi, rdi, and r12-r15.
pub fn encode_function_enter_bytes() -> [u8; 12] {
    [
        0x53, // push rbx
        0x55, // push rbp
        0x56, // push rsi
        0x57, // push rdi
        0x41, 0x54, // push r12
        0x41, 0x55, // push r13
        0x41, 0x56, // push r14
        0x41, 0x57, // push r15
    ]
}

pub fn return_width() -> usize {
    13
}

pub fn encode_return_bytes() -> [u8; 13] {
    [
        0x41, 0x5f, // pop r15
        0x41, 0x5e, // pop r14
        0x41, 0x5d, // pop r13
        0x41, 0x5c, // pop r12
        0x5f, // pop rdi
        0x5e, // pop rsi
        0x5d, // pop rbp
        0x5b, // pop rbx
        0xc3, // ret
    ]
}

/// Register writes performed by the ordinary x86-64 function-entry sequence.
/// Pushes only update SP; the stored nonvolatile register values are reads.
pub fn function_enter_register_writes() -> RegisterSet {
    RegisterSet::default()
}

pub fn function_enter_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::StackPointer])
}

/// Exact state written while restoring the fixed frame and returning. The
/// explicit RSP identity is retained in addition to its stack-pointer class.
pub fn return_register_writes() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::X86Rbx,
        MachineRegister::X86Rsp,
        MachineRegister::X86Rbp,
        MachineRegister::X86Rsi,
        MachineRegister::X86Rdi,
        MachineRegister::X86R12,
        MachineRegister::X86R13,
        MachineRegister::X86R14,
        MachineRegister::X86R15,
    ])
}

pub fn return_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::InstructionPointer, MachineState::StackPointer])
}

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
pub const fn encode_memory_fence_bytes(kind: omega_core::inline_assembly::AsmFenceKind) -> [u8; 3] {
    use omega_core::inline_assembly::AsmFenceKind;
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
    kind: omega_core::inline_assembly::AsmInterruptControlKind,
) -> [u8; 1] {
    use omega_core::inline_assembly::AsmInterruptControlKind;
    match kind {
        AsmInterruptControlKind::Disable => [0xfa],
        AsmInterruptControlKind::Enable => [0xfb],
    }
}

pub const fn lidt_from_r10_width() -> usize {
    4
}

/// Deriver-only `lidt [r10]`: R10 points at the private packed 10-byte x86-64
/// descriptor produced for the exact content/ledger-bound installed table.
/// Source assembly cannot request this encoding or observe the pointer.
pub const fn encode_lidt_from_r10_bytes() -> [u8; 4] {
    [0x41, 0x0f, 0x01, 0x1a]
}

/// Materialize one plan-selected private pointer in R10. The pointer arrives
/// through a normalized boundary placement; it is never embedded as an
/// immediate or retained in source-visible operation data.
pub fn encode_private_pointer_to_r10_bytes(
    source: omega_calling_conventions::MachineRegister,
) -> Result<[u8; 3], Diagnostic> {
    use omega_calling_conventions::MachineRegister;

    let code = match source {
        MachineRegister::X86Rax => 0,
        MachineRegister::X86Rcx => 1,
        MachineRegister::X86Rdx => 2,
        MachineRegister::X86Rbx => 3,
        MachineRegister::X86Rsp => 4,
        MachineRegister::X86Rbp => 5,
        MachineRegister::X86Rsi => 6,
        MachineRegister::X86Rdi => 7,
        MachineRegister::X86R8 => 8,
        MachineRegister::X86R9 => 9,
        MachineRegister::X86R10 => 10,
        MachineRegister::X86R11 => 11,
        MachineRegister::X86R12 => 12,
        MachineRegister::X86R13 => 13,
        MachineRegister::X86R14 => 14,
        MachineRegister::X86R15 => 15,
        MachineRegister::X86Xmm(_)
        | MachineRegister::Aarch64X(_)
        | MachineRegister::Aarch64V(_) => {
            return Err(Diagnostic::error(format!(
                "generated x86 private pointer cannot arrive in {source:?}"
            )));
        }
    };
    // REX.W + R (r10 destination), plus B for a high source register;
    // `8b /r` reads the source r/m64 into r10.
    Ok([0x4c | u8::from(code >= 8), 0x8b, 0xd0 | (code & 7)])
}

pub fn generated_idt_load_width(
    pointer_register: omega_calling_conventions::MachineRegister,
) -> Result<usize, Diagnostic> {
    encode_private_pointer_to_r10_bytes(pointer_register)?;
    Ok(3 + lidt_from_r10_width())
}

pub fn encode_generated_idt_load_bytes(
    pointer_register: omega_calling_conventions::MachineRegister,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_private_pointer_to_r10_bytes(pointer_register)?.to_vec();
    bytes.extend(encode_lidt_from_r10_bytes());
    Ok(bytes)
}

/// Exact scratch footprint of the generated descriptor-address
/// materialization plus `lidt [r10]` sequence.
pub fn lidt_from_r10_clobbers() -> omega_calling_conventions::RegisterSet {
    omega_calling_conventions::RegisterSet::new([
        omega_calling_conventions::MachineRegister::X86R10,
    ])
}

/// Packed provider-private input consumed by the generated writer while R10
/// points at byte zero. The destination pointer is followed by a dense array
/// of u64 source values which the concrete provider must populate from the
/// sealed writer preparation.
pub const GENERATED_IDT_WRITER_DESTINATION_OFFSET: usize =
    omega_target_operations::GENERATED_IDT_WRITER_DESTINATION_OFFSET;
pub const GENERATED_IDT_WRITER_SOURCE_SLOTS_OFFSET: usize =
    omega_target_operations::GENERATED_IDT_WRITER_SOURCE_SLOTS_OFFSET;
pub const GENERATED_IDT_WRITER_SOURCE_SLOT_WIDTH: usize =
    omega_target_operations::GENERATED_IDT_WRITER_SOURCE_SLOT_WIDTH;

pub fn generated_idt_writer_context_width(source_slot_count: usize) -> Option<usize> {
    omega_target_operations::generated_idt_writer_context_byte_len(source_slot_count)
}

/// Exact registers written by the checked x86 IDT fragment writer. The
/// plan-selected private pointer is first copied into R10; R11 then holds the
/// destination, RAX the source fragment, RCX the destination container, and
/// RDX the masks.
pub fn generated_idt_writer_clobbers() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::X86Rax,
        MachineRegister::X86Rcx,
        MachineRegister::X86Rdx,
        MachineRegister::X86R10,
        MachineRegister::X86R11,
    ])
}

pub fn generated_idt_writer_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

pub fn generated_idt_writer_width(
    pointer_register: omega_calling_conventions::MachineRegister,
    byte_len: usize,
    little_endian: bool,
    context_abi: u64,
    source_slot_count: usize,
    steps: &[omega_target_operations::GeneratedIdtWriterStep],
) -> Result<usize, Diagnostic> {
    Ok(encode_generated_idt_writer_bytes(
        pointer_register,
        byte_len,
        little_endian,
        context_abi,
        source_slot_count,
        steps,
    )?
    .len())
}

/// Emit the complete direct-destination writer. The selected boundary register
/// supplies the packed private context and is copied into R10 before any
/// access. Every access uses a pinned disp32 encoding, and the generated
/// sequence revalidates all geometry before producing any bytes.
pub fn encode_generated_idt_writer_bytes(
    pointer_register: omega_calling_conventions::MachineRegister,
    byte_len: usize,
    little_endian: bool,
    context_abi: u64,
    source_slot_count: usize,
    steps: &[omega_target_operations::GeneratedIdtWriterStep],
) -> Result<Vec<u8>, Diagnostic> {
    validate_generated_idt_writer(
        byte_len,
        little_endian,
        context_abi,
        source_slot_count,
        steps,
    )?;

    let mut bytes = encode_private_pointer_to_r10_bytes(pointer_register)?.to_vec();
    bytes.extend([0x4d, 0x8b, 0x1a]); // mov r11, [r10]
    for step in steps {
        let source_offset = GENERATED_IDT_WRITER_SOURCE_SLOTS_OFFSET
            + step.source_slot * GENERATED_IDT_WRITER_SOURCE_SLOT_WIDTH;
        let source_displacement = i32::try_from(source_offset)
            .expect("generated writer validation bounds private source displacement");
        let destination_displacement = i32::try_from(step.container_byte_offset)
            .expect("generated writer validation bounds destination displacement");

        bytes.extend([0x49, 0x8b, 0x82]); // mov rax, [r10+disp32]
        bytes.extend(source_displacement.to_le_bytes());
        if step.source_lsb != 0 {
            bytes.extend([0x48, 0xc1, 0xe8, step.source_lsb as u8]); // shr rax, imm8
        }
        let fragment_mask = generated_idt_writer_low_mask(step.width);
        bytes.extend([0x48, 0xba]); // mov rdx, imm64
        bytes.extend(fragment_mask.to_le_bytes());
        bytes.extend([0x48, 0x21, 0xd0]); // and rax, rdx
        if step.destination_lsb != 0 {
            bytes.extend([0x48, 0xc1, 0xe0, step.destination_lsb as u8]); // shl rax, imm8
        }

        match step.container_width_bits {
            8 => bytes.extend([0x41, 0x0f, 0xb6, 0x8b]), // movzx ecx, byte [r11+disp32]
            16 => bytes.extend([0x41, 0x0f, 0xb7, 0x8b]), // movzx ecx, word [r11+disp32]
            32 => bytes.extend([0x41, 0x8b, 0x8b]),      // mov ecx, [r11+disp32]
            64 => bytes.extend([0x49, 0x8b, 0x8b]),      // mov rcx, [r11+disp32]
            _ => unreachable!("generated writer container width was validated"),
        }
        bytes.extend(destination_displacement.to_le_bytes());

        let destination_mask = fragment_mask << step.destination_lsb;
        bytes.extend([0x48, 0xba]); // mov rdx, imm64
        bytes.extend((!destination_mask).to_le_bytes());
        bytes.extend([0x48, 0x21, 0xd1]); // and rcx, rdx
        bytes.extend([0x48, 0x09, 0xc1]); // or rcx, rax

        match step.container_width_bits {
            8 => bytes.extend([0x41, 0x88, 0x8b]), // mov byte [r11+disp32], cl
            16 => bytes.extend([0x66, 0x41, 0x89, 0x8b]), // mov word [r11+disp32], cx
            32 => bytes.extend([0x41, 0x89, 0x8b]), // mov dword [r11+disp32], ecx
            64 => bytes.extend([0x49, 0x89, 0x8b]), // mov qword [r11+disp32], rcx
            _ => unreachable!("generated writer container width was validated"),
        }
        bytes.extend(destination_displacement.to_le_bytes());
    }
    Ok(bytes)
}

fn validate_generated_idt_writer(
    byte_len: usize,
    little_endian: bool,
    context_abi: u64,
    source_slot_count: usize,
    steps: &[omega_target_operations::GeneratedIdtWriterStep],
) -> Result<(), Diagnostic> {
    if context_abi != omega_target_operations::GENERATED_IDT_WRITER_CONTEXT_ABI_V1 {
        return Err(Diagnostic::error(format!(
            "generated IDT writer context ABI {context_abi:016x} is not the pinned IDTWRIT1 contract"
        )));
    }
    if !little_endian {
        return Err(Diagnostic::error(
            "generated x86 IDT writer requires little-endian containers",
        ));
    }
    if steps.is_empty() || source_slot_count == 0 {
        return Err(Diagnostic::error(
            "generated IDT writer requires at least one fragment and private source slot",
        ));
    }
    let context_width = generated_idt_writer_context_width(source_slot_count)
        .ok_or_else(|| Diagnostic::error("generated IDT writer private context size overflows"))?;
    if context_width > i32::MAX as usize {
        return Err(Diagnostic::error(
            "generated IDT writer private context exceeds disp32 addressing",
        ));
    }

    let mut used_slots = vec![false; source_slot_count];
    for step in steps {
        let Some(used) = used_slots.get_mut(step.source_slot) else {
            return Err(Diagnostic::error(format!(
                "generated IDT writer fragment names private source slot {}, but the context has {source_slot_count}",
                step.source_slot
            )));
        };
        *used = true;
        if !matches!(step.container_width_bits, 8 | 16 | 32 | 64) {
            return Err(Diagnostic::error(format!(
                "generated IDT writer has invalid {}-bit container",
                step.container_width_bits
            )));
        }
        if step.width == 0
            || step.width > 64
            || step
                .source_lsb
                .checked_add(step.width)
                .is_none_or(|end| end > 64)
            || step
                .destination_lsb
                .checked_add(step.width)
                .is_none_or(|end| end > step.container_width_bits)
        {
            return Err(Diagnostic::error(
                "generated IDT writer has an invalid source or destination bit range",
            ));
        }
        let container_bytes = u64::from(step.container_width_bits / 8);
        if step
            .container_byte_offset
            .checked_add(container_bytes)
            .is_none_or(|end| end > byte_len as u64)
        {
            return Err(Diagnostic::error(format!(
                "generated IDT writer fragment at byte {} lies outside its {byte_len}-byte destination",
                step.container_byte_offset
            )));
        }
        if step.container_byte_offset > i32::MAX as u64 {
            return Err(Diagnostic::error(
                "generated IDT writer destination offset exceeds disp32 addressing",
            ));
        }
    }
    if used_slots.iter().any(|used| !used) {
        return Err(Diagnostic::error(
            "generated IDT writer private source slots are not a dense exact set",
        ));
    }
    Ok(())
}

const fn generated_idt_writer_low_mask(width: u16) -> u64 {
    if width == 64 {
        u64::MAX
    } else {
        (1_u64 << width) - 1
    }
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

const fn control_register_modrm(register: omega_core::inline_assembly::AsmControlRegister) -> u8 {
    use omega_core::inline_assembly::AsmControlRegister;
    match register {
        AsmControlRegister::Cr0 => 0xc2,
        AsmControlRegister::Cr2 => 0xd2,
        AsmControlRegister::Cr3 => 0xda,
        AsmControlRegister::Cr4 => 0xe2,
    }
}

/// Read CR0/CR2/CR3/CR4 into R10, then store the exact u64 value to `dest`.
pub fn encode_control_register_read(
    register: omega_core::inline_assembly::AsmControlRegister,
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
    register: omega_core::inline_assembly::AsmControlRegister,
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

pub fn return_register_integer_write_width(
    register: omega_calling_conventions::MachineRegister,
    byte_size: usize,
) -> usize {
    let high_register = x86_gpr_number(register).is_some_and(|number| number >= 8);
    if byte_size == 8 {
        10
    } else {
        5 + usize::from(high_register)
    }
}

pub fn runtime_storage_copy_to_return_register_width(
    register: omega_calling_conventions::MachineRegister,
    byte_offset: usize,
    byte_size: usize,
) -> usize {
    // mov r15,imm64(region base, relocated) (10) + either a scalar XMM load
    // (9) or a GPR load (7; sign-extending 1/2-byte forms carry an 0F prefix,
    // 8).
    let _ = byte_offset;
    if matches!(
        register,
        omega_calling_conventions::MachineRegister::X86Xmm(_)
    ) {
        return 19;
    }
    let load_width = if matches!(byte_size, 1 | 2) { 8 } else { 7 };
    10 + load_width
}

/// Byte offset where the INDEX-region base's `mov r15, imm64` begins inside
/// `encode_runtime_machine_indexed_address_to_runtime_frame_write` (the
/// relocation machinery adds the +2 to reach its immediate).
pub const MACHINE_INDEXED_ADDRESS_INDEX_BASE_IMM_OFFSET: usize = 10;

fn x86_gpr_number(register: omega_calling_conventions::MachineRegister) -> Option<u8> {
    Some(match register {
        omega_calling_conventions::MachineRegister::X86Rax => 0,
        omega_calling_conventions::MachineRegister::X86Rcx => 1,
        omega_calling_conventions::MachineRegister::X86Rdx => 2,
        omega_calling_conventions::MachineRegister::X86Rbx => 3,
        omega_calling_conventions::MachineRegister::X86Rsp => 4,
        omega_calling_conventions::MachineRegister::X86Rbp => 5,
        omega_calling_conventions::MachineRegister::X86Rsi => 6,
        omega_calling_conventions::MachineRegister::X86Rdi => 7,
        omega_calling_conventions::MachineRegister::X86R8 => 8,
        omega_calling_conventions::MachineRegister::X86R9 => 9,
        omega_calling_conventions::MachineRegister::X86R10 => 10,
        omega_calling_conventions::MachineRegister::X86R11 => 11,
        omega_calling_conventions::MachineRegister::X86R12 => 12,
        omega_calling_conventions::MachineRegister::X86R13 => 13,
        omega_calling_conventions::MachineRegister::X86R14 => 14,
        omega_calling_conventions::MachineRegister::X86R15 => 15,
        _ => return None,
    })
}

pub fn entry_argument_register_write_width(
    register: omega_calling_conventions::MachineRegister,
    byte_size: usize,
) -> usize {
    // mov r15,imm64(frame base, relocated at +2) (10), followed by either a
    // GPR store (7; 16-bit adds one prefix) or movss/movsd from XMM (9).
    match register {
        omega_calling_conventions::MachineRegister::X86Xmm(_) => 19,
        _ => 17 + usize::from(byte_size == 2),
    }
}

/// Registers overwritten by one inbound register-to-frame copy. Source
/// registers are reads, not clobbers, and therefore are deliberately absent.
pub fn entry_argument_register_write_clobbers() -> RegisterSet {
    RegisterSet::new([MachineRegister::X86R15])
}

/// The ENTRY PROLOGUE's inbound unmarshal: store the exact GPR selected by the
/// normalized call plan into the entry parameter's runtime-frame slot. Runs
/// before anything else at the entry because argument registers are volatile.
pub fn encode_entry_argument_register_write_bytes(
    register: omega_calling_conventions::MachineRegister,
    byte_offset: usize,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if register == MachineRegister::X86R15 {
        return Err(Diagnostic::error(
            "x86-64 entry prologue cannot read an argument from its r15 frame-base scratch",
        ));
    }
    if let omega_calling_conventions::MachineRegister::X86Xmm(register_index) = register {
        if register_index > 15 || !matches!(byte_size, 4 | 8) {
            return Err(Diagnostic::error(format!(
                "x86-64 entry prologue cannot store {byte_size} bytes from XMM{register_index}"
            )));
        }
        let mut bytes =
            Vec::with_capacity(entry_argument_register_write_width(register, byte_size));
        append_mov_r15_imm64(&mut bytes, 0);
        // movss/movsd [r15+disp32], xmmN. The mandatory F3/F2 prefix precedes
        // REX; B names r15 and R extends XMM8..15.
        bytes.push(if byte_size == 4 { 0xf3 } else { 0xf2 });
        bytes.push(0x41 | if register_index >= 8 { 0x04 } else { 0 });
        bytes.extend([0x0f, 0x11, 0x87 | ((register_index & 7) << 3)]);
        bytes.extend(disp32(byte_offset)?.to_le_bytes());
        debug_assert_eq!(
            bytes.len(),
            entry_argument_register_write_width(register, byte_size)
        );
        return Ok(bytes);
    }
    if !matches!(byte_size, 1 | 2 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "x86-64 entry prologue cannot store a {byte_size}-byte register value"
        )));
    }
    let mut bytes = Vec::with_capacity(entry_argument_register_write_width(register, byte_size));
    append_mov_r15_imm64(&mut bytes, 0); // relocated to the runtime-frame region base
    let register_number = x86_gpr_number(register).ok_or_else(|| {
        Diagnostic::error(format!(
            "x86-64 entry prologue cannot store non-GPR plan location {register:?}"
        ))
    })?;
    if byte_size == 2 {
        bytes.push(0x66);
    }
    let rex =
        (if byte_size == 8 { 0x49 } else { 0x41 }) | if register_number >= 8 { 0x04 } else { 0 };
    let modrm = 0x87 | ((register_number & 7) << 3);
    bytes.extend([rex, if byte_size == 1 { 0x88 } else { 0x89 }, modrm]);
    bytes.extend(disp32(byte_offset)?.to_le_bytes());
    debug_assert_eq!(
        bytes.len(),
        entry_argument_register_write_width(register, byte_size)
    );
    Ok(bytes)
}

/// Width of one incoming stack-fragment copy. The runtime-frame base
/// materialization is 10 bytes; byte/dword/qword loads and stores total 15,
/// while the operand-size prefixes make the word form two bytes longer.
pub fn entry_stack_argument_write_width(byte_size: usize) -> usize {
    if byte_size == 2 { 27 } else { 25 }
}

/// Registers overwritten while copying one incoming stack fragment. `rsp` is
/// only used as an address base and is not clobbered by this fragment.
pub fn entry_stack_argument_write_clobbers() -> RegisterSet {
    RegisterSet::new([MachineRegister::X86R10, MachineRegister::X86R15])
}

/// Copy an incoming x86-64 stack argument into runtime-frame storage. Calling
/// plans measure `stack_byte_offset` from the ABI stack-argument area, so the
/// source is beyond the fixed saved-register frame and return address.
pub fn encode_entry_stack_argument_write_bytes(
    stack_byte_offset: u32,
    byte_offset: usize,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 2 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "x86-64 entry prologue cannot copy a {byte_size}-byte stack value"
        )));
    }
    let source_offset = stack_byte_offset
        .checked_add((FUNCTION_FRAME_BYTES + 8) as u32)
        .ok_or_else(|| Diagnostic::error("x86-64 incoming stack offset overflow"))?;
    let source_offset = i32::try_from(source_offset)
        .map_err(|_| Diagnostic::error("x86-64 incoming stack offset exceeds disp32"))?;
    let mut bytes = Vec::with_capacity(entry_stack_argument_write_width(byte_size));
    append_mov_r15_imm64(&mut bytes, 0); // runtime-frame base, relocated

    // mov r10{b,w,d,q}, [rsp + disp32]
    if byte_size == 2 {
        bytes.push(0x66);
    }
    bytes.extend([
        if byte_size == 8 { 0x4c } else { 0x44 },
        if byte_size == 1 { 0x8a } else { 0x8b },
        0x94,
        0x24,
    ]);
    bytes.extend(source_offset.to_le_bytes());

    // mov [r15 + disp32], r10{b,w,d,q}
    if byte_size == 2 {
        bytes.push(0x66);
    }
    bytes.extend([
        if byte_size == 8 { 0x4d } else { 0x45 },
        if byte_size == 1 { 0x88 } else { 0x89 },
        0x97,
    ]);
    bytes.extend(disp32(byte_offset)?.to_le_bytes());
    debug_assert_eq!(bytes.len(), entry_stack_argument_write_width(byte_size));
    Ok(bytes)
}

pub fn entry_indirect_argument_write_width(
    pointer: IndirectPointerLocation,
    byte_size: usize,
) -> usize {
    let pointer_setup = match pointer {
        IndirectPointerLocation::Register(_) => 3,
        IndirectPointerLocation::Stack { .. } => 8,
    };
    let mut width = pointer_setup + 10;
    let mut copied = 0usize;
    while copied < byte_size {
        let fragment = [8, 4, 2, 1]
            .into_iter()
            .find(|fragment| byte_size - copied >= *fragment)
            .expect("indirect entry copy has bytes remaining");
        width += 14 + usize::from(fragment == 2) * 2;
        copied += fragment;
    }
    width
}

pub fn entry_indirect_argument_frame_base_offset(pointer: IndirectPointerLocation) -> usize {
    match pointer {
        IndirectPointerLocation::Register(_) => 3,
        IndirectPointerLocation::Stack { .. } => 8,
    }
}

/// Registers overwritten while copying one indirectly passed aggregate.
pub fn entry_indirect_argument_write_clobbers() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::X86R10,
        MachineRegister::X86R11,
        MachineRegister::X86R15,
    ])
}

/// Copy one indirectly passed Microsoft x64 aggregate into its runtime-frame
/// slot. The ABI pointer occupies the argument's positional GPR or stack slot;
/// r11 preserves it while r10 moves one naturally sized fragment at a time.
pub fn encode_entry_indirect_argument_write_bytes(
    pointer: IndirectPointerLocation,
    byte_offset: usize,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if matches!(byte_size, 0 | 1 | 2 | 4 | 8) {
        return Err(Diagnostic::error(
            "Microsoft x64 indirect entry aggregate must have a nondirect record width",
        ));
    }
    let mut bytes = Vec::with_capacity(entry_indirect_argument_write_width(pointer, byte_size));
    match pointer {
        IndirectPointerLocation::Register(
            register @ (MachineRegister::X86Rcx
            | MachineRegister::X86Rdx
            | MachineRegister::X86R8
            | MachineRegister::X86R9),
        ) => {
            let register_number = x86_gpr_number(register).expect("matched x86 GPR");
            bytes.extend([
                0x4c | u8::from(register_number >= 8),
                0x8b,
                0xd8 | (register_number & 7),
            ]); // mov r11, selected pointer register
        }
        IndirectPointerLocation::Register(register) => {
            return Err(Diagnostic::error(format!(
                "Microsoft x64 indirect entry aggregate uses unsupported pointer register {register:?}"
            )));
        }
        IndirectPointerLocation::Stack {
            stack_byte_offset, ..
        } => {
            let source_offset = stack_byte_offset
                .checked_add((FUNCTION_FRAME_BYTES + 8) as u32)
                .ok_or_else(|| Diagnostic::error("x86-64 incoming pointer offset overflow"))?;
            let source_offset = i32::try_from(source_offset)
                .map_err(|_| Diagnostic::error("x86-64 incoming pointer offset exceeds disp32"))?;
            bytes.extend([0x4c, 0x8b, 0x9c, 0x24]); // mov r11, [rsp+disp32]
            bytes.extend(source_offset.to_le_bytes());
        }
    }
    append_mov_r15_imm64(&mut bytes, 0); // runtime-frame base, relocated
    let mut copied = 0usize;
    while copied < byte_size {
        let fragment = [8, 4, 2, 1]
            .into_iter()
            .find(|fragment| byte_size - copied >= *fragment)
            .expect("indirect entry copy has bytes remaining");
        if fragment == 2 {
            bytes.push(0x66);
        }
        bytes.extend([
            if fragment == 8 { 0x4d } else { 0x45 },
            if fragment == 1 { 0x8a } else { 0x8b },
            0x93,
        ]); // mov r10{b,w,d,q}, [r11+disp32]
        bytes.extend(disp32(copied)?.to_le_bytes());
        if fragment == 2 {
            bytes.push(0x66);
        }
        bytes.extend([
            if fragment == 8 { 0x4d } else { 0x45 },
            if fragment == 1 { 0x88 } else { 0x89 },
            0x97,
        ]); // mov [r15+disp32], r10{b,w,d,q}
        bytes.extend(
            disp32(byte_offset.checked_add(copied).ok_or_else(|| {
                Diagnostic::error("x86-64 indirect entry destination offset overflow")
            })?)?
            .to_le_bytes(),
        );
        copied += fragment;
    }
    debug_assert_eq!(
        bytes.len(),
        entry_indirect_argument_write_width(pointer, byte_size)
    );
    Ok(bytes)
}

#[cfg(test)]
mod entry_argument_register_tests {
    use super::*;
    use omega_calling_conventions::MachineRegister;

    #[test]
    fn ordinary_frame_preserves_generated_nonvolatile_gprs_and_alignment() {
        assert_eq!(FUNCTION_FRAME_BYTES, 64);
        assert_eq!(encode_function_enter_bytes().len(), function_enter_width());
        assert_eq!(encode_return_bytes().len(), return_width());
        assert_eq!(
            encode_function_enter_bytes(),
            [
                0x53, 0x55, 0x56, 0x57, 0x41, 0x54, 0x41, 0x55, 0x41, 0x56, 0x41, 0x57
            ]
        );
        assert_eq!(
            encode_return_bytes(),
            [
                0x41, 0x5f, 0x41, 0x5e, 0x41, 0x5d, 0x41, 0x5c, 0x5f, 0x5e, 0x5d, 0x5b, 0xc3
            ]
        );
    }

    #[test]
    fn scalar_float_entry_arguments_store_from_the_selected_xmm_register() {
        let bytes = encode_entry_argument_register_write_bytes(MachineRegister::X86Xmm(8), 8, 8)
            .expect("movsd entry store");
        assert_eq!(bytes.len(), 19);
        assert_eq!(&bytes[10..15], &[0xf2, 0x45, 0x0f, 0x11, 0x87]);
        assert_eq!(&bytes[15..19], &8i32.to_le_bytes());
    }

    #[test]
    fn scalar_float_entry_arguments_reject_non_scalar_widths() {
        let error = encode_entry_argument_register_write_bytes(MachineRegister::X86Xmm(0), 0, 16)
            .expect_err("unclassified vector argument must reject");
        assert!(error.message.contains("cannot store 16 bytes"));
    }

    #[test]
    fn entry_argument_cannot_alias_the_frame_base_scratch() {
        let error = encode_entry_argument_register_write_bytes(MachineRegister::X86R15, 0, 8)
            .expect_err("r15 input would be destroyed while materializing the frame base");
        assert!(error.message.contains("r15 frame-base scratch"));
    }

    #[test]
    fn ms_x64_fifth_argument_loads_after_return_address_and_shadow_space() {
        let bytes =
            encode_entry_stack_argument_write_bytes(32, 24, 8).expect("incoming stack copy");
        assert_eq!(bytes.len(), 25);
        assert_eq!(&bytes[10..18], &[0x4c, 0x8b, 0x94, 0x24, 104, 0, 0, 0]);
        assert_eq!(&bytes[18..25], &[0x4d, 0x89, 0x97, 24, 0, 0, 0]);
    }

    #[test]
    fn indirect_entry_aggregate_copies_from_a_pointer_register() {
        let pointer = IndirectPointerLocation::Register(MachineRegister::X86Rcx);
        let bytes = encode_entry_indirect_argument_write_bytes(pointer, 64, 16)
            .expect("two-fragment indirect entry copy");

        assert_eq!(
            bytes.len(),
            entry_indirect_argument_write_width(pointer, 16)
        );
        assert_eq!(entry_indirect_argument_frame_base_offset(pointer), 3);
        assert_eq!(&bytes[..3], &[0x4c, 0x8b, 0xd9]);
        assert_eq!(&bytes[13..20], &[0x4d, 0x8b, 0x93, 0, 0, 0, 0]);
        assert_eq!(&bytes[20..27], &[0x4d, 0x89, 0x97, 64, 0, 0, 0]);
        assert_eq!(&bytes[27..34], &[0x4d, 0x8b, 0x93, 8, 0, 0, 0]);
        assert_eq!(&bytes[34..41], &[0x4d, 0x89, 0x97, 72, 0, 0, 0]);
    }

    #[test]
    fn indirect_entry_aggregate_loads_a_stack_passed_pointer() {
        let pointer = IndirectPointerLocation::Stack {
            stack_byte_offset: 32,
            alignment: 8,
        };
        let bytes = encode_entry_indirect_argument_write_bytes(pointer, 64, 16)
            .expect("stack-pointer indirect entry copy");

        assert_eq!(
            bytes.len(),
            entry_indirect_argument_write_width(pointer, 16)
        );
        assert_eq!(entry_indirect_argument_frame_base_offset(pointer), 8);
        assert_eq!(&bytes[..8], &[0x4c, 0x8b, 0x9c, 0x24, 104, 0, 0, 0]);
        assert_eq!(&bytes[8..10], &[0x49, 0xbf]);
    }
}

pub fn entry_arguments_slice_descriptor_write_width() -> usize {
    // mov r15,imm64(frame base, relocated at +2) (10) + lea rax,[r15+spill] (7)
    // + mov [r15+desc],rax (7) + mov qword [r15+desc+8],imm32(len) (11).
    35
}

/// Exact register footprint of the bytes-handoff descriptor encoder below.
/// Keep this beside the implementation so certificate derivation cannot drift
/// from its fixed frame-base and descriptor-address scratch choices.
pub fn entry_arguments_slice_descriptor_write_clobbers() -> RegisterSet {
    RegisterSet::new([MachineRegister::X86Rax, MachineRegister::X86R15])
}

/// The bytes-handoff half of the entry prologue: bind `args: &[u8]` as a view
/// over the entry-argument spill -- write the slice descriptor
/// {ptr @ desc+0 = frame+spill_offset, len @ desc+8 = byte_length}.
pub fn encode_entry_arguments_slice_descriptor_write_bytes(
    descriptor_offset: usize,
    spill_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(entry_arguments_slice_descriptor_write_width());
    append_mov_r15_imm64(&mut bytes, 0); // relocated to the runtime-frame region base
    bytes.extend([0x49, 0x8d, 0x87]); // lea rax, [r15 + disp32]
    bytes.extend(disp32(spill_offset)?.to_le_bytes());
    bytes.extend([0x49, 0x89, 0x87]); // mov [r15 + disp32], rax
    bytes.extend(disp32(descriptor_offset)?.to_le_bytes());
    bytes.extend([0x49, 0xc7, 0x87]); // mov qword [r15 + disp32], imm32
    bytes.extend(disp32(descriptor_offset + 8)?.to_le_bytes());
    let length = i32::try_from(byte_length)
        .map_err(|_| Diagnostic::error("entry-argument slice length exceeds an imm32"))?;
    bytes.extend(length.to_le_bytes());
    debug_assert_eq!(bytes.len(), entry_arguments_slice_descriptor_write_width());
    Ok(bytes)
}

/// Relocation imm offset (pre-`+2`) of the frame base loaded for the target slot
/// store in `encode_runtime_frame_base_indexed_address_to_runtime_frame_write`.
pub const FRAME_BASE_INDEXED_ADDRESS_TARGET_FRAME_IMM_OFFSET: usize = 34;

pub const RUNTIME_TEXT_STORED_PLACE_APPEND_TARGET_IMM_OFFSET: usize = 10;
pub const RUNTIME_TEXT_STORED_PLACE_APPEND_SOURCE_IMM_OFFSET: usize = 33;
/// Like the non-pointee source offset, but the pointee variant inserts one extra
/// `mov r15, [r15+disp32]` (7 bytes) to dereference the runtime pointer before the
/// source-region `mov rcx, imm64`, pushing the source immediate from 33 to 40.
pub const RUNTIME_TEXT_STORED_PLACE_APPEND_POINTEE_SOURCE_IMM_OFFSET: usize = 40;

pub fn runtime_text_stored_place_append_width() -> usize {
    86
}

/// Appends a stored source string (a `{ptr,len}` descriptor in `source_region`)
/// to the end of a target string that lives in a fixed output `buffer`, updating
/// the target descriptor. r14=buffer base, r15=target region base, the source
/// region base is loaded into rcx. The copy itself is a `rep movsb` (rsi/rdi are
/// preserved around it). `buffer_offset` is unused (the append point is the
/// target's current length).
pub fn encode_runtime_text_stored_place_append(
    source_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_text_stored_place_append_width());
    append_mov_r14_imm64(&mut bytes, 0); // buffer base (reloc @ +2)
    append_mov_r15_imm64(&mut bytes, 0); // target region base (reloc @ +12)
    append_load_r11_from_r15(&mut bytes, target_offset + 8)?; // r11 = current length
    append_mov_r10_r14(&mut bytes); // r10 = buffer base
    append_add_r10_r11(&mut bytes); // r10 = dest = buffer + current length
    debug_assert_eq!(
        bytes.len(),
        RUNTIME_TEXT_STORED_PLACE_APPEND_SOURCE_IMM_OFFSET
    );
    append_mov_rcx_imm64(&mut bytes, 0); // source region base (reloc @ +2)
    append_load_rax_from_rcx(&mut bytes, source_offset)?; // rax = source pointer
    append_load_rcx_from_rcx(&mut bytes, source_offset + 8)?; // rcx = source length
    append_add_r11_rcx(&mut bytes); // r11 = new length = current + source
    append_store_r14_to_r15(&mut bytes, target_offset)?; // descriptor.ptr = buffer
    append_store_r11_to_r15(&mut bytes, target_offset + 8)?; // descriptor.len = new length
    append_push_rsi_rdi(&mut bytes);
    append_mov_rsi_rax(&mut bytes); // rsi = source pointer
    append_mov_rdi_r10(&mut bytes); // rdi = dest
    append_rep_movsb(&mut bytes); // copy rcx bytes
    append_pop_rdi_rsi(&mut bytes);
    debug_assert_eq!(bytes.len(), runtime_text_stored_place_append_width());
    Ok(bytes)
}

pub fn runtime_text_stored_place_append_to_runtime_pointee_width() -> usize {
    93
}

/// Appends a stored source string to a target string whose `{ptr,len}` descriptor
/// is reached through a RUNTIME pointer: the descriptor lives at
/// `*(frame + pointer_byte_offset) + field_byte_offset`. Mirrors
/// `encode_runtime_text_stored_place_append`, but loads the descriptor base by
/// dereferencing the runtime pointer (one extra `mov r15,[r15+disp32]`) instead of
/// using a relocated target-region base. r14=materialized buffer base, r15=descriptor
/// address, rcx=source region base; the copy is a `rep movsb` (rsi/rdi preserved).
/// The descriptor's `ptr` is overwritten to the buffer base and `len` grows by the
/// source length -- so a prior stale `ptr` (e.g. from WriteRuntimePointeeString) is
/// corrected here.
pub fn encode_runtime_text_stored_place_append_to_runtime_pointee(
    source_offset: usize,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_text_stored_place_append_to_runtime_pointee_width());
    append_mov_r14_imm64(&mut bytes, 0); // buffer base (reloc @ instruction start)
    append_mov_r15_imm64(&mut bytes, 0); // runtime-frame base (reloc @ +10 == TARGET offset)
    append_load_r15_from_r15(&mut bytes, pointer_byte_offset)?; // r15 = runtime pointer
    append_load_r11_from_r15(&mut bytes, field_byte_offset + 8)?; // r11 = current length
    append_mov_r10_r14(&mut bytes); // r10 = buffer base
    append_add_r10_r11(&mut bytes); // r10 = dest = buffer + current length
    debug_assert_eq!(
        bytes.len(),
        RUNTIME_TEXT_STORED_PLACE_APPEND_POINTEE_SOURCE_IMM_OFFSET
    );
    append_mov_rcx_imm64(&mut bytes, 0); // source region base (reloc @ +40)
    append_load_rax_from_rcx(&mut bytes, source_offset)?; // rax = source pointer
    append_load_rcx_from_rcx(&mut bytes, source_offset + 8)?; // rcx = source length
    append_add_r11_rcx(&mut bytes); // r11 = new length = current + source
    append_store_r14_to_r15(&mut bytes, field_byte_offset)?; // descriptor.ptr = buffer
    append_store_r11_to_r15(&mut bytes, field_byte_offset + 8)?; // descriptor.len = new length
    append_push_rsi_rdi(&mut bytes);
    append_mov_rsi_rax(&mut bytes); // rsi = source pointer
    append_mov_rdi_r10(&mut bytes); // rdi = dest
    append_rep_movsb(&mut bytes); // copy rcx bytes
    append_pop_rdi_rsi(&mut bytes);
    debug_assert_eq!(
        bytes.len(),
        runtime_text_stored_place_append_to_runtime_pointee_width()
    );
    Ok(bytes)
}

pub const RUNTIME_TEXT_STORED_SUFFIX_APPEND_SOURCE_IMM_OFFSET: usize = 10;
pub const RUNTIME_TEXT_STORED_SUFFIX_APPEND_TARGET_IMM_OFFSET: usize = 59;

pub fn runtime_text_stored_suffix_append_width() -> usize {
    90
}

/// Writes a stored source string into `buffer + buffer_offset` and sets the
/// target descriptor to `{ buffer, source_len + length_delta }`. Used to build a
/// string whose first `length_delta` bytes are an already-present prefix.
pub fn encode_runtime_text_stored_suffix_append(
    buffer_offset: usize,
    source_offset: usize,
    target_offset: usize,
    length_delta: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_text_stored_suffix_append_width());
    append_mov_r14_imm64(&mut bytes, 0); // buffer base (reloc @ +2)
    append_mov_rcx_imm64(&mut bytes, 0); // source region base (reloc @ +12)
    append_load_rax_from_rcx(&mut bytes, source_offset)?; // rax = source pointer
    append_load_rcx_from_rcx(&mut bytes, source_offset + 8)?; // rcx = source length
    append_mov_r11_rcx(&mut bytes); // r11 = saved source length
    append_mov_r10_r14(&mut bytes); // r10 = buffer base
    append_add_r10_imm32(&mut bytes, buffer_offset)?; // r10 = dest = buffer + buffer_offset
    append_push_rsi_rdi(&mut bytes);
    append_mov_rsi_rax(&mut bytes); // rsi = source pointer
    append_mov_rdi_r10(&mut bytes); // rdi = dest
    append_rep_movsb(&mut bytes); // copy rcx bytes
    append_pop_rdi_rsi(&mut bytes);
    debug_assert_eq!(
        bytes.len(),
        RUNTIME_TEXT_STORED_SUFFIX_APPEND_TARGET_IMM_OFFSET
    );
    append_mov_r15_imm64(&mut bytes, 0); // target region base (reloc @ +2)
    append_store_r14_to_r15(&mut bytes, target_offset)?; // descriptor.ptr = buffer
    append_add_r11_imm32(&mut bytes, length_delta)?; // r11 = source_len + length_delta
    append_store_r11_to_r15(&mut bytes, target_offset + 8)?; // descriptor.len
    debug_assert_eq!(bytes.len(), runtime_text_stored_suffix_append_width());
    Ok(bytes)
}

/// Relocation imm offset (pre-`+2`) of the TARGET region base `mov` in the
/// fixed-indexed slice-element copy (the materializer's canonical shape:
/// source base mov (10) + descriptor deref (7)).
pub const FRAME_FIXED_INDEXED_COPY_TARGET_IMM_OFFSET: usize = 17;

/// Relocation imm offset (pre-`+2`) of the TARGET region base `mov` in the
/// runtime-indexed slice-element copy (the materializer's canonical shape:
/// frame base (10) + index load (7) + imul (7) + descriptor deref (7) +
/// add (3)).
pub const FRAME_INDEXED_COPY_TARGET_IMM_OFFSET: usize = 34;

pub fn encode_return_register_integer_write_bytes(
    register: omega_calling_conventions::MachineRegister,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 2 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "X86_64 MVP encoder cannot write {byte_size}-byte return integers yet"
        )));
    }
    let register_number = x86_gpr_number(register).ok_or_else(|| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot use {register:?} as an integer result register"
        ))
    })?;
    let mut bytes = Vec::with_capacity(return_register_integer_write_width(register, byte_size));
    if byte_size == 8 {
        bytes.extend([
            0x48 | u8::from(register_number >= 8),
            0xb8 + (register_number & 7),
        ]);
        bytes.extend(value.to_le_bytes());
    } else {
        let value = i32::try_from(value).map_err(|_| {
            Diagnostic::error(format!(
                "X86_64 MVP encoder cannot write return integer `{value}` yet"
            ))
        })?;
        if register_number >= 8 {
            bytes.push(0x41);
        }
        bytes.push(0xb8 + (register_number & 7));
        bytes.extend(value.to_le_bytes());
    }
    debug_assert_eq!(
        bytes.len(),
        return_register_integer_write_width(register, byte_size)
    );
    Ok(bytes)
}

/// Exact register footprint of immediate result materialization.
pub fn return_register_integer_write_clobbers(register: MachineRegister) -> RegisterSet {
    RegisterSet::new([register])
}

/// Load a runtime-storage scalar into the plan-selected integer result register so a
/// NON-CONSTANT terminal value (a local read, a field read-back) becomes the
/// process exit code. The `mov r15, imm64=0` (imm at instruction start + 2) is
/// relocated to the storage region's data symbol by the relocation planner,
/// exactly like a dispatch guard's storage load. Narrow operands use the
/// sign-extending movsx forms so a negative i8/i16 terminal survives the
/// widening read.
pub fn encode_runtime_storage_copy_to_return_register_bytes(
    register: omega_calling_conventions::MachineRegister,
    byte_offset: usize,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_copy_to_return_register_width(
        register,
        byte_offset,
        byte_size,
    ));
    append_mov_r15_imm64(&mut bytes, 0);
    let displacement = disp32(byte_offset)?;
    if let omega_calling_conventions::MachineRegister::X86Xmm(register_index) = register {
        if register_index > 15 || !matches!(byte_size, 4 | 8) {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot copy {byte_size} bytes into XMM{register_index}"
            )));
        }
        bytes.push(if byte_size == 4 { 0xf3 } else { 0xf2 });
        bytes.push(0x41 | if register_index >= 8 { 0x04 } else { 0 });
        bytes.extend([0x0f, 0x10, 0x87 | ((register_index & 7) << 3)]);
        bytes.extend(displacement.to_le_bytes());
        debug_assert_eq!(
            bytes.len(),
            runtime_storage_copy_to_return_register_width(register, byte_offset, byte_size)
        );
        return Ok(bytes);
    }
    let register_number = x86_gpr_number(register).ok_or_else(|| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot use {register:?} as an integer result register"
        ))
    })?;
    let modrm = 0x87 | ((register_number & 7) << 3);
    let rex_r = if register_number >= 8 { 0x04 } else { 0 };
    match byte_size {
        1 => bytes.extend([0x41 | rex_r, 0x0f, 0xbe, modrm]),
        2 => bytes.extend([0x41 | rex_r, 0x0f, 0xbf, modrm]),
        4 => bytes.extend([0x41 | rex_r, 0x8b, modrm]),
        8 => bytes.extend([0x49 | rex_r, 0x8b, modrm]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot copy {byte_size}-byte terminal values to the return register yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    debug_assert_eq!(
        bytes.len(),
        runtime_storage_copy_to_return_register_width(register, byte_offset, byte_size)
    );
    Ok(bytes)
}

/// Exact register footprint of the runtime-frame result load above.
pub fn runtime_storage_copy_to_return_register_clobbers(register: MachineRegister) -> RegisterSet {
    RegisterSet::new([register, MachineRegister::X86R15])
}

#[cfg(test)]
mod result_register_tests {
    use super::*;
    use omega_calling_conventions::MachineRegister;

    #[test]
    fn constant_result_uses_the_plan_selected_high_gpr() {
        let bytes = encode_return_register_integer_write_bytes(MachineRegister::X86R9, 4, 7)
            .expect("r9d result write");
        assert_eq!(bytes, [0x41, 0xb9, 7, 0, 0, 0]);
    }

    #[test]
    fn constant_result_accepts_the_normalized_u16_width() {
        let bytes = encode_return_register_integer_write_bytes(MachineRegister::X86Rax, 2, 7)
            .expect("ax result represented through eax");
        assert_eq!(bytes, [0xb8, 7, 0, 0, 0]);
    }

    #[test]
    fn runtime_result_load_uses_the_plan_selected_high_gpr() {
        let bytes =
            encode_runtime_storage_copy_to_return_register_bytes(MachineRegister::X86R10, 16, 4)
                .expect("r10d result load");
        assert_eq!(&bytes[10..17], &[0x45, 0x8b, 0x97, 16, 0, 0, 0]);
    }

    #[test]
    fn runtime_result_load_uses_the_plan_selected_xmm_register() {
        let bytes =
            encode_runtime_storage_copy_to_return_register_bytes(MachineRegister::X86Xmm(2), 24, 8)
                .expect("xmm2 result load");
        assert_eq!(&bytes[10..19], &[0xf2, 0x41, 0x0f, 0x10, 0x97, 24, 0, 0, 0]);
    }
}

pub fn dispatch_loop_enter_width() -> usize {
    6
}

pub fn dispatch_case_enter_width() -> usize {
    13
}

pub fn dispatch_state_write_width() -> usize {
    11
}

pub fn dispatch_case_leave_width() -> usize {
    5
}

pub fn encode_dispatch_loop_enter_bytes(entry_dispatch_index: u32) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(dispatch_loop_enter_width());
    append_mov_r12d_imm32(&mut bytes, entry_dispatch_index)?;
    Ok(bytes)
}

pub fn encode_dispatch_case_enter_bytes(
    dispatch_index: u32,
    skip_byte_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(dispatch_case_enter_width());
    append_cmp_r12d_imm32(&mut bytes, dispatch_index)?;
    append_jcc_rel32(&mut bytes, 0x85, skip_byte_distance - 9)?; // jne
    Ok(bytes)
}

pub fn encode_dispatch_state_write_bytes(
    dispatch_index: u32,
    case_leave_byte_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(dispatch_state_write_width());
    append_mov_r12d_imm32(&mut bytes, dispatch_index)?;
    append_jmp_rel32(&mut bytes, case_leave_byte_distance - 7)?;
    Ok(bytes)
}

pub fn encode_dispatch_case_leave_bytes(loop_byte_distance: isize) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(dispatch_case_leave_width());
    append_jmp_rel32(&mut bytes, loop_byte_distance - 5)?;
    Ok(bytes)
}

pub fn dispatch_loop_enter_register_writes() -> RegisterSet {
    RegisterSet::new([MachineRegister::X86R12])
}

pub fn dispatch_case_enter_register_writes() -> RegisterSet {
    RegisterSet::default()
}

pub fn dispatch_case_enter_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

pub fn dispatch_state_write_register_writes() -> RegisterSet {
    RegisterSet::new([MachineRegister::X86R12])
}

pub fn dispatch_case_leave_register_writes() -> RegisterSet {
    RegisterSet::default()
}

pub fn dispatch_guard_compare_static_width(is_float: bool, byte_size: usize) -> usize {
    // mov r15, imm64 (10) + load r10, [r15+disp32] (7; 8 for the 0x66-prefixed
    // 2-byte form) + mov r11, imm64 (10) + compare + jcc rel32 (6). Integer
    // compare is `cmp r10,r11` (3; 4 with the 0x66 prefix); float is
    // movq/movd + movq/movd + ucomisd/ucomiss.
    let load_width = if !is_float && byte_size == 2 { 8 } else { 7 };
    // Floats prepend a 6-byte `jp` parity branch before the failure jcc (NaN routing).
    let float_parity_branch = if is_float { 6 } else { 0 };
    10 + load_width
        + 10
        + runtime_float_or_integer_compare_width(is_float, byte_size)
        + 6
        + float_parity_branch
}

fn runtime_float_or_integer_compare_width(is_float: bool, byte_size: usize) -> usize {
    if is_float {
        // f64: movq(5)+movq(5)+ucomisd(4). f32: movd(5)+movd(5)+ucomiss(3) — the
        // single-precision SSE compare drops the 0x66 prefix, so it is 1 byte shorter.
        if byte_size == 4 { 13 } else { 14 }
    } else if byte_size == 2 {
        // 16-bit `cmp r10w,r11w` carries the 0x66 operand-size prefix.
        4
    } else {
        3
    }
}

/// Compare the bits already in r10 (left) and r11 (right) as `byte_size`-wide IEEE
/// floats via the SSE unit. For an 8-byte operand: `movq` into xmm0/xmm1 + `ucomisd`
/// (double precision). For a 4-byte operand: `movd` the low dword + `ucomiss` (single
/// precision). `ucomis*` sets CF/ZF exactly like an unsigned integer `cmp` (and PF on
/// unordered/NaN, which the unsigned failure branches ignore — a documented first-cut
/// limitation), so the same unsigned/equal failure-jcc conditions apply.
fn append_float_compare_r10_r11(bytes: &mut Vec<u8>, byte_size: usize) {
    if byte_size == 4 {
        bytes.extend([0x66, 0x41, 0x0f, 0x6e, 0xc2]); // movd xmm0, r10d
        bytes.extend([0x66, 0x41, 0x0f, 0x6e, 0xcb]); // movd xmm1, r11d
        bytes.extend([0x0f, 0x2e, 0xc1]); // ucomiss xmm0, xmm1
    } else {
        bytes.extend([0x66, 0x49, 0x0f, 0x6e, 0xc2]); // movq xmm0, r10
        bytes.extend([0x66, 0x49, 0x0f, 0x6e, 0xcb]); // movq xmm1, r11
        bytes.extend([0x66, 0x0f, 0x2e, 0xc1]); // ucomisd xmm0, xmm1
    }
}

/// Narrow a guard's float `expected_value` (stored as f64 bits) to the operand's
/// width: for a 4-byte float operand the comparison runs in single precision, so the
/// immediate must be the f32 bit pattern. Exact for any value representable in f32
/// (which a constant compared against an f32 field always is).
fn float_compare_expected_bits(expected_value: i64, byte_size: usize) -> u64 {
    if byte_size == 4 {
        u64::from((f64::from_bits(expected_value as u64) as f32).to_bits())
    } else {
        expected_value as u64
    }
}

pub fn encode_dispatch_guard_compare_static_bytes(
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    skip_byte_distance: isize,
    operator: StateGuardOperator,
    is_float: bool,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 2 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "X86_64 MVP encoder cannot compare {byte_size}-byte dispatch guards yet"
        )));
    }
    let mut bytes = Vec::with_capacity(dispatch_guard_compare_static_width(is_float, byte_size));
    // Storage base; the imm64 (at instruction start + 2) is relocated to the
    // guard's storage-region data symbol by the relocation planner.
    append_mov_r15_imm64(&mut bytes, 0);
    append_load_reg_from_r15(&mut bytes, Reg64::R10, byte_offset, byte_size)?;
    let expected_bits = if is_float {
        float_compare_expected_bits(expected_value, byte_size)
    } else {
        expected_value as u64
    };
    append_mov_reg_imm64(&mut bytes, Reg64::R11, expected_bits);
    if is_float {
        append_float_compare_r10_r11(&mut bytes, byte_size);
    } else {
        append_cmp_r10_r11(&mut bytes, byte_size)?;
    }
    // `skip_byte_distance` is anchored at the instruction's rel32 field start
    // (`current.offset + byte_width - 4`, now architecture-aware in the branch-
    // distance helper). The jcc rel is measured from the field's end, 4 bytes
    // later, so the relative target is `skip_byte_distance - 4`.
    append_failure_branch(&mut bytes, operator, skip_byte_distance - 4, is_float)?;
    debug_assert_eq!(
        bytes.len(),
        dispatch_guard_compare_static_width(is_float, byte_size)
    );
    Ok(bytes)
}

/// Exact registers overwritten by a storage-backed static dispatch guard.
/// Integer guards stay in the GPR bank; float guards additionally stage the
/// operands through xmm0/xmm1 before `ucomis*` writes condition flags.
pub fn dispatch_guard_compare_static_register_writes(is_float: bool) -> RegisterSet {
    let mut registers = vec![
        MachineRegister::X86R10,
        MachineRegister::X86R11,
        MachineRegister::X86R15,
    ];
    if is_float {
        registers.extend([MachineRegister::X86Xmm(0), MachineRegister::X86Xmm(1)]);
    }
    RegisterSet::new(registers)
}

pub fn dispatch_guard_compare_static_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

pub fn host_call_sequence_width<T: InstructionOperandLike>(
    policy: CallingPolicy,
    operation_key: HostOperationKey,
    operands: &[T],
) -> usize {
    match encode_host_call_sequence(policy, operation_key, operands) {
        Ok(bytes) => bytes.len(),
        Err(error) => {
            if std::env::var_os("OMEGA_DEBUG_RECEIVER").is_some() {
                eprintln!(
                    "BTW host call width 0: {}.{}: {}",
                    operation_key.capability_name(),
                    operation_key.operation_name(),
                    error.message
                );
            }
            0
        }
    }
}

pub fn host_call_data_relocation_site<T: InstructionOperandLike>(
    operation_key: HostOperationKey,
    operands: &[T],
    operand_index: usize,
) -> Option<X86_64RelocationSite> {
    host_call_data_relocation_site_for_policy(
        CallingPolicy::MicrosoftX64,
        operation_key,
        operands,
        operand_index,
    )
}

pub fn host_call_data_relocation_site_for_policy<T: InstructionOperandLike>(
    policy: CallingPolicy,
    operation_key: HostOperationKey,
    operands: &[T],
    operand_index: usize,
) -> Option<X86_64RelocationSite> {
    host_call_relocation_sites_for_policy(policy, operation_key, operands)
        .into_iter()
        .find(|site| {
            site.operand_index == Some(operand_index)
                && site.kind == X86_64RelocationSiteKind::Absolute64
        })
}

/// A `mov <arg-reg>, imm64` is 10 bytes (2-byte REX.W+B8 prefix, then the imm64), and
/// for both an immediate/data-address argument (`mov arg, imm64`) and a runtime-storage
/// argument (whose first instruction is `mov r11, imm64=0` for the relocated region base)
/// the relocated imm64 sits at the argument's start + 2.
pub const SYSCALL_ARG_MOV_WIDTH: usize = 10;

/// Byte width of marshalling a single syscall argument into its register. Simple
/// arguments (immediate, byte-length, data-address) are a direct `mov arg, imm64`;
/// runtime-storage arguments stage the value through r11/rax (see `encode_syscall_sequence`).
fn syscall_arg_operand_width<T: InstructionOperandLike>(operand: &T) -> usize {
    if operand.runtime_pointee_string_pointer().is_some()
        || operand.runtime_pointee_string_length().is_some()
    {
        // mov r11,imm64 (10) + mov r11,[r11+off] (7) + mov rax,[r11+disp] (7) + mov arg,rax (3)
        SYSCALL_ARG_MOV_WIDTH + 7 + 7 + 3
    } else if operand.runtime_string_pointer().is_some()
        || operand.runtime_string_length().is_some()
        || operand.runtime_scalar_integer().is_some()
    {
        // mov r11,imm64 (10) + mov rax,[r11+disp] (7) + mov arg,rax (3)
        SYSCALL_ARG_MOV_WIDTH + 7 + 3
    } else {
        // mov arg,imm64
        SYSCALL_ARG_MOV_WIDTH
    }
}

/// Byte offset (within the syscall sequence) of the relocated imm64 for the argument at
/// `operand_index`: the sum of the widths of all preceding arguments, plus the 2-byte
/// prefix before the imm64. Applies to both data-address and runtime-storage arguments,
/// whose relocated `mov`/`mov r11` is always the argument's first instruction.
pub fn syscall_data_relocation_byte_offset<T: InstructionOperandLike>(
    operands: &[T],
    operand_index: usize,
) -> usize {
    operands
        .iter()
        .take(operand_index)
        .map(syscall_arg_operand_width)
        .sum::<usize>()
        + 2
}

/// Total byte width of a Linux syscall sequence: each argument's marshalling, plus
/// `mov rax, imm64` (the syscall number) and the 2-byte `syscall`.
pub fn syscall_sequence_width<T: InstructionOperandLike>(operands: &[T]) -> usize {
    operands
        .iter()
        .map(syscall_arg_operand_width)
        .sum::<usize>()
        + SYSCALL_ARG_MOV_WIDTH
        + 2
}

/// x86_64 Linux (System V) syscall sequence: marshal each argument into the syscall
/// argument registers in order (RDI, RSI, RDX, R10, R8, R9), load the syscall number
/// into RAX, then `syscall` (0F 05).
///
/// Simple arguments emit a direct `mov arg, imm64` (data-address arguments use imm64=0
/// fixed up by an Absolute64 relocation). Runtime-storage arguments (a String descriptor
/// in a statically-allocated frame/machine/data region) stage through r11 and rax: load
/// the relocated region base into r11, read the pointer/length field (descriptor layout:
/// pointer at +0, length at +8) into rax, then `mov arg, rax`. Both scratch registers are
/// in the normalized syscall plan's ordinary-clobber set; no callee-saved register is
/// silently destroyed by the marshaller.
pub fn encode_syscall_sequence<T: InstructionOperandLike>(
    operands: &[T],
    syscall_number: u32,
    argument_registers: &[omega_calling_conventions::MachineRegister],
    number_register: omega_calling_conventions::MachineRegister,
    supervisor_call: u16,
) -> Result<Vec<u8>, Diagnostic> {
    if operands.len() != argument_registers.len() {
        return Err(Diagnostic::error(format!(
            "X86_64 syscall plan supplied {} argument registers for {} operands",
            argument_registers.len(),
            operands.len()
        )));
    }
    if supervisor_call != 0 {
        return Err(Diagnostic::error(format!(
            "X86_64 `syscall` has no supervisor-call immediate, but the normalized plan supplied {supervisor_call}"
        )));
    }
    let mut bytes = Vec::with_capacity(syscall_sequence_width(operands));
    for (operand, register) in operands.iter().zip(argument_registers.iter().copied()) {
        if let Some((_, byte_offset)) = operand.runtime_pointee_string_pointer() {
            append_mov_r11_imm64(&mut bytes, 0); // relocated region base
            append_load_r11_qword_from_r11(&mut bytes, byte_offset)?; // r11 = &descriptor
            append_load_rax_from_r11(&mut bytes, 0)?; // rax = descriptor.pointer
            append_mov_syscall_arg_from_rax(&mut bytes, register)?;
        } else if let Some((_, byte_offset)) = operand.runtime_pointee_string_length() {
            append_mov_r11_imm64(&mut bytes, 0);
            append_load_r11_qword_from_r11(&mut bytes, byte_offset)?;
            append_load_rax_from_r11(&mut bytes, 8)?; // rax = descriptor.length
            append_mov_syscall_arg_from_rax(&mut bytes, register)?;
        } else if let Some((_, byte_offset)) = operand.runtime_string_pointer() {
            append_mov_r11_imm64(&mut bytes, 0);
            if operand.runtime_string_is_bounded_buffer() {
                // Owned carrier: content pointer = base + byte_offset + pointer_size.
                bytes.extend([0x49, 0x8d, 0x83]); // lea rax, [r11 + disp32]
                bytes.extend(disp32(byte_offset + 8)?.to_le_bytes());
            } else {
                append_load_rax_from_r11(&mut bytes, byte_offset)?; // rax = descriptor.pointer
            }
            append_mov_syscall_arg_from_rax(&mut bytes, register)?;
        } else if let Some((_, byte_offset)) = operand.runtime_string_length() {
            append_mov_r11_imm64(&mut bytes, 0);
            if operand.runtime_string_is_bounded_buffer() {
                append_load_rax_from_r11(&mut bytes, byte_offset)?; // carrier len @ offset 0
            } else {
                append_load_rax_from_r11(&mut bytes, byte_offset + 8)?; // rax = descriptor.length
            }
            append_mov_syscall_arg_from_rax(&mut bytes, register)?;
        } else if let Some((_, byte_offset, _)) = operand.runtime_scalar_integer() {
            append_mov_r11_imm64(&mut bytes, 0); // relocated region base
            append_load_rax_from_r11(&mut bytes, byte_offset)?; // rax = scalar value
            append_mov_syscall_arg_from_rax(&mut bytes, register)?;
        } else {
            let opcode = syscall_arg_mov_imm64_opcode(register)?;
            let value = if let Some(value) = operand.immediate_integer() {
                value as u64
            } else if let Some(value) = operand.byte_length() {
                value as u64
            } else if operand.data_address().is_some() {
                0 // relocated to the data symbol's address
            } else {
                return Err(Diagnostic::error(
                    "X86_64 syscall encoder cannot marshal this argument yet (expected \
                     immediate, byte-length, data-address, or runtime-storage)",
                ));
            };
            bytes.extend(opcode);
            bytes.extend(value.to_le_bytes());
        }
    }
    append_mov_syscall_register_imm64(&mut bytes, number_register, u64::from(syscall_number))?;
    bytes.extend([0x0f, 0x05]); // syscall
    debug_assert_eq!(bytes.len(), syscall_sequence_width(operands));
    Ok(bytes)
}

fn append_load_r11_qword_from_r11(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    bytes.extend([0x4d, 0x8b, 0x9b]);
    bytes.extend(disp32(byte_offset)?.to_le_bytes());
    Ok(())
}

fn append_load_rax_from_r11(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    bytes.extend([0x49, 0x8b, 0x83]);
    bytes.extend(disp32(byte_offset)?.to_le_bytes());
    Ok(())
}

/// `mov <plan-selected-register>, imm64` (REX.W + B8+rd).
fn syscall_arg_mov_imm64_opcode(
    register: omega_calling_conventions::MachineRegister,
) -> Result<[u8; 2], Diagnostic> {
    use omega_calling_conventions::MachineRegister::*;
    Ok(match register {
        X86Rax => [0x48, 0xb8],
        X86Rcx => [0x48, 0xb9],
        X86Rdx => [0x48, 0xba],
        X86Rbx => [0x48, 0xbb],
        X86Rsp => [0x48, 0xbc],
        X86Rbp => [0x48, 0xbd],
        X86Rsi => [0x48, 0xbe],
        X86Rdi => [0x48, 0xbf],
        X86R8 => [0x49, 0xb8],
        X86R9 => [0x49, 0xb9],
        X86R10 => [0x49, 0xba],
        X86R11 => [0x49, 0xbb],
        X86R12 => [0x49, 0xbc],
        X86R13 => [0x49, 0xbd],
        X86R14 => [0x49, 0xbe],
        X86R15 => [0x49, 0xbf],
        other => {
            return Err(Diagnostic::error(format!(
                "X86_64 syscall plan selected non-GPR argument register {other:?}"
            )));
        }
    })
}

/// `mov <plan-selected-register>, rax` (opcode 89 /r, source rax = reg field 0).
fn append_mov_syscall_arg_from_rax(
    bytes: &mut Vec<u8>,
    register: omega_calling_conventions::MachineRegister,
) -> Result<(), Diagnostic> {
    let [rex, opcode] = syscall_arg_mov_imm64_opcode(register)?;
    let register_code = opcode - 0xb8;
    bytes.extend([rex, 0x89, 0xc0 | register_code]);
    Ok(())
}

fn append_mov_syscall_register_imm64(
    bytes: &mut Vec<u8>,
    register: omega_calling_conventions::MachineRegister,
    value: u64,
) -> Result<(), Diagnostic> {
    bytes.extend(syscall_arg_mov_imm64_opcode(register)?);
    bytes.extend(value.to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod syscall_plan_register_tests {
    use super::*;
    use omega_calling_conventions::MachineRegister;
    use omega_target_operations::{InstructionOperandKind, TargetInstructionOperand};

    #[test]
    fn syscall_arguments_use_the_plan_selected_register() {
        let operands = [TargetInstructionOperand {
            kind: InstructionOperandKind::ImmediateInteger(7),
        }];
        let bytes = encode_syscall_sequence(
            &operands,
            60,
            &[MachineRegister::X86R10],
            MachineRegister::X86Rax,
            0,
        )
        .expect("noncanonical syscall register should encode");

        assert_eq!(&bytes[..2], &[0x49, 0xba], "argument must target r10");
        assert_eq!(&bytes[2..10], &7u64.to_le_bytes());
        assert_eq!(&bytes[10..12], &[0x48, 0xb8], "number must target rax");
        assert_eq!(&bytes[12..20], &60u64.to_le_bytes());
        assert_eq!(&bytes[20..], &[0x0f, 0x05]);
    }

    #[test]
    fn runtime_syscall_arguments_use_only_volatile_plan_scratch() {
        let operands = [TargetInstructionOperand {
            kind: InstructionOperandKind::RuntimeScalarInteger {
                region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 24,
                byte_count: 8,
            },
        }];
        let bytes = encode_syscall_sequence(
            &operands,
            1,
            &[MachineRegister::X86Rdi],
            MachineRegister::X86Rax,
            0,
        )
        .expect("runtime syscall argument");

        assert_eq!(&bytes[..2], &[0x49, 0xbb], "base must use volatile r11");
        assert_eq!(&bytes[10..13], &[0x49, 0x8b, 0x83]);
        assert_eq!(&bytes[17..20], &[0x48, 0x89, 0xc7]);
        assert!(!bytes.windows(2).any(|window| window == [0x49, 0xbf]));
    }
}

pub fn host_call_external_relocation_site<T: InstructionOperandLike>(
    operation_key: HostOperationKey,
    operands: &[T],
) -> Option<X86_64RelocationSite> {
    host_call_external_relocation_site_for_policy(
        CallingPolicy::MicrosoftX64,
        operation_key,
        operands,
    )
}

pub fn host_call_external_relocation_site_for_policy<T: InstructionOperandLike>(
    policy: CallingPolicy,
    operation_key: HostOperationKey,
    operands: &[T],
) -> Option<X86_64RelocationSite> {
    host_call_relocation_sites_for_policy(policy, operation_key, operands)
        .into_iter()
        .find(|site| site.kind == X86_64RelocationSiteKind::Relative32)
}

pub fn encode_host_call_sequence<T: InstructionOperandLike>(
    policy: CallingPolicy,
    operation_key: HostOperationKey,
    operands: &[T],
) -> Result<Vec<u8>, Diagnostic> {
    // Target calibration constants do not cross a call boundary. Keep their
    // architecture-local materialization available under every x86 policy.
    if matches!(
        (operation_key.capability, operation_key.operation),
        (
            HostCapability::Clock,
            HostOperation::WallClockUnitsPerSecond | HostOperation::WallClockEpochOffsetSeconds
        )
    ) {
        return encode_constant_result(operands);
    }
    if policy == CallingPolicy::SystemVAMD64
        && matches!(
            operation_key.capability,
            HostCapability::Unknown | HostCapability::Custom(_)
        )
    {
        return encode_sysv_import_call(operands, true);
    }
    if policy != CallingPolicy::MicrosoftX64 {
        return Err(Diagnostic::error(format!(
            "X86_64 compatibility host encoder implements Microsoft x64, not {policy:?}"
        )));
    }
    match (operation_key.capability, operation_key.operation) {
        (
            HostCapability::Stdin | HostCapability::Stdout | HostCapability::Stderr,
            HostOperation::GetStdHandle,
        ) => encode_win64_import_call(operands, false, false),
        (
            HostCapability::Stdout | HostCapability::Stderr,
            HostOperation::Write | HostOperation::WriteFile,
        ) => encode_file_operation(operation_key, operands),
        (HostCapability::Stdin, HostOperation::ReadFile) => {
            encode_file_operation(operation_key, operands)
        }
        (HostCapability::Process, HostOperation::ExitProcess)
        | (HostCapability::Clock, HostOperation::Sleep) => {
            encode_win64_import_call(operands, false, false)
        }
        // A 0-arg value-returning import through the GENERAL import-call encoder
        // (byte-identical to the original bespoke tick_count sequence for an
        // 8-byte result, and width-correct for a 4-byte one).
        (HostCapability::Clock, HostOperation::TickCount) => {
            encode_win64_import_call(operands, true, false)
        }
        // 0-arg value-returning imports whose result arrives through an
        // OUT-PARAM (QueryPerformanceCounter/-Frequency write a LARGE_INTEGER,
        // GetSystemTimePreciseAsFileTime a FILETIME): bracket the call with a
        // stack slot and load the u64 back (std::time rung 5).
        (
            HostCapability::Clock,
            HostOperation::MonotonicTicks
            | HostOperation::MonotonicTicksPerSecond
            | HostOperation::WallClockRaw,
        ) => encode_win64_out_param_call(operation_key, operands),
        (HostCapability::Input, HostOperation::KeyState) => encode_key_state_call(operands),
        // Every Gui import is value-returning and encodes through the GENERAL
        // import call: operands[0] = result place, then the full ABI argument
        // list (selection interleaves the hard-wired immediates).
        (HostCapability::Gui, _) => encode_win64_import_call(operands, true, false),
        // Every Filesystem raw-seam op is value-returning (fd/count/rc) and
        // rides the same general import call (msvcrt's POSIX-shaped CRT calls
        // marshal like any Win64 import). `read_errno` (`_errno()` returns
        // `&errno`) derefs the returned pointer before the store, exactly the
        // darwin `___error()` shape.
        (HostCapability::Filesystem, _) => {
            encode_win64_import_call(operands, true, operation_key.dereferences_result())
        }
        // Provides-AUTHORED ops (extern brief §12): outside the closed catalog
        // the key is (Unknown, Unknown), and the op only reaches encoding when
        // its authored DllImport binding exists -- ride the same general
        // value-returning import call as the Filesystem/Gui rows.
        (HostCapability::Unknown | HostCapability::Custom(_), _) => {
            encode_win64_import_call(operands, true, false)
        }
        _ => Err(Diagnostic::error(format!(
            "X86_64 host operation {}.{} is not implemented",
            operation_key.capability_name(),
            operation_key.operation_name()
        ))),
    }
}

/// Encode an authored import from the exact validated source-selected plan.
/// The concrete image target does not replace the boundary's policy choice.
pub fn encode_authored_import_call_sequence<T: InstructionOperandLike>(
    plan: &CallPlan,
    operands: &[T],
) -> Result<Vec<u8>, Diagnostic> {
    match plan.policy {
        CallingPolicy::MicrosoftX64 => {
            encode_win64_import_call_with_plan(operands, true, false, Some(plan))
        }
        CallingPolicy::SystemVAMD64 => {
            Ok(sysv_import_layout_with_plan(operands, true, Some(plan))?.bytes)
        }
        policy => Err(Diagnostic::error(format!(
            "x86-64 authored import encoder cannot realize {policy:?}"
        ))),
    }
}

pub fn authored_import_relocation_sites<T: InstructionOperandLike>(
    plan: &CallPlan,
    operands: &[T],
) -> Vec<X86_64RelocationSite> {
    match plan.policy {
        CallingPolicy::MicrosoftX64 => {
            win64_import_call_relocation_sites_with_plan(operands, true, false, Some(plan))
        }
        CallingPolicy::SystemVAMD64 => sysv_import_layout_with_plan(operands, true, Some(plan))
            .map(|layout| layout.relocation_sites)
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// `GetAsyncKeyState(vk)` -- a value-returning USER32 import (the multi-DLL
/// proof): shadow space, the vk marshalled into ecx from operands[1] (constant
/// or runtime scalar), the relocated `call rel32`, the shadow restore, then
/// `movzx eax, ax` (the return is a SHORT; zero the undefined upper bits) and
/// the store-rax tail into the result place (operands[0]).
fn encode_key_state_call<T: InstructionOperandLike>(operands: &[T]) -> Result<Vec<u8>, Diagnostic> {
    let Some((_, result_offset, _)) = operands
        .first()
        .and_then(|operand| operand.runtime_scalar_integer())
    else {
        return Err(Diagnostic::error(
            "cannot encode X86_64 key_state: the result storage place did not lower to a              runtime scalar operand",
        ));
    };
    let plan = normalized_win64_call_plan(operands, Some(0), 1)?;
    let result_register = normalized_win64_result_register(&plan, true)?;
    if result_register != Some(MachineRegister::X86Rax) {
        return Err(Diagnostic::error(format!(
            "Microsoft x64 key-state result requires rax, got {result_register:?}"
        )));
    }
    let reserve = win64_import_reserve(plan.parameters.len());
    let mut bytes = Vec::with_capacity(4 + 17 + 5 + 4 + 3 + 17);
    append_sub_rsp(&mut bytes, reserve);
    append_win64_call_arguments(&mut bytes, operands, 1, Some(&plan.parameters))?;
    bytes.extend([0xe8, 0, 0, 0, 0]); // call rel32 (relocated)
    append_add_rsp(&mut bytes, reserve);
    bytes.extend([0x0f, 0xb7, 0xc0]); // movzx eax, ax (zero the upper bits)
    append_mov_r11_imm64(&mut bytes, 0); // relocated to the result region base
    bytes.extend([0x49, 0x89, 0x83]); // mov [r11 + disp32], rax
    let displacement: i32 = result_offset
        .try_into()
        .map_err(|_| Diagnostic::error("key_state result offset exceeds i32"))?;
    bytes.extend(displacement.to_le_bytes());
    Ok(bytes)
}

fn encode_file_operation<T: InstructionOperandLike>(
    operation_key: HostOperationKey,
    operands: &[T],
) -> Result<Vec<u8>, Diagnostic> {
    let (pointer_index, length_index) = file_pointer_and_length_indices(operands)?;
    if operands.len() <= length_index {
        return Err(Diagnostic::error(
            "cannot encode X86_64 file operation: missing pointer/length operands",
        ));
    }
    let layout = normalized_win64_file_io_layout()?;

    let mut bytes = Vec::new();
    append_sub_rsp(&mut bytes, layout.reserve);
    if pointer_index == 1 {
        let handle = immediate_i32(operands, 0, "file handle")?;
        bytes.push(0xb9); // mov ecx, imm32
        bytes.extend(handle.to_le_bytes());
    } else {
        bytes.extend([0x48, 0x89, 0xc1]); // mov rcx, rax
    }
    append_file_pointer_operand(&mut bytes, &operands[pointer_index])?;
    if operation_key.capability == HostCapability::Stdin
        && operation_key.operation == HostOperation::ReadFile
    {
        bytes.extend([0xc6, 0x02, 0]); // mov byte ptr [rdx], 0
    }
    append_file_length_operand(&mut bytes, &operands[length_index])?;
    bytes.extend([0x4c, 0x8d, 0x4c, 0x24, layout.transferred_disp]);
    bytes.extend([0x48, 0xc7, 0x44, 0x24, layout.overlapped_disp, 0, 0, 0, 0]);
    bytes.extend([0xe8, 0, 0, 0, 0]); // call rel32
    append_add_rsp(&mut bytes, layout.reserve);
    Ok(bytes)
}

/// ReadFile and WriteFile share the same five-argument Win32 signature:
/// HANDLE, buffer pointer, DWORD count, transferred-count pointer, and an
/// optional OVERLAPPED pointer. Their BOOL result is intentionally ignored by
/// this compatibility sequence.
fn normalized_win64_file_io_plan() -> Result<CallPlan, Diagnostic> {
    evaluate_normalized_win64_plan(&CallSignature {
        parameters: vec![
            ValueShape::integer(8, 8),
            ValueShape::integer(8, 8),
            ValueShape::integer(4, 4),
            ValueShape::integer(8, 8),
            ValueShape::integer(8, 8),
        ],
        result: Some(ValueShape::integer(4, 4)),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Win64FileIoLayout {
    reserve: usize,
    overlapped_disp: u8,
    transferred_disp: u8,
}

fn normalized_win64_file_io_layout() -> Result<Win64FileIoLayout, Diagnostic> {
    let plan = normalized_win64_file_io_plan()?;
    for (index, expected) in [
        MachineRegister::X86Rcx,
        MachineRegister::X86Rdx,
        MachineRegister::X86R8,
        MachineRegister::X86R9,
    ]
    .into_iter()
    .enumerate()
    {
        let actual = win64_argument_location(&plan.parameters[index], index)?;
        if actual != Win64ArgumentLocation::Register(expected) {
            return Err(Diagnostic::error(format!(
                "Win64 file-I/O parameter {index} requires {expected:?}, got {actual:?}"
            )));
        }
    }
    let overlapped_location = win64_argument_location(&plan.parameters[4], 4)?;
    let Win64ArgumentLocation::Stack(overlapped_offset) = overlapped_location else {
        return Err(Diagnostic::error(format!(
            "Win64 file-I/O encoder requires OVERLAPPED on the stack, got {overlapped_location:?}"
        )));
    };
    let native_result = normalized_win64_result_register(&plan, true)?;
    if native_result != Some(MachineRegister::X86Rax) {
        return Err(Diagnostic::error(format!(
            "Win64 file-I/O encoder requires its native BOOL result in rax, got {native_result:?}"
        )));
    }
    let transferred_offset = overlapped_offset
        .checked_add(8)
        .ok_or_else(|| Diagnostic::error("Win64 file-I/O temporary stack offset overflowed"))?;
    Ok(Win64FileIoLayout {
        reserve: win64_composite_reserve(transferred_offset + 8)?,
        transferred_disp: u8::try_from(transferred_offset).map_err(|_| {
            Diagnostic::error("Win64 file-I/O transferred-count slot exceeds disp8")
        })?,
        overlapped_disp: u8::try_from(overlapped_offset)
            .map_err(|_| Diagnostic::error("Win64 file-I/O OVERLAPPED slot exceeds disp8"))?,
    })
}

fn validate_normalized_win64_get_std_handle_plan() -> Result<(), Diagnostic> {
    let plan = evaluate_normalized_win64_plan(&CallSignature {
        parameters: vec![ValueShape::integer(4, 4)],
        result: Some(ValueShape::integer(8, 8)),
    })?;
    let argument = win64_argument_location(&plan.parameters[0], 0)?;
    let result = normalized_win64_result_register(&plan, true)?;
    if argument != Win64ArgumentLocation::Register(MachineRegister::X86Rcx)
        || result != Some(MachineRegister::X86Rax)
    {
        return Err(Diagnostic::error(format!(
            "Win64 GetStdHandle encoder cannot realize argument={argument:?}, result={result:?}"
        )));
    }
    Ok(())
}

/// Reserve through a composite call's final local byte while preserving the
/// encoder's entry invariant: rsp is 8 mod 16 before `sub`, so the reservation
/// itself must also be 8 mod 16 at the call boundary.
fn win64_composite_reserve(required_bytes: u32) -> Result<usize, Diagnostic> {
    let required = usize::try_from(required_bytes)
        .map_err(|_| Diagnostic::error("Win64 composite stack reservation exceeds usize"))?;
    let remainder = required % 16;
    let padding = (8 + 16 - remainder) % 16;
    Ok(required + padding)
}

fn append_file_pointer_operand<T: InstructionOperandLike>(
    bytes: &mut Vec<u8>,
    operand: &T,
) -> Result<(), Diagnostic> {
    if operand.data_address().is_some() {
        append_mov_rdx_imm64(bytes, 0);
        Ok(())
    } else if let Some((_, byte_offset)) = operand.runtime_string_pointer() {
        append_mov_r10_imm64(bytes, 0);
        if operand.runtime_string_is_bounded_buffer() {
            // Owned carrier: the content pointer is the COMPUTED inline-bytes
            // address `base + byte_offset + pointer_size` (lea), not a stored
            // descriptor pointer. Same width as the descriptor-pointer load.
            bytes.extend([0x49, 0x8d, 0x92]); // lea rdx, [r10 + disp32]
            bytes.extend(disp32(byte_offset + 8)?.to_le_bytes());
        } else {
            append_load_rdx_from_r10(bytes, byte_offset)?;
        }
        Ok(())
    } else if let Some((_, byte_offset)) = operand.runtime_pointee_string_pointer() {
        append_mov_r10_imm64(bytes, 0);
        append_load_r10_from_r10(bytes, byte_offset)?;
        append_load_rdx_from_r10(bytes, 0)?;
        Ok(())
    } else {
        Err(Diagnostic::error(
            "cannot encode X86_64 file operation: pointer operand is unsupported",
        ))
    }
}

fn append_file_length_operand<T: InstructionOperandLike>(
    bytes: &mut Vec<u8>,
    operand: &T,
) -> Result<(), Diagnostic> {
    if let Some(value) = operand.byte_length() {
        let value = u32::try_from(value).map_err(|_| {
            Diagnostic::error(format!(
                "cannot encode X86_64 file operation: byte length {value} does not fit u32"
            ))
        })?;
        bytes.extend([0x41, 0xb8]); // mov r8d, imm32
        bytes.extend(value.to_le_bytes());
        Ok(())
    } else if let Some((_, byte_offset)) = operand.runtime_string_length() {
        append_mov_r10_imm64(bytes, 0);
        if operand.runtime_string_is_bounded_buffer() {
            // Owned carrier: length is at offset 0 (not the descriptor's len word
            // at offset pointer_size).
            append_load_r8_from_r10(bytes, byte_offset)?;
        } else {
            append_load_r8_from_r10(bytes, byte_offset + 8)?;
        }
        Ok(())
    } else if let Some((_, byte_offset)) = operand.runtime_pointee_string_length() {
        append_mov_r10_imm64(bytes, 0);
        append_load_r10_from_r10(bytes, byte_offset)?;
        append_load_r8_from_r10(bytes, 8)?;
        Ok(())
    } else {
        Err(Diagnostic::error(
            "cannot encode X86_64 file operation: length operand is unsupported",
        ))
    }
}

/// The Win64 integer argument registers, in call order, as
/// (mov-imm32 opcode bytes, load-from-[r11+disp32] opcode bytes) pairs:
/// rcx, rdx, r8, r9. Immediates use the 32-bit `mov r32, imm32` forms (the
/// kernel32 surface is u32-shaped today); loads are 64-bit `mov r64,
/// [r11+disp32]` (callees read the low 32 bits).
const WIN64_ARG_REGISTERS: [(&[u8], &[u8]); 4] = [
    (&[0xb9], &[0x49, 0x8b, 0x8b]), // mov ecx, imm32 / mov rcx, [r11+d]
    (&[0xba], &[0x49, 0x8b, 0x93]), // mov edx, imm32 / mov rdx, [r11+d]
    (&[0x41, 0xb8], &[0x4d, 0x8b, 0x83]), // mov r8d, imm32 / mov r8,  [r11+d]
    (&[0x41, 0xb9], &[0x4d, 0x8b, 0x8b]), // mov r9d, imm32 / mov r9,  [r11+d]
];

/// `lea <reg64>, [r11+disp32]` opcode bytes for the Win64 integer argument
/// registers rcx/rdx/r8/r9 -- `WIN64_ARG_REGISTERS`' load opcodes with the mov
/// (8B) swapped for lea (8D), byte-for-byte the same width.
const WIN64_ARG_LEA_OPCODES: [&[u8]; 4] = [
    &[0x49, 0x8d, 0x8b], // lea rcx, [r11+d]
    &[0x49, 0x8d, 0x93], // lea rdx, [r11+d]
    &[0x4d, 0x8d, 0x83], // lea r8,  [r11+d]
    &[0x4d, 0x8d, 0x8b], // lea r9,  [r11+d]
];

/// `mov <reg64>, imm64` opcode bytes for the Win64 integer argument registers
/// rcx/rdx/r8/r9 -- a DATA-ADDRESS argument (a string-literal path, e.g.
/// `_open("...")`) marshals as the data symbol's absolute address, imm64=0
/// relocated Absolute64 at the opcode's +2 (the same imm64 position as the
/// staged `mov r11, imm64` forms, so the relocation-site walker treats all
/// three identically).
const WIN64_ARG_MOV_IMM64_OPCODES: [&[u8]; 4] = [
    &[0x48, 0xb9], // mov rcx, imm64
    &[0x48, 0xba], // mov rdx, imm64
    &[0x49, 0xb8], // mov r8,  imm64
    &[0x49, 0xb9], // mov r9,  imm64
];

/// The outgoing stack-argument area starts right above the 32-byte shadow space.
const WIN64_STACK_ARG_HOME: usize = 32;

/// The stack reservation for a general Win64 import call with `arg_count`
/// arguments: the 32-byte shadow space plus one 8-byte outgoing slot per
/// argument past the 4 register args, padded so rsp stays 16-byte aligned at
/// the `call` (the emitted code runs with rsp ≡ 8 mod 16 -- the invariant the
/// existing 40-byte no-stack-arg reservation encodes).
fn win64_import_reserve(arg_count: usize) -> usize {
    let stack_slots = arg_count.saturating_sub(4);
    win64_import_reserve_bytes(WIN64_STACK_ARG_HOME + 8 * stack_slots)
}

fn win64_import_reserve_for_plan(plan: &CallPlan) -> usize {
    let stack_bytes = plan
        .parameters
        .iter()
        .flat_map(|placement| placement.locations.iter())
        .map(|location| match location {
            ValueLocation::Register { .. } => 0,
            ValueLocation::Stack {
                stack_byte_offset,
                byte_size,
                ..
            } => *stack_byte_offset as usize + usize::from((*byte_size).max(8)),
            ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset,
                byte_size,
                ..
            } => {
                let pointer_end = match pointer {
                    IndirectPointerLocation::Register(_) => 0,
                    IndirectPointerLocation::Stack {
                        stack_byte_offset, ..
                    } => *stack_byte_offset as usize + 8,
                };
                let copy_end = copy_stack_byte_offset
                    .map(|offset| offset as usize + usize::from(*byte_size))
                    .unwrap_or(0);
                pointer_end.max(copy_end)
            }
        })
        .max()
        .unwrap_or(0)
        .max(usize::from(plan.shadow_bytes));
    win64_import_reserve_bytes(stack_bytes)
}

fn win64_import_reserve_bytes(stack_bytes: usize) -> usize {
    // Emitted Omega call sites enter with rsp == 8 (mod 16). Reserve the
    // smallest area that covers every slot/copy and leaves rsp 16-byte aligned
    // immediately before CALL, including odd-sized indirect record copies.
    (stack_bytes + 8).next_multiple_of(16) - 8
}

/// `sub/add rsp, imm` width: the imm8 form (4 bytes) up to 127, else imm32 (7).
fn rsp_adjust_width(reserve: usize) -> usize {
    if reserve <= 127 { 4 } else { 7 }
}

fn append_sub_rsp(bytes: &mut Vec<u8>, reserve: usize) {
    if reserve <= 127 {
        bytes.extend([0x48, 0x83, 0xec, reserve as u8]); // sub rsp, imm8
    } else {
        bytes.extend([0x48, 0x81, 0xec]); // sub rsp, imm32
        bytes.extend((reserve as u32).to_le_bytes());
    }
}

fn append_add_rsp(bytes: &mut Vec<u8>, reserve: usize) {
    if reserve <= 127 {
        bytes.extend([0x48, 0x83, 0xc4, reserve as u8]); // add rsp, imm8
    } else {
        bytes.extend([0x48, 0x81, 0xc4]); // add rsp, imm32
        bytes.extend((reserve as u32).to_le_bytes());
    }
}

/// Register-indirect near call -- `call r/m64` in the `FF /2` register-DIRECT
/// form: optional `REX.B` (0x41) for r8-r15, `FF`, then ModRM `11 010 rrr`
/// (`0xD0 | (reg & 7)`; mod=11 register-direct, reg=`/2`=010, rm=the register).
/// The target is a POINTER VALUE already in `reg`, NOT an import relocation --
/// this is the runtime-pointer call the first-boot path needs (UEFI
/// `SystemTable -> ConOut -> OutputString` is three pointer hops) and the same
/// emission a `VtableSlot` dispatch will use (extern brief §12.4). `reg` is the
/// x86_64 register number 0..=15 (0=rax..7=rdi, 8=r8..15=r15).
///
fn append_call_register(bytes: &mut Vec<u8>, reg: u8) {
    debug_assert!(reg < 16, "x86_64 register number out of range");
    if reg >= 8 {
        bytes.push(0x41); // REX.B extends ModRM.rm into r8-r15
    }
    bytes.push(0xff);
    bytes.push(0xd0 | (reg & 0x7)); // ModRM: mod=11 reg=/2(010) rm=reg
}

/// Whether a general-import argument operand marshals through the relocated r11
/// region base (a runtime-storage scalar LOAD or a runtime-storage ADDRESS lea)
/// rather than as a constant immediate.
fn win64_import_arg_is_staged<T: InstructionOperandLike>(operand: Option<&T>) -> bool {
    operand.is_some_and(|operand| {
        operand.runtime_scalar_integer().is_some()
            || operand.runtime_scalar_float().is_some()
            || operand.runtime_small_aggregate().is_some()
            || operand.runtime_large_aggregate().is_some()
            || operand.runtime_storage_address().is_some()
            || operand.data_address().is_some()
            || operand.runtime_string_pointer().is_some()
    })
}

/// Whether a general-import argument is a data-object address (`mov <reg>,
/// imm64` relocated to the symbol, no r11 staging) -- narrower than
/// `win64_import_arg_is_staged`, which also covers the r11-staged forms; both
/// place their relocated imm64 at the argument's start + 2.
fn win64_import_arg_is_data_address<T: InstructionOperandLike>(operand: Option<&T>) -> bool {
    operand.is_some_and(|operand| operand.data_address().is_some())
}

/// Marshalling width of general-import argument `index` (0-based ABI order,
/// stored at `operands[arg_start + index]`). For register args, an address lea
/// is the same width as a scalar load; a data-object address is one
/// `mov <reg64>, imm64` (10). Stack args
/// stage through r11/rax (10 + 7 + a 5-byte `mov [rsp+disp8], rax`; a data
/// address is 10 + 5), or store a constant directly (9-byte
/// `mov qword [rsp+disp8], imm32`).
fn win64_import_arg_width<T: InstructionOperandLike>(
    operands: &[T],
    arg_start: usize,
    index: usize,
    placement: Option<&ValuePlacement>,
) -> usize {
    let operand = operands.get(arg_start + index);
    if let Some((_, _, byte_count)) = operand.and_then(|operand| operand.runtime_scalar_float())
        && let Some(placement) = placement
    {
        return match placement.locations.as_slice() {
            [ValueLocation::Register { .. }] => 19,
            [ValueLocation::Stack { .. }] => {
                10 + 7 + win64_direct_aggregate_stack_store_width(byte_count)
            }
            _ => 0,
        };
    }
    if let Some((_, _, byte_count, _)) = operand.and_then(win64_aggregate_operand)
        && let Some(placement) = placement
    {
        match placement.locations.as_slice() {
            [
                ValueLocation::Indirect {
                    pointer,
                    copy_stack_byte_offset: Some(_),
                    ..
                },
            ] => {
                return 10
                    + win64_indirect_aggregate_copy_width(byte_count)
                    + match pointer {
                        IndirectPointerLocation::Register(_) => 8,
                        IndirectPointerLocation::Stack { .. } => 16,
                    };
            }
            [ValueLocation::Register { .. }] => {
                return 10 + win64_direct_aggregate_load_width(byte_count);
            }
            [ValueLocation::Stack { .. }] => {
                return 10
                    + win64_direct_aggregate_load_width(byte_count)
                    + win64_direct_aggregate_stack_store_width(byte_count);
            }
            _ => {}
        }
    }
    let data_address = win64_import_arg_is_data_address(operand);
    let staged = win64_import_arg_is_staged(operand);
    if index < 4 {
        let (imm_opcode, load_opcode) = WIN64_ARG_REGISTERS[index];
        if data_address {
            10
        } else if staged {
            10 + load_opcode.len() + 4
        } else {
            imm_opcode.len() + 4
        }
    } else if data_address {
        10 + 5
    } else if staged {
        10 + 7 + 5
    } else {
        9
    }
}

fn win64_direct_aggregate_load_width(byte_count: usize) -> usize {
    7 + usize::from(byte_count == 2)
}

fn win64_direct_aggregate_stack_store_width(byte_count: usize) -> usize {
    match byte_count {
        8 | 2 => 8,
        4 | 1 => 7,
        _ => 0,
    }
}

fn win64_aggregate_operand<T: InstructionOperandLike>(
    operand: &T,
) -> Option<(RuntimeStorageRegion, usize, usize, usize)> {
    operand
        .runtime_small_aggregate()
        .or_else(|| operand.runtime_large_aggregate())
}

fn win64_indirect_aggregate_copy_width(byte_count: usize) -> usize {
    let mut copied = 0usize;
    let mut width = 0usize;
    while copied < byte_count {
        let fragment = win64_aggregate_copy_fragment_byte_count(byte_count - copied);
        width += match fragment {
            8 => 15,
            4 | 1 => 14,
            2 => 16,
            _ => unreachable!("aggregate copy fragment width is canonical"),
        };
        copied += fragment;
    }
    width
}

fn win64_aggregate_copy_fragment_byte_count(remaining: usize) -> usize {
    [8, 4, 2, 1]
        .into_iter()
        .find(|fragment| remaining >= *fragment)
        .expect("aggregate copy always has bytes remaining")
}

/// Total width of a `encode_win64_import_call` sequence -- must mirror the
/// encoder byte for byte (the relocation cursor math depends on it).
fn win64_import_call_width<T: InstructionOperandLike>(
    operands: &[T],
    returns_value: bool,
    dereferences_result: bool,
) -> usize {
    let arg_start = usize::from(returns_value);
    let arg_count = operands.len().saturating_sub(arg_start);
    let plan = normalized_win64_import_plan(operands, returns_value).ok();
    let reserve = plan
        .as_ref()
        .map(win64_import_reserve_for_plan)
        .unwrap_or_else(|| win64_import_reserve(arg_count));
    let mut width = 2 * rsp_adjust_width(reserve) + 5;
    width += plan.as_ref().map(win64_result_pre_call_width).unwrap_or(0);
    for index in 0..arg_count {
        width += win64_import_arg_width(
            operands,
            arg_start,
            index,
            plan.as_ref().and_then(|plan| plan.parameters.get(index)),
        );
    }
    if dereferences_result {
        width += 2; // mov eax, [rax]
    }
    width += plan
        .as_ref()
        .map(win64_result_post_call_width)
        .unwrap_or_else(|| usize::from(returns_value) * 17);
    width
}

/// A host-call immediate encoded into a 32-bit field: accepts the i32 range AND
/// the u32 range (DWORD flag words like `WS_POPUP|WS_VISIBLE` = 0x9000_0000),
/// encoding the low 32 bits. Register args use `mov r32, imm32` (zero-extends);
/// stack slots use `mov qword, imm32` (SIGN-extends -- correct for ints and for
/// DWORD-consuming callees, so keep pointer-sized big constants out of stack
/// slots).
fn immediate_imm32<T: InstructionOperandLike>(
    operands: &[T],
    index: usize,
    label: &str,
) -> Result<i32, Diagnostic> {
    let Some(value) = operands
        .get(index)
        .and_then(|operand| operand.immediate_integer())
    else {
        return Err(Diagnostic::error(format!(
            "cannot encode X86_64 host call: {label} did not lower to a marshallable operand"
        )));
    };
    if value < i64::from(i32::MIN) || value > i64::from(u32::MAX) {
        return Err(Diagnostic::error(format!(
            "cannot encode X86_64 host call: {label} value {value} does not fit a 32-bit immediate"
        )));
    }
    Ok(value as u32 as i32)
}

/// The GENERAL Win64 import call -- the full extern-ABI shape. Marshals the
/// argument operands into rcx/rdx/r8/r9 then the outgoing stack slots
/// `[rsp + 32 + 8k]`, emits the relocated `call rel32`, restores the stack
/// reservation, and (for a value-returning import) stores rax into the result
/// place at the result's declared width (4-byte results store eax -- an int
/// return's upper 32 bits are undefined under Win64).
///
/// Operand roles: when `returns_value`, `operands[0]` is the RESULT place (a
/// runtime scalar; its byte_count picks the store width) and the arguments
/// follow; otherwise every operand is an argument. Each argument is a constant
/// immediate, a runtime-storage scalar (loaded through the relocated r11 region
/// base), or a runtime-storage ADDRESS (`lea` through the same base -- the
/// pointer-argument shape: buffers, OS structs, C strings).
/// Marshal MS-x64 call arguments `operands[arg_start..]` into RCX/RDX/R8/R9
/// (staged runtime loads/leas through the relocated r11 region base, or plain
/// immediates) and the shadow-space stack home for args past the fourth.
/// Shared by the import call and the vtable call (their only difference is how
/// the callee address is obtained: a relocated `call rel32` vs `call rax`).
fn append_win64_aggregate_argument<T: InstructionOperandLike>(
    bytes: &mut Vec<u8>,
    operand: &T,
    parameter_index: usize,
    placement: &ValuePlacement,
) -> Result<(), Diagnostic> {
    match placement.locations.as_slice() {
        [ValueLocation::Register { .. } | ValueLocation::Stack { .. }] => {
            append_win64_direct_aggregate_argument(bytes, operand, parameter_index, placement)
        }
        [ValueLocation::Indirect { .. }] => {
            append_win64_indirect_aggregate_argument(bytes, operand, parameter_index, placement)
        }
        locations => Err(Diagnostic::error(format!(
            "Microsoft x64 aggregate parameter {parameter_index} has unsupported placement {locations:?}"
        ))),
    }
}

fn append_win64_float_argument<T: InstructionOperandLike>(
    bytes: &mut Vec<u8>,
    operand: &T,
    parameter_index: usize,
    placement: &ValuePlacement,
) -> Result<(), Diagnostic> {
    let Some((_, byte_offset, byte_count)) = operand.runtime_scalar_float() else {
        return Err(Diagnostic::error(format!(
            "Microsoft x64 float parameter {parameter_index} is not a float operand"
        )));
    };
    if !matches!(byte_count, 4 | 8)
        || !matches!(placement.shape.class, ValueClass::Float)
        || usize::from(placement.shape.byte_size) != byte_count
    {
        return Err(Diagnostic::error(format!(
            "Microsoft x64 float parameter {parameter_index} has inconsistent shape"
        )));
    }

    append_mov_r11_imm64(bytes, 0); // relocated to the float's region base
    match placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] if usize::from(*byte_size) == byte_count => {
            append_x86_load_float_from_r11(bytes, *register, byte_offset, byte_count)
        }
        [
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset: 0,
                byte_size,
                ..
            },
        ] if usize::from(*byte_size) == byte_count => {
            append_win64_load_register_from_r11(
                bytes,
                MachineRegister::X86Rax,
                byte_offset,
                byte_count,
            )?;
            append_win64_store_rax_to_rsp(bytes, *stack_byte_offset, byte_count)
        }
        locations => Err(Diagnostic::error(format!(
            "Microsoft x64 float parameter {parameter_index} has unsupported placement {locations:?}"
        ))),
    }
}

fn append_win64_direct_aggregate_argument<T: InstructionOperandLike>(
    bytes: &mut Vec<u8>,
    operand: &T,
    parameter_index: usize,
    placement: &ValuePlacement,
) -> Result<(), Diagnostic> {
    let Some((_, byte_offset, byte_count, alignment)) = win64_aggregate_operand(operand) else {
        return Err(Diagnostic::error(format!(
            "Microsoft x64 direct parameter {parameter_index} is not an aggregate operand"
        )));
    };
    if !matches!(byte_count, 1 | 2 | 4 | 8)
        || usize::from(placement.shape.byte_size) != byte_count
        || usize::from(placement.shape.alignment) != alignment
    {
        return Err(Diagnostic::error(format!(
            "Microsoft x64 direct aggregate parameter {parameter_index} has inconsistent shape"
        )));
    }

    append_mov_r11_imm64(bytes, 0); // relocated to the aggregate's region base
    match placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] if usize::from(*byte_size) == byte_count => {
            append_win64_load_register_from_r11(bytes, *register, byte_offset, byte_count)
        }
        [
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset: 0,
                byte_size,
                ..
            },
        ] if usize::from(*byte_size) == byte_count => {
            append_win64_load_register_from_r11(
                bytes,
                MachineRegister::X86Rax,
                byte_offset,
                byte_count,
            )?;
            append_win64_store_rax_to_rsp(bytes, *stack_byte_offset, byte_count)
        }
        locations => Err(Diagnostic::error(format!(
            "Microsoft x64 direct aggregate parameter {parameter_index} has unsupported placement {locations:?}"
        ))),
    }
}

fn append_win64_load_register_from_r11(
    bytes: &mut Vec<u8>,
    register: MachineRegister,
    byte_offset: usize,
    byte_count: usize,
) -> Result<(), Diagnostic> {
    let register_number = x86_gpr_number(register).ok_or_else(|| {
        Diagnostic::error(format!(
            "Microsoft x64 direct aggregate uses unsupported register {register:?}"
        ))
    })?;
    if !matches!(
        register,
        MachineRegister::X86Rax
            | MachineRegister::X86Rcx
            | MachineRegister::X86Rdx
            | MachineRegister::X86R8
            | MachineRegister::X86R9
    ) || !matches!(byte_count, 1 | 2 | 4 | 8)
    {
        return Err(Diagnostic::error(format!(
            "Microsoft x64 direct aggregate cannot load {byte_count} bytes into {register:?}"
        )));
    }
    if byte_count == 2 {
        bytes.push(0x66);
    }
    bytes.extend([
        0x40 | u8::from(byte_count == 8) * 0x08 | u8::from(register_number >= 8) * 0x04 | 0x01,
        if byte_count == 1 { 0x8a } else { 0x8b },
        0x83 | ((register_number & 7) << 3),
    ]); // mov selected register, [r11+disp32]
    bytes.extend(disp32(byte_offset)?.to_le_bytes());
    Ok(())
}

fn append_win64_store_rax_to_rsp(
    bytes: &mut Vec<u8>,
    stack_byte_offset: u32,
    byte_count: usize,
) -> Result<(), Diagnostic> {
    match byte_count {
        8 => bytes.extend([0x48, 0x89, 0x84, 0x24]),
        4 => bytes.extend([0x89, 0x84, 0x24]),
        2 => bytes.extend([0x66, 0x89, 0x84, 0x24]),
        1 => bytes.extend([0x88, 0x84, 0x24]),
        _ => {
            return Err(Diagnostic::error(format!(
                "Microsoft x64 direct aggregate stack width {byte_count} is unsupported"
            )));
        }
    }
    bytes.extend(
        i32::try_from(stack_byte_offset)
            .map_err(|_| Diagnostic::error("Microsoft x64 stack offset exceeds disp32"))?
            .to_le_bytes(),
    );
    Ok(())
}

fn append_win64_indirect_aggregate_argument<T: InstructionOperandLike>(
    bytes: &mut Vec<u8>,
    operand: &T,
    parameter_index: usize,
    placement: &ValuePlacement,
) -> Result<(), Diagnostic> {
    let Some((_, byte_offset, byte_count, alignment)) = win64_aggregate_operand(operand) else {
        return Err(Diagnostic::error(format!(
            "Microsoft x64 indirect parameter {parameter_index} is not an aggregate operand"
        )));
    };
    let [
        ValueLocation::Indirect {
            pointer,
            copy_stack_byte_offset: Some(copy_stack_byte_offset),
            byte_size,
            alignment: planned_alignment,
        },
    ] = placement.locations.as_slice()
    else {
        return Err(Diagnostic::error(format!(
            "Microsoft x64 aggregate parameter {parameter_index} has no caller-copy placement"
        )));
    };
    if matches!(byte_count, 1 | 2 | 4 | 8)
        || usize::from(*byte_size) != byte_count
        || usize::from(*planned_alignment) != alignment
        || !alignment.is_power_of_two()
        || copy_stack_byte_offset % 16 != 0
    {
        return Err(Diagnostic::error(format!(
            "Microsoft x64 aggregate parameter {parameter_index} has inconsistent shape or copy alignment"
        )));
    }

    append_mov_r11_imm64(bytes, 0); // relocated to the aggregate's region base
    let mut copied = 0usize;
    while copied < byte_count {
        let fragment = win64_aggregate_copy_fragment_byte_count(byte_count - copied);
        let source_offset = byte_offset
            .checked_add(copied)
            .ok_or_else(|| Diagnostic::error("Microsoft x64 aggregate source offset overflow"))?;
        match fragment {
            8 => bytes.extend([0x49, 0x8b, 0x83]), // mov rax, [r11+disp32]
            4 => bytes.extend([0x41, 0x8b, 0x83]), // mov eax, [r11+disp32]
            2 => bytes.extend([0x66, 0x41, 0x8b, 0x83]), // mov ax, [r11+disp32]
            1 => bytes.extend([0x41, 0x8a, 0x83]), // mov al, [r11+disp32]
            _ => unreachable!("aggregate copy fragment width is canonical"),
        }
        bytes.extend(disp32(source_offset)?.to_le_bytes());

        let target_offset = usize::try_from(*copy_stack_byte_offset)
            .ok()
            .and_then(|offset| offset.checked_add(copied))
            .ok_or_else(|| Diagnostic::error("Microsoft x64 aggregate copy offset overflow"))?;
        match fragment {
            8 => bytes.extend([0x48, 0x89, 0x84, 0x24]), // mov [rsp+disp32], rax
            4 => bytes.extend([0x89, 0x84, 0x24]),       // mov [rsp+disp32], eax
            2 => bytes.extend([0x66, 0x89, 0x84, 0x24]), // mov [rsp+disp32], ax
            1 => bytes.extend([0x88, 0x84, 0x24]),       // mov [rsp+disp32], al
            _ => unreachable!("aggregate copy fragment width is canonical"),
        }
        bytes.extend(disp32(target_offset)?.to_le_bytes());
        copied += fragment;
    }

    match *pointer {
        IndirectPointerLocation::Register(register) => {
            append_win64_lea_register_from_rsp(bytes, register, *copy_stack_byte_offset)?;
        }
        IndirectPointerLocation::Stack {
            stack_byte_offset, ..
        } => {
            append_win64_lea_register_from_rsp(
                bytes,
                MachineRegister::X86Rax,
                *copy_stack_byte_offset,
            )?;
            bytes.extend([0x48, 0x89, 0x84, 0x24]); // mov [rsp+disp32], rax
            bytes.extend(
                i32::try_from(stack_byte_offset)
                    .map_err(|_| {
                        Diagnostic::error("Microsoft x64 pointer stack offset exceeds disp32")
                    })?
                    .to_le_bytes(),
            );
        }
    }
    Ok(())
}

fn append_win64_lea_register_from_rsp(
    bytes: &mut Vec<u8>,
    register: MachineRegister,
    stack_byte_offset: u32,
) -> Result<(), Diagnostic> {
    let register_number = x86_gpr_number(register).ok_or_else(|| {
        Diagnostic::error(format!(
            "Microsoft x64 aggregate pointer uses unsupported register {register:?}"
        ))
    })?;
    if !matches!(
        register,
        MachineRegister::X86Rax
            | MachineRegister::X86Rcx
            | MachineRegister::X86Rdx
            | MachineRegister::X86R8
            | MachineRegister::X86R9
    ) {
        return Err(Diagnostic::error(format!(
            "Microsoft x64 aggregate pointer uses non-positional register {register:?}"
        )));
    }
    bytes.extend([
        0x48 | if register_number >= 8 { 0x04 } else { 0 },
        0x8d,
        0x84 | ((register_number & 7) << 3),
        0x24,
    ]); // lea selected register, [rsp+disp32]
    bytes.extend(
        i32::try_from(stack_byte_offset)
            .map_err(|_| Diagnostic::error("Microsoft x64 copy offset exceeds disp32"))?
            .to_le_bytes(),
    );
    Ok(())
}

fn append_win64_call_arguments<T: InstructionOperandLike>(
    bytes: &mut Vec<u8>,
    operands: &[T],
    arg_start: usize,
    planned_parameters: Option<&[ValuePlacement]>,
) -> Result<(), Diagnostic> {
    let arg_count = operands.len() - arg_start;
    if let Some(parameters) = planned_parameters
        && parameters.len() != arg_count
    {
        return Err(Diagnostic::error(format!(
            "Microsoft x64 call plan supplied {} parameter placements for {arg_count} operands",
            parameters.len()
        )));
    }
    for index in 0..arg_count {
        let operand = &operands[arg_start + index];
        if operand.runtime_scalar_float().is_some() {
            let placement = planned_parameters
                .and_then(|parameters| parameters.get(index))
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "Microsoft x64 float parameter {index} has no normalized placement"
                    ))
                })?;
            append_win64_float_argument(bytes, operand, index, placement)?;
            continue;
        }
        if win64_aggregate_operand(operand).is_some() {
            let placement = planned_parameters
                .and_then(|parameters| parameters.get(index))
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "Microsoft x64 aggregate parameter {index} has no normalized placement"
                    ))
                })?;
            append_win64_aggregate_argument(bytes, operand, index, placement)?;
            continue;
        }
        let planned_location = planned_parameters
            .map(|parameters| win64_argument_location(&parameters[index], index))
            .transpose()?;
        let register_slot = match planned_location {
            Some(Win64ArgumentLocation::Register(register)) => Some(
                win64_argument_register_slot(register).ok_or_else(|| {
                    Diagnostic::error(format!(
                        "Microsoft x64 import parameter {index} uses unsupported register {register:?}"
                    ))
                })?,
            ),
            Some(Win64ArgumentLocation::Stack(_)) => None,
            None if index < 4 => Some(index),
            None => None,
        };
        if let Some(register_slot) = register_slot {
            let (imm_opcode, load_opcode) = WIN64_ARG_REGISTERS[register_slot];
            if let Some((_, byte_offset, _)) = operand.runtime_scalar_integer() {
                append_mov_r11_imm64(bytes, 0); // relocated to the argument's region base
                bytes.extend_from_slice(load_opcode);
                bytes.extend(disp32(byte_offset)?.to_le_bytes());
            } else if let Some((_, byte_offset)) = operand.runtime_string_pointer() {
                // A string/slice DESCRIPTOR in a storage region (a path or text
                // argument riding a runtime slot, e.g. a value-call param bound
                // to a literal): the C-string argument is the descriptor's
                // POINTER word (at +0), or the inline content after the len
                // word for an owned bounded-buffer carrier -- mirroring the
                // syscall encoder's string-pointer staging.
                append_mov_r11_imm64(bytes, 0); // relocated to the argument's region base
                if operand.runtime_string_is_bounded_buffer() {
                    bytes.extend_from_slice(WIN64_ARG_LEA_OPCODES[register_slot]);
                    bytes.extend(disp32(byte_offset + 8)?.to_le_bytes());
                } else {
                    bytes.extend_from_slice(load_opcode);
                    bytes.extend(disp32(byte_offset)?.to_le_bytes());
                }
            } else if let Some((_, byte_offset)) = operand.runtime_storage_address() {
                append_mov_r11_imm64(bytes, 0); // relocated to the argument's region base
                bytes.extend_from_slice(WIN64_ARG_LEA_OPCODES[register_slot]);
                bytes.extend(disp32(byte_offset)?.to_le_bytes());
            } else if operand.data_address().is_some() {
                // A data-object address (string-literal path): the imm64 is
                // relocated Absolute64 to the symbol's address.
                bytes.extend_from_slice(WIN64_ARG_MOV_IMM64_OPCODES[register_slot]);
                bytes.extend(0u64.to_le_bytes());
            } else if let Some(length) = operand.byte_length() {
                // A literal payload's byte length rides as a plain integer.
                bytes.extend_from_slice(imm_opcode);
                bytes.extend(
                    i32::try_from(length)
                        .map_err(|_| {
                            Diagnostic::error("X86_64 call byte-length argument exceeds i32")
                        })?
                        .to_le_bytes(),
                );
            } else {
                let argument = immediate_imm32(operands, arg_start + index, "call argument")?;
                bytes.extend_from_slice(imm_opcode);
                bytes.extend(argument.to_le_bytes());
            }
        } else {
            let stack_offset = match planned_location {
                Some(Win64ArgumentLocation::Stack(stack_offset)) => stack_offset,
                Some(Win64ArgumentLocation::Register(register)) => {
                    return Err(Diagnostic::error(format!(
                        "Microsoft x64 import parameter {index} could not marshal planned register {register:?}"
                    )));
                }
                None => (WIN64_STACK_ARG_HOME + 8 * (index - 4)) as u32,
            };
            let stack_disp8 = u8::try_from(stack_offset)
                .ok()
                .filter(|_| stack_offset <= 127)
                .ok_or_else(|| Diagnostic::error("X86_64 call supports at most 16 arguments"))?;
            if let Some((_, byte_offset, _)) = operand.runtime_scalar_integer() {
                append_mov_r11_imm64(bytes, 0); // relocated to the argument's region base
                bytes.extend([0x49, 0x8b, 0x83]); // mov rax, [r11+disp32]
                bytes.extend(disp32(byte_offset)?.to_le_bytes());
                bytes.extend([0x48, 0x89, 0x44, 0x24, stack_disp8]); // mov [rsp+o], rax
            } else if let Some((_, byte_offset)) = operand.runtime_storage_address() {
                append_mov_r11_imm64(bytes, 0); // relocated to the argument's region base
                bytes.extend([0x49, 0x8d, 0x83]); // lea rax, [r11+disp32]
                bytes.extend(disp32(byte_offset)?.to_le_bytes());
                bytes.extend([0x48, 0x89, 0x44, 0x24, stack_disp8]); // mov [rsp+o], rax
            } else if operand.data_address().is_some() {
                bytes.extend([0x48, 0xb8]); // mov rax, imm64 (relocated Absolute64)
                bytes.extend(0u64.to_le_bytes());
                bytes.extend([0x48, 0x89, 0x44, 0x24, stack_disp8]); // mov [rsp+o], rax
            } else if let Some(length) = operand.byte_length() {
                bytes.extend([0x48, 0xc7, 0x44, 0x24, stack_disp8]); // mov qword [rsp+o], imm32
                bytes.extend(
                    i32::try_from(length)
                        .map_err(|_| {
                            Diagnostic::error("X86_64 call byte-length argument exceeds i32")
                        })?
                        .to_le_bytes(),
                );
            } else {
                let argument = immediate_imm32(operands, arg_start + index, "call argument")?;
                bytes.extend([0x48, 0xc7, 0x44, 0x24, stack_disp8]); // mov qword [rsp+o], imm32
                bytes.extend(argument.to_le_bytes());
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Win64ArgumentLocation {
    Register(MachineRegister),
    Stack(u32),
}

fn win64_argument_location(
    placement: &ValuePlacement,
    index: usize,
) -> Result<Win64ArgumentLocation, Diagnostic> {
    match placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] if *byte_size == placement.shape.byte_size => {
            Ok(Win64ArgumentLocation::Register(*register))
        }
        [
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset: 0,
                byte_size,
                ..
            },
        ] if *byte_size == placement.shape.byte_size => {
            Ok(Win64ArgumentLocation::Stack(*stack_byte_offset))
        }
        locations => Err(Diagnostic::error(format!(
            "Microsoft x64 import parameter {index} has unsupported fragmented placement {locations:?}"
        ))),
    }
}

fn win64_argument_register_slot(register: MachineRegister) -> Option<usize> {
    match register {
        MachineRegister::X86Rcx => Some(0),
        MachineRegister::X86Rdx => Some(1),
        MachineRegister::X86R8 => Some(2),
        MachineRegister::X86R9 => Some(3),
        _ => None,
    }
}

fn encode_win64_import_call<T: InstructionOperandLike>(
    operands: &[T],
    returns_value: bool,
    dereferences_result: bool,
) -> Result<Vec<u8>, Diagnostic> {
    encode_win64_import_call_with_plan(operands, returns_value, dereferences_result, None)
}

fn encode_win64_import_call_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    returns_value: bool,
    dereferences_result: bool,
    authoritative_plan: Option<&CallPlan>,
) -> Result<Vec<u8>, Diagnostic> {
    if returns_value && operands.is_empty() {
        return Err(Diagnostic::error(
            "cannot encode X86_64 import call: the result storage place did not lower to a \
             runtime scalar operand",
        ));
    }
    let arg_start = usize::from(returns_value);
    let plan = if let Some(plan) = authoritative_plan {
        validate_win64_encoder_plan(plan)?;
        validate_win64_plan_operand_shapes(plan, operands, returns_value)?;
        plan.clone()
    } else {
        normalized_win64_import_plan(operands, returns_value)?
    };
    let indirect_result = plan.result.as_ref().is_some_and(win64_result_is_indirect);
    let result_register = if indirect_result {
        None
    } else {
        normalized_win64_result_register(&plan, returns_value)?
    };
    let reserve = win64_import_reserve_for_plan(&plan);
    let mut bytes = Vec::with_capacity(win64_import_call_width(
        operands,
        returns_value,
        dereferences_result,
    ));
    append_sub_rsp(&mut bytes, reserve);
    if indirect_result {
        append_win64_indirect_result_address(
            &mut bytes,
            &operands[0],
            plan.result.as_ref().expect("indirect result placement"),
        )?;
    }
    append_win64_call_arguments(&mut bytes, operands, arg_start, Some(&plan.parameters))?;
    bytes.extend([0xe8, 0, 0, 0, 0]); // call rel32 (relocated)
    append_add_rsp(&mut bytes, reserve);
    if dereferences_result {
        if result_register != Some(MachineRegister::X86Rax) {
            return Err(Diagnostic::error(format!(
                "Microsoft x64 pointer-result dereference requires rax, got {result_register:?}"
            )));
        }
        // The callee returned a POINTER to the real result (`_errno()` returns
        // `&errno`); deref once so the store tail writes the integer.
        bytes.extend([0x8b, 0x00]); // mov eax, [rax]
    }
    if returns_value && !indirect_result {
        append_win64_result_store(
            &mut bytes,
            &operands[0],
            "import call",
            plan.result.as_ref().expect("direct result placement"),
        )?;
    }
    debug_assert_eq!(
        bytes.len(),
        win64_import_call_width(operands, returns_value, dereferences_result)
    );
    Ok(bytes)
}

#[derive(Debug)]
struct SysvImportLayout {
    bytes: Vec<u8>,
    relocation_sites: Vec<X86_64RelocationSite>,
}

/// Encode a SysV AMD64 indirect call through a function pointer field on the
/// receiver. The receiver is the first wire argument and therefore must be
/// placed in `rdi` by the normalized plan.
pub fn encode_sysv_vtable_call<T: InstructionOperandLike>(
    operands: &[T],
    byte_offset: i64,
    result_present: bool,
) -> Result<Vec<u8>, Diagnostic> {
    encode_sysv_vtable_call_with_plan(operands, byte_offset, result_present, None)
}

pub fn encode_sysv_vtable_call_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    byte_offset: i64,
    result_present: bool,
    authoritative_plan: Option<&CallPlan>,
) -> Result<Vec<u8>, Diagnostic> {
    Ok(sysv_field_call_layout(
        operands,
        byte_offset,
        result_present,
        true,
        authoritative_plan,
    )?
    .bytes)
}

pub fn sysv_vtable_call_width<T: InstructionOperandLike>(
    operands: &[T],
    byte_offset: i64,
    result_present: bool,
) -> usize {
    sysv_vtable_call_width_with_plan(operands, byte_offset, result_present, None)
}

pub fn sysv_vtable_call_width_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    byte_offset: i64,
    result_present: bool,
    authoritative_plan: Option<&CallPlan>,
) -> usize {
    sysv_field_call_layout(
        operands,
        byte_offset,
        result_present,
        true,
        authoritative_plan,
    )
    .map(|layout| layout.bytes.len())
    .unwrap_or(0)
}

pub fn sysv_vtable_call_data_relocation_byte_offset<T: InstructionOperandLike>(
    operands: &[T],
    byte_offset: i64,
    result_present: bool,
    operand_index: usize,
) -> usize {
    sysv_vtable_call_data_relocation_byte_offset_with_plan(
        operands,
        byte_offset,
        result_present,
        operand_index,
        None,
    )
}

pub fn sysv_vtable_call_data_relocation_byte_offset_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    byte_offset: i64,
    result_present: bool,
    operand_index: usize,
    authoritative_plan: Option<&CallPlan>,
) -> usize {
    sysv_field_call_layout(
        operands,
        byte_offset,
        result_present,
        true,
        authoritative_plan,
    )
    .ok()
    .and_then(|layout| {
        layout
            .relocation_sites
            .into_iter()
            .find(|site| site.operand_index == Some(operand_index))
    })
    .map(|site| site.byte_offset)
    .unwrap_or(0)
}

/// Encode a SysV AMD64 service-table call. The table operand is used only to
/// find the callee; it is deliberately excluded from the wire signature.
pub fn encode_sysv_table_function_call<T: InstructionOperandLike>(
    operands: &[T],
    byte_offset: i64,
    result_present: bool,
) -> Result<Vec<u8>, Diagnostic> {
    encode_sysv_table_function_call_with_plan(operands, byte_offset, result_present, None)
}

pub fn encode_sysv_table_function_call_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    byte_offset: i64,
    result_present: bool,
    authoritative_plan: Option<&CallPlan>,
) -> Result<Vec<u8>, Diagnostic> {
    Ok(sysv_field_call_layout(
        operands,
        byte_offset,
        result_present,
        false,
        authoritative_plan,
    )?
    .bytes)
}

pub fn sysv_table_function_call_width<T: InstructionOperandLike>(
    operands: &[T],
    byte_offset: i64,
    result_present: bool,
) -> usize {
    sysv_table_function_call_width_with_plan(operands, byte_offset, result_present, None)
}

pub fn sysv_table_function_call_width_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    byte_offset: i64,
    result_present: bool,
    authoritative_plan: Option<&CallPlan>,
) -> usize {
    sysv_field_call_layout(
        operands,
        byte_offset,
        result_present,
        false,
        authoritative_plan,
    )
    .map(|layout| layout.bytes.len())
    .unwrap_or(0)
}

pub fn sysv_table_function_call_data_relocation_byte_offset<T: InstructionOperandLike>(
    operands: &[T],
    byte_offset: i64,
    result_present: bool,
    operand_index: usize,
) -> usize {
    sysv_table_function_call_data_relocation_byte_offset_with_plan(
        operands,
        byte_offset,
        result_present,
        operand_index,
        None,
    )
}

pub fn sysv_table_function_call_data_relocation_byte_offset_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    byte_offset: i64,
    result_present: bool,
    operand_index: usize,
    authoritative_plan: Option<&CallPlan>,
) -> usize {
    sysv_field_call_layout(
        operands,
        byte_offset,
        result_present,
        false,
        authoritative_plan,
    )
    .ok()
    .and_then(|layout| {
        layout
            .relocation_sites
            .into_iter()
            .find(|site| site.operand_index == Some(operand_index))
    })
    .map(|site| site.byte_offset)
    .unwrap_or(0)
}

fn sysv_field_call_layout<T: InstructionOperandLike>(
    operands: &[T],
    byte_offset: i64,
    result_present: bool,
    passes_receiver: bool,
    authoritative_plan: Option<&CallPlan>,
) -> Result<SysvImportLayout, Diagnostic> {
    let result_index = result_present.then_some(0);
    let dispatch_index = usize::from(result_present);
    let argument_start = if passes_receiver {
        dispatch_index
    } else {
        dispatch_index + 1
    };
    if operands.len() <= dispatch_index {
        return Err(Diagnostic::error(if passes_receiver {
            "cannot encode SysV AMD64 vtable call without its receiver"
        } else {
            "cannot encode SysV AMD64 table-function call without its dispatch table"
        }));
    }
    if !passes_receiver
        && !matches!(
            operands[dispatch_index].runtime_scalar_integer(),
            Some((_, _, 8))
        )
    {
        return Err(Diagnostic::error(
            "SysV AMD64 table-function dispatch table must be an eight-byte runtime scalar",
        ));
    }

    let signature = CallSignature {
        parameters: operands[argument_start..]
            .iter()
            .map(sysv_operand_shape)
            .collect::<Result<Vec<_>, _>>()?,
        result: result_index
            .map(|index| sysv_operand_shape(&operands[index]))
            .transpose()?,
    };
    let plan = if let Some(plan) = authoritative_plan {
        validate_call_plan(plan, &signature).map_err(|error| {
            Diagnostic::error(format!(
                "source-selected SysV AMD64 field-call plan does not match the lowered signature: {error}"
            ))
        })?;
        plan.clone()
    } else {
        evaluate_call_plan(CallingPolicy::SystemVAMD64, &signature).map_err(|error| {
            Diagnostic::error(format!(
                "cannot evaluate SysV AMD64 field-call plan: {error}"
            ))
        })?
    };
    validate_sysv_import_plan(&plan)?;

    let receiver_register = if passes_receiver {
        match plan
            .parameters
            .first()
            .map(|placement| placement.locations.as_slice())
        {
            Some(
                [
                    ValueLocation::Register {
                        register,
                        value_byte_offset: 0,
                        byte_size: 8,
                    },
                ],
            ) => Some(*register),
            _ => {
                return Err(Diagnostic::error(
                    "SysV AMD64 vtable call requires one full-width register receiver",
                ));
            }
        }
    } else {
        None
    };

    let stack_bytes = plan
        .parameters
        .iter()
        .flat_map(|placement| placement.locations.iter())
        .filter_map(|location| match location {
            ValueLocation::Stack {
                stack_byte_offset,
                byte_size,
                ..
            } => Some(usize::try_from(*stack_byte_offset).ok()? + usize::from(*byte_size)),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    let reserve = sysv_import_reserve(stack_bytes);
    let mut bytes = Vec::new();
    let mut relocation_sites = Vec::new();
    append_sub_rsp(&mut bytes, reserve);
    if let (Some(result_index), Some(result)) = (result_index, plan.result.as_ref())
        && sysv_result_is_indirect(result)
    {
        append_sysv_indirect_result_address(
            &mut bytes,
            &mut relocation_sites,
            &operands[result_index],
            result_index,
            result,
        )?;
    }
    for (parameter_index, placement) in plan.parameters.iter().enumerate() {
        let operand_index = argument_start + parameter_index;
        append_sysv_parameter(
            &mut bytes,
            &mut relocation_sites,
            &operands[operand_index],
            operand_index,
            placement,
        )?;
    }

    let field_disp = i32::try_from(byte_offset)
        .map_err(|_| Diagnostic::error("indirect field offset exceeds an imm32"))?;
    if passes_receiver {
        append_sysv_load_rax_from_base(
            &mut bytes,
            receiver_register.expect("validated receiver register"),
            field_disp,
        )?;
    } else {
        let (_, table_slot_offset, _) = operands[dispatch_index]
            .runtime_scalar_integer()
            .expect("validated table operand");
        append_sysv_runtime_base(&mut bytes, &mut relocation_sites, dispatch_index);
        bytes.extend([0x49, 0x8b, 0x83]); // mov rax, [r11 + disp32]
        bytes.extend(disp32(table_slot_offset)?.to_le_bytes());
        bytes.extend([0x48, 0x8b, 0x80]); // mov rax, [rax + disp32]
        bytes.extend(field_disp.to_le_bytes());
    }
    append_call_register(&mut bytes, 0);
    append_add_rsp(&mut bytes, reserve);

    if let Some(result_index) = result_index
        && !plan.result.as_ref().is_some_and(sysv_result_is_indirect)
    {
        append_sysv_result(
            &mut bytes,
            &mut relocation_sites,
            &operands[result_index],
            plan.result.as_ref().ok_or_else(|| {
                Diagnostic::error("SysV AMD64 field-call plan omitted its required result")
            })?,
        )?;
    }
    Ok(SysvImportLayout {
        bytes,
        relocation_sites,
    })
}

/// The normalized SysV AMD64 import slice. Provides-authored calls may carry
/// four/eight-byte integer or float scalars, pointers, and pure-INTEGER records
/// of at most two eightbytes whose fragments are four/eight bytes. The
/// evaluated plan owns the independent GPR/XMM banks, whole-value stack
/// rollback, and `rax`/`rdx`/`xmm0` results; this encoder only realizes those
/// locations. Vector and mixed-class aggregate cases stay closed.
fn encode_sysv_import_call<T: InstructionOperandLike>(
    operands: &[T],
    returns_value: bool,
) -> Result<Vec<u8>, Diagnostic> {
    Ok(sysv_import_layout(operands, returns_value)?.bytes)
}

fn sysv_import_layout<T: InstructionOperandLike>(
    operands: &[T],
    returns_value: bool,
) -> Result<SysvImportLayout, Diagnostic> {
    sysv_import_layout_with_plan(operands, returns_value, None)
}

fn sysv_import_layout_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    returns_value: bool,
    authoritative_plan: Option<&CallPlan>,
) -> Result<SysvImportLayout, Diagnostic> {
    if returns_value && operands.is_empty() {
        return Err(Diagnostic::error(
            "cannot encode SysV AMD64 import call without its result storage operand",
        ));
    }
    let arg_start = usize::from(returns_value);
    let plan = if let Some(plan) = authoritative_plan {
        validate_sysv_import_plan(plan)?;
        validate_sysv_plan_operand_shapes(plan, operands, returns_value)?;
        plan.clone()
    } else {
        normalized_sysv_import_plan(operands, returns_value)?
    };
    let stack_bytes = plan
        .parameters
        .iter()
        .flat_map(|placement| placement.locations.iter())
        .filter_map(|location| match location {
            ValueLocation::Stack {
                stack_byte_offset,
                byte_size,
                ..
            } => Some(usize::try_from(*stack_byte_offset).ok()? + usize::from(*byte_size)),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    let reserve = sysv_import_reserve(stack_bytes);
    let mut bytes = Vec::new();
    let mut relocation_sites = Vec::new();
    append_sub_rsp(&mut bytes, reserve);

    if returns_value
        && let Some(result) = plan.result.as_ref()
        && sysv_result_is_indirect(result)
    {
        append_sysv_indirect_result_address(
            &mut bytes,
            &mut relocation_sites,
            &operands[0],
            0,
            result,
        )?;
    }

    for (parameter_index, placement) in plan.parameters.iter().enumerate() {
        append_sysv_parameter(
            &mut bytes,
            &mut relocation_sites,
            &operands[arg_start + parameter_index],
            arg_start + parameter_index,
            placement,
        )?;
    }

    relocation_sites.push(X86_64RelocationSite {
        operand_index: None,
        byte_offset: bytes.len() + 1,
        byte_width: 4,
        kind: X86_64RelocationSiteKind::Relative32,
    });
    bytes.extend([0xe8, 0, 0, 0, 0]);
    append_add_rsp(&mut bytes, reserve);

    if returns_value && !plan.result.as_ref().is_some_and(sysv_result_is_indirect) {
        append_sysv_result(
            &mut bytes,
            &mut relocation_sites,
            &operands[0],
            plan.result.as_ref().ok_or_else(|| {
                Diagnostic::error("SysV AMD64 import plan omitted its required result")
            })?,
        )?;
    }

    Ok(SysvImportLayout {
        bytes,
        relocation_sites,
    })
}

fn sysv_import_reserve(stack_bytes: usize) -> usize {
    // Emitted Omega call sites enter with rsp == 8 (mod 16). Reserve the
    // smallest area that covers every outgoing stack slot and leaves rsp
    // 16-byte aligned immediately before CALL.
    (stack_bytes + 8).next_multiple_of(16) - 8
}

fn append_sysv_parameter<T: InstructionOperandLike>(
    bytes: &mut Vec<u8>,
    relocation_sites: &mut Vec<X86_64RelocationSite>,
    operand: &T,
    operand_index: usize,
    placement: &ValuePlacement,
) -> Result<(), Diagnostic> {
    if let Some((_, byte_offset, byte_count, _, sse_eightbytes)) =
        operand.runtime_system_v_aggregate()
    {
        if byte_count != usize::from(placement.shape.byte_size)
            || !matches!(sse_eightbytes, 0b01 | 0b10 | 0b11)
        {
            return Err(Diagnostic::error(format!(
                "SysV AMD64 classified aggregate operand {operand_index} disagrees with its plan"
            )));
        }
        append_sysv_runtime_base(bytes, relocation_sites, operand_index);
        for location in &placement.locations {
            match *location {
                ValueLocation::Register {
                    register,
                    value_byte_offset,
                    byte_size,
                } => {
                    let source_offset = byte_offset + usize::from(value_byte_offset);
                    if matches!(register, MachineRegister::X86Xmm(_)) {
                        append_x86_load_float_from_r11(
                            bytes,
                            register,
                            source_offset,
                            usize::from(byte_size),
                        )?;
                    } else {
                        append_sysv_load_register_from_r11(
                            bytes,
                            register,
                            source_offset,
                            byte_size,
                        )?;
                    }
                }
                ValueLocation::Stack {
                    stack_byte_offset,
                    value_byte_offset,
                    byte_size,
                    ..
                } => {
                    append_sysv_load_rax_from_r11(
                        bytes,
                        byte_offset + usize::from(value_byte_offset),
                        byte_size,
                    )?;
                    append_sysv_store_rax_to_rsp(bytes, stack_byte_offset)?;
                }
                ValueLocation::Indirect { .. } => {
                    return Err(Diagnostic::error(
                        "SysV AMD64 classified aggregate received an indirect placement",
                    ));
                }
            }
        }
        return Ok(());
    }
    if let Some((_, byte_offset, member_byte_count, members)) =
        operand.runtime_homogeneous_float_aggregate()
    {
        if member_byte_count * usize::from(members) != usize::from(placement.shape.byte_size) {
            return Err(Diagnostic::error(format!(
                "SysV AMD64 float aggregate operand {operand_index} disagrees with its plan width"
            )));
        }
        append_sysv_runtime_base(bytes, relocation_sites, operand_index);
        for location in &placement.locations {
            match *location {
                ValueLocation::Register {
                    register,
                    value_byte_offset,
                    byte_size,
                } => append_x86_load_float_from_r11(
                    bytes,
                    register,
                    byte_offset + usize::from(value_byte_offset),
                    usize::from(byte_size),
                )?,
                ValueLocation::Stack {
                    stack_byte_offset,
                    value_byte_offset,
                    byte_size,
                    ..
                } => {
                    append_sysv_load_rax_from_r11(
                        bytes,
                        byte_offset + usize::from(value_byte_offset),
                        byte_size,
                    )?;
                    append_sysv_store_rax_to_rsp(bytes, stack_byte_offset)?;
                }
                ValueLocation::Indirect { .. } => {
                    return Err(Diagnostic::error(
                        "SysV AMD64 float aggregate received an indirect placement",
                    ));
                }
            }
        }
        return Ok(());
    }
    if let Some((_, byte_offset, byte_count)) = operand.runtime_scalar_float() {
        let [location] = placement.locations.as_slice() else {
            return Err(Diagnostic::error(format!(
                "SysV AMD64 float operand {operand_index} has fragmented placement {:?}",
                placement.locations
            )));
        };
        append_sysv_runtime_base(bytes, relocation_sites, operand_index);
        return match *location {
            ValueLocation::Register { register, .. } => {
                append_x86_load_float_from_r11(bytes, register, byte_offset, byte_count)
            }
            ValueLocation::Stack {
                stack_byte_offset, ..
            } => {
                append_sysv_load_rax_from_r11(
                    bytes,
                    byte_offset,
                    u16::try_from(byte_count)
                        .map_err(|_| Diagnostic::error("SysV AMD64 float width exceeds u16"))?,
                )?;
                append_sysv_store_rax_to_rsp(bytes, stack_byte_offset)
            }
            ValueLocation::Indirect { .. } => Err(Diagnostic::error(
                "SysV AMD64 scalar float import received an indirect placement",
            )),
        };
    }
    if let Some((_, byte_offset, byte_count, _)) = operand
        .runtime_small_aggregate()
        .or_else(|| operand.runtime_large_aggregate())
    {
        if byte_count != usize::from(placement.shape.byte_size) {
            return Err(Diagnostic::error(format!(
                "SysV AMD64 aggregate operand {operand_index} width {byte_count} disagrees with plan width {}",
                placement.shape.byte_size
            )));
        }
        append_sysv_runtime_base(bytes, relocation_sites, operand_index);
        for location in &placement.locations {
            match *location {
                ValueLocation::Register {
                    register,
                    value_byte_offset,
                    byte_size,
                } => append_sysv_load_register_from_r11(
                    bytes,
                    register,
                    byte_offset + usize::from(value_byte_offset),
                    byte_size,
                )?,
                ValueLocation::Stack {
                    stack_byte_offset,
                    value_byte_offset,
                    byte_size,
                    ..
                } => {
                    append_sysv_load_rax_from_r11(
                        bytes,
                        byte_offset + usize::from(value_byte_offset),
                        byte_size,
                    )?;
                    append_sysv_store_rax_to_rsp(bytes, stack_byte_offset)?;
                }
                ValueLocation::Indirect { .. } => {
                    return Err(Diagnostic::error(
                        "SysV AMD64 small-aggregate import received an indirect placement",
                    ));
                }
            }
        }
        return Ok(());
    }

    let [location] = placement.locations.as_slice() else {
        return Err(Diagnostic::error(format!(
            "SysV AMD64 scalar import operand {operand_index} has fragmented placement {:?}",
            placement.locations
        )));
    };
    match *location {
        ValueLocation::Register { register, .. } => append_sysv_scalar_to_register(
            bytes,
            relocation_sites,
            operand,
            operand_index,
            register,
        ),
        ValueLocation::Stack {
            stack_byte_offset, ..
        } => append_sysv_scalar_to_stack(
            bytes,
            relocation_sites,
            operand,
            operand_index,
            stack_byte_offset,
        ),
        ValueLocation::Indirect { .. } => Err(Diagnostic::error(
            "SysV AMD64 scalar import received an indirect placement",
        )),
    }
}

fn append_sysv_scalar_to_register<T: InstructionOperandLike>(
    bytes: &mut Vec<u8>,
    relocation_sites: &mut Vec<X86_64RelocationSite>,
    operand: &T,
    operand_index: usize,
    register: MachineRegister,
) -> Result<(), Diagnostic> {
    if let Some((_, byte_offset, byte_count)) = operand.runtime_scalar_integer() {
        append_sysv_runtime_base(bytes, relocation_sites, operand_index);
        return append_sysv_load_register_from_r11(
            bytes,
            register,
            byte_offset,
            u16::try_from(byte_count)
                .map_err(|_| Diagnostic::error("SysV AMD64 scalar width exceeds u16"))?,
        );
    }
    if let Some((_, byte_offset)) = operand.runtime_storage_address() {
        append_sysv_runtime_base(bytes, relocation_sites, operand_index);
        return append_sysv_lea_register_from_r11(bytes, register, byte_offset);
    }
    if let Some((_, byte_offset)) = operand.runtime_string_pointer() {
        append_sysv_runtime_base(bytes, relocation_sites, operand_index);
        return if operand.runtime_string_is_bounded_buffer() {
            append_sysv_lea_register_from_r11(bytes, register, byte_offset + 8)
        } else {
            append_sysv_load_register_from_r11(bytes, register, byte_offset, 8)
        };
    }
    if operand.data_address().is_some() {
        relocation_sites.push(X86_64RelocationSite {
            operand_index: Some(operand_index),
            byte_offset: bytes.len() + 2,
            byte_width: 8,
            kind: X86_64RelocationSiteKind::Absolute64,
        });
        return append_sysv_mov_register_imm64(bytes, register, 0);
    }
    if let Some(value) = operand.immediate_integer().or_else(|| {
        operand
            .byte_length()
            .and_then(|value| i64::try_from(value).ok())
    }) {
        return append_sysv_mov_register_imm64(bytes, register, value as u64);
    }
    Err(Diagnostic::error(format!(
        "SysV AMD64 import operand {operand_index} has no supported integer representation"
    )))
}

fn append_sysv_scalar_to_stack<T: InstructionOperandLike>(
    bytes: &mut Vec<u8>,
    relocation_sites: &mut Vec<X86_64RelocationSite>,
    operand: &T,
    operand_index: usize,
    stack_byte_offset: u32,
) -> Result<(), Diagnostic> {
    if let Some((_, byte_offset, byte_count)) = operand.runtime_scalar_integer() {
        append_sysv_runtime_base(bytes, relocation_sites, operand_index);
        append_sysv_load_rax_from_r11(
            bytes,
            byte_offset,
            u16::try_from(byte_count)
                .map_err(|_| Diagnostic::error("SysV AMD64 scalar width exceeds u16"))?,
        )?;
        return append_sysv_store_rax_to_rsp(bytes, stack_byte_offset);
    }
    if let Some((_, byte_offset)) = operand.runtime_storage_address() {
        append_sysv_runtime_base(bytes, relocation_sites, operand_index);
        append_sysv_lea_register_from_r11(bytes, MachineRegister::X86Rax, byte_offset)?;
        return append_sysv_store_rax_to_rsp(bytes, stack_byte_offset);
    }
    if operand.data_address().is_some() {
        relocation_sites.push(X86_64RelocationSite {
            operand_index: Some(operand_index),
            byte_offset: bytes.len() + 2,
            byte_width: 8,
            kind: X86_64RelocationSiteKind::Absolute64,
        });
        append_mov_rax_imm64(bytes, 0);
        return append_sysv_store_rax_to_rsp(bytes, stack_byte_offset);
    }
    if let Some(value) = operand.immediate_integer().or_else(|| {
        operand
            .byte_length()
            .and_then(|value| i64::try_from(value).ok())
    }) {
        append_mov_rax_imm64(bytes, value as u64);
        return append_sysv_store_rax_to_rsp(bytes, stack_byte_offset);
    }
    Err(Diagnostic::error(format!(
        "SysV AMD64 stack operand {operand_index} has no supported integer representation"
    )))
}

fn append_sysv_result<T: InstructionOperandLike>(
    bytes: &mut Vec<u8>,
    relocation_sites: &mut Vec<X86_64RelocationSite>,
    operand: &T,
    placement: &ValuePlacement,
) -> Result<(), Diagnostic> {
    if let Some((_, byte_offset, byte_count, _, sse_eightbytes)) =
        operand.runtime_system_v_aggregate()
    {
        if byte_count != usize::from(placement.shape.byte_size)
            || !matches!(sse_eightbytes, 0b01 | 0b10 | 0b11)
        {
            return Err(Diagnostic::error(
                "SysV AMD64 classified aggregate result disagrees with its plan",
            ));
        }
        append_sysv_runtime_base(bytes, relocation_sites, 0);
        for location in &placement.locations {
            let ValueLocation::Register {
                register,
                value_byte_offset,
                byte_size,
            } = *location
            else {
                return Err(Diagnostic::error(
                    "SysV AMD64 classified aggregate result is not register-resident",
                ));
            };
            let destination_offset = byte_offset + usize::from(value_byte_offset);
            if matches!(register, MachineRegister::X86Xmm(_)) {
                append_x86_store_float_to_r11(
                    bytes,
                    register,
                    destination_offset,
                    usize::from(byte_size),
                )?;
            } else {
                append_sysv_store_result_register_to_r11(
                    bytes,
                    register,
                    destination_offset,
                    byte_size,
                )?;
            }
        }
        return Ok(());
    }
    if let Some((_, byte_offset, member_byte_count, members)) =
        operand.runtime_homogeneous_float_aggregate()
    {
        if member_byte_count * usize::from(members) != usize::from(placement.shape.byte_size) {
            return Err(Diagnostic::error(
                "SysV AMD64 float aggregate result disagrees with its plan width",
            ));
        }
        append_sysv_runtime_base(bytes, relocation_sites, 0);
        for location in &placement.locations {
            let ValueLocation::Register {
                register,
                value_byte_offset,
                byte_size,
            } = *location
            else {
                return Err(Diagnostic::error(
                    "SysV AMD64 float aggregate result is not register-resident",
                ));
            };
            append_x86_store_float_to_r11(
                bytes,
                register,
                byte_offset + usize::from(value_byte_offset),
                usize::from(byte_size),
            )?;
        }
        return Ok(());
    }
    if let Some((_, byte_offset, byte_count)) = operand.runtime_scalar_float() {
        if byte_count != usize::from(placement.shape.byte_size) {
            return Err(Diagnostic::error(
                "SysV AMD64 float result storage disagrees with the normalized result width",
            ));
        }
        let [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] = placement.locations.as_slice()
        else {
            return Err(Diagnostic::error(format!(
                "SysV AMD64 float result has unsupported placement {:?}",
                placement.locations
            )));
        };
        append_sysv_runtime_base(bytes, relocation_sites, 0);
        return append_x86_store_float_to_r11(
            bytes,
            *register,
            byte_offset,
            usize::from(*byte_size),
        );
    }
    let (byte_offset, byte_count, aggregate) =
        if let Some((_, offset, count, _)) = operand.runtime_small_aggregate() {
            (offset, count, true)
        } else if let Some((_, offset, count)) = operand.runtime_scalar_integer() {
            (offset, count, false)
        } else {
            return Err(Diagnostic::error(
                "SysV AMD64 import result did not lower to integer runtime storage",
            ));
        };
    if byte_count != usize::from(placement.shape.byte_size) {
        return Err(Diagnostic::error(
            "SysV AMD64 import result storage disagrees with the normalized result width",
        ));
    }
    if !aggregate && placement.locations.len() != 1 {
        return Err(Diagnostic::error(
            "SysV AMD64 scalar import result has fragmented placement",
        ));
    }
    append_sysv_runtime_base(bytes, relocation_sites, 0);
    for location in &placement.locations {
        let ValueLocation::Register {
            register,
            value_byte_offset,
            byte_size,
        } = *location
        else {
            return Err(Diagnostic::error(
                "SysV AMD64 import result is not register-resident",
            ));
        };
        append_sysv_store_result_register_to_r11(
            bytes,
            register,
            byte_offset + usize::from(value_byte_offset),
            byte_size,
        )?;
    }
    Ok(())
}

fn sysv_result_is_indirect(placement: &ValuePlacement) -> bool {
    matches!(
        placement.locations.as_slice(),
        [ValueLocation::Indirect { .. }]
    )
}

fn append_sysv_indirect_result_address<T: InstructionOperandLike>(
    bytes: &mut Vec<u8>,
    relocation_sites: &mut Vec<X86_64RelocationSite>,
    operand: &T,
    operand_index: usize,
    placement: &ValuePlacement,
) -> Result<(), Diagnostic> {
    let [
        ValueLocation::Indirect {
            pointer: omega_calling_conventions::IndirectPointerLocation::Register(register),
            copy_stack_byte_offset: None,
            byte_size,
            alignment,
        },
    ] = placement.locations.as_slice()
    else {
        return Err(Diagnostic::error(
            "SysV AMD64 indirect result has an unsupported pointer placement",
        ));
    };
    let Some((_, byte_offset, operand_byte_size, operand_alignment)) =
        operand.runtime_large_aggregate()
    else {
        return Err(Diagnostic::error(
            "SysV AMD64 indirect result did not lower to large-aggregate runtime storage",
        ));
    };
    if operand_byte_size != usize::from(*byte_size) || operand_alignment != usize::from(*alignment)
    {
        return Err(Diagnostic::error(
            "SysV AMD64 indirect result storage disagrees with the normalized result shape",
        ));
    }
    append_sysv_runtime_base(bytes, relocation_sites, operand_index);
    append_sysv_lea_register_from_r11(bytes, *register, byte_offset)
}

fn append_sysv_load_rax_from_base(
    bytes: &mut Vec<u8>,
    base: MachineRegister,
    displacement: i32,
) -> Result<(), Diagnostic> {
    let (rex, modrm) = match base {
        MachineRegister::X86Rdi => (0x48, 0x87),
        MachineRegister::X86Rsi => (0x48, 0x86),
        MachineRegister::X86Rdx => (0x48, 0x82),
        MachineRegister::X86Rcx => (0x48, 0x81),
        MachineRegister::X86R8 => (0x49, 0x80),
        MachineRegister::X86R9 => (0x49, 0x81),
        _ => {
            return Err(Diagnostic::error(format!(
                "SysV AMD64 vtable receiver register {base:?} is not encodable"
            )));
        }
    };
    bytes.extend([rex, 0x8b, modrm]);
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_sysv_runtime_base(
    bytes: &mut Vec<u8>,
    relocation_sites: &mut Vec<X86_64RelocationSite>,
    operand_index: usize,
) {
    relocation_sites.push(X86_64RelocationSite {
        operand_index: Some(operand_index),
        byte_offset: bytes.len() + 2,
        byte_width: 8,
        kind: X86_64RelocationSiteKind::Absolute64,
    });
    append_mov_r11_imm64(bytes, 0);
}

fn normalized_sysv_import_plan<T: InstructionOperandLike>(
    operands: &[T],
    returns_value: bool,
) -> Result<CallPlan, Diagnostic> {
    let arg_start = usize::from(returns_value);
    let signature = CallSignature {
        parameters: operands[arg_start..]
            .iter()
            .map(sysv_operand_shape)
            .collect::<Result<Vec<_>, _>>()?,
        result: returns_value
            .then(|| sysv_operand_shape(&operands[0]))
            .transpose()?,
    };
    let plan = evaluate_call_plan(CallingPolicy::SystemVAMD64, &signature).map_err(|error| {
        Diagnostic::error(format!("cannot evaluate SysV AMD64 import plan: {error}"))
    })?;
    validate_sysv_import_plan(&plan)?;
    Ok(plan)
}

fn validate_sysv_plan_operand_shapes<T: InstructionOperandLike>(
    plan: &CallPlan,
    operands: &[T],
    returns_value: bool,
) -> Result<(), Diagnostic> {
    let arg_start = usize::from(returns_value);
    let parameter_shapes = operands
        .get(arg_start..)
        .ok_or_else(|| Diagnostic::error("SysV AMD64 authored import has no arguments"))?
        .iter()
        .map(sysv_operand_shape)
        .collect::<Result<Vec<_>, _>>()?;
    let result_shape = if returns_value {
        Some(sysv_operand_shape(operands.first().ok_or_else(|| {
            Diagnostic::error("SysV AMD64 authored import has no result operand")
        })?)?)
    } else {
        None
    };
    if plan.parameters.len() != parameter_shapes.len()
        || plan
            .parameters
            .iter()
            .map(|placement| placement.shape)
            .ne(parameter_shapes)
        || plan.result.as_ref().map(|placement| placement.shape) != result_shape
    {
        return Err(Diagnostic::error(
            "SysV AMD64 source calling plan does not match the selected authored import operands",
        ));
    }
    Ok(())
}

fn sysv_operand_shape<T: InstructionOperandLike>(operand: &T) -> Result<ValueShape, Diagnostic> {
    if let Some((_, _, byte_count, alignment, sse_eightbytes)) =
        operand.runtime_system_v_aggregate()
    {
        if !matches!(byte_count, 9..=16) || !matches!(sse_eightbytes, 0b01 | 0b10 | 0b11) {
            return Err(Diagnostic::error(
                "SysV AMD64 classified aggregates require 9-16 bytes and at least one SSE eightbyte",
            ));
        }
        let class = |index: u8| {
            if sse_eightbytes & (1u8 << index) == 0 {
                SystemVEightbyteClass::Integer
            } else {
                SystemVEightbyteClass::Sse
            }
        };
        return Ok(ValueShape::system_v_aggregate(
            u16::try_from(byte_count)
                .map_err(|_| Diagnostic::error("SysV AMD64 mixed aggregate width exceeds u16"))?,
            u16::try_from(alignment).map_err(|_| {
                Diagnostic::error("SysV AMD64 mixed aggregate alignment exceeds u16")
            })?,
            class(0),
            class(1),
        ));
    }
    if let Some((_, _, member_byte_count, members)) = operand.runtime_homogeneous_float_aggregate()
    {
        if !matches!(member_byte_count, 4 | 8)
            || !(2..=4).contains(&members)
            || member_byte_count * usize::from(members) > 16
        {
            return Err(Diagnostic::error(
                "SysV AMD64 float aggregates require two to four f32/f64 members totaling at most 16 bytes",
            ));
        }
        return Ok(ValueShape::homogeneous_float_aggregate(
            u16::try_from(member_byte_count)
                .map_err(|_| Diagnostic::error("SysV AMD64 float member width exceeds u16"))?,
            members,
        ));
    }
    if let Some((_, _, byte_count)) = operand.runtime_scalar_float() {
        let byte_count = u16::try_from(byte_count)
            .map_err(|_| Diagnostic::error("SysV AMD64 float width exceeds u16"))?;
        return Ok(ValueShape::float(byte_count));
    }
    if let Some((_, _, byte_count, alignment)) = operand
        .runtime_small_aggregate()
        .or_else(|| operand.runtime_large_aggregate())
    {
        let byte_count = u16::try_from(byte_count)
            .map_err(|_| Diagnostic::error("SysV AMD64 aggregate width exceeds u16"))?;
        let alignment = u16::try_from(alignment)
            .map_err(|_| Diagnostic::error("SysV AMD64 aggregate alignment exceeds u16"))?;
        if byte_count == 0 {
            return Err(Diagnostic::error(
                "SysV AMD64 aggregate calls require a nonzero value width",
            ));
        }
        return Ok(ValueShape::integer(byte_count, alignment));
    }
    if let Some((_, _, byte_count)) = operand.runtime_scalar_integer() {
        let byte_count = u16::try_from(byte_count)
            .map_err(|_| Diagnostic::error("SysV AMD64 integer width exceeds u16"))?;
        return Ok(ValueShape::integer(byte_count, byte_count.max(1)));
    }
    if operand.data_address().is_some()
        || operand.runtime_string_pointer().is_some()
        || operand.runtime_storage_address().is_some()
        || operand.immediate_integer().is_some()
        || operand.byte_length().is_some()
    {
        return Ok(ValueShape::integer(8, 8));
    }
    Err(Diagnostic::error(
        "SysV AMD64 authored import operand has no supported integer/pointer shape",
    ))
}

fn validate_sysv_import_plan(plan: &CallPlan) -> Result<(), Diagnostic> {
    if plan.policy != CallingPolicy::SystemVAMD64
        || plan.entry_control != EntryControl::CallReturn
        || plan.stack_alignment != 16
        || plan.shadow_bytes != 0
    {
        return Err(Diagnostic::error(format!(
            "SysV AMD64 import encoder cannot realize plan policy={:?}, control={:?}, alignment={}, shadow_bytes={}",
            plan.policy, plan.entry_control, plan.stack_alignment, plan.shadow_bytes
        )));
    }
    for scratch in [MachineRegister::X86Rax, MachineRegister::X86R11] {
        if !plan.ordinary_clobbers.contains(scratch) {
            return Err(Diagnostic::error(format!(
                "SysV AMD64 encoder scratch register {scratch:?} exceeds the plan's ordinary-clobber ceiling"
            )));
        }
    }
    let unsupported_parameter = plan.parameters.iter().any(|placement| {
        !matches!(
            placement.shape.class,
            ValueClass::Integer
                | ValueClass::Float
                | ValueClass::HomogeneousFloatAggregate { members: 2..=4 }
                | ValueClass::SystemVAggregate { .. }
        ) || (placement.shape.byte_size > 16
            && placement
                .locations
                .iter()
                .any(|location| !matches!(location, ValueLocation::Stack { .. })))
            || placement
                .locations
                .iter()
                .any(|location| matches!(location, ValueLocation::Indirect { .. }))
    });
    let unsupported_result = plan.result.as_ref().is_some_and(|placement| {
        !matches!(
            placement.shape.class,
            ValueClass::Integer
                | ValueClass::Float
                | ValueClass::HomogeneousFloatAggregate { members: 2..=4 }
                | ValueClass::SystemVAggregate { .. }
        ) || (placement.shape.byte_size > 16
            && !matches!(
                placement.locations.as_slice(),
                [ValueLocation::Indirect {
                    pointer: omega_calling_conventions::IndirectPointerLocation::Register(
                        MachineRegister::X86Rdi
                    ),
                    copy_stack_byte_offset: None,
                    ..
                }]
            ))
    });
    if unsupported_parameter || unsupported_result {
        return Err(Diagnostic::error(
            "SysV AMD64 import plan contains an unsupported aggregate class or indirect placement",
        ));
    }
    Ok(())
}

fn append_sysv_mov_register_imm64(
    bytes: &mut Vec<u8>,
    register: MachineRegister,
    value: u64,
) -> Result<(), Diagnostic> {
    let opcode = match register {
        MachineRegister::X86Rax => [0x48, 0xb8],
        MachineRegister::X86Rcx => [0x48, 0xb9],
        MachineRegister::X86Rdx => [0x48, 0xba],
        MachineRegister::X86Rsi => [0x48, 0xbe],
        MachineRegister::X86Rdi => [0x48, 0xbf],
        MachineRegister::X86R8 => [0x49, 0xb8],
        MachineRegister::X86R9 => [0x49, 0xb9],
        _ => {
            return Err(Diagnostic::error(format!(
                "SysV AMD64 import cannot materialize argument register {register:?}"
            )));
        }
    };
    bytes.extend(opcode);
    bytes.extend(value.to_le_bytes());
    Ok(())
}

fn append_x86_load_float_from_r11(
    bytes: &mut Vec<u8>,
    register: MachineRegister,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let MachineRegister::X86Xmm(index @ 0..=7) = register else {
        return Err(Diagnostic::error(format!(
            "X86_64 call cannot load float argument register {register:?}"
        )));
    };
    let prefix = match byte_size {
        4 => 0xf3,
        8 => 0xf2,
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 scalar float width {byte_size} is not encodable"
            )));
        }
    };
    bytes.extend([prefix, 0x41, 0x0f, 0x10, 0x83 | (index << 3)]);
    bytes.extend(disp32(byte_offset)?.to_le_bytes());
    Ok(())
}

fn append_x86_store_float_to_r11(
    bytes: &mut Vec<u8>,
    register: MachineRegister,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let MachineRegister::X86Xmm(index @ 0..=7) = register else {
        return Err(Diagnostic::error(format!(
            "X86_64 call cannot store float result register {register:?}"
        )));
    };
    let prefix = match byte_size {
        4 => 0xf3,
        8 => 0xf2,
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 scalar float result width {byte_size} is not encodable"
            )));
        }
    };
    bytes.extend([prefix, 0x41, 0x0f, 0x11, 0x83 | (index << 3)]);
    bytes.extend(disp32(byte_offset)?.to_le_bytes());
    Ok(())
}

fn append_sysv_load_register_from_r11(
    bytes: &mut Vec<u8>,
    register: MachineRegister,
    byte_offset: usize,
    byte_size: u16,
) -> Result<(), Diagnostic> {
    let modrm = match register {
        MachineRegister::X86Rax => 0x83,
        MachineRegister::X86Rcx => 0x8b,
        MachineRegister::X86Rdx => 0x93,
        MachineRegister::X86Rsi => 0xb3,
        MachineRegister::X86Rdi => 0xbb,
        MachineRegister::X86R8 => 0x83,
        MachineRegister::X86R9 => 0x8b,
        _ => {
            return Err(Diagnostic::error(format!(
                "SysV AMD64 import cannot load argument register {register:?}"
            )));
        }
    };
    let rex = match (byte_size, register) {
        (8, MachineRegister::X86R8 | MachineRegister::X86R9) => 0x4d,
        (8, _) => 0x49,
        (4, MachineRegister::X86R8 | MachineRegister::X86R9) => 0x45,
        (4, _) => 0x41,
        _ => {
            return Err(Diagnostic::error(format!(
                "SysV AMD64 integer fragment width {byte_size} is not yet encodable"
            )));
        }
    };
    bytes.extend([rex, 0x8b, modrm]);
    bytes.extend(disp32(byte_offset)?.to_le_bytes());
    Ok(())
}

fn append_sysv_lea_register_from_r11(
    bytes: &mut Vec<u8>,
    register: MachineRegister,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    let modrm = match register {
        MachineRegister::X86Rax => 0x83,
        MachineRegister::X86Rcx => 0x8b,
        MachineRegister::X86Rdx => 0x93,
        MachineRegister::X86Rsi => 0xb3,
        MachineRegister::X86Rdi => 0xbb,
        MachineRegister::X86R8 => 0x83,
        MachineRegister::X86R9 => 0x8b,
        _ => {
            return Err(Diagnostic::error(format!(
                "SysV AMD64 import cannot address argument register {register:?}"
            )));
        }
    };
    let rex = if matches!(register, MachineRegister::X86R8 | MachineRegister::X86R9) {
        0x4d
    } else {
        0x49
    };
    bytes.extend([rex, 0x8d, modrm]);
    bytes.extend(disp32(byte_offset)?.to_le_bytes());
    Ok(())
}

fn append_sysv_load_rax_from_r11(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    byte_size: u16,
) -> Result<(), Diagnostic> {
    append_sysv_load_register_from_r11(bytes, MachineRegister::X86Rax, byte_offset, byte_size)
}

fn append_sysv_store_rax_to_rsp(
    bytes: &mut Vec<u8>,
    stack_byte_offset: u32,
) -> Result<(), Diagnostic> {
    let displacement = i32::try_from(stack_byte_offset)
        .map_err(|_| Diagnostic::error("SysV AMD64 stack offset exceeds i32"))?;
    bytes.extend([0x48, 0x89, 0x84, 0x24]);
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_sysv_store_result_register_to_r11(
    bytes: &mut Vec<u8>,
    register: MachineRegister,
    byte_offset: usize,
    byte_size: u16,
) -> Result<(), Diagnostic> {
    let modrm = match register {
        MachineRegister::X86Rax => 0x83,
        MachineRegister::X86Rdx => 0x93,
        _ => {
            return Err(Diagnostic::error(format!(
                "SysV AMD64 import cannot store result register {register:?}"
            )));
        }
    };
    let rex = match byte_size {
        8 => 0x49,
        4 => 0x41,
        _ => {
            return Err(Diagnostic::error(format!(
                "SysV AMD64 result fragment width {byte_size} is not yet encodable"
            )));
        }
    };
    bytes.extend([rex, 0x89, modrm]);
    bytes.extend(disp32(byte_offset)?.to_le_bytes());
    Ok(())
}

/// ENT2c compatibility seam: evaluate the Microsoft x64 plan from the selected
/// operands before the general import encoder marshals anything. Register and
/// shadow-relative stack placements are passed into the marshaller verbatim;
/// unsupported vector/fragmented shapes fail closed.
fn normalized_win64_import_plan<T: InstructionOperandLike>(
    operands: &[T],
    returns_value: bool,
) -> Result<CallPlan, Diagnostic> {
    let arg_start = usize::from(returns_value);
    normalized_win64_call_plan(operands, returns_value.then_some(0), arg_start)
}

fn normalized_win64_call_plan<T: InstructionOperandLike>(
    operands: &[T],
    result_index: Option<usize>,
    arg_start: usize,
) -> Result<CallPlan, Diagnostic> {
    let result = if let Some(result_index) = result_index {
        Some(win64_operand_shape(
            operands.get(result_index).ok_or_else(|| {
                Diagnostic::error("Microsoft x64 call result index is out of range")
            })?,
        )?)
    } else {
        None
    };
    let signature = CallSignature {
        parameters: operands
            .get(arg_start..)
            .ok_or_else(|| Diagnostic::error("Microsoft x64 call argument start is out of range"))?
            .iter()
            .map(win64_operand_shape)
            .collect::<Result<Vec<_>, _>>()?,
        result,
    };
    evaluate_normalized_win64_plan(&signature)
}

fn validate_win64_plan_operand_shapes<T: InstructionOperandLike>(
    plan: &CallPlan,
    operands: &[T],
    returns_value: bool,
) -> Result<(), Diagnostic> {
    validate_win64_call_plan_operand_shapes(
        plan,
        operands,
        returns_value.then_some(0),
        usize::from(returns_value),
    )
}

fn validate_win64_call_plan_operand_shapes<T: InstructionOperandLike>(
    plan: &CallPlan,
    operands: &[T],
    result_index: Option<usize>,
    arg_start: usize,
) -> Result<(), Diagnostic> {
    let parameters = operands
        .get(arg_start..)
        .ok_or_else(|| Diagnostic::error("Microsoft x64 call has no argument slice"))?
        .iter()
        .map(win64_operand_shape)
        .collect::<Result<Vec<_>, _>>()?;
    let result = result_index
        .map(|index| {
            operands
                .get(index)
                .ok_or_else(|| Diagnostic::error("Microsoft x64 call result index is out of range"))
                .and_then(win64_operand_shape)
        })
        .transpose()?;
    validate_call_plan(plan, &CallSignature { parameters, result }).map_err(|error| {
        Diagnostic::error(format!(
            "Microsoft x64 source calling plan does not match the selected call operands: {error}"
        ))
    })
}

fn evaluate_normalized_win64_plan(signature: &CallSignature) -> Result<CallPlan, Diagnostic> {
    let plan = evaluate_call_plan(CallingPolicy::MicrosoftX64, signature).map_err(|error| {
        Diagnostic::error(format!("cannot evaluate Microsoft x64 call plan: {error}"))
    })?;
    validate_win64_encoder_plan(&plan)?;
    Ok(plan)
}

fn validate_win64_encoder_plan(plan: &CallPlan) -> Result<(), Diagnostic> {
    if plan.policy != CallingPolicy::MicrosoftX64
        || plan.entry_control != EntryControl::CallReturn
        || plan.stack_alignment != 16
        || plan.shadow_bytes != WIN64_STACK_ARG_HOME as u16
    {
        return Err(Diagnostic::error(format!(
            "Microsoft x64 import encoder cannot realize plan policy={:?}, control={:?}, alignment={}, shadow_bytes={}",
            plan.policy, plan.entry_control, plan.stack_alignment, plan.shadow_bytes
        )));
    }
    for scratch in [
        MachineRegister::X86Rax,
        MachineRegister::X86R10,
        MachineRegister::X86R11,
    ] {
        if !plan.ordinary_clobbers.contains(scratch) {
            return Err(Diagnostic::error(format!(
                "Microsoft x64 encoder scratch register {scratch:?} exceeds the plan's ordinary-clobber ceiling"
            )));
        }
    }
    Ok(())
}

fn win64_operand_shape<T: InstructionOperandLike>(operand: &T) -> Result<ValueShape, Diagnostic> {
    if let Some((_, _, byte_count, alignment)) = win64_aggregate_operand(operand) {
        let byte_count = u16::try_from(byte_count)
            .map_err(|_| Diagnostic::error("Microsoft x64 aggregate width exceeds u16"))?;
        let alignment = u16::try_from(alignment)
            .map_err(|_| Diagnostic::error("Microsoft x64 aggregate alignment exceeds u16"))?;
        return Ok(ValueShape::integer(byte_count, alignment));
    }
    if let Some((_, _, byte_count)) = operand.runtime_scalar_float() {
        let byte_count = u16::try_from(byte_count)
            .map_err(|_| Diagnostic::error("Microsoft x64 float operand width exceeds u16"))?;
        return Ok(ValueShape::float(byte_count));
    }
    if let Some((_, _, byte_count)) = operand.runtime_scalar_integer() {
        return win64_integer_shape(byte_count, "integer operand");
    }
    if operand.data_address().is_some()
        || operand.runtime_string_pointer().is_some()
        || operand.runtime_storage_address().is_some()
        || operand.immediate_integer().is_some()
        || operand.byte_length().is_some()
    {
        return Ok(ValueShape::integer(8, 8));
    }
    Err(Diagnostic::error(
        "Microsoft x64 import operand has no normalized scalar/pointer shape",
    ))
}

fn win64_integer_shape(byte_count: usize, label: &str) -> Result<ValueShape, Diagnostic> {
    let byte_count = u16::try_from(byte_count)
        .map_err(|_| Diagnostic::error(format!("Microsoft x64 {label} width exceeds u16")))?;
    Ok(ValueShape::integer(byte_count, byte_count.max(1)))
}

fn normalized_win64_result_register(
    plan: &CallPlan,
    returns_value: bool,
) -> Result<Option<MachineRegister>, Diagnostic> {
    match (returns_value, plan.result.as_ref()) {
        (false, None) => Ok(None),
        (true, Some(placement)) => match placement.locations.as_slice() {
            [
                ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size,
                },
            ] if *byte_size == placement.shape.byte_size => Ok(Some(*register)),
            locations => Err(Diagnostic::error(format!(
                "Microsoft x64 import result has unsupported placement {locations:?}"
            ))),
        },
        _ => Err(Diagnostic::error(
            "Microsoft x64 import plan/result shape is internally inconsistent",
        )),
    }
}

fn win64_result_is_indirect(placement: &ValuePlacement) -> bool {
    matches!(
        placement.locations.as_slice(),
        [ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register(MachineRegister::X86Rcx),
            copy_stack_byte_offset: None,
            ..
        }]
    )
}

fn win64_result_pre_call_width(plan: &CallPlan) -> usize {
    usize::from(plan.result.as_ref().is_some_and(win64_result_is_indirect)) * 17
}

fn win64_result_post_call_width(plan: &CallPlan) -> usize {
    match plan.result.as_ref() {
        Some(placement) if matches!(placement.shape.class, ValueClass::Float) => 19,
        Some(placement) if !win64_result_is_indirect(placement) => {
            17 + usize::from(placement.shape.byte_size == 2)
        }
        _ => 0,
    }
}

fn append_win64_indirect_result_address<T: InstructionOperandLike>(
    bytes: &mut Vec<u8>,
    operand: &T,
    placement: &ValuePlacement,
) -> Result<(), Diagnostic> {
    let [
        ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register(MachineRegister::X86Rcx),
            copy_stack_byte_offset: None,
            byte_size,
            alignment,
        },
    ] = placement.locations.as_slice()
    else {
        return Err(Diagnostic::error(
            "Microsoft x64 indirect result does not use the hidden RCX destination",
        ));
    };
    let Some((_, byte_offset, operand_byte_size, operand_alignment)) =
        win64_aggregate_operand(operand)
    else {
        return Err(Diagnostic::error(
            "Microsoft x64 indirect result did not lower to aggregate storage",
        ));
    };
    if operand_byte_size != usize::from(*byte_size) || operand_alignment != usize::from(*alignment)
    {
        return Err(Diagnostic::error(
            "Microsoft x64 indirect result storage disagrees with its normalized shape",
        ));
    }
    append_mov_r11_imm64(bytes, 0); // relocated to the result region base
    bytes.extend([0x49, 0x8d, 0x8b]); // lea rcx, [r11+disp32]
    bytes.extend(disp32(byte_offset)?.to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod x86_import_plan_tests {
    use super::*;
    use omega_target_operations::{
        RuntimeStorageRegion, TargetInstructionOperand, TargetInstructionOperandKind,
    };

    fn operand(kind: TargetInstructionOperandKind) -> TargetInstructionOperand {
        TargetInstructionOperand { kind }
    }

    #[test]
    fn general_import_plan_carries_register_stack_and_result_placements() {
        let operands = std::iter::once(operand(
            TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 8,
            },
        ))
        .chain((0..6).map(|value| operand(TargetInstructionOperandKind::ImmediateInteger(value))))
        .collect::<Vec<_>>();

        let plan = normalized_win64_import_plan(&operands, true).expect("Microsoft x64 plan");

        assert_eq!(
            plan.parameters[0].locations,
            [ValueLocation::Register {
                register: MachineRegister::X86Rcx,
                value_byte_offset: 0,
                byte_size: 8,
            }]
        );
        assert_eq!(
            plan.parameters[4].locations,
            [ValueLocation::Stack {
                stack_byte_offset: 32,
                value_byte_offset: 0,
                byte_size: 8,
                alignment: 8,
            }]
        );
        assert_eq!(
            plan.parameters[5].locations,
            [ValueLocation::Stack {
                stack_byte_offset: 40,
                value_byte_offset: 0,
                byte_size: 8,
                alignment: 8,
            }]
        );
        assert_eq!(
            normalized_win64_result_register(&plan, true).expect("result placement"),
            Some(MachineRegister::X86Rax)
        );
        let bytes = encode_win64_import_call(&operands, true, false)
            .expect("the general encoder must consume the evaluated placements");
        assert!(
            bytes.windows(2).any(|window| window == [0x49, 0xbb]),
            "the result base must use plan-clobbered r11"
        );
        assert!(!bytes.windows(2).any(|window| window == [0x49, 0xbf]));
    }

    #[test]
    fn win64_indirect_aggregate_arguments_use_aligned_copies_and_positional_pointers() {
        let operands = vec![
            operand(TargetInstructionOperandKind::ImmediateInteger(1)),
            operand(TargetInstructionOperandKind::RuntimeLargeAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 64,
                byte_count: 24,
                alignment: 8,
            }),
            operand(TargetInstructionOperandKind::ImmediateInteger(2)),
            operand(TargetInstructionOperandKind::ImmediateInteger(3)),
            operand(TargetInstructionOperandKind::RuntimeSmallAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 96,
                byte_count: 16,
                alignment: 8,
            }),
        ];
        let plan = normalized_win64_import_plan(&operands, false)
            .expect("Microsoft x64 aggregate argument plan");
        assert!(matches!(
            plan.parameters[1].locations.as_slice(),
            [ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(MachineRegister::X86Rdx),
                copy_stack_byte_offset: Some(48),
                ..
            }]
        ));
        assert!(matches!(
            plan.parameters[4].locations.as_slice(),
            [ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Stack {
                    stack_byte_offset: 32,
                    ..
                },
                copy_stack_byte_offset: Some(80),
                ..
            }]
        ));

        let bytes = encode_win64_import_call(&operands, false, false)
            .expect("Microsoft x64 aggregate argument call");
        assert_eq!(
            bytes.len(),
            win64_import_call_width(&operands, false, false)
        );
        assert_eq!(&bytes[..4], &[0x48, 0x83, 0xec, 104]);
        assert!(
            bytes
                .windows(8)
                .any(|window| window == [0x48, 0x8d, 0x94, 0x24, 48, 0, 0, 0]),
            "the second positional argument must point RDX at its aligned copy"
        );
        assert!(
            bytes.windows(16).any(|window| window
                == [
                    0x48, 0x8d, 0x84, 0x24, 80, 0, 0, 0, 0x48, 0x89, 0x84, 0x24, 32, 0, 0, 0,
                ]),
            "the fifth positional argument must store its copy pointer above shadow space"
        );
        assert_eq!(
            win64_import_call_relocation_sites(&operands, false, false)
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(1), Some(4), None]
        );
    }

    #[test]
    fn win64_odd_width_record_uses_an_indirect_copy_without_breaking_stack_alignment() {
        let operands = [operand(
            TargetInstructionOperandKind::RuntimeSmallAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 7,
                byte_count: 3,
                alignment: 1,
            },
        )];
        let plan = normalized_win64_import_plan(&operands, false)
            .expect("odd-width Microsoft x64 aggregate plan");
        assert!(matches!(
            plan.parameters[0].locations.as_slice(),
            [ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(MachineRegister::X86Rcx),
                copy_stack_byte_offset: Some(32),
                ..
            }]
        ));

        let bytes = encode_win64_import_call(&operands, false, false)
            .expect("odd-width Microsoft x64 aggregate call");
        assert_eq!(&bytes[..4], &[0x48, 0x83, 0xec, 40]);
        assert_eq!(
            bytes.len(),
            win64_import_call_width(&operands, false, false)
        );
        assert!(
            bytes
                .windows(8)
                .any(|window| window == [0x48, 0x8d, 0x8c, 0x24, 32, 0, 0, 0]),
            "RCX must point at the three-byte caller copy"
        );
    }

    #[test]
    fn win64_direct_aggregate_arguments_use_positional_registers_and_stack_slots() {
        let aggregate = |byte_offset, byte_count| {
            operand(TargetInstructionOperandKind::RuntimeSmallAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset,
                byte_count,
                alignment: byte_count,
            })
        };
        let operands = [
            aggregate(0, 1),
            aggregate(8, 2),
            aggregate(16, 4),
            aggregate(24, 8),
            aggregate(32, 4),
        ];
        let plan = normalized_win64_import_plan(&operands, false)
            .expect("direct Microsoft x64 aggregate plan");
        assert!(matches!(
            plan.parameters[0].locations.as_slice(),
            [ValueLocation::Register {
                register: MachineRegister::X86Rcx,
                byte_size: 1,
                ..
            }]
        ));
        assert!(matches!(
            plan.parameters[4].locations.as_slice(),
            [ValueLocation::Stack {
                stack_byte_offset: 32,
                byte_size: 4,
                ..
            }]
        ));

        let bytes = encode_win64_import_call(&operands, false, false)
            .expect("direct Microsoft x64 aggregate call");
        assert_eq!(
            bytes.len(),
            win64_import_call_width(&operands, false, false)
        );
        assert_eq!(&bytes[..4], &[0x48, 0x83, 0xec, 40]);
        for load in [
            &[0x41, 0x8a, 0x8b, 0, 0, 0, 0][..],
            &[0x66, 0x41, 0x8b, 0x93, 8, 0, 0, 0],
            &[0x45, 0x8b, 0x83, 16, 0, 0, 0],
            &[0x4d, 0x8b, 0x8b, 24, 0, 0, 0],
        ] {
            assert!(
                bytes.windows(load.len()).any(|window| window == load),
                "missing direct aggregate register load {load:02x?}"
            );
        }
        assert!(
            bytes
                .windows(14)
                .any(|window| window
                    == [0x41, 0x8b, 0x83, 32, 0, 0, 0, 0x89, 0x84, 0x24, 32, 0, 0, 0,]),
            "the fifth direct record must occupy the low bytes of stack slot 32"
        );
        assert_eq!(
            win64_import_call_relocation_sites(&operands, false, false)
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(0), Some(1), Some(2), Some(3), Some(4), None]
        );
    }

    #[test]
    fn win64_direct_aggregate_results_spill_rax_at_the_record_width() {
        for (byte_count, store) in [
            (1, &[0x41, 0x88, 0x83][..]),
            (2, &[0x66, 0x41, 0x89, 0x83][..]),
            (4, &[0x41, 0x89, 0x83][..]),
            (8, &[0x49, 0x89, 0x83][..]),
        ] {
            let operands = [operand(
                TargetInstructionOperandKind::RuntimeSmallAggregate {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 24,
                    byte_count,
                    alignment: byte_count,
                },
            )];
            let plan = normalized_win64_import_plan(&operands, true)
                .expect("direct Microsoft x64 aggregate result plan");
            assert_eq!(
                normalized_win64_result_register(&plan, true).expect("result register"),
                Some(MachineRegister::X86Rax)
            );

            let bytes = encode_win64_import_call(&operands, true, false)
                .expect("direct Microsoft x64 aggregate result call");
            assert_eq!(bytes.len(), win64_import_call_width(&operands, true, false));
            let store_start = bytes.len() - store.len() - 4;
            assert_eq!(&bytes[store_start..store_start + store.len()], store);
            assert_eq!(&bytes[bytes.len() - 4..], &24u32.to_le_bytes());
            assert_eq!(
                win64_import_call_relocation_sites(&operands, true, false)
                    .iter()
                    .map(|site| site.operand_index)
                    .collect::<Vec<_>>(),
                [None, Some(0)]
            );
        }
    }

    #[test]
    fn win64_indirect_aggregate_result_uses_hidden_rcx_and_shifts_arguments() {
        let operands = [
            operand(TargetInstructionOperandKind::RuntimeLargeAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 64,
                byte_count: 24,
                alignment: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 8,
                byte_count: 8,
            }),
        ];
        let plan = normalized_win64_import_plan(&operands, true)
            .expect("indirect Microsoft x64 aggregate result plan");
        assert!(plan.result.as_ref().is_some_and(win64_result_is_indirect));
        assert!(matches!(
            plan.parameters[0].locations.as_slice(),
            [ValueLocation::Register {
                register: MachineRegister::X86Rdx,
                ..
            }]
        ));

        let bytes = encode_win64_import_call(&operands, true, false)
            .expect("indirect Microsoft x64 aggregate result call");
        assert_eq!(bytes.len(), win64_import_call_width(&operands, true, false));
        assert_eq!(&bytes[..4], &[0x48, 0x83, 0xec, 40]);
        assert_eq!(&bytes[4..6], &[0x49, 0xbb]);
        assert_eq!(
            &bytes[14..21],
            &[0x49, 0x8d, 0x8b, 64, 0, 0, 0],
            "RCX must address the caller-owned result record"
        );
        assert_eq!(
            &bytes[31..38],
            &[0x49, 0x8b, 0x93, 8, 0, 0, 0],
            "the first declared argument must shift to RDX"
        );
        assert_eq!(
            win64_import_call_relocation_sites(&operands, true, false)
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(0), Some(1), None]
        );
    }

    #[test]
    fn win64_scalar_floats_use_positional_xmm_registers_stack_and_xmm0_result() {
        let float = |byte_offset, byte_count| {
            operand(TargetInstructionOperandKind::RuntimeScalarFloat {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset,
                byte_count,
            })
        };
        let integer = |byte_offset| {
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset,
                byte_count: 8,
            })
        };
        let operands = [
            float(0, 8),
            integer(8),
            float(16, 4),
            integer(24),
            float(32, 8),
            float(40, 4),
        ];
        let plan = normalized_win64_import_plan(&operands, true)
            .expect("Microsoft x64 scalar-float import plan");
        assert!(matches!(
            plan.result.as_ref().unwrap().locations.as_slice(),
            [ValueLocation::Register {
                register: MachineRegister::X86Xmm(0),
                ..
            }]
        ));
        assert!(matches!(
            plan.parameters[1].locations.as_slice(),
            [ValueLocation::Register {
                register: MachineRegister::X86Xmm(1),
                ..
            }]
        ));
        assert!(matches!(
            plan.parameters[3].locations.as_slice(),
            [ValueLocation::Register {
                register: MachineRegister::X86Xmm(3),
                ..
            }]
        ));
        assert!(matches!(
            plan.parameters[4].locations.as_slice(),
            [ValueLocation::Stack {
                stack_byte_offset: 32,
                ..
            }]
        ));

        let bytes = encode_win64_import_call(&operands, true, false)
            .expect("Microsoft x64 scalar-float import call");
        assert_eq!(bytes.len(), win64_import_call_width(&operands, true, false));
        for instruction in [
            &[0xf3, 0x41, 0x0f, 0x10, 0x8b, 16, 0, 0, 0][..],
            &[0xf2, 0x41, 0x0f, 0x10, 0x9b, 32, 0, 0, 0],
            &[0xf2, 0x41, 0x0f, 0x11, 0x83, 0, 0, 0, 0],
        ] {
            assert!(
                bytes
                    .windows(instruction.len())
                    .any(|window| window == instruction),
                "missing float instruction {instruction:02x?}"
            );
        }
        assert!(
            bytes
                .windows(14)
                .any(|window| window
                    == [0x41, 0x8b, 0x83, 40, 0, 0, 0, 0x89, 0x84, 0x24, 32, 0, 0, 0]),
            "the fifth-position f32 must occupy the low four bytes of stack slot 32"
        );
        assert_eq!(
            win64_import_call_relocation_sites(&operands, true, false)
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(1), Some(2), Some(3), Some(4), Some(5), None, Some(0)]
        );
    }

    #[test]
    fn win64_encoder_rejects_scratch_above_the_plan_clobber_ceiling() {
        let mut plan = evaluate_call_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature {
                parameters: Vec::new(),
                result: Some(ValueShape::integer(8, 8)),
            },
        )
        .expect("baseline Microsoft x64 plan");
        plan.ordinary_clobbers = omega_calling_conventions::RegisterSet::new(
            plan.ordinary_clobbers
                .as_slice()
                .iter()
                .copied()
                .filter(|register| *register != MachineRegister::X86R11),
        );

        let error =
            validate_win64_encoder_plan(&plan).expect_err("missing volatile scratch must reject");
        assert!(error.message.contains("X86R11"));
        assert!(error.message.contains("ordinary-clobber ceiling"));
    }

    #[test]
    fn compatibility_host_encoder_rejects_a_sysv_target_policy() {
        let key = HostOperationKey::new(HostCapability::Clock, HostOperation::TickCount);
        let operands = [operand(
            TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 8,
            },
        )];

        let error = encode_host_call_sequence(CallingPolicy::SystemVAMD64, key, &operands)
            .expect_err("the Win64 compatibility encoder must not silently choose its ABI");

        assert!(error.message.contains("not SystemVAMD64"));
    }

    #[test]
    fn authored_import_consumes_the_supplied_plan_and_matching_relocation_walk() {
        let operands = [
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::ImmediateInteger(7)),
        ];
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: Some(ValueShape::integer(8, 8)),
        };
        let mut plan = evaluate_call_plan(CallingPolicy::SystemVAMD64, &signature)
            .expect("baseline SysV plan");
        plan.parameters[0].locations = vec![ValueLocation::Register {
            register: MachineRegister::X86Rcx,
            value_byte_offset: 0,
            byte_size: 8,
        }];
        omega_calling_conventions::validate_call_plan(&plan, &signature)
            .expect("source-selected nondefault placement remains structurally valid");

        let bytes = encode_authored_import_call_sequence(&plan, &operands)
            .expect("source-selected authored import");
        assert!(
            bytes.windows(2).any(|window| window == [0x48, 0xb9]),
            "the authored parameter placement must select rcx"
        );
        assert!(
            !bytes.windows(2).any(|window| window == [0x48, 0xbf]),
            "the target-derived SysV rdi placement must not replace the authored plan"
        );

        let sites = authored_import_relocation_sites(&plan, &operands);
        let call = sites
            .iter()
            .find(|site| site.kind == X86_64RelocationSiteKind::Relative32)
            .expect("call relocation");
        assert_eq!(bytes[call.byte_offset - 1], 0xe8);
        assert_eq!(
            sites
                .iter()
                .filter(|site| site.kind == X86_64RelocationSiteKind::Absolute64)
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(0)]
        );
    }

    #[test]
    fn authored_sysv_small_aggregates_use_planned_registers_and_results() {
        let operands = [
            operand(TargetInstructionOperandKind::RuntimeSmallAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 16,
                alignment: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 32,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeSmallAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 40,
                byte_count: 16,
                alignment: 8,
            }),
        ];
        let key = HostOperationKey::new(HostCapability::Unknown, HostOperation::Unknown);
        let layout = sysv_import_layout(&operands, true).expect("SysV aggregate import layout");

        assert_eq!(&layout.bytes[..4], &[0x48, 0x83, 0xec, 8]);
        assert!(
            layout
                .bytes
                .windows(7)
                .any(|window| window == [0x49, 0x8b, 0xbb, 32, 0, 0, 0]),
            "tag must load into planned rdi"
        );
        assert!(
            layout
                .bytes
                .windows(14)
                .any(|window| window
                    == [0x49, 0x8b, 0xb3, 40, 0, 0, 0, 0x49, 0x8b, 0x93, 48, 0, 0, 0]),
            "aggregate fragments must load into planned rsi/rdx"
        );
        assert!(
            layout.bytes.windows(14).any(
                |window| window == [0x49, 0x89, 0x83, 0, 0, 0, 0, 0x49, 0x89, 0x93, 8, 0, 0, 0]
            ),
            "result fragments must store from planned rax/rdx"
        );
        assert_eq!(
            layout
                .relocation_sites
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(1), Some(2), None, Some(0)]
        );
        assert_eq!(
            encode_host_call_sequence(CallingPolicy::SystemVAMD64, key, &operands)
                .expect("routed SysV authored import"),
            layout.bytes
        );
    }

    #[test]
    fn authored_sysv_scalar_floats_use_the_independent_xmm_bank_and_result() {
        let operands = [
            operand(TargetInstructionOperandKind::RuntimeScalarFloat {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 8,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarFloat {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 16,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 24,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarFloat {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 32,
                byte_count: 8,
            }),
        ];
        let layout = sysv_import_layout(&operands, true).expect("SysV scalar-float import");

        assert!(
            layout
                .bytes
                .windows(9)
                .any(|window| window == [0xf2, 0x41, 0x0f, 0x10, 0x83, 16, 0, 0, 0]),
            "first float must load into xmm0 independently of rdi"
        );
        assert!(
            layout
                .bytes
                .windows(9)
                .any(|window| window == [0xf2, 0x41, 0x0f, 0x10, 0x8b, 32, 0, 0, 0]),
            "second float must load into xmm1 independently of rsi"
        );
        assert!(
            layout
                .bytes
                .windows(9)
                .any(|window| window == [0xf2, 0x41, 0x0f, 0x11, 0x83, 0, 0, 0, 0]),
            "the float result must spill from planned xmm0"
        );
        assert_eq!(
            layout
                .relocation_sites
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(1), Some(2), Some(3), Some(4), None, Some(0)]
        );
    }

    #[test]
    fn authored_sysv_ninth_scalar_float_moves_to_the_stack() {
        let mut operands = vec![operand(
            TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 8,
            },
        )];
        operands.extend((0..9).map(|index| {
            operand(TargetInstructionOperandKind::RuntimeScalarFloat {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 16 + index * 8,
                byte_count: 8,
            })
        }));
        operands.push(operand(
            TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 96,
                byte_count: 8,
            },
        ));

        let layout = sysv_import_layout(&operands, true).expect("SysV stack-float import");
        assert_eq!(&layout.bytes[..4], &[0x48, 0x83, 0xec, 8]);
        assert!(
            layout.bytes.windows(15).any(|window| window
                == [
                    0x49, 0x8b, 0x83, 80, 0, 0, 0, 0x48, 0x89, 0x84, 0x24, 0, 0, 0, 0,
                ]),
            "the ninth float's bits must occupy outgoing stack offset zero: {:02x?}",
            layout.bytes
        );
        assert!(
            layout
                .bytes
                .windows(7)
                .any(|window| window == [0x49, 0x8b, 0xbb, 96, 0, 0, 0]),
            "the independent integer bank must still start at rdi"
        );
    }

    #[test]
    fn authored_sysv_register_exhausted_aggregate_rolls_wholly_to_stack() {
        let mut operands = vec![operand(
            TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 8,
            },
        )];
        operands.extend((0..5).map(|index| {
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 32 + index * 8,
                byte_count: 8,
            })
        }));
        operands.push(operand(
            TargetInstructionOperandKind::RuntimeSmallAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 80,
                byte_count: 16,
                alignment: 8,
            },
        ));
        operands.push(operand(
            TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 96,
                byte_count: 8,
            },
        ));

        let layout = sysv_import_layout(&operands, true).expect("SysV rollback import layout");
        assert_eq!(&layout.bytes[..4], &[0x48, 0x83, 0xec, 24]);
        assert!(
            layout.bytes.windows(30).any(|window| window
                == [
                    0x49, 0x8b, 0x83, 80, 0, 0, 0, 0x48, 0x89, 0x84, 0x24, 0, 0, 0, 0, 0x49, 0x8b,
                    0x83, 88, 0, 0, 0, 0x48, 0x89, 0x84, 0x24, 8, 0, 0, 0,
                ]),
            "the complete aggregate must occupy outgoing stack offsets 0 and 8"
        );
        assert!(
            layout
                .bytes
                .windows(7)
                .any(|window| window == [0x4d, 0x8b, 0x8b, 96, 0, 0, 0]),
            "the trailing scalar must retain the rolled-back r9 register"
        );
    }

    #[test]
    fn sysv_vtable_field_marshals_wire_arguments_and_small_result() {
        let operands = [
            operand(TargetInstructionOperandKind::RuntimeSmallAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 16,
                alignment: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 32,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 40,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeSmallAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 48,
                byte_count: 16,
                alignment: 8,
            }),
        ];
        let layout = sysv_field_call_layout(&operands, 24, true, true, None)
            .expect("SysV vtable field call");

        assert!(
            layout
                .bytes
                .windows(7)
                .any(|window| window == [0x49, 0x8b, 0xbb, 32, 0, 0, 0]),
            "receiver must load into planned rdi"
        );
        assert!(
            layout
                .bytes
                .windows(9)
                .any(|window| window == [0x48, 0x8b, 0x87, 24, 0, 0, 0, 0xff, 0xd0]),
            "dispatch must read the field from the receiver and call rax"
        );
        assert!(
            layout.bytes.windows(14).any(
                |window| window == [0x49, 0x89, 0x83, 0, 0, 0, 0, 0x49, 0x89, 0x93, 8, 0, 0, 0]
            ),
            "small result must spill from planned rax/rdx fragments"
        );
        assert_eq!(
            layout
                .relocation_sites
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(1), Some(2), Some(3), Some(0)]
        );
    }

    #[test]
    fn sysv_table_function_excludes_dispatch_table_from_wire_signature() {
        let operands = [
            operand(TargetInstructionOperandKind::RuntimeScalarFloat {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 8,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarFloat {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 16,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 24,
                byte_count: 8,
            }),
        ];
        let layout = sysv_field_call_layout(&operands, 40, true, false, None)
            .expect("SysV table-function call");

        assert!(
            layout
                .bytes
                .windows(9)
                .any(|window| window == [0xf2, 0x41, 0x0f, 0x10, 0x83, 16, 0, 0, 0]),
            "first wire float must use xmm0"
        );
        assert!(
            layout
                .bytes
                .windows(7)
                .any(|window| window == [0x49, 0x8b, 0xbb, 24, 0, 0, 0]),
            "first wire integer must use rdi, proving the table consumed no slot"
        );
        assert!(
            layout.bytes.windows(16).any(|window| window
                == [
                    0x49, 0x8b, 0x83, 8, 0, 0, 0, 0x48, 0x8b, 0x80, 40, 0, 0, 0, 0xff, 0xd0,
                ]),
            "dispatch must load the table slot, then the function field"
        );
        assert!(
            layout
                .bytes
                .windows(9)
                .any(|window| window == [0xf2, 0x41, 0x0f, 0x11, 0x83, 0, 0, 0, 0]),
            "float result must spill from xmm0"
        );
        assert_eq!(
            layout
                .relocation_sites
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(2), Some(3), Some(1), Some(0)]
        );
    }

    #[test]
    fn authored_sysv_memory_class_uses_stack_and_hidden_result_pointer() {
        let operands = [
            operand(TargetInstructionOperandKind::RuntimeLargeAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 24,
                alignment: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeLargeAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 32,
                byte_count: 24,
                alignment: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 64,
                byte_count: 8,
            }),
        ];
        let layout = sysv_import_layout(&operands, true).expect("SysV MEMORY-class import");

        assert_eq!(&layout.bytes[..4], &[0x48, 0x83, 0xec, 24]);
        assert!(
            layout
                .bytes
                .windows(7)
                .any(|window| window == [0x49, 0x8d, 0xbb, 0, 0, 0, 0]),
            "hidden result destination must materialize in rdi"
        );
        for stack_offset in [0u8, 8, 16] {
            assert!(
                layout
                    .bytes
                    .windows(8)
                    .any(|window| window == [0x48, 0x89, 0x84, 0x24, stack_offset, 0, 0, 0]),
                "large argument fragment must occupy stack offset {stack_offset}"
            );
        }
        assert!(
            layout
                .bytes
                .windows(7)
                .any(|window| window == [0x49, 0x8b, 0xb3, 64, 0, 0, 0]),
            "declared scalar must shift to rsi behind the hidden result pointer"
        );
        assert_eq!(
            layout
                .relocation_sites
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(0), Some(1), Some(2), None]
        );
    }

    #[test]
    fn authored_sysv_two_f64_record_uses_xmm_fragments_and_result() {
        let operands = [
            operand(
                TargetInstructionOperandKind::RuntimeHomogeneousFloatAggregate {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 0,
                    member_byte_count: 8,
                    members: 2,
                },
            ),
            operand(
                TargetInstructionOperandKind::RuntimeHomogeneousFloatAggregate {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 16,
                    member_byte_count: 8,
                    members: 2,
                },
            ),
        ];
        let layout = sysv_import_layout(&operands, true).expect("SysV two-f64 record import");

        assert!(
            layout.bytes.windows(18).any(|window| window
                == [
                    0xf2, 0x41, 0x0f, 0x10, 0x83, 16, 0, 0, 0, 0xf2, 0x41, 0x0f, 0x10, 0x8b, 24, 0,
                    0, 0,
                ]),
            "argument members must load into xmm0/xmm1"
        );
        assert!(
            layout
                .bytes
                .windows(9)
                .any(|window| window == [0xf2, 0x41, 0x0f, 0x11, 0x83, 0, 0, 0, 0])
        );
        assert!(
            layout
                .bytes
                .windows(9)
                .any(|window| window == [0xf2, 0x41, 0x0f, 0x11, 0x8b, 8, 0, 0, 0])
        );
        assert_eq!(
            layout
                .relocation_sites
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(1), None, Some(0)]
        );
    }

    #[test]
    fn authored_sysv_three_f32_record_packs_by_eightbyte() {
        let aggregate = || {
            operand(
                TargetInstructionOperandKind::RuntimeHomogeneousFloatAggregate {
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 16,
                    member_byte_count: 4,
                    members: 3,
                },
            )
        };
        let operands = [aggregate(), aggregate()];
        let layout = sysv_import_layout(&operands, true).expect("SysV three-f32 record import");

        assert!(layout.bytes.windows(18).any(|window| window
            == [
                0xf2, 0x41, 0x0f, 0x10, 0x83, 16, 0, 0, 0, 0xf3, 0x41, 0x0f, 0x10, 0x8b, 24, 0, 0,
                0,
            ]));
        assert!(
            layout
                .bytes
                .windows(9)
                .any(|window| window == [0xf2, 0x41, 0x0f, 0x11, 0x83, 16, 0, 0, 0])
        );
        assert!(
            layout
                .bytes
                .windows(9)
                .any(|window| window == [0xf3, 0x41, 0x0f, 0x11, 0x8b, 24, 0, 0, 0])
        );
    }

    #[test]
    fn authored_sysv_mixed_record_uses_rax_and_xmm0_result() {
        let aggregate = |byte_offset| {
            operand(TargetInstructionOperandKind::RuntimeSystemVAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset,
                byte_count: 16,
                alignment: 8,
                sse_eightbytes: 0b10,
            })
        };
        let operands = [aggregate(0), aggregate(16)];
        let layout = sysv_import_layout(&operands, true).expect("SysV mixed aggregate import");

        assert!(
            layout
                .bytes
                .windows(7)
                .any(|window| window == [0x49, 0x8b, 0xbb, 16, 0, 0, 0]),
            "INTEGER argument eightbyte must load into rdi"
        );
        assert!(
            layout
                .bytes
                .windows(9)
                .any(|window| window == [0xf2, 0x41, 0x0f, 0x10, 0x83, 24, 0, 0, 0]),
            "SSE argument eightbyte must load into xmm0"
        );
        assert!(
            layout
                .bytes
                .windows(7)
                .any(|window| window == [0x49, 0x89, 0x83, 0, 0, 0, 0]),
            "INTEGER result eightbyte must store from rax"
        );
        assert!(
            layout
                .bytes
                .windows(9)
                .any(|window| window == [0xf2, 0x41, 0x0f, 0x11, 0x83, 8, 0, 0, 0]),
            "SSE result eightbyte must store from xmm0"
        );
        assert_eq!(
            layout
                .relocation_sites
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(1), None, Some(0)]
        );
    }

    #[test]
    fn authored_sysv_nonhomogeneous_sse_record_uses_two_xmm_fragments() {
        let aggregate = |byte_offset| {
            operand(TargetInstructionOperandKind::RuntimeSystemVAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset,
                byte_count: 16,
                alignment: 8,
                sse_eightbytes: 0b11,
            })
        };
        let operands = [aggregate(0), aggregate(16)];
        let layout =
            sysv_import_layout(&operands, true).expect("SysV non-homogeneous SSE aggregate");

        assert!(layout.bytes.windows(18).any(|window| window
            == [
                0xf2, 0x41, 0x0f, 0x10, 0x83, 16, 0, 0, 0, 0xf2, 0x41, 0x0f, 0x10, 0x8b, 24, 0, 0,
                0,
            ]));
        assert!(
            layout
                .bytes
                .windows(9)
                .any(|window| window == [0xf2, 0x41, 0x0f, 0x11, 0x83, 0, 0, 0, 0])
        );
        assert!(
            layout
                .bytes
                .windows(9)
                .any(|window| window == [0xf2, 0x41, 0x0f, 0x11, 0x8b, 8, 0, 0, 0])
        );
    }

    #[test]
    fn sysv_vtable_large_result_shifts_receiver_to_rsi() {
        let operands = [
            operand(TargetInstructionOperandKind::RuntimeLargeAggregate {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 24,
                alignment: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 32,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 40,
                byte_count: 8,
            }),
        ];
        let layout =
            sysv_field_call_layout(&operands, 24, true, true, None).expect("SysV sret vtable call");

        assert!(
            layout
                .bytes
                .windows(9)
                .any(|window| window == [0x48, 0x8b, 0x86, 24, 0, 0, 0, 0xff, 0xd0]),
            "receiver dispatch must use planned rsi behind hidden rdi"
        );
        assert_eq!(
            layout
                .relocation_sites
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(0), Some(1), Some(2)]
        );
    }

    #[test]
    fn authored_sysv_encoder_rejects_scratch_above_the_plan_clobber_ceiling() {
        let mut plan = evaluate_call_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: vec![ValueShape::integer(16, 8)],
                result: Some(ValueShape::integer(16, 8)),
            },
        )
        .expect("baseline SysV aggregate plan");
        plan.ordinary_clobbers = omega_calling_conventions::RegisterSet::new(
            plan.ordinary_clobbers
                .as_slice()
                .iter()
                .copied()
                .filter(|register| *register != MachineRegister::X86R11),
        );

        let error = validate_sysv_import_plan(&plan)
            .expect_err("missing volatile staging scratch must reject");
        assert!(error.message.contains("X86R11"));
        assert!(error.message.contains("ordinary-clobber ceiling"));
    }

    #[test]
    fn non_boundary_constant_results_remain_policy_independent() {
        let key = HostOperationKey::new(
            HostCapability::Clock,
            HostOperation::WallClockUnitsPerSecond,
        );
        let operands = [
            operand(TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 0,
                byte_count: 8,
            }),
            operand(TargetInstructionOperandKind::ImmediateInteger(
                1_000_000_000,
            )),
        ];

        encode_host_call_sequence(CallingPolicy::SystemVAMD64, key, &operands)
            .expect("constant materialization does not apply a calling policy");
    }

    #[test]
    fn simple_kernel32_calls_keep_their_exact_bytes_and_relocations() {
        let get_std = HostOperationKey::new(HostCapability::Stdout, HostOperation::GetStdHandle);
        let get_std_operands = [operand(TargetInstructionOperandKind::ImmediateInteger(-11))];
        let bytes =
            encode_host_call_sequence(CallingPolicy::MicrosoftX64, get_std, &get_std_operands)
                .expect("plan-driven GetStdHandle");
        assert_eq!(
            bytes,
            [
                0x48, 0x83, 0xec, 0x28, 0xb9, 0xf5, 0xff, 0xff, 0xff, 0xe8, 0, 0, 0, 0, 0x48, 0x83,
                0xc4, 0x28,
            ]
        );
        assert_eq!(
            host_call_relocation_sites(get_std, &get_std_operands),
            [X86_64RelocationSite {
                operand_index: None,
                byte_offset: 10,
                byte_width: 4,
                kind: X86_64RelocationSiteKind::Relative32,
            }]
        );

        let exit = HostOperationKey::new(HostCapability::Process, HostOperation::ExitProcess);
        let exit_operands = [operand(
            TargetInstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 24,
                byte_count: 4,
            },
        )];
        encode_host_call_sequence(CallingPolicy::MicrosoftX64, exit, &exit_operands)
            .expect("plan-driven ExitProcess");
        let sites = host_call_relocation_sites(exit, &exit_operands);
        assert_eq!(sites[0].byte_offset, 6, "runtime region-base imm64");
        assert_eq!(sites[1].byte_offset, 22, "call rel32");
    }

    #[test]
    fn time_out_parameter_plans_model_the_actual_native_signatures() {
        let qpc = normalized_win64_out_param_plan(HostOperation::MonotonicTicks)
            .expect("QueryPerformanceCounter plan");
        assert_eq!(
            qpc.parameters[0].locations,
            [ValueLocation::Register {
                register: MachineRegister::X86Rcx,
                value_byte_offset: 0,
                byte_size: 8,
            }]
        );
        assert_eq!(
            normalized_win64_result_register(&qpc, true).expect("QPC native BOOL result"),
            Some(MachineRegister::X86Rax)
        );

        let filetime = normalized_win64_out_param_plan(HostOperation::WallClockRaw)
            .expect("GetSystemTimePreciseAsFileTime plan");
        assert!(
            filetime.result.is_none(),
            "FILETIME native call returns void"
        );
    }

    #[test]
    fn file_io_plan_models_registers_stack_argument_and_native_result() {
        let plan = normalized_win64_file_io_plan().expect("ReadFile/WriteFile plan");
        let expected_registers = [
            MachineRegister::X86Rcx,
            MachineRegister::X86Rdx,
            MachineRegister::X86R8,
            MachineRegister::X86R9,
        ];
        for (index, expected) in expected_registers.into_iter().enumerate() {
            assert_eq!(
                win64_argument_location(&plan.parameters[index], index)
                    .expect("file-I/O register placement"),
                Win64ArgumentLocation::Register(expected)
            );
        }
        assert_eq!(
            win64_argument_location(&plan.parameters[4], 4).expect("OVERLAPPED stack placement"),
            Win64ArgumentLocation::Stack(32)
        );
        assert_eq!(
            normalized_win64_result_register(&plan, true).expect("native BOOL result"),
            Some(MachineRegister::X86Rax)
        );
        assert_eq!(
            win64_composite_reserve(48).expect("outgoing area plus temporary"),
            56
        );
    }
}

/// Relocation sites for a `encode_win64_import_call` sequence: one Absolute64
/// region-base site per staged argument (inside its `mov r11, imm64`), the
/// Relative32 `call rel32` after all marshalling, and (value-returning) the
/// result region base inside the store tail's `mov r11, imm64`.
/// Total byte width of `encode_win64_out_param_call`'s fixed sequence:
/// sub(4) + lea(5) + call(5) + load(5) + add(4) + mov r11,imm64(10) + store(7).
const WIN64_OUT_PARAM_CALL_WIDTH: usize = 40;

/// A 0-arg Win64 import whose RESULT arrives through an OUT-PARAM (std::time
/// rung 5: QueryPerformanceCounter/-Frequency and
/// GetSystemTimePreciseAsFileTime all write a u64 through their pointer
/// argument). Reserve 56 bytes — the 0-arg import reserve (40) + 16 so the
/// out slot at `[rsp+40]` sits ABOVE the callee-owned 32-byte shadow space
/// and rsp keeps the same 16-byte parity — pass the slot's address in RCX,
/// call, load the u64 back into RAX, release, then store through the
/// standard result tail. operands[0] = the result place.
fn encode_win64_out_param_call<T: InstructionOperandLike>(
    operation_key: HostOperationKey,
    operands: &[T],
) -> Result<Vec<u8>, Diagnostic> {
    const RESERVE: usize = 56;
    const SLOT: u8 = 40;
    let Some((_, byte_offset, byte_count)) = operands
        .first()
        .and_then(InstructionOperandLike::runtime_scalar_integer)
    else {
        return Err(Diagnostic::error(
            "cannot encode X86_64 out-param import call: the result storage place did not lower \
             to a runtime scalar operand",
        ));
    };
    let plan = normalized_win64_out_param_plan(operation_key.operation)?;
    match win64_argument_location(&plan.parameters[0], 0)? {
        Win64ArgumentLocation::Register(MachineRegister::X86Rcx) => {}
        location => {
            return Err(Diagnostic::error(format!(
                "Win64 out-parameter encoder requires its pointer in rcx, got {location:?}"
            )));
        }
    }
    let native_result = normalized_win64_result_register(&plan, plan.result.is_some())?;
    if native_result.is_some_and(|register| register != MachineRegister::X86Rax) {
        return Err(Diagnostic::error(format!(
            "Win64 out-parameter encoder cannot ignore planned native result {native_result:?}"
        )));
    }
    let mut bytes = Vec::with_capacity(WIN64_OUT_PARAM_CALL_WIDTH);
    append_sub_rsp(&mut bytes, RESERVE);
    bytes.extend([0x48, 0x8d, 0x4c, 0x24, SLOT]); // lea rcx, [rsp+SLOT]
    bytes.extend([0xe8, 0, 0, 0, 0]); // call rel32 (relocated)
    bytes.extend([0x48, 0x8b, 0x44, 0x24, SLOT]); // mov rax, [rsp+SLOT]
    append_add_rsp(&mut bytes, RESERVE);
    append_mov_r11_imm64(&mut bytes, 0); // relocated to the result region base
    match byte_count {
        4 => bytes.extend([0x41, 0x89, 0x83]), // mov [r11+disp32], eax
        8 => bytes.extend([0x49, 0x89, 0x83]), // mov [r11+disp32], rax
        other => {
            return Err(Diagnostic::error(format!(
                "X86_64 out-param import call cannot store a {other}-byte result (expected 4 or 8)"
            )));
        }
    }
    bytes.extend(disp32(byte_offset)?.to_le_bytes());
    debug_assert_eq!(bytes.len(), WIN64_OUT_PARAM_CALL_WIDTH);
    Ok(bytes)
}

fn normalized_win64_out_param_plan(operation: HostOperation) -> Result<CallPlan, Diagnostic> {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8)],
        result: match operation {
            HostOperation::MonotonicTicks | HostOperation::MonotonicTicksPerSecond => {
                Some(ValueShape::integer(4, 4))
            }
            HostOperation::WallClockRaw => None,
            operation => {
                return Err(Diagnostic::error(format!(
                    "unsupported Win64 out-parameter operation {operation:?}"
                )));
            }
        },
    };
    evaluate_normalized_win64_plan(&signature)
}

/// Relocation sites for `encode_win64_out_param_call`: the import-thunk call
/// rel32 at 10 (sub 4 + lea 5 + the call opcode) and the result region base
/// at 25 (14 + load 5 + add 4 + the mov r11,imm64 prefix).
fn win64_out_param_call_relocation_sites() -> Vec<X86_64RelocationSite> {
    vec![
        X86_64RelocationSite {
            operand_index: None,
            byte_offset: 10,
            byte_width: 4,
            kind: X86_64RelocationSiteKind::Relative32,
        },
        X86_64RelocationSite {
            operand_index: Some(0),
            byte_offset: 25,
            byte_width: 8,
            kind: X86_64RelocationSiteKind::Absolute64,
        },
    ]
}

/// Total byte width of `encode_constant_result`'s fixed sequence:
/// mov rax,imm64(10) + mov r15,imm64(10) + store(7).
const CONSTANT_RESULT_WIDTH: usize = 27;

/// A host operation lowered to a per-target CONSTANT (std::time rung 5's
/// wall-clock calibration constants, `PlatformCallData::ConstantResult`): no
/// call at all — materialize the immediate in RAX and run the standard
/// result store tail. operands[0] = the result place, operands[1] = the
/// constant as an immediate operand.
fn encode_constant_result<T: InstructionOperandLike>(
    operands: &[T],
) -> Result<Vec<u8>, Diagnostic> {
    let Some((_, byte_offset, byte_count)) = operands
        .first()
        .and_then(InstructionOperandLike::runtime_scalar_integer)
    else {
        return Err(Diagnostic::error(
            "cannot encode X86_64 constant-result host call: the result storage place did not \
             lower to a runtime scalar operand",
        ));
    };
    let Some(value) = operands
        .get(1)
        .and_then(InstructionOperandLike::immediate_integer)
    else {
        return Err(Diagnostic::error(
            "cannot encode X86_64 constant-result host call: the constant did not lower to an \
             immediate operand",
        ));
    };
    let mut bytes = Vec::with_capacity(CONSTANT_RESULT_WIDTH);
    bytes.extend([0x48, 0xb8]); // mov rax, imm64
    bytes.extend((value as u64).to_le_bytes());
    append_mov_r15_imm64(&mut bytes, 0); // relocated to the result region base
    match byte_count {
        4 => bytes.extend([0x41, 0x89, 0x87]), // mov [r15+disp32], eax
        8 => bytes.extend([0x49, 0x89, 0x87]), // mov [r15+disp32], rax
        other => {
            return Err(Diagnostic::error(format!(
                "X86_64 constant-result host call cannot store a {other}-byte result (expected 4 \
                 or 8)"
            )));
        }
    }
    bytes.extend(disp32(byte_offset)?.to_le_bytes());
    debug_assert_eq!(bytes.len(), CONSTANT_RESULT_WIDTH);
    Ok(bytes)
}

/// Relocation sites for `encode_constant_result`: only the result region base
/// at 12 (the mov rax,imm64 + the mov r15,imm64 prefix). No call site.
fn constant_result_relocation_sites() -> Vec<X86_64RelocationSite> {
    vec![X86_64RelocationSite {
        operand_index: Some(0),
        byte_offset: 12,
        byte_width: 8,
        kind: X86_64RelocationSiteKind::Absolute64,
    }]
}

fn win64_import_call_relocation_sites<T: InstructionOperandLike>(
    operands: &[T],
    returns_value: bool,
    dereferences_result: bool,
) -> Vec<X86_64RelocationSite> {
    win64_import_call_relocation_sites_with_plan(operands, returns_value, dereferences_result, None)
}

fn win64_import_call_relocation_sites_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    returns_value: bool,
    dereferences_result: bool,
    authoritative_plan: Option<&CallPlan>,
) -> Vec<X86_64RelocationSite> {
    let arg_start = usize::from(returns_value);
    let arg_count = operands.len().saturating_sub(arg_start);
    let plan = authoritative_plan
        .filter(|plan| {
            validate_win64_encoder_plan(plan).is_ok()
                && validate_win64_plan_operand_shapes(plan, operands, returns_value).is_ok()
        })
        .cloned()
        .or_else(|| {
            authoritative_plan
                .is_none()
                .then(|| normalized_win64_import_plan(operands, returns_value).ok())
                .flatten()
        });
    let reserve = plan
        .as_ref()
        .map(win64_import_reserve_for_plan)
        .unwrap_or_else(|| win64_import_reserve(arg_count));
    let mut sites = Vec::new();
    let mut cursor = rsp_adjust_width(reserve);
    if plan
        .as_ref()
        .and_then(|plan| plan.result.as_ref())
        .is_some_and(win64_result_is_indirect)
    {
        sites.push(X86_64RelocationSite {
            operand_index: Some(0),
            byte_offset: cursor + 2,
            byte_width: 8,
            kind: X86_64RelocationSiteKind::Absolute64,
        });
        cursor += 17;
    }
    for index in 0..arg_count {
        if win64_import_arg_is_staged(operands.get(arg_start + index)) {
            sites.push(X86_64RelocationSite {
                operand_index: Some(arg_start + index),
                byte_offset: cursor + 2, // inside mov r11/argreg, imm64
                byte_width: 8,
                kind: X86_64RelocationSiteKind::Absolute64,
            });
        }
        cursor += win64_import_arg_width(
            operands,
            arg_start,
            index,
            plan.as_ref().and_then(|plan| plan.parameters.get(index)),
        );
    }
    sites.push(X86_64RelocationSite {
        operand_index: None,
        byte_offset: cursor + 1, // past the call opcode
        byte_width: 4,
        kind: X86_64RelocationSiteKind::Relative32,
    });
    cursor += 5 + rsp_adjust_width(reserve);
    if dereferences_result {
        cursor += 2; // mov eax, [rax]
    }
    if returns_value
        && plan
            .as_ref()
            .and_then(|plan| plan.result.as_ref())
            .is_some_and(|placement| !win64_result_is_indirect(placement))
        && operands.first().is_some_and(|operand| {
            operand.runtime_scalar_integer().is_some()
                || operand.runtime_scalar_float().is_some()
                || win64_aggregate_operand(operand).is_some()
        })
    {
        sites.push(X86_64RelocationSite {
            operand_index: Some(0),
            byte_offset: cursor + 2, // inside the result mov r11, imm64
            byte_width: 8,
            kind: X86_64RelocationSiteKind::Absolute64,
        });
    }
    sites
}

/// A VtableSlot call (extern brief §12.1): marshal the declared args MS-x64
/// (this -> RCX, then RDX/R8/R9), then read the callee from the RECEIVER --
/// `mov rax, [rcx + index*8]; call rax`. The protocol struct IS the vtable
/// (UEFI SimpleTextOutput: OutputString at slot 1 = +8). No result store
/// (legacy void shape), no import thunk, no call relocation (the target is a
/// runtime pointer). The receiver (arg 0) must already sit in RCX -- so it is
/// a plain register arg like any other; the `mov rax, [rcx..]` reads it back.
pub fn encode_win64_vtable_call<T: InstructionOperandLike>(
    operands: &[T],
    index: i64,
) -> Result<Vec<u8>, Diagnostic> {
    encode_win64_vtable_call_with_plan(operands, index, None)
}

pub fn encode_win64_vtable_call_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    index: i64,
    authoritative_plan: Option<&CallPlan>,
) -> Result<Vec<u8>, Diagnostic> {
    let byte_offset = index
        .checked_mul(8)
        .ok_or_else(|| Diagnostic::error("vtable slot index overflows a byte offset"))?;
    encode_win64_vtable_call_at_offset_with_plan(operands, byte_offset, false, authoritative_plan)
}

/// The result store tail shared by the field-model call encoders (the same
/// shape as the import call's): `mov r11, imm64` relocated to the result
/// region base, then store rax/eax or xmm0 at the result's declared width.
fn append_win64_result_store<T: InstructionOperandLike>(
    bytes: &mut Vec<u8>,
    operand: &T,
    label: &str,
    placement: &ValuePlacement,
) -> Result<(), Diagnostic> {
    let result_register = match placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] if *byte_size == placement.shape.byte_size => *register,
        locations => {
            return Err(Diagnostic::error(format!(
                "X86_64 {label} has unsupported direct result placement {locations:?}"
            )));
        }
    };
    if let Some((_, byte_offset, byte_count)) = operand.runtime_scalar_float() {
        if result_register != MachineRegister::X86Xmm(0)
            || !matches!(placement.shape.class, ValueClass::Float)
            || usize::from(placement.shape.byte_size) != byte_count
        {
            return Err(Diagnostic::error(format!(
                "X86_64 {label} float result disagrees with its normalized XMM0 placement"
            )));
        }
        append_mov_r11_imm64(bytes, 0); // relocated to the result region base
        return append_x86_store_float_to_r11(bytes, result_register, byte_offset, byte_count);
    }
    if result_register != MachineRegister::X86Rax {
        return Err(Diagnostic::error(format!(
            "X86_64 {label} result store cannot realize planned register {result_register:?}"
        )));
    }
    let result_storage = operand
        .runtime_scalar_integer()
        .map(|(region, offset, size)| (region, offset, size))
        .or_else(|| {
            win64_aggregate_operand(operand).map(|(region, offset, size, _)| (region, offset, size))
        });
    let Some((_, byte_offset, byte_count)) = result_storage else {
        return Err(Diagnostic::error(format!(
            "cannot encode X86_64 {label}: the result storage place did not lower to a \
             runtime scalar or aggregate operand"
        )));
    };
    if usize::from(placement.shape.byte_size) != byte_count {
        return Err(Diagnostic::error(format!(
            "X86_64 {label} result storage disagrees with its normalized shape"
        )));
    }
    append_mov_r11_imm64(bytes, 0); // relocated to the result region base
    match byte_count {
        1 => bytes.extend([0x41, 0x88, 0x83]), // mov [r11+disp32], al
        2 => bytes.extend([0x66, 0x41, 0x89, 0x83]), // mov [r11+disp32], ax
        4 => bytes.extend([0x41, 0x89, 0x83]), // mov [r11+disp32], eax
        8 => bytes.extend([0x49, 0x89, 0x83]), // mov [r11+disp32], rax
        other => {
            return Err(Diagnostic::error(format!(
                "X86_64 {label} cannot store a direct {other}-byte result (expected 1, 2, 4, or 8)"
            )));
        }
    }
    bytes.extend(disp32(byte_offset)?.to_le_bytes());
    Ok(())
}

fn append_win64_vtable_dispatch_load(
    bytes: &mut Vec<u8>,
    receiver: &ValuePlacement,
    byte_offset: i64,
) -> Result<(), Diagnostic> {
    let register = match win64_argument_location(receiver, 0)? {
        Win64ArgumentLocation::Register(register) => register,
        Win64ArgumentLocation::Stack(_) => {
            return Err(Diagnostic::error(
                "Microsoft x64 vtable receiver unexpectedly lowered to the stack",
            ));
        }
    };
    let slot_disp = i32::try_from(byte_offset)
        .map_err(|_| Diagnostic::error("vtable field offset exceeds an imm32"))?;
    match register {
        MachineRegister::X86Rcx => bytes.extend([0x48, 0x8b, 0x81]),
        MachineRegister::X86Rdx => bytes.extend([0x48, 0x8b, 0x82]),
        MachineRegister::X86R8 => bytes.extend([0x49, 0x8b, 0x80]),
        MachineRegister::X86R9 => bytes.extend([0x49, 0x8b, 0x81]),
        other => {
            return Err(Diagnostic::error(format!(
                "Microsoft x64 vtable receiver uses unsupported register {other:?}"
            )));
        }
    }
    bytes.extend(slot_disp.to_le_bytes());
    Ok(())
}

/// The FIELD-MODEL flavor (extern brief SS12.1): the fn-ptr offset comes from
/// the vtable struct's layout, already in bytes -- `mov rax, [rcx + offset];
/// call rax`. The slot flavor above is offset = index * 8. This is the
/// This-call shape: the receiver IS the first wire argument (COM/UEFI
/// protocols). When `result_present`, `operands[0]` is the RESULT place
/// (`let status = ...` prepends one); the receiver and declared arguments
/// follow, and the callee's return value stores through the import-call tail.
pub fn encode_win64_vtable_call_at_offset<T: InstructionOperandLike>(
    operands: &[T],
    byte_offset: i64,
    result_present: bool,
) -> Result<Vec<u8>, Diagnostic> {
    encode_win64_vtable_call_at_offset_with_plan(operands, byte_offset, result_present, None)
}

pub fn encode_win64_vtable_call_at_offset_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    byte_offset: i64,
    result_present: bool,
    authoritative_plan: Option<&CallPlan>,
) -> Result<Vec<u8>, Diagnostic> {
    let arg_start = usize::from(result_present);
    if operands.len() <= arg_start {
        return Err(Diagnostic::error(
            "cannot encode X86_64 vtable call: the receiver (arg 0) did not lower to an operand",
        ));
    }
    let plan = if let Some(plan) = authoritative_plan {
        validate_win64_encoder_plan(plan)?;
        validate_win64_call_plan_operand_shapes(
            plan,
            operands,
            result_present.then_some(0),
            arg_start,
        )?;
        plan.clone()
    } else {
        normalized_win64_call_plan(operands, result_present.then_some(0), arg_start)?
    };
    let indirect_result = plan.result.as_ref().is_some_and(win64_result_is_indirect);
    if !indirect_result {
        normalized_win64_result_register(&plan, result_present)?;
    }
    let reserve = win64_import_reserve_for_plan(&plan);
    let mut bytes = Vec::with_capacity(win64_vtable_call_width_with_plan(
        operands,
        byte_offset,
        result_present,
        Some(&plan),
    ));
    append_sub_rsp(&mut bytes, reserve);
    if indirect_result {
        append_win64_indirect_result_address(
            &mut bytes,
            &operands[0],
            plan.result.as_ref().expect("indirect result placement"),
        )?;
    }
    append_win64_call_arguments(&mut bytes, operands, arg_start, Some(&plan.parameters))?;
    // A hidden indirect-result destination occupies RCX, shifting the
    // receiver to RDX. Read the dispatch pointer from its planned register.
    append_win64_vtable_dispatch_load(&mut bytes, &plan.parameters[0], byte_offset)?;
    append_call_register(&mut bytes, 0); // call rax
    append_add_rsp(&mut bytes, reserve);
    if result_present && !indirect_result {
        append_win64_result_store(
            &mut bytes,
            &operands[0],
            "vtable call",
            plan.result.as_ref().expect("direct result placement"),
        )?;
    }
    debug_assert_eq!(
        bytes.len(),
        win64_vtable_call_width_with_plan(operands, byte_offset, result_present, Some(&plan))
    );
    Ok(bytes)
}

pub fn win64_vtable_call_width<T: InstructionOperandLike>(
    operands: &[T],
    _index: i64,
    result_present: bool,
) -> usize {
    win64_vtable_call_width_with_plan(operands, _index, result_present, None)
}

pub fn win64_vtable_call_width_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    _index: i64,
    result_present: bool,
    authoritative_plan: Option<&CallPlan>,
) -> usize {
    let arg_start = usize::from(result_present);
    let arg_count = operands.len() - arg_start;
    let plan = authoritative_plan
        .filter(|plan| {
            validate_win64_encoder_plan(plan).is_ok()
                && validate_win64_call_plan_operand_shapes(
                    plan,
                    operands,
                    result_present.then_some(0),
                    arg_start,
                )
                .is_ok()
        })
        .cloned()
        .or_else(|| {
            authoritative_plan.is_none().then(|| {
                normalized_win64_call_plan(operands, result_present.then_some(0), arg_start).ok()
            })?
        });
    if authoritative_plan.is_some() && plan.is_none() {
        return 0;
    }
    let reserve = plan
        .as_ref()
        .map(win64_import_reserve_for_plan)
        .unwrap_or_else(|| win64_import_reserve(arg_count));
    let mut width = rsp_adjust_width(reserve);
    width += plan.as_ref().map(win64_result_pre_call_width).unwrap_or(0);
    for index in 0..arg_count {
        width += win64_import_arg_width(
            operands,
            arg_start,
            index,
            plan.as_ref().and_then(|plan| plan.parameters.get(index)),
        );
    }
    width += 7; // mov rax, [rcx + disp32]
    width += 2; // call rax (no REX.B for rax)
    width += rsp_adjust_width(reserve);
    width += plan
        .as_ref()
        .map(win64_result_post_call_width)
        .unwrap_or_else(|| usize::from(result_present) * 17);
    width
}

/// A SERVICE-TABLE function call (UEFI BootServices/RuntimeServices): the
/// table pointer is DISPATCH-ONLY -- the declared arguments AFTER it marshal
/// into RCX/RDX/R8/R9/stack (EFI table services take no This), then the
/// callee loads from the table's fn-ptr field: `mov r11, imm64` (relocated
/// to the table's region base), `mov rax, [r11 + slot]`, `mov rax, [rax +
/// field_offset]`, `call rax`. Operand roles: `[result?][table][args...]`.
pub fn encode_win64_table_function_call<T: InstructionOperandLike>(
    operands: &[T],
    byte_offset: i64,
    result_present: bool,
) -> Result<Vec<u8>, Diagnostic> {
    encode_win64_table_function_call_with_plan(operands, byte_offset, result_present, None)
}

pub fn encode_win64_table_function_call_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    byte_offset: i64,
    result_present: bool,
    authoritative_plan: Option<&CallPlan>,
) -> Result<Vec<u8>, Diagnostic> {
    let table_index = usize::from(result_present);
    if operands.len() <= table_index {
        return Err(Diagnostic::error(
            "cannot encode X86_64 table-function call: the service table pointer did not \
             lower to an operand",
        ));
    }
    let Some((_, table_slot_offset, _)) = operands[table_index].runtime_scalar_integer() else {
        return Err(Diagnostic::error(
            "cannot encode X86_64 table-function call: the service table pointer must lower \
             to a runtime scalar operand",
        ));
    };
    let arg_start = table_index + 1;
    let plan = if let Some(plan) = authoritative_plan {
        validate_win64_encoder_plan(plan)?;
        validate_win64_call_plan_operand_shapes(
            plan,
            operands,
            result_present.then_some(0),
            arg_start,
        )?;
        plan.clone()
    } else {
        normalized_win64_call_plan(operands, result_present.then_some(0), arg_start)?
    };
    let indirect_result = plan.result.as_ref().is_some_and(win64_result_is_indirect);
    if !indirect_result {
        normalized_win64_result_register(&plan, result_present)?;
    }
    let reserve = win64_import_reserve_for_plan(&plan);
    let mut bytes = Vec::with_capacity(win64_table_function_call_width_with_plan(
        operands,
        byte_offset,
        result_present,
        Some(&plan),
    ));
    append_sub_rsp(&mut bytes, reserve);
    if indirect_result {
        append_win64_indirect_result_address(
            &mut bytes,
            &operands[0],
            plan.result.as_ref().expect("indirect result placement"),
        )?;
    }
    append_win64_call_arguments(&mut bytes, operands, arg_start, Some(&plan.parameters))?;
    // Load the table pointer (dispatch-only, never a wire argument), read the
    // fn-ptr field, call it.
    append_mov_r11_imm64(&mut bytes, 0); // relocated to the table's region base
    bytes.extend([0x49, 0x8b, 0x83]); // mov rax, [r11 + disp32]
    bytes.extend(disp32(table_slot_offset)?.to_le_bytes());
    let field_disp = i32::try_from(byte_offset)
        .map_err(|_| Diagnostic::error("service table field offset exceeds an imm32"))?;
    bytes.extend([0x48, 0x8b, 0x80]); // mov rax, [rax + disp32]
    bytes.extend(field_disp.to_le_bytes());
    append_call_register(&mut bytes, 0); // call rax
    append_add_rsp(&mut bytes, reserve);
    if result_present && !indirect_result {
        append_win64_result_store(
            &mut bytes,
            &operands[0],
            "table-function call",
            plan.result.as_ref().expect("direct result placement"),
        )?;
    }
    debug_assert_eq!(
        bytes.len(),
        win64_table_function_call_width_with_plan(
            operands,
            byte_offset,
            result_present,
            Some(&plan),
        )
    );
    Ok(bytes)
}

pub fn win64_table_function_call_width<T: InstructionOperandLike>(
    operands: &[T],
    _byte_offset: i64,
    result_present: bool,
) -> usize {
    win64_table_function_call_width_with_plan(operands, _byte_offset, result_present, None)
}

pub fn win64_table_function_call_width_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    _byte_offset: i64,
    result_present: bool,
    authoritative_plan: Option<&CallPlan>,
) -> usize {
    let arg_start = usize::from(result_present) + 1;
    let arg_count = operands.len().saturating_sub(arg_start);
    let plan = authoritative_plan
        .filter(|plan| {
            validate_win64_encoder_plan(plan).is_ok()
                && validate_win64_call_plan_operand_shapes(
                    plan,
                    operands,
                    result_present.then_some(0),
                    arg_start,
                )
                .is_ok()
        })
        .cloned()
        .or_else(|| {
            authoritative_plan.is_none().then(|| {
                normalized_win64_call_plan(operands, result_present.then_some(0), arg_start).ok()
            })?
        });
    if authoritative_plan.is_some() && plan.is_none() {
        return 0;
    }
    let reserve = plan
        .as_ref()
        .map(win64_import_reserve_for_plan)
        .unwrap_or_else(|| win64_import_reserve(arg_count));
    let mut width = rsp_adjust_width(reserve);
    width += plan.as_ref().map(win64_result_pre_call_width).unwrap_or(0);
    for index in 0..arg_count {
        width += win64_import_arg_width(
            operands,
            arg_start,
            index,
            plan.as_ref().and_then(|plan| plan.parameters.get(index)),
        );
    }
    width += 10; // mov r11, imm64 (table region base)
    width += 7; // mov rax, [r11 + disp32]
    width += 7; // mov rax, [rax + disp32]
    width += 2; // call rax
    width += rsp_adjust_width(reserve);
    width += plan
        .as_ref()
        .map(win64_result_post_call_width)
        .unwrap_or_else(|| usize::from(result_present) * 17);
    width
}

/// The region-base fixup byte offset for vtable-call argument `operand_index`
/// (the `mov r11, imm64` imm), matching `encode_win64_vtable_call`'s layout.
pub fn vtable_call_data_relocation_byte_offset<T: InstructionOperandLike>(
    operands: &[T],
    operand_index: usize,
    result_present: bool,
) -> usize {
    win64_vtable_call_relocation_sites(operands, result_present)
        .into_iter()
        .find(|site| site.operand_index == Some(operand_index))
        .map(|site| site.byte_offset)
        .unwrap_or(0)
}

/// The region-base fixup byte offset for table-function-call operand
/// `operand_index`, matching `encode_win64_table_function_call`'s layout.
pub fn table_function_call_data_relocation_byte_offset<T: InstructionOperandLike>(
    operands: &[T],
    operand_index: usize,
    result_present: bool,
) -> usize {
    win64_table_function_call_relocation_sites(operands, result_present)
        .into_iter()
        .find(|site| site.operand_index == Some(operand_index))
        .map(|site| site.byte_offset)
        .unwrap_or(0)
}

/// Relocation sites for a vtable call: the staged-argument region bases (no
/// call relocation -- the callee is a runtime pointer read from RCX) and,
/// when a result place leads the operands, the result region base inside the
/// store tail's `mov r11, imm64`.
pub fn win64_vtable_call_relocation_sites<T: InstructionOperandLike>(
    operands: &[T],
    result_present: bool,
) -> Vec<X86_64RelocationSite> {
    win64_vtable_call_relocation_sites_with_plan(operands, result_present, None)
}

pub fn win64_vtable_call_relocation_sites_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    result_present: bool,
    authoritative_plan: Option<&CallPlan>,
) -> Vec<X86_64RelocationSite> {
    let arg_start = usize::from(result_present);
    let arg_count = operands.len() - arg_start;
    let plan = authoritative_plan
        .filter(|plan| {
            validate_win64_encoder_plan(plan).is_ok()
                && validate_win64_call_plan_operand_shapes(
                    plan,
                    operands,
                    result_present.then_some(0),
                    arg_start,
                )
                .is_ok()
        })
        .cloned()
        .or_else(|| {
            authoritative_plan.is_none().then(|| {
                normalized_win64_call_plan(operands, result_present.then_some(0), arg_start).ok()
            })?
        });
    if authoritative_plan.is_some() && plan.is_none() {
        return Vec::new();
    }
    let reserve = plan
        .as_ref()
        .map(win64_import_reserve_for_plan)
        .unwrap_or_else(|| win64_import_reserve(arg_count));
    let mut sites = Vec::new();
    let mut cursor = rsp_adjust_width(reserve);
    if plan
        .as_ref()
        .and_then(|plan| plan.result.as_ref())
        .is_some_and(win64_result_is_indirect)
    {
        sites.push(X86_64RelocationSite {
            operand_index: Some(0),
            byte_offset: cursor + 2,
            byte_width: 8,
            kind: X86_64RelocationSiteKind::Absolute64,
        });
        cursor += 17;
    }
    for index in 0..arg_count {
        if win64_import_arg_is_staged(operands.get(arg_start + index)) {
            sites.push(X86_64RelocationSite {
                operand_index: Some(arg_start + index),
                byte_offset: cursor + 2, // inside mov r11, imm64
                byte_width: 8,
                kind: X86_64RelocationSiteKind::Absolute64,
            });
        }
        cursor += win64_import_arg_width(
            operands,
            arg_start,
            index,
            plan.as_ref().and_then(|plan| plan.parameters.get(index)),
        );
    }
    cursor += 7 + 2 + rsp_adjust_width(reserve); // fn-ptr read + call rax + add rsp
    if result_present
        && plan
            .as_ref()
            .and_then(|plan| plan.result.as_ref())
            .is_some_and(|placement| !win64_result_is_indirect(placement))
        && operands.first().is_some_and(|operand| {
            operand.runtime_scalar_integer().is_some()
                || operand.runtime_scalar_float().is_some()
                || win64_aggregate_operand(operand).is_some()
        })
    {
        sites.push(X86_64RelocationSite {
            operand_index: Some(0),
            byte_offset: cursor + 2, // inside the result mov r11, imm64
            byte_width: 8,
            kind: X86_64RelocationSiteKind::Absolute64,
        });
    }
    sites
}

/// Relocation sites for a table-function call: the staged-argument region
/// bases, the TABLE pointer's region base (inside its dispatch load -- always
/// staged), and (result-present) the result region base in the store tail.
pub fn win64_table_function_call_relocation_sites<T: InstructionOperandLike>(
    operands: &[T],
    result_present: bool,
) -> Vec<X86_64RelocationSite> {
    win64_table_function_call_relocation_sites_with_plan(operands, result_present, None)
}

pub fn win64_table_function_call_relocation_sites_with_plan<T: InstructionOperandLike>(
    operands: &[T],
    result_present: bool,
    authoritative_plan: Option<&CallPlan>,
) -> Vec<X86_64RelocationSite> {
    let table_index = usize::from(result_present);
    let arg_start = table_index + 1;
    let arg_count = operands.len().saturating_sub(arg_start);
    let plan = authoritative_plan
        .filter(|plan| {
            validate_win64_encoder_plan(plan).is_ok()
                && validate_win64_call_plan_operand_shapes(
                    plan,
                    operands,
                    result_present.then_some(0),
                    arg_start,
                )
                .is_ok()
        })
        .cloned()
        .or_else(|| {
            authoritative_plan.is_none().then(|| {
                normalized_win64_call_plan(operands, result_present.then_some(0), arg_start).ok()
            })?
        });
    if authoritative_plan.is_some() && plan.is_none() {
        return Vec::new();
    }
    let reserve = plan
        .as_ref()
        .map(win64_import_reserve_for_plan)
        .unwrap_or_else(|| win64_import_reserve(arg_count));
    let mut sites = Vec::new();
    let mut cursor = rsp_adjust_width(reserve);
    if plan
        .as_ref()
        .and_then(|plan| plan.result.as_ref())
        .is_some_and(win64_result_is_indirect)
    {
        sites.push(X86_64RelocationSite {
            operand_index: Some(0),
            byte_offset: cursor + 2,
            byte_width: 8,
            kind: X86_64RelocationSiteKind::Absolute64,
        });
        cursor += 17;
    }
    for index in 0..arg_count {
        if win64_import_arg_is_staged(operands.get(arg_start + index)) {
            sites.push(X86_64RelocationSite {
                operand_index: Some(arg_start + index),
                byte_offset: cursor + 2, // inside mov r11, imm64
                byte_width: 8,
                kind: X86_64RelocationSiteKind::Absolute64,
            });
        }
        cursor += win64_import_arg_width(
            operands,
            arg_start,
            index,
            plan.as_ref().and_then(|plan| plan.parameters.get(index)),
        );
    }
    if win64_import_arg_is_staged(operands.get(table_index)) {
        sites.push(X86_64RelocationSite {
            operand_index: Some(table_index),
            byte_offset: cursor + 2, // inside the table load's mov r11, imm64
            byte_width: 8,
            kind: X86_64RelocationSiteKind::Absolute64,
        });
    }
    cursor += 10 + 7; // table load: mov r11, imm64 + mov rax, [r11+disp32]
    cursor += 7 + 2 + rsp_adjust_width(reserve); // fn-ptr read + call rax + add rsp
    if result_present
        && plan
            .as_ref()
            .and_then(|plan| plan.result.as_ref())
            .is_some_and(|placement| !win64_result_is_indirect(placement))
        && operands.first().is_some_and(|operand| {
            operand.runtime_scalar_integer().is_some()
                || operand.runtime_scalar_float().is_some()
                || win64_aggregate_operand(operand).is_some()
        })
    {
        sites.push(X86_64RelocationSite {
            operand_index: Some(0),
            byte_offset: cursor + 2, // inside the result mov r11, imm64
            byte_width: 8,
            kind: X86_64RelocationSiteKind::Absolute64,
        });
    }
    sites
}

fn host_call_relocation_sites<T: InstructionOperandLike>(
    operation_key: HostOperationKey,
    operands: &[T],
) -> Vec<X86_64RelocationSite> {
    host_call_relocation_sites_for_policy(CallingPolicy::MicrosoftX64, operation_key, operands)
}

fn host_call_relocation_sites_for_policy<T: InstructionOperandLike>(
    policy: CallingPolicy,
    operation_key: HostOperationKey,
    operands: &[T],
) -> Vec<X86_64RelocationSite> {
    if policy == CallingPolicy::SystemVAMD64
        && matches!(
            operation_key.capability,
            HostCapability::Unknown | HostCapability::Custom(_)
        )
    {
        return sysv_import_layout(operands, true)
            .map(|layout| layout.relocation_sites)
            .unwrap_or_default();
    }
    if policy != CallingPolicy::MicrosoftX64 {
        return Vec::new();
    }
    match (operation_key.capability, operation_key.operation) {
        (
            HostCapability::Stdin | HostCapability::Stdout | HostCapability::Stderr,
            HostOperation::GetStdHandle,
        ) => win64_import_call_relocation_sites(operands, false, false),
        (HostCapability::Process, HostOperation::ExitProcess)
        | (HostCapability::Clock, HostOperation::Sleep) => {
            // Single-u32-arg kernel32 calls now share the plan-driven general
            // import marshaller and therefore its relocation walker.
            win64_import_call_relocation_sites(operands, false, false)
        }
        (HostCapability::Input, HostOperation::KeyState) => {
            // Layout: sub(4) + vk marshalling (17 runtime / 5 const) + call(5)
            // + add(4) + movzx(3) + mov r11,imm64(10) + store(7).
            let vk_is_runtime = operands
                .get(1)
                .is_some_and(|operand| operand.runtime_scalar_integer().is_some());
            let vk_width = if vk_is_runtime { 17 } else { 5 };
            let mut sites = Vec::new();
            if vk_is_runtime {
                sites.push(X86_64RelocationSite {
                    operand_index: Some(1),
                    byte_offset: 4 + 2, // inside the vk mov r11, imm64
                    byte_width: 8,
                    kind: X86_64RelocationSiteKind::Absolute64,
                });
            }
            sites.push(X86_64RelocationSite {
                operand_index: None,
                byte_offset: 4 + vk_width + 1, // past the call opcode
                byte_width: 4,
                kind: X86_64RelocationSiteKind::Relative32,
            });
            sites.push(X86_64RelocationSite {
                operand_index: Some(0),
                byte_offset: 4 + vk_width + 5 + 4 + 3 + 2, // inside result mov r11, imm64
                byte_width: 8,
                kind: X86_64RelocationSiteKind::Absolute64,
            });
            sites
        }
        (HostCapability::Clock, HostOperation::TickCount) => {
            // 0-arg value-returning call through the general import-call layout
            // (call at 4+1; result-region base at 13+2 -- identical to the
            // original bespoke site list).
            win64_import_call_relocation_sites(operands, true, false)
        }
        (
            HostCapability::Clock,
            HostOperation::MonotonicTicks
            | HostOperation::MonotonicTicksPerSecond
            | HostOperation::WallClockRaw,
        ) => win64_out_param_call_relocation_sites(),
        (
            HostCapability::Clock,
            HostOperation::WallClockUnitsPerSecond | HostOperation::WallClockEpochOffsetSeconds,
        ) => constant_result_relocation_sites(),
        (HostCapability::Gui, _) => {
            // Value-returning general import calls (mirrors the encode arm).
            win64_import_call_relocation_sites(operands, true, false)
        }
        (HostCapability::Filesystem, _) => {
            // Value-returning general import calls; read_errno's deref shifts
            // the result-store site by 2 (mirrors the encode arm).
            win64_import_call_relocation_sites(operands, true, operation_key.dereferences_result())
        }
        (HostCapability::Unknown | HostCapability::Custom(_), _) => {
            // Provides-authored imports (mirrors the encode arm).
            win64_import_call_relocation_sites(operands, true, false)
        }
        (
            HostCapability::Stdout | HostCapability::Stderr,
            HostOperation::Write | HostOperation::WriteFile,
        )
        | (HostCapability::Stdin, HostOperation::ReadFile) => {
            let mut sites = Vec::new();
            let Ok((pointer_index, length_index)) = file_pointer_and_length_indices(operands)
            else {
                return sites;
            };
            let mut cursor = if pointer_index == 1 { 9 } else { 7 };

            if operands.get(pointer_index).is_some_and(|operand| {
                operand.data_address().is_some()
                    || operand.runtime_string_pointer().is_some()
                    || operand.runtime_pointee_string_pointer().is_some()
            }) {
                sites.push(X86_64RelocationSite {
                    operand_index: Some(pointer_index),
                    byte_offset: cursor + 2,
                    byte_width: 8,
                    kind: X86_64RelocationSiteKind::Absolute64,
                });
            }
            cursor += file_pointer_operand_width(operands.get(pointer_index));
            if operation_key.capability == HostCapability::Stdin
                && operation_key.operation == HostOperation::ReadFile
            {
                cursor += 3;
            }

            if operands.get(length_index).is_some_and(|operand| {
                operand.runtime_string_length().is_some()
                    || operand.runtime_pointee_string_length().is_some()
            }) {
                sites.push(X86_64RelocationSite {
                    operand_index: Some(length_index),
                    byte_offset: cursor + 2,
                    byte_width: 8,
                    kind: X86_64RelocationSiteKind::Absolute64,
                });
            }
            cursor += file_length_operand_width(operands.get(length_index));
            cursor += 15; // lea r9 + qword null + call opcode

            sites.push(X86_64RelocationSite {
                operand_index: None,
                byte_offset: cursor,
                byte_width: 4,
                kind: X86_64RelocationSiteKind::Relative32,
            });
            sites
        }
        _ => Vec::new(),
    }
}

fn file_pointer_and_length_indices<T: InstructionOperandLike>(
    operands: &[T],
) -> Result<(usize, usize), Diagnostic> {
    match operands.first() {
        Some(operand) if operand.immediate_integer().is_some() => Ok((1, 2)),
        Some(operand)
            if operand.data_address().is_some()
                || operand.runtime_string_pointer().is_some()
                || operand.runtime_pointee_string_pointer().is_some() =>
        {
            Ok((0, 1))
        }
        _ => Err(Diagnostic::error(
            "cannot encode X86_64 file operation: unsupported operand shape",
        )),
    }
}

fn file_pointer_operand_width<T: InstructionOperandLike>(operand: Option<&T>) -> usize {
    match operand {
        Some(operand) if operand.data_address().is_some() => 10,
        Some(operand) if operand.runtime_string_pointer().is_some() => 17,
        Some(operand) if operand.runtime_pointee_string_pointer().is_some() => 24,
        _ => 0,
    }
}

fn file_length_operand_width<T: InstructionOperandLike>(operand: Option<&T>) -> usize {
    match operand {
        Some(operand) if operand.byte_length().is_some() => 6,
        Some(operand) if operand.runtime_string_length().is_some() => 17,
        Some(operand) if operand.runtime_pointee_string_length().is_some() => 24,
        _ => 0,
    }
}

pub fn runtime_text_literal_compare_width(literal: &str) -> usize {
    10 + literal.len() * 15 + 36
}

// Write a literal's bytes into a runtime text buffer at a fixed byte offset
// (the first segment of a concatenation). r15 = buffer (reloc @ +2); store each
// literal byte at [r15 + byte_offset + i].
pub fn runtime_text_literal_segment_write_width(literal: &str) -> usize {
    10 + literal.len() * 8
}

pub fn encode_runtime_text_literal_segment_write(
    byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_text_literal_segment_write_width(literal));
    append_mov_r15_imm64(&mut bytes, 0); // buffer base (reloc @ +2)
    for (i, byte) in literal.as_bytes().iter().enumerate() {
        let disp = disp32(byte_offset + i)?;
        bytes.extend([0x41, 0xc6, 0x87]); // mov byte [r15 + disp32], imm8
        bytes.extend(disp.to_le_bytes());
        bytes.push(*byte);
    }
    debug_assert_eq!(
        bytes.len(),
        runtime_text_literal_segment_write_width(literal)
    );
    Ok(bytes)
}

// Append a literal to a runtime text buffer, growing the {ptr,len} descriptor.
// r15 = buffer (reloc @ +2); r14 = descriptor base (reloc @ +12 via offset 10);
// rax = current len; store literal bytes at [r15 + len + i]; then
// descriptor.ptr = buffer, descriptor.len += literal.len.
pub const RUNTIME_TEXT_LITERAL_APPEND_TARGET_IMM_OFFSET: usize = 10;

pub fn runtime_text_literal_append_width(literal: &str) -> usize {
    // mov r15,imm64 (10) + mov r14,imm64 (10) + mov rax,[r14+len] (7) = 27
    // + per byte: mov cl,imm8 (2) + mov [r15+rax],cl (4) + inc rax (3) = 9
    // + mov [r14+ptr],r15 (7) + mov [r14+len],rax (7) = 14
    27 + literal.len() * 9 + 14
}

pub fn encode_runtime_text_literal_append(
    target_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let ptr_disp = disp32(target_offset)?;
    let len_disp = disp32(target_offset + 8)?;
    let lit_len = i32::try_from(literal.len()).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot append literal of length `{}` yet",
            literal.len()
        ))
    })?;
    let mut bytes = Vec::with_capacity(runtime_text_literal_append_width(literal));
    append_mov_r15_imm64(&mut bytes, 0); // buffer base (reloc @ +2)
    append_mov_r14_imm64(&mut bytes, 0); // descriptor base (reloc @ +12)
    // rax = current length.
    bytes.extend([0x49, 0x8b, 0x86]); // mov rax, [r14 + len_disp]
    bytes.extend(len_disp.to_le_bytes());
    // append bytes at buffer[rax]; rax advances per byte.
    for byte in literal.as_bytes() {
        bytes.extend([0xb1, *byte]); // mov cl, imm8
        bytes.extend([0x41, 0x88, 0x0c, 0x07]); // mov [r15+rax], cl
        bytes.extend([0x48, 0xff, 0xc0]); // inc rax
    }
    // descriptor.ptr = buffer (r15).
    bytes.extend([0x4d, 0x89, 0xbe]); // mov [r14 + ptr_disp], r15
    bytes.extend(ptr_disp.to_le_bytes());
    // descriptor.len = original_len + literal.len.  rax currently = len + lit.len
    // (advanced once per byte), so just store rax.
    let _ = lit_len;
    bytes.extend([0x49, 0x89, 0x86]); // mov [r14 + len_disp], rax
    bytes.extend(len_disp.to_le_bytes());
    debug_assert_eq!(bytes.len(), runtime_text_literal_append_width(literal));
    Ok(bytes)
}

pub fn runtime_text_literal_append_to_runtime_pointee_width(literal: &str) -> usize {
    // Like the non-pointee literal append (41 + len*9) plus one extra
    // `mov r14, [r14 + disp32]` (7) to dereference the runtime pointer.
    48 + literal.len() * 9
}

/// Appends a compile-time literal to a target string whose `{ptr,len}` descriptor
/// is reached through a RUNTIME pointer (`*(frame + pointer_byte_offset) +
/// field_byte_offset`). Mirrors `encode_runtime_text_literal_append`, dereferencing
/// the runtime pointer into r14 first. r15=materialized buffer base. The descriptor
/// `ptr` is overwritten to the buffer base and `len` grows by the literal length.
pub fn encode_runtime_text_literal_append_to_runtime_pointee(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let ptr_disp = disp32(field_byte_offset)?;
    let len_disp = disp32(field_byte_offset + 8)?;
    let mut bytes = Vec::with_capacity(runtime_text_literal_append_to_runtime_pointee_width(
        literal,
    ));
    append_mov_r15_imm64(&mut bytes, 0); // buffer base (reloc @ instruction start)
    append_mov_r14_imm64(&mut bytes, 0); // runtime-frame base (reloc @ +10 == TARGET offset)
    append_load_r14_from_r14(&mut bytes, pointer_byte_offset)?; // r14 = runtime pointer
    // rax = current length.
    bytes.extend([0x49, 0x8b, 0x86]); // mov rax, [r14 + len_disp]
    bytes.extend(len_disp.to_le_bytes());
    // append bytes at buffer[rax]; rax advances per byte.
    for byte in literal.as_bytes() {
        bytes.extend([0xb1, *byte]); // mov cl, imm8
        bytes.extend([0x41, 0x88, 0x0c, 0x07]); // mov [r15+rax], cl
        bytes.extend([0x48, 0xff, 0xc0]); // inc rax
    }
    // descriptor.ptr = buffer (r15).
    bytes.extend([0x4d, 0x89, 0xbe]); // mov [r14 + ptr_disp], r15
    bytes.extend(ptr_disp.to_le_bytes());
    // descriptor.len = original_len + literal.len (rax advanced once per byte).
    bytes.extend([0x49, 0x89, 0x86]); // mov [r14 + len_disp], rax
    bytes.extend(len_disp.to_le_bytes());
    debug_assert_eq!(
        bytes.len(),
        runtime_text_literal_append_to_runtime_pointee_width(literal)
    );
    Ok(bytes)
}

/// The target-region `mov r15, imm64` is the second instruction (after the
/// 10-byte buffer `mov r14, imm64`), so its relocated immediate sits at offset
/// 10 (the relocation planner adds the +2 imm position itself).
pub const RUNTIME_TEXT_BUFFER_MATERIALIZE_TARGET_IMM_OFFSET: usize = 10;

pub fn runtime_text_buffer_materialize_width() -> usize {
    // mov r14,imm64(10) + mov r15,imm64(10) + load rax,[r15+t](7) + load rcx,[r15+t+8](7)
    // + mov r11,rcx(3) + mov r10,r14(3) + push rsi;push rdi(2) + mov rsi,rax(3)
    // + mov rdi,r10(3) + rep movsb(2) + pop rdi;pop rsi(2) + store r14(7) + store r11(7)
    66
}

/// Materializes a fresh writable text buffer for an in-place concat: copies the
/// current `{ptr,len}` descriptor at `target_offset` (in the relocated target
/// region) into the relocated `buffer`, then repoints the descriptor at the
/// buffer (ptr=buffer, len unchanged). A later append then grows the copy in
/// place without disturbing the original literal/source the descriptor named.
pub fn encode_runtime_text_buffer_materialize(target_offset: usize) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_text_buffer_materialize_width());
    append_mov_r14_imm64(&mut bytes, 0); // buffer base (reloc @ instruction start)
    append_mov_r15_imm64(&mut bytes, 0); // target region base (reloc @ +10)
    append_load_rax_from_r15(&mut bytes, target_offset)?; // rax = source pointer
    append_load_rcx_from_r15(&mut bytes, target_offset + 8)?; // rcx = source length
    append_mov_r11_rcx(&mut bytes); // r11 = saved length
    append_mov_r10_r14(&mut bytes); // r10 = dest = buffer base
    append_push_rsi_rdi(&mut bytes);
    append_mov_rsi_rax(&mut bytes); // rsi = source pointer
    append_mov_rdi_r10(&mut bytes); // rdi = dest
    append_rep_movsb(&mut bytes); // copy rcx bytes
    append_pop_rdi_rsi(&mut bytes);
    append_store_r14_to_r15(&mut bytes, target_offset)?; // descriptor.ptr = buffer
    append_store_r11_to_r15(&mut bytes, target_offset + 8)?; // descriptor.len = original length
    debug_assert_eq!(bytes.len(), runtime_text_buffer_materialize_width());
    Ok(bytes)
}

pub fn runtime_text_literal_compare_branch_next_offset(byte_index: usize) -> usize {
    10 + byte_index * 15 + 15
}

/// Exact register writes of the literal-buffer guard encoder below. Every
/// path materializes the buffer in r15 and loads the compared/delimiter byte
/// through AL before its flag-setting comparisons.
pub fn runtime_text_literal_compare_register_writes() -> RegisterSet {
    RegisterSet::new([MachineRegister::X86Rax, MachineRegister::X86R15])
}

pub fn runtime_text_literal_compare_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

pub fn encode_runtime_text_literal_compare(
    literal: &str,
    failure_branch_distances: impl ExactSizeIterator<Item = isize>,
    delimiter_failure_branch_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    if literal.len() != failure_branch_distances.len() {
        return Err(Diagnostic::error(format!(
            "X86_64 runtime text guard expected {} branch distance(s), got {}",
            literal.len(),
            failure_branch_distances.len()
        )));
    }

    let mut bytes = Vec::with_capacity(runtime_text_literal_compare_width(literal));
    append_mov_r15_imm64(&mut bytes, 0);
    for (byte_index, (expected_byte, failure_branch_distance)) in literal
        .as_bytes()
        .iter()
        .zip(failure_branch_distances)
        .enumerate()
    {
        append_load_al_from_r15(&mut bytes, byte_index)?;
        bytes.extend([0x3c, *expected_byte]); // cmp al, imm8
        append_jcc_rel32(&mut bytes, 0x85, failure_branch_distance)?; // jne
    }
    append_input_delimiter_check(
        &mut bytes,
        literal.len(),
        delimiter_failure_branch_distance - 4,
    )?;
    Ok(bytes)
}

// Compare a stored String (descriptor {ptr,len} at source storage) against a
// data-section literal of known length.
//
// The lowering wraps this compare as: write the optimistic result (text_ok=1) ->
// COMPARE -> write the failure result (text_ok=0). On a MATCH we must branch
// PAST the trailing "write 0" (keeping the optimistic 1); on a MISMATCH we fall
// through into it. So MATCH jumps to the external distance ("next guarded effect
// end") and MISMATCH falls through. Every internal match path funnels through a
// single terminal `jmp rel32` so emission only needs one branch offset.
//
// r15 = literal buffer (reloc @ instruction start +2); r14 = source base (reloc);
// rax = stored.ptr; r9 = stored.len; r8 = index; cl = scratch byte.
//
// Layout: [setup + compare loop + trailing delimiter check] ... fail: (fall
// through) ; match: jmp rel32(external)   <- terminal 5 bytes; rel32 end == width.
pub fn encode_runtime_text_storage_compare_bytes(
    source_offset: usize,
    literal_len: usize,
    match_branch_distance: isize,
    negated: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let literal_len_i = i32::try_from(literal_len).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot compare literal of length `{literal_len}` yet"
        ))
    })?;
    let mut bytes = Vec::new();
    let mut fail_fixups: Vec<usize> = Vec::new();
    let mut success_fixups: Vec<usize> = Vec::new();

    // r15 = literal base (reloc@+2); r14 = source base (reloc).
    append_mov_r15_imm64(&mut bytes, 0);
    append_mov_r14_imm64(&mut bytes, 0);
    append_load_rax_from_r14(&mut bytes, source_offset, 8)?; // rax = stored.ptr
    bytes.extend([0x4d, 0x8b, 0x8e]); // mov r9, [r14 + disp32]  (stored.len)
    bytes.extend(disp32(source_offset + 8)?.to_le_bytes());

    let mut jcc_fail = |bytes: &mut Vec<u8>, opcode: u8| {
        bytes.push(0x0f);
        bytes.push(opcode);
        fail_fixups.push(bytes.len());
        bytes.extend([0, 0, 0, 0]);
    };
    // stored.len < literal_len  => not equal.
    bytes.extend([0x49, 0x81, 0xf9]); // cmp r9, imm32
    bytes.extend(literal_len_i.to_le_bytes());
    jcc_fail(&mut bytes, 0x82); // jb fail
    bytes.extend([0x4d, 0x31, 0xc0]); // xor r8, r8 (index = 0)

    let loop_start = bytes.len();
    bytes.extend([0x49, 0x81, 0xf8]); // cmp r8, imm32 (literal_len)
    bytes.extend(literal_len_i.to_le_bytes());
    let to_trailing = {
        bytes.extend([0x0f, 0x83]); // jae rel32 -> trailing check
        let at = bytes.len();
        bytes.extend([0, 0, 0, 0]);
        at
    };
    bytes.extend([0x42, 0x8a, 0x0c, 0x00]); // mov cl, [rax+r8]
    bytes.extend([0x43, 0x3a, 0x0c, 0x07]); // cmp cl, [r15+r8]
    jcc_fail(&mut bytes, 0x85); // jne fail
    bytes.extend([0x49, 0xff, 0xc0]); // inc r8
    {
        bytes.push(0xe9); // jmp loop_start
        let at = bytes.len();
        bytes.extend([0, 0, 0, 0]);
        bytes[at..at + 4]
            .copy_from_slice(&((loop_start as isize - (at as isize + 4)) as i32).to_le_bytes());
    }

    // trailing: if stored.len == literal_len -> success; else stored[len] must
    // be a line delimiter for equality (input had a trailing terminator).
    let trailing = bytes.len();
    bytes[to_trailing..to_trailing + 4]
        .copy_from_slice(&((trailing as isize - (to_trailing as isize + 4)) as i32).to_le_bytes());
    bytes.extend([0x49, 0x81, 0xf9]); // cmp r9, imm32 (literal_len)
    bytes.extend(literal_len_i.to_le_bytes());
    {
        bytes.extend([0x0f, 0x84]); // je success
        success_fixups.push(bytes.len());
        bytes.extend([0, 0, 0, 0]);
    }
    bytes.extend([0x42, 0x8a, 0x0c, 0x00]); // mov cl, [rax+r8] (stored[literal_len])
    let mut je_success = |bytes: &mut Vec<u8>, imm: u8| {
        bytes.extend([0x80, 0xf9, imm]); // cmp cl, imm8
        bytes.extend([0x0f, 0x84]); // je success
        success_fixups.push(bytes.len());
        bytes.extend([0, 0, 0, 0]);
    };
    je_success(&mut bytes, 0x0a); // '\n'
    je_success(&mut bytes, 0x0d); // '\r'
    je_success(&mut bytes, 0x00); // '\0'
    {
        bytes.push(0xe9); // jmp fail (no delimiter -> not equal)
        fail_fixups.push(bytes.len());
        bytes.extend([0, 0, 0, 0]);
    }

    // Exit trampolines. The FIRST falls through to the instruction end (the
    // following "write text_ok = 0"); the SECOND jmps the external "next
    // guarded effect end" distance, skipping that write. `negated` (a `!=`
    // compare) swaps which OUTCOME routes where: `==` sends match outcomes
    // external; `!=` sends MISMATCH outcomes external -- the flag was ignored
    // and `!=` behaved as `==` (the frame-slot text-comparison writer's
    // preset-1/compare/write-0 pattern kept the 1 for equal strings). Same
    // byte layout either way; only the fixup routing differs.
    let (end_fixups, external_fixups) = if negated {
        (success_fixups, fail_fixups)
    } else {
        (fail_fixups, success_fixups)
    };
    let mismatch = bytes.len();
    bytes.push(0xe9);
    let mismatch_jmp_at = bytes.len();
    bytes.extend([0, 0, 0, 0]);
    for fixup in &end_fixups {
        bytes[*fixup..*fixup + 4]
            .copy_from_slice(&((mismatch as isize - (*fixup as isize + 4)) as i32).to_le_bytes());
    }

    let matched = bytes.len();
    for fixup in &external_fixups {
        bytes[*fixup..*fixup + 4]
            .copy_from_slice(&((matched as isize - (*fixup as isize + 4)) as i32).to_le_bytes());
    }
    bytes.push(0xe9); // jmp match target (rel32)
    let match_jmp_at = bytes.len();
    bytes.extend((match_branch_distance as i32).to_le_bytes());

    let width = bytes.len();
    // mismatch path jumps to the instruction end (the trailing write-0).
    bytes[mismatch_jmp_at..mismatch_jmp_at + 4]
        .copy_from_slice(&((width as isize - (mismatch_jmp_at as isize + 4)) as i32).to_le_bytes());
    debug_assert_eq!(
        match_jmp_at + 4,
        width,
        "match jmp must terminate the instruction"
    );

    Ok(bytes)
}

/// Exact register writes of the descriptor-vs-literal content comparison.
/// The emitted loop owns both relocated bases, pointer/length/index state,
/// and CL as its byte scratch.
pub fn runtime_text_storage_compare_register_writes() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::X86Rax,
        MachineRegister::X86Rcx,
        MachineRegister::X86R8,
        MachineRegister::X86R9,
        MachineRegister::X86R14,
        MachineRegister::X86R15,
    ])
}

pub fn runtime_text_storage_compare_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::Flags])
}

/// Byte offset (within a `CompareRuntimeTextStorage`) of the rel32 displacement
/// end of the terminal failure `jmp` -- i.e. the instruction width. Emission
/// anchors the failure branch distance here.
pub fn runtime_text_storage_compare_failure_branch_offset(literal_len: usize) -> usize {
    runtime_text_storage_compare_width_x86(literal_len)
}

pub fn runtime_text_storage_compare_width_x86(literal_len: usize) -> usize {
    // Encode once with placeholder distance to recover the authoritative width.
    encode_runtime_text_storage_compare_bytes(0, literal_len, 0, false)
        .map(|bytes| bytes.len())
        .unwrap_or(0)
}

// --- Frame-indexed String descriptor write + literal append ---
//
// The String descriptor lives at `*(frame+descriptor_offset) + index*elem +
// field` (a slice element reached through the slice's data pointer). Both
// encoders share a fixed 34-byte address-computation prefix that leaves the
// element address in rax:
//   mov r14,imm64(frame) (10, reloc@+2) ; mov rax,[r14+descriptor] (7)
//   mov r11,[r14+index] (7) ; imul r11,r11,elem (7) ; add rax,r11 (3)
// so the second relocated immediate (data/buffer) always sits at offset 36.
const FRAME_INDEXED_STRING_PREFIX_WIDTH: usize = 34;
pub const RUNTIME_FRAME_INDEXED_STRING_DATA_IMM_OFFSET: usize = FRAME_INDEXED_STRING_PREFIX_WIDTH;

fn append_frame_indexed_element_address_into_rax(
    bytes: &mut Vec<u8>,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
) -> Result<(), Diagnostic> {
    append_mov_r14_imm64(bytes, 0); // frame base (reloc @ +2)
    append_load_rax_from_r14(bytes, descriptor_offset, 8)?; // rax = slice data ptr
    append_load_r11_from_r14(bytes, index_offset)?; // r11 = index
    append_imul_r11_imm32(bytes, element_scale(element_byte_size)?);
    append_add_rax_r11(bytes); // rax = element address
    debug_assert_eq!(bytes.len(), FRAME_INDEXED_STRING_PREFIX_WIDTH);
    Ok(())
}

pub fn runtime_text_literal_append_to_runtime_frame_indexed_width(
    _element_byte_size: usize,
    _field_byte_offset: usize,
    literal: &str,
) -> usize {
    // prefix (34) + mov r15,imm64 buffer (10) + mov r11,[rax+field+8] len (7)
    // + per byte: mov cl,imm8 (2) + mov [r15+r11],cl (4) + inc r11 (3) = 9
    // + store r15->[rax+field] ptr (7) + store r11->[rax+field+8] len (7)
    FRAME_INDEXED_STRING_PREFIX_WIDTH + 17 + literal.len() * 9 + 14
}

pub fn encode_runtime_text_literal_append_to_runtime_frame_indexed(
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_text_literal_append_to_runtime_frame_indexed_width(
        element_byte_size,
        field_byte_offset,
        literal,
    ));
    append_frame_indexed_element_address_into_rax(
        &mut bytes,
        descriptor_offset,
        index_offset,
        element_byte_size,
    )?;
    // r15 = buffer (reloc @ prefix+2); r11 = current len from the indexed descriptor.
    append_mov_r15_imm64(&mut bytes, 0);
    append_load_r11_from_rax(&mut bytes, field_byte_offset + 8)?;
    // append bytes at buffer[len]; r11 advances per byte.
    for byte in literal.as_bytes() {
        bytes.extend([0xb1, *byte]); // mov cl, imm8
        bytes.extend([0x43, 0x88, 0x0c, 0x1f]); // mov [r15+r11], cl
        bytes.extend([0x49, 0xff, 0xc3]); // inc r11
    }
    // descriptor.ptr = buffer (r15); descriptor.len = r11 (already grown).
    append_store_r15_to_rax(&mut bytes, field_byte_offset)?;
    append_store_r11_to_rax(&mut bytes, field_byte_offset + 8)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_text_literal_append_to_runtime_frame_indexed_width(
            element_byte_size,
            field_byte_offset,
            literal
        )
    );
    Ok(bytes)
}

// --- Machine-indexed String descriptor write ---
//
// Writes {ptr,len} into a machine-owned array element `machine[base + index*elem
// + field]` (the array is inline, so no pointer deref -- unlike the frame slice
// variant). The index lives in a runtime-frame slot. Fixed prefix leaves the
// element address in r15:
//   mov r15,imm64(machine) (10, reloc@+2) ; mov r14,imm64(frame) (10, reloc@+12)
//   mov r11,[r14+index] (7) ; imul r11,r11,elem (7) ; add r15,r11 (3)
// so the runtime-frame reloc imm is at offset 12 and the literal reloc at 39.
const MACHINE_INDEXED_STRING_PREFIX_WIDTH: usize = 37;
pub const MACHINE_INDEXED_STRING_FRAME_IMM_OFFSET: usize = 10;
pub const MACHINE_INDEXED_STRING_DATA_IMM_OFFSET: usize = MACHINE_INDEXED_STRING_PREFIX_WIDTH;

pub fn runtime_value_compare_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
) -> usize {
    // cmp (3; 4 with the 0x66 prefix at 2-byte width) + jcc rel32 (6).
    let compare_width = if byte_size == 2 { 4 } else { 3 };
    runtime_value_operand_width(runtime_value_operands, left)
        + runtime_value_operand_width(runtime_value_operands, right)
        + compare_width
        + 6
}

pub fn encode_runtime_value_compare(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
    byte_size: usize,
    failure_branch_distance: isize,
    operator: StateGuardOperator,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_value_compare_width(
        runtime_value_operands,
        byte_size,
        left,
        right,
    ));
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R10, left)?;
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R11, right)?;
    append_cmp_r10_r11(&mut bytes, byte_size)?;
    append_failure_branch(&mut bytes, operator, failure_branch_distance - 4, false)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_value_compare_width(runtime_value_operands, byte_size, left, right)
    );
    Ok(bytes)
}

/// Closed may-write ceiling of the recursive runtime-value comparison
/// encoder. Operand shapes select subsets of this encoder-owned bank; keeping
/// the family ceiling beside the evaluator makes the retained evidence sound
/// across nested arithmetic, conversions, indexed loads, and text equality.
pub fn runtime_value_compare_register_write_ceiling() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::X86Rax,
        MachineRegister::X86Rcx,
        MachineRegister::X86Rdx,
        MachineRegister::X86R8,
        MachineRegister::X86R9,
        MachineRegister::X86R10,
        MachineRegister::X86R11,
        MachineRegister::X86R15,
        MachineRegister::X86Xmm(0),
        MachineRegister::X86Xmm(1),
    ])
}

fn runtime_value_operand_uses_stack(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    operand: RuntimeValueOperandHandle,
) -> bool {
    if runtime_value_operands.binary(operand).is_some() {
        // Every binary operand preserves its recursively evaluated left value
        // with push r10 / pop r10 around evaluation of the right value.
        true
    } else if let Some((source, ..)) = runtime_value_operands.convert(operand) {
        runtime_value_operand_uses_stack(runtime_value_operands, source)
    } else {
        false
    }
}

pub fn runtime_value_compare_additional_machine_state(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
) -> MachineStateSet {
    let mut state = MachineStateSet::new([MachineState::Flags]);
    if runtime_value_operand_uses_stack(runtime_value_operands, left)
        || runtime_value_operand_uses_stack(runtime_value_operands, right)
    {
        state = state.union(MachineStateSet::new([MachineState::StackPointer]));
    }
    state
}

pub fn runtime_machine_integer_write_width(_byte_offset: usize, byte_size: usize) -> usize {
    // mov r15,imm64 (10) + mov rax,imm64 (10) + store [r15+disp32] (7; 8 with
    // the 0x66 prefix for a 2-byte store).
    if byte_size == 2 { 28 } else { 27 }
}

pub fn encode_runtime_machine_integer_write(
    byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    // Write rung 1b: DELEGATES byte-for-byte to the place materializer
    // (unit-pinned identity). The region on the transitional place is
    // documentation only -- a direct place's bytes never consult it; the
    // walker patches the base from the instruction's own region.
    let target =
        place_copy::transitional_place(omega_target_operations::RuntimeStorageRegion::Machine)
            .with_step(omega_target_operations::PlaceStep::ConstOffset(byte_offset))
            .expect("a direct place is two steps, within PLACE_MAX_STEPS");
    place_copy::encode_place_integer_write(&target, value, byte_size).map(|(bytes, _)| bytes)
}

pub fn runtime_machine_indexed_integer_write_width(
    index_region: omega_target_operations::RuntimeStorageRegion,
    _element_byte_size: usize,
    _byte_size: usize,
) -> usize {
    // mov r15,imm64 (10) [+ mov r10,imm64 (10) for RuntimeFrame index]
    // + mov rax,[base+index_off] (7) + imul rax,rax,imm32 (7)
    // + add r15,rax (3) + mov rax,imm64 (10) + store [r15+disp] (7).
    match index_region {
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame => 54,
        omega_target_operations::RuntimeStorageRegion::Machine => 44,
    }
}

/// For x86_64 the runtime-frame index base is loaded by the second instruction
/// (`mov r10, imm64`), which begins 10 bytes into the sequence; the relocation
/// planner adds the +2 immediate offset itself.
pub fn runtime_machine_indexed_integer_runtime_frame_address_offset() -> usize {
    10
}

pub fn encode_runtime_machine_indexed_integer_write(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "X86_64 MVP encoder cannot store {byte_size}-byte machine indexed integers yet"
        )));
    }
    let element_scale = i32::try_from(element_byte_size).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot scale machine index by element size `{element_byte_size}`"
        ))
    })?;
    let _ = element_scale;
    // Write rung 1c: DELEGATES to the place materializer -- a REGISTER
    // RENAME canonicalization (the retired layout staged the index through
    // RAX and a frame-resident index base through r10; the materializer
    // uses the r11 discipline). Same instruction WIDTHS at every position,
    // so the walker's +10 frame-base offset and the width fn hold as-is;
    // the differential legs oracle the byte change.
    let target =
        place_copy::transitional_place(omega_target_operations::RuntimeStorageRegion::Machine)
            .with_step(omega_target_operations::PlaceStep::ConstOffset(
                base_byte_offset,
            ))
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ScaledIndex {
                    index_region,
                    index_offset,
                    element_byte_size,
                })
            })
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ConstOffset(
                    field_byte_offset,
                ))
            })
            .expect("a machine-indexed place is four steps, within PLACE_MAX_STEPS");
    let (bytes, _) = place_copy::encode_place_integer_write(&target, value, byte_size)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_machine_indexed_integer_write_width(index_region, element_byte_size, byte_size)
    );
    Ok(bytes)
}

/// Relocation imm offset (pre-`+2`) of the frame base loaded for the target slot
/// store in `encode_runtime_storage_copy_from_runtime_frame_base_indexed_to_runtime_frame`.
pub const FRAME_BASE_INDEXED_COPY_TARGET_FRAME_IMM_OFFSET: usize = 41;

/// Start of the SECOND `mov r15,imm64` (the machine base) inside the
/// frame-source variant of the write half -- the machine relocation; the
/// relocation planner adds the +2 immediate offset itself.
pub fn runtime_storage_copy_to_runtime_machine_indexed_frame_source_machine_base_offset() -> usize {
    17
}

/// Start of the `mov r10,imm64` (the frame base for a FRAME-resident index)
/// inside the write half -- the frame relocation; sits after the source
/// load (+17) and after the frame-source machine re-load when present (+10).
pub fn runtime_storage_copy_to_runtime_machine_indexed_frame_index_base_offset(
    source_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    if source_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        27
    } else {
        17
    }
}

pub fn runtime_storage_copy_machine_indexed_to_machine_indexed_width(
    source_index_region: omega_target_operations::RuntimeStorageRegion,
    target_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    // Read part: mov r15,imm64 (10) + mov eax,[idx base] (7) + imul rax,imm32 (7)
    // + add r15,rax (3) + load rax,[r15+disp] (7) = 34.
    // Write part: mov r15,imm64 (10) + mov r10d,[idx base] (7) + imul r10,imm32
    // (7) + add r15,r10 (3) + store [r15+disp] (7) = 34.
    // A FRAME-resident index on either side inserts its own frame-base
    // `mov r10,imm64` (+10) before that side's index load.
    runtime_storage_copy_machine_indexed_read_part_width(source_index_region)
        + if target_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
            44
        } else {
            34
        }
}

/// Width of the READ half of the dual-indexed copy (also the start of the
/// WRITE part's machine-base `mov r15,imm64`).
pub fn runtime_storage_copy_machine_indexed_read_part_width(
    source_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    if source_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        44
    } else {
        34
    }
}

/// Width of [`encode_runtime_storage_copy_from_runtime_machine_double_indexed_to_runtime_storage`].
/// MUST equal the emitter exactly. Any frame-resident index adds one r10
/// frame-base load (mov r10,imm64 at +10).
pub fn runtime_storage_copy_from_runtime_machine_double_indexed_to_runtime_storage_width(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    // mov r15,imm64 (10) [+ mov r10,imm64 (10) if any frame index]
    // + mov eax,[..+outer] (7) + mov r11d,[..+inner] (7)
    // + imul rax,imm32 (7) + imul r11,imm32 (7) + add r15,rax (3)
    // + add r15,r11 (3) + load rax,[r15+disp] (7)
    // + mov r15,imm64 (10) + store [r15+target] (7)
    if double_indexed_any_frame(outer_index_region, inner_index_region) {
        78
    } else {
        68
    }
}

fn double_indexed_any_frame(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> bool {
    outer_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
        || inner_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
}

/// Start of the `mov r10,imm64` frame-base load inside the double-indexed
/// read (pre-`+2`; present only when an index is frame-resident).
/// Write rung 1c (the canonicalized double-indexed WRITE): the OUTER
/// frame-resident index base (`mov r11,imm64`) begins right after the opening
/// machine mov.
pub fn runtime_machine_double_indexed_integer_write_outer_frame_offset() -> usize {
    10
}

/// The INNER frame-resident index base (`mov r10,imm64`): after the opening
/// mov + the outer index sequence (17 cross-region / 7 same-region) + its imul.
pub fn runtime_machine_double_indexed_integer_write_inner_frame_offset(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    let outer = if outer_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
    {
        17
    } else {
        7
    };
    10 + outer + 7
}

pub fn runtime_storage_copy_from_runtime_machine_double_indexed_frame_base_offset() -> usize {
    10
}

/// Start of the WRITE-half `mov r15,imm64` (the target-region relocation,
/// pre-`+2`) inside the double-indexed read.
pub fn runtime_storage_copy_from_runtime_machine_double_indexed_target_base_offset(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    runtime_storage_copy_from_runtime_machine_double_indexed_to_runtime_storage_width(
        outer_index_region,
        inner_index_region,
    ) - 17
}

/// Width of [`encode_runtime_storage_copy_to_runtime_machine_double_indexed_from_runtime_storage`].
pub fn runtime_storage_copy_to_runtime_machine_double_indexed_from_runtime_storage_width(
    source_region: omega_target_operations::RuntimeStorageRegion,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    // mov r15,imm64 (10) [+ mov r10,imm64 (10) if any frame place]
    // + mov r14,[..+src] (7) + mov eax,[..+outer] (7) + mov r11d,[..+inner] (7)
    // + imul rax,imm32 (7) + imul r11,imm32 (7) + add r15,rax (3)
    // + add r15,r11 (3) + store [r15+disp],r14 (7)
    if double_indexed_write_any_frame(source_region, outer_index_region, inner_index_region) {
        68
    } else {
        58
    }
}

fn double_indexed_write_any_frame(
    source_region: omega_target_operations::RuntimeStorageRegion,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> bool {
    [source_region, outer_index_region, inner_index_region]
        .iter()
        .any(|region| *region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame)
}

/// Width of [`encode_runtime_machine_double_indexed_integer_write`].
pub fn runtime_machine_double_indexed_integer_write_width(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    // Canonicalized by the place materializer (Write rung 1c): mov r15,imm64
    // (10) + per-index [cross-region: mov reg,imm64 (10) + load (7) | same-
    // region: load (7)] + imul (7) each + add r15,r11 (3) + add r15,r10 (3)
    // + mov rax,imm64 (10) + store (7). Each FRAME index adds its OWN base
    // (r11 for the outer, r10 for the inner) -- no shared r10 anymore.
    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    61 + if outer_index_region == frame { 10 } else { 0 }
        + if inner_index_region == frame { 10 } else { 0 }
}

/// Const-value write into a both-runtime nested element (`grid[i][j] = 70`):
/// the address computation of the double-indexed read, then `mov rax, imm64`
/// and a width-correct store (rax is free after the adds).
pub fn encode_runtime_machine_double_indexed_integer_write(
    base_byte_offset: usize,
    outer_index_offset: usize,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    outer_stride: usize,
    inner_index_offset: usize,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_stride: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "X86_64 MVP encoder cannot store {byte_size}-byte double-indexed integers yet"
        )));
    }
    for region in [outer_index_region, inner_index_region] {
        if !matches!(
            region,
            omega_target_operations::RuntimeStorageRegion::Machine
                | omega_target_operations::RuntimeStorageRegion::RuntimeFrame
        ) {
            return Err(Diagnostic::error(
                "X86_64 MVP encoder cannot write a double-indexed integer with this index region yet",
            ));
        }
    }
    // Write rung 1c: DELEGATES to the place materializer -- CANONICALIZED:
    // the retired layout materialized ONE shared r10 frame base for BOTH
    // frame-resident indices and staged the outer index in RAX; the
    // materializer materializes each cross-region index base separately
    // (r11 then r10). Widths and frame-base reloc positions move -- the
    // width fn and the walker's per-index arm move in lockstep.
    let target =
        place_copy::transitional_place(omega_target_operations::RuntimeStorageRegion::Machine)
            .with_step(omega_target_operations::PlaceStep::ConstOffset(
                base_byte_offset,
            ))
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ScaledIndex {
                    index_region: outer_index_region,
                    index_offset: outer_index_offset,
                    element_byte_size: outer_stride,
                })
            })
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ScaledIndex {
                    index_region: inner_index_region,
                    index_offset: inner_index_offset,
                    element_byte_size: inner_stride,
                })
            })
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ConstOffset(
                    field_byte_offset,
                ))
            })
            .expect("a double-indexed place is five steps, within PLACE_MAX_STEPS");
    let (bytes, _) = place_copy::encode_place_integer_write(&target, value, byte_size)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_machine_double_indexed_integer_write_width(outer_index_region, inner_index_region,)
    );
    Ok(bytes)
}

pub fn runtime_pointee_integer_write_width(_field_byte_offset: usize, _byte_size: usize) -> usize {
    // mov r15,imm64 (10) + mov r15,[r15+ptr] (7) + mov rax,imm64 (10) + store [r15+field] (7)
    34
}

pub fn encode_runtime_pointee_integer_write(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "X86_64 MVP encoder cannot store {byte_size}-byte pointee integers yet"
        )));
    }
    // Write rung 1b: DELEGATES byte-for-byte to the place materializer
    // ([Const(ptr), Deref, Const(field)]; unit-pinned identity).
    let target =
        place_copy::transitional_place(omega_target_operations::RuntimeStorageRegion::RuntimeFrame)
            .with_step(omega_target_operations::PlaceStep::ConstOffset(
                pointer_byte_offset,
            ))
            .and_then(|place| place.with_step(omega_target_operations::PlaceStep::Deref))
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ConstOffset(
                    field_byte_offset,
                ))
            })
            .expect("a pointee place is four steps, within PLACE_MAX_STEPS");
    let (bytes, _) = place_copy::encode_place_integer_write(&target, value, byte_size)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_pointee_integer_write_width(field_byte_offset, byte_size)
    );
    Ok(bytes)
}

pub fn runtime_frame_indexed_integer_write_width(
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_size: usize,
) -> usize {
    // mov r14,imm64 (10) + mov r15,[r14+desc] (7) + mov r11,[r14+idx] (7)
    // + imul r11,r11,elem (7) + add r15,r11 (3) + mov rax,imm64 (10) + store [r15+field] (7)
    51
}

pub fn encode_runtime_frame_indexed_integer_write(
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "X86_64 MVP encoder cannot store {byte_size}-byte frame indexed integers yet"
        )));
    }
    // Write rung 1c: DELEGATES to the place materializer -- the SAME
    // instruction multiset REORDERED (the index pre-loads into r11 while
    // r15 still equals the frame base, BEFORE the descriptor deref consumes
    // it; the retired layout loaded the descriptor first through a separate
    // r14 base). Same width, one start relocation -- the Copy rung-1c-i
    // reorder precedent.
    let target =
        place_copy::transitional_place(omega_target_operations::RuntimeStorageRegion::RuntimeFrame)
            .with_step(omega_target_operations::PlaceStep::ConstOffset(
                descriptor_offset,
            ))
            .and_then(|place| place.with_step(omega_target_operations::PlaceStep::Deref))
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ScaledIndex {
                    index_region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                    index_offset,
                    element_byte_size,
                })
            })
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ConstOffset(
                    field_byte_offset,
                ))
            })
            .expect("a frame-indexed place is five steps, within PLACE_MAX_STEPS");
    let (bytes, _) = place_copy::encode_place_integer_write(&target, value, byte_size)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_frame_indexed_integer_write_width(element_byte_size, field_byte_offset, byte_size)
    );
    Ok(bytes)
}

pub fn runtime_frame_base_indexed_integer_write_width(
    _base_byte_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_size: usize,
) -> usize {
    // Canonicalized by the place materializer (Write rung 1c): mov r15,imm64
    // (10) + mov r11d,[r15+idx] (7) + imul r11,r11,elem (7) + add r15,r11 (3)
    // + mov rax,imm64 (10) + store [r15+base+field] (7). The retired layout's
    // redundant `mov r15,r14` is gone.
    44
}

pub fn encode_runtime_frame_base_indexed_integer_write(
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "X86_64 MVP encoder cannot store {byte_size}-byte frame base-indexed integers yet"
        )));
    }
    // Write rung 1c: DELEGATES to the place materializer -- CANONICALIZED
    // 47 -> 44 bytes (the retired layout staged the base in r14 and copied
    // it to r15 with a redundant `mov r15,r14`; the materializer opens in
    // r15 directly). The one frame-base relocation stays at instruction
    // start; the width fn shrinks in lockstep.
    let target =
        place_copy::transitional_place(omega_target_operations::RuntimeStorageRegion::RuntimeFrame)
            .with_step(omega_target_operations::PlaceStep::ConstOffset(
                base_byte_offset,
            ))
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ScaledIndex {
                    index_region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                    index_offset,
                    element_byte_size,
                })
            })
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ConstOffset(
                    field_byte_offset,
                ))
            })
            .expect("a frame-base-indexed place is four steps, within PLACE_MAX_STEPS");
    let (bytes, _) = place_copy::encode_place_integer_write(&target, value, byte_size)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_frame_base_indexed_integer_write_width(
            base_byte_offset,
            element_byte_size,
            field_byte_offset,
            byte_size
        )
    );
    Ok(bytes)
}

// --- Runtime text line read (Windows stdin via GetStdHandle + ReadFile) ---
//
// Self-contained instruction reading ONE logical line. Stdin is read one byte
// at a time (a bulk ReadFile would consume bytes belonging to the next
// read_line); the loop calls ReadFile with count=1 until a \n/\r/\0 delimiter,
// EOF (0 bytes), or capacity, then stores {ptr, len} at the target descriptor.
//
// Win64 callee-saved r13/r14/r15 survive the ReadFile call:
//   r14 = buffer base   r13 = stdin handle   r15 = line length / write index
//
// Branch displacements are resolved by post-patching recorded label positions,
// so the four relocation offsets are read back from the encoder rather than
// hand-computed; see `runtime_text_line_read_relocation_offsets`.

/// Byte offsets (within the instruction) of the four relocations the planner
/// must patch: buffer imm64, GetStdHandle call rel32, ReadFile call rel32,
/// and the target-descriptor imm64. Computed by encoding once with a dummy
/// target so the layout is authoritative (no hand-maintained constants).
pub struct RuntimeTextLineReadLayout {
    pub get_std_handle_call_offset: usize,
    pub read_file_call_offset: usize,
    pub target_imm_offset: usize,
    pub width: usize,
}

fn build_runtime_text_line_read(
    target_offset: usize,
    capacity: u32,
    is_bounded_buffer: bool,
) -> Result<(Vec<u8>, RuntimeTextLineReadLayout), Diagnostic> {
    validate_normalized_win64_get_std_handle_plan()?;
    let file_layout = normalized_win64_file_io_layout()?;
    let target_ptr_disp = disp32(target_offset)?;
    let target_len_disp = disp32(target_offset + 8)?;
    // Owned carrier: r14 must point at the inline bytes (`region + target_offset +
    // pointer_size`), so the imm64 relocates to the carrier's own region and an
    // `add` advances past the leading 8-byte length word.
    let carrier_bytes_disp = disp32(target_offset + 8)?;
    let mut bytes = Vec::with_capacity(128);

    // r14 = read buffer (imm64 at +2 relocated to the buffer data symbol, OR to
    // the carrier's own region for an owned `[u8; N]` target).
    bytes.extend([0x49, 0xbe]);
    bytes.extend(0u64.to_le_bytes());
    if is_bounded_buffer {
        // add r14, target_offset + pointer_size -> r14 = carrier inline bytes.
        bytes.extend([0x49, 0x81, 0xc6]);
        bytes.extend(carrier_bytes_disp.to_le_bytes());
    }
    append_sub_rsp(&mut bytes, file_layout.reserve);
    // mov ecx, -10 (STD_INPUT_HANDLE).
    bytes.push(0xb9);
    bytes.extend((-10i32).to_le_bytes());
    // call GetStdHandle (rel32).
    bytes.push(0xe8);
    let get_std_handle_call_offset = bytes.len();
    bytes.extend([0, 0, 0, 0]);
    // r13 = handle; r15 = 0.
    bytes.extend([0x49, 0x89, 0xc5]); // mov r13, rax
    bytes.extend([0x4d, 0x31, 0xff]); // xor r15, r15

    let loop_start = bytes.len();
    bytes.extend([0x4c, 0x89, 0xe9]); // mov rcx, r13 (handle)
    bytes.extend([0x4b, 0x8d, 0x14, 0x3e]); // lea rdx, [r14+r15]
    bytes.extend([0x41, 0xb8, 0x01, 0x00, 0x00, 0x00]); // mov r8d, 1
    bytes.extend([0x4c, 0x8d, 0x4c, 0x24, file_layout.transferred_disp]);
    bytes.extend([
        0x48,
        0xc7,
        0x44,
        0x24,
        file_layout.overlapped_disp,
        0,
        0,
        0,
        0,
    ]);
    bytes.push(0xe8); // call ReadFile (rel32)
    let read_file_call_offset = bytes.len();
    bytes.extend([0, 0, 0, 0]);
    bytes.extend([0x8b, 0x44, 0x24, file_layout.transferred_disp]);

    // Forward jumps to `done`, patched after `done` is known.
    let mut done_fixups: Vec<usize> = Vec::new();
    let mut jcc_done = |bytes: &mut Vec<u8>, opcode: u8| {
        bytes.push(0x0f);
        bytes.push(opcode);
        done_fixups.push(bytes.len());
        bytes.extend([0, 0, 0, 0]);
    };
    bytes.extend([0x85, 0xc0]); // test eax, eax
    jcc_done(&mut bytes, 0x84); // je done (EOF)
    bytes.extend([0x43, 0x8a, 0x04, 0x3e]); // mov al, [r14+r15] (byte read)
    // A '\n'/'\r' delimiter terminates the line only once content is present
    // (r15 > 0); a LEADING one is skipped (loop back without accepting it). This
    // makes CRLF a single terminator -- the '\n' trailing a '\r'-ended line, and
    // a bare Enter, no longer surface as a phantom empty line to the next
    // read_line. Per delimiter: cmp al,d; jne over; test r15,r15; jnz done;
    // jmp loop_start; over:
    for delim in [0x0au8, 0x0du8] {
        bytes.extend([0x3c, delim]); // cmp al, delim
        bytes.push(0x75); // jne over (skip the eol-handling block)
        let jne_over = bytes.len();
        bytes.push(0x00);
        bytes.extend([0x4d, 0x85, 0xff]); // test r15, r15
        jcc_done(&mut bytes, 0x85); // jnz done (content present -> finish line)
        bytes.push(0xe9); // jmp loop_start (leading delimiter: skip, read next)
        let jmp_loop = bytes.len();
        bytes.extend([0, 0, 0, 0]);
        let rel = loop_start as isize - (jmp_loop as isize + 4);
        bytes[jmp_loop..jmp_loop + 4].copy_from_slice(&(rel as i32).to_le_bytes());
        let over = bytes.len();
        bytes[jne_over] = (over - (jne_over + 1)) as u8;
    }
    bytes.extend([0x3c, 0x00]); // cmp al, 0
    jcc_done(&mut bytes, 0x84); // a NUL always terminates (EOF sentinel)
    bytes.extend([0x49, 0xff, 0xc7]); // inc r15 (accept the byte)
    // cmp r15, capacity ; jb loop  (keep reading while length < capacity, else
    // fall through to done so we never overrun the buffer).
    bytes.extend([0x49, 0x81, 0xff]); // cmp r15, imm32
    bytes.extend(capacity.to_le_bytes());
    bytes.extend([0x0f, 0x82]); // jb rel32
    let loop_jmp_disp = bytes.len();
    bytes.extend([0, 0, 0, 0]);
    {
        let rel = loop_start as isize - (loop_jmp_disp as isize + 4);
        bytes[loop_jmp_disp..loop_jmp_disp + 4].copy_from_slice(&(rel as i32).to_le_bytes());
    }

    // done:
    let done = bytes.len();
    for fixup in done_fixups {
        let rel = done as isize - (fixup as isize + 4);
        bytes[fixup..fixup + 4].copy_from_slice(&(rel as i32).to_le_bytes());
    }
    append_add_rsp(&mut bytes, file_layout.reserve);

    let target_mov_offset = if is_bounded_buffer {
        // Owned carrier: the bytes are already in place (r14 read straight into the
        // inline storage). Write only the length at `[r14 - 8]` (= region +
        // target_offset, the leading len word). No `{ptr, len}` descriptor, hence
        // no second relocation.
        bytes.extend([0x4d, 0x89, 0x7e, 0xf8]); // mov [r14-8], r15
        0
    } else {
        // r13 = target descriptor base (imm64 relocated). The relocation planner
        // anchors at the instruction start and adds the +2 immediate offset itself,
        // so record the start.
        let target_mov_offset = bytes.len();
        bytes.extend([0x49, 0xbd]);
        bytes.extend(0u64.to_le_bytes());
        // mov [r13+target_offset], r14  (descriptor.ptr = buffer).
        bytes.extend([0x4d, 0x89, 0xb5]);
        bytes.extend(target_ptr_disp.to_le_bytes());
        // mov [r13+target_offset+8], r15 (descriptor.len = line length).
        bytes.extend([0x4d, 0x89, 0xbd]);
        bytes.extend(target_len_disp.to_le_bytes());
        target_mov_offset
    };

    let width = bytes.len();
    Ok((
        bytes,
        RuntimeTextLineReadLayout {
            get_std_handle_call_offset,
            read_file_call_offset,
            target_imm_offset: target_mov_offset,
            width,
        },
    ))
}

fn runtime_text_line_read_layout_for(is_bounded_buffer: bool) -> RuntimeTextLineReadLayout {
    // Capacity/target do not affect the layout (all immediates are fixed width),
    // so encode once with placeholders to recover the authoritative offsets.
    build_runtime_text_line_read(0, 1, is_bounded_buffer)
        .expect("runtime text line read layout encodes")
        .1
}

fn runtime_text_line_read_layout() -> RuntimeTextLineReadLayout {
    runtime_text_line_read_layout_for(false)
}

pub fn runtime_text_line_read_width(_byte_capacity: usize) -> usize {
    runtime_text_line_read_layout().width
}

pub fn runtime_text_line_read_get_std_handle_call_offset() -> usize {
    runtime_text_line_read_layout().get_std_handle_call_offset
}

pub fn runtime_text_line_read_read_file_call_offset() -> usize {
    runtime_text_line_read_layout().read_file_call_offset
}

pub fn runtime_text_line_read_target_imm_offset() -> usize {
    runtime_text_line_read_layout().target_imm_offset
}

/// Owned `[u8; N]` carrier read encodes a wider prologue (the `add r14` past the
/// length word) and a shorter epilogue (a single `len` store, no `{ptr, len}`
/// descriptor), so its import-call offsets and width differ from the String path.
pub fn runtime_text_line_read_carrier_width(_byte_capacity: usize) -> usize {
    runtime_text_line_read_layout_for(true).width
}

pub fn runtime_text_line_read_carrier_get_std_handle_call_offset() -> usize {
    runtime_text_line_read_layout_for(true).get_std_handle_call_offset
}

pub fn runtime_text_line_read_carrier_read_file_call_offset() -> usize {
    runtime_text_line_read_layout_for(true).read_file_call_offset
}

/// x86_64 Linux line read via the `read(2)` syscall (no GetStdHandle/ReadFile imports).
/// Byte-at-a-time read from stdin (fd 0) into the relocated buffer (r14), tracking the
/// line length in r15, with the same CRLF/NUL terminator handling as the win32 import
/// path, then store the {pointer, length} String descriptor into the target region.
pub fn encode_runtime_text_line_read_syscall(
    target_offset: usize,
    byte_capacity: usize,
    number: u32,
    parameter_registers: &[omega_calling_conventions::MachineRegister],
    result_register: omega_calling_conventions::MachineRegister,
    number_register: omega_calling_conventions::MachineRegister,
    supervisor_call: u16,
) -> Result<Vec<u8>, Diagnostic> {
    validate_composite_linux_syscall_plan(
        parameter_registers,
        result_register,
        number_register,
        supervisor_call,
    )?;
    let capacity = u32::try_from(byte_capacity).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot encode line-read capacity `{byte_capacity}` yet"
        ))
    })?;
    Ok(build_runtime_text_line_read_syscall(target_offset, capacity, number, false)?.0)
}

/// Linux `read(2)` line read into an owned `[u8; N]` carrier: stdin bytes land in
/// the carrier's inline storage and the line length is written to its leading
/// length word; no `{ptr, len}` descriptor.
pub fn encode_runtime_text_line_read_syscall_carrier(
    target_offset: usize,
    byte_capacity: usize,
    number: u32,
    parameter_registers: &[omega_calling_conventions::MachineRegister],
    result_register: omega_calling_conventions::MachineRegister,
    number_register: omega_calling_conventions::MachineRegister,
    supervisor_call: u16,
) -> Result<Vec<u8>, Diagnostic> {
    validate_composite_linux_syscall_plan(
        parameter_registers,
        result_register,
        number_register,
        supervisor_call,
    )?;
    let capacity = u32::try_from(byte_capacity).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot encode line-read capacity `{byte_capacity}` yet"
        ))
    })?;
    Ok(build_runtime_text_line_read_syscall(target_offset, capacity, number, true)?.0)
}

fn build_runtime_text_line_read_syscall(
    target_offset: usize,
    capacity: u32,
    number: u32,
    is_bounded_buffer: bool,
) -> Result<(Vec<u8>, usize), Diagnostic> {
    let target_ptr_disp = disp32(target_offset)?;
    let target_len_disp = disp32(target_offset + 8)?;
    let carrier_bytes_disp = disp32(target_offset + 8)?;
    let mut bytes = Vec::with_capacity(128);

    // r14 = read buffer (imm64 at +2 relocated to the buffer data symbol, OR to the
    // carrier's own region for an owned `[u8; N]` target); r15 = length.
    bytes.extend([0x49, 0xbe]);
    bytes.extend(0u64.to_le_bytes());
    if is_bounded_buffer {
        // add r14, target_offset + pointer_size -> r14 = carrier inline bytes.
        bytes.extend([0x49, 0x81, 0xc6]);
        bytes.extend(carrier_bytes_disp.to_le_bytes());
    }
    bytes.extend([0x4d, 0x31, 0xff]); // xor r15, r15

    let loop_start = bytes.len();
    bytes.extend([0x31, 0xff]); // xor edi, edi (fd = 0, stdin)
    bytes.extend([0x4b, 0x8d, 0x34, 0x3e]); // lea rsi, [r14+r15] (buffer + length)
    bytes.extend([0xba, 0x01, 0x00, 0x00, 0x00]); // mov edx, 1 (read one byte)
    bytes.push(0xb8); // mov eax, read-syscall-number
    bytes.extend(number.to_le_bytes());
    bytes.extend([0x0f, 0x05]); // syscall
    bytes.extend([0x48, 0x85, 0xc0]); // test rax, rax (rax = bytes read / -errno)

    // Forward jumps to `done`, patched after `done` is known.
    let mut done_fixups: Vec<usize> = Vec::new();
    let mut jcc_done = |bytes: &mut Vec<u8>, opcode: u8| {
        bytes.push(0x0f);
        bytes.push(opcode);
        done_fixups.push(bytes.len());
        bytes.extend([0, 0, 0, 0]);
    };
    jcc_done(&mut bytes, 0x8e); // jle done (read returned 0 (EOF) or < 0 (error))
    bytes.extend([0x43, 0x8a, 0x04, 0x3e]); // mov al, [r14+r15] (byte read)
    // A '\n'/'\r' delimiter terminates the line only once content is present
    // (r15 > 0); a LEADING one is skipped (loop back without accepting it), so CRLF
    // is a single terminator. Mirrors the win32 import path's terminator handling.
    for delim in [0x0au8, 0x0du8] {
        bytes.extend([0x3c, delim]); // cmp al, delim
        bytes.push(0x75); // jne over
        let jne_over = bytes.len();
        bytes.push(0x00);
        bytes.extend([0x4d, 0x85, 0xff]); // test r15, r15
        jcc_done(&mut bytes, 0x85); // jnz done (content present -> finish line)
        bytes.push(0xe9); // jmp loop_start (leading delimiter: skip, read next)
        let jmp_loop = bytes.len();
        bytes.extend([0, 0, 0, 0]);
        let rel = loop_start as isize - (jmp_loop as isize + 4);
        bytes[jmp_loop..jmp_loop + 4].copy_from_slice(&(rel as i32).to_le_bytes());
        let over = bytes.len();
        bytes[jne_over] = (over - (jne_over + 1)) as u8;
    }
    bytes.extend([0x3c, 0x00]); // cmp al, 0
    jcc_done(&mut bytes, 0x84); // a NUL always terminates (EOF sentinel)
    bytes.extend([0x49, 0xff, 0xc7]); // inc r15 (accept the byte)
    bytes.extend([0x49, 0x81, 0xff]); // cmp r15, imm32
    bytes.extend(capacity.to_le_bytes());
    bytes.extend([0x0f, 0x82]); // jb rel32 -> loop_start (keep reading while < capacity)
    let loop_jmp_disp = bytes.len();
    bytes.extend([0, 0, 0, 0]);
    {
        let rel = loop_start as isize - (loop_jmp_disp as isize + 4);
        bytes[loop_jmp_disp..loop_jmp_disp + 4].copy_from_slice(&(rel as i32).to_le_bytes());
    }

    // done:
    let done = bytes.len();
    for fixup in done_fixups {
        let rel = done as isize - (fixup as isize + 4);
        bytes[fixup..fixup + 4].copy_from_slice(&(rel as i32).to_le_bytes());
    }
    let target_mov_offset = if is_bounded_buffer {
        // Owned carrier: the bytes are already in place; write only the length at
        // `[r14 - 8]` (the leading len word). No `{ptr, len}` descriptor.
        bytes.extend([0x4d, 0x89, 0x7e, 0xf8]); // mov [r14-8], r15
        0
    } else {
        // mov r13, imm64(target) (relocated at +2); store the descriptor.
        let target_mov_offset = bytes.len();
        bytes.extend([0x49, 0xbd]);
        bytes.extend(0u64.to_le_bytes());
        bytes.extend([0x4d, 0x89, 0xb5]); // mov [r13+target_offset], r14 (descriptor.ptr)
        bytes.extend(target_ptr_disp.to_le_bytes());
        bytes.extend([0x4d, 0x89, 0xbd]); // mov [r13+target_offset+8], r15 (descriptor.len)
        bytes.extend(target_len_disp.to_le_bytes());
        target_mov_offset
    };

    Ok((bytes, target_mov_offset))
}

fn runtime_text_line_read_syscall_layout_for(is_bounded_buffer: bool) -> (usize, usize) {
    // Capacity/number/target are all fixed-width immediates, so they do not affect the
    // layout; encode once with placeholders to recover the width + target imm offset.
    let (bytes, target_mov_offset) =
        build_runtime_text_line_read_syscall(0, 1, 0, is_bounded_buffer)
            .expect("runtime text line read syscall layout encodes");
    (bytes.len(), target_mov_offset)
}

fn runtime_text_line_read_syscall_layout() -> (usize, usize) {
    runtime_text_line_read_syscall_layout_for(false)
}

pub fn runtime_text_line_read_syscall_width() -> usize {
    runtime_text_line_read_syscall_layout().0
}

pub fn runtime_text_line_read_syscall_target_imm_offset() -> usize {
    runtime_text_line_read_syscall_layout().1
}

/// Owned carrier syscall read: wider prologue (`add r14`), shorter epilogue (a
/// single `len` store), so its width differs from the String descriptor path.
pub fn runtime_text_line_read_syscall_carrier_width() -> usize {
    runtime_text_line_read_syscall_layout_for(true).0
}

pub fn encode_runtime_text_line_read(
    target_offset: usize,
    byte_capacity: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let capacity = u32::try_from(byte_capacity).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot encode line-read capacity `{byte_capacity}` yet"
        ))
    })?;
    Ok(build_runtime_text_line_read(target_offset, capacity, false)?.0)
}

/// Read a stdin line into an owned `[u8; N]` carrier: stdin bytes land directly in
/// the carrier's inline storage (`region + target_offset + pointer_size`) and the
/// line length is written to the carrier's leading length word (`target_offset`).
pub fn encode_runtime_text_line_read_carrier(
    target_offset: usize,
    byte_capacity: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let capacity = u32::try_from(byte_capacity).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot encode line-read capacity `{byte_capacity}` yet"
        ))
    })?;
    Ok(build_runtime_text_line_read(target_offset, capacity, true)?.0)
}

// ---- compact_binary v0 wire-encode appends (chapter 20, decision 10) ----
//
// Both operations share one cursor convention: the caller's `written` slot
// holds the running byte count, so every append loads it, stores through a
// moving pointer (`out base + out offset + cursor`), and writes the advanced
// cursor back. Register use: r15 = moving out pointer, r14 = written page,
// r10 = cursor, rax = runtime scalar, r11 = byte/zigzag scratch; the text
// append also uses r9 = source ptr and rcx = remaining copy count (r12 is the
// dispatch-state register and stays untouched).
//
// THE WIDTHS INVARIANT: every emitted byte must move the `_width` functions
// and the `wire_append_*_offset` relocation offsets below in exact lockstep,
// or relocations drift and the binary segfaults.

/// Shared prologue: `mov r15, imm64(out)` (10, relocated at the instruction
/// start) + `add r15, imm32(out_offset)` (7) + `mov r14, imm64(written)` (10,
/// relocated at +17) + `mov r10, [r14+written_offset]` (7) + `add r15, r10`
/// (3).
fn wire_append_prologue_width() -> usize {
    37
}

fn append_wire_append_prologue(
    bytes: &mut Vec<u8>,
    out_offset: usize,
    written_offset: usize,
) -> Result<(), Diagnostic> {
    append_mov_r15_imm64(bytes, 0);
    append_add_r15_imm32(bytes, out_offset)?;
    append_mov_r14_imm64(bytes, 0);
    append_load_r10_from_r14(bytes, written_offset)?;
    bytes.extend([0x4d, 0x01, 0xd7]); // add r15, r10
    Ok(())
}

pub fn append_wire_literal_byte_width(_out_offset: usize, _written_offset: usize) -> usize {
    // Prologue + `mov byte [r15], imm8` (4) + `inc r10` (3) + cursor store (7).
    wire_append_prologue_width() + 4 + 3 + 7
}

/// One compile-time framing byte (era/tag varint bytes): store it at the
/// cursor and advance by one.
pub fn encode_append_wire_literal_byte(
    out_offset: usize,
    written_offset: usize,
    value: u8,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(append_wire_literal_byte_width(out_offset, written_offset));
    append_wire_append_prologue(&mut bytes, out_offset, written_offset)?;
    bytes.extend([0x41, 0xc6, 0x07, value]); // mov byte [r15], imm8
    bytes.extend([0x49, 0xff, 0xc2]); // inc r10
    append_store_r10_to_r14(&mut bytes, written_offset, 8)?;
    debug_assert_eq!(
        bytes.len(),
        append_wire_literal_byte_width(out_offset, written_offset)
    );
    Ok(bytes)
}

/// The sized scalar load from `[r11 + source_offset]` into rax: 64-bit and
/// 32-bit moves are 7 bytes, the zero-extending byte load (movzx) is 8, and a
/// 4-byte SIGNED source loads sign-extending (movsxd, 7).
fn wire_varint_source_load_width(byte_size: usize) -> usize {
    if byte_size == 1 { 8 } else { 7 }
}

/// `mov r11, rax` + `sar r11, 63` + `shl rax, 1` + `xor rax, r11`.
fn wire_zigzag_width() -> usize {
    14
}

/// The fixed LEB128 emit loop + final-byte tail (see the encoder body).
fn wire_varint_emit_loop_width() -> usize {
    40
}

pub fn append_wire_scalar_varint_width(
    _source_offset: usize,
    byte_size: usize,
    zigzag: bool,
    _out_offset: usize,
    _written_offset: usize,
) -> usize {
    wire_append_prologue_width()
        + 10
        + wire_varint_source_load_width(byte_size)
        + if zigzag { wire_zigzag_width() } else { 0 }
        + wire_varint_emit_loop_width()
        + 7
}

/// LEB128-encode a runtime scalar at the cursor. The value loads zero-extended
/// at its source width; signed sources (`zigzag`) sign-extend to 64 bits and
/// zigzag (`(n << 1) ^ (n >> 63)`) before the emit loop.
pub fn encode_append_wire_scalar_varint(
    source_region: omega_target_operations::RuntimeStorageRegion,
    source_offset: usize,
    byte_size: usize,
    zigzag: bool,
    out_offset: usize,
    written_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "X86_64 wire encoder cannot varint-encode {byte_size}-byte scalars yet"
        )));
    }
    // The region only picks the relocation symbol; the encoded shape is
    // identical for machine and frame sources.
    let _ = source_region;

    let mut bytes = Vec::with_capacity(append_wire_scalar_varint_width(
        source_offset,
        byte_size,
        zigzag,
        out_offset,
        written_offset,
    ));
    append_wire_append_prologue(&mut bytes, out_offset, written_offset)?;

    // r11 = source base (imm64 relocated at +37), rax = the scalar.
    append_mov_reg_imm64(&mut bytes, Reg64::R11, 0);
    let displacement = disp32(source_offset)?;
    match (byte_size, zigzag) {
        (8, _) => bytes.extend([0x49, 0x8b, 0x83]), // mov rax, [r11+disp32]
        (4, false) => bytes.extend([0x41, 0x8b, 0x83]), // mov eax, [r11+disp32]
        (4, true) => bytes.extend([0x49, 0x63, 0x83]), // movsxd rax, dword [r11+disp32]
        (1, _) => bytes.extend([0x41, 0x0f, 0xb6, 0x83]), // movzx eax, byte [r11+disp32]
        _ => unreachable!("byte_size validated above"),
    }
    bytes.extend(displacement.to_le_bytes());

    if zigzag {
        // zigzag(n) = (n << 1) ^ (n >> 63); r11 holds the sign mask.
        bytes.extend([0x49, 0x89, 0xc3]); // mov r11, rax
        bytes.extend([0x49, 0xc1, 0xfb, 0x3f]); // sar r11, 63
        bytes.extend([0x48, 0xc1, 0xe0, 0x01]); // shl rax, 1
        bytes.extend([0x4c, 0x31, 0xd8]); // xor rax, r11
    }

    // LEB128 emit loop (fixed 40 bytes, `wire_varint_emit_loop_width`):
    //   loop: mov  r11, rax
    //         and  r11, 0x7f
    //         shr  rax, 7
    //         test rax, rax
    //         je   last            (+18: skip or/store/inc/inc/jmp)
    //         or   r11, 0x80
    //         mov  [r15], r11b
    //         inc  r15
    //         inc  r10
    //         jmp  loop            (-34)
    //   last: mov  [r15], r11b
    //         inc  r10
    bytes.extend([0x49, 0x89, 0xc3]); // mov r11, rax
    bytes.extend([0x49, 0x83, 0xe3, 0x7f]); // and r11, 0x7f
    bytes.extend([0x48, 0xc1, 0xe8, 0x07]); // shr rax, 7
    bytes.extend([0x48, 0x85, 0xc0]); // test rax, rax
    bytes.extend([0x74, 0x12]); // je +18 -> last
    bytes.extend([0x49, 0x81, 0xcb, 0x80, 0x00, 0x00, 0x00]); // or r11, 0x80
    bytes.extend([0x45, 0x88, 0x1f]); // mov [r15], r11b
    bytes.extend([0x49, 0xff, 0xc7]); // inc r15
    bytes.extend([0x49, 0xff, 0xc2]); // inc r10
    bytes.extend([0xeb, 0xde]); // jmp -34 -> loop
    bytes.extend([0x45, 0x88, 0x1f]); // mov [r15], r11b
    bytes.extend([0x49, 0xff, 0xc2]); // inc r10

    append_store_r10_to_r14(&mut bytes, written_offset, 8)?;
    debug_assert_eq!(
        bytes.len(),
        append_wire_scalar_varint_width(
            source_offset,
            byte_size,
            zigzag,
            out_offset,
            written_offset
        )
    );
    Ok(bytes)
}

/// The fixed bounds-checked byte-copy loop in `encode_append_wire_text_bytes`.
fn wire_text_copy_loop_width() -> usize {
    35
}

/// The compile-time out-buffer capacity as a `cmp r10, imm32` operand.
fn wire_encode_capacity_imm32(out_length: usize) -> Result<i32, Diagnostic> {
    i32::try_from(out_length).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 wire encoder cannot bounds-check a {out_length}-byte buffer yet"
        ))
    })
}

pub fn append_wire_text_bytes_width(
    _source_offset: usize,
    _out_offset: usize,
    _out_length: usize,
    _written_offset: usize,
) -> usize {
    // Prologue + source imm64 (10) + ptr load (7) + len load (7) + count copy
    // (3) + length-varint emit loop + dest-pointer re-sync inc (3) + bounded
    // copy loop + cursor store (7).
    wire_append_prologue_width()
        + 10
        + 7
        + 7
        + 3
        + wire_varint_emit_loop_width()
        + 3
        + wire_text_copy_loop_width()
        + 7
}

/// Append a runtime `String` field: the source place holds a `{ptr @ +0,
/// len @ +8}` text descriptor; emit len as an unsigned LEB128 varint, then
/// copy len raw bytes from ptr. The length varint reuses the scalar emit loop
/// (validation's worst-case budget covers its ten bytes -- String fields
/// encode LAST). The byte-copy is the one append whose size is
/// runtime-unbounded, so every copy store is bounds-checked against
/// `out_length` and content past capacity is DROPPED: the cursor stops at
/// `out_length`, never past it.
pub fn encode_append_wire_text_bytes(
    source_region: omega_target_operations::RuntimeStorageRegion,
    source_offset: usize,
    out_offset: usize,
    out_length: usize,
    written_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    // The region only picks the relocation symbol; the encoded shape is
    // identical for machine and frame sources.
    let _ = source_region;

    let mut bytes = Vec::with_capacity(append_wire_text_bytes_width(
        source_offset,
        out_offset,
        out_length,
        written_offset,
    ));
    append_wire_append_prologue(&mut bytes, out_offset, written_offset)?;

    // r11 = source base (imm64 relocated at +37), r9 = ptr, rax = len.
    append_mov_reg_imm64(&mut bytes, Reg64::R11, 0);
    bytes.extend([0x4d, 0x8b, 0x8b]); // mov r9, [r11+disp32]
    bytes.extend(disp32(source_offset)?.to_le_bytes());
    bytes.extend([0x49, 0x8b, 0x83]); // mov rax, [r11+disp32]
    bytes.extend(disp32(source_offset + 8)?.to_le_bytes());
    // rcx keeps the byte count for the copy loop; the emit loop consumes rax.
    bytes.extend([0x48, 0x89, 0xc1]); // mov rcx, rax

    // The same fixed 40-byte LEB128 emit loop as the scalar varint (see
    // `encode_append_wire_scalar_varint`), here emitting the LENGTH.
    bytes.extend([0x49, 0x89, 0xc3]); // mov r11, rax
    bytes.extend([0x49, 0x83, 0xe3, 0x7f]); // and r11, 0x7f
    bytes.extend([0x48, 0xc1, 0xe8, 0x07]); // shr rax, 7
    bytes.extend([0x48, 0x85, 0xc0]); // test rax, rax
    bytes.extend([0x74, 0x12]); // je +18 -> last
    bytes.extend([0x49, 0x81, 0xcb, 0x80, 0x00, 0x00, 0x00]); // or r11, 0x80
    bytes.extend([0x45, 0x88, 0x1f]); // mov [r15], r11b
    bytes.extend([0x49, 0xff, 0xc7]); // inc r15
    bytes.extend([0x49, 0xff, 0xc2]); // inc r10
    bytes.extend([0xeb, 0xde]); // jmp -34 -> loop
    bytes.extend([0x45, 0x88, 0x1f]); // mov [r15], r11b
    bytes.extend([0x49, 0xff, 0xc2]); // inc r10
    // The emit loop's final store does not advance the dest pointer (the
    // scalar append ends there); re-sync r15 with the cursor for the copy.
    bytes.extend([0x49, 0xff, 0xc7]); // inc r15

    // Bounded byte-copy loop (fixed 35 bytes, `wire_text_copy_loop_width`):
    //   copy: test rcx, rcx
    //         je   done            (+30: all bytes copied)
    //         cmp  r10, imm32(N)
    //         jae  done            (+21: capacity full -- drop the rest)
    //         movzx r11d, byte [r9]
    //         inc  r9
    //         mov  [r15], r11b
    //         inc  r15
    //         inc  r10
    //         dec  rcx
    //         jmp  copy            (-35)
    //   done:
    bytes.extend([0x48, 0x85, 0xc9]); // test rcx, rcx
    bytes.extend([0x74, 0x1e]); // je +30 -> done
    bytes.extend([0x49, 0x81, 0xfa]); // cmp r10, imm32
    bytes.extend(wire_encode_capacity_imm32(out_length)?.to_le_bytes());
    bytes.extend([0x73, 0x15]); // jae +21 -> done
    bytes.extend([0x45, 0x0f, 0xb6, 0x19]); // movzx r11d, byte [r9]
    bytes.extend([0x49, 0xff, 0xc1]); // inc r9
    bytes.extend([0x45, 0x88, 0x1f]); // mov [r15], r11b
    bytes.extend([0x49, 0xff, 0xc7]); // inc r15
    bytes.extend([0x49, 0xff, 0xc2]); // inc r10
    bytes.extend([0x48, 0xff, 0xc9]); // dec rcx
    bytes.extend([0xeb, 0xdd]); // jmp -35 -> copy

    append_store_r10_to_r14(&mut bytes, written_offset, 8)?;
    debug_assert_eq!(
        bytes.len(),
        append_wire_text_bytes_width(source_offset, out_offset, out_length, written_offset)
    );
    Ok(bytes)
}

/// Byte offset of the WRITTEN page mov inside both wire appends (the
/// relocation planner adds the +2 imm64 offset itself).
pub fn wire_append_written_page_offset(_out_offset: usize) -> usize {
    17
}

/// Byte offset of the SOURCE page mov inside the varint append AND the
/// text-bytes append (both materialize the source page right after the shared
/// prologue).
pub fn wire_append_varint_source_page_offset(_out_offset: usize, _written_offset: usize) -> usize {
    37
}

// ---- compact_binary v0 wire-decode reads (chapter 20, wire stage 2b) ----
//
// Both operations share the encoder's cursor convention: the caller's `read`
// slot holds the running byte count, so every read loads it, reads through a
// moving pointer (`buffer base + buffer offset + cursor`), and writes the
// advanced cursor back. The success flag in the caller's `ok` slot is STICKY:
// each operation ANDs its own success bit into the slot and never sets it, so
// the first failure makes the whole decode report failure while later
// operations keep executing (every byte read stays bounds-checked against the
// buffer's compile-time length, so a failed decode never reads out of
// bounds). Register use: r15 = moving buffer pointer, r14 = read page,
// r13 = ok page, r10 = cursor, rax = value, rcx = shift, r11 = byte scratch,
// r9 = this-op success, r8 = 7-bit chunk / target page (r12 is the
// dispatch-state register and stays untouched).
//
// THE WIDTHS INVARIANT: every emitted byte must move the `_width` functions
// and the `wire_decode_*_offset` relocation offsets below in exact lockstep,
// or relocations drift and the binary segfaults.

/// Shared prologue: `mov r15, imm64(buffer)` (10, relocated at the
/// instruction start) + `add r15, imm32(buffer_offset)` (7) +
/// `mov r14, imm64(read)` (10, relocated at +17) +
/// `mov r10, [r14+read_offset]` (7) + `add r15, r10` (3) +
/// `mov r13, imm64(ok)` (10, relocated at +37).
fn wire_decode_prologue_width() -> usize {
    47
}

fn append_wire_decode_prologue(
    bytes: &mut Vec<u8>,
    buffer_offset: usize,
    read_offset: usize,
) -> Result<(), Diagnostic> {
    append_mov_r15_imm64(bytes, 0);
    append_add_r15_imm32(bytes, buffer_offset)?;
    append_mov_r14_imm64(bytes, 0);
    append_load_r10_from_r14(bytes, read_offset)?;
    bytes.extend([0x4d, 0x01, 0xd7]); // add r15, r10
    bytes.extend([0x49, 0xbd]); // mov r13, imm64(ok page)
    bytes.extend(0u64.to_le_bytes());
    Ok(())
}

/// Shared epilogue: AND this operation's success bit (r9) into the sticky ok
/// slot, then store the advanced cursor back to the read slot.
/// `movzx r11d, byte [r13+ok]` (8) + `and r11, r9` (3) +
/// `mov [r13+ok], r11b` (7) + cursor store (7).
fn wire_decode_tail_width() -> usize {
    25
}

fn append_wire_decode_epilogue(
    bytes: &mut Vec<u8>,
    read_offset: usize,
    ok_offset: usize,
) -> Result<(), Diagnostic> {
    let ok_displacement = disp32(ok_offset)?;
    bytes.extend([0x45, 0x0f, 0xb6, 0x9d]); // movzx r11d, byte [r13+disp32]
    bytes.extend(ok_displacement.to_le_bytes());
    bytes.extend([0x4d, 0x21, 0xcb]); // and r11, r9
    bytes.extend([0x45, 0x88, 0x9d]); // mov [r13+disp32], r11b
    bytes.extend(ok_displacement.to_le_bytes());
    append_store_r10_to_r14(bytes, read_offset, 8)
}

/// The compile-time buffer length as a `cmp r10, imm32` operand.
fn wire_decode_length_imm32(buffer_length: usize) -> Result<i32, Diagnostic> {
    i32::try_from(buffer_length).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 wire decoder cannot bounds-check a {buffer_length}-byte buffer yet"
        ))
    })
}

pub fn read_wire_expected_byte_width(
    _buffer_offset: usize,
    _buffer_length: usize,
    _read_offset: usize,
    _ok_offset: usize,
) -> usize {
    // Prologue + the fixed check block (success-bit mov + bounds cmp/jae +
    // byte load + cursor inc + expected cmp/je + fail xor) + epilogue.
    wire_decode_prologue_width() + 34 + wire_decode_tail_width()
}

/// Expect one compile-time framing byte (era/tag varint bytes) at the cursor:
/// out of bounds clears ok without consuming; a mismatch consumes the byte
/// and clears ok; a match consumes the byte.
pub fn encode_read_wire_expected_byte(
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
    expected: u8,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(read_wire_expected_byte_width(
        buffer_offset,
        buffer_length,
        read_offset,
        ok_offset,
    ));
    append_wire_decode_prologue(&mut bytes, buffer_offset, read_offset)?;

    // Fixed 34-byte check block:
    //         mov  r9d, 1
    //         cmp  r10, imm32(length)
    //         jae  fail            (+16: skip movzx/inc/cmp/je)
    //         movzx r11d, byte [r15]
    //         inc  r10
    //         cmp  r11, imm32(expected)
    //         je   done            (+3: skip the fail xor)
    //   fail: xor  r9d, r9d
    //   done:
    bytes.extend([0x41, 0xb9, 0x01, 0x00, 0x00, 0x00]); // mov r9d, 1
    bytes.extend([0x49, 0x81, 0xfa]); // cmp r10, imm32
    bytes.extend(wire_decode_length_imm32(buffer_length)?.to_le_bytes());
    bytes.extend([0x73, 0x10]); // jae +16 -> fail
    bytes.extend([0x45, 0x0f, 0xb6, 0x1f]); // movzx r11d, byte [r15]
    bytes.extend([0x49, 0xff, 0xc2]); // inc r10
    bytes.extend([0x49, 0x81, 0xfb]); // cmp r11, imm32
    bytes.extend(i32::from(expected).to_le_bytes());
    bytes.extend([0x74, 0x03]); // je +3 -> done
    bytes.extend([0x45, 0x31, 0xc9]); // xor r9d, r9d

    append_wire_decode_epilogue(&mut bytes, read_offset, ok_offset)?;
    debug_assert_eq!(
        bytes.len(),
        read_wire_expected_byte_width(buffer_offset, buffer_length, read_offset, ok_offset)
    );
    Ok(bytes)
}

/// The fixed LEB128 read loop + fail tail (see the decoder body).
fn wire_varint_read_loop_width() -> usize {
    56
}

/// `mov r11, rax` + `and r11, 1` + `neg r11` + `shr rax, 1` + `xor rax, r11`.
fn wire_unzigzag_width() -> usize {
    16
}

pub fn read_wire_scalar_varint_width(
    _buffer_offset: usize,
    _buffer_length: usize,
    _read_offset: usize,
    _ok_offset: usize,
    _target_offset: usize,
    _byte_size: usize,
    zigzag: bool,
) -> usize {
    // Prologue + success/value/shift init (10) + read loop + optional
    // unzigzag + target imm64 (10) + truncating store (7) + epilogue.
    wire_decode_prologue_width()
        + 10
        + wire_varint_read_loop_width()
        + if zigzag { wire_unzigzag_width() } else { 0 }
        + 10
        + 7
        + wire_decode_tail_width()
}

/// LEB128-read a runtime scalar at the cursor into the target place. The
/// loop's iteration count is data dependent but its EMITTED width is constant
/// (the widths invariant): truncation and overlong varints (a continuation
/// past shift 63) branch to the fail arm. Signed targets un-zigzag
/// (`(n >> 1) ^ -(n & 1)`) before the store; the store truncates to the field
/// width.
#[allow(clippy::too_many_arguments)]
pub fn encode_read_wire_scalar_varint(
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
    target_region: omega_target_operations::RuntimeStorageRegion,
    target_offset: usize,
    byte_size: usize,
    zigzag: bool,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "X86_64 wire decoder cannot varint-decode {byte_size}-byte scalars yet"
        )));
    }
    // The region only picks the relocation symbol; the encoded shape is
    // identical for machine and frame targets.
    let _ = target_region;

    let mut bytes = Vec::with_capacity(read_wire_scalar_varint_width(
        buffer_offset,
        buffer_length,
        read_offset,
        ok_offset,
        target_offset,
        byte_size,
        zigzag,
    ));
    append_wire_decode_prologue(&mut bytes, buffer_offset, read_offset)?;

    bytes.extend([0x41, 0xb9, 0x01, 0x00, 0x00, 0x00]); // mov r9d, 1
    bytes.extend([0x31, 0xc0]); // xor eax, eax (value)
    bytes.extend([0x31, 0xc9]); // xor ecx, ecx (shift)

    // LEB128 read loop (fixed 56 bytes, `wire_varint_read_loop_width`):
    //   loop: cmp  rcx, 63
    //         ja   fail            (+47: overlong varint, >10 groups)
    //         cmp  r10, imm32(length)
    //         jae  fail            (+38: truncated input)
    //         movzx r11d, byte [r15]
    //         inc  r15
    //         inc  r10
    //         mov  r8, r11
    //         and  r8, 0x7f
    //         shl  r8, cl
    //         or   rax, r8
    //         add  rcx, 7
    //         test r11, 0x80       (continuation bit)
    //         jnz  loop            (-51)
    //         jmp  done            (+3: skip the fail xor)
    //   fail: xor  r9d, r9d
    //   done:
    bytes.extend([0x48, 0x83, 0xf9, 0x3f]); // cmp rcx, 63
    bytes.extend([0x77, 0x2f]); // ja +47 -> fail
    bytes.extend([0x49, 0x81, 0xfa]); // cmp r10, imm32
    bytes.extend(wire_decode_length_imm32(buffer_length)?.to_le_bytes());
    bytes.extend([0x73, 0x26]); // jae +38 -> fail
    bytes.extend([0x45, 0x0f, 0xb6, 0x1f]); // movzx r11d, byte [r15]
    bytes.extend([0x49, 0xff, 0xc7]); // inc r15
    bytes.extend([0x49, 0xff, 0xc2]); // inc r10
    bytes.extend([0x4d, 0x89, 0xd8]); // mov r8, r11
    bytes.extend([0x49, 0x83, 0xe0, 0x7f]); // and r8, 0x7f
    bytes.extend([0x49, 0xd3, 0xe0]); // shl r8, cl
    bytes.extend([0x4c, 0x09, 0xc0]); // or rax, r8
    bytes.extend([0x48, 0x83, 0xc1, 0x07]); // add rcx, 7
    bytes.extend([0x49, 0xf7, 0xc3, 0x80, 0x00, 0x00, 0x00]); // test r11, 0x80
    bytes.extend([0x75, 0xcd]); // jnz -51 -> loop
    bytes.extend([0xeb, 0x03]); // jmp +3 -> done
    bytes.extend([0x45, 0x31, 0xc9]); // xor r9d, r9d

    if zigzag {
        // unzigzag(n) = (n >> 1) ^ -(n & 1); r11 holds the mask.
        bytes.extend([0x49, 0x89, 0xc3]); // mov r11, rax
        bytes.extend([0x49, 0x83, 0xe3, 0x01]); // and r11, 1
        bytes.extend([0x49, 0xf7, 0xdb]); // neg r11
        bytes.extend([0x48, 0xd1, 0xe8]); // shr rax, 1
        bytes.extend([0x4c, 0x31, 0xd8]); // xor rax, r11
    }

    // r8 = the target base (imm64 relocated at
    // `wire_decode_varint_target_page_offset`), then the truncating store.
    bytes.extend([0x49, 0xb8]); // mov r8, imm64(target page)
    bytes.extend(0u64.to_le_bytes());
    let target_displacement = disp32(target_offset)?;
    match byte_size {
        1 => bytes.extend([0x41, 0x88, 0x80]), // mov [r8+disp32], al
        4 => bytes.extend([0x41, 0x89, 0x80]), // mov [r8+disp32], eax
        8 => bytes.extend([0x49, 0x89, 0x80]), // mov [r8+disp32], rax
        _ => unreachable!("byte_size validated above"),
    }
    bytes.extend(target_displacement.to_le_bytes());

    append_wire_decode_epilogue(&mut bytes, read_offset, ok_offset)?;
    debug_assert_eq!(
        bytes.len(),
        read_wire_scalar_varint_width(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            target_offset,
            byte_size,
            zigzag
        )
    );
    Ok(bytes)
}

/// compact_binary v0 borrowed `&[u8]` decode (#43): read the byte-LENGTH varint
/// (the shared prologue + LEB128 loop leave r15 = &buffer[content start] and
/// rax = the length), bounds-check the content against the buffer, store the fat
/// `{ptr = r15, len = rax}` descriptor into the target, and advance the cursor
/// past the content. A content run past the buffer clears the sticky `ok`.
pub fn encode_read_wire_byte_slice(
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
    target_region: omega_target_operations::RuntimeStorageRegion,
    target_offset: usize,
    predicate_mask: u8,
) -> Result<Vec<u8>, Diagnostic> {
    // The region only picks the relocation symbol; the shape is identical.
    let _ = target_region;

    let mut bytes = Vec::with_capacity(read_wire_byte_slice_width(
        buffer_offset,
        buffer_length,
        read_offset,
        ok_offset,
        target_offset,
        predicate_mask,
    ));
    append_wire_decode_prologue(&mut bytes, buffer_offset, read_offset)?;

    bytes.extend([0x41, 0xb9, 0x01, 0x00, 0x00, 0x00]); // mov r9d, 1 (ok)
    bytes.extend([0x31, 0xc0]); // xor eax, eax (length)
    bytes.extend([0x31, 0xc9]); // xor ecx, ecx (shift)

    // Identical LEB128 read loop to the scalar decoder: rax = length, r15 now
    // points at the CONTENT (just past the length varint), r10 = cursor.
    bytes.extend([0x48, 0x83, 0xf9, 0x3f]); // cmp rcx, 63
    bytes.extend([0x77, 0x2f]); // ja +47 -> fail
    bytes.extend([0x49, 0x81, 0xfa]); // cmp r10, imm32
    bytes.extend(wire_decode_length_imm32(buffer_length)?.to_le_bytes());
    bytes.extend([0x73, 0x26]); // jae +38 -> fail
    bytes.extend([0x45, 0x0f, 0xb6, 0x1f]); // movzx r11d, byte [r15]
    bytes.extend([0x49, 0xff, 0xc7]); // inc r15
    bytes.extend([0x49, 0xff, 0xc2]); // inc r10
    bytes.extend([0x4d, 0x89, 0xd8]); // mov r8, r11
    bytes.extend([0x49, 0x83, 0xe0, 0x7f]); // and r8, 0x7f
    bytes.extend([0x49, 0xd3, 0xe0]); // shl r8, cl
    bytes.extend([0x4c, 0x09, 0xc0]); // or rax, r8
    bytes.extend([0x48, 0x83, 0xc1, 0x07]); // add rcx, 7
    bytes.extend([0x49, 0xf7, 0xc3, 0x80, 0x00, 0x00, 0x00]); // test r11, 0x80
    bytes.extend([0x75, 0xcd]); // jnz -51 -> loop
    bytes.extend([0xeb, 0x03]); // jmp +3 -> done
    bytes.extend([0x45, 0x31, 0xc9]); // xor r9d, r9d (fail: clear ok)

    // Bounds + advance (fixed 21 bytes): end = cursor + len; if end >
    // buffer_length clear ok; cursor = end.
    bytes.extend([0x4d, 0x89, 0xd0]); // mov r8, r10  (r8 = cursor)
    bytes.extend([0x49, 0x01, 0xc0]); // add r8, rax  (r8 = cursor + len = end)
    bytes.extend([0x49, 0x81, 0xf8]); // cmp r8, imm32
    bytes.extend(wire_decode_length_imm32(buffer_length)?.to_le_bytes());
    bytes.extend([0x76, 0x03]); // jbe +3 (skip clear when end <= length)
    bytes.extend([0x45, 0x31, 0xc9]); // xor r9d, r9d (content overruns -> clear ok)
    bytes.extend([0x4d, 0x89, 0xc2]); // mov r10, r8 (advance cursor to end)

    // Decode-boundary byte-domain validation over the just-decoded content
    // (ptr r15, len rax): every predicate in the mask checks the UNTRUSTED
    // bytes and clears the sticky ok flag (r9d) on violation -- the aarch64
    // twin's contract exactly.
    append_wire_byte_predicate_checks(&mut bytes, predicate_mask);

    // Store the descriptor: ptr = r15 (content start) @ +0, len = rax @ +8.
    bytes.extend([0x49, 0xb8]); // mov r8, imm64(target page)
    bytes.extend(0u64.to_le_bytes());
    bytes.extend([0x4d, 0x89, 0xb8]); // mov [r8+disp32], r15
    bytes.extend(disp32(target_offset)?.to_le_bytes());
    bytes.extend([0x49, 0x89, 0x80]); // mov [r8+disp32], rax
    bytes.extend(disp32(target_offset + 8)?.to_le_bytes());

    append_wire_decode_epilogue(&mut bytes, read_offset, ok_offset)?;
    debug_assert_eq!(
        bytes.len(),
        read_wire_byte_slice_width(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            target_offset,
            predicate_mask
        )
    );
    Ok(bytes)
}

/// Decode-boundary byte-domain validation blocks (one per predicate in the
/// mask, `ByteSequencePredicate::ALL` order -- the aarch64 twin's contract):
/// content ptr in r15, length in rax, sticky ok flag in r9d; rcx (walking
/// pointer), r11 (end bound), and r8 (byte scratch) are spent at this point
/// in the byte-slice sequence -- the target page claims r8 only AFTER these
/// checks. Widths via `wire_byte_predicate_checks_width` (which measures
/// this emitter -- it is pure, and a hand-summed constant for the ~90-entry
/// utf8 block would be pure drift risk).
fn append_wire_byte_predicate_checks(bytes: &mut Vec<u8>, predicate_mask: u8) {
    use omega_core::byte_predicates::ByteSequencePredicate;
    for predicate in ByteSequencePredicate::in_mask(predicate_mask) {
        match predicate {
            ByteSequencePredicate::NonEmpty => {
                bytes.extend([0x48, 0x85, 0xc0]); // test rax, rax
                bytes.extend([0x75, 0x03]); // jnz +3 (nonzero length: ok)
                bytes.extend([0x45, 0x31, 0xc9]); // xor r9d, r9d
            }
            ByteSequencePredicate::NoNul => {
                bytes.extend([0x4c, 0x89, 0xf9]); // mov rcx, r15 (p)
                bytes.extend([0x4d, 0x89, 0xfb]); // mov r11, r15
                bytes.extend([0x49, 0x01, 0xc3]); // add r11, rax (end)
                bytes.extend([0x4c, 0x39, 0xd9]); // loop: cmp rcx, r11
                bytes.extend([0x73, 0x0f]); // jae done (+15)
                bytes.extend([0x44, 0x0f, 0xb6, 0x01]); // movzx r8d, byte [rcx]
                bytes.extend([0x48, 0xff, 0xc1]); // inc rcx
                bytes.extend([0x45, 0x85, 0xc0]); // test r8d, r8d
                bytes.extend([0x75, 0xef]); // jnz loop (-17)
                bytes.extend([0x45, 0x31, 0xc9]); // xor r9d, r9d (a NUL byte)
            }
            ByteSequencePredicate::AsciiOnly => {
                bytes.extend([0x4c, 0x89, 0xf9]); // mov rcx, r15
                bytes.extend([0x4d, 0x89, 0xfb]); // mov r11, r15
                bytes.extend([0x49, 0x01, 0xc3]); // add r11, rax
                bytes.extend([0x4c, 0x39, 0xd9]); // loop: cmp rcx, r11
                bytes.extend([0x73, 0x10]); // jae done (+16)
                bytes.extend([0x44, 0x0f, 0xb6, 0x01]); // movzx r8d, byte [rcx]
                bytes.extend([0x48, 0xff, 0xc1]); // inc rcx
                bytes.extend([0x41, 0xf6, 0xc0, 0x80]); // test r8b, 0x80
                bytes.extend([0x74, 0xee]); // jz loop (-18)
                bytes.extend([0x45, 0x31, 0xc9]); // xor r9d, r9d (high bit set)
            }
            ByteSequencePredicate::ValidUtf8 => {
                append_wire_utf8_validation(bytes);
            }
        }
    }
}

/// UTF-8 validation over [r15, r15+rax): the aarch64 twin's decoded-scalar
/// walk in x86 idiom, dispatching on the LEAD before loading continuations
/// so ONE scratch register (r8) serves both roles. Lead classes: ASCII;
/// C2..DF one continuation; E0/ED/E1..EC,EE..EF two with E0 requiring
/// cont1 >= A0 (overlongs) and ED requiring cont1 < A0 (surrogates);
/// F0/F1..F3/F4 three with F0 requiring cont1 >= 90 and F4 requiring
/// cont1 < 90 (beyond U+10FFFF); 0x80..0xC1 and 0xF5.. invalid. Assembled
/// with a local two-pass label resolver; ALL label branches are rel32 for
/// uniform, safe distances.
fn append_wire_utf8_validation(bytes: &mut Vec<u8>) {
    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    enum Label {
        Loop,
        Two,
        E0Block,
        EdBlock,
        ThreePlain,
        OneMore,
        F0Block,
        F4Block,
        FourPlain,
        TwoMore,
        Fail,
        Done,
    }
    enum Ins {
        Fixed(&'static [u8]),
        /// jcc rel32: (0x0f, opcode) pair.
        Jcc(u8, Label),
        Jmp(Label),
    }
    use Ins::*;
    use Label::*;
    const JB: u8 = 0x82; // unsigned <
    const JAE: u8 = 0x83; // unsigned >=
    const JNE: u8 = 0x85;

    // One continuation read with an UNSIGNED range check [low, high):
    // bounds, load into r8d, range compare. cmp r8d, imm32 (41 81 f8 + 4).
    fn continuation(
        program: &mut Vec<(Option<Label>, Ins)>,
        at: Option<Label>,
        low: u32,
        high: u32,
    ) {
        let cmp_imm = |value: u32| -> &'static [u8] {
            // Leaked tiny allocations keep Ins::Fixed 'static; bounded by the
            // fixed set of (low, high) pairs this validator uses.
            Box::leak(
                [0x41, 0x81, 0xf8]
                    .iter()
                    .copied()
                    .chain(value.to_le_bytes())
                    .collect::<Vec<u8>>()
                    .into_boxed_slice(),
            )
        };
        program.push((at, Fixed(&[0x4c, 0x39, 0xd9]))); // cmp rcx, r11
        program.push((None, Jcc(JAE, Fail))); // truncated
        program.push((None, Fixed(&[0x44, 0x0f, 0xb6, 0x01]))); // movzx r8d, [rcx]
        program.push((None, Fixed(&[0x48, 0xff, 0xc1]))); // inc rcx
        program.push((None, Fixed(cmp_imm(low))));
        program.push((None, Jcc(JB, Fail)));
        program.push((None, Fixed(cmp_imm(high))));
        program.push((None, Jcc(JAE, Fail)));
    }
    fn lead_cmp(value: u32) -> &'static [u8] {
        Box::leak(
            [0x41, 0x81, 0xf8]
                .iter()
                .copied()
                .chain(value.to_le_bytes())
                .collect::<Vec<u8>>()
                .into_boxed_slice(),
        )
    }

    let mut program: Vec<(Option<Label>, Ins)> = Vec::new();
    program.push((None, Fixed(&[0x4c, 0x89, 0xf9]))); // mov rcx, r15 (p)
    program.push((None, Fixed(&[0x4d, 0x89, 0xfb]))); // mov r11, r15
    program.push((None, Fixed(&[0x49, 0x01, 0xc3]))); // add r11, rax (end)
    program.push((Some(Loop), Fixed(&[0x4c, 0x39, 0xd9]))); // cmp rcx, r11
    program.push((None, Jcc(JAE, Done)));
    program.push((None, Fixed(&[0x44, 0x0f, 0xb6, 0x01]))); // movzx r8d, [rcx] (lead)
    program.push((None, Fixed(&[0x48, 0xff, 0xc1]))); // inc rcx
    program.push((None, Fixed(lead_cmp(0x80))));
    program.push((None, Jcc(JB, Loop))); // ASCII
    program.push((None, Fixed(lead_cmp(0xC2))));
    program.push((None, Jcc(JB, Fail))); // invalid lead 0x80..0xC1
    program.push((None, Fixed(lead_cmp(0xE0))));
    program.push((None, Jcc(JB, Two))); // C2..DF
    program.push((None, Jcc(JNE, Fail))); // placeholder replaced below
    program.pop(); // (structured dispatch below instead)
    // Dispatch E0 / ED / other-threes / F0 / F4 / other-fours / >= F5.
    program.push((None, Fixed(lead_cmp(0xE0 + 1)))); // cmp 0xE1
    program.push((None, Jcc(JB, E0Block))); // exactly 0xE0
    program.push((None, Fixed(lead_cmp(0xED))));
    program.push((None, Jcc(JB, ThreePlain))); // E1..EC
    program.push((None, Fixed(lead_cmp(0xED + 1)))); // cmp 0xEE
    program.push((None, Jcc(JB, EdBlock))); // exactly 0xED
    program.push((None, Fixed(lead_cmp(0xF0))));
    program.push((None, Jcc(JB, ThreePlain))); // EE..EF
    program.push((None, Fixed(lead_cmp(0xF0 + 1)))); // cmp 0xF1
    program.push((None, Jcc(JB, F0Block))); // exactly 0xF0
    program.push((None, Fixed(lead_cmp(0xF4))));
    program.push((None, Jcc(JB, FourPlain))); // F1..F3
    program.push((None, Fixed(lead_cmp(0xF4 + 1)))); // cmp 0xF5
    program.push((None, Jcc(JAE, Fail))); // F5..
    // exactly 0xF4 falls through:
    continuation(&mut program, Some(F4Block), 0x80, 0x90);
    program.push((None, Jmp(TwoMore)));
    continuation(&mut program, Some(F0Block), 0x90, 0xC0);
    program.push((None, Jmp(TwoMore)));
    continuation(&mut program, Some(FourPlain), 0x80, 0xC0);
    program.push((Some(TwoMore), Fixed(&[]))); // label carrier
    continuation(&mut program, None, 0x80, 0xC0);
    continuation(&mut program, None, 0x80, 0xC0);
    program.push((None, Jmp(Loop)));
    continuation(&mut program, Some(E0Block), 0xA0, 0xC0);
    program.push((None, Jmp(OneMore)));
    continuation(&mut program, Some(EdBlock), 0x80, 0xA0);
    program.push((None, Jmp(OneMore)));
    continuation(&mut program, Some(ThreePlain), 0x80, 0xC0);
    program.push((Some(OneMore), Fixed(&[]))); // label carrier
    continuation(&mut program, None, 0x80, 0xC0);
    program.push((None, Jmp(Loop)));
    continuation(&mut program, Some(Two), 0x80, 0xC0);
    program.push((None, Jmp(Loop)));
    program.push((Some(Fail), Fixed(&[0x45, 0x31, 0xc9]))); // xor r9d, r9d
    // Done = first instruction after the block.

    // Pass 1: byte positions (Jcc = 6 bytes, Jmp = 5, Fixed = len).
    let width_of = |instruction: &Ins| -> usize {
        match instruction {
            Fixed(word) => word.len(),
            Jcc(..) => 6,
            Jmp(..) => 5,
        }
    };
    let mut positions = std::collections::HashMap::new();
    let mut at = 0usize;
    for (label, instruction) in &program {
        if let Some(label) = label {
            positions.insert(*label, at);
        }
        at += width_of(instruction);
    }
    positions.insert(Done, at);
    // Pass 2: emit with resolved rel32 offsets (relative to the next
    // instruction's start).
    let mut at = 0usize;
    for (_, instruction) in &program {
        let end = at + width_of(instruction);
        match instruction {
            Fixed(word) => bytes.extend(*word),
            Jcc(opcode, target) => {
                bytes.extend([0x0f, *opcode]);
                bytes.extend(((positions[target] as i64 - end as i64) as i32).to_le_bytes());
            }
            Jmp(target) => {
                bytes.push(0xe9);
                bytes.extend(((positions[target] as i64 - end as i64) as i32).to_le_bytes());
            }
        }
        at = end;
    }
}

/// Bytes of [`append_wire_byte_predicate_checks`]: measured from the pure
/// emitter itself -- ONE source of truth (a hand-summed constant for the
/// ~90-entry utf8 block would be pure drift risk).
pub fn wire_byte_predicate_checks_width(predicate_mask: u8) -> usize {
    let mut scratch = Vec::new();
    append_wire_byte_predicate_checks(&mut scratch, predicate_mask);
    scratch.len()
}

pub fn read_wire_byte_slice_width(
    _buffer_offset: usize,
    _buffer_length: usize,
    _read_offset: usize,
    _ok_offset: usize,
    _target_offset: usize,
    predicate_mask: u8,
) -> usize {
    // Prologue + success/value/shift init (10) + read loop + bounds&advance
    // (21) + the byte-predicate validation blocks + target imm64 (10) + ptr
    // store (7) + len store (7) + epilogue.
    wire_decode_prologue_width()
        + 10
        + wire_varint_read_loop_width()
        + 21
        + wire_byte_predicate_checks_width(predicate_mask)
        + 10
        + 7
        + 7
        + wire_decode_tail_width()
}

/// Byte offset of the TARGET page mov inside the byte-slice decode (the
/// relocation planner adds the +2 imm64 offset itself).
pub fn wire_decode_byte_slice_target_page_offset(
    _buffer_offset: usize,
    _buffer_length: usize,
    _read_offset: usize,
    predicate_mask: u8,
) -> usize {
    // The validation blocks precede the target page mov.
    wire_decode_prologue_width()
        + 10
        + wire_varint_read_loop_width()
        + 21
        + wire_byte_predicate_checks_width(predicate_mask)
}

/// Byte offset of the READ (cursor) page mov inside both wire decodes (the
/// relocation planner adds the +2 imm64 offset itself).
pub fn wire_decode_read_page_offset(_buffer_offset: usize) -> usize {
    17
}

/// Byte offset of the OK (sticky flag) page mov inside both wire decodes.
pub fn wire_decode_ok_page_offset(_buffer_offset: usize, _read_offset: usize) -> usize {
    37
}

/// Byte offset of the TARGET page mov inside the varint decode.
pub fn wire_decode_varint_target_page_offset(
    _buffer_offset: usize,
    _buffer_length: usize,
    _read_offset: usize,
    zigzag: bool,
) -> usize {
    wire_decode_prologue_width()
        + 10
        + wire_varint_read_loop_width()
        + if zigzag { wire_unzigzag_width() } else { 0 }
}

pub fn read_wire_nested_open_width(
    _buffer_offset: usize,
    _buffer_length: usize,
    _read_offset: usize,
    _ok_offset: usize,
    _end_offset: usize,
) -> usize {
    // Prologue + end page mov (10) + length load (7) + success mov (6) +
    // length cmp/jbe/fail xor (11) + end add (3) + bound cmp/jbe/fail xor
    // (11) + end store (7) + epilogue.
    wire_decode_prologue_width() + 55 + wire_decode_tail_width()
}

/// Open a nested sub-message region (chapter 20, nested message fields): the
/// end slot holds the sub-message LENGTH the caller just varint-read into it;
/// replace it with the ABSOLUTE end bound (`cursor + length`) and clear ok
/// when that bound exceeds the buffer's compile-time length. The cursor does
/// not move (the epilogue's write-back stores it unchanged, keeping the
/// shared prologue/epilogue and their relocation offsets identical to the
/// other wire decodes).
pub fn encode_read_wire_nested_open(
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
    end_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(read_wire_nested_open_width(
        buffer_offset,
        buffer_length,
        read_offset,
        ok_offset,
        end_offset,
    ));
    append_wire_decode_prologue(&mut bytes, buffer_offset, read_offset)?;

    // r8 = the end-slot page (imm64 relocated at
    // `wire_decode_nested_end_page_offset`), rax = the LENGTH stored there.
    bytes.extend([0x49, 0xb8]); // mov r8, imm64(end page)
    bytes.extend(0u64.to_le_bytes());
    let end_displacement = disp32(end_offset)?;
    bytes.extend([0x49, 0x8b, 0x80]); // mov rax, [r8+disp32]
    bytes.extend(end_displacement.to_le_bytes());

    // ok &= length <= buffer length (a raw length past the buffer could wrap
    // the 64-bit end sum back inside the bound -- reject it before adding);
    // then end = cursor + length and ok &= end <= buffer length. The cursor
    // never exceeds the buffer length and the length just passed its own
    // check, so the sum cannot wrap.
    //          mov  r9d, 1
    //          cmp  rax, imm32(length)
    //          jbe  len_ok          (+3: skip the fail xor)
    //   fail1: xor  r9d, r9d
    //  len_ok: add  rax, r10
    //          cmp  rax, imm32(length)
    //          jbe  done            (+3: bound fits -- skip the fail xor)
    //   fail2: xor  r9d, r9d
    //   done:
    bytes.extend([0x41, 0xb9, 0x01, 0x00, 0x00, 0x00]); // mov r9d, 1
    bytes.extend([0x48, 0x3d]); // cmp rax, imm32
    bytes.extend(wire_decode_length_imm32(buffer_length)?.to_le_bytes());
    bytes.extend([0x76, 0x03]); // jbe +3 -> len_ok
    bytes.extend([0x45, 0x31, 0xc9]); // xor r9d, r9d
    bytes.extend([0x4c, 0x01, 0xd0]); // add rax, r10
    bytes.extend([0x48, 0x3d]); // cmp rax, imm32
    bytes.extend(wire_decode_length_imm32(buffer_length)?.to_le_bytes());
    bytes.extend([0x76, 0x03]); // jbe +3 -> done
    bytes.extend([0x45, 0x31, 0xc9]); // xor r9d, r9d

    bytes.extend([0x49, 0x89, 0x80]); // mov [r8+disp32], rax
    bytes.extend(end_displacement.to_le_bytes());

    append_wire_decode_epilogue(&mut bytes, read_offset, ok_offset)?;
    debug_assert_eq!(
        bytes.len(),
        read_wire_nested_open_width(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            end_offset
        )
    );
    Ok(bytes)
}

pub fn read_wire_nested_close_width(
    _buffer_offset: usize,
    _read_offset: usize,
    _ok_offset: usize,
    _end_offset: usize,
) -> usize {
    // Prologue + end page mov (10) + end load (7) + success mov (6) +
    // cursor cmp (3) + je (2) + fail xor (3) + epilogue.
    wire_decode_prologue_width() + 31 + wire_decode_tail_width()
}

/// Close a nested sub-message region (chapter 20, nested message fields):
/// clear ok unless the cursor landed EXACTLY on the end bound the matching
/// open stored -- the declared sub-message length must equal the bytes its
/// fields consumed. The cursor does not move.
pub fn encode_read_wire_nested_close(
    buffer_offset: usize,
    read_offset: usize,
    ok_offset: usize,
    end_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(read_wire_nested_close_width(
        buffer_offset,
        read_offset,
        ok_offset,
        end_offset,
    ));
    append_wire_decode_prologue(&mut bytes, buffer_offset, read_offset)?;

    // r8 = the end-slot page, rax = the end bound stored there.
    bytes.extend([0x49, 0xb8]); // mov r8, imm64(end page)
    bytes.extend(0u64.to_le_bytes());
    let end_displacement = disp32(end_offset)?;
    bytes.extend([0x49, 0x8b, 0x80]); // mov rax, [r8+disp32]
    bytes.extend(end_displacement.to_le_bytes());

    // ok &= cursor == end:
    //         mov  r9d, 1
    //         cmp  r10, rax
    //         je   done            (+3: skip the fail xor)
    //   fail: xor  r9d, r9d
    //   done:
    bytes.extend([0x41, 0xb9, 0x01, 0x00, 0x00, 0x00]); // mov r9d, 1
    bytes.extend([0x49, 0x39, 0xc2]); // cmp r10, rax
    bytes.extend([0x74, 0x03]); // je +3 -> done
    bytes.extend([0x45, 0x31, 0xc9]); // xor r9d, r9d

    append_wire_decode_epilogue(&mut bytes, read_offset, ok_offset)?;
    debug_assert_eq!(
        bytes.len(),
        read_wire_nested_close_width(buffer_offset, read_offset, ok_offset, end_offset)
    );
    Ok(bytes)
}

/// Byte offset of the END-slot page mov inside both nested decodes
/// (materialized right after the shared prologue). The repeated-element read
/// materializes its end page at the same position.
pub fn wire_decode_nested_end_page_offset(_buffer_offset: usize, _read_offset: usize) -> usize {
    wire_decode_prologue_width()
}

// ---- compact_binary v0 wire REPEATED fields (chapter 20) ----
//
// A repeated field packs LENGTH-delimited (tag + byte-length varint +
// back-to-back element varints). The element count is runtime-sized but
// bounded by the schema's declared maximum, so selection UNROLLS the maximum
// and each unrolled operation guards itself: the encode-side append runs only
// when its compile-time element index is below the count-companion slot's
// value; the decode-side read runs only while the cursor sits below the end
// bound the surrounding nested OPEN stored. Guarding keeps every emitted
// width compile-time-fixed (the widths invariant) while the wire bytes track
// the live count.

/// Guard block of the repeated scalar append: count page mov (10, relocated)
/// + count load (7) + index cmp (7) + jbe skip (2).
fn wire_repeated_append_guard_width() -> usize {
    26
}

pub fn append_wire_repeated_scalar_varint_width(
    _source_offset: usize,
    byte_size: usize,
    zigzag: bool,
    _index: u64,
    _count_offset: usize,
    _out_offset: usize,
    _written_offset: usize,
) -> usize {
    // Prologue + guard + source imm64 (10) + sized load + optional zigzag +
    // emit loop + cursor store (7).
    wire_append_prologue_width()
        + wire_repeated_append_guard_width()
        + 10
        + wire_varint_source_load_width(byte_size)
        + if zigzag { wire_zigzag_width() } else { 0 }
        + wire_varint_emit_loop_width()
        + 7
}

/// LEB128-encode element `index` of a packed repeated field at the cursor,
/// ONLY IF `index < count` (the count-companion slot, read as unsigned
/// 64-bit). A skipped element leaves the cursor untouched, so the staged
/// payload holds exactly the live elements. Counts past the declared maximum
/// clamp for free: selection unrolls only `max` of these.
#[allow(clippy::too_many_arguments)]
pub fn encode_append_wire_repeated_scalar_varint(
    source_region: omega_target_operations::RuntimeStorageRegion,
    source_offset: usize,
    byte_size: usize,
    zigzag: bool,
    index: u64,
    count_region: omega_target_operations::RuntimeStorageRegion,
    count_offset: usize,
    out_offset: usize,
    written_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "X86_64 wire encoder cannot varint-encode {byte_size}-byte scalars yet"
        )));
    }
    let index_imm = i32::try_from(index).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 wire encoder cannot guard repeated element index {index} yet"
        ))
    })?;
    // The regions only pick the relocation symbols; the encoded shape is
    // identical for machine and frame places.
    let _ = (source_region, count_region);

    let mut bytes = Vec::with_capacity(append_wire_repeated_scalar_varint_width(
        source_offset,
        byte_size,
        zigzag,
        index,
        count_offset,
        out_offset,
        written_offset,
    ));
    append_wire_append_prologue(&mut bytes, out_offset, written_offset)?;

    // Guard: r9 = count (from the relocated count page); skip the whole
    // append when count <= index (unsigned). The skip lands past the cursor
    // store, so a skipped element changes nothing.
    let skip_distance = 10
        + wire_varint_source_load_width(byte_size)
        + if zigzag { wire_zigzag_width() } else { 0 }
        + wire_varint_emit_loop_width()
        + 7;
    let skip_rel8 =
        i8::try_from(skip_distance).expect("the guarded append body is well under the rel8 range");
    bytes.extend([0x49, 0xb9]); // mov r9, imm64(count page)
    bytes.extend(0u64.to_le_bytes());
    bytes.extend([0x4d, 0x8b, 0x89]); // mov r9, [r9+disp32]
    bytes.extend(disp32(count_offset)?.to_le_bytes());
    bytes.extend([0x49, 0x81, 0xf9]); // cmp r9, imm32(index)
    bytes.extend(index_imm.to_le_bytes());
    bytes.extend([0x76, skip_rel8 as u8]); // jbe skip (count <= index)

    // The unguarded scalar-varint body (see `encode_append_wire_scalar_varint`):
    // r11 = source page (imm64 relocated), rax = the scalar.
    append_mov_reg_imm64(&mut bytes, Reg64::R11, 0);
    let displacement = disp32(source_offset)?;
    match (byte_size, zigzag) {
        (8, _) => bytes.extend([0x49, 0x8b, 0x83]), // mov rax, [r11+disp32]
        (4, false) => bytes.extend([0x41, 0x8b, 0x83]), // mov eax, [r11+disp32]
        (4, true) => bytes.extend([0x49, 0x63, 0x83]), // movsxd rax, dword [r11+disp32]
        (1, _) => bytes.extend([0x41, 0x0f, 0xb6, 0x83]), // movzx eax, byte [r11+disp32]
        _ => unreachable!("byte_size validated above"),
    }
    bytes.extend(displacement.to_le_bytes());

    if zigzag {
        bytes.extend([0x49, 0x89, 0xc3]); // mov r11, rax
        bytes.extend([0x49, 0xc1, 0xfb, 0x3f]); // sar r11, 63
        bytes.extend([0x48, 0xc1, 0xe0, 0x01]); // shl rax, 1
        bytes.extend([0x4c, 0x31, 0xd8]); // xor rax, r11
    }

    // The same fixed 40-byte LEB128 emit loop as the scalar varint.
    bytes.extend([0x49, 0x89, 0xc3]); // mov r11, rax
    bytes.extend([0x49, 0x83, 0xe3, 0x7f]); // and r11, 0x7f
    bytes.extend([0x48, 0xc1, 0xe8, 0x07]); // shr rax, 7
    bytes.extend([0x48, 0x85, 0xc0]); // test rax, rax
    bytes.extend([0x74, 0x12]); // je +18 -> last
    bytes.extend([0x49, 0x81, 0xcb, 0x80, 0x00, 0x00, 0x00]); // or r11, 0x80
    bytes.extend([0x45, 0x88, 0x1f]); // mov [r15], r11b
    bytes.extend([0x49, 0xff, 0xc7]); // inc r15
    bytes.extend([0x49, 0xff, 0xc2]); // inc r10
    bytes.extend([0xeb, 0xde]); // jmp -34 -> loop
    bytes.extend([0x45, 0x88, 0x1f]); // mov [r15], r11b
    bytes.extend([0x49, 0xff, 0xc2]); // inc r10

    append_store_r10_to_r14(&mut bytes, written_offset, 8)?;
    debug_assert_eq!(
        bytes.len(),
        append_wire_repeated_scalar_varint_width(
            source_offset,
            byte_size,
            zigzag,
            index,
            count_offset,
            out_offset,
            written_offset
        )
    );
    Ok(bytes)
}

/// Byte offset of the COUNT page mov inside the repeated append (right after
/// the shared prologue).
pub fn wire_append_repeated_count_page_offset(_out_offset: usize, _written_offset: usize) -> usize {
    wire_append_prologue_width()
}

/// Byte offset of the SOURCE page mov inside the repeated append (after the
/// guard block).
pub fn wire_append_repeated_source_page_offset(
    _out_offset: usize,
    _written_offset: usize,
    _count_offset: usize,
    _index: u64,
) -> usize {
    wire_append_prologue_width() + wire_repeated_append_guard_width()
}

/// Guard block of the repeated scalar read: end page mov (10, relocated) +
/// end load (7) + cursor cmp (3) + jae rel32 skip (6).
fn wire_repeated_read_guard_width() -> usize {
    26
}

/// Count bump of the repeated scalar read: count page mov (10, relocated) +
/// count load (7) + inc (3) + count store (7).
fn wire_repeated_read_count_bump_width() -> usize {
    27
}

#[allow(clippy::too_many_arguments)]
pub fn read_wire_repeated_scalar_varint_width(
    _buffer_offset: usize,
    _buffer_length: usize,
    _read_offset: usize,
    _ok_offset: usize,
    _end_offset: usize,
    _count_offset: usize,
    _target_offset: usize,
    _byte_size: usize,
    zigzag: bool,
) -> usize {
    // Prologue + guard + success/value/shift init (10) + read loop + optional
    // unzigzag + target imm64 (10) + truncating store (7) + count bump +
    // epilogue.
    wire_decode_prologue_width()
        + wire_repeated_read_guard_width()
        + 10
        + wire_varint_read_loop_width()
        + if zigzag { wire_unzigzag_width() } else { 0 }
        + 10
        + 7
        + wire_repeated_read_count_bump_width()
        + wire_decode_tail_width()
}

/// LEB128-read one packed repeated element at the cursor into the target
/// slot, ONLY IF the cursor sits strictly below the end bound the
/// surrounding nested OPEN stored; the taken path also increments the
/// count-companion slot. A skipped read changes nothing -- the jump lands
/// past the epilogue, so cursor, ok, target, and count all stay put.
/// Selection unrolls the declared maximum of these, so a payload packing
/// more elements leaves the cursor short of the bound and the closing
/// nested CLOSE clears ok (the hostile-count cap); every taken read stays
/// bounds-checked against the buffer like any other wire read.
#[allow(clippy::too_many_arguments)]
pub fn encode_read_wire_repeated_scalar_varint(
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
    end_offset: usize,
    count_region: omega_target_operations::RuntimeStorageRegion,
    count_offset: usize,
    target_region: omega_target_operations::RuntimeStorageRegion,
    target_offset: usize,
    byte_size: usize,
    zigzag: bool,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "X86_64 wire decoder cannot varint-decode {byte_size}-byte scalars yet"
        )));
    }
    // The regions only pick the relocation symbols; the encoded shape is
    // identical for machine and frame places.
    let _ = (count_region, target_region);

    let mut bytes = Vec::with_capacity(read_wire_repeated_scalar_varint_width(
        buffer_offset,
        buffer_length,
        read_offset,
        ok_offset,
        end_offset,
        count_offset,
        target_offset,
        byte_size,
        zigzag,
    ));
    append_wire_decode_prologue(&mut bytes, buffer_offset, read_offset)?;

    // Guard: r8 = the end-slot page (imm64 relocated at the nested end page
    // offset), rax = the absolute end bound stored there; skip everything
    // (including the epilogue) when cursor >= end.
    let skip_distance = 10
        + wire_varint_read_loop_width()
        + if zigzag { wire_unzigzag_width() } else { 0 }
        + 10
        + 7
        + wire_repeated_read_count_bump_width()
        + wire_decode_tail_width();
    bytes.extend([0x49, 0xb8]); // mov r8, imm64(end page)
    bytes.extend(0u64.to_le_bytes());
    let end_displacement = disp32(end_offset)?;
    bytes.extend([0x49, 0x8b, 0x80]); // mov rax, [r8+disp32]
    bytes.extend(end_displacement.to_le_bytes());
    bytes.extend([0x49, 0x39, 0xc2]); // cmp r10, rax
    bytes.extend([0x0f, 0x83]); // jae rel32 -> skip
    bytes.extend(
        i32::try_from(skip_distance)
            .expect("the guarded read body is well under the rel32 range")
            .to_le_bytes(),
    );

    // The unguarded scalar-varint body (see `encode_read_wire_scalar_varint`).
    bytes.extend([0x41, 0xb9, 0x01, 0x00, 0x00, 0x00]); // mov r9d, 1
    bytes.extend([0x31, 0xc0]); // xor eax, eax (value)
    bytes.extend([0x31, 0xc9]); // xor ecx, ecx (shift)

    bytes.extend([0x48, 0x83, 0xf9, 0x3f]); // cmp rcx, 63
    bytes.extend([0x77, 0x2f]); // ja +47 -> fail
    bytes.extend([0x49, 0x81, 0xfa]); // cmp r10, imm32
    bytes.extend(wire_decode_length_imm32(buffer_length)?.to_le_bytes());
    bytes.extend([0x73, 0x26]); // jae +38 -> fail
    bytes.extend([0x45, 0x0f, 0xb6, 0x1f]); // movzx r11d, byte [r15]
    bytes.extend([0x49, 0xff, 0xc7]); // inc r15
    bytes.extend([0x49, 0xff, 0xc2]); // inc r10
    bytes.extend([0x4d, 0x89, 0xd8]); // mov r8, r11
    bytes.extend([0x49, 0x83, 0xe0, 0x7f]); // and r8, 0x7f
    bytes.extend([0x49, 0xd3, 0xe0]); // shl r8, cl
    bytes.extend([0x4c, 0x09, 0xc0]); // or rax, r8
    bytes.extend([0x48, 0x83, 0xc1, 0x07]); // add rcx, 7
    bytes.extend([0x49, 0xf7, 0xc3, 0x80, 0x00, 0x00, 0x00]); // test r11, 0x80
    bytes.extend([0x75, 0xcd]); // jnz -51 -> loop
    bytes.extend([0xeb, 0x03]); // jmp +3 -> done
    bytes.extend([0x45, 0x31, 0xc9]); // xor r9d, r9d

    if zigzag {
        bytes.extend([0x49, 0x89, 0xc3]); // mov r11, rax
        bytes.extend([0x49, 0x83, 0xe3, 0x01]); // and r11, 1
        bytes.extend([0x49, 0xf7, 0xdb]); // neg r11
        bytes.extend([0x48, 0xd1, 0xe8]); // shr rax, 1
        bytes.extend([0x4c, 0x31, 0xd8]); // xor rax, r11
    }

    // r8 = the target page (imm64 relocated), then the truncating store.
    bytes.extend([0x49, 0xb8]); // mov r8, imm64(target page)
    bytes.extend(0u64.to_le_bytes());
    let target_displacement = disp32(target_offset)?;
    match byte_size {
        1 => bytes.extend([0x41, 0x88, 0x80]), // mov [r8+disp32], al
        4 => bytes.extend([0x41, 0x89, 0x80]), // mov [r8+disp32], eax
        8 => bytes.extend([0x49, 0x89, 0x80]), // mov [r8+disp32], rax
        _ => unreachable!("byte_size validated above"),
    }
    bytes.extend(target_displacement.to_le_bytes());

    // Count bump: r11 = the count page (imm64 relocated), rcx = count + 1
    // (rcx is free -- the read loop's shift use ended above).
    bytes.extend([0x49, 0xbb]); // mov r11, imm64(count page)
    bytes.extend(0u64.to_le_bytes());
    let count_displacement = disp32(count_offset)?;
    bytes.extend([0x49, 0x8b, 0x8b]); // mov rcx, [r11+disp32]
    bytes.extend(count_displacement.to_le_bytes());
    bytes.extend([0x48, 0xff, 0xc1]); // inc rcx
    bytes.extend([0x49, 0x89, 0x8b]); // mov [r11+disp32], rcx
    bytes.extend(count_displacement.to_le_bytes());

    append_wire_decode_epilogue(&mut bytes, read_offset, ok_offset)?;
    debug_assert_eq!(
        bytes.len(),
        read_wire_repeated_scalar_varint_width(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            end_offset,
            count_offset,
            target_offset,
            byte_size,
            zigzag
        )
    );
    Ok(bytes)
}

/// Byte offset of the TARGET page mov inside the repeated read (after the
/// guard block and the read loop).
pub fn wire_decode_repeated_target_page_offset(
    _buffer_offset: usize,
    _buffer_length: usize,
    _read_offset: usize,
    _end_offset: usize,
    zigzag: bool,
) -> usize {
    wire_decode_prologue_width()
        + wire_repeated_read_guard_width()
        + 10
        + wire_varint_read_loop_width()
        + if zigzag { wire_unzigzag_width() } else { 0 }
}

/// Byte offset of the COUNT page mov inside the repeated read (after the
/// target store).
#[allow(clippy::too_many_arguments)]
pub fn wire_decode_repeated_count_page_offset(
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    end_offset: usize,
    _target_offset: usize,
    _byte_size: usize,
    zigzag: bool,
) -> usize {
    wire_decode_repeated_target_page_offset(
        buffer_offset,
        buffer_length,
        read_offset,
        end_offset,
        zigzag,
    ) + 10
        + 7
}

// Append a source carrier's content onto a target carrier (concat builder source
// segment, after the first literal initialized the target). r15 = machine
// storage base (reloc @ +2). rax = target running len; rcx = source len (rep
// count); rsi = source bytes (source + 8); rdi = target bytes + running len; copy
// rcx bytes; store new len = target_len + source_len. Fixed width (no per-byte
// loop), one relocation (the base).
pub fn runtime_machine_bounded_buffer_source_append_width(source_in_frame: bool) -> usize {
    // mov r15,imm64 (10) + mov rax,[r15+t] (7) + mov rcx,[base+s] (7)
    // + lea rsi,[base+s+8] (7) + lea rdi,[r15+t+8] (7) + add rdi,rax (3)
    // + rep movsb (2) + add rax,rcx (3) + mov [r15+t],rax (7) = 53.
    // A frame-local source adds `mov r14, imm64(frame)` (10) for the source base.
    if source_in_frame { 63 } else { 53 }
}

pub fn encode_runtime_machine_bounded_buffer_source_append(
    target_byte_offset: usize,
    source_byte_offset: usize,
    source_in_frame: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let target = disp32(target_byte_offset)?;
    let target_bytes = disp32(target_byte_offset + 8)?;
    let source = disp32(source_byte_offset)?;
    let source_bytes = disp32(source_byte_offset + 8)?;
    let mut bytes = Vec::with_capacity(runtime_machine_bounded_buffer_source_append_width(
        source_in_frame,
    ));
    append_mov_r15_imm64(&mut bytes, 0); // machine storage base (target; reloc @ +2)
    // The source carrier is read off r15 (machine) by default; a `let`-local source
    // loads the runtime frame base into r14 (a second relocation @ +12) and reads
    // from there. The two source instructions differ only in their base register.
    let (source_len_modrm, source_bytes_modrm) = if source_in_frame {
        append_mov_r14_imm64(&mut bytes, 0); // frame base (reloc @ +12)
        (0x8eu8, 0xb6u8) // mov rcx,[r14+s] ; lea rsi,[r14+s+8]
    } else {
        (0x8fu8, 0xb7u8) // mov rcx,[r15+s] ; lea rsi,[r15+s+8]
    };
    bytes.extend([0x49, 0x8b, 0x87]); // mov rax, [r15 + target]   (target running len)
    bytes.extend(target.to_le_bytes());
    bytes.extend([0x49, 0x8b, source_len_modrm]); // mov rcx, [base + source] (source len)
    bytes.extend(source.to_le_bytes());
    bytes.extend([0x49, 0x8d, 0xbf]); // lea rdi, [r15 + target+8] (target bytes base)
    bytes.extend(target_bytes.to_le_bytes());
    bytes.extend([0x48, 0x01, 0xc7]); // add rdi, rax  (target bytes + running len)
    // new len = target_len + source_len -- MUST precede `rep movsb`, which
    // decrements rcx to 0 as it copies; computing it after would always add 0.
    bytes.extend([0x48, 0x01, 0xc8]); // add rax, rcx  (rax = target_len + source_len)
    bytes.extend([0x49, 0x8d, source_bytes_modrm]); // lea rsi, [base + source+8] (source bytes)
    bytes.extend(source_bytes.to_le_bytes());
    bytes.extend([0xf3, 0xa4]); // rep movsb  (copy rcx bytes; consumes rcx)
    bytes.extend([0x49, 0x89, 0x87]); // mov [r15 + target], rax  (store new len)
    bytes.extend(target.to_le_bytes());
    debug_assert_eq!(
        bytes.len(),
        runtime_machine_bounded_buffer_source_append_width(source_in_frame)
    );
    Ok(bytes)
}

// Append a string LITERAL onto a target carrier at its running length (a later
// concat segment, e.g. the trailing `" =="`). r15 = machine storage base (reloc
// @ +2). rax = target running len; rdi = target bytes + running len; the literal
// bytes are written as immediates at `[rdi + i]`; store new len = old + lit.len.
// One relocation (the base); fixed width (no per-byte loop -- the bytes are
// unrolled immediate stores).
pub fn runtime_machine_bounded_buffer_literal_append_width(literal: &str) -> usize {
    // mov r15,imm64 (10) + mov rax,[r15+t] (7) + lea rdi,[r15+t+8] (7)
    // + add rdi,rax (3) + per byte: mov byte [rdi+disp8],imm8 (4)
    // + add rax,imm32 (`48 05`+imm32 = 6) + mov [r15+t],rax (7) = 40 + 4*len
    40 + 4 * literal.len()
}

pub fn encode_runtime_machine_bounded_buffer_literal_append(
    target_byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let target = disp32(target_byte_offset)?;
    let target_bytes = disp32(target_byte_offset + 8)?;
    let literal_bytes = literal.as_bytes();
    let literal_len = u32::try_from(literal_bytes.len()).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 encoder cannot append a carrier literal of {} bytes",
            literal_bytes.len()
        ))
    })?;
    let mut bytes =
        Vec::with_capacity(runtime_machine_bounded_buffer_literal_append_width(literal));
    append_mov_r15_imm64(&mut bytes, 0); // machine storage base (reloc @ +2)
    bytes.extend([0x49, 0x8b, 0x87]); // mov rax, [r15 + target]   (target running len)
    bytes.extend(target.to_le_bytes());
    bytes.extend([0x49, 0x8d, 0xbf]); // lea rdi, [r15 + target+8] (target bytes base)
    bytes.extend(target_bytes.to_le_bytes());
    bytes.extend([0x48, 0x01, 0xc7]); // add rdi, rax  (dest = target bytes + running len)
    for (index, byte) in literal_bytes.iter().enumerate() {
        let disp = u8::try_from(index).map_err(|_| {
            Diagnostic::error(
                "X86_64 encoder cannot append a carrier literal longer than 127 bytes".to_string(),
            )
        })?;
        bytes.extend([0xc6, 0x47, disp, *byte]); // mov byte [rdi + disp8], imm8
    }
    bytes.extend([0x48, 0x05]); // add rax, imm32  (new len = old + literal length)
    bytes.extend(literal_len.to_le_bytes());
    bytes.extend([0x49, 0x89, 0x87]); // mov [r15 + target], rax  (store new len)
    bytes.extend(target.to_le_bytes());
    debug_assert_eq!(
        bytes.len(),
        runtime_machine_bounded_buffer_literal_append_width(literal)
    );
    Ok(bytes)
}

/// Bytes inserted between the left and right operand evaluations of a binary
/// write on x86_64: a single `push r10` that preserves the left result while the
/// right operand is evaluated (both accumulate in r10). Relocation planning adds
/// this to the right operand's start offset.
pub const BINARY_RIGHT_OPERAND_PUSH_WIDTH: usize = 2;

#[allow(clippy::too_many_arguments)]
pub fn runtime_storage_binary_write_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
    is_float: bool,
    domain: ArithmeticDomain,
    target_signed: bool,
) -> usize {
    // The integer op is normally the default 64-bit op; Saturating/Trapping
    // instead emit a width-correct add/sub followed by the clamp/trap sequence.
    let saturating_or_trapping = !is_float
        && matches!(
            domain,
            ArithmeticDomain::Saturating | ArithmeticDomain::Trapping
        );
    let operation_width = if saturating_or_trapping && operator == StateGuardOperator::Multiply {
        saturating_trapping_multiply_width(
            domain,
            byte_size,
            target_signed,
            runtime_value_operands.immediate_integer(left).is_some(),
            runtime_value_operands.immediate_integer(right).is_some(),
        )
    } else if saturating_or_trapping && operator == StateGuardOperator::ShiftLeft {
        saturating_trapping_shift_left_width(domain, byte_size, target_signed)
    } else if saturating_or_trapping
        && matches!(
            operator,
            StateGuardOperator::Add | StateGuardOperator::Subtract
        )
    {
        saturating_trapping_add_sub_width(
            domain,
            operator,
            byte_size,
            target_signed,
            runtime_value_operands.immediate_integer(left).is_some(),
            runtime_value_operands.immediate_integer(right).is_some(),
        )
    } else if domain == ArithmeticDomain::Saturating
        && matches!(
            operator,
            StateGuardOperator::Divide | StateGuardOperator::Modulo
        )
    {
        // Saturating SIGNED divide/modulo wraps the normal idiv in a TYPE_MIN/-1
        // guard (see append_saturating_signed_divide_modulo).
        saturating_signed_divide_modulo_width(byte_size, operator == StateGuardOperator::Modulo)
    } else if domain == ArithmeticDomain::Wrapping
        && matches!(
            operator,
            StateGuardOperator::Divide | StateGuardOperator::Modulo
        )
    {
        // Wrapping SIGNED divide/modulo guards TYPE_MIN/-1 so idiv does not #DE
        // (see append_wrapping_signed_divide_modulo). Unsigned uses the *Unsigned
        // operators and cannot overflow, so it falls through.
        wrapping_signed_divide_modulo_width(byte_size, operator == StateGuardOperator::Modulo)
    } else if (domain == ArithmeticDomain::Wrapping && operator == StateGuardOperator::ShiftLeft)
        || (domain != ArithmeticDomain::Exact
            && matches!(
                operator,
                StateGuardOperator::ShiftRight | StateGuardOperator::ShiftRightLogical
            ))
    {
        // Domain-governed shifts: F8b WRAPPING masks the COUNT (sub-word AND
        // only; the hardware mask IS the ruling at widths 4/8), while
        // Saturating/Trapping `>>` keep the floor-semantics count fixes
        // until F8c. Same operand-derived byte size as the emission arm.
        let operation_byte_size = runtime_binary_operation_byte_size(
            runtime_value_operands,
            operator,
            left,
            right,
            byte_size,
        );
        let fix = if domain == ArithmeticDomain::Wrapping {
            wrapping_shift_count_mask_width(operation_byte_size)
        } else if domain == ArithmeticDomain::Trapping {
            SHIFT_COUNT_TRAP_GUARD_WIDTH
        } else if operator == StateGuardOperator::ShiftRight {
            WRAPPING_SHIFT_RIGHT_COUNT_SATURATE_WIDTH
        } else {
            WRAPPING_SHIFT_ZERO_CLAMP_WIDTH
        };
        runtime_binary_operation_width(operator, operation_byte_size) + fix
    } else if is_float {
        runtime_float_binary_operation_width_with_domain(operator, byte_size, domain)
    } else {
        // Trapping div/mod (idiv traps == Trapping semantics), Exact (proven
        // non-overflowing), and unsigned div/mod (cannot overflow) use the normal
        // op width -- derived from the OPERANDS exactly as the encoder does
        // (`runtime_binary_operation_byte_size`): div/mod/shift run at the
        // operand width, comparisons at the compared width. Pricing them at the
        // STORE's width instead diverges by a byte when e.g. a folded 8-byte
        // divide feeds a `% literal` into a 4-byte ranged slot (cqo idiv = 11,
        // 32-bit = 10).
        runtime_binary_operation_width(
            operator,
            runtime_binary_operation_byte_size(
                runtime_value_operands,
                operator,
                left,
                right,
                byte_size,
            ),
        )
    };
    // 10 (mov r14,imm64) + left + push r10 (2) + right + mov r11,r10 (3)
    // + pop r10 (2) + operation + store.
    10 + runtime_value_operand_width(runtime_value_operands, left)
        + 2
        + runtime_value_operand_width(runtime_value_operands, right)
        + 3
        + 2
        + operation_width
        + 7.max(store_width(byte_size))
}

/// Bytes of [`append_saturating_signed_divide_modulo`], for the relocation layout.
/// MUST equal the emitter exactly. cmp r11,-1 (4) + jne (2) + the divisor==-1
/// fixup + jmp (2) + the normal idiv core (the plain signed op width).
fn saturating_signed_divide_modulo_width(byte_size: usize, want_remainder: bool) -> usize {
    let fixup = if want_remainder {
        3 // xor r10d, r10d
    } else if byte_size <= 2 {
        16 // neg r10d (3) + mov r9d,imm32 (6) + cmp r10d,r9d (3) + cmovg r10d,r9d (4)
    } else if byte_size <= 4 {
        13 // neg r10d (3) + mov r9d,imm32 (6) + cmovo r10d,r9d (4)
    } else {
        17 // neg r10 (3) + mov r9,imm64 (10) + cmovo r10,r9 (4)
    };
    let normal = runtime_binary_operation_width(
        if want_remainder {
            StateGuardOperator::Modulo
        } else {
            StateGuardOperator::Divide
        },
        byte_size,
    );
    4 + 2 + fixup + 2 + normal
}

/// Bytes of [`append_wrapping_signed_divide_modulo`], for the relocation layout.
/// MUST equal the emitter exactly. cmp r11,-1 (4) + jne (2) + the divisor==-1
/// fixup (always 3: `neg r10` for divide, `xor r10d,r10d` for modulo) + jmp (2) +
/// the normal idiv core.
fn wrapping_signed_divide_modulo_width(byte_size: usize, want_remainder: bool) -> usize {
    let fixup = 3; // neg r10/r10d, or xor r10d,r10d
    let normal = runtime_binary_operation_width(
        if want_remainder {
            StateGuardOperator::Modulo
        } else {
            StateGuardOperator::Divide
        },
        byte_size,
    );
    4 + 2 + fixup + 2 + normal
}

/// The domain-honoring OPERAND-POSITION operation a fused `Binary` operand
/// needs, or `None` for the plain integer path. THE single dispatch shared by
/// the emission arm and its width twin so they can never disagree: Add/Sub and
/// Multiply under Saturating/Trapping clamp/trap; SIGNED div/mod under
/// Saturating take the TYPE_MIN/-1 clamp fixup and under Wrapping the idiv
/// #DE guard (unsigned div/mod use the *Unsigned operators, never overflow,
/// and fall through; Trapping div/mod fall through -- `idiv` traps on
/// overflow and /0, which IS Trapping semantics). Wrapping SHIFTS take the
/// at-width count fix (shift-domain ruling: shifts are value operations --
/// x * 2^n mod 2^w and floor(x / 2^n) -- but the hardware masks the count
/// instead): `<<` and logical `>>` clamp the result to zero, arithmetic `>>`
/// saturates the count to width-1 (= the sign-fill shift).
enum OperandDomainOperation {
    AddSub {
        domain: ArithmeticDomain,
        operands_signed: bool,
    },
    Multiply {
        domain: ArithmeticDomain,
        operands_signed: bool,
    },
    SaturatingSignedDivMod {
        want_remainder: bool,
    },
    WrappingSignedDivMod {
        want_remainder: bool,
    },
    // Carries the domain: Wrapping masks the COUNT (F8b), while
    // Saturating/Trapping `>>` keep the floor-semantics count fixes (F8c
    // pending) -- one variant, domain-dispatched at emission.
    DomainShift {
        domain: ArithmeticDomain,
        operands_signed: bool,
    },
    SaturatingTrappingShiftLeft {
        domain: ArithmeticDomain,
        operands_signed: bool,
    },
}

fn operand_position_domain_operation(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    operand: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
) -> Option<OperandDomainOperation> {
    let (domain, operands_signed) = runtime_value_operands.binary_arithmetic_domain(operand)?;
    match (operator, domain) {
        (
            StateGuardOperator::Add | StateGuardOperator::Subtract,
            ArithmeticDomain::Saturating | ArithmeticDomain::Trapping,
        ) => Some(OperandDomainOperation::AddSub {
            domain,
            operands_signed,
        }),
        (
            StateGuardOperator::Multiply,
            ArithmeticDomain::Saturating | ArithmeticDomain::Trapping,
        ) => Some(OperandDomainOperation::Multiply {
            domain,
            operands_signed,
        }),
        (StateGuardOperator::Divide | StateGuardOperator::Modulo, ArithmeticDomain::Saturating)
            if operands_signed =>
        {
            Some(OperandDomainOperation::SaturatingSignedDivMod {
                want_remainder: operator == StateGuardOperator::Modulo,
            })
        }
        (StateGuardOperator::Divide | StateGuardOperator::Modulo, ArithmeticDomain::Wrapping)
            if operands_signed =>
        {
            Some(OperandDomainOperation::WrappingSignedDivMod {
                want_remainder: operator == StateGuardOperator::Modulo,
            })
        }
        (StateGuardOperator::ShiftLeft, ArithmeticDomain::Wrapping) => {
            Some(OperandDomainOperation::DomainShift {
                domain,
                operands_signed,
            })
        }
        (
            StateGuardOperator::ShiftLeft,
            ArithmeticDomain::Saturating | ArithmeticDomain::Trapping,
        ) => Some(OperandDomainOperation::SaturatingTrappingShiftLeft {
            domain,
            operands_signed,
        }),
        // `>>` cannot overflow: Wrapping masks the count (F8b); the
        // floor-semantics count fix survives under Saturating/Trapping
        // until F8c. Both dispatch on the carried domain at emission.
        (
            StateGuardOperator::ShiftRight | StateGuardOperator::ShiftRightLogical,
            ArithmeticDomain::Wrapping | ArithmeticDomain::Saturating | ArithmeticDomain::Trapping,
        ) => Some(OperandDomainOperation::DomainShift {
            domain,
            operands_signed,
        }),
        _ => None,
    }
}

/// Bytes of [`append_width_integer_add_sub`]: 4 for 16-bit (0x66 prefix), else 3.
fn width_integer_add_sub_width(byte_size: usize) -> usize {
    if byte_size == 2 { 4 } else { 3 }
}

/// Width of the in-register operation step, dispatching to the SSE float op when
/// the write is floating-point.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_binary_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
    is_float: bool,
    domain: ArithmeticDomain,
    target_signed: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_binary_write_width(
        runtime_value_operands,
        byte_size,
        left,
        operator,
        right,
        is_float,
        domain,
        target_signed,
    ));
    // Hold the target base in r14, not r15: evaluating the operands below
    // reloads r15 with each source base, which would otherwise clobber the
    // target pointer before the store. r14 is untouched by operand evaluation.
    // `mov r14, imm64` and `mov r15, imm64` are both 10 bytes with the relocated
    // immediate at +2, so the target relocation offset is unchanged.
    append_mov_r14_imm64(&mut bytes, 0);
    append_binary_operands_op_and_store(
        runtime_value_operands,
        &mut bytes,
        target_offset,
        byte_size,
        left,
        operator,
        right,
        is_float,
        domain,
        target_signed,
    )?;
    Ok(bytes)
}

/// The target-address-AGNOSTIC half of every binary write: evaluate the
/// operand pair (r10 accumulator, left stashed across the right eval),
/// apply the operator under the arithmetic domain (floats, Saturating/
/// Trapping, shift-count policies), and store r10 to [r14 + target_offset].
/// The caller owns getting the target address into r14 (the retired
/// encoders' `mov r14,imm64`; the place materializer's walk + `mov r14,r15`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn append_binary_operands_op_and_store(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    bytes: &mut Vec<u8>,
    target_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
    is_float: bool,
    domain: ArithmeticDomain,
    target_signed: bool,
) -> Result<(), Diagnostic> {
    let saturating_or_trapping = !is_float
        && matches!(
            domain,
            ArithmeticDomain::Saturating | ArithmeticDomain::Trapping
        );
    // Each operand's evaluation accumulates in r10, so the right operand would
    // clobber the left result. Stash left on the stack across the right eval.
    append_runtime_value_operand(runtime_value_operands, bytes, Reg64::R10, left)?;
    append_push_r10(bytes);
    append_runtime_value_operand(runtime_value_operands, bytes, Reg64::R10, right)?;
    append_mov_reg_reg(bytes, Reg64::R11, Reg64::R10); // right -> r11
    append_pop_r10(bytes); // restore left -> r10
    let saturating_or_trapping = !is_float
        && matches!(
            domain,
            ArithmeticDomain::Saturating | ArithmeticDomain::Trapping
        );
    if is_float {
        // Comparisons run at the OPERAND width (a bool target is 1 byte, but
        // the xmm moves + ucomis need the f32/f64 width); arithmetic keeps the
        // target width, which equals the operand width for float targets.
        append_runtime_float_binary_operation(
            bytes,
            operator,
            runtime_binary_operation_byte_size(
                runtime_value_operands,
                operator,
                left,
                right,
                byte_size,
            ),
            domain,
        )?;
    } else if saturating_or_trapping && operator == StateGuardOperator::Multiply {
        // Saturating/Trapping multiply: a 64-bit `imul` yields the EXACT product
        // for <=32-bit operands (it cannot exceed 64 bits), so compare the full
        // product against the target type's range and clamp / trap.
        append_saturating_trapping_multiply(
            bytes,
            domain,
            byte_size,
            target_signed,
            runtime_value_operands.immediate_integer(left).is_some(),
            runtime_value_operands.immediate_integer(right).is_some(),
        )?;
    } else if saturating_or_trapping && operator == StateGuardOperator::ShiftLeft {
        // Saturating/Trapping `<<`: clamp/trap when the TRUE value x * 2^n
        // leaves the target range (shift slice C; mirrors aarch64).
        append_saturating_trapping_shift_left(bytes, domain, byte_size, target_signed)?;
    } else if saturating_or_trapping
        && matches!(
            operator,
            StateGuardOperator::Add | StateGuardOperator::Subtract
        )
    {
        // Decision 17: narrow targets wide-compute + range-tail (immune to
        // wide literal operands -- the MIN-idiom fix); 64-bit keeps the
        // flag-driven clamp inside the helper.
        append_saturating_trapping_add_sub(
            bytes,
            domain,
            operator,
            byte_size,
            target_signed,
            runtime_value_operands.immediate_integer(left).is_some(),
            runtime_value_operands.immediate_integer(right).is_some(),
        )?;
    } else if domain == ArithmeticDomain::Saturating
        && matches!(
            operator,
            StateGuardOperator::Divide | StateGuardOperator::Modulo
        )
    {
        // Saturating SIGNED divide/modulo: clamp the one overflowing corner
        // (TYPE_MIN / -1) to TYPE_MAX / 0 instead of trapping. The UNSIGNED variants
        // cannot overflow, so they are absent from this arm and fall through to the
        // normal path below. (Trapping div/mod also falls through, where `idiv`
        // traps on overflow and divide-by-zero -- exactly Trapping semantics.)
        append_saturating_signed_divide_modulo(
            bytes,
            byte_size,
            operator == StateGuardOperator::Modulo,
        )?;
    } else if domain == ArithmeticDomain::Wrapping
        && matches!(
            operator,
            StateGuardOperator::Divide | StateGuardOperator::Modulo
        )
    {
        // Wrapping SIGNED divide/modulo: guard TYPE_MIN / -1 so the bare `idiv`
        // does not raise #DE -- produce the WRAPPED result (TYPE_MIN / 0) instead.
        // Unsigned div/mod uses the *Unsigned operators (cannot overflow) and
        // falls through to the normal path below.
        append_wrapping_signed_divide_modulo(
            bytes,
            byte_size,
            operator == StateGuardOperator::Modulo,
        )?;
    } else if (domain == ArithmeticDomain::Wrapping && operator == StateGuardOperator::ShiftLeft)
        || (domain != ArithmeticDomain::Exact
            && matches!(
                operator,
                StateGuardOperator::ShiftRight | StateGuardOperator::ShiftRightLogical
            ))
    {
        // Domain-governed shifts: F8b (ch5 shift-count ruling) WRAPPING masks
        // the COUNT to the operand width -- the hardware `shl`/`shr`/`sar`
        // mask mod 32/64 already (the ruling at widths 4/8), sub-word widths
        // take the explicit AND. Saturating/Trapping `>>` keep the floor
        // fixes (arithmetic >> saturates the count to width-1 BEFORE the
        // sar; logical >> zero-clamps after) until F8c lands the count trap.
        // Matches interp + aarch64. The store's truncation remains the
        // in-range wrap.
        let operation_byte_size = runtime_binary_operation_byte_size(
            runtime_value_operands,
            operator,
            left,
            right,
            byte_size,
        );
        if domain == ArithmeticDomain::Wrapping {
            append_wrapping_shift_count_mask(bytes, operation_byte_size);
            append_runtime_binary_operation(bytes, operator, operation_byte_size)?;
        } else if domain == ArithmeticDomain::Trapping {
            // F8c: an out-of-range count traps before the shift, value-blind.
            append_shift_count_trap_guard(bytes, operation_byte_size);
            append_runtime_binary_operation(bytes, operator, operation_byte_size)?;
        } else {
            if operator == StateGuardOperator::ShiftRight {
                append_wrapping_shift_right_count_saturate(bytes, operation_byte_size);
            }
            append_runtime_binary_operation(bytes, operator, operation_byte_size)?;
            if operator != StateGuardOperator::ShiftRight {
                append_wrapping_shift_zero_clamp(bytes, operation_byte_size);
            }
        }
    } else {
        append_runtime_binary_operation(
            bytes,
            operator,
            runtime_binary_operation_byte_size(
                runtime_value_operands,
                operator,
                left,
                right,
                byte_size,
            ),
        )?;
    }
    append_store_r10_to_r14(bytes, target_offset, byte_size)?;
    Ok(())
}

/// Bytes of [`append_saturating_trapping_multiply`], for the relocation layout.
/// MUST equal what that function emits.
fn saturating_trapping_multiply_width(
    domain: ArithmeticDomain,
    byte_size: usize,
    target_signed: bool,
    left_is_wide_immediate: bool,
    right_is_wide_immediate: bool,
) -> usize {
    let imul = 4; // imul r10, r11
    if byte_size == 8 {
        // The 128-bit one-operand multiply sequences (see the emission's
        // byte_size == 8 arms). MUST equal them exactly.
        return match (domain, target_signed) {
            // mov+mov+imul+mov (12) + sar (4) + xor (3) + movabs (10)
            // + test (3) + mov (3) + not (3) + cmovns (4) + cmp (3) + cmovne (4)
            (ArithmeticDomain::Saturating, true) => 49,
            // mov+mul+mov (9) + movabs (10) + test (3) + cmovne (4)
            (ArithmeticDomain::Saturating, false) => 26,
            // mov+imul+mov (9) + sar (4) + cmp (3) + je (2) + ud2 (2)
            (ArithmeticDomain::Trapping, true) => 20,
            // mov+mul+mov (9) + test (3) + jz (2) + ud2 (2)
            (ArithmeticDomain::Trapping, false) => 16,
            _ => 0,
        };
    }
    if !matches!(byte_size, 1 | 2 | 4) {
        return imul; // emission errors; width is irrelevant then
    }
    // One sign-extension per SIGNED NON-IMMEDIATE operand (see emission):
    // movsx is 4 bytes (8/16-bit), movsxd is 3 (32-bit).
    let extend_one = |skip: bool| {
        if !target_signed || skip {
            0
        } else if byte_size == 4 {
            3
        } else {
            4
        }
    };
    let sign_extend = extend_one(left_is_wide_immediate) + extend_one(right_is_wide_immediate);
    imul + sign_extend + narrow_range_clamp_or_trap_width(domain, target_signed)
}

/// Saturating/Trapping multiply (decision 17). A 64-bit `imul r10, r11` produces
/// the EXACT product for <=32-bit operands (the product cannot exceed 64 bits),
/// so the full result is range-compared against the target type and clamped
/// (Saturating) or trapped (Trapping). 64-bit targets are not handled (the
/// product can exceed 64 bits -- needs the 128-bit `mul`/`imul` form). r11 (the
/// spent right operand) is the clamp-constant scratch.
fn append_saturating_trapping_multiply(
    bytes: &mut Vec<u8>,
    domain: ArithmeticDomain,
    byte_size: usize,
    target_signed: bool,
    left_is_wide_immediate: bool,
    right_is_wide_immediate: bool,
) -> Result<(), Diagnostic> {
    if byte_size == 8 {
        // 64-bit multiply overflow: the 128-bit one-operand forms make the
        // HIGH half the witness (RDX:RAX = RAX * r11), mirroring the aarch64
        // SMULH/UMULH arms. Signed overflow iff RDX != RAX>>63 (the low
        // half's sign broadcast); unsigned iff RDX != 0. r11 (the untouched
        // right operand), rax/rdx (clobbered by the multiply anyway), and
        // r9/r15 (free after operand evaluation) are the scratch. Branchless
        // (cmov), so the width is a constant per (domain, signedness).
        match (domain, target_signed) {
            (ArithmeticDomain::Saturating, true) => {
                // Boundary = MIN if the TRUE product sign (left^right) is
                // negative, else MAX = NOT(MIN); select it on overflow.
                bytes.extend([0x4c, 0x89, 0xd0]); // mov rax, r10
                bytes.extend([0x4d, 0x89, 0xd1]); // mov r9, r10  (save left)
                bytes.extend([0x49, 0xf7, 0xeb]); // imul r11    (rdx:rax)
                bytes.extend([0x49, 0x89, 0xc2]); // mov r10, rax (low)
                bytes.extend([0x48, 0xc1, 0xf8, 0x3f]); // sar rax, 63 (broadcast)
                bytes.extend([0x4d, 0x31, 0xd9]); // xor r9, r11 (true-sign witness)
                bytes.push(0x49);
                bytes.push(0xbf);
                bytes.extend((i64::MIN as u64).to_le_bytes()); // mov r15, MIN
                bytes.extend([0x4d, 0x85, 0xc9]); // test r9, r9
                bytes.extend([0x4d, 0x89, 0xf9]); // mov r9, r15 (MIN)
                bytes.extend([0x49, 0xf7, 0xd7]); // not r15     (MAX)
                bytes.extend([0x4d, 0x0f, 0x49, 0xcf]); // cmovns r9, r15 (positive -> MAX)
                bytes.extend([0x48, 0x39, 0xc2]); // cmp rdx, rax (high vs broadcast)
                bytes.extend([0x4d, 0x0f, 0x45, 0xd1]); // cmovne r10, r9
            }
            (ArithmeticDomain::Saturating, false) => {
                bytes.extend([0x4c, 0x89, 0xd0]); // mov rax, r10
                bytes.extend([0x49, 0xf7, 0xe3]); // mul r11     (rdx:rax)
                bytes.extend([0x49, 0x89, 0xc2]); // mov r10, rax
                bytes.push(0x49);
                bytes.push(0xbf);
                bytes.extend(u64::MAX.to_le_bytes()); // mov r15, u64::MAX
                bytes.extend([0x48, 0x85, 0xd2]); // test rdx, rdx
                bytes.extend([0x4d, 0x0f, 0x45, 0xd7]); // cmovne r10, r15
            }
            (ArithmeticDomain::Trapping, true) => {
                bytes.extend([0x4c, 0x89, 0xd0]); // mov rax, r10
                bytes.extend([0x49, 0xf7, 0xeb]); // imul r11
                bytes.extend([0x49, 0x89, 0xc2]); // mov r10, rax
                bytes.extend([0x48, 0xc1, 0xf8, 0x3f]); // sar rax, 63
                bytes.extend([0x48, 0x39, 0xc2]); // cmp rdx, rax
                bytes.extend([0x74, 0x02]); // je +2 (skip the trap)
                bytes.extend([0x0f, 0x0b]); // ud2
            }
            (ArithmeticDomain::Trapping, false) => {
                bytes.extend([0x4c, 0x89, 0xd0]); // mov rax, r10
                bytes.extend([0x49, 0xf7, 0xe3]); // mul r11
                bytes.extend([0x49, 0x89, 0xc2]); // mov r10, rax
                bytes.extend([0x48, 0x85, 0xd2]); // test rdx, rdx
                bytes.extend([0x74, 0x02]); // jz +2 (skip the trap)
                bytes.extend([0x0f, 0x0b]); // ud2
            }
            _ => unreachable!("only Saturating/Trapping reach this helper"),
        }
        return Ok(());
    }
    if !matches!(byte_size, 1 | 2 | 4) {
        return Err(Diagnostic::error(format!(
            "saturating/trapping multiply cannot handle {byte_size}-byte targets yet"
        )));
    }
    // The 64-bit `imul` needs full-width-correct operands. Narrow STORAGE
    // operands are loaded ZERO-extended, so a SIGNED negative value (e.g. i8
    // -50 -> 0xCE = 206) would multiply wrong: sign-extend them from the
    // target width. IMMEDIATE operands are already their true wide value --
    // re-extending one corrupts wide literals (the MIN-idiom fix), so each
    // side skips when immediate.
    if target_signed {
        if !left_is_wide_immediate {
            bytes.extend(match byte_size {
                1 => &[0x4d, 0x0f, 0xbe, 0xd2][..], // movsx r10, r10b
                2 => &[0x4d, 0x0f, 0xbf, 0xd2][..], // movsx r10, r10w
                _ => &[0x4d, 0x63, 0xd2][..],       // movsxd r10, r10d
            });
        }
        if !right_is_wide_immediate {
            bytes.extend(match byte_size {
                1 => &[0x4d, 0x0f, 0xbe, 0xdb][..], // movsx r11, r11b
                2 => &[0x4d, 0x0f, 0xbf, 0xdb][..], // movsx r11, r11w
                _ => &[0x4d, 0x63, 0xdb][..],       // movsxd r11, r11d
            });
        }
    }
    bytes.extend([0x4d, 0x0f, 0xaf, 0xd3]); // imul r10, r11 (64-bit)
    let unsigned_max: u64 = (1u64 << (8 * byte_size)) - 1;
    let signed_min = (-(1i128 << (8 * byte_size - 1))) as i64 as u64;
    let signed_max = ((1i128 << (8 * byte_size - 1)) - 1) as u64;
    append_narrow_range_clamp_or_trap(
        bytes,
        domain,
        StateGuardOperator::Multiply,
        byte_size,
        target_signed,
    );
    Ok(())
}

/// Saturating/Trapping ADD/SUB (decision 17). 64-bit targets keep the
/// FLAG-driven clamp (adds/subs carry/overflow at the full width is the only
/// exact witness there). Narrow targets wide-compute like multiply/shl:
/// sign-extend SIGNED NON-IMMEDIATE operands (an immediate is already its
/// true wide value -- re-extending it from the target width corrupts wide
/// literals, the MIN-idiom fix), one exact 64-bit add/sub, then the shared
/// range tail. This replaces the old width-correct-flags narrow path, which
/// could not hold a wide immediate at all (r11's low byte of 128 is -128).
fn append_saturating_trapping_add_sub(
    bytes: &mut Vec<u8>,
    domain: ArithmeticDomain,
    operator: StateGuardOperator,
    byte_size: usize,
    target_signed: bool,
    left_is_wide_immediate: bool,
    right_is_wide_immediate: bool,
) -> Result<(), Diagnostic> {
    if byte_size == 8 {
        append_width_integer_add_sub(bytes, operator, 8)?;
        append_arithmetic_domain_clamp(bytes, domain, operator, 8, target_signed)?;
        return Ok(());
    }
    if !matches!(byte_size, 1 | 2 | 4) {
        return Err(Diagnostic::error(format!(
            "saturating/trapping add/sub cannot handle {byte_size}-byte targets yet"
        )));
    }
    if target_signed {
        if !left_is_wide_immediate {
            bytes.extend(match byte_size {
                1 => &[0x4d, 0x0f, 0xbe, 0xd2][..], // movsx r10, r10b
                2 => &[0x4d, 0x0f, 0xbf, 0xd2][..], // movsx r10, r10w
                _ => &[0x4d, 0x63, 0xd2][..],       // movsxd r10, r10d
            });
        }
        if !right_is_wide_immediate {
            bytes.extend(match byte_size {
                1 => &[0x4d, 0x0f, 0xbe, 0xdb][..], // movsx r11, r11b
                2 => &[0x4d, 0x0f, 0xbf, 0xdb][..], // movsx r11, r11w
                _ => &[0x4d, 0x63, 0xdb][..],       // movsxd r11, r11d
            });
        }
    }
    append_runtime_binary_operation(bytes, operator, 8)?; // exact 64-bit add/sub
    append_narrow_range_clamp_or_trap(bytes, domain, operator, byte_size, target_signed);
    Ok(())
}

/// Bytes of [`append_saturating_trapping_add_sub`]. MUST stay in lockstep.
fn saturating_trapping_add_sub_width(
    domain: ArithmeticDomain,
    operator: StateGuardOperator,
    byte_size: usize,
    target_signed: bool,
    left_is_wide_immediate: bool,
    right_is_wide_immediate: bool,
) -> usize {
    if byte_size == 8 {
        return width_integer_add_sub_width(8)
            + arithmetic_domain_clamp_width(domain, operator, 8, target_signed);
    }
    let extend_one = |skip: bool| {
        if !target_signed || skip {
            0
        } else if byte_size == 4 {
            3
        } else {
            4
        }
    };
    extend_one(left_is_wide_immediate)
        + extend_one(right_is_wide_immediate)
        + 3 // 64-bit add/sub
        + narrow_range_clamp_or_trap_width(domain, target_signed)
}

/// The narrow (<= 4-byte) exact-wide-value range tail shared by the
/// saturating/trapping MULTIPLY and SHIFT-LEFT: the 64-bit op computed the
/// exact result in r10, so compare it against the target range and clamp
/// (cmov) or trap (ud2). r11 is spent by then and serves as the bound
/// scratch. The unsigned arms take a SINGLE UNSIGNED upper compare -- a u32
/// product or shift can exceed 2^63 (signed reading negative), and unsigned
/// results cannot go below zero.
fn append_narrow_range_clamp_or_trap(
    bytes: &mut Vec<u8>,
    domain: ArithmeticDomain,
    operator: StateGuardOperator,
    byte_size: usize,
    target_signed: bool,
) {
    let unsigned_max: u64 = (1u64 << (8 * byte_size)) - 1;
    let signed_min = (-(1i128 << (8 * byte_size - 1))) as i64 as u64;
    let signed_max = ((1i128 << (8 * byte_size - 1)) - 1) as u64;
    fn mov_r11(bytes: &mut Vec<u8>, value: u64) {
        bytes.push(0x49);
        bytes.push(0xbb);
        bytes.extend(value.to_le_bytes());
    }
    match (domain, target_signed) {
        (ArithmeticDomain::Saturating, false) => {
            // Unsigned wide results overflow in ONE direction per operator
            // (the aarch64 tail's rule): subtract only DOWNWARD -- the
            // wrapped wide underflow reads signed-negative, so clamp to 0
            // with a SIGNED compare -- add/mul/shl only UPWARD, where the
            // compare must be UNSIGNED (a 2^63+ product reads negative).
            if operator == StateGuardOperator::Subtract {
                mov_r11(bytes, 0);
                bytes.extend([0x4d, 0x39, 0xda]); // cmp r10, r11
                bytes.extend([0x4d, 0x0f, 0x4c, 0xd3]); // cmovl r10, r11 (<s 0 -> 0)
            } else {
                mov_r11(bytes, unsigned_max);
                bytes.extend([0x4d, 0x39, 0xda]); // cmp r10, r11
                bytes.extend([0x4d, 0x0f, 0x47, 0xd3]); // cmova r10, r11 (r10 >u max -> max)
            }
        }
        (ArithmeticDomain::Saturating, true) => {
            mov_r11(bytes, signed_max);
            bytes.extend([0x4d, 0x39, 0xda]); // cmp r10, r11
            bytes.extend([0x4d, 0x0f, 0x4f, 0xd3]); // cmovg r10, r11 (> imax -> imax)
            mov_r11(bytes, signed_min);
            bytes.extend([0x4d, 0x39, 0xda]); // cmp r10, r11
            bytes.extend([0x4d, 0x0f, 0x4c, 0xd3]); // cmovl r10, r11 (< imin -> imin)
        }
        (ArithmeticDomain::Trapping, false) => {
            if operator == StateGuardOperator::Subtract {
                mov_r11(bytes, 0);
                bytes.extend([0x4d, 0x39, 0xda]); // cmp r10, r11
                bytes.extend([0x7d, 0x02]); // jge +2 (>=s 0: ok)
                bytes.extend([0x0f, 0x0b]); // ud2
            } else {
                mov_r11(bytes, unsigned_max);
                bytes.extend([0x4d, 0x39, 0xda]); // cmp r10, r11
                bytes.extend([0x76, 0x02]); // jbe +2 (<= max: ok)
                bytes.extend([0x0f, 0x0b]); // ud2
            }
        }
        (ArithmeticDomain::Trapping, true) => {
            mov_r11(bytes, signed_max);
            bytes.extend([0x4d, 0x39, 0xda]); // cmp r10, r11
            bytes.extend([0x7f, 0x0f]); // jg +15 -> ud2 (skip mov+cmp+jge)
            mov_r11(bytes, signed_min);
            bytes.extend([0x4d, 0x39, 0xda]); // cmp r10, r11
            bytes.extend([0x7d, 0x02]); // jge +2 (>= imin: ok)
            bytes.extend([0x0f, 0x0b]); // ud2
        }
        _ => {}
    }
}

/// Bytes of [`append_narrow_range_clamp_or_trap`]. MUST stay in lockstep.
/// (The unsigned direction split is width-neutral: one bound either way --
/// mov 10 + cmp 3 + cmov/jcc+ud2 4 -- so no operator parameter here.)
fn narrow_range_clamp_or_trap_width(domain: ArithmeticDomain, target_signed: bool) -> usize {
    match (domain, target_signed) {
        // mov r11,imm64 (10) + cmp (3) + cmova (4)
        (ArithmeticDomain::Saturating, false) => 17,
        // (mov + cmp + cmovg) + (mov + cmp + cmovl)
        (ArithmeticDomain::Saturating, true) => 34,
        // mov (10) + cmp (3) + jbe (2) + ud2 (2)
        (ArithmeticDomain::Trapping, false) => 17,
        // mov (10) + cmp (3) + jg (2) + mov (10) + cmp (3) + jge (2) + ud2 (2)
        (ArithmeticDomain::Trapping, true) => 32,
        _ => 0,
    }
}

/// Saturating/Trapping `<<` (shift slice C): the TRUE value is x * 2^n, so
/// clamp/trap when it leaves the target range. Narrow widths cap the COUNT
/// at the type width w -- any count >= w overflows every nonzero x, and the
/// cap keeps the 64-bit shl EXACT -- then take the shared range tail; only
/// the VALUE sign-extends (the count reads unsigned, so a negative signed
/// count is huge and caps to w, matching the interpreter). 64-bit uses the
/// RECOVERY witness (y >> n == x, arithmetic/logical by signedness) with
/// explicit checks for the two cases the hardware count mask hides: a count
/// >= 64 overflows every nonzero x, and x == 0 never overflows. Mirrors the
/// aarch64 sequences; r9/rax/rcx/r15 are scratch as in the multiply arms.
fn append_saturating_trapping_shift_left(
    bytes: &mut Vec<u8>,
    domain: ArithmeticDomain,
    byte_size: usize,
    target_signed: bool,
) -> Result<(), Diagnostic> {
    // F8c: a TRAPPING out-of-range COUNT traps before the value math (the
    // count is invalid, not the result -- `0 << 40` traps). Saturating
    // cannot reach one post-F8a; its count cap below stays for robustness.
    if domain == ArithmeticDomain::Trapping {
        append_shift_count_trap_guard(bytes, byte_size);
    }
    if byte_size == 8 {
        let fixup: u8 = match (domain, target_signed) {
            // mov r15,MIN (10) + test r9 (3) + mov r10,r15 (3) + not r15 (3)
            // + cmovns r10,r15 (4).
            (ArithmeticDomain::Saturating, true) => 23,
            // mov r10, u64::MAX (10).
            (ArithmeticDomain::Saturating, false) => 10,
            // ud2.
            _ => 2,
        };
        bytes.extend([0x4d, 0x89, 0xd1]); // mov r9, r10 (save x)
        append_runtime_binary_operation(bytes, StateGuardOperator::ShiftLeft, 8)?;
        bytes.extend([0x4c, 0x89, 0xd0]); // mov rax, r10 (y)
        bytes.extend(if target_signed {
            [0x48, 0xd3, 0xf8] // sar rax, cl (count still in cl)
        } else {
            [0x48, 0xd3, 0xe8] // shr rax, cl
        });
        bytes.extend([0x4c, 0x39, 0xc8]); // cmp rax, r9 (recovery == x ?)
        bytes.extend([0x75, 11]); // jne -> fixup
        bytes.extend([0x49, 0x83, 0xfb, 64]); // cmp r11, 64
        bytes.extend([0x72, 5 + fixup]); // jb -> keep (in-range count)
        bytes.extend([0x4d, 0x85, 0xc9]); // test r9, r9
        bytes.extend([0x74, fixup]); // je -> keep (x == 0)
        match (domain, target_signed) {
            (ArithmeticDomain::Saturating, true) => {
                bytes.push(0x49);
                bytes.push(0xbf);
                bytes.extend((i64::MIN as u64).to_le_bytes()); // mov r15, MIN
                bytes.extend([0x4d, 0x85, 0xc9]); // test r9, r9 (x's sign)
                bytes.extend([0x4d, 0x89, 0xfa]); // mov r10, r15 (MIN)
                bytes.extend([0x49, 0xf7, 0xd7]); // not r15 (MAX)
                bytes.extend([0x4d, 0x0f, 0x49, 0xd7]); // cmovns r10, r15 (x >= 0 -> MAX)
            }
            (ArithmeticDomain::Saturating, false) => {
                bytes.push(0x49);
                bytes.push(0xba);
                bytes.extend(u64::MAX.to_le_bytes()); // mov r10, u64::MAX
            }
            _ => bytes.extend([0x0f, 0x0b]), // ud2
        }
        return Ok(());
    }
    if !matches!(byte_size, 1 | 2 | 4) {
        return Err(Diagnostic::error(format!(
            "saturating/trapping shift-left cannot handle {byte_size}-byte targets yet"
        )));
    }
    if target_signed {
        match byte_size {
            1 => bytes.extend([0x4d, 0x0f, 0xbe, 0xd2]), // movsx r10, r10b
            2 => bytes.extend([0x4d, 0x0f, 0xbf, 0xd2]), // movsx r10, r10w
            _ => bytes.extend([0x4d, 0x63, 0xd2]),       // movsxd r10, r10d
        }
    }
    let width_bits = (byte_size * 8) as u8;
    bytes.push(0xb8); // mov eax, imm32 (= w)
    bytes.extend(u32::from(width_bits).to_le_bytes());
    bytes.extend([0x49, 0x83, 0xfb, width_bits]); // cmp r11, w
    bytes.extend([0x4c, 0x0f, 0x43, 0xd8]); // cmovae r11, rax (cap count at w)
    append_runtime_binary_operation(bytes, StateGuardOperator::ShiftLeft, 8)?; // exact 64-bit shl
    append_narrow_range_clamp_or_trap(
        bytes,
        domain,
        StateGuardOperator::ShiftLeft,
        byte_size,
        target_signed,
    );
    Ok(())
}

/// Bytes of [`append_saturating_trapping_shift_left`]. MUST stay in lockstep.
fn saturating_trapping_shift_left_width(
    domain: ArithmeticDomain,
    byte_size: usize,
    target_signed: bool,
) -> usize {
    // F8c: Trapping prepends the count trap guard (cmp + jb + ud2 = 8).
    let count_guard = if domain == ArithmeticDomain::Trapping {
        SHIFT_COUNT_TRAP_GUARD_WIDTH
    } else {
        0
    };
    if byte_size == 8 {
        // save (3) + shl op (6) + mov rax (3) + sar/shr (3) + cmp (3)
        // + jne (2) + cmp #64 (4) + jb (2) + test (3) + je (2) = 31 + fixup.
        return count_guard
            + 31
            + match (domain, target_signed) {
                (ArithmeticDomain::Saturating, true) => 23,
                (ArithmeticDomain::Saturating, false) => 10,
                _ => 2,
            };
    }
    if !matches!(byte_size, 1 | 2 | 4) {
        return 6; // emission errors; placeholder for the pre-error capacity
    }
    // movsx (4) / movsxd (3) for signed values only, + the count cap
    // (mov eax 5 + cmp 4 + cmovae 4 = 13) + the 64-bit shl (6) + the tail.
    let sign_extend = if target_signed {
        if byte_size == 4 { 3 } else { 4 }
    } else {
        0
    };
    count_guard + sign_extend + 13 + 6 + narrow_range_clamp_or_trap_width(domain, target_signed)
}

/// Width-correct integer `add`/`sub` of `r10 (op)= r11` so the carry/overflow
/// flags reflect the TARGET byte width (the default binary op is always 64-bit
/// and relies on the truncating store). Only `+`/`-` are supported for the
/// saturating/trapping domains today; other operators error.
fn append_width_integer_add_sub(
    bytes: &mut Vec<u8>,
    operator: StateGuardOperator,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    // ADD r/m,r = 0x00 (8-bit) / 0x01 (wider); SUB = 0x28 / 0x29. ModRM 0xDA is
    // (r/m = r10, reg = r11); the REX prefix selects the width and extends both.
    let (op8, opw) = match operator {
        StateGuardOperator::Add => (0x00u8, 0x01u8),
        StateGuardOperator::Subtract => (0x28u8, 0x29u8),
        _ => {
            return Err(Diagnostic::error(
                "saturating/trapping arithmetic is only implemented for + and - so far".to_owned(),
            ));
        }
    };
    match byte_size {
        1 => bytes.extend([0x45, op8, 0xda]),
        2 => bytes.extend([0x66, 0x45, opw, 0xda]),
        4 => bytes.extend([0x45, opw, 0xda]),
        8 => bytes.extend([0x4d, opw, 0xda]),
        _ => {
            return Err(Diagnostic::error(format!(
                "saturating/trapping arithmetic cannot handle {byte_size}-byte targets yet"
            )));
        }
    }
    Ok(())
}

/// Bytes of [`append_arithmetic_domain_clamp`], for the relocation layout. MUST
/// equal what that function emits.
fn arithmetic_domain_clamp_width(
    domain: ArithmeticDomain,
    _operator: StateGuardOperator,
    _byte_size: usize,
    target_signed: bool,
) -> usize {
    match domain {
        ArithmeticDomain::Exact | ArithmeticDomain::Wrapping => 0,
        // jno/jnc rel8 (2) + ud2 (2)
        ArithmeticDomain::Trapping => 4,
        ArithmeticDomain::Saturating => {
            if target_signed {
                // mov r11,imm64 (10) + mov r9,imm64 (10) + cmovs r11,r9 (4) + cmovo r10,r11 (4)
                28
            } else {
                // mov r11,imm64 (10) + cmovc r10,r11 (4)
                14
            }
        }
    }
}

/// Clamp (Saturating) or trap (Trapping) the width-correct op's result in r10,
/// reading the flags it set. Unsigned overflow is the carry flag (add: clamp to
/// the unsigned max; sub: clamp to 0); signed overflow is the overflow flag
/// (clamp to the signed min/max, chosen by the result's sign bit). r11 (the
/// spent right operand) and r9 are used as scratch.
fn append_arithmetic_domain_clamp(
    bytes: &mut Vec<u8>,
    domain: ArithmeticDomain,
    operator: StateGuardOperator,
    byte_size: usize,
    target_signed: bool,
) -> Result<(), Diagnostic> {
    match domain {
        ArithmeticDomain::Exact | ArithmeticDomain::Wrapping => {}
        ArithmeticDomain::Trapping => {
            // Skip the 2-byte ud2 when there was NO overflow: unsigned watches the
            // carry flag (jnc/jae), signed watches the overflow flag (jno).
            let skip_when_ok = if target_signed { 0x71u8 } else { 0x73u8 };
            bytes.extend([skip_when_ok, 0x02, 0x0f, 0x0b]);
        }
        ArithmeticDomain::Saturating if target_signed => {
            let imin = (-(1i128 << (8 * byte_size - 1))) as i64 as u64;
            let imax = ((1i128 << (8 * byte_size - 1)) - 1) as u64;
            bytes.push(0x49);
            bytes.push(0xbb);
            bytes.extend(imin.to_le_bytes()); // mov r11, IMIN
            bytes.push(0x49);
            bytes.push(0xb9);
            bytes.extend(imax.to_le_bytes()); // mov r9, IMAX
            // On signed overflow the stored result's sign is inverted, so a
            // negative result means the true value overflowed POSITIVE -> IMAX.
            bytes.extend([0x4d, 0x0f, 0x48, 0xd9]); // cmovs r11, r9
            bytes.extend([0x4d, 0x0f, 0x40, 0xd3]); // cmovo r10, r11
        }
        ArithmeticDomain::Saturating => {
            let clamp_value: u64 = match operator {
                StateGuardOperator::Add => {
                    if byte_size >= 8 {
                        u64::MAX
                    } else {
                        (1u64 << (8 * byte_size)) - 1
                    }
                }
                StateGuardOperator::Subtract => 0,
                _ => {
                    return Err(Diagnostic::error(
                        "saturating arithmetic is only implemented for + and - so far".to_owned(),
                    ));
                }
            };
            bytes.push(0x49);
            bytes.push(0xbb);
            bytes.extend(clamp_value.to_le_bytes()); // mov r11, clamp
            bytes.extend([0x4d, 0x0f, 0x42, 0xd3]); // cmovc r10, r11
        }
    }
    Ok(())
}

/// Bytes of the in-register conversion step for a numeric `as` cast (the source
/// bits are already in r10; the result is left in r10 for the store).
fn runtime_convert_operation_width(
    source_byte_size: usize,
    target_byte_size: usize,
    source_is_float: bool,
    target_is_float: bool,
    source_signed: bool,
    target_signed: bool,
    trapping: bool,
    saturating: bool,
) -> usize {
    match (source_is_float, target_is_float) {
        // movq/movd xmm0,r10 (5), then either the bare conversion (Exact) or
        // an x86 policy fixup around cvttsd2si/cvttss2si. Unlike aarch64,
        // x86 returns one ambiguous integer-indefinite value for NaN and every
        // overflow, so Saturating and Trapping must classify the FP value.
        (true, false) => {
            5 + if trapping {
                float_to_int_trap_width(source_byte_size, target_byte_size, target_signed)
            } else if saturating {
                float_to_int_saturating_width(source_byte_size, target_byte_size, target_signed)
            } else {
                float_to_int_convert_width(source_byte_size, target_byte_size, target_signed)
            }
        }
        // cvtsi2sd/ss xmm0,r10 (5) + movq/movd r10,xmm0 (5)
        (false, true) => 10,
        (true, true) => {
            if source_byte_size == target_byte_size {
                0 // f64->f64: bits already in r10
            } else {
                14 // movq/movd (5) + cvtsd2ss/cvtss2sd (4) + movd/movq (5)
            }
        }
        (false, false) => {
            // Widen a narrow integer source into r10. A 1/2-byte source was loaded
            // with movb/movw, which leave the upper bits GARBAGE, so it MUST be
            // movzx/movsx-extended (zero for unsigned, sign for signed). A 4-byte
            // source was loaded with movl (already zero-extended), so only a SIGNED
            // 4-byte source needs movsxd; an unsigned 4-byte source is already
            // correct. Narrowing/equal widths need nothing (the store truncates).
            if target_byte_size > source_byte_size {
                match source_byte_size {
                    1 | 2 => 4,              // movzx/movsx r10, r10b / r10w
                    4 if source_signed => 3, // movsxd r10, r10d
                    _ => 0,
                }
            } else {
                0
            }
        }
    }
}

/// Append the in-register conversion (see [`runtime_convert_operation_width`]).
fn append_runtime_convert_operation(
    bytes: &mut Vec<u8>,
    source_byte_size: usize,
    target_byte_size: usize,
    source_is_float: bool,
    target_is_float: bool,
    source_signed: bool,
    target_signed: bool,
    trapping: bool,
    saturating: bool,
) {
    match (source_is_float, target_is_float) {
        (true, false) => {
            // float -> int: move bits into xmm0, truncating-convert to r10.
            if source_byte_size > 4 {
                bytes.extend([0x66, 0x49, 0x0f, 0x6e, 0xc2]); // movq xmm0, r10
            } else {
                bytes.extend([0x66, 0x41, 0x0f, 0x6e, 0xc2]); // movd xmm0, r10d
            }
            if trapping {
                append_float_to_int_trap(bytes, source_byte_size, target_byte_size, target_signed);
            } else if saturating {
                append_float_to_int_saturating(
                    bytes,
                    source_byte_size,
                    target_byte_size,
                    target_signed,
                );
            } else {
                append_float_to_int_convert(
                    bytes,
                    source_byte_size,
                    target_byte_size,
                    target_signed,
                );
            }
        }
        (false, true) => {
            // int -> float: convert r10 (signed) into xmm0, move bits back to r10.
            if source_byte_size > 4 {
                if target_byte_size > 4 {
                    bytes.extend([0xf2, 0x49, 0x0f, 0x2a, 0xc2]); // cvtsi2sd xmm0, r10
                } else {
                    bytes.extend([0xf3, 0x49, 0x0f, 0x2a, 0xc2]); // cvtsi2ss xmm0, r10
                }
            } else if target_byte_size > 4 {
                bytes.extend([0xf2, 0x41, 0x0f, 0x2a, 0xc2]); // cvtsi2sd xmm0, r10d
            } else {
                bytes.extend([0xf3, 0x41, 0x0f, 0x2a, 0xc2]); // cvtsi2ss xmm0, r10d
            }
            if target_byte_size > 4 {
                bytes.extend([0x66, 0x49, 0x0f, 0x7e, 0xc2]); // movq r10, xmm0
            } else {
                bytes.extend([0x66, 0x41, 0x0f, 0x7e, 0xc2]); // movd r10d, xmm0
            }
        }
        (true, true) => {
            if source_byte_size == target_byte_size {
                // f64 -> f64: nothing to do.
            } else if source_byte_size > target_byte_size {
                bytes.extend([0x66, 0x49, 0x0f, 0x6e, 0xc2]); // movq xmm0, r10
                bytes.extend([0xf2, 0x0f, 0x5a, 0xc0]); // cvtsd2ss xmm0, xmm0
                bytes.extend([0x66, 0x41, 0x0f, 0x7e, 0xc2]); // movd r10d, xmm0
            } else {
                bytes.extend([0x66, 0x41, 0x0f, 0x6e, 0xc2]); // movd xmm0, r10d
                bytes.extend([0xf3, 0x0f, 0x5a, 0xc0]); // cvtss2sd xmm0, xmm0
                bytes.extend([0x66, 0x49, 0x0f, 0x7e, 0xc2]); // movq r10, xmm0
            }
        }
        (false, false) => {
            if target_byte_size > source_byte_size {
                match (source_byte_size, source_signed) {
                    // movb/movw left the upper bits garbage: extend r10b/r10w -> r10.
                    (1, true) => bytes.extend([0x4d, 0x0f, 0xbe, 0xd2]), // movsx r10, r10b
                    (1, false) => bytes.extend([0x4d, 0x0f, 0xb6, 0xd2]), // movzx r10, r10b
                    (2, true) => bytes.extend([0x4d, 0x0f, 0xbf, 0xd2]), // movsx r10, r10w
                    (2, false) => bytes.extend([0x4d, 0x0f, 0xb7, 0xd2]), // movzx r10, r10w
                    (4, true) => bytes.extend([0x4d, 0x63, 0xd2]),       // movsxd r10, r10d
                    // 4-byte unsigned (and 8-byte) sources were already zero-extended
                    // by the movl/movq load.
                    _ => {}
                }
            }
        }
    }
}

fn float_compare_xmm0_width(source_byte_size: usize) -> usize {
    if source_byte_size > 4 { 4 } else { 3 }
}

fn append_float_compare_xmm0(bytes: &mut Vec<u8>, source_byte_size: usize, rhs: u8) {
    let modrm = 0xc0 | rhs;
    if source_byte_size > 4 {
        bytes.extend([0x66, 0x0f, 0x2e, modrm]); // ucomisd xmm0, xmm{rhs}
    } else {
        bytes.extend([0x0f, 0x2e, modrm]); // ucomiss xmm0, xmm{rhs}
    }
}

fn append_float_bound_xmm1(bytes: &mut Vec<u8>, source_byte_size: usize, bits: u64) {
    append_mov_reg_imm64(bytes, Reg64::R11, bits);
    if source_byte_size > 4 {
        bytes.extend([0x66, 0x49, 0x0f, 0x6e, 0xcb]); // movq xmm1, r11
    } else {
        bytes.extend([0x66, 0x41, 0x0f, 0x6e, 0xcb]); // movd xmm1, r11d
    }
}

fn append_signed_float_to_int_convert(bytes: &mut Vec<u8>, source_byte_size: usize) {
    if source_byte_size > 4 {
        bytes.extend([0xf2, 0x4c, 0x0f, 0x2c, 0xd0]); // cvttsd2si r10, xmm0
    } else {
        bytes.extend([0xf3, 0x4c, 0x0f, 0x2c, 0xd0]); // cvttss2si r10, xmm0
    }
}

/// Bounds for truncating a source float into a signed target. `upper` is
/// exclusive. `lower` is either the exclusive `MIN - 1` threshold (when the
/// source format can represent it) or the inclusive `MIN` threshold.
fn float_to_int_bounds(
    source_byte_size: usize,
    target_byte_size: usize,
    target_signed: bool,
) -> (u64, u64, bool) {
    let target_bits = (target_byte_size * 8) as i32;
    let upper = 2.0_f64.powi(target_bits - i32::from(target_signed));
    if !target_signed {
        return if source_byte_size > 4 {
            (upper.to_bits(), (-1.0_f64).to_bits(), false)
        } else {
            (
                u64::from((upper as f32).to_bits()),
                u64::from((-1.0_f32).to_bits()),
                false,
            )
        };
    }
    let minimum = -upper;
    if source_byte_size > 4 {
        let lower_candidate = minimum - 1.0;
        let lower_inclusive = lower_candidate == minimum;
        (
            upper.to_bits(),
            (if lower_inclusive {
                minimum
            } else {
                lower_candidate
            })
            .to_bits(),
            lower_inclusive,
        )
    } else {
        let minimum = minimum as f32;
        let lower_candidate = minimum - 1.0;
        let lower_inclusive = lower_candidate == minimum;
        (
            u64::from((upper as f32).to_bits()),
            u64::from(
                (if lower_inclusive {
                    minimum
                } else {
                    lower_candidate
                })
                .to_bits(),
            ),
            lower_inclusive,
        )
    }
}

fn integer_clamps(target_byte_size: usize, target_signed: bool) -> (u64, u64) {
    let bits = target_byte_size * 8;
    if target_signed {
        let sign_bit = 1_u64 << (bits - 1);
        (sign_bit - 1, sign_bit)
    } else {
        (
            if bits == 64 {
                u64::MAX
            } else {
                (1_u64 << bits) - 1
            },
            0,
        )
    }
}

fn float_to_int_convert_width(
    source_byte_size: usize,
    target_byte_size: usize,
    target_signed: bool,
) -> usize {
    if target_signed || target_byte_size < 8 {
        5
    } else {
        // 2^63 materialization + compare + branch + subtract + two cvtt arms
        // + bts sign-bit reconstruction + two jumps.
        38 + float_compare_xmm0_width(source_byte_size)
    }
}

fn append_float_to_int_convert(
    bytes: &mut Vec<u8>,
    source_byte_size: usize,
    target_byte_size: usize,
    target_signed: bool,
) {
    if target_signed || target_byte_size < 8 {
        append_signed_float_to_int_convert(bytes, source_byte_size);
        return;
    }

    let split = if source_byte_size > 4 {
        (9223372036854775808.0_f64).to_bits()
    } else {
        u64::from((9223372036854775808.0_f32).to_bits())
    };
    append_float_bound_xmm1(bytes, source_byte_size, split);
    append_float_compare_xmm0(bytes, source_byte_size, 1);
    bytes.extend([0x72, 0x10]); // jb low-half
    bytes.extend([
        if source_byte_size > 4 { 0xf2 } else { 0xf3 },
        0x0f,
        0x5c,
        0xc1,
    ]); // subsd/subss xmm0, xmm1
    append_signed_float_to_int_convert(bytes, source_byte_size);
    bytes.extend([0x49, 0x0f, 0xba, 0xea, 0x3f]); // bts r10, 63
    bytes.extend([0xeb, 0x05]); // jmp done
    append_signed_float_to_int_convert(bytes, source_byte_size); // low-half
}

fn float_to_int_trap_width(
    source_byte_size: usize,
    target_byte_size: usize,
    target_signed: bool,
) -> usize {
    // Three compares + two bound materializations + four short branches +
    // cvtt + ud2. The final jump hops over the shared trap site.
    3 * float_compare_xmm0_width(source_byte_size)
        + 40
        + float_to_int_convert_width(source_byte_size, target_byte_size, target_signed)
}

fn append_float_to_int_trap(
    bytes: &mut Vec<u8>,
    source_byte_size: usize,
    target_byte_size: usize,
    target_signed: bool,
) {
    let compare_width = float_compare_xmm0_width(source_byte_size);
    let (upper, lower, lower_inclusive) =
        float_to_int_bounds(source_byte_size, target_byte_size, target_signed);
    let convert_width =
        float_to_int_convert_width(source_byte_size, target_byte_size, target_signed);

    append_float_compare_xmm0(bytes, source_byte_size, 0);
    bytes.extend([0x7a, (36 + 2 * compare_width + convert_width) as u8]); // jp trap
    append_float_bound_xmm1(bytes, source_byte_size, upper);
    append_float_compare_xmm0(bytes, source_byte_size, 1);
    bytes.extend([0x73, (19 + compare_width + convert_width) as u8]); // jae trap
    append_float_bound_xmm1(bytes, source_byte_size, lower);
    append_float_compare_xmm0(bytes, source_byte_size, 1);
    bytes.extend([
        if lower_inclusive { 0x72 } else { 0x76 },
        (convert_width + 2) as u8,
    ]); // jb/jbe trap
    append_float_to_int_convert(bytes, source_byte_size, target_byte_size, target_signed);
    bytes.extend([0xeb, 0x02]); // jmp done
    bytes.extend([0x0f, 0x0b]); // trap: ud2
}

fn float_to_int_saturating_width(
    source_byte_size: usize,
    target_byte_size: usize,
    target_signed: bool,
) -> usize {
    // Three compares + two bound materializations + policy result arms.
    3 * float_compare_xmm0_width(source_byte_size)
        + 65
        + float_to_int_convert_width(source_byte_size, target_byte_size, target_signed)
}

fn append_float_to_int_saturating(
    bytes: &mut Vec<u8>,
    source_byte_size: usize,
    target_byte_size: usize,
    target_signed: bool,
) {
    let compare_width = float_compare_xmm0_width(source_byte_size);
    let (upper, lower, lower_inclusive) =
        float_to_int_bounds(source_byte_size, target_byte_size, target_signed);
    let (maximum, minimum) = integer_clamps(target_byte_size, target_signed);
    let convert_width =
        float_to_int_convert_width(source_byte_size, target_byte_size, target_signed);

    append_float_compare_xmm0(bytes, source_byte_size, 0);
    bytes.extend([0x7a, (36 + 2 * compare_width + convert_width) as u8]); // jp nan
    append_float_bound_xmm1(bytes, source_byte_size, upper);
    append_float_compare_xmm0(bytes, source_byte_size, 1);
    bytes.extend([0x73, (24 + compare_width + convert_width) as u8]); // jae high
    append_float_bound_xmm1(bytes, source_byte_size, lower);
    append_float_compare_xmm0(bytes, source_byte_size, 1);
    bytes.extend([
        if lower_inclusive { 0x72 } else { 0x76 },
        (19 + convert_width) as u8,
    ]); // jb/jbe low
    append_float_to_int_convert(bytes, source_byte_size, target_byte_size, target_signed);
    bytes.extend([0xeb, 0x1b]); // jmp done
    bytes.extend([0x45, 0x31, 0xd2]); // nan: xor r10d, r10d
    bytes.extend([0xeb, 0x16]); // jmp done
    append_mov_reg_imm64(bytes, Reg64::R10, maximum); // high
    bytes.extend([0xeb, 0x0a]); // jmp done
    append_mov_reg_imm64(bytes, Reg64::R10, minimum); // low
}

pub fn encode_atomic_load_to_storage(
    source_offset: usize,
    byte_size: usize,
    result_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_atomic_load_to_storage_width(
        source_offset,
        byte_size,
        result_offset,
    ));
    append_mov_r14_imm64(&mut bytes, 0);
    append_load_reg_from_r14(&mut bytes, Reg64::R10, source_offset, byte_size)?;
    append_mov_r14_imm64(&mut bytes, 0);
    append_store_r10_to_r14(&mut bytes, result_offset, byte_size)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_atomic_load_to_storage_width(source_offset, byte_size, result_offset)
    );
    Ok(bytes)
}

pub fn runtime_atomic_load_to_storage_width(
    _source_offset: usize,
    byte_size: usize,
    _result_offset: usize,
) -> usize {
    10 + load_width(byte_size) + 10 + store_width(byte_size)
}

pub fn runtime_atomic_load_result_address_offset(byte_size: usize) -> usize {
    10 + load_width(byte_size)
}

pub fn encode_atomic_store_from_operand(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    value: RuntimeValueOperandHandle,
    seq_cst: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_atomic_store_from_operand_width(
        runtime_value_operands,
        byte_size,
        value,
    ));
    append_mov_r14_imm64(&mut bytes, 0);
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R10, value)?;
    if seq_cst {
        append_xchg_r10_to_r14(&mut bytes, target_offset, byte_size)?;
    } else {
        append_store_r10_to_r14(&mut bytes, target_offset, byte_size)?;
    }
    debug_assert_eq!(
        bytes.len(),
        runtime_atomic_store_from_operand_width(runtime_value_operands, byte_size, value)
    );
    Ok(bytes)
}

pub fn runtime_atomic_store_from_operand_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    value: RuntimeValueOperandHandle,
) -> usize {
    10 + runtime_value_operand_width(runtime_value_operands, value) + store_width(byte_size)
}

pub fn runtime_atomic_fetch_add_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    _result_offset: usize,
    delta: RuntimeValueOperandHandle,
) -> usize {
    // mov r14,imm64(target base) (10) + delta operand load into r10 + lock xadd.
    10 + runtime_value_operand_width(runtime_value_operands, delta)
        + lock_xadd_r10_to_r14_width(byte_size)
        + 10
        + store_width(byte_size)
}

pub fn runtime_atomic_fetch_add_result_address_offset(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    delta: RuntimeValueOperandHandle,
) -> usize {
    10 + runtime_value_operand_width(runtime_value_operands, delta)
        + lock_xadd_r10_to_r14_width(byte_size)
}

/// Atomic `fetch_add`: hold the target base in r14 (untouched by operand
/// evaluation, which reloads r15), evaluate `delta` into r10, then `lock xadd
/// [r14+offset], r10` -- one atomic read-modify-write of the place. XADD leaves
/// the instruction-observed prior in r10; the encoder stores that exact value
/// into the result place before returning.
pub fn encode_atomic_fetch_add(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    delta: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_atomic_fetch_add_width(
        runtime_value_operands,
        byte_size,
        result_offset,
        delta,
    ));
    append_mov_r14_imm64(&mut bytes, 0); // target base (imm64 @ +2 relocated)
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R10, delta)?;
    append_lock_xadd_r10_to_r14(&mut bytes, target_offset, byte_size)?;
    append_mov_r14_imm64(&mut bytes, 0); // result base (relocated independently)
    append_store_r10_to_r14(&mut bytes, result_offset, byte_size)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_atomic_fetch_add_width(runtime_value_operands, byte_size, result_offset, delta)
    );
    Ok(bytes)
}

pub fn runtime_atomic_fetch_sub_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    _result_offset: usize,
    delta: RuntimeValueOperandHandle,
) -> usize {
    runtime_atomic_fetch_add_width(runtime_value_operands, byte_size, 0, delta)
        + negate_r10_width(byte_size)
}

pub fn runtime_atomic_fetch_sub_result_address_offset(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    delta: RuntimeValueOperandHandle,
) -> usize {
    runtime_atomic_fetch_add_result_address_offset(runtime_value_operands, byte_size, delta)
        + negate_r10_width(byte_size)
}

pub fn encode_atomic_fetch_sub(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    delta: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_atomic_fetch_sub_width(
        runtime_value_operands,
        byte_size,
        result_offset,
        delta,
    ));
    append_mov_r14_imm64(&mut bytes, 0);
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R10, delta)?;
    append_negate_r10(&mut bytes, byte_size)?;
    append_lock_xadd_r10_to_r14(&mut bytes, target_offset, byte_size)?;
    append_mov_r14_imm64(&mut bytes, 0);
    append_store_r10_to_r14(&mut bytes, result_offset, byte_size)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_atomic_fetch_sub_width(runtime_value_operands, byte_size, result_offset, delta)
    );
    Ok(bytes)
}

fn runtime_atomic_fetch_bitwise_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    _result_offset: usize,
    value: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
) -> usize {
    10 + runtime_value_operand_width(runtime_value_operands, value)
        + 3 // mov r11, r10: preserve bitwise operand
        + load_rax_from_r14_width(byte_size)
        + 3 // loop: mov r10, rax
        + runtime_binary_operation_width(operator, byte_size)
        + lock_cmpxchg_r10_to_r14_width(byte_size)
        + 2 // jne rel8 back to retry
        + 3 // mov r10, rax: instruction-observed prior
        + 10 // mov r14, result base
        + store_width(byte_size)
}

fn runtime_atomic_fetch_bitwise_result_address_offset(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    value: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
) -> usize {
    runtime_atomic_fetch_bitwise_width(runtime_value_operands, byte_size, 0, value, operator)
        - 10
        - store_width(byte_size)
}

fn encode_atomic_fetch_bitwise(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    value: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    operation_name: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_atomic_fetch_bitwise_width(
        runtime_value_operands,
        byte_size,
        result_offset,
        value,
        operator,
    ));
    append_mov_r14_imm64(&mut bytes, 0);
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R10, value)?;
    append_mov_reg_reg(&mut bytes, Reg64::R11, Reg64::R10);
    append_load_rax_from_r14(&mut bytes, target_offset, byte_size)?;
    let retry_offset = bytes.len();
    bytes.extend([0x49, 0x89, 0xc2]); // mov r10, rax
    append_runtime_binary_operation(&mut bytes, operator, byte_size)?;
    append_lock_cmpxchg_r10_to_r14(&mut bytes, target_offset, byte_size)?;
    let branch_end = bytes.len() + 2;
    let retry_distance = isize::try_from(retry_offset).unwrap_or(isize::MAX)
        - isize::try_from(branch_end).unwrap_or(isize::MIN);
    let retry_distance = i8::try_from(retry_distance).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 atomic {operation_name} retry loop exceeds rel8 reach"
        ))
    })?;
    bytes.extend([0x75, retry_distance as u8]); // jne retry
    bytes.extend([0x49, 0x89, 0xc2]); // mov r10, rax (prior)
    append_mov_r14_imm64(&mut bytes, 0);
    append_store_r10_to_r14(&mut bytes, result_offset, byte_size)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_atomic_fetch_bitwise_width(
            runtime_value_operands,
            byte_size,
            result_offset,
            value,
            operator
        )
    );
    Ok(bytes)
}

pub fn runtime_atomic_fetch_xor_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    result_offset: usize,
    value: RuntimeValueOperandHandle,
) -> usize {
    runtime_atomic_fetch_bitwise_width(
        runtime_value_operands,
        byte_size,
        result_offset,
        value,
        StateGuardOperator::BitwiseXor,
    )
}

pub fn runtime_atomic_fetch_xor_result_address_offset(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    value: RuntimeValueOperandHandle,
) -> usize {
    runtime_atomic_fetch_bitwise_result_address_offset(
        runtime_value_operands,
        byte_size,
        value,
        StateGuardOperator::BitwiseXor,
    )
}

/// X86 has no fetch-XOR instruction that returns the old value. Use a genuine
/// locked CMPXCHG retry loop whose successful observation becomes the result.
pub fn encode_atomic_fetch_xor(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    value: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    encode_atomic_fetch_bitwise(
        runtime_value_operands,
        target_offset,
        byte_size,
        result_offset,
        value,
        StateGuardOperator::BitwiseXor,
        "fetch_xor",
    )
}

pub fn runtime_atomic_fetch_or_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    result_offset: usize,
    value: RuntimeValueOperandHandle,
) -> usize {
    runtime_atomic_fetch_bitwise_width(
        runtime_value_operands,
        byte_size,
        result_offset,
        value,
        StateGuardOperator::BitwiseOr,
    )
}

pub fn runtime_atomic_fetch_or_result_address_offset(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    value: RuntimeValueOperandHandle,
) -> usize {
    runtime_atomic_fetch_bitwise_result_address_offset(
        runtime_value_operands,
        byte_size,
        value,
        StateGuardOperator::BitwiseOr,
    )
}

/// X86 has no fetch-OR instruction that returns the old value. Use the shared
/// locked CMPXCHG retry lowering and return the successful observation.
pub fn encode_atomic_fetch_or(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    value: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    encode_atomic_fetch_bitwise(
        runtime_value_operands,
        target_offset,
        byte_size,
        result_offset,
        value,
        StateGuardOperator::BitwiseOr,
        "fetch_or",
    )
}

pub fn runtime_atomic_fetch_and_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    result_offset: usize,
    value: RuntimeValueOperandHandle,
) -> usize {
    runtime_atomic_fetch_bitwise_width(
        runtime_value_operands,
        byte_size,
        result_offset,
        value,
        StateGuardOperator::BitwiseAnd,
    )
}

pub fn runtime_atomic_fetch_and_result_address_offset(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    value: RuntimeValueOperandHandle,
) -> usize {
    runtime_atomic_fetch_bitwise_result_address_offset(
        runtime_value_operands,
        byte_size,
        value,
        StateGuardOperator::BitwiseAnd,
    )
}

/// X86 has no fetch-AND instruction that returns the old value. Use the shared
/// locked CMPXCHG retry lowering and return the successful observation.
pub fn encode_atomic_fetch_and(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    value: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    encode_atomic_fetch_bitwise(
        runtime_value_operands,
        target_offset,
        byte_size,
        result_offset,
        value,
        StateGuardOperator::BitwiseAnd,
        "fetch_and",
    )
}

pub fn runtime_atomic_swap_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    _result_offset: usize,
    new_value: RuntimeValueOperandHandle,
) -> usize {
    10 + runtime_value_operand_width(runtime_value_operands, new_value)
        + store_width(byte_size)
        + 10
        + store_width(byte_size)
}

pub fn runtime_atomic_swap_result_address_offset(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    new_value: RuntimeValueOperandHandle,
) -> usize {
    10 + runtime_value_operand_width(runtime_value_operands, new_value) + store_width(byte_size)
}

/// Atomic exchange. A memory XCHG is implicitly locked and leaves the
/// instruction-observed prior in r10, which is copied to the result place.
pub fn encode_atomic_swap(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    new_value: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_atomic_swap_width(
        runtime_value_operands,
        byte_size,
        result_offset,
        new_value,
    ));
    append_mov_r14_imm64(&mut bytes, 0);
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R10, new_value)?;
    append_xchg_r10_to_r14(&mut bytes, target_offset, byte_size)?;
    append_mov_r14_imm64(&mut bytes, 0);
    append_store_r10_to_r14(&mut bytes, result_offset, byte_size)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_atomic_swap_width(runtime_value_operands, byte_size, result_offset, new_value)
    );
    Ok(bytes)
}

pub fn runtime_atomic_compare_exchange_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    _result_offset: usize,
    expected: RuntimeValueOperandHandle,
    new_value: RuntimeValueOperandHandle,
) -> usize {
    // mov r14,imm64(base) (10) + new_value load (r10) + push r10 + expected load
    // (r10) + mov rax,r10 + pop r10 + lock cmpxchg. The push/pop stash mirrors
    // the binary write so operand evaluation (which accumulates in r10) cannot
    // clobber the other operand; `new_value` is the "left" at the fixed offset 10
    // and `expected` the "right" after the push gap.
    10 + runtime_value_operand_width(runtime_value_operands, new_value)
        + BINARY_RIGHT_OPERAND_PUSH_WIDTH
        + runtime_value_operand_width(runtime_value_operands, expected)
        + MOV_RAX_R10_WIDTH
        + BINARY_RIGHT_OPERAND_PUSH_WIDTH
        + lock_cmpxchg_r10_to_r14_width(byte_size)
        + 3 // mov r10, rax (instruction-observed prior)
        + 10 // mov r14, imm64(result base)
        + store_width(byte_size)
}

pub fn runtime_atomic_compare_exchange_result_address_offset(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    expected: RuntimeValueOperandHandle,
    new_value: RuntimeValueOperandHandle,
) -> usize {
    10 + runtime_value_operand_width(runtime_value_operands, new_value)
        + BINARY_RIGHT_OPERAND_PUSH_WIDTH
        + runtime_value_operand_width(runtime_value_operands, expected)
        + MOV_RAX_R10_WIDTH
        + BINARY_RIGHT_OPERAND_PUSH_WIDTH
        + lock_cmpxchg_r10_to_r14_width(byte_size)
        + 3
}

/// Atomic `compare_exchange`: hold the target base in r14, evaluate `new_value`
/// into r10 and stash it on the stack, evaluate `expected` into r10 and move it
/// to rax, restore `new_value` into r10, then `lock cmpxchg [r14+offset], r10`.
/// CMPXCHG compares rax (expected) with the place and swaps in r10 (new_value)
/// only on equality; the instruction-observed prior left in rax is copied into
/// the result place. The stash mirrors the binary write because operand
/// evaluation accumulates in r10.
pub fn encode_atomic_compare_exchange(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    expected: RuntimeValueOperandHandle,
    new_value: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_atomic_compare_exchange_width(
        runtime_value_operands,
        byte_size,
        result_offset,
        expected,
        new_value,
    ));
    append_mov_r14_imm64(&mut bytes, 0); // target base (imm64 @ +2 relocated)
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R10, new_value)?;
    append_push_r10(&mut bytes); // stash new_value across the expected eval
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R10, expected)?;
    append_mov_rax_r10(&mut bytes); // expected -> rax (CMPXCHG's implicit accumulator)
    append_pop_r10(&mut bytes); // restore new_value -> r10
    append_lock_cmpxchg_r10_to_r14(&mut bytes, target_offset, byte_size)?;
    bytes.extend([0x49, 0x89, 0xc2]); // mov r10, rax (prior)
    append_mov_r14_imm64(&mut bytes, 0); // result base (relocated independently)
    append_store_r10_to_r14(&mut bytes, result_offset, byte_size)?;
    debug_assert_eq!(
        bytes.len(),
        runtime_atomic_compare_exchange_width(
            runtime_value_operands,
            byte_size,
            result_offset,
            expected,
            new_value
        )
    );
    Ok(bytes)
}

#[allow(clippy::too_many_arguments)]
pub fn runtime_storage_convert_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    source: RuntimeValueOperandHandle,
    source_byte_size: usize,
    target_byte_size: usize,
    source_is_float: bool,
    target_is_float: bool,
    source_signed: bool,
    target_signed: bool,
    trapping: bool,
    saturating: bool,
) -> usize {
    // mov r14,imm64(target base) (10) + source operand load + convert + store.
    10 + runtime_value_operand_width(runtime_value_operands, source)
        + runtime_convert_operation_width(
            source_byte_size,
            target_byte_size,
            source_is_float,
            target_is_float,
            source_signed,
            target_signed,
            trapping,
            saturating,
        )
        + store_width(target_byte_size)
}

/// `target = source as T`: hold the target base in r14 (untouched by operand
/// evaluation, which reloads r15), evaluate the source operand into r10, convert
/// it in place between integer/float representations, and store the result.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_convert(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    target_byte_size: usize,
    source: RuntimeValueOperandHandle,
    source_byte_size: usize,
    source_is_float: bool,
    target_is_float: bool,
    source_signed: bool,
    target_signed: bool,
    trapping: bool,
    saturating: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_convert_width(
        runtime_value_operands,
        source,
        source_byte_size,
        target_byte_size,
        source_is_float,
        target_is_float,
        source_signed,
        target_signed,
        trapping,
        saturating,
    ));
    append_mov_r14_imm64(&mut bytes, 0); // target base (imm64 @ +2 relocated)
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R10, source)?;
    append_runtime_convert_operation(
        &mut bytes,
        source_byte_size,
        target_byte_size,
        source_is_float,
        target_is_float,
        source_signed,
        target_signed,
        trapping,
        saturating,
    );
    append_store_r10_to_r14(&mut bytes, target_offset, target_byte_size)?;
    Ok(bytes)
}

/// Address-computation prefix before the value operands in a pointee binary
/// write -- CANONICALIZED by the place materializer (Binary rung 1b):
/// `mov r15,imm64(frame)` (10) + `mov r15,[r15+ptr]` (7) + `mov r14,r15` (3)
/// -- r14 then holds the dereferenced runtime pointer (the target base)
/// across operand evaluation, exactly as before.
pub fn runtime_pointee_binary_operand_start_width() -> usize {
    20
}

pub fn runtime_pointee_binary_write_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    // 17 (frame base + deref ptr) + left + push r10 (2) + right + mov r11,r10 (3)
    // + pop r10 (2) + operation + store.
    runtime_pointee_binary_operand_start_width()
        + runtime_value_operand_width(runtime_value_operands, left)
        + 2
        + runtime_value_operand_width(runtime_value_operands, right)
        + 3
        + 2
        + runtime_binary_operation_width(operator, byte_size)
        + 7.max(store_width(byte_size))
}

/// `*(frame[pointer_byte_offset]) + field_byte_offset = left OP right`, where the
/// operands resolve against the runtime frame. The dereferenced target pointer is
/// held in r14 (untouched by operand evaluation, which reloads r15/r10/r11).
pub fn encode_runtime_pointee_binary_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    // Binary delegations (rung 1b): the place walk (mov r15,imm64; deref)
    // + the r14 hop -- the operand-start prefix grows 17 -> 20 and the
    // offset fn moves in lockstep. Exact-domain tail preserved via the
    // shared helper (Exact never enters the domain arms).
    let target =
        place_copy::transitional_place(omega_target_operations::RuntimeStorageRegion::RuntimeFrame)
            .with_step(omega_target_operations::PlaceStep::ConstOffset(
                pointer_byte_offset,
            ))
            .and_then(|place| place.with_step(omega_target_operations::PlaceStep::Deref))
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ConstOffset(
                    field_byte_offset,
                ))
            })
            .expect("a pointee place is four steps, within PLACE_MAX_STEPS");
    let (bytes, _) = place_copy::encode_place_binary_write(
        runtime_value_operands,
        &target,
        byte_size,
        left,
        operator,
        right,
        false,
        ArithmeticDomain::Exact,
        false,
    )?;
    Ok(bytes)
}

/// Length of the address-computation prefix that precedes the value operands
/// in a frame-base-indexed binary write -- CANONICALIZED by the place
/// materializer (Binary rung 1b): `mov r15,imm64(frame)` (10) +
/// `mov r11d,[r15+idx]` (7, 32-bit ZX) + `imul r11,r11,elem` (7) +
/// `add r15,r11` (3) + `mov r14,r15` (3).
pub fn runtime_frame_base_indexed_binary_left_operand_offset() -> usize {
    30
}

/// Length of the address-computation prefix that precedes the value operands
/// in a frame-INDEXED (slice-descriptor) binary write -- CANONICALIZED by
/// the place materializer (Binary rung 1b): `mov r15,imm64(frame)` (10) +
/// `mov r11d,[r15+idx]` (7) + `imul r11,r11,elem` (7) + `mov r15,[r15+desc]`
/// (7) + `add r15,r11` (3) + `mov r14,r15` (3). The element address ends in
/// r14, which operand evaluation never clobbers.
pub fn runtime_frame_indexed_binary_left_operand_offset() -> usize {
    37
}

pub fn runtime_frame_indexed_binary_write_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    runtime_frame_indexed_binary_left_operand_offset()
        + runtime_value_operand_width(runtime_value_operands, left)
        + 2 // push r10
        + runtime_value_operand_width(runtime_value_operands, right)
        + 3 // mov r11, r10
        + 2 // pop r10
        + runtime_binary_operation_width(operator, byte_size)
        + 7.max(store_width(byte_size))
}

/// `slice[i] = left OP right` through a frame-resident slice DESCRIPTOR with a
/// runtime index: deref the descriptor's data pointer, scale the index, and
/// run the same operand/binary/store tail as the frame-base-indexed binary
/// write (the inline-array twin above).
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_frame_indexed_binary_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    // Binary rung 1b: DELEGATES through the place materializer -- prefix
    // 34 -> 37 (the same multiset reordered + the r14 hop; the index still
    // 32-bit ZX in r11, the descriptor deref hops r15 in place).
    let target =
        place_copy::transitional_place(omega_target_operations::RuntimeStorageRegion::RuntimeFrame)
            .with_step(omega_target_operations::PlaceStep::ConstOffset(
                descriptor_offset,
            ))
            .and_then(|place| place.with_step(omega_target_operations::PlaceStep::Deref))
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ScaledIndex {
                    index_region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                    index_offset,
                    element_byte_size,
                })
            })
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ConstOffset(
                    field_byte_offset,
                ))
            })
            .expect("a frame-indexed place is five steps, within PLACE_MAX_STEPS");
    let (bytes, _) = place_copy::encode_place_binary_write(
        runtime_value_operands,
        &target,
        byte_size,
        left,
        operator,
        right,
        false,
        ArithmeticDomain::Exact,
        false,
    )?;
    debug_assert_eq!(
        bytes.len(),
        runtime_frame_indexed_binary_write_width(
            runtime_value_operands,
            byte_size,
            left,
            operator,
            right,
        )
    );
    Ok(bytes)
}

pub fn runtime_frame_base_indexed_binary_write_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    runtime_frame_base_indexed_binary_left_operand_offset()
        + runtime_value_operand_width(runtime_value_operands, left)
        + 2 // push r10
        + runtime_value_operand_width(runtime_value_operands, right)
        + 3 // mov r11, r10
        + 2 // pop r10
        + runtime_binary_operation_width(operator, byte_size)
        + 7.max(store_width(byte_size))
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_frame_base_indexed_binary_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    // Binary rung 1b: DELEGATES through the place materializer -- prefix
    // 27 -> 30 (the r14 hop), and the index load CANONICALIZES to the
    // 32-bit zero-extended discipline (the retired 64-bit index load could
    // splice a neighboring slot's bytes into the high half; the
    // materializer's ZX read is the correct one).
    let target =
        place_copy::transitional_place(omega_target_operations::RuntimeStorageRegion::RuntimeFrame)
            .with_step(omega_target_operations::PlaceStep::ConstOffset(
                base_byte_offset,
            ))
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ScaledIndex {
                    index_region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                    index_offset,
                    element_byte_size,
                })
            })
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ConstOffset(
                    field_byte_offset,
                ))
            })
            .expect("a frame-base-indexed place is four steps, within PLACE_MAX_STEPS");
    let (bytes, _) = place_copy::encode_place_binary_write(
        runtime_value_operands,
        &target,
        byte_size,
        left,
        operator,
        right,
        false,
        ArithmeticDomain::Exact,
        false,
    )?;
    debug_assert_eq!(
        bytes.len(),
        runtime_frame_base_indexed_binary_write_width(
            runtime_value_operands,
            byte_size,
            left,
            operator,
            right,
        )
    );
    Ok(bytes)
}

pub fn runtime_machine_indexed_binary_write_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    index_region: omega_target_operations::RuntimeStorageRegion,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    // Byte layout matches the frame-base binary write (only the base relocation
    // targets the machine symbol, handled by the relocations crate); a
    // FRAME-resident index inserts a `mov r15,imm64` frame-base load (+10)
    // before the index read.
    let frame_index_extra =
        if index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
            10
        } else {
            0
        };
    runtime_frame_base_indexed_binary_write_width(
        runtime_value_operands,
        byte_size,
        left,
        operator,
        right,
    ) + frame_index_extra
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_machine_indexed_binary_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    // Binary rung 1b: DELEGATES through the place materializer -- the
    // machine-region-index prefix moves 27 -> 30, the frame-region 37 -> 40
    // (the r14 hop); the frame-index base stays a `mov r11,imm64` at +10
    // (the retired `mov r15,imm64` position -- the walker's frame reloc and
    // +10 operand shift hold as-is), realigning the encoder with the shared
    // frame-base offset fn the walker consumes.
    let target =
        place_copy::transitional_place(omega_target_operations::RuntimeStorageRegion::Machine)
            .with_step(omega_target_operations::PlaceStep::ConstOffset(
                base_byte_offset,
            ))
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ScaledIndex {
                    index_region,
                    index_offset,
                    element_byte_size,
                })
            })
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ConstOffset(
                    field_byte_offset,
                ))
            })
            .expect("a machine-indexed place is four steps, within PLACE_MAX_STEPS");
    let (bytes, _) = place_copy::encode_place_binary_write(
        runtime_value_operands,
        &target,
        byte_size,
        left,
        operator,
        right,
        false,
        ArithmeticDomain::Exact,
        false,
    )?;
    debug_assert_eq!(
        bytes.len(),
        runtime_machine_indexed_binary_write_width(
            runtime_value_operands,
            index_region,
            byte_size,
            left,
            operator,
            right,
        )
    );
    Ok(bytes)
}

/// Prologue width of the double-indexed binary write (the left operand starts
/// here): mov r14,imm64 (10) [+ mov r10,imm64 (10) if any frame index]
/// + mov r15d (7) + mov r11d (7) + imul r15 (7) + imul r11 (7)
/// + add r14,r15 (3) + add r14,r11 (3).
pub fn runtime_machine_double_indexed_binary_left_operand_offset(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    // Canonicalized by the place materializer (Binary rung 1b): mov r15,
    // imm64 (10) + per-index [cross-region mov+load (17) | same-region
    // load (7)] + imul (7) each + add r15,r11 (3) + add r15,r10 (3) +
    // mov r14,r15 (3). Each frame index adds its OWN base.
    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    47 + if outer_index_region == frame { 10 } else { 0 }
        + if inner_index_region == frame { 10 } else { 0 }
}

pub fn runtime_machine_double_indexed_binary_write_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    runtime_machine_double_indexed_binary_left_operand_offset(
        outer_index_region,
        inner_index_region,
    ) + runtime_value_operand_width(runtime_value_operands, left)
        + 2 // push r10
        + runtime_value_operand_width(runtime_value_operands, right)
        + 3 // mov r11, r10
        + 2 // pop r10
        + runtime_binary_operation_width(operator, byte_size)
        + 7.max(store_width(byte_size))
}

/// Binary value into a BOTH-RUNTIME nested target (`grid[i][j] = a OP b`):
/// r14 = base + outer*outer_stride + inner*inner_stride, computed FIRST with
/// BOTH indices loaded before r14 is biased (the r14-before-bias key), then
/// the exact operand-evaluation tail of the single-index sibling -- operand
/// evaluation clobbers r15/r10/r11 but never r14.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_machine_double_indexed_binary_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    base_byte_offset: usize,
    outer_index_offset: usize,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    outer_stride: usize,
    inner_index_offset: usize,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_stride: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    for region in [outer_index_region, inner_index_region] {
        if !matches!(
            region,
            omega_target_operations::RuntimeStorageRegion::Machine
                | omega_target_operations::RuntimeStorageRegion::RuntimeFrame
        ) {
            return Err(Diagnostic::error(
                "X86_64 MVP encoder cannot write a double-indexed binary with this index region yet",
            ));
        }
    }
    // Binary rung 1b: DELEGATES through the place materializer -- each
    // frame-resident index materializes its OWN base (r11 outer, r10 inner;
    // the retired layout shared one r10 mov); prefixes move 44 -> 47
    // (both machine), 54 -> 57 (one frame), 54 -> 67 (both frame); the
    // offset fn becomes per-region sums and the walker arm splits per-index
    // relocs, all in the SAME commit (the shared-constant lesson).
    let target =
        place_copy::transitional_place(omega_target_operations::RuntimeStorageRegion::Machine)
            .with_step(omega_target_operations::PlaceStep::ConstOffset(
                base_byte_offset,
            ))
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ScaledIndex {
                    index_region: outer_index_region,
                    index_offset: outer_index_offset,
                    element_byte_size: outer_stride,
                })
            })
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ScaledIndex {
                    index_region: inner_index_region,
                    index_offset: inner_index_offset,
                    element_byte_size: inner_stride,
                })
            })
            .and_then(|place| {
                place.with_step(omega_target_operations::PlaceStep::ConstOffset(
                    field_byte_offset,
                ))
            })
            .expect("a double-indexed place is five steps, within PLACE_MAX_STEPS");
    let (bytes, _) = place_copy::encode_place_binary_write(
        runtime_value_operands,
        &target,
        byte_size,
        left,
        operator,
        right,
        false,
        ArithmeticDomain::Exact,
        false,
    )?;
    debug_assert_eq!(
        bytes.len(),
        runtime_machine_double_indexed_binary_write_width(
            runtime_value_operands,
            outer_index_region,
            inner_index_region,
            byte_size,
            left,
            operator,
            right,
        )
    );
    Ok(bytes)
}

/// Width of [`encode_runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_storage`].
pub fn runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_storage_width()
-> usize {
    // mov r14,imm64(frame) (10) + mov eax,[r14+outer] (7) + mov r11d,[r14+inner] (7)
    // + imul rax,imm32 (7) + imul r11,imm32 (7) + add r14,rax (3) + add r14,r11 (3)
    // + load rax,[r14+base+field] (7) + mov r15,imm64(target) (10) + store [r15+target] (7)
    68
}

/// Target-region relocation start (the `mov r15,imm64` before the store,
/// pre-`+2`) inside the frame-base double-indexed read.
pub fn runtime_storage_copy_from_runtime_frame_base_double_indexed_target_base_offset() -> usize {
    68 - 17
}

/// Fixed width of a value-position text-equals operand (the `TextEquals` arm
/// of `append_runtime_value_operand`): two relocated descriptor-base imm64
/// movs (10 each) with two 7-byte disp32 descriptor word loads apiece, then a
/// fixed 39-byte length-compare + bounded byte loop block and the 3-byte
/// result mov. MUST stay in lockstep with that encoder (it ends with a
/// `debug_assert_eq!` against this function) and with
/// `RUNTIME_TEXT_EQUALS_RIGHT_BASE_OFFSET` below.
pub fn runtime_text_equals_operand_width() -> usize {
    (10 + 7 + 7) + (10 + 7 + 7) + 39 + 3
}

/// Byte offset of the RIGHT descriptor's base `mov r15, imm64` inside a
/// text-equals operand (the relocation planner adds the +2 imm offset itself).
pub const RUNTIME_TEXT_EQUALS_RIGHT_BASE_OFFSET: usize = 10 + 7 + 7;

/// Width of a guard-position text-vs-literal content compare operand (the
/// `TextEqualsLiteral` arm of `append_runtime_value_operand`): the place's
/// descriptor-address setup (13 bytes for a storage base, 17 for a pointee or
/// fixed-indexed deref, 30 for a frame-base-indexed element address, 34 for a
/// frame-indexed element address, each starting with the relocated
/// `mov r15, imm64`), then a fixed 30-byte head (two disp32 descriptor word
/// loads, result zero, length compare + branch), one 13-byte disp32 byte
/// compare + branch per literal byte, and the fixed 9-byte tail (equal-result
/// mov + result move into the destination). MUST stay in lockstep with that
/// encoder (it ends with a `debug_assert_eq!` against this function).
pub fn runtime_text_equals_literal_operand_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    place: RuntimeValueOperandHandle,
    literal: &str,
) -> usize {
    let place_setup_width = if runtime_value_operands.storage(place).is_some() {
        // mov r15,imm64 (10) + mov rax,r15 (3)
        13
    } else if runtime_value_operands.pointee(place).is_some() {
        // mov r15,imm64 (10) + mov rax,[r15+ptr_off] (7)
        17
    } else if let Some((_, index_region, _, _, _, _)) = runtime_value_operands.frame_indexed(place)
    {
        // mov r15,imm64 (10) + mov rax,[r15+desc] (7) + mov r11,[r15+idx] (7)
        // + imul r11,r11,elem (7) + add rax,r11 (3)
        34 + usize::from(index_region == RuntimeStorageRegion::Machine) * 10
    } else if runtime_value_operands.frame_base_indexed(place).is_some() {
        // mov r15,imm64 (10) + mov r11,[r15+idx] (7) + imul r11,r11,elem (7)
        // + mov rax,r15 (3) + add rax,r11 (3)
        30
    } else if runtime_value_operands.frame_fixed_indexed(place).is_some() {
        // Constant element index folds into the descriptor displacement:
        // mov r15,imm64 (10) + mov rax,[r15+desc] (7)
        17
    } else {
        // Selection only builds this operand over storage/pointee/indexed
        // text places; the encoder rejects anything else with a hard
        // diagnostic before this width could be compared against emitted
        // bytes.
        0
    };
    place_setup_width + 30 + 13 * literal.len() + 9
}

pub fn runtime_value_operand_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    operand: RuntimeValueOperandHandle,
) -> usize {
    if runtime_value_operands.immediate_integer(operand).is_some() {
        10
    } else if let Some((_, _, byte_size)) = runtime_value_operands.storage(operand) {
        10 + load_width(byte_size)
    } else if let Some((_, _, byte_size)) = runtime_value_operands.pointee(operand) {
        // mov r15,imm64 (10) + mov rax,[r15+ptr_off] (7) + load dest,[rax+field].
        // A 16-bit load has the extra 0x66 operand-size prefix.
        17 + load_width(byte_size)
    } else if let Some((_, index_region, _, _, _, byte_size)) =
        runtime_value_operands.frame_indexed(operand)
    {
        // mov r15,imm64 (10) + mov rax,[r15+desc] (7) + mov r11,[r15+idx] (7)
        // + imul r11,r11,elem (7) + add rax,r11 (3) + load dest,[rax+field].
        34 + usize::from(index_region == RuntimeStorageRegion::Machine) * 10 + load_width(byte_size)
    } else if let Some((_, _, _, _, byte_size)) = runtime_value_operands.frame_base_indexed(operand)
    {
        // mov r15,imm64 (10) + mov r11,[r15+idx] (7) + imul r11,r11,elem (7)
        // + mov rax,r15 (3) + add rax,r11 (3) + load dest,[rax+base+field].
        30 + load_width(byte_size)
    } else if let Some((_, _, _, _, byte_size)) =
        runtime_value_operands.frame_fixed_indexed(operand)
    {
        // Constant element index folds into the load displacement, so the shape
        // matches the pointee case: mov r15,imm64 (10) + mov rax,[r15+desc] (7)
        // + load dest,[rax+const].
        17 + load_width(byte_size)
    } else if let Some((_, index_region, _, _, _, byte_size)) =
        runtime_value_operands.machine_indexed(operand)
    {
        // MUST mirror the machine-indexed emission arm: mov r15,imm64 (10)
        // + mov rax,r15 (3) + [frame index: mov r15,imm64 (10)] + index
        // load (7) + imul (7) + add rax,r11 (3) + element load.
        let frame_base =
            if index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
                10
            } else {
                0
            };
        10 + 3 + frame_base + 7 + 7 + 3 + load_width(byte_size)
    } else if runtime_value_operands.text_equals(operand).is_some() {
        runtime_text_equals_operand_width()
    } else if let Some((place, literal, _is_bounded_buffer)) =
        runtime_value_operands.text_equals_literal(operand)
    {
        // Carrier vs descriptor place are byte-width identical, so the width is
        // independent of `is_bounded_buffer`.
        runtime_text_equals_literal_operand_width(runtime_value_operands, place, &literal)
    } else if let Some((left, operator, right)) = runtime_value_operands.binary(operand) {
        let operation_width = if runtime_value_operands.binary_is_float(operand) {
            // Float operands: the SSE sequence width is PER-OPERATOR (comparisons
            // materialize 0/1) but f32/f64-identical at each operator. MUST match
            // the emission below or the recorded relocation offsets drift (silent
            // runtime segfault).
            let byte_width = runtime_value_operands
                .binary_byte_width(operand)
                .unwrap_or(8);
            let domain = runtime_value_operands
                .binary_arithmetic_domain(operand)
                .map(|(domain, _)| domain)
                .unwrap_or(ArithmeticDomain::Exact);
            runtime_float_binary_operation_width_with_domain(operator, byte_width, domain)
        } else if let Some(domain_operation) =
            operand_position_domain_operation(runtime_value_operands, operand, operator)
        {
            // Domain-honoring operand-position arithmetic: MUST mirror the
            // emission arm's dispatch exactly or relocation offsets drift.
            let byte_width = runtime_value_operands
                .binary_byte_width(operand)
                .unwrap_or(8);
            match domain_operation {
                OperandDomainOperation::AddSub {
                    domain,
                    operands_signed,
                } => saturating_trapping_add_sub_width(
                    domain,
                    operator,
                    byte_width,
                    operands_signed,
                    runtime_value_operands.immediate_integer(left).is_some(),
                    runtime_value_operands.immediate_integer(right).is_some(),
                ),
                OperandDomainOperation::Multiply {
                    domain,
                    operands_signed,
                } => saturating_trapping_multiply_width(
                    domain,
                    byte_width,
                    operands_signed,
                    runtime_value_operands.immediate_integer(left).is_some(),
                    runtime_value_operands.immediate_integer(right).is_some(),
                ),
                OperandDomainOperation::SaturatingSignedDivMod { want_remainder } => {
                    saturating_signed_divide_modulo_width(byte_width, want_remainder)
                }
                OperandDomainOperation::WrappingSignedDivMod { want_remainder } => {
                    wrapping_signed_divide_modulo_width(byte_width, want_remainder)
                }
                OperandDomainOperation::DomainShift { domain, .. } => {
                    let fix = if domain == ArithmeticDomain::Wrapping {
                        wrapping_shift_count_mask_width(byte_width)
                    } else if domain == ArithmeticDomain::Trapping {
                        SHIFT_COUNT_TRAP_GUARD_WIDTH
                    } else if operator == StateGuardOperator::ShiftRight {
                        WRAPPING_SHIFT_RIGHT_COUNT_SATURATE_WIDTH
                    } else {
                        WRAPPING_SHIFT_ZERO_CLAMP_WIDTH
                    };
                    runtime_binary_operation_width(operator, byte_width)
                        + fix
                        + wrapping_node_width_extension_width(byte_width)
                }
                OperandDomainOperation::SaturatingTrappingShiftLeft {
                    domain,
                    operands_signed,
                } => saturating_trapping_shift_left_width(domain, byte_width, operands_signed),
            }
        } else {
            // Use the SAME byte_size the emission picks (runtime_binary_operation_byte_size):
            // div/mod run at the operand width so a negative i32 dividend is handled
            // correctly, which changes the idiv/div core length -- the width MUST track
            // it or relocation offsets drift (silent segfault). Other ops keep 64-bit.
            // A nested WRAPPING node < 8 bytes appends one truncation move
            // (movzx/movsx: 4 bytes; the width-4 forms: 3) -- MUST stay in
            // lockstep with the emission arm.
            let wrapping_truncation = match (
                runtime_value_operands.binary_arithmetic_domain(operand),
                runtime_value_operands.binary_byte_width(operand),
            ) {
                (Some((omega_core::arithmetic::ArithmeticDomain::Wrapping, _)), Some(width))
                    if width < 8 =>
                {
                    wrapping_node_width_extension_width(width)
                }
                _ => 0,
            };
            runtime_binary_operation_width(
                operator,
                runtime_binary_operation_byte_size(
                    runtime_value_operands,
                    operator,
                    left,
                    right,
                    8,
                ),
            ) + wrapping_truncation
        };
        runtime_value_operand_width(runtime_value_operands, left)
            + runtime_value_operand_width(runtime_value_operands, right)
            + operation_width
            // push r10 (2) + mov r11,r10 (3) + pop r10 (2) + mov dest,r10 (3)
            + 10
    } else if let Some((source, src_bytes, tgt_bytes, src_float, tgt_float, src_signed)) =
        runtime_value_operands.convert(operand)
    {
        // Load source into r10, convert it in place, then mov dest,r10 (3). MUST
        // match the emission below or relocation offsets drift (runtime segfault).
        runtime_value_operand_width(runtime_value_operands, source)
            + runtime_convert_operation_width(
                src_bytes,
                tgt_bytes,
                src_float,
                tgt_float,
                src_signed,
                runtime_value_operands.convert_target_signed(operand),
                runtime_value_operands.convert_trapping(operand),
                runtime_value_operands.convert_saturating(operand),
            )
            + 3
    } else {
        0
    }
}

fn append_runtime_value_operand(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    bytes: &mut Vec<u8>,
    destination: Reg64,
    operand: RuntimeValueOperandHandle,
) -> Result<(), Diagnostic> {
    if let Some(value) = runtime_value_operands.immediate_integer(operand) {
        append_mov_reg_imm64(bytes, destination, value as u64);
        Ok(())
    } else if let Some((_, byte_offset, byte_size)) = runtime_value_operands.storage(operand) {
        append_mov_r15_imm64(bytes, 0);
        append_load_reg_from_r15(bytes, destination, byte_offset, byte_size)
    } else if let Some((pointer_byte_offset, field_byte_offset, byte_size)) =
        runtime_value_operands.pointee(operand)
    {
        // r15 = frame base (relocated). rax = the stored pointer; load through it.
        append_mov_r15_imm64(bytes, 0);
        append_load_rax_from_r15(bytes, pointer_byte_offset)?;
        append_load_reg_from_rax(bytes, destination, field_byte_offset, byte_size)
    } else if let Some((
        descriptor_offset,
        index_region,
        index_offset,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )) = runtime_value_operands.frame_indexed(operand)
    {
        // r15 = frame base (relocated). rax = slice data pointer from the descriptor;
        // r11 = index; rax += index*element + ... then load [rax + field].
        append_mov_r15_imm64(bytes, 0);
        append_load_rax_from_r15(bytes, descriptor_offset)?;
        if index_region == RuntimeStorageRegion::Machine {
            append_mov_r15_imm64(bytes, 0);
        }
        append_load_reg_from_r15(bytes, Reg64::R11, index_offset, 8)?;
        append_imul_r11_imm32(bytes, element_scale(element_byte_size)?);
        append_add_rax_r11(bytes);
        append_load_reg_from_rax(bytes, destination, field_byte_offset, byte_size)
    } else if let Some((
        base_byte_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )) = runtime_value_operands.frame_base_indexed(operand)
    {
        // r15 = frame base (relocated). The base lives inline in the frame at
        // base_byte_offset; rax = frame base, then add scaled index + base + field.
        append_mov_r15_imm64(bytes, 0);
        append_load_reg_from_r15(bytes, Reg64::R11, index_offset, 8)?;
        append_imul_r11_imm32(bytes, element_scale(element_byte_size)?);
        append_mov_rax_r15(bytes);
        append_add_rax_r11(bytes);
        append_load_reg_from_rax(
            bytes,
            destination,
            base_byte_offset + field_byte_offset,
            byte_size,
        )
    } else if let Some((
        descriptor_offset,
        element_index,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )) = runtime_value_operands.frame_fixed_indexed(operand)
    {
        // Descriptor-based access with a constant element index: r15 = frame base
        // (relocated), rax = the slice data pointer, then load through it at the
        // constant displacement `element_index*element + field`.
        append_mov_r15_imm64(bytes, 0);
        append_load_rax_from_r15(bytes, descriptor_offset)?;
        let displacement = element_index
            .checked_mul(element_byte_size)
            .and_then(|scaled| scaled.checked_add(field_byte_offset))
            .ok_or_else(|| {
                Diagnostic::error("X86_64 fixed indexed value operand offset overflow")
            })?;
        append_load_reg_from_rax(bytes, destination, displacement, byte_size)
    } else if let Some((
        base_byte_offset,
        index_region,
        index_offset,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )) = runtime_value_operands.machine_indexed(operand)
    {
        // MACHINE-owned array element in operand position: machine base
        // (relocated at the operand start) copied into rax as the address
        // accumulator; a FRAME-resident index re-materializes r15 with the
        // frame base at the PINNED offset 13 (mov imm64 10 + mov rax,r15 3;
        // see machine_indexed_operand_frame_index_base_offset). r11 is the
        // index/scale scratch (safe: the binary evaluator stashes the left
        // result on the stack across right-operand evaluation).
        append_mov_r15_imm64(bytes, 0);
        append_mov_rax_r15(bytes);
        let index_from_frame =
            index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
        if index_from_frame {
            append_mov_r15_imm64(bytes, 0);
            append_load_reg_from_r15(bytes, Reg64::R11, index_offset, 4)?;
        } else {
            append_load_reg_from_rax(bytes, Reg64::R11, index_offset, 4)?;
        }
        append_imul_r11_imm32(bytes, element_scale(element_byte_size)?);
        append_add_rax_r11(bytes);
        append_load_reg_from_rax(
            bytes,
            destination,
            base_byte_offset
                .checked_add(field_byte_offset)
                .ok_or_else(|| Diagnostic::error("machine-indexed operand offset overflow"))?,
            byte_size,
        )?;
        Ok(())
    } else if let Some((
        _,
        left_offset,
        left_is_bounded_buffer,
        _,
        right_offset,
        right_is_bounded_buffer,
    )) = runtime_value_operands.text_equals(operand)
    {
        append_runtime_text_equals_operand(
            bytes,
            destination,
            left_offset,
            left_is_bounded_buffer,
            right_offset,
            right_is_bounded_buffer,
        )?;
        Ok(())
    } else if let Some((place, literal, place_is_bounded_buffer)) =
        runtime_value_operands.text_equals_literal(operand)
    {
        append_runtime_text_equals_literal_operand(
            runtime_value_operands,
            bytes,
            destination,
            place,
            &literal,
            place_is_bounded_buffer,
        )?;
        Ok(())
    } else if let Some((left, operator, right)) = runtime_value_operands.binary(operand) {
        // Every comparison/operation accumulates its result in r10, so evaluating
        // the right operand clobbers the left result. Stash left on the stack
        // across the right evaluation, then combine.
        append_runtime_value_operand(runtime_value_operands, bytes, Reg64::R10, left)?;
        append_push_r10(bytes);
        append_runtime_value_operand(runtime_value_operands, bytes, Reg64::R10, right)?;
        append_mov_reg_reg(bytes, Reg64::R11, Reg64::R10); // right -> r11
        append_pop_r10(bytes); // restore left -> r10
        if runtime_value_operands.binary_is_float(operand) {
            // Float operands carry their IEEE bits in r10/r11; do the SSE op on the
            // bits (addss/addsd/...) rather than an integer add over them. The width
            // is threaded from build time (set once from the operands' scalar type),
            // so f32 picks `addss`/`movss` (4) and f64 picks `addsd`/`movsd` (8) —
            // no longer hardcoded. The encoded length is identical for both widths
            // at a given policy; the width twin emits the same policy guard to keep
            // relocation offsets in lockstep.
            let byte_width = runtime_value_operands
                .binary_byte_width(operand)
                .unwrap_or(8);
            let domain = runtime_value_operands
                .binary_arithmetic_domain(operand)
                .map(|(domain, _)| domain)
                .unwrap_or(ArithmeticDomain::Exact);
            append_runtime_float_binary_operation(bytes, operator, byte_width, domain)?;
        } else if let Some(domain_operation) =
            operand_position_domain_operation(runtime_value_operands, operand, operator)
        {
            // Decision 17 in OPERAND position: reuse the binary WRITE path's
            // r10/r11 sequences verbatim. Add/Sub take the width-correct op
            // whose flags reflect the operand width + the flag-driven
            // clamp/trap; Multiply takes the wide multiply + range clamp/trap;
            // signed Saturating div/mod take the TYPE_MIN/-1 fixup; signed
            // Wrapping div/mod take the idiv #DE guard (the byte-width
            // compare truncates the negated value exactly as the write path's
            // store would). The operand's byte_width is its REAL scalar width
            // here (set at construction for non-Exact domains). Upper r10
            // bits may be stale on the non-overflow path, which every
            // consumer tolerates (compares/stores run at width).
            let byte_width = runtime_value_operands
                .binary_byte_width(operand)
                .unwrap_or(8);
            match domain_operation {
                OperandDomainOperation::AddSub {
                    domain,
                    operands_signed,
                } => {
                    append_saturating_trapping_add_sub(
                        bytes,
                        domain,
                        operator,
                        byte_width,
                        operands_signed,
                        runtime_value_operands.immediate_integer(left).is_some(),
                        runtime_value_operands.immediate_integer(right).is_some(),
                    )?;
                }
                OperandDomainOperation::Multiply {
                    domain,
                    operands_signed,
                } => {
                    append_saturating_trapping_multiply(
                        bytes,
                        domain,
                        byte_width,
                        operands_signed,
                        runtime_value_operands.immediate_integer(left).is_some(),
                        runtime_value_operands.immediate_integer(right).is_some(),
                    )?;
                }
                OperandDomainOperation::SaturatingSignedDivMod { want_remainder } => {
                    append_saturating_signed_divide_modulo(bytes, byte_width, want_remainder)?;
                }
                OperandDomainOperation::WrappingSignedDivMod { want_remainder } => {
                    append_wrapping_signed_divide_modulo(bytes, byte_width, want_remainder)?;
                }
                OperandDomainOperation::DomainShift {
                    domain,
                    operands_signed,
                } => {
                    // Width-correct shift + the domain count fix (F8b:
                    // Wrapping masks the count -- sub-word AND only, the
                    // hardware mask IS the ruling at widths 4/8; Sat/Trap
                    // `>>` keep the floor fixes until F8c) + the node-width
                    // extension the parent contract requires.
                    if domain == ArithmeticDomain::Wrapping {
                        append_wrapping_shift_count_mask(bytes, byte_width);
                        append_runtime_binary_operation(bytes, operator, byte_width)?;
                    } else if domain == ArithmeticDomain::Trapping {
                        // F8c: an out-of-range count traps before the shift.
                        append_shift_count_trap_guard(bytes, byte_width);
                        append_runtime_binary_operation(bytes, operator, byte_width)?;
                    } else {
                        if operator == StateGuardOperator::ShiftRight {
                            append_wrapping_shift_right_count_saturate(bytes, byte_width);
                        }
                        append_runtime_binary_operation(bytes, operator, byte_width)?;
                        if operator != StateGuardOperator::ShiftRight {
                            append_wrapping_shift_zero_clamp(bytes, byte_width);
                        }
                    }
                    append_wrapping_node_width_extension(bytes, byte_width, operands_signed);
                }
                OperandDomainOperation::SaturatingTrappingShiftLeft {
                    domain,
                    operands_signed,
                } => {
                    // The write path's clamp/trap sequence verbatim; the result
                    // is range-correct at the node width (clamped bounds carry
                    // the right extension), so no width extension follows --
                    // the AddSub/Multiply operand contract.
                    append_saturating_trapping_shift_left(
                        bytes,
                        domain,
                        byte_width,
                        operands_signed,
                    )?;
                }
            }
        } else {
            // Comparisons use the operand width; other nested binaries do not carry
            // their result width, so assume 64-bit (matches runtime_value_operand_
            // width above for relocation consistency).
            append_runtime_binary_operation(
                bytes,
                operator,
                runtime_binary_operation_byte_size(
                    runtime_value_operands,
                    operator,
                    left,
                    right,
                    8,
                ),
            )?;
            // A nested WRAPPING binary must hand its PARENT the width-wrapped
            // VALUE in r10: the plain 64-bit op leaves the untruncated result
            // (0u32 - 2 = 0xFFFF_FFFF_FFFF_FFFE), and a sign/width-sensitive
            // parent (>>, /, %, comparisons) then reads it wrong -- the
            // interpreter wraps AT THE NODE (decision 17); the
            // store-truncation-is-the-wrap shortcut only holds at the WRITE.
            // Extension follows the node's own signedness. Width tracked in
            // runtime_value_operand_width -- MUST stay in lockstep (4 bytes,
            // except 3 for the width-4 forms).
            if let Some((omega_core::arithmetic::ArithmeticDomain::Wrapping, operands_signed)) =
                runtime_value_operands.binary_arithmetic_domain(operand)
                && let Some(byte_width) = runtime_value_operands.binary_byte_width(operand)
                && byte_width < 8
            {
                append_wrapping_node_width_extension(bytes, byte_width, operands_signed);
            }
        }
        append_mov_reg_reg(bytes, destination, Reg64::R10);
        Ok(())
    } else if let Some((source, src_bytes, tgt_bytes, src_float, tgt_float, src_signed)) =
        runtime_value_operands.convert(operand)
    {
        // Load the cast's source into r10, convert it in place (cvttsd2si /
        // cvtsi2sd / cvtsd2ss / movsxd), then move the result to `destination`.
        append_runtime_value_operand(runtime_value_operands, bytes, Reg64::R10, source)?;
        append_runtime_convert_operation(
            bytes,
            src_bytes,
            tgt_bytes,
            src_float,
            tgt_float,
            src_signed,
            runtime_value_operands.convert_target_signed(operand),
            runtime_value_operands.convert_trapping(operand),
            runtime_value_operands.convert_saturating(operand),
        );
        append_mov_reg_reg(bytes, destination, Reg64::R10);
        Ok(())
    } else {
        Err(Diagnostic::error(
            "X86_64 runtime value operand is not implemented yet",
        ))
    }
}

/// Value-position text content equality: `destination = (left == right)` as
/// bool 0/1, where both sides are `{ptr @ +0, len @ +8}` text descriptors at
/// relocated region bases. FIXED-WIDTH (`runtime_text_equals_operand_width`):
/// every descriptor word loads through a disp32 form, keeping the relocation
/// offsets (left base mov at the operand start, right base mov at
/// `RUNTIME_TEXT_EQUALS_RIGHT_BASE_OFFSET`) pinned.
///
/// Register use: r15 = descriptor base, then the right length, then the byte
/// scratch in the loop; rax/rcx = left ptr/len, rdx = right ptr, r9 = the
/// bool result (moved into `destination` last). r12/r13/r14 stay untouched
/// (dispatch state and the binary-write shapes' target base live there).
fn append_runtime_text_equals_operand(
    bytes: &mut Vec<u8>,
    destination: Reg64,
    left_offset: usize,
    left_is_bounded_buffer: bool,
    right_offset: usize,
    right_is_bounded_buffer: bool,
) -> Result<(), Diagnostic> {
    let operand_start = bytes.len();

    // Left descriptor: base (imm64 relocated at the operand start), ptr, len.
    append_mov_r15_imm64(bytes, 0);
    if left_is_bounded_buffer {
        bytes.extend([0x49, 0x8d, 0x87]); // lea rax, [r15+disp32] (left bytes)
        bytes.extend(disp32(left_offset + 8)?.to_le_bytes());
        bytes.extend([0x49, 0x8b, 0x8f]); // mov rcx, [r15+disp32] (left len)
        bytes.extend(disp32(left_offset)?.to_le_bytes());
    } else {
        bytes.extend([0x49, 0x8b, 0x87]); // mov rax, [r15+disp32] (left ptr)
        bytes.extend(disp32(left_offset)?.to_le_bytes());
        bytes.extend([0x49, 0x8b, 0x8f]); // mov rcx, [r15+disp32] (left len)
        bytes.extend(disp32(left_offset + 8)?.to_le_bytes());
    }

    // Right descriptor: base relocated at the pinned right-base offset; the
    // length load consumes r15 LAST (the base is no longer needed after it).
    debug_assert_eq!(
        bytes.len() - operand_start,
        RUNTIME_TEXT_EQUALS_RIGHT_BASE_OFFSET,
        "right descriptor base must sit at the pinned relocation offset"
    );
    append_mov_r15_imm64(bytes, 0);
    if right_is_bounded_buffer {
        bytes.extend([0x49, 0x8d, 0x97]); // lea rdx, [r15+disp32] (right bytes)
        bytes.extend(disp32(right_offset + 8)?.to_le_bytes());
        bytes.extend([0x4d, 0x8b, 0xbf]); // mov r15, [r15+disp32] (right len)
        bytes.extend(disp32(right_offset)?.to_le_bytes());
    } else {
        bytes.extend([0x49, 0x8b, 0x97]); // mov rdx, [r15+disp32] (right ptr)
        bytes.extend(disp32(right_offset)?.to_le_bytes());
        bytes.extend([0x4d, 0x8b, 0xbf]); // mov r15, [r15+disp32] (right len)
        bytes.extend(disp32(right_offset + 8)?.to_le_bytes());
    }

    // result = 0; unequal lengths are unequal text. The jne also means a
    // zero-length pair never enters the loop, so an all-zero (default)
    // descriptor's null pointer is never dereferenced. Fixed 39-byte block:
    //         xor   r9d, r9d
    //         cmp   rcx, r15
    //         jne   done            (+31)
    //   loop: test  rcx, rcx
    //         je    equal           (+20: all bytes matched)
    //         movzx r15d, byte [rax]
    //         cmp   r15b, [rdx]
    //         jne   done            (+17)
    //         inc   rax
    //         inc   rdx
    //         dec   rcx
    //         jmp   loop            (-25)
    //  equal: mov   r9d, 1
    //   done:
    bytes.extend([0x45, 0x31, 0xc9]); // xor r9d, r9d
    bytes.extend([0x4c, 0x39, 0xf9]); // cmp rcx, r15
    bytes.extend([0x75, 0x1f]); // jne +31 -> done
    bytes.extend([0x48, 0x85, 0xc9]); // test rcx, rcx
    bytes.extend([0x74, 0x14]); // je +20 -> equal
    bytes.extend([0x44, 0x0f, 0xb6, 0x38]); // movzx r15d, byte [rax]
    bytes.extend([0x44, 0x3a, 0x3a]); // cmp r15b, [rdx]
    bytes.extend([0x75, 0x11]); // jne +17 -> done
    bytes.extend([0x48, 0xff, 0xc0]); // inc rax
    bytes.extend([0x48, 0xff, 0xc2]); // inc rdx
    bytes.extend([0x48, 0xff, 0xc9]); // dec rcx
    bytes.extend([0xeb, 0xe7]); // jmp -25 -> loop
    bytes.extend([0x41, 0xb9]); // mov r9d, imm32 (equal: result = 1)
    bytes.extend(1i32.to_le_bytes());

    // done: move the bool into the requested destination register.
    match destination {
        Reg64::R10 => bytes.extend([0x4d, 0x89, 0xca]), // mov r10, r9
        Reg64::R11 => bytes.extend([0x4d, 0x89, 0xcb]), // mov r11, r9
    }

    debug_assert_eq!(
        bytes.len() - operand_start,
        runtime_text_equals_operand_width(),
        "text-equals operand encoder length must match its width"
    );
    Ok(())
}

/// Guard-position text content equality against an inline literal:
/// `destination = (place == literal)` as bool 0/1, where `place` names the
/// String side's `{ptr @ +0, len @ +8}` text descriptor (a relocated storage
/// base, a pointee field behind a frame pointer slot, or a frame-indexed /
/// frame-base-indexed / frame-fixed-indexed element field) and the literal's
/// expected bytes are compared as inline immediates -- no rodata descriptor
/// exists for the literal side. Width is
/// `runtime_text_equals_literal_operand_width`
/// (place-setup plus a fixed head plus 13 bytes per literal byte; every
/// memory operand uses the disp32 form so the shape never varies with the
/// offsets).
///
/// Register use: r15 = relocated base, rax = descriptor address base,
/// r11 = index scratch (frame-indexed setup), rcx/rdx = ptr/len, r9 = the
/// bool result (moved into `destination` last). r12/r13/r14 stay untouched.
fn append_runtime_text_equals_literal_operand(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    bytes: &mut Vec<u8>,
    destination: Reg64,
    place: RuntimeValueOperandHandle,
    literal: &str,
    place_is_bounded_buffer: bool,
) -> Result<(), Diagnostic> {
    let operand_start = bytes.len();

    // Descriptor address base -> rax (+ `descriptor_disp` displacement). The
    // relocated `mov r15, imm64` sits at the operand start (the relocation
    // planner targets it there).
    let descriptor_disp;
    if let Some((_, byte_offset, _)) = runtime_value_operands.storage(place) {
        append_mov_r15_imm64(bytes, 0);
        append_mov_rax_r15(bytes);
        descriptor_disp = byte_offset;
    } else if let Some((pointer_byte_offset, field_byte_offset, _)) =
        runtime_value_operands.pointee(place)
    {
        // r15 = frame base (relocated); rax = the stored pointer. The
        // descriptor sits in the POINTEE at the field offset -- never read
        // the pointer slot's own bytes as a descriptor.
        append_mov_r15_imm64(bytes, 0);
        append_load_rax_from_r15(bytes, pointer_byte_offset)?;
        descriptor_disp = field_byte_offset;
    } else if let Some((
        descriptor_offset,
        index_region,
        index_offset,
        element_byte_size,
        field_byte_offset,
        _,
    )) = runtime_value_operands.frame_indexed(place)
    {
        append_mov_r15_imm64(bytes, 0);
        append_load_rax_from_r15(bytes, descriptor_offset)?;
        if index_region == RuntimeStorageRegion::Machine {
            append_mov_r15_imm64(bytes, 0);
        }
        append_load_reg_from_r15(bytes, Reg64::R11, index_offset, 8)?;
        append_imul_r11_imm32(bytes, element_scale(element_byte_size)?);
        append_add_rax_r11(bytes);
        descriptor_disp = field_byte_offset;
    } else if let Some((base_byte_offset, index_offset, element_byte_size, field_byte_offset, _)) =
        runtime_value_operands.frame_base_indexed(place)
    {
        // Inline frame fixed array: the elements live in the frame itself at
        // base_byte_offset; rax = frame base + index*element (same shape as
        // the frame-base-indexed load operand above).
        append_mov_r15_imm64(bytes, 0);
        append_load_reg_from_r15(bytes, Reg64::R11, index_offset, 8)?;
        append_imul_r11_imm32(bytes, element_scale(element_byte_size)?);
        append_mov_rax_r15(bytes);
        append_add_rax_r11(bytes);
        descriptor_disp = base_byte_offset + field_byte_offset;
    } else if let Some((
        descriptor_offset,
        element_index,
        element_byte_size,
        field_byte_offset,
        _,
    )) = runtime_value_operands.frame_fixed_indexed(place)
    {
        // Constant element index: rax = the slice data pointer; the scaled
        // index folds into the descriptor displacement.
        append_mov_r15_imm64(bytes, 0);
        append_load_rax_from_r15(bytes, descriptor_offset)?;
        descriptor_disp = element_index
            .checked_mul(element_byte_size)
            .and_then(|scaled| scaled.checked_add(field_byte_offset))
            .ok_or_else(|| {
                Diagnostic::error("X86_64 fixed indexed text descriptor offset overflow")
            })?;
    } else {
        return Err(Diagnostic::error(
            "X86_64 MVP encoder cannot compare this text place against a literal yet",
        ));
    }

    if place_is_bounded_buffer {
        // Owned carrier `{len@0, bytes@8}`: rcx = bytes ADDRESS (rax+disp+8,
        // computed, not a stored pointer); rdx = len read at offset 0. Same widths
        // as the descriptor path (lea/mov are both `48 .. 88/90 disp32` = 7 bytes),
        // so the byte-compare loop, branch offsets, and operand width are all
        // unchanged.
        bytes.extend([0x48, 0x8d, 0x88]); // lea rcx, [rax+disp32] (carrier bytes addr)
        bytes.extend(disp32(descriptor_disp + 8)?.to_le_bytes());
        bytes.extend([0x48, 0x8b, 0x90]); // mov rdx, [rax+disp32] (carrier.len @ 0)
        bytes.extend(disp32(descriptor_disp)?.to_le_bytes());
    } else {
        bytes.extend([0x48, 0x8b, 0x88]); // mov rcx, [rax+disp32] (ptr)
        bytes.extend(disp32(descriptor_disp)?.to_le_bytes());
        bytes.extend([0x48, 0x8b, 0x90]); // mov rdx, [rax+disp32] (len)
        bytes.extend(disp32(descriptor_disp + 8)?.to_le_bytes());
    }

    // result = 0; a length mismatch is unequal text. The jne also means an
    // all-zero (default) descriptor never has its null pointer dereferenced
    // when the literal is non-empty.
    let literal_bytes = literal.as_bytes();
    bytes.extend([0x45, 0x31, 0xc9]); // xor r9d, r9d
    bytes.extend([0x48, 0x81, 0xfa]); // cmp rdx, imm32 (literal length)
    bytes.extend(disp32(literal_bytes.len())?.to_le_bytes());
    // Forward distances to `done` (the result move at the end): each byte
    // compare block is 13 bytes, plus the 6-byte equal-result mov.
    bytes.extend([0x0f, 0x85]); // jne rel32 -> done
    bytes.extend(disp32(13 * literal_bytes.len() + 6)?.to_le_bytes());
    for (byte_index, expected_byte) in literal_bytes.iter().enumerate() {
        bytes.extend([0x80, 0xb9]); // cmp byte [rcx+disp32], imm8
        bytes.extend(disp32(byte_index)?.to_le_bytes());
        bytes.push(*expected_byte);
        let remaining_blocks = literal_bytes.len() - 1 - byte_index;
        bytes.extend([0x0f, 0x85]); // jne rel32 -> done
        bytes.extend(disp32(13 * remaining_blocks + 6)?.to_le_bytes());
    }
    bytes.extend([0x41, 0xb9]); // mov r9d, imm32 (equal: result = 1)
    bytes.extend(1i32.to_le_bytes());

    // done: move the bool into the requested destination register.
    match destination {
        Reg64::R10 => bytes.extend([0x4d, 0x89, 0xca]), // mov r10, r9
        Reg64::R11 => bytes.extend([0x4d, 0x89, 0xcb]), // mov r11, r9
    }

    debug_assert_eq!(
        bytes.len() - operand_start,
        runtime_text_equals_literal_operand_width(runtime_value_operands, place, literal),
        "text-equals-literal operand encoder length must match its width"
    );
    Ok(())
}

/// Value width of a runtime operand, looking through nested binary operands.
/// `None` for immediates (which carry no width). Used to size comparisons, whose
/// result type (bool) does not reflect the compared operands' width.
fn runtime_value_operand_value_byte_size(
    operands: &impl RuntimeValueOperandSource,
    operand: RuntimeValueOperandHandle,
) -> Option<usize> {
    if let Some((_, _, byte_size)) = operands.storage(operand) {
        return Some(byte_size);
    }
    if let Some((_, _, byte_size)) = operands.pointee(operand) {
        return Some(byte_size);
    }
    if let Some((_, _, _, _, _, byte_size)) = operands.frame_indexed(operand) {
        return Some(byte_size);
    }
    if let Some((_, _, _, _, byte_size)) = operands.frame_base_indexed(operand) {
        return Some(byte_size);
    }
    if let Some((_, _, _, _, byte_size)) = operands.frame_fixed_indexed(operand) {
        return Some(byte_size);
    }
    if let Some((left, _, right)) = operands.binary(operand) {
        return runtime_value_operand_value_byte_size(operands, left)
            .or_else(|| runtime_value_operand_value_byte_size(operands, right));
    }
    if let Some((_, _, target_byte_size, _, _, _)) = operands.convert(operand) {
        return Some(target_byte_size);
    }
    if operands.text_equals(operand).is_some() || operands.text_equals_literal(operand).is_some() {
        // Text content equality evaluates to a bool.
        return Some(1);
    }
    None
}

/// Width to compare two operands at: the first operand with a known width, else
/// the i32 default. (`a OP b` requires `a` and `b` to share a type, so either
/// operand's width is the comparison width.)
fn runtime_binary_compare_byte_size(
    operands: &impl RuntimeValueOperandSource,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
) -> usize {
    runtime_value_operand_value_byte_size(operands, left)
        .or_else(|| runtime_value_operand_value_byte_size(operands, right))
        .unwrap_or(4)
}

fn is_comparison_operator(operator: StateGuardOperator) -> bool {
    matches!(
        operator,
        StateGuardOperator::Equal
            | StateGuardOperator::NotEqual
            | StateGuardOperator::Greater
            | StateGuardOperator::GreaterOrEqual
            | StateGuardOperator::Less
            | StateGuardOperator::LessOrEqual
            | StateGuardOperator::GreaterUnsigned
            | StateGuardOperator::GreaterOrEqualUnsigned
            | StateGuardOperator::LessUnsigned
            | StateGuardOperator::LessOrEqualUnsigned
    )
}

/// Width to pass to `append_runtime_binary_operation`. Comparisons produce a
/// `bool`, so the target width is not the compared-operands' width — derive it
/// from the operands instead. All other operations share the target's width.
fn runtime_binary_operation_byte_size(
    operands: &impl RuntimeValueOperandSource,
    operator: StateGuardOperator,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
    target_byte_size: usize,
) -> usize {
    if is_comparison_operator(operator) {
        runtime_binary_compare_byte_size(operands, left, right)
    } else if matches!(
        operator,
        StateGuardOperator::Divide
            | StateGuardOperator::Modulo
            | StateGuardOperator::DivideUnsigned
            | StateGuardOperator::ModuloUnsigned
            | StateGuardOperator::ShiftLeft
            | StateGuardOperator::ShiftRight
            | StateGuardOperator::ShiftRightLogical
    ) {
        // Division/modulo are NOT modular: a 64-bit idiv/div on a zero-extended
        // negative i32 dividend yields a wrong quotient. Run at the OPERAND width (an
        // immediate has no width, so use the non-immediate operand's), so a 32-bit
        // op handles the i32 dividend correctly -- signed via cdq, unsigned via the
        // resolver mapping Divide->DivideUnsigned. Add/sub/mul are modular and keep
        // the default 64-bit form. See [[guard-negative-i32-arithmetic]].
        //
        // SHIFTS join this branch for the same reason: a 64-bit `sar` on a
        // zero-extended negative i32 reads the high bit wrong (`-320 >> 2` would
        // become 0x3FFFFFB0, not -80), so run the shift at the shifted VALUE's width
        // (its left operand). A 32-bit `sar`/`shr`/`shl` honors the i32 sign/high bit,
        // and `<<` at the operand width also drops i32 overflow (wrapping semantics)
        // instead of leaking into the upper 32 bits. Both width encodings are the same
        // length, so relocation offsets are unaffected.
        //
        // When BOTH operands are immediates (a constant/constant divide that did not
        // fold) neither has a storage width, so fall back to the TARGET (declared)
        // width -- NOT 4. An i64 constant divide must run 64-bit; a 32-bit core would
        // truncate the dividend (e.g. -9_000_000_000) and the planned/emitted widths
        // would disagree (`runtime_storage_binary_write_width` uses the target size).
        runtime_value_operand_value_byte_size(operands, left)
            .or_else(|| runtime_value_operand_value_byte_size(operands, right))
            .unwrap_or(target_byte_size)
    } else {
        target_byte_size
    }
}

/// The width-correct integer idiv/div core: dividend in r10, divisor in r11,
/// quotient (or remainder, when `want_remainder`) back in r10. A 32-bit divide
/// reads only the low dword, so the width must match the operands. Signed uses
/// cdq/cqo + `idiv`; unsigned zeroes the dividend-high half + `div`. Shared by the
/// normal binary-op path and the saturating divide/modulo helper.
fn append_integer_divide_modulo_core(
    bytes: &mut Vec<u8>,
    byte_size: usize,
    want_remainder: bool,
    signed: bool,
) {
    if byte_size <= 4 {
        // Narrow SIGNED operands may arrive ZERO-extended (e.g. the guard-subject
        // load path; see append_saturating_trapping_multiply), so a 32-bit idiv would
        // divide i8 -20 as 236. Sign-extend both to 32 bits first. Idempotent when
        // they are already sign-extended (the storage-write path); unsigned div is
        // correct zero-extended and skips this.
        if signed && byte_size == 1 {
            bytes.extend([0x4d, 0x0f, 0xbe, 0xd2]); // movsx r10, r10b
            bytes.extend([0x4d, 0x0f, 0xbe, 0xdb]); // movsx r11, r11b
        } else if signed && byte_size == 2 {
            bytes.extend([0x4d, 0x0f, 0xbf, 0xd2]); // movsx r10, r10w
            bytes.extend([0x4d, 0x0f, 0xbf, 0xdb]); // movsx r11, r11w
        }
        bytes.extend([0x41, 0x8b, 0xc2]); // mov eax, r10d
        if signed {
            bytes.push(0x99); // cdq (sign-extend eax -> edx)
            bytes.extend([0x41, 0xf7, 0xfb]); // idiv r11d
        } else {
            bytes.extend([0x31, 0xd2]); // xor edx, edx
            bytes.extend([0x41, 0xf7, 0xf3]); // div r11d
        }
        if want_remainder {
            bytes.extend([0x41, 0x89, 0xd2]); // mov r10d, edx (remainder)
        } else {
            bytes.extend([0x41, 0x89, 0xc2]); // mov r10d, eax (quotient)
        }
    } else {
        bytes.extend([0x4c, 0x89, 0xd0]); // mov rax, r10
        if signed {
            bytes.extend([0x48, 0x99]); // cqo (sign-extend rax -> rdx)
            bytes.extend([0x49, 0xf7, 0xfb]); // idiv r11
        } else {
            bytes.extend([0x31, 0xd2]); // xor edx, edx (clears rdx)
            bytes.extend([0x49, 0xf7, 0xf3]); // div r11
        }
        if want_remainder {
            bytes.extend([0x49, 0x89, 0xd2]); // mov r10, rdx (remainder)
        } else {
            bytes.extend([0x49, 0x89, 0xc2]); // mov r10, rax (quotient)
        }
    }
}

/// Saturating SIGNED divide/modulo (dividend r10, divisor r11, result r10).
/// Integer division overflows only at TYPE_MIN / -1, the one corner `idiv`
/// hardware-traps on; guard the `divisor == -1` case so Saturating clamps instead
/// of trapping: `a % -1 == 0`, and `a / -1 == -a` saturating TYPE_MIN -> TYPE_MAX.
/// Every other divisor goes through the normal idiv (division reduces magnitude,
/// so no quotient/remainder can overflow). Unsigned div/mod never overflow and so
/// never reach here -- they fall through to the normal path.
fn append_saturating_signed_divide_modulo(
    bytes: &mut Vec<u8>,
    byte_size: usize,
    want_remainder: bool,
) -> Result<(), Diagnostic> {
    // cmp r11, -1 (sized): the only divisor needing the saturating fixup.
    if byte_size <= 4 {
        bytes.extend([0x41, 0x83, 0xfb, 0xff]); // cmp r11d, -1
    } else {
        bytes.extend([0x49, 0x83, 0xfb, 0xff]); // cmp r11, -1
    }
    // The divisor == -1 fixup block.
    let mut special: Vec<u8> = Vec::new();
    if want_remainder {
        special.extend([0x45, 0x31, 0xd2]); // xor r10d, r10d  (a % -1 == 0)
    } else if byte_size <= 2 {
        // i8/i16: the dividend rides sign-extended in a 32-bit register, so `neg`
        // does NOT wrap at the narrow width -- a == TYPE_MIN yields -TYPE_MIN ==
        // TYPE_MAX + 1 (e.g. 128 for i8), the only overflow. The i32/i64 path below
        // detects TYPE_MIN via `neg`'s overflow flag, which a narrow TYPE_MIN cannot
        // set; here instead clamp any result above TYPE_MAX down to TYPE_MAX.
        let imax = ((1i128 << (8 * byte_size - 1)) - 1) as u32;
        special.extend([0x41, 0xf7, 0xda]); // neg r10d  (-a; a==TYPE_MIN -> TYPE_MAX+1)
        special.push(0x41);
        special.push(0xb9);
        special.extend(imax.to_le_bytes()); // mov r9d, TYPE_MAX
        special.extend([0x45, 0x39, 0xca]); // cmp r10d, r9d
        special.extend([0x45, 0x0f, 0x4f, 0xd1]); // cmovg r10d, r9d  (> TYPE_MAX -> TYPE_MAX)
    } else if byte_size <= 4 {
        let imax = ((1i128 << (8 * byte_size - 1)) - 1) as u32;
        special.extend([0x41, 0xf7, 0xda]); // neg r10d  (sets OF iff r10d == TYPE_MIN)
        special.push(0x41);
        special.push(0xb9);
        special.extend(imax.to_le_bytes()); // mov r9d, TYPE_MAX
        special.extend([0x45, 0x0f, 0x40, 0xd1]); // cmovo r10d, r9d  (TYPE_MIN -> TYPE_MAX)
    } else {
        let imax = ((1i128 << (8 * byte_size - 1)) - 1) as u64;
        special.extend([0x49, 0xf7, 0xda]); // neg r10
        special.push(0x49);
        special.push(0xb9);
        special.extend(imax.to_le_bytes()); // mov r9, TYPE_MAX
        special.extend([0x4d, 0x0f, 0x40, 0xd1]); // cmovo r10, r9
    }
    // The normal idiv (every divisor except -1).
    let mut normal: Vec<u8> = Vec::new();
    append_integer_divide_modulo_core(&mut normal, byte_size, want_remainder, true);
    // jne over (special + the jmp) to the idiv; run special; jmp past the idiv.
    // Both blocks are well under 128 bytes, so rel8 offsets suffice.
    bytes.push(0x75);
    bytes.push((special.len() + 2) as u8); // jne -> normal
    bytes.extend(special);
    bytes.push(0xeb);
    bytes.push(normal.len() as u8); // jmp -> done
    bytes.extend(normal);
    Ok(())
}

/// WRAPPING signed divide/modulo. x86 `idiv` raises #DE (integer-overflow trap)
/// for TYPE_MIN / -1; the Wrapping domain must instead produce the WRAPPED result
/// (TYPE_MIN for divide -- the true quotient TYPE_MAX+1 wraps to TYPE_MIN -- and 0
/// for modulo). Guard the single overflowing divisor (-1) and avoid idiv for it:
/// `a / -1 == -a` via `neg r10` (and `neg` of TYPE_MIN naturally wraps to
/// TYPE_MIN, so no clamp is needed, unlike the saturating variant); `a % -1 == 0`.
/// Narrow widths (i8/i16) let the store truncate the negated 32-bit value back to
/// the correct wrapped byte. Divide-by-zero still reaches `idiv` and traps,
/// matching the interpreter. (aarch64 `sdiv` does not trap on overflow, so this
/// guard is x86_64-only.)
fn append_wrapping_signed_divide_modulo(
    bytes: &mut Vec<u8>,
    byte_size: usize,
    want_remainder: bool,
) -> Result<(), Diagnostic> {
    // cmp r11, -1 (sized): the only divisor that would overflow idiv.
    if byte_size <= 4 {
        bytes.extend([0x41, 0x83, 0xfb, 0xff]); // cmp r11d, -1
    } else {
        bytes.extend([0x49, 0x83, 0xfb, 0xff]); // cmp r11, -1
    }
    // The divisor == -1 fixup block (always 3 bytes).
    let mut special: Vec<u8> = Vec::new();
    if want_remainder {
        special.extend([0x45, 0x31, 0xd2]); // xor r10d, r10d  (a % -1 == 0)
    } else if byte_size <= 4 {
        special.extend([0x41, 0xf7, 0xda]); // neg r10d  (-a; TYPE_MIN wraps to TYPE_MIN)
    } else {
        special.extend([0x49, 0xf7, 0xda]); // neg r10
    }
    // The normal idiv (every divisor except -1).
    let mut normal: Vec<u8> = Vec::new();
    append_integer_divide_modulo_core(&mut normal, byte_size, want_remainder, true);
    bytes.push(0x75);
    bytes.push((special.len() + 2) as u8); // jne -> normal
    bytes.extend(special);
    bytes.push(0xeb);
    bytes.push(normal.len() as u8); // jmp -> done
    bytes.extend(normal);
    Ok(())
}

fn append_runtime_binary_operation(
    bytes: &mut Vec<u8>,
    operator: StateGuardOperator,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    match operator {
        StateGuardOperator::Add => bytes.extend([0x4d, 0x01, 0xda]), // add r10, r11
        StateGuardOperator::And => bytes.extend([0x4d, 0x21, 0xda]), // and r10, r11
        StateGuardOperator::Or => bytes.extend([0x4d, 0x09, 0xda]),  // or r10, r11
        StateGuardOperator::BitwiseAnd => bytes.extend([0x4d, 0x21, 0xda]), // and r10, r11
        StateGuardOperator::BitwiseOr => bytes.extend([0x4d, 0x09, 0xda]), // or r10, r11
        StateGuardOperator::BitwiseXor => bytes.extend([0x4d, 0x31, 0xda]), // xor r10, r11
        StateGuardOperator::Subtract => bytes.extend([0x4d, 0x29, 0xda]), // sub r10, r11
        StateGuardOperator::Multiply => bytes.extend([0x4d, 0x0f, 0xaf, 0xd3]), // imul r10, r11
        StateGuardOperator::Max
        | StateGuardOperator::Min
        | StateGuardOperator::MaxUnsigned
        | StateGuardOperator::MinUnsigned => {
            // Compare at the operand width (32-bit for i32, else 64-bit) so an
            // i32 sign/high bit is read correctly, then conditionally take r11.
            // Max keeps the larger (cmovl signed / cmovb unsigned: replace when
            // r10 < r11); Min keeps the smaller (cmovg / cmova: replace when
            // r10 > r11).
            let keep_smaller = matches!(
                operator,
                StateGuardOperator::Min | StateGuardOperator::MinUnsigned
            );
            let unsigned = matches!(
                operator,
                StateGuardOperator::MaxUnsigned | StateGuardOperator::MinUnsigned
            );
            // cmov opcode byte: signed below/above use 4c/4f; unsigned 42/47.
            let cmov = match (keep_smaller, unsigned) {
                (false, false) => 0x4c, // cmovl
                (true, false) => 0x4f,  // cmovg
                (false, true) => 0x42,  // cmovb
                (true, true) => 0x47,   // cmova
            };
            if byte_size <= 4 {
                bytes.extend([0x45, 0x39, 0xda]); // cmp r10d, r11d
                bytes.extend([0x45, 0x0f, cmov, 0xd3]); // cmovcc r10d, r11d
            } else {
                bytes.extend([0x4d, 0x39, 0xda]); // cmp r10, r11
                bytes.extend([0x4d, 0x0f, cmov, 0xd3]); // cmovcc r10, r11
            }
        }
        StateGuardOperator::Divide
        | StateGuardOperator::Modulo
        | StateGuardOperator::DivideUnsigned
        | StateGuardOperator::ModuloUnsigned => {
            // Quotient -> (r/e)ax, remainder -> (r/e)dx; the width-correct idiv
            // sequence lives in the shared core (also used by saturating div/mod).
            let want_remainder = matches!(
                operator,
                StateGuardOperator::Modulo | StateGuardOperator::ModuloUnsigned
            );
            let signed = matches!(
                operator,
                StateGuardOperator::Divide | StateGuardOperator::Modulo
            );
            append_integer_divide_modulo_core(bytes, byte_size, want_remainder, signed);
        }
        StateGuardOperator::ShiftLeft
        | StateGuardOperator::ShiftRight
        | StateGuardOperator::ShiftRightLogical => {
            // Shift count must live in cl. Right shift is arithmetic (`sar`) for
            // signed operands and logical (`shr`) for unsigned; sized to the
            // operands so an i32 high bit is honored.
            let arithmetic_right = matches!(operator, StateGuardOperator::ShiftRight);
            let logical_right = matches!(operator, StateGuardOperator::ShiftRightLogical);
            if byte_size <= 4 {
                bytes.extend([0x44, 0x89, 0xd9]); // mov ecx, r11d
                if arithmetic_right {
                    bytes.extend([0x41, 0xd3, 0xfa]); // sar r10d, cl
                } else if logical_right {
                    bytes.extend([0x41, 0xd3, 0xea]); // shr r10d, cl
                } else {
                    bytes.extend([0x41, 0xd3, 0xe2]); // shl r10d, cl
                }
            } else {
                bytes.extend([0x4c, 0x89, 0xd9]); // mov rcx, r11
                if arithmetic_right {
                    bytes.extend([0x49, 0xd3, 0xfa]); // sar r10, cl
                } else if logical_right {
                    bytes.extend([0x49, 0xd3, 0xea]); // shr r10, cl
                } else {
                    bytes.extend([0x49, 0xd3, 0xe2]); // shl r10, cl
                }
            }
        }
        StateGuardOperator::Equal
        | StateGuardOperator::NotEqual
        | StateGuardOperator::Greater
        | StateGuardOperator::GreaterOrEqual
        | StateGuardOperator::Less
        | StateGuardOperator::LessOrEqual
        | StateGuardOperator::GreaterUnsigned
        | StateGuardOperator::GreaterOrEqualUnsigned
        | StateGuardOperator::LessUnsigned
        | StateGuardOperator::LessOrEqualUnsigned => {
            // Compare at the operand width (`byte_size` here is the operand
            // width, not the bool result) so an i32 sign bit is read correctly.
            // Ordering uses signed setcc (setl/setg/...) or unsigned (setb/seta/
            // ...) per the operand type.
            append_cmp_r10_r11(bytes, byte_size)?;
            bytes.extend(match operator {
                StateGuardOperator::Equal => [0x0f, 0x94, 0xc0], // sete
                StateGuardOperator::NotEqual => [0x0f, 0x95, 0xc0], // setne
                StateGuardOperator::Greater => [0x0f, 0x9f, 0xc0], // setg
                StateGuardOperator::GreaterOrEqual => [0x0f, 0x9d, 0xc0], // setge
                StateGuardOperator::Less => [0x0f, 0x9c, 0xc0],  // setl
                StateGuardOperator::LessOrEqual => [0x0f, 0x9e, 0xc0], // setle
                StateGuardOperator::GreaterUnsigned => [0x0f, 0x97, 0xc0], // seta
                StateGuardOperator::GreaterOrEqualUnsigned => [0x0f, 0x93, 0xc0], // setae
                StateGuardOperator::LessUnsigned => [0x0f, 0x92, 0xc0], // setb
                StateGuardOperator::LessOrEqualUnsigned => [0x0f, 0x96, 0xc0], // setbe
                _ => unreachable!(),
            });
            bytes.extend([0x44, 0x0f, 0xb6, 0xd0]); // movzx r10d, al
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 runtime binary operator `{operator:?}` is not implemented yet"
            )));
        }
    }
    Ok(())
}

/// Saturating/Trapping logical `>>` zero clamp (floor semantics, until F8c):
/// floor(x / 2^n) with a count at/above the TYPE width yields 0, but the
/// hardware `shr` masks the count to the op width instead (40 & 31 = 8). The
/// FULL count survives in r11 (the shift arm only copies it to cl), so
/// compare it UNSIGNED against the bit width and cmov zero over the shifted
/// result -- a negative signed count is huge unsigned and clamps. rax is
/// scratch mid-operation, as in the div/setcc arms. WRAPPING shifts no
/// longer take this fix: F8b masks their count instead (ch5 ruling).
fn append_wrapping_shift_zero_clamp(bytes: &mut Vec<u8>, byte_size: usize) {
    bytes.extend([0x31, 0xc0]); // xor eax, eax
    bytes.extend([0x49, 0x83, 0xfb, (byte_size * 8) as u8]); // cmp r11, width_bits
    bytes.extend([0x4c, 0x0f, 0x43, 0xd0]); // cmovae r10, rax
}

/// Bytes of [`append_wrapping_shift_zero_clamp`]: xor (2) + cmp (4) + cmov (4).
const WRAPPING_SHIFT_ZERO_CLAMP_WIDTH: usize = 10;

/// Saturating/Trapping arithmetic `>>` count saturation (floor semantics,
/// until F8c): floor(x / 2^n) SIGN-FILLS for an at/above-width count, and a
/// post-fix cannot recover the sign once the hardware-masked `sar` has
/// consumed the value -- so saturate the COUNT to width-1 first (`sar` by
/// width-1 IS the sign-fill). Runs BEFORE the plain shift arm, which copies
/// the (now saturated) r11 into cl. rax is scratch. WRAPPING `>>` no longer
/// takes this fix: F8b masks its count instead (ch5 ruling).
fn append_wrapping_shift_right_count_saturate(bytes: &mut Vec<u8>, byte_size: usize) {
    let width_bits = (byte_size * 8) as u8;
    bytes.push(0xb8); // mov eax, imm32
    bytes.extend(u32::from(width_bits - 1).to_le_bytes());
    bytes.extend([0x49, 0x83, 0xfb, width_bits]); // cmp r11, width_bits
    bytes.extend([0x4c, 0x0f, 0x43, 0xd8]); // cmovae r11, rax
}

/// Bytes of [`append_wrapping_shift_right_count_saturate`]: mov (5) + cmp (4)
/// + cmov (4).
const WRAPPING_SHIFT_RIGHT_COUNT_SATURATE_WIDTH: usize = 13;

/// F8b (ch5 shift-count ruling): a WRAPPING shift masks the COUNT to the
/// operand width (`k & (width - 1)`). The hardware `shl`/`shr`/`sar` already
/// mask mod the OP width (32/64) -- exactly the ruling at widths 4/8 -- so
/// only sub-word operands need the explicit mask. Runs BEFORE the plain
/// shift arm (which copies r11 into cl); masks r11 in place.
fn append_wrapping_shift_count_mask(bytes: &mut Vec<u8>, byte_size: usize) {
    if matches!(byte_size, 1 | 2) {
        let mask = (byte_size * 8 - 1) as u8;
        bytes.extend([0x41, 0x83, 0xe3, mask]); // and r11d, mask
    }
}

/// Bytes of [`append_wrapping_shift_count_mask`]: and r11d, imm8 (4) for
/// sub-word operands; 0 at widths 4/8 (the hardware mask IS the ruling).
const fn wrapping_shift_count_mask_width(byte_size: usize) -> usize {
    match byte_size {
        1 | 2 => 4,
        _ => 0,
    }
}

/// F8c count guard: `cmp r11, width ; jb +2 ; ud2` -- a TRAPPING shift's
/// out-of-range count traps BEFORE the shift runs, regardless of the shifted
/// value (`0 << 40` traps; the count is invalid, not the result). The full
/// count survives in r11 (the shift arm only copies it to cl), and reads
/// UNSIGNED so a negative signed count is huge and traps.
fn append_shift_count_trap_guard(bytes: &mut Vec<u8>, byte_size: usize) {
    bytes.extend([0x49, 0x83, 0xfb, (byte_size * 8) as u8]); // cmp r11, width_bits
    bytes.extend([0x72, 0x02]); // jb +2 (an in-range count hops the ud2)
    bytes.extend([0x0f, 0x0b]); // ud2
}

/// Bytes of [`append_shift_count_trap_guard`]: cmp (4) + jb (2) + ud2 (2).
const SHIFT_COUNT_TRAP_GUARD_WIDTH: usize = 8;

/// A nested WRAPPING binary hands its PARENT the width-wrapped VALUE in r10
/// (the interpreter wraps AT THE NODE, decision 17; the store-truncation
/// shortcut only holds at the WRITE): extend r10 from the node's width by the
/// node's signedness. No-op at full width.
fn append_wrapping_node_width_extension(
    bytes: &mut Vec<u8>,
    byte_width: usize,
    operands_signed: bool,
) {
    match (byte_width, operands_signed) {
        (1, false) => bytes.extend([0x4d, 0x0f, 0xb6, 0xd2]), // movzx r10, r10b
        (2, false) => bytes.extend([0x4d, 0x0f, 0xb7, 0xd2]), // movzx r10, r10w
        (4, false) => bytes.extend([0x45, 0x89, 0xd2]),       // mov r10d, r10d
        (1, true) => bytes.extend([0x4d, 0x0f, 0xbe, 0xd2]),  // movsx r10, r10b
        (2, true) => bytes.extend([0x4d, 0x0f, 0xbf, 0xd2]),  // movsx r10, r10w
        (4, true) => bytes.extend([0x4d, 0x63, 0xd2]),        // movsxd r10, r10d
        _ => {}
    }
}

/// Bytes of [`append_wrapping_node_width_extension`]: 4, except 3 for the
/// width-4 forms and 0 at full width.
fn wrapping_node_width_extension_width(byte_width: usize) -> usize {
    match byte_width {
        4 => 3,
        1 | 2 => 4,
        _ => 0,
    }
}

fn float_policy_applies(operator: StateGuardOperator, domain: ArithmeticDomain) -> bool {
    matches!(
        domain,
        ArithmeticDomain::Saturating | ArithmeticDomain::Trapping
    ) && matches!(
        operator,
        StateGuardOperator::Add
            | StateGuardOperator::Subtract
            | StateGuardOperator::Multiply
            | StateGuardOperator::Divide
    )
}

#[derive(Clone, Copy)]
enum FloatPolicySource {
    Result,
    Left,
    Right,
}

/// Copy one raw f32/f64 bit pattern to rax and clear its sign bit. The F5
/// policy guard classifies floats entirely as unsigned integers: below/equal/
/// above the positive-infinity pattern means finite/infinite/NaN.
fn append_float_abs_to_rax(bytes: &mut Vec<u8>, source: FloatPolicySource, byte_size: usize) {
    match source {
        FloatPolicySource::Result => bytes.extend([0x4c, 0x89, 0xd0]), // mov rax,r10
        FloatPolicySource::Left => bytes.extend([0x4c, 0x89, 0xc0]),   // mov rax,r8
        FloatPolicySource::Right => bytes.extend([0x4c, 0x89, 0xd8]),  // mov rax,r11
    }
    if byte_size > 4 {
        bytes.extend([0x48, 0x0f, 0xba, 0xf0, 0x3f]); // btr rax,63
    } else {
        bytes.push(0x25); // and eax,0x7fff_ffff
        bytes.extend(0x7fff_ffff_u32.to_le_bytes());
    }
}

fn append_cmp_rax_r9(bytes: &mut Vec<u8>) {
    bytes.extend([0x4c, 0x39, 0xc8]); // cmp rax,r9
}

fn append_policy_branch_placeholder(bytes: &mut Vec<u8>, opcode: u8) -> usize {
    let start = bytes.len();
    bytes.extend([0x0f, opcode, 0, 0, 0, 0]);
    start
}

fn patch_policy_branch(
    bytes: &mut [u8],
    branch_start: usize,
    target: usize,
) -> Result<(), Diagnostic> {
    let displacement = target as isize - (branch_start + 6) as isize;
    let displacement = rel32(displacement)?;
    bytes[branch_start + 2..branch_start + 6].copy_from_slice(&displacement.to_le_bytes());
    Ok(())
}

/// F5 float-arithmetic policy guard. Entry: r10=result bits, r8=preserved
/// left bits, r11=right bits. Exit: r10 is unchanged or clamped; r8/r9/r11
/// and rax are scratch. The branch targets are patched from the emitted byte
/// stream, so the width twin can use this function's actual length.
fn float_policy_guard_bytes(
    domain: ArithmeticDomain,
    operator: StateGuardOperator,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if !float_policy_applies(operator, domain) {
        return Ok(Vec::new());
    }

    let (inf_bits, max_bits, sign_bits) = if byte_size > 4 {
        (
            0x7ff0_0000_0000_0000_u64,
            0x7fef_ffff_ffff_ffff_u64,
            0x8000_0000_0000_0000_u64,
        )
    } else {
        (0x7f80_0000_u64, 0x7f7f_ffff_u64, 0x8000_0000_u64)
    };
    let mut bytes = Vec::new();
    bytes.extend([0x49, 0xb9]); // mov r9,imm64 (positive infinity bits)
    bytes.extend(inf_bits.to_le_bytes());
    append_float_abs_to_rax(&mut bytes, FloatPolicySource::Result, byte_size);
    append_cmp_rax_r9(&mut bytes);

    match domain {
        ArithmeticDomain::Saturating => {
            let mut end_branches = Vec::new();
            // Only an exactly infinite result is magnitude overflow. Finite
            // results and NaNs pass through (invalid remains a Finite duty).
            end_branches.push(append_policy_branch_placeholder(&mut bytes, 0x85)); // jne end

            append_float_abs_to_rax(&mut bytes, FloatPolicySource::Right, byte_size);
            if operator == StateGuardOperator::Divide {
                bytes.extend([0x48, 0x83, 0xf8, 0x00]); // cmp rax,0
                // Division by +/-0 keeps IEEE infinity; it is not overflow.
                end_branches.push(append_policy_branch_placeholder(&mut bytes, 0x84)); // je end
            }
            append_cmp_rax_r9(&mut bytes);
            end_branches.push(append_policy_branch_placeholder(&mut bytes, 0x83)); // jae end

            append_float_abs_to_rax(&mut bytes, FloatPolicySource::Left, byte_size);
            append_cmp_rax_r9(&mut bytes);
            end_branches.push(append_policy_branch_placeholder(&mut bytes, 0x83)); // jae end

            // Clamp to MAX_FINITE with the result's sign.
            bytes.extend([0x49, 0xb9]);
            bytes.extend(sign_bits.to_le_bytes());
            bytes.extend([0x4d, 0x21, 0xca]); // and r10,r9
            bytes.extend([0x49, 0xb9]);
            bytes.extend(max_bits.to_le_bytes());
            bytes.extend([0x4d, 0x09, 0xca]); // or r10,r9

            let end = bytes.len();
            for branch in end_branches {
                patch_policy_branch(&mut bytes, branch, end)?;
            }
        }
        ArithmeticDomain::Trapping => {
            let mut end_branches = Vec::new();
            // Finite result: done. NaN jumps over the infinity case.
            end_branches.push(append_policy_branch_placeholder(&mut bytes, 0x82)); // jb end
            let nan_branch = append_policy_branch_placeholder(&mut bytes, 0x87); // ja nan

            // Infinite result is legal only when an input was already Inf/NaN.
            append_float_abs_to_rax(&mut bytes, FloatPolicySource::Left, byte_size);
            append_cmp_rax_r9(&mut bytes);
            end_branches.push(append_policy_branch_placeholder(&mut bytes, 0x83)); // jae end
            append_float_abs_to_rax(&mut bytes, FloatPolicySource::Right, byte_size);
            append_cmp_rax_r9(&mut bytes);
            end_branches.push(append_policy_branch_placeholder(&mut bytes, 0x83)); // jae end
            bytes.extend([0x0f, 0x0b]); // ud2: overflow or divide-by-zero

            let nan = bytes.len();
            patch_policy_branch(&mut bytes, nan_branch, nan)?;
            // NaN propagation is legal only when an input was already NaN.
            append_float_abs_to_rax(&mut bytes, FloatPolicySource::Left, byte_size);
            append_cmp_rax_r9(&mut bytes);
            end_branches.push(append_policy_branch_placeholder(&mut bytes, 0x87)); // ja end
            append_float_abs_to_rax(&mut bytes, FloatPolicySource::Right, byte_size);
            append_cmp_rax_r9(&mut bytes);
            end_branches.push(append_policy_branch_placeholder(&mut bytes, 0x87)); // ja end
            bytes.extend([0x0f, 0x0b]); // ud2: invalid operation

            let end = bytes.len();
            for branch in end_branches {
                patch_policy_branch(&mut bytes, branch, end)?;
            }
        }
        _ => unreachable!("policy applicability gated above"),
    }
    Ok(bytes)
}

/// Floating-point binary op (f64/f32) that reuses the integer operand pipeline:
/// the operand bit patterns are already loaded in r10 (left) and r11 (right).
/// Move them into xmm0/xmm1, run the SSE arithmetic op, then move the result
/// bits back to r10 so the shared store path writes them out. `byte_size > 4`
/// selects f64 (`movq` + `*sd`); otherwise f32 (`movd` + `*ss`). Always the
/// base per-operator width plus any emitted policy guard; the domain-aware
/// width twin calls the same guard emitter.
fn append_runtime_float_binary_operation(
    bytes: &mut Vec<u8>,
    operator: StateGuardOperator,
    byte_size: usize,
    domain: ArithmeticDomain,
) -> Result<(), Diagnostic> {
    let wide = byte_size > 4;
    let guarded = float_policy_applies(operator, domain);
    if guarded {
        // The result overwrites r10. Keep the raw left operand for the policy
        // guard; r11 already keeps the raw right operand.
        bytes.extend([0x4d, 0x89, 0xd0]); // mov r8,r10
    }
    if wide {
        bytes.extend([0x66, 0x49, 0x0f, 0x6e, 0xc2]); // movq xmm0, r10
        bytes.extend([0x66, 0x49, 0x0f, 0x6e, 0xcb]); // movq xmm1, r11
    } else {
        bytes.extend([0x66, 0x41, 0x0f, 0x6e, 0xc2]); // movd xmm0, r10d
        bytes.extend([0x66, 0x41, 0x0f, 0x6e, 0xcb]); // movd xmm1, r11d
    }
    // F2 = scalar-double prefix (`*sd`), F3 = scalar-single (`*ss`).
    let scalar_prefix = if wide { 0xf2 } else { 0xf3 };
    let opcode = match operator {
        StateGuardOperator::Add => 0x58,      // addsd/addss
        StateGuardOperator::Subtract => 0x5c, // subsd/subss
        StateGuardOperator::Multiply => 0x59, // mulsd/mulss
        StateGuardOperator::Divide => 0x5e,   // divsd/divss
        // `maxsd a, b` / `minsd a, b` return b on unordered (NaN) or equal, so
        // they realize `if a > b { a } else { b }` (and the min mirror) --
        // which the interpreter's float min/max matches exactly. This is what
        // makes float min/max, and hence abs/clamp over floats, lower.
        StateGuardOperator::Max => 0x5f, // maxsd/maxss
        StateGuardOperator::Min => 0x5d, // minsd/minss
        // sqrt is UNARY, carried with both operands = x: `sqrtsd xmm0, xmm1`
        // computes sqrt(xmm1) = sqrt(x) into xmm0, so the shared final line
        // below (op on xmm0, xmm1) already produces the right result.
        StateGuardOperator::Sqrt => 0x51, // sqrtsd/sqrtss
        // COMPARISON into a 0/1 result in r10 (`let ok: bool = self.a >
        // self.b` with float operands), the aarch64 twin. `ucomis*` sets
        // ZF/PF/CF (unordered = all three): ordering picks the operand ORDER
        // so an unsigned-above condition is FALSE on unordered for free
        // (`>`/`>=` compare (xmm0,xmm1) + seta/setae; `<`/`<=` swap to
        // (xmm1,xmm0)); equality needs the parity dance (unordered sets ZF,
        // so a bare sete/setne would call NaN == NaN true) -- a short
        // branch-over pattern keeps it register-free. f32's 3-byte `ucomiss`
        // takes a 1-byte NOP pad so the sequence length stays f32/f64
        // identical (the relocation-offset invariant). Widths tracked by
        // `runtime_float_binary_operation_width` -- MUST stay in lockstep.
        StateGuardOperator::Equal
        | StateGuardOperator::NotEqual
        | StateGuardOperator::Greater
        | StateGuardOperator::GreaterOrEqual
        | StateGuardOperator::Less
        | StateGuardOperator::LessOrEqual
        | StateGuardOperator::GreaterUnsigned
        | StateGuardOperator::GreaterOrEqualUnsigned
        | StateGuardOperator::LessUnsigned
        | StateGuardOperator::LessOrEqualUnsigned => {
            let swapped = matches!(
                operator,
                StateGuardOperator::Less
                    | StateGuardOperator::LessOrEqual
                    | StateGuardOperator::LessUnsigned
                    | StateGuardOperator::LessOrEqualUnsigned
            );
            let modrm = if swapped { 0xc8 } else { 0xc1 }; // xmm1,xmm0 / xmm0,xmm1
            if wide {
                bytes.extend([0x66, 0x0f, 0x2e, modrm]); // ucomisd
            } else {
                bytes.extend([0x0f, 0x2e, modrm]); // ucomiss
                bytes.push(0x90); // pad: keep f32/f64 sequence lengths equal
            }
            match operator {
                StateGuardOperator::Equal => {
                    bytes.extend([0xb0, 0x00]); // mov al, 0
                    bytes.extend([0x7a, 0x04]); // jp  +4 (unordered -> false)
                    bytes.extend([0x75, 0x02]); // jne +2 (not equal -> false)
                    bytes.extend([0xb0, 0x01]); // mov al, 1
                }
                StateGuardOperator::NotEqual => {
                    bytes.extend([0xb0, 0x01]); // mov al, 1
                    bytes.extend([0x7a, 0x04]); // jp  +4 (unordered -> TRUE)
                    bytes.extend([0x75, 0x02]); // jne +2 (not equal -> true)
                    bytes.extend([0xb0, 0x00]); // mov al, 0
                }
                StateGuardOperator::Greater
                | StateGuardOperator::GreaterUnsigned
                | StateGuardOperator::Less
                | StateGuardOperator::LessUnsigned => {
                    bytes.extend([0x0f, 0x97, 0xc0]); // seta (CF=0 && ZF=0)
                }
                StateGuardOperator::GreaterOrEqual
                | StateGuardOperator::GreaterOrEqualUnsigned
                | StateGuardOperator::LessOrEqual
                | StateGuardOperator::LessOrEqualUnsigned => {
                    bytes.extend([0x0f, 0x93, 0xc0]); // setae (CF=0)
                }
                _ => unreachable!(),
            }
            bytes.extend([0x44, 0x0f, 0xb6, 0xd0]); // movzx r10d, al
            return Ok(());
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 runtime float binary operator `{operator:?}` is not implemented yet"
            )));
        }
    };
    bytes.extend([scalar_prefix, 0x0f, opcode, 0xc1]); // <op> xmm0, xmm1
    if wide {
        bytes.extend([0x66, 0x49, 0x0f, 0x7e, 0xc2]); // movq r10, xmm0
    } else {
        bytes.extend([0x66, 0x41, 0x0f, 0x7e, 0xc2]); // movd r10d, xmm0
    }
    if guarded {
        bytes.extend(float_policy_guard_bytes(domain, operator, byte_size)?);
    }
    Ok(())
}

/// Width of [`append_runtime_float_binary_operation`]: two operand moves
/// (5 each) + per operator -- the SSE op (4) + the result move (5) = 19 for
/// arithmetic/min/max/sqrt; comparisons are ucomis (4, f32 NOP-padded) +
/// setcc (3) or the equality branch pattern (8) + movzx (4). Identical for
/// f32 and f64 at every operator (the relocation-offset invariant). MUST
/// stay in lockstep with the emission.
fn runtime_float_binary_operation_width(operator: StateGuardOperator) -> usize {
    runtime_float_binary_operation_width_with_domain(operator, 8, ArithmeticDomain::Exact)
}

fn runtime_float_binary_operation_width_with_domain(
    operator: StateGuardOperator,
    byte_size: usize,
    domain: ArithmeticDomain,
) -> usize {
    let policy_width = if float_policy_applies(operator, domain) {
        3 + float_policy_guard_bytes(domain, operator, byte_size)
            .map(|bytes| bytes.len())
            .unwrap_or(0)
    } else {
        0
    };
    policy_width + runtime_float_binary_operation_width_base(operator)
}

fn runtime_float_binary_operation_width_base(operator: StateGuardOperator) -> usize {
    match operator {
        StateGuardOperator::Equal | StateGuardOperator::NotEqual => 10 + 4 + 8 + 4,
        StateGuardOperator::Greater
        | StateGuardOperator::GreaterOrEqual
        | StateGuardOperator::Less
        | StateGuardOperator::LessOrEqual
        | StateGuardOperator::GreaterUnsigned
        | StateGuardOperator::GreaterOrEqualUnsigned
        | StateGuardOperator::LessUnsigned
        | StateGuardOperator::LessOrEqualUnsigned => 10 + 4 + 3 + 4,
        _ => 19,
    }
}

fn runtime_binary_operation_width(operator: StateGuardOperator, byte_size: usize) -> usize {
    match operator {
        StateGuardOperator::Add
        | StateGuardOperator::And
        | StateGuardOperator::Or
        | StateGuardOperator::BitwiseAnd
        | StateGuardOperator::BitwiseOr
        | StateGuardOperator::BitwiseXor
        | StateGuardOperator::Subtract => 3,
        StateGuardOperator::Multiply => 4,
        // cmp (3) + cmov (4), same at 32-bit or 64-bit.
        StateGuardOperator::Max
        | StateGuardOperator::Min
        | StateGuardOperator::MaxUnsigned
        | StateGuardOperator::MinUnsigned => 7,
        // signed 32-bit: mov(3)+cdq(1)+idiv(3)+mov(3)=10; signed 64-bit: cqo(2)=11.
        // Narrow signed (i8/i16) prepends two movsx (8) to sign-extend the operands
        // to the 32-bit op width; see append_integer_divide_modulo_core.
        StateGuardOperator::Divide | StateGuardOperator::Modulo => {
            let sign_extend = if byte_size <= 2 { 8 } else { 0 };
            sign_extend + if byte_size <= 4 { 10 } else { 11 }
        }
        // unsigned: mov(3)+xor edx,edx(2)+div(3)+mov(3)=11 at either size.
        StateGuardOperator::DivideUnsigned | StateGuardOperator::ModuloUnsigned => 11,
        // mov c-reg, r11 (3) + shift r10, cl (3), same width at either size.
        StateGuardOperator::ShiftLeft
        | StateGuardOperator::ShiftRight
        | StateGuardOperator::ShiftRightLogical => 6,
        // cmp (3; 4 with the 0x66 prefix at 2-byte width) + setcc (3) + movzx (4).
        StateGuardOperator::Equal
        | StateGuardOperator::NotEqual
        | StateGuardOperator::Greater
        | StateGuardOperator::GreaterOrEqual
        | StateGuardOperator::Less
        | StateGuardOperator::LessOrEqual
        | StateGuardOperator::GreaterUnsigned
        | StateGuardOperator::GreaterOrEqualUnsigned
        | StateGuardOperator::LessUnsigned
        | StateGuardOperator::LessOrEqualUnsigned => {
            if byte_size == 2 {
                11
            } else {
                10
            }
        }
        _ => 0,
    }
}

fn append_input_delimiter_check(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    failure_branch_distance: isize,
) -> Result<(), Diagnostic> {
    append_load_al_from_r15(bytes, byte_offset)?;
    bytes.extend([0x3c, 10]); // cmp al, '\n'
    append_jcc_rel32(bytes, 0x84, 21)?; // je success
    bytes.extend([0x3c, 13]); // cmp al, '\r'
    append_jcc_rel32(bytes, 0x84, 13)?; // je success
    bytes.extend([0x3c, 0]); // cmp al, 0
    append_jcc_rel32(bytes, 0x84, 5)?; // je success
    append_jmp_rel32(bytes, failure_branch_distance)?;
    Ok(())
}

fn append_failure_branch(
    bytes: &mut Vec<u8>,
    operator: StateGuardOperator,
    failure_branch_distance: isize,
    is_float: bool,
) -> Result<(), Diagnostic> {
    // The guard jumps to the failure branch when the comparison is FALSE, so each
    // operator maps to its negation. Ordering uses signed (jl/jg/...) or unsigned
    // (jb/ja/...) conditions per the operand type.
    let opcode = match operator {
        StateGuardOperator::Equal => 0x85,                  // jne
        StateGuardOperator::NotEqual => 0x84,               // je
        StateGuardOperator::Greater => 0x8e,                // jle
        StateGuardOperator::GreaterOrEqual => 0x8c,         // jl
        StateGuardOperator::Less => 0x8d,                   // jge
        StateGuardOperator::LessOrEqual => 0x8f,            // jg
        StateGuardOperator::GreaterUnsigned => 0x86,        // jbe
        StateGuardOperator::GreaterOrEqualUnsigned => 0x82, // jb
        StateGuardOperator::LessUnsigned => 0x83,           // jae
        StateGuardOperator::LessOrEqualUnsigned => 0x87,    // ja
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 runtime compare operator `{operator:?}` is not implemented yet"
            )));
        }
    };
    // IEEE semantics for a NaN operand: every comparison is FALSE except `!=` (true).
    // `ucomis*` reports an unordered/NaN operand by setting PF=1 (alongside ZF=CF=1),
    // which the ZF/CF-only failure jcc above misreads as "equal". Prepend a parity
    // branch so NaN is routed correctly. This 6-byte `jp` sits BEFORE the main jcc, so
    // the main jcc's own rel32 is unchanged (both it and its target shift down by 6);
    // the float width functions account for the extra 6 bytes.
    if is_float {
        if matches!(operator, StateGuardOperator::NotEqual) {
            // `!=` on NaN is TRUE (guard succeeds): jump PAST the 6-byte `je` so NaN
            // falls through to the success arm instead of taking the equal-failure jump.
            append_jcc_rel32(bytes, 0x8a, 6)?; // jp over the je
        } else {
            // Every other operator is FALSE on NaN (guard fails): jump to the same
            // failure arm as the main jcc, which now sits 6 bytes further along.
            append_jcc_rel32(bytes, 0x8a, failure_branch_distance + 6)?; // jp to failure
        }
    }
    append_jcc_rel32(bytes, opcode, failure_branch_distance)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reg64 {
    R10,
    R11,
}

fn append_mov_reg_imm64(bytes: &mut Vec<u8>, register: Reg64, value: u64) {
    match register {
        Reg64::R10 => append_mov_r10_imm64(bytes, value),
        Reg64::R11 => {
            bytes.extend([0x49, 0xbb]);
            bytes.extend(value.to_le_bytes());
        }
    }
}

fn append_mov_rax_imm64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend([0x48, 0xb8]);
    bytes.extend(value.to_le_bytes());
}

fn append_mov_rdx_imm64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend([0x48, 0xba]);
    bytes.extend(value.to_le_bytes());
}

fn append_mov_r10_imm64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend([0x49, 0xba]);
    bytes.extend(value.to_le_bytes());
}

fn append_mov_r14_imm64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend([0x49, 0xbe]);
    bytes.extend(value.to_le_bytes());
}

/// The CROSS-REGION index base (place materializer): when a ScaledIndex
/// slot lives in a different region than the place's own base, r11 first
/// holds the INDEX region's base, then loads the index through itself --
/// no extra scratch register enters the discipline.
fn append_mov_r11_imm64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend([0x49, 0xbb]);
    bytes.extend(value.to_le_bytes());
}

/// 32-bit zero-extended index load through r11's own value (the
/// cross-region index base pattern; see `append_load_r11_from_r14`'s
/// width rationale).
fn append_load_r11_from_r11(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x45, 0x8b, 0x9b]); // mov r11d, [r11 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_mov_r15_imm64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend([0x49, 0xbf]);
    bytes.extend(value.to_le_bytes());
}

fn append_mov_reg_reg(bytes: &mut Vec<u8>, destination: Reg64, source: Reg64) {
    match (destination, source) {
        (Reg64::R10, Reg64::R10) => bytes.extend([0x4d, 0x89, 0xd2]),
        (Reg64::R10, Reg64::R11) => bytes.extend([0x4d, 0x89, 0xda]),
        (Reg64::R11, Reg64::R10) => bytes.extend([0x4d, 0x89, 0xd3]),
        (Reg64::R11, Reg64::R11) => bytes.extend([0x4d, 0x89, 0xdb]),
    }
}

fn append_push_r10(bytes: &mut Vec<u8>) {
    bytes.extend([0x41, 0x52]); // push r10
}

fn append_pop_r10(bytes: &mut Vec<u8>) {
    bytes.extend([0x41, 0x5a]); // pop r10
}

// --- Helpers for the runtime-length text-append memcpy (`rep movsb`) ---

fn append_mov_rcx_imm64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend([0x48, 0xb9]); // mov rcx, imm64
    bytes.extend(value.to_le_bytes());
}

fn append_load_rax_from_rcx(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x48, 0x8b, 0x81]); // mov rax, [rcx + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_load_rcx_from_rcx(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x48, 0x8b, 0x89]); // mov rcx, [rcx + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_mov_r10_r14(bytes: &mut Vec<u8>) {
    bytes.extend([0x4d, 0x89, 0xf2]); // mov r10, r14
}

fn append_add_r10_r11(bytes: &mut Vec<u8>) {
    bytes.extend([0x4d, 0x01, 0xda]); // add r10, r11
}

fn append_add_r10_imm32(bytes: &mut Vec<u8>, value: usize) -> Result<(), Diagnostic> {
    let value = i32::try_from(value).map_err(|_| {
        Diagnostic::error(format!("X86_64 encoder cannot add offset `{value}` to r10"))
    })?;
    bytes.extend([0x49, 0x81, 0xc2]); // add r10, imm32
    bytes.extend(value.to_le_bytes());
    Ok(())
}

fn append_add_r11_rcx(bytes: &mut Vec<u8>) {
    bytes.extend([0x49, 0x01, 0xcb]); // add r11, rcx
}

fn append_add_r11_imm32(bytes: &mut Vec<u8>, value: usize) -> Result<(), Diagnostic> {
    let value = i32::try_from(value).map_err(|_| {
        Diagnostic::error(format!("X86_64 encoder cannot add offset `{value}` to r11"))
    })?;
    bytes.extend([0x49, 0x81, 0xc3]); // add r11, imm32
    bytes.extend(value.to_le_bytes());
    Ok(())
}

fn append_mov_r11_rcx(bytes: &mut Vec<u8>) {
    bytes.extend([0x49, 0x89, 0xcb]); // mov r11, rcx
}

fn append_load_r11_from_r15(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x8b, 0x9f]); // mov r11, [r15 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

/// Load an array INDEX into r11 from `[r15 + disp32]` as a 32-bit zero-extended
/// value (`mov r11d`). An index always fits in 32 bits and is non-negative, but
/// its frame slot may be a 4-byte `i32` whose adjacent bytes hold an unrelated
/// value; a 64-bit load would splice that garbage into the high half of the index
/// and compute a wild element address. (Same rationale as the r14-based index
/// load; the length/pointer r15 loads stay 64-bit.)
fn append_load_index_r11_from_r15(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x45, 0x8b, 0x9f]); // mov r11d, [r15 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_store_r11_to_r15(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x89, 0x9f]); // mov [r15 + disp32], r11
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_mov_rsi_rax(bytes: &mut Vec<u8>) {
    bytes.extend([0x48, 0x89, 0xc6]); // mov rsi, rax
}

fn append_mov_rdi_r10(bytes: &mut Vec<u8>) {
    bytes.extend([0x4c, 0x89, 0xd7]); // mov rdi, r10
}

fn append_rep_movsb(bytes: &mut Vec<u8>) {
    bytes.extend([0xf3, 0xa4]); // rep movsb (copy rcx bytes [rsi]->[rdi], DF=0)
}

fn append_push_rsi_rdi(bytes: &mut Vec<u8>) {
    bytes.extend([0x56, 0x57]); // push rsi ; push rdi
}

fn append_pop_rdi_rsi(bytes: &mut Vec<u8>) {
    bytes.extend([0x5f, 0x5e]); // pop rdi ; pop rsi
}

fn append_mov_r12d_imm32(bytes: &mut Vec<u8>, value: u32) -> Result<(), Diagnostic> {
    bytes.extend([0x41, 0xbc]);
    bytes.extend(value.to_le_bytes());
    Ok(())
}

fn append_cmp_r12d_imm32(bytes: &mut Vec<u8>, value: u32) -> Result<(), Diagnostic> {
    bytes.extend([0x41, 0x81, 0xfc]);
    bytes.extend(value.to_le_bytes());
    Ok(())
}

fn append_add_r14_r11(bytes: &mut Vec<u8>) {
    bytes.extend([0x4d, 0x01, 0xde]); // add r14, r11
}

fn append_load_al_from_r15(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x41, 0x8a, 0x87]);
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_load_r15_from_r14(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x8b, 0xbe]); // mov r15, [r14 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_load_r15_from_r15(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x8b, 0xbf]); // mov r15, [r15 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_add_r15_imm32(bytes: &mut Vec<u8>, value: usize) -> Result<(), Diagnostic> {
    let value = i32::try_from(value).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot add offset `{value}` to r15"
        ))
    })?;
    bytes.extend([0x49, 0x81, 0xc7]); // add r15, imm32
    bytes.extend(value.to_le_bytes());
    Ok(())
}

fn append_store_r15_to_r14(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x89, 0xbe]); // mov [r14 + disp32], r15
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_load_r10_from_r14(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x8b, 0x96]); // mov r10, [r14 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_load_r11_from_r14(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    // 32-bit load (`mov r11d`), which zero-extends into the full r11. This is the
    // array-INDEX load (every caller follows it with `imul r11, element_scale`).
    // An index is a non-negative array offset that always fits in 32 bits, but its
    // frame slot may be a 4-byte `i32` whose adjacent 4 bytes hold an unrelated
    // value; a 64-bit load would splice that garbage into the high half of the
    // index and compute a wild element address. Reading exactly 4 zero-extended
    // bytes is correct for both `i32` and (small) `usize` indices.
    bytes.extend([0x45, 0x8b, 0x9e]); // mov r11d, [r14 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_mov_r14_r15(bytes: &mut Vec<u8>) {
    bytes.extend([0x4d, 0x89, 0xfe]); // mov r14, r15
}

fn append_add_r15_r11(bytes: &mut Vec<u8>) {
    bytes.extend([0x4d, 0x01, 0xdf]); // add r15, r11
}

fn append_load_rdx_from_r10(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x49, 0x8b, 0x92]);
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_load_r10_from_r10(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x8b, 0x92]);
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_load_r8_from_r10(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x8b, 0x82]);
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_load_rax_from_r15(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x49, 0x8b, 0x87]); // mov rax, [r15 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_load_rcx_from_r15(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x49, 0x8b, 0x8f]); // mov rcx, [r15 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_load_r14_from_r15(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x8b, 0xb7]); // mov r14, [r15 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_load_r14_from_r14(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x8b, 0xb6]); // mov r14, [r14 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_imul_r11_imm32(bytes: &mut Vec<u8>, value: i32) {
    bytes.extend([0x4d, 0x69, 0xdb]); // imul r11, r11, imm32
    bytes.extend(value.to_le_bytes());
}

// r10 = the SECOND index scratch (the double-index rung): same 32-bit
// zero-extended load + scale discipline as r11's family.
// (append_mov_r10_imm64 already exists above.)

fn append_load_index_r10_from_r14(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x45, 0x8b, 0x96]); // mov r10d, [r14 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_load_index_r10_from_r15(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x45, 0x8b, 0x97]); // mov r10d, [r15 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_load_index_r10_from_r10(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x45, 0x8b, 0x92]); // mov r10d, [r10 + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_imul_r10_imm32(bytes: &mut Vec<u8>, value: i32) {
    bytes.extend([0x4d, 0x69, 0xd2]); // imul r10, r10, imm32
    bytes.extend(value.to_le_bytes());
}

fn append_add_r14_r10(bytes: &mut Vec<u8>) {
    bytes.extend([0x4d, 0x01, 0xd6]); // add r14, r10
}

fn append_add_r15_r10(bytes: &mut Vec<u8>) {
    bytes.extend([0x4d, 0x01, 0xd7]); // add r15, r10
}

fn append_add_rax_r11(bytes: &mut Vec<u8>) {
    // add rax, r11 -- REX.W+REX.R (0x4c), opcode 0x01, ModRM 11 reg=r11(011) rm=rax(000) = 0xd8
    bytes.extend([0x4c, 0x01, 0xd8]);
}

fn append_store_r15_to_rax(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4c, 0x89, 0xb8]); // mov [rax + disp32], r15
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_store_r11_to_rax(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4c, 0x89, 0x98]); // mov [rax + disp32], r11
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_load_r11_from_rax(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4c, 0x8b, 0x98]); // mov r11, [rax + disp32]
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_mov_rax_r15(bytes: &mut Vec<u8>) {
    // mov rax, r15 -- REX.W+REX.R(no)+REX.B(r15 src as r/m): 0x4c 0x89 0xf8
    bytes.extend([0x4c, 0x89, 0xf8]);
}

fn append_mov_rax_r10(bytes: &mut Vec<u8>) {
    // mov rax, r10 -- 89 /r with r10 in the reg field (REX.R) and rax in r/m.
    bytes.extend([0x4c, 0x89, 0xd0]);
}

/// Byte count of [`append_mov_rax_r10`].
const MOV_RAX_R10_WIDTH: usize = 3;

fn element_scale(element_byte_size: usize) -> Result<i32, Diagnostic> {
    i32::try_from(element_byte_size).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot scale runtime index by element size `{element_byte_size}`"
        ))
    })
}

fn append_load_reg_from_rax(
    bytes: &mut Vec<u8>,
    destination: Reg64,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    match (destination, byte_size) {
        // mov r10{b,w,d,}, [rax + disp32] -- ModRM mod=10 reg=r10(010) rm=rax(000) = 0x90
        (Reg64::R10, 1) => bytes.extend([0x44, 0x8a, 0x90]),
        (Reg64::R10, 2) => bytes.extend([0x66, 0x44, 0x8b, 0x90]),
        (Reg64::R10, 4) => bytes.extend([0x44, 0x8b, 0x90]),
        (Reg64::R10, 8) => bytes.extend([0x4c, 0x8b, 0x90]),
        // mov r11{b,w,d,}, [rax + disp32] -- ModRM mod=10 reg=r11(011) rm=rax(000) = 0x98
        (Reg64::R11, 1) => bytes.extend([0x44, 0x8a, 0x98]),
        (Reg64::R11, 2) => bytes.extend([0x66, 0x44, 0x8b, 0x98]),
        (Reg64::R11, 4) => bytes.extend([0x44, 0x8b, 0x98]),
        (Reg64::R11, 8) => bytes.extend([0x4c, 0x8b, 0x98]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot load {byte_size}-byte runtime operands yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_load_rax_from_r14(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    match byte_size {
        1 => bytes.extend([0x41, 0x8a, 0x86]),
        2 => bytes.extend([0x66, 0x41, 0x8b, 0x86]),
        4 => bytes.extend([0x41, 0x8b, 0x86]),
        8 => bytes.extend([0x49, 0x8b, 0x86]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot load {byte_size}-byte storage values yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn load_rax_from_r14_width(byte_size: usize) -> usize {
    match byte_size {
        2 => 8,
        1 | 4 | 8 => 7,
        _ => 7,
    }
}

fn append_load_reg_from_r15(
    bytes: &mut Vec<u8>,
    destination: Reg64,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    match (destination, byte_size) {
        (Reg64::R10, 1) => bytes.extend([0x45, 0x8a, 0x97]),
        (Reg64::R10, 2) => bytes.extend([0x66, 0x45, 0x8b, 0x97]),
        (Reg64::R10, 4) => bytes.extend([0x45, 0x8b, 0x97]),
        (Reg64::R10, 8) => bytes.extend([0x4d, 0x8b, 0x97]),
        (Reg64::R11, 1) => bytes.extend([0x45, 0x8a, 0x9f]),
        (Reg64::R11, 2) => bytes.extend([0x66, 0x45, 0x8b, 0x9f]),
        (Reg64::R11, 4) => bytes.extend([0x45, 0x8b, 0x9f]),
        (Reg64::R11, 8) => bytes.extend([0x4d, 0x8b, 0x9f]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot load {byte_size}-byte runtime operands yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

/// The r14-base twin of `append_load_reg_from_r15` (ModRM r/m = r14): the
/// place-compare materializer walks its LEFT operand's address in r14 (the
/// CopyPlaces source discipline) and loads the operand through it.
fn append_load_reg_from_r14(
    bytes: &mut Vec<u8>,
    destination: Reg64,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    match (destination, byte_size) {
        (Reg64::R10, 1) => bytes.extend([0x45, 0x8a, 0x96]),
        (Reg64::R10, 2) => bytes.extend([0x66, 0x45, 0x8b, 0x96]),
        (Reg64::R10, 4) => bytes.extend([0x45, 0x8b, 0x96]),
        (Reg64::R10, 8) => bytes.extend([0x4d, 0x8b, 0x96]),
        (Reg64::R11, 1) => bytes.extend([0x45, 0x8a, 0x9e]),
        (Reg64::R11, 2) => bytes.extend([0x66, 0x45, 0x8b, 0x9e]),
        (Reg64::R11, 4) => bytes.extend([0x45, 0x8b, 0x9e]),
        (Reg64::R11, 8) => bytes.extend([0x4d, 0x8b, 0x9e]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot load {byte_size}-byte runtime operands yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_store_rax_to_r15(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    match byte_size {
        1 => bytes.extend([0x41, 0x88, 0x87]),
        2 => bytes.extend([0x66, 0x41, 0x89, 0x87]),
        4 => bytes.extend([0x41, 0x89, 0x87]),
        8 => bytes.extend([0x49, 0x89, 0x87]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot store {byte_size}-byte runtime values yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_store_r10_to_r15(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = i32::try_from(byte_offset)
        .map_err(|_| Diagnostic::error("RFLAGS snapshot destination offset exceeds i32"))?;
    // mov qword ptr [r15+disp32], r10
    bytes.extend([0x4d, 0x89, 0x97]);
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_store_r14_to_r15(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    bytes.extend([0x4d, 0x89, 0xb7]);
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_store_r10_to_r14(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    match byte_size {
        // mov [r14+disp32], r10{b,w,d,} -- ModRM reg=r10, r/m=r14
        1 => bytes.extend([0x45, 0x88, 0x96]),
        2 => bytes.extend([0x66, 0x45, 0x89, 0x96]),
        4 => bytes.extend([0x45, 0x89, 0x96]),
        8 => bytes.extend([0x4d, 0x89, 0x96]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot store {byte_size}-byte runtime values yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_xchg_r10_to_r14(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    match byte_size {
        // XCHG with a memory operand is implicitly locked.
        1 => bytes.extend([0x45, 0x86, 0x96]),
        2 => bytes.extend([0x66, 0x45, 0x87, 0x96]),
        4 => bytes.extend([0x45, 0x87, 0x96]),
        8 => bytes.extend([0x4d, 0x87, 0x96]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot atomically exchange a {byte_size}-byte runtime value"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_cmp_r10_r11(bytes: &mut Vec<u8>, byte_size: usize) -> Result<(), Diagnostic> {
    match byte_size {
        1 => bytes.extend([0x45, 0x38, 0xda]),
        2 => bytes.extend([0x66, 0x45, 0x39, 0xda]),
        4 => bytes.extend([0x45, 0x39, 0xda]),
        8 => bytes.extend([0x4d, 0x39, 0xda]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 MVP encoder cannot compare {byte_size}-byte runtime values yet"
            )));
        }
    }
    Ok(())
}

fn append_jcc_rel32(
    bytes: &mut Vec<u8>,
    opcode: u8,
    byte_distance: isize,
) -> Result<(), Diagnostic> {
    let displacement = rel32(byte_distance)?;
    bytes.extend([0x0f, opcode]);
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn append_jmp_rel32(bytes: &mut Vec<u8>, byte_distance: isize) -> Result<(), Diagnostic> {
    let displacement = rel32(byte_distance)?;
    bytes.push(0xe9);
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

fn for_each_runtime_copy_chunk(
    source_base_offset: usize,
    target_base_offset: usize,
    byte_count: usize,
    mut visit: impl FnMut(usize, usize) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    let mut remaining = byte_count;
    let mut offset = 0usize;

    while remaining > 0 {
        let source_offset = source_base_offset + offset;
        let target_offset = target_base_offset + offset;
        let chunk_size =
            if remaining >= 8 && source_offset.is_multiple_of(8) && target_offset.is_multiple_of(8)
            {
                8
            } else if remaining >= 4
                && source_offset.is_multiple_of(4)
                && target_offset.is_multiple_of(4)
            {
                4
            } else {
                1
            };

        visit(offset, chunk_size)?;
        offset += chunk_size;
        remaining -= chunk_size;
    }

    Ok(())
}

fn load_width(byte_size: usize) -> usize {
    match byte_size {
        1 | 4 | 8 => 7,
        // The 2-byte form is the 4-byte form plus the 0x66 operand-size prefix.
        2 => 8,
        _ => 0,
    }
}

fn store_width(byte_size: usize) -> usize {
    match byte_size {
        1 | 4 | 8 => 7,
        // The 2-byte form is the 4-byte form plus the 0x66 operand-size prefix.
        2 => 8,
        _ => 0,
    }
}

fn immediate_i32<T: InstructionOperandLike>(
    operands: &[T],
    index: usize,
    label: &str,
) -> Result<i32, Diagnostic> {
    let Some(operand) = operands.get(index) else {
        return Err(Diagnostic::error(format!(
            "cannot encode X86_64 host call: missing {label}"
        )));
    };
    let Some(value) = operand.immediate_integer() else {
        return Err(Diagnostic::error(format!(
            "cannot encode X86_64 host call: {label} is not an immediate integer"
        )));
    };
    i32::try_from(value).map_err(|_| {
        Diagnostic::error(format!(
            "cannot encode X86_64 host call: {label} value {value} does not fit i32"
        ))
    })
}

fn disp32(value: usize) -> Result<i32, Diagnostic> {
    i32::try_from(value).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 MVP encoder cannot address displacement `{value}`"
        ))
    })
}

fn rel32(value: isize) -> Result<i32, Diagnostic> {
    i32::try_from(value).map_err(|_| {
        Diagnostic::error(format!(
            "X86_64 branch target is out of rel32 range: {value} byte(s)"
        ))
    })
}

/// Emit `lock xadd [r14 + disp32], r10` at the given operand width. XADD swaps
/// then adds: it loads the prior `[mem]` into the source register (r10) and
/// stores `[mem] + r10` back, all as ONE atomic read-modify-write under the
/// LOCK prefix -- exactly `fetch_add`'s contract (r10 ends with the OLD value).
/// Caller sets r10 = the delta and r14 = the atomic field's base BEFORE this.
/// Used by `encode_atomic_fetch_add`; byte-verified by `atomic_tests` below.
fn append_lock_xadd_r10_to_r14(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    // F0 = LOCK. REX picks operand width (W) + r10 (R) + r14 (B). XADD is
    // `0F C1 /r` (or `0F C0 /r` for 8-bit). ModRM 0x96 = mod=10 (disp32),
    // reg=r10&7=2, r/m=r14&7=6.
    match byte_size {
        1 => bytes.extend([0xf0, 0x45, 0x0f, 0xc0, 0x96]),
        2 => bytes.extend([0xf0, 0x66, 0x45, 0x0f, 0xc1, 0x96]),
        4 => bytes.extend([0xf0, 0x45, 0x0f, 0xc1, 0x96]),
        8 => bytes.extend([0xf0, 0x4d, 0x0f, 0xc1, 0x96]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 encoder cannot LOCK xadd {byte_size}-byte atomics yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

/// Emitted byte count of [`append_lock_xadd_r10_to_r14`] (opcode block + disp32).
fn lock_xadd_r10_to_r14_width(byte_size: usize) -> usize {
    let opcode = match byte_size {
        1 | 4 => 5,
        2 => 6,
        8 => 5,
        _ => 5,
    };
    opcode + 4
}

/// Negate r10 at the atomic operand width. XADD then adds this truncated
/// two's-complement value, implementing wrapping fetch_sub while leaving the
/// prior memory value in r10.
fn append_negate_r10(bytes: &mut Vec<u8>, byte_size: usize) -> Result<(), Diagnostic> {
    match byte_size {
        1 => bytes.extend([0x41, 0xf6, 0xda]),
        2 => bytes.extend([0x66, 0x41, 0xf7, 0xda]),
        4 => bytes.extend([0x41, 0xf7, 0xda]),
        8 => bytes.extend([0x49, 0xf7, 0xda]),
        other => {
            return Err(Diagnostic::error(format!(
                "X86_64 encoder cannot negate a {other}-byte atomic operand"
            )));
        }
    }
    Ok(())
}

fn negate_r10_width(byte_size: usize) -> usize {
    match byte_size {
        2 => 4,
        1 | 4 | 8 => 3,
        _ => 3,
    }
}

/// `LOCK CMPXCHG [r14+disp32], r10`: compare rax with the place; if equal store
/// r10 (ZF=1), else load the place into rax (ZF=0). Identical layout to
/// `append_lock_xadd_r10_to_r14` but with the CMPXCHG opcode (`0F B1`, or
/// `0F B0` for 8-bit). Used by `encode_atomic_compare_exchange`; byte-verified
/// by `atomic_tests`.
fn append_lock_cmpxchg_r10_to_r14(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    let displacement = disp32(byte_offset)?;
    match byte_size {
        1 => bytes.extend([0xf0, 0x45, 0x0f, 0xb0, 0x96]),
        2 => bytes.extend([0xf0, 0x66, 0x45, 0x0f, 0xb1, 0x96]),
        4 => bytes.extend([0xf0, 0x45, 0x0f, 0xb1, 0x96]),
        8 => bytes.extend([0xf0, 0x4d, 0x0f, 0xb1, 0x96]),
        _ => {
            return Err(Diagnostic::error(format!(
                "X86_64 encoder cannot LOCK cmpxchg {byte_size}-byte atomics yet"
            )));
        }
    }
    bytes.extend(displacement.to_le_bytes());
    Ok(())
}

/// Emitted byte count of [`append_lock_cmpxchg_r10_to_r14`] (opcode + disp32).
/// Same layout as `lock_xadd_r10_to_r14_width` (only the opcode byte differs).
fn lock_cmpxchg_r10_to_r14_width(byte_size: usize) -> usize {
    lock_xadd_r10_to_r14_width(byte_size)
}

#[cfg(test)]
mod runtime_operand_load_tests {
    use super::*;

    #[test]
    fn rax_based_u16_operands_use_word_loads_in_width_lockstep() {
        for (register, prefix) in [
            (Reg64::R10, [0x66, 0x44, 0x8b, 0x90]),
            (Reg64::R11, [0x66, 0x44, 0x8b, 0x98]),
        ] {
            let mut bytes = Vec::new();
            append_load_reg_from_rax(&mut bytes, register, 24, 2)
                .expect("u16 pointee load should encode");
            assert_eq!(&bytes[..4], &prefix);
            assert_eq!(&bytes[4..], &24i32.to_le_bytes());
            assert_eq!(bytes.len(), load_width(2));
        }
    }
}

#[cfg(test)]
mod float_to_integer_policy_tests {
    use super::*;

    #[test]
    fn policy_sequences_stay_in_width_lockstep() {
        for source_byte_size in [4usize, 8] {
            for target_byte_size in [1usize, 2, 4, 8] {
                for target_signed in [false, true] {
                    let mut trapping = Vec::new();
                    append_float_to_int_trap(
                        &mut trapping,
                        source_byte_size,
                        target_byte_size,
                        target_signed,
                    );
                    assert_eq!(
                        trapping.len(),
                        float_to_int_trap_width(source_byte_size, target_byte_size, target_signed,),
                        "Trapping f{source_byte_size}->int{target_byte_size} signed={target_signed} width"
                    );

                    let mut saturating = Vec::new();
                    append_float_to_int_saturating(
                        &mut saturating,
                        source_byte_size,
                        target_byte_size,
                        target_signed,
                    );
                    assert_eq!(
                        saturating.len(),
                        float_to_int_saturating_width(
                            source_byte_size,
                            target_byte_size,
                            target_signed,
                        ),
                        "Saturating f{source_byte_size}->int{target_byte_size} signed={target_signed} width"
                    );
                }
            }
        }
    }

    #[test]
    fn exact_conversion_keeps_the_zero_guard_cost() {
        assert_eq!(
            runtime_convert_operation_width(8, 4, true, false, false, true, false, false),
            10,
        );
        assert_eq!(
            runtime_convert_operation_width(8, 4, true, false, false, true, true, false),
            5 + float_to_int_trap_width(8, 4, true),
        );
        assert_eq!(
            runtime_convert_operation_width(8, 4, true, false, false, true, false, true),
            5 + float_to_int_saturating_width(8, 4, true),
        );
    }

    #[test]
    fn bounds_describe_truncation_not_only_integer_membership() {
        let (upper, lower, lower_inclusive) = float_to_int_bounds(8, 4, true);
        assert_eq!(f64::from_bits(upper), 2147483648.0);
        assert_eq!(f64::from_bits(lower), -2147483649.0);
        assert!(!lower_inclusive, "-2147483648.5 truncates into i32");

        let (upper, lower, lower_inclusive) = float_to_int_bounds(4, 4, true);
        assert_eq!(f32::from_bits(upper as u32), 2147483648.0);
        assert_eq!(f32::from_bits(lower as u32), -2147483648.0);
        assert!(lower_inclusive, "f32 cannot represent i32::MIN - 1");

        let (upper, lower, lower_inclusive) = float_to_int_bounds(8, 4, false);
        assert_eq!(f64::from_bits(upper), 4294967296.0);
        assert_eq!(f64::from_bits(lower), -1.0);
        assert!(!lower_inclusive, "-0.5 truncates into u32");
    }
}

#[cfg(test)]
mod float_arithmetic_policy_tests {
    use super::*;

    #[test]
    fn policy_sequences_stay_in_width_lockstep() {
        for byte_size in [4usize, 8] {
            for operator in [
                StateGuardOperator::Add,
                StateGuardOperator::Subtract,
                StateGuardOperator::Multiply,
                StateGuardOperator::Divide,
            ] {
                for domain in [
                    ArithmeticDomain::Exact,
                    ArithmeticDomain::Saturating,
                    ArithmeticDomain::Trapping,
                ] {
                    let mut bytes = Vec::new();
                    append_runtime_float_binary_operation(&mut bytes, operator, byte_size, domain)
                        .expect("encode float operation");
                    assert_eq!(
                        bytes.len(),
                        runtime_float_binary_operation_width_with_domain(
                            operator, byte_size, domain,
                        ),
                        "f{} {operator:?} {domain:?} width",
                        byte_size * 8,
                    );
                }
            }
        }
    }

    #[test]
    fn policy_branches_target_emitted_labels() {
        for byte_size in [4usize, 8] {
            for operator in [
                StateGuardOperator::Add,
                StateGuardOperator::Subtract,
                StateGuardOperator::Multiply,
                StateGuardOperator::Divide,
            ] {
                for domain in [ArithmeticDomain::Saturating, ArithmeticDomain::Trapping] {
                    let bytes = float_policy_guard_bytes(domain, operator, byte_size)
                        .expect("encode policy guard");
                    let mut branches = 0;
                    for start in 0..bytes.len().saturating_sub(5) {
                        if bytes[start] == 0x0f
                            && matches!(bytes[start + 1], 0x82 | 0x83 | 0x84 | 0x85 | 0x87)
                        {
                            let displacement = i32::from_le_bytes(
                                bytes[start + 2..start + 6].try_into().expect("rel32 bytes"),
                            );
                            let target = (start + 6) as isize + displacement as isize;
                            assert!(
                                target >= 0 && target as usize <= bytes.len(),
                                "branch at {start} targets {target}, outside {} bytes",
                                bytes.len(),
                            );
                            branches += 1;
                        }
                    }
                    assert!(
                        branches >= 3,
                        "policy guard must contain its decision branches"
                    );
                    assert_eq!(
                        bytes.windows(2).any(|window| window == [0x0f, 0x0b]),
                        domain == ArithmeticDomain::Trapping,
                        "only Trapping emits ud2",
                    );
                }
            }
        }
    }

    #[test]
    fn non_arithmetic_float_operations_never_gain_policy_bytes() {
        for operator in [
            StateGuardOperator::Equal,
            StateGuardOperator::Min,
            StateGuardOperator::Max,
            StateGuardOperator::Sqrt,
        ] {
            assert!(
                float_policy_guard_bytes(ArithmeticDomain::Trapping, operator, 8)
                    .expect("gated guard")
                    .is_empty()
            );
        }
    }
}

#[cfg(test)]
mod call_encoding_tests {
    use super::append_call_register;

    #[test]
    fn low_registers_emit_ff_d0_through_ff_d7_without_rex() {
        // `FF /2` register-direct: ModRM = 0xD0 | rm, no REX for rax..rdi.
        // rax=D0 rcx=D1 rdx=D2 rbx=D3 rsp=D4 rbp=D5 rsi=D6 rdi=D7.
        for reg in 0u8..8 {
            let mut bytes = Vec::new();
            append_call_register(&mut bytes, reg);
            assert_eq!(
                bytes,
                vec![0xff, 0xd0 + reg],
                "call r{reg} must be FF {:02X} with no REX",
                0xd0 + reg
            );
        }
    }

    #[test]
    fn extended_registers_take_a_rex_b_prefix() {
        // r8..r15 need REX.B (0x41); ModRM low 3 bits wrap (r8 -> D0, r11 -> D3).
        for reg in 8u8..16 {
            let mut bytes = Vec::new();
            append_call_register(&mut bytes, reg);
            assert_eq!(
                bytes,
                vec![0x41, 0xff, 0xd0 | (reg & 0x7)],
                "call r{reg} must be 41 FF {:02X}",
                0xd0 | (reg & 0x7)
            );
        }
    }

    #[test]
    fn canonical_targets_are_exact() {
        // Spot-check the registers the first-boot path actually uses.
        let mut rax = Vec::new();
        append_call_register(&mut rax, 0);
        assert_eq!(rax, vec![0xff, 0xd0], "call rax");

        let mut r11 = Vec::new();
        append_call_register(&mut r11, 11);
        assert_eq!(r11, vec![0x41, 0xff, 0xd3], "call r11");
    }
}

#[cfg(test)]
mod vtable_call_encoding_tests {
    use super::{
        X86_64RelocationSiteKind, encode_win64_table_function_call, encode_win64_vtable_call,
        encode_win64_vtable_call_at_offset, win64_table_function_call_relocation_sites,
        win64_table_function_call_width, win64_vtable_call_relocation_sites,
        win64_vtable_call_width,
    };
    use omega_target_operations::{InstructionOperandLike, RuntimeStorageRegion};

    /// A minimal operand: either a runtime scalar (RCX = this from a field) or
    /// a runtime storage address (RDX = &text field). Everything else None.
    enum Op {
        Scalar {
            region: RuntimeStorageRegion,
            offset: usize,
            size: usize,
        },
        Float {
            region: RuntimeStorageRegion,
            offset: usize,
            size: usize,
        },
        Address {
            region: RuntimeStorageRegion,
            offset: usize,
        },
        Aggregate {
            region: RuntimeStorageRegion,
            offset: usize,
            size: usize,
            alignment: usize,
        },
    }
    impl InstructionOperandLike for Op {
        fn data_address(&self) -> Option<omega_target_operations::TargetDataObjectHandle> {
            None
        }
        fn runtime_string_pointer(&self) -> Option<(RuntimeStorageRegion, usize)> {
            None
        }
        fn runtime_string_length(&self) -> Option<(RuntimeStorageRegion, usize)> {
            None
        }
        fn runtime_string_is_bounded_buffer(&self) -> bool {
            false
        }
        fn runtime_pointee_string_pointer(&self) -> Option<(RuntimeStorageRegion, usize)> {
            None
        }
        fn runtime_pointee_string_length(&self) -> Option<(RuntimeStorageRegion, usize)> {
            None
        }
        fn runtime_scalar_integer(&self) -> Option<(RuntimeStorageRegion, usize, usize)> {
            match self {
                Op::Scalar {
                    region,
                    offset,
                    size,
                } => Some((*region, *offset, *size)),
                _ => None,
            }
        }
        fn runtime_scalar_float(&self) -> Option<(RuntimeStorageRegion, usize, usize)> {
            match self {
                Op::Float {
                    region,
                    offset,
                    size,
                } => Some((*region, *offset, *size)),
                _ => None,
            }
        }
        fn runtime_large_aggregate(&self) -> Option<(RuntimeStorageRegion, usize, usize, usize)> {
            match self {
                Op::Aggregate {
                    region,
                    offset,
                    size,
                    alignment,
                } => Some((*region, *offset, *size, *alignment)),
                _ => None,
            }
        }
        fn runtime_storage_address(&self) -> Option<(RuntimeStorageRegion, usize)> {
            match self {
                Op::Address { region, offset } => Some((*region, *offset)),
                _ => None,
            }
        }
        fn immediate_integer(&self) -> Option<i64> {
            None
        }
        fn byte_length(&self) -> Option<usize> {
            None
        }
    }

    #[test]
    fn output_string_marshals_this_and_text_then_calls_through_slot_1() {
        // output_string(this: addr@machine+0, text: &field@machine+8) -> VtableSlot(1).
        let operands = vec![
            Op::Scalar {
                region: RuntimeStorageRegion::Machine,
                offset: 0,
                size: 8,
            },
            Op::Address {
                region: RuntimeStorageRegion::Machine,
                offset: 8,
            },
        ];
        let bytes = encode_win64_vtable_call(&operands, 1).expect("encode");
        assert_eq!(
            bytes.len(),
            win64_vtable_call_width(&operands, 1, false),
            "width matches"
        );

        // 2 register args -> reserve = 32 (padded to 40); sub rsp, 40 (imm8).
        assert_eq!(&bytes[0..4], &[0x48, 0x83, 0xec, 40], "sub rsp, 40");
        // arg 0 (this -> RCX): mov r11,imm64 (10) then mov rcx,[r11+0].
        assert_eq!(bytes[4], 0x49, "mov r11,imm64 opcode #0");
        assert_eq!(
            &bytes[14..21],
            &[0x49, 0x8b, 0x8b, 0, 0, 0, 0],
            "rcx = [r11+0]"
        );
        // arg 1 (text -> RDX lea): mov r11,imm64 then lea rdx,[r11+8].
        assert_eq!(
            &bytes[31..38],
            &[0x49, 0x8d, 0x93, 8, 0, 0, 0],
            "lea rdx, [r11+8]"
        );
        // the vtable read + indirect call, then restore.
        assert_eq!(
            &bytes[38..45],
            &[0x48, 0x8b, 0x81, 8, 0, 0, 0],
            "mov rax, [rcx+8] (slot 1)"
        );
        assert_eq!(&bytes[45..47], &[0xff, 0xd0], "call rax");
        assert_eq!(&bytes[47..51], &[0x48, 0x83, 0xc4, 40], "add rsp, 40");
    }

    #[test]
    fn vtable_call_with_result_skips_the_result_operand_and_stores_rax() {
        // let status = protocol.method(text): operands = [result, this, text];
        // the result place must NOT marshal as an argument (the old encoder
        // put it in RCX and dispatched through it -- the M2 #UD at 0xB0000).
        let operands = vec![
            Op::Scalar {
                region: RuntimeStorageRegion::Machine,
                offset: 16,
                size: 8,
            },
            Op::Scalar {
                region: RuntimeStorageRegion::Machine,
                offset: 0,
                size: 8,
            },
            Op::Address {
                region: RuntimeStorageRegion::Machine,
                offset: 8,
            },
        ];
        let bytes = encode_win64_vtable_call_at_offset(&operands, 8, true).expect("encode");
        assert_eq!(
            bytes.len(),
            win64_vtable_call_width(&operands, 8, true),
            "width matches"
        );

        // Args marshal exactly as the no-result shape: this -> RCX, text -> RDX.
        assert_eq!(&bytes[0..4], &[0x48, 0x83, 0xec, 40], "sub rsp, 40");
        assert_eq!(
            &bytes[14..21],
            &[0x49, 0x8b, 0x8b, 0, 0, 0, 0],
            "rcx = [r11+0] (this)"
        );
        assert_eq!(
            &bytes[31..38],
            &[0x49, 0x8d, 0x93, 8, 0, 0, 0],
            "lea rdx, [r11+8]"
        );
        assert_eq!(
            &bytes[38..45],
            &[0x48, 0x8b, 0x81, 8, 0, 0, 0],
            "mov rax, [rcx+8]"
        );
        assert_eq!(&bytes[45..47], &[0xff, 0xd0], "call rax");
        assert_eq!(&bytes[47..51], &[0x48, 0x83, 0xc4, 40], "add rsp, 40");
        // The result store tail: mov r11,imm64 (relocated) + mov [r11+16], rax.
        assert_eq!(
            &bytes[51..53],
            &[0x49, 0xbb],
            "mov r11, imm64 (result base)"
        );
        assert_eq!(
            &bytes[61..68],
            &[0x49, 0x89, 0x83, 16, 0, 0, 0],
            "mov [r11+16], rax"
        );
        assert_eq!(bytes.len(), 68);
    }

    #[test]
    fn table_function_call_keeps_the_table_off_the_wire() {
        // let status = boot_services.get_memory_map(&arg): operands =
        // [result@16, table@0, &arg@8]. EFI table services take NO This: the
        // declared argument after the table lands in RCX, and the table is
        // read only to load the fn-ptr field (+56 here).
        let operands = vec![
            Op::Scalar {
                region: RuntimeStorageRegion::Machine,
                offset: 16,
                size: 8,
            },
            Op::Scalar {
                region: RuntimeStorageRegion::Machine,
                offset: 0,
                size: 8,
            },
            Op::Address {
                region: RuntimeStorageRegion::Machine,
                offset: 8,
            },
        ];
        let bytes = encode_win64_table_function_call(&operands, 56, true).expect("encode");
        assert_eq!(
            bytes.len(),
            win64_table_function_call_width(&operands, 56, true),
            "width matches"
        );

        // One register arg -> reserve 40; the FIRST DECLARED ARG (not the
        // table) lands in RCX.
        assert_eq!(&bytes[0..4], &[0x48, 0x83, 0xec, 40], "sub rsp, 40");
        assert_eq!(
            &bytes[14..21],
            &[0x49, 0x8d, 0x8b, 8, 0, 0, 0],
            "lea rcx, [r11+8] (arg)"
        );
        // The table pointer loads for dispatch only: mov r11,imm64 (relocated
        // to the table's region base) + mov rax,[r11+0], then the fn-ptr read.
        assert_eq!(&bytes[21..23], &[0x49, 0xbb], "mov r11, imm64 (table base)");
        assert_eq!(
            &bytes[31..38],
            &[0x49, 0x8b, 0x83, 0, 0, 0, 0],
            "rax = [r11+0] (table)"
        );
        assert_eq!(
            &bytes[38..45],
            &[0x48, 0x8b, 0x80, 56, 0, 0, 0],
            "rax = [rax+56] (fn ptr)"
        );
        assert_eq!(&bytes[45..47], &[0xff, 0xd0], "call rax");
        assert_eq!(&bytes[47..51], &[0x48, 0x83, 0xc4, 40], "add rsp, 40");
        // Result store tail.
        assert_eq!(
            &bytes[51..53],
            &[0x49, 0xbb],
            "mov r11, imm64 (result base)"
        );
        assert_eq!(
            &bytes[61..68],
            &[0x49, 0x89, 0x83, 16, 0, 0, 0],
            "mov [r11+16], rax"
        );
        assert_eq!(bytes.len(), 68);

        // Relocation sites: the arg lea (operand 2) at 4+2, the table load
        // (operand 1) at 21+2, the result store (operand 0) at 51+2 -- all
        // Absolute64 region bases.
        let sites = win64_table_function_call_relocation_sites(&operands, true);
        let offsets: Vec<(Option<usize>, usize)> = sites
            .iter()
            .map(|site| (site.operand_index, site.byte_offset))
            .collect();
        assert_eq!(offsets, vec![(Some(2), 6), (Some(1), 23), (Some(0), 53)]);
        assert!(
            sites
                .iter()
                .all(|site| matches!(site.kind, X86_64RelocationSiteKind::Absolute64))
        );
    }

    #[test]
    fn indirect_calls_share_win64_aggregate_caller_copy_layouts() {
        let receiver = || Op::Scalar {
            region: RuntimeStorageRegion::Machine,
            offset: 0,
            size: 8,
        };
        let aggregate = || Op::Aggregate {
            region: RuntimeStorageRegion::Machine,
            offset: 16,
            size: 24,
            alignment: 8,
        };

        let vtable_operands = vec![receiver(), aggregate()];
        let vtable = encode_win64_vtable_call_at_offset(&vtable_operands, 8, false)
            .expect("Win64 vtable aggregate call");
        assert_eq!(
            vtable.len(),
            win64_vtable_call_width(&vtable_operands, 8, false)
        );
        assert_eq!(&vtable[..4], &[0x48, 0x83, 0xec, 56]);
        assert!(
            vtable
                .windows(8)
                .any(|window| window == [0x48, 0x8d, 0x94, 0x24, 32, 0, 0, 0]),
            "the record following the receiver must point RDX at its copy"
        );
        assert_eq!(
            win64_vtable_call_relocation_sites(&vtable_operands, false)
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(0), Some(1)]
        );

        let table_operands = vec![receiver(), aggregate()];
        let table = encode_win64_table_function_call(&table_operands, 56, false)
            .expect("Win64 service-table aggregate call");
        assert_eq!(
            table.len(),
            win64_table_function_call_width(&table_operands, 56, false)
        );
        assert_eq!(&table[..4], &[0x48, 0x83, 0xec, 56]);
        assert!(
            table
                .windows(8)
                .any(|window| window == [0x48, 0x8d, 0x8c, 0x24, 32, 0, 0, 0]),
            "the first declared service argument must point RCX at its copy"
        );
        assert_eq!(
            win64_table_function_call_relocation_sites(&table_operands, false)
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(1), Some(0)]
        );
    }

    #[test]
    fn indirect_vtable_result_shifts_the_receiver_and_has_no_store_tail() {
        let operands = vec![
            Op::Aggregate {
                region: RuntimeStorageRegion::Machine,
                offset: 32,
                size: 24,
                alignment: 8,
            },
            Op::Scalar {
                region: RuntimeStorageRegion::Machine,
                offset: 0,
                size: 8,
            },
        ];
        let bytes = encode_win64_vtable_call_at_offset(&operands, 8, true)
            .expect("Win64 vtable indirect result call");
        assert_eq!(bytes.len(), win64_vtable_call_width(&operands, 8, true));
        assert_eq!(&bytes[..4], &[0x48, 0x83, 0xec, 40]);
        assert_eq!(
            &bytes[14..21],
            &[0x49, 0x8d, 0x8b, 32, 0, 0, 0],
            "hidden RCX must address the result record"
        );
        assert_eq!(
            &bytes[31..38],
            &[0x49, 0x8b, 0x93, 0, 0, 0, 0],
            "the receiver must shift to RDX"
        );
        assert_eq!(
            &bytes[38..45],
            &[0x48, 0x8b, 0x82, 8, 0, 0, 0],
            "dispatch must read through the shifted receiver"
        );
        assert_eq!(&bytes[45..47], &[0xff, 0xd0]);
        assert_eq!(&bytes[47..51], &[0x48, 0x83, 0xc4, 40]);
        assert_eq!(bytes.len(), 51, "the callee writes the result in place");
        assert_eq!(
            win64_vtable_call_relocation_sites(&operands, true)
                .iter()
                .map(|site| (site.operand_index, site.byte_offset))
                .collect::<Vec<_>>(),
            [(Some(0), 6), (Some(1), 23)]
        );
    }

    #[test]
    fn indirect_table_function_result_shifts_declared_arguments_only() {
        let operands = vec![
            Op::Aggregate {
                region: RuntimeStorageRegion::Machine,
                offset: 32,
                size: 24,
                alignment: 8,
            },
            Op::Scalar {
                region: RuntimeStorageRegion::Machine,
                offset: 0,
                size: 8,
            },
            Op::Scalar {
                region: RuntimeStorageRegion::Machine,
                offset: 8,
                size: 8,
            },
        ];
        let bytes = encode_win64_table_function_call(&operands, 56, true)
            .expect("Win64 service-table indirect result call");
        assert_eq!(
            bytes.len(),
            win64_table_function_call_width(&operands, 56, true)
        );
        assert_eq!(
            &bytes[14..21],
            &[0x49, 0x8d, 0x8b, 32, 0, 0, 0],
            "hidden RCX must address the result record"
        );
        assert_eq!(
            &bytes[31..38],
            &[0x49, 0x8b, 0x93, 8, 0, 0, 0],
            "the first declared service argument must shift to RDX"
        );
        assert_eq!(
            &bytes[48..55],
            &[0x49, 0x8b, 0x83, 0, 0, 0, 0],
            "the table remains dispatch-only"
        );
        assert_eq!(
            win64_table_function_call_relocation_sites(&operands, true)
                .iter()
                .map(|site| (site.operand_index, site.byte_offset))
                .collect::<Vec<_>>(),
            [(Some(0), 6), (Some(2), 23), (Some(1), 40)]
        );
    }

    #[test]
    fn vtable_float_argument_and_result_use_their_positional_xmm_registers() {
        let operands = vec![
            Op::Float {
                region: RuntimeStorageRegion::Machine,
                offset: 16,
                size: 8,
            },
            Op::Scalar {
                region: RuntimeStorageRegion::Machine,
                offset: 0,
                size: 8,
            },
            Op::Float {
                region: RuntimeStorageRegion::Machine,
                offset: 8,
                size: 8,
            },
        ];
        let bytes = encode_win64_vtable_call_at_offset(&operands, 8, true)
            .expect("Win64 vtable float call");
        assert_eq!(bytes.len(), win64_vtable_call_width(&operands, 8, true));
        assert!(
            bytes
                .windows(9)
                .any(|window| window == [0xf2, 0x41, 0x0f, 0x10, 0x8b, 8, 0, 0, 0]),
            "the second positional argument must load into XMM1"
        );
        assert!(
            bytes
                .windows(9)
                .any(|window| window == [0xf2, 0x41, 0x0f, 0x11, 0x83, 16, 0, 0, 0]),
            "the result must spill from XMM0"
        );
        assert_eq!(
            win64_vtable_call_relocation_sites(&operands, true)
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(1), Some(2), Some(0)]
        );
    }

    #[test]
    fn table_function_float_layout_excludes_the_dispatch_table() {
        let operands = vec![
            Op::Float {
                region: RuntimeStorageRegion::Machine,
                offset: 16,
                size: 4,
            },
            Op::Scalar {
                region: RuntimeStorageRegion::Machine,
                offset: 0,
                size: 8,
            },
            Op::Float {
                region: RuntimeStorageRegion::Machine,
                offset: 8,
                size: 4,
            },
        ];
        let bytes = encode_win64_table_function_call(&operands, 56, true)
            .expect("Win64 service-table float call");
        assert_eq!(
            bytes.len(),
            win64_table_function_call_width(&operands, 56, true)
        );
        assert!(
            bytes
                .windows(9)
                .any(|window| window == [0xf3, 0x41, 0x0f, 0x10, 0x83, 8, 0, 0, 0]),
            "the first declared service argument must use XMM0"
        );
        assert!(
            bytes
                .windows(9)
                .any(|window| window == [0xf3, 0x41, 0x0f, 0x11, 0x83, 16, 0, 0, 0]),
            "the service result must spill from XMM0"
        );
        assert_eq!(
            win64_table_function_call_relocation_sites(&operands, true)
                .iter()
                .map(|site| site.operand_index)
                .collect::<Vec<_>>(),
            [Some(2), Some(1), Some(0)]
        );
    }
}

#[cfg(test)]
mod machine_control_tests {
    use super::*;

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
        use omega_core::inline_assembly::AsmFenceKind;

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
        use omega_core::inline_assembly::AsmInterruptControlKind;

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
    fn deriver_only_lidt_reads_the_private_descriptor_through_r10() {
        assert_eq!(encode_lidt_from_r10_bytes(), [0x41, 0x0f, 0x01, 0x1a]);
        assert_eq!(encode_lidt_from_r10_bytes().len(), lidt_from_r10_width());
        assert_eq!(
            encode_generated_idt_load_bytes(MachineRegister::X86Rcx)
                .expect("Microsoft private pointer materialization"),
            [0x4c, 0x8b, 0xd1, 0x41, 0x0f, 0x01, 0x1a]
        );
        assert!(
            encode_generated_idt_load_bytes(MachineRegister::X86Xmm(0))
                .expect_err("vector pointer placement must reject")
                .message
                .contains("cannot arrive")
        );
    }

    #[test]
    fn generated_idt_writer_has_exact_packed_context_and_full_width_encoding() {
        let steps = [omega_target_operations::GeneratedIdtWriterStep {
            container_byte_offset: 0,
            container_width_bits: 64,
            destination_lsb: 0,
            source_lsb: 0,
            width: 64,
            source_slot: 0,
        }];
        let bytes = encode_generated_idt_writer_bytes(
            MachineRegister::X86Rdi,
            8,
            true,
            omega_target_operations::GENERATED_IDT_WRITER_CONTEXT_ABI_V1,
            1,
            &steps,
        )
        .expect("valid full-width generated writer");
        let mut expected = vec![
            0x4c, 0x8b, 0xd7, // mov r10, rdi (plan-selected private pointer)
            0x4d, 0x8b, 0x1a, // mov r11, [r10]
            0x49, 0x8b, 0x82, 0x08, 0x00, 0x00, 0x00, // mov rax, [r10+8]
            0x48, 0xba, // mov rdx, u64::MAX
        ];
        expected.extend(u64::MAX.to_le_bytes());
        expected.extend([
            0x48, 0x21, 0xd0, // and rax, rdx
            0x49, 0x8b, 0x8b, 0x00, 0x00, 0x00, 0x00, // mov rcx, [r11]
            0x48, 0xba, // mov rdx, 0
        ]);
        expected.extend(0_u64.to_le_bytes());
        expected.extend([
            0x48, 0x21, 0xd1, // and rcx, rdx
            0x48, 0x09, 0xc1, // or rcx, rax
            0x49, 0x89, 0x8b, 0x00, 0x00, 0x00, 0x00, // mov [r11], rcx
        ]);

        assert_eq!(bytes, expected);
        assert_eq!(bytes.len(), 56);
        assert_eq!(
            generated_idt_writer_width(
                MachineRegister::X86Rdi,
                8,
                true,
                omega_target_operations::GENERATED_IDT_WRITER_CONTEXT_ABI_V1,
                1,
                &steps,
            )
            .expect("writer width"),
            bytes.len()
        );
        assert_eq!(generated_idt_writer_context_width(1), Some(16));
        assert_eq!(GENERATED_IDT_WRITER_DESTINATION_OFFSET, 0);
        assert_eq!(GENERATED_IDT_WRITER_SOURCE_SLOTS_OFFSET, 8);
        assert_eq!(GENERATED_IDT_WRITER_SOURCE_SLOT_WIDTH, 8);
        assert_eq!(
            generated_idt_writer_clobbers().as_slice(),
            &[
                MachineRegister::X86Rax,
                MachineRegister::X86Rcx,
                MachineRegister::X86Rdx,
                MachineRegister::X86R10,
                MachineRegister::X86R11,
            ]
        );
        assert!(
            generated_idt_writer_additional_machine_state()
                .contains_all(MachineStateSet::new([MachineState::Flags]))
        );
    }

    #[test]
    fn generated_idt_writer_emits_exact_fragment_shifts_masks_and_word_access() {
        let steps = [omega_target_operations::GeneratedIdtWriterStep {
            container_byte_offset: 4,
            container_width_bits: 16,
            destination_lsb: 4,
            source_lsb: 12,
            width: 8,
            source_slot: 0,
        }];
        let bytes = encode_generated_idt_writer_bytes(
            MachineRegister::X86Rdi,
            8,
            true,
            omega_target_operations::GENERATED_IDT_WRITER_CONTEXT_ABI_V1,
            1,
            &steps,
        )
        .expect("valid fragmented generated writer");
        let mut expected = vec![
            0x4c, 0x8b, 0xd7, // mov r10, rdi (plan-selected private pointer)
            0x4d, 0x8b, 0x1a, // mov r11, [r10]
            0x49, 0x8b, 0x82, 0x08, 0x00, 0x00, 0x00, // mov rax, [r10+8]
            0x48, 0xc1, 0xe8, 0x0c, // shr rax, 12
            0x48, 0xba, // mov rdx, 0xff
        ];
        expected.extend(0xff_u64.to_le_bytes());
        expected.extend([
            0x48, 0x21, 0xd0, // and rax, rdx
            0x48, 0xc1, 0xe0, 0x04, // shl rax, 4
            0x41, 0x0f, 0xb7, 0x8b, 0x04, 0x00, 0x00, 0x00, // movzx ecx, word [r11+4]
            0x48, 0xba, // mov rdx, !0xff0
        ]);
        expected.extend((!0xff0_u64).to_le_bytes());
        expected.extend([
            0x48, 0x21, 0xd1, // and rcx, rdx
            0x48, 0x09, 0xc1, // or rcx, rax
            0x66, 0x41, 0x89, 0x8b, 0x04, 0x00, 0x00, 0x00, // mov [r11+4], cx
        ]);

        assert_eq!(bytes, expected);
        assert_eq!(bytes.len(), 66);
    }

    #[test]
    fn generated_idt_writer_rejects_unrepresentable_geometry_before_emission() {
        let step = omega_target_operations::GeneratedIdtWriterStep {
            container_byte_offset: 0,
            container_width_bits: 64,
            destination_lsb: 0,
            source_lsb: 0,
            width: 64,
            source_slot: 0,
        };
        assert!(
            encode_generated_idt_writer_bytes(
                MachineRegister::X86Rdi,
                8,
                false,
                omega_target_operations::GENERATED_IDT_WRITER_CONTEXT_ABI_V1,
                1,
                &[step],
            )
            .expect_err("x86 writer must be little-endian")
            .message
            .contains("little-endian")
        );
        assert!(
            encode_generated_idt_writer_bytes(
                MachineRegister::X86Rdi,
                8,
                true,
                omega_target_operations::GENERATED_IDT_WRITER_CONTEXT_ABI_V1,
                1,
                &[omega_target_operations::GeneratedIdtWriterStep {
                    source_slot: 1,
                    ..step
                }],
            )
            .expect_err("source slot outside context must reject")
            .message
            .contains("context has 1")
        );
        assert!(
            encode_generated_idt_writer_bytes(
                MachineRegister::X86Rdi,
                8,
                true,
                omega_target_operations::GENERATED_IDT_WRITER_CONTEXT_ABI_V1,
                1,
                &[omega_target_operations::GeneratedIdtWriterStep {
                    container_byte_offset: 8,
                    ..step
                }],
            )
            .expect_err("destination fragment outside table must reject")
            .message
            .contains("outside")
        );
        assert!(
            encode_generated_idt_writer_bytes(
                MachineRegister::X86Rdi,
                8,
                true,
                omega_target_operations::GENERATED_IDT_WRITER_CONTEXT_ABI_V1,
                2,
                &[omega_target_operations::GeneratedIdtWriterStep {
                    source_slot: 1,
                    ..step
                }],
            )
            .expect_err("private source slots must be dense")
            .message
            .contains("dense exact set")
        );
        assert!(
            encode_generated_idt_writer_bytes(
                MachineRegister::X86Rdi,
                8,
                true,
                0xdead,
                1,
                &[step],
            )
                .expect_err("unknown private context ABI must reject")
                .message
                .contains("IDTWRIT1")
        );
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

    /// A RuntimeValueOperandSource where every handle is an immediate integer
    /// (handle arena index -> value). Immediates emit no relocation, so the
    /// port-encoder byte layout is fully deterministic.
    use omega_target_operations::RuntimeStorageRegion;
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
        fn pointee(&self, _: RuntimeValueOperandHandle) -> Option<(usize, usize, usize)> {
            None
        }
        fn frame_indexed(
            &self,
            _: RuntimeValueOperandHandle,
        ) -> Option<(usize, RuntimeStorageRegion, usize, usize, usize, usize)> {
            None
        }
        fn frame_base_indexed(
            &self,
            _: RuntimeValueOperandHandle,
        ) -> Option<(usize, usize, usize, usize, usize)> {
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
        ) -> Option<(usize, RuntimeStorageRegion, usize, usize, usize, usize)> {
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
        ) -> Option<(omega_core::arithmetic::ArithmeticDomain, bool)> {
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
        ) -> Option<(RuntimeValueOperandHandle, String, bool)> {
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
        use omega_core::inline_assembly::AsmControlRegister;

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
        use omega_core::inline_assembly::AsmControlRegister;

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

#[cfg(test)]
mod atomic_tests {
    use super::*;

    fn operands(
        values: &[i64],
    ) -> omega_core::arena::Arena<omega_target_operations::RuntimeValueOperand> {
        let mut operands = omega_core::arena::Arena::default();
        for value in values {
            operands.insert(omega_target_operations::RuntimeValueOperand::Immediate(
                *value,
            ));
        }
        operands
    }

    #[test]
    fn full_atomic_rmw_encoders_store_the_instruction_observed_prior() {
        let operands = operands(&[5, 10, 99]);
        let delta = RuntimeValueOperandHandle::from_parts(0, 1);
        let expected = RuntimeValueOperandHandle::from_parts(1, 1);
        let new_value = RuntimeValueOperandHandle::from_parts(2, 1);

        let fetch = encode_atomic_fetch_add(&operands, 24, 4, 32, delta).unwrap();
        let fetch_result_base = runtime_atomic_fetch_add_result_address_offset(&operands, 4, delta);
        assert_eq!(
            &fetch[fetch_result_base..fetch_result_base + 2],
            &[0x49, 0xbe]
        );
        assert_eq!(&fetch[fetch.len() - 4..], &32i32.to_le_bytes());
        assert_eq!(
            fetch.len(),
            runtime_atomic_fetch_add_width(&operands, 4, 32, delta)
        );

        let fetch_sub = encode_atomic_fetch_sub(&operands, 24, 4, 34, delta).unwrap();
        let fetch_sub_result_base =
            runtime_atomic_fetch_sub_result_address_offset(&operands, 4, delta);
        assert_eq!(
            &fetch_sub[20..23],
            &[0x41, 0xf7, 0xda],
            "fetch_sub must negate r10d before the atomic XADD"
        );
        assert_eq!(
            &fetch_sub[23..28],
            &[0xf0, 0x45, 0x0f, 0xc1, 0x96],
            "fetch_sub must retain the locked XADD RMW"
        );
        assert_eq!(
            &fetch_sub[fetch_sub_result_base..fetch_sub_result_base + 2],
            &[0x49, 0xbe]
        );
        assert_eq!(&fetch_sub[fetch_sub.len() - 4..], &34i32.to_le_bytes());
        assert_eq!(
            fetch_sub.len(),
            runtime_atomic_fetch_sub_width(&operands, 4, 34, delta)
        );

        let fetch_xor = encode_atomic_fetch_xor(&operands, 24, 4, 35, delta).unwrap();
        let fetch_xor_result_base =
            runtime_atomic_fetch_xor_result_address_offset(&operands, 4, delta);
        assert_eq!(&fetch_xor[33..36], &[0x4d, 0x31, 0xda], "xor r10,r11");
        assert_eq!(
            &fetch_xor[36..41],
            &[0xf0, 0x45, 0x0f, 0xb1, 0x96],
            "fetch_xor retries with locked CMPXCHG"
        );
        assert_eq!(&fetch_xor[45..47], &[0x75, 0xef], "jne -17 to retry");
        assert_eq!(
            &fetch_xor[fetch_xor_result_base..fetch_xor_result_base + 2],
            &[0x49, 0xbe]
        );
        assert_eq!(&fetch_xor[fetch_xor.len() - 4..], &35i32.to_le_bytes());
        assert_eq!(
            fetch_xor.len(),
            runtime_atomic_fetch_xor_width(&operands, 4, 35, delta)
        );

        let fetch_or = encode_atomic_fetch_or(&operands, 24, 4, 35, delta).unwrap();
        let fetch_or_result_base =
            runtime_atomic_fetch_or_result_address_offset(&operands, 4, delta);
        assert_eq!(&fetch_or[33..36], &[0x4d, 0x09, 0xda], "or r10,r11");
        assert_eq!(
            &fetch_or[36..41],
            &[0xf0, 0x45, 0x0f, 0xb1, 0x96],
            "fetch_or retries with locked CMPXCHG"
        );
        assert_eq!(&fetch_or[45..47], &[0x75, 0xef], "jne -17 to retry");
        assert_eq!(
            &fetch_or[fetch_or_result_base..fetch_or_result_base + 2],
            &[0x49, 0xbe]
        );
        assert_eq!(&fetch_or[fetch_or.len() - 4..], &35i32.to_le_bytes());
        assert_eq!(
            fetch_or.len(),
            runtime_atomic_fetch_or_width(&operands, 4, 35, delta)
        );

        let fetch_and = encode_atomic_fetch_and(&operands, 24, 4, 35, delta).unwrap();
        let fetch_and_result_base =
            runtime_atomic_fetch_and_result_address_offset(&operands, 4, delta);
        assert_eq!(&fetch_and[33..36], &[0x4d, 0x21, 0xda], "and r10,r11");
        assert_eq!(
            &fetch_and[36..41],
            &[0xf0, 0x45, 0x0f, 0xb1, 0x96],
            "fetch_and retries with locked CMPXCHG"
        );
        assert_eq!(&fetch_and[45..47], &[0x75, 0xef], "jne -17 to retry");
        assert_eq!(
            &fetch_and[fetch_and_result_base..fetch_and_result_base + 2],
            &[0x49, 0xbe]
        );
        assert_eq!(&fetch_and[fetch_and.len() - 4..], &35i32.to_le_bytes());
        assert_eq!(
            fetch_and.len(),
            runtime_atomic_fetch_and_width(&operands, 4, 35, delta)
        );

        let swap = encode_atomic_swap(&operands, 24, 4, 36, new_value).unwrap();
        let swap_result_base = runtime_atomic_swap_result_address_offset(&operands, 4, new_value);
        assert_eq!(
            &swap[swap_result_base - 7..swap_result_base - 4],
            &[0x45, 0x87, 0x96],
            "memory XCHG is the atomic swap operation"
        );
        assert_eq!(&swap[swap_result_base..swap_result_base + 2], &[0x49, 0xbe]);
        assert_eq!(&swap[swap.len() - 4..], &36i32.to_le_bytes());
        assert_eq!(
            swap.len(),
            runtime_atomic_swap_width(&operands, 4, 36, new_value)
        );

        let cas =
            encode_atomic_compare_exchange(&operands, 24, 4, 40, expected, new_value).unwrap();
        let cas_result_base = runtime_atomic_compare_exchange_result_address_offset(
            &operands, 4, expected, new_value,
        );
        assert_eq!(
            &cas[cas_result_base - 3..cas_result_base],
            &[0x49, 0x89, 0xc2]
        );
        assert_eq!(&cas[cas_result_base..cas_result_base + 2], &[0x49, 0xbe]);
        assert_eq!(&cas[cas.len() - 4..], &40i32.to_le_bytes());
        assert_eq!(
            cas.len(),
            runtime_atomic_compare_exchange_width(&operands, 4, 40, expected, new_value)
        );
    }

    #[test]
    fn seq_cst_store_uses_implicitly_locked_xchg() {
        let operands = operands(&[42]);
        let value = RuntimeValueOperandHandle::from_parts(0, 1);
        let relaxed = encode_atomic_store_from_operand(&operands, 8, 4, value, false).unwrap();
        let seq_cst = encode_atomic_store_from_operand(&operands, 8, 4, value, true).unwrap();
        assert_eq!(&relaxed[20..23], &[0x45, 0x89, 0x96]);
        assert_eq!(&seq_cst[20..23], &[0x45, 0x87, 0x96]);
        assert_eq!(relaxed.len(), seq_cst.len());
    }

    #[test]
    fn lock_xadd_emits_lock_prefix_and_xadd_opcode() {
        for &byte_size in &[1usize, 2, 4, 8] {
            let mut bytes = Vec::new();
            append_lock_xadd_r10_to_r14(&mut bytes, 0x18, byte_size).expect("encode");
            assert_eq!(
                bytes.len(),
                lock_xadd_r10_to_r14_width(byte_size),
                "width mismatch for {byte_size}-byte lock xadd"
            );
            assert_eq!(bytes[0], 0xf0, "must begin with the LOCK prefix (0xF0)");
            // Operand-size prefix only for 16-bit.
            let rex_index = if byte_size == 2 { 2 } else { 1 };
            if byte_size == 2 {
                assert_eq!(bytes[1], 0x66, "16-bit needs the operand-size prefix");
            }
            assert_eq!(
                bytes[rex_index],
                if byte_size == 8 { 0x4d } else { 0x45 },
                "REX"
            );
            assert_eq!(bytes[rex_index + 1], 0x0f, "two-byte opcode escape");
            let xadd_opcode = if byte_size == 1 { 0xc0 } else { 0xc1 };
            assert_eq!(bytes[rex_index + 2], xadd_opcode, "XADD opcode");
            assert_eq!(bytes[rex_index + 3], 0x96, "ModRM [r14+disp32], r10");
            // disp32 little-endian tail.
            assert_eq!(&bytes[rex_index + 4..], &0x18i32.to_le_bytes());

            let mut negation = Vec::new();
            append_negate_r10(&mut negation, byte_size).expect("encode negate");
            let expected: &[u8] = match byte_size {
                1 => &[0x41, 0xf6, 0xda],
                2 => &[0x66, 0x41, 0xf7, 0xda],
                4 => &[0x41, 0xf7, 0xda],
                8 => &[0x49, 0xf7, 0xda],
                _ => unreachable!(),
            };
            assert_eq!(negation, expected, "width-specific NEG r10 encoding");
            assert_eq!(negation.len(), negate_r10_width(byte_size));
        }
    }

    #[test]
    fn lock_cmpxchg_emits_lock_prefix_and_cmpxchg_opcode() {
        for &byte_size in &[1usize, 2, 4, 8] {
            let mut bytes = Vec::new();
            append_lock_cmpxchg_r10_to_r14(&mut bytes, 0x24, byte_size).expect("encode");
            assert_eq!(
                bytes.len(),
                lock_cmpxchg_r10_to_r14_width(byte_size),
                "width mismatch for {byte_size}-byte lock cmpxchg"
            );
            assert_eq!(bytes[0], 0xf0, "must begin with the LOCK prefix (0xF0)");
            let rex_index = if byte_size == 2 { 2 } else { 1 };
            if byte_size == 2 {
                assert_eq!(bytes[1], 0x66, "16-bit needs the operand-size prefix");
            }
            assert_eq!(
                bytes[rex_index],
                if byte_size == 8 { 0x4d } else { 0x45 },
                "REX"
            );
            assert_eq!(bytes[rex_index + 1], 0x0f, "two-byte opcode escape");
            // CMPXCHG is 0F B1 (or 0F B0 for 8-bit), NOT xadd's 0F C1/C0.
            let cmpxchg_opcode = if byte_size == 1 { 0xb0 } else { 0xb1 };
            assert_eq!(bytes[rex_index + 2], cmpxchg_opcode, "CMPXCHG opcode");
            assert_eq!(bytes[rex_index + 3], 0x96, "ModRM [r14+disp32], r10");
            assert_eq!(&bytes[rex_index + 4..], &0x24i32.to_le_bytes());
        }
    }
}

#[cfg(test)]
mod wrapping_shift_clamp_tests {
    use super::*;

    #[test]
    fn clamp_compares_the_full_count_and_cmovs_zero() {
        for &byte_size in &[1usize, 2, 4, 8] {
            let mut bytes = Vec::new();
            append_wrapping_shift_zero_clamp(&mut bytes, byte_size);
            assert_eq!(
                bytes.len(),
                WRAPPING_SHIFT_ZERO_CLAMP_WIDTH,
                "width mismatch for the {byte_size}-byte clamp"
            );
            assert_eq!(&bytes[0..2], &[0x31, 0xc0], "xor eax, eax");
            // The FULL count in r11 (not the cl copy): a count with set high
            // bits (i64 2^32+1, or a negative signed count) must still clamp.
            assert_eq!(&bytes[2..5], &[0x49, 0x83, 0xfb], "cmp r11, imm8");
            assert_eq!(bytes[5] as usize, byte_size * 8, "width_bits immediate");
            assert_eq!(&bytes[6..10], &[0x4c, 0x0f, 0x43, 0xd0], "cmovae r10, rax");
        }
    }

    #[test]
    fn wrapping_count_mask_masks_subword_only() {
        // F8b: the Wrapping count mask is an explicit AND at sub-word widths
        // (`and r11d, 7/15`) and ABSENT at 4/8 -- the hardware `shl`/`sar`
        // mask mod 32/64 there, which IS the ch5 masked-count ruling.
        for &(byte_size, expect) in &[(1usize, Some(7u8)), (2, Some(15)), (4, None), (8, None)] {
            let mut bytes = Vec::new();
            append_wrapping_shift_count_mask(&mut bytes, byte_size);
            assert_eq!(
                bytes.len(),
                wrapping_shift_count_mask_width(byte_size),
                "emission and width accounting must agree at {byte_size} bytes"
            );
            match expect {
                Some(mask) => assert_eq!(bytes, vec![0x41, 0x83, 0xe3, mask], "and r11d, mask"),
                None => assert!(bytes.is_empty(), "no mask at width {byte_size}"),
            }
        }
    }

    #[test]
    fn wrapping_shl_sequence_shifts_then_clamps_without_touching_the_count() {
        // The write-path pair: width-correct shl (hardware masks the count
        // mod 32) followed by the modular clamp reading the intact r11.
        let mut bytes = Vec::new();
        append_runtime_binary_operation(&mut bytes, StateGuardOperator::ShiftLeft, 4).expect("shl");
        append_wrapping_shift_zero_clamp(&mut bytes, 4);
        assert_eq!(
            bytes,
            vec![
                0x44, 0x89, 0xd9, // mov ecx, r11d (count copy; r11 stays intact)
                0x41, 0xd3, 0xe2, // shl r10d, cl
                0x31, 0xc0, // xor eax, eax
                0x49, 0x83, 0xfb, 32, // cmp r11, 32
                0x4c, 0x0f, 0x43, 0xd0, // cmovae r10, rax
            ]
        );
        assert_eq!(
            bytes.len(),
            runtime_binary_operation_width(StateGuardOperator::ShiftLeft, 4)
                + WRAPPING_SHIFT_ZERO_CLAMP_WIDTH,
            "emission and width accounting must agree"
        );
    }

    #[test]
    fn arithmetic_shr_saturates_the_count_before_the_sar() {
        // The pre-fix: at/above-width counts become width-1, so the sar
        // itself produces the sign-fill. cmovae writes r11 (the count),
        // NOT r10 (the value).
        let mut bytes = Vec::new();
        append_wrapping_shift_right_count_saturate(&mut bytes, 4);
        append_runtime_binary_operation(&mut bytes, StateGuardOperator::ShiftRight, 4)
            .expect("sar");
        assert_eq!(
            bytes,
            vec![
                0xb8, 31, 0, 0, 0, // mov eax, 31 (width-1)
                0x49, 0x83, 0xfb, 32, // cmp r11, 32
                0x4c, 0x0f, 0x43, 0xd8, // cmovae r11, rax (count, not value)
                0x44, 0x89, 0xd9, // mov ecx, r11d (saturated count copy)
                0x41, 0xd3, 0xfa, // sar r10d, cl
            ]
        );
        assert_eq!(
            bytes.len(),
            WRAPPING_SHIFT_RIGHT_COUNT_SATURATE_WIDTH
                + runtime_binary_operation_width(StateGuardOperator::ShiftRight, 4),
            "emission and width accounting must agree"
        );
    }

    #[test]
    fn saturating_trapping_shift_left_width_stays_in_lockstep() {
        // Every (domain x signedness x width) arm's emitted length must
        // match the width twin, or relocation offsets drift.
        for domain in [ArithmeticDomain::Saturating, ArithmeticDomain::Trapping] {
            for target_signed in [false, true] {
                for byte_size in [1usize, 2, 4, 8] {
                    let mut bytes = Vec::new();
                    append_saturating_trapping_shift_left(
                        &mut bytes,
                        domain,
                        byte_size,
                        target_signed,
                    )
                    .expect("emit");
                    assert_eq!(
                        bytes.len(),
                        saturating_trapping_shift_left_width(domain, byte_size, target_signed),
                        "width mismatch: {domain:?} signed={target_signed} {byte_size}b"
                    );
                }
            }
        }
    }

    #[test]
    fn saturating_shl_narrow_caps_the_count_then_takes_the_unsigned_upper_clamp() {
        // u8 Saturating: [cap count at 8] + 64-bit shl + cmova against 255.
        let mut bytes = Vec::new();
        append_saturating_trapping_shift_left(&mut bytes, ArithmeticDomain::Saturating, 1, false)
            .expect("emit");
        assert_eq!(
            bytes,
            vec![
                0xb8, 8, 0, 0, 0, // mov eax, 8 (the width)
                0x49, 0x83, 0xfb, 8, // cmp r11, 8
                0x4c, 0x0f, 0x43, 0xd8, // cmovae r11, rax (cap the COUNT)
                0x4c, 0x89, 0xd9, // mov rcx, r11
                0x49, 0xd3, 0xe2, // shl r10, cl (64-bit, exact)
                0x49, 0xbb, 255, 0, 0, 0, 0, 0, 0, 0, // mov r11, 255
                0x4d, 0x39, 0xda, // cmp r10, r11
                0x4d, 0x0f, 0x47, 0xd3, // cmova r10, r11 (UNSIGNED upper)
            ]
        );
    }

    #[test]
    fn saturating_trapping_add_sub_width_stays_in_lockstep() {
        // Every (domain x op x signedness x width x per-side-immediate) arm's
        // emitted length must match the width twin, or relocation offsets
        // drift.
        for domain in [ArithmeticDomain::Saturating, ArithmeticDomain::Trapping] {
            for operator in [StateGuardOperator::Add, StateGuardOperator::Subtract] {
                for target_signed in [false, true] {
                    for byte_size in [1usize, 2, 4, 8] {
                        for left_imm in [false, true] {
                            for right_imm in [false, true] {
                                let mut bytes = Vec::new();
                                append_saturating_trapping_add_sub(
                                    &mut bytes,
                                    domain,
                                    operator,
                                    byte_size,
                                    target_signed,
                                    left_imm,
                                    right_imm,
                                )
                                .expect("emit");
                                assert_eq!(
                                    bytes.len(),
                                    saturating_trapping_add_sub_width(
                                        domain,
                                        operator,
                                        byte_size,
                                        target_signed,
                                        left_imm,
                                        right_imm,
                                    ),
                                    "width mismatch: {domain:?} {operator:?} \
                                     signed={target_signed} {byte_size}b \
                                     imm=({left_imm},{right_imm})"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn min_idiom_subtract_skips_the_immediate_and_wide_computes() {
        // The MIN idiom `(0 as i32 in Saturating) - 2147483648`: left is a
        // convert (extends), right is a WIDE immediate (must NOT re-extend);
        // one exact 64-bit sub; both signed bounds.
        let mut bytes = Vec::new();
        append_saturating_trapping_add_sub(
            &mut bytes,
            ArithmeticDomain::Saturating,
            StateGuardOperator::Subtract,
            4,
            true,
            false, // left: convert-of-literal, not an immediate operand
            true,  // right: the wide literal
        )
        .expect("emit");
        assert_eq!(
            &bytes[0..3],
            &[0x4d, 0x63, 0xd2],
            "movsxd r10 (left extends)"
        );
        // NO movsxd r11 (4d 63 db) anywhere: the immediate keeps its wide value.
        assert!(
            !bytes.windows(3).any(|w| w == [0x4d, 0x63, 0xdb]),
            "the immediate operand must not re-extend"
        );
        assert_eq!(
            &bytes[3..6],
            &[0x4d, 0x29, 0xda],
            "wide 64-bit sub r10, r11"
        );
    }

    #[test]
    fn unsigned_saturating_subtract_clamps_downward_with_a_signed_compare() {
        // 10u8 - 100u8 wide-computes to -90, whose UNSIGNED reading is huge:
        // the subtract arm clamps to 0 through cmovl (signed), never cmova.
        let mut bytes = Vec::new();
        append_saturating_trapping_add_sub(
            &mut bytes,
            ArithmeticDomain::Saturating,
            StateGuardOperator::Subtract,
            1,
            false,
            false,
            false,
        )
        .expect("emit");
        assert!(
            bytes.windows(4).any(|w| w == [0x4d, 0x0f, 0x4c, 0xd3]),
            "expected cmovl (signed lower clamp to 0)"
        );
        assert!(
            !bytes.windows(4).any(|w| w == [0x4d, 0x0f, 0x47, 0xd3]),
            "an unsigned upper cmova would clamp underflow to MAX"
        );
    }

    #[test]
    fn wire_byte_predicate_checks_emit_deterministically() {
        // The width fn measures the pure emitter; determinism and the
        // block-prefix bytes are the executable-free sanity available on
        // this host (runtime behavior rides the linux_x64 ELF pin + the
        // differential once an x86 host runs the suite).
        use omega_core::byte_predicates::ByteSequencePredicate;
        for predicate in ByteSequencePredicate::ALL {
            let mask = predicate.mask_bit();
            let mut once = Vec::new();
            append_wire_byte_predicate_checks(&mut once, mask);
            let mut twice = Vec::new();
            append_wire_byte_predicate_checks(&mut twice, mask);
            assert_eq!(once, twice, "{predicate:?} must emit deterministically");
            assert_eq!(once.len(), wire_byte_predicate_checks_width(mask));
            assert!(!once.is_empty(), "{predicate:?} must emit a check");
            // Every block ends able to clear the ok flag: xor r9d, r9d.
            assert!(
                once.windows(3).any(|w| w == [0x45, 0x31, 0xc9]),
                "{predicate:?} must clear r9d on violation"
            );
        }
        // The utf8 walk begins with the pointer/end setup shared by the
        // loop blocks: mov rcx, r15 / mov r11, r15 / add r11, rax.
        let mut utf8 = Vec::new();
        append_wire_byte_predicate_checks(&mut utf8, ByteSequencePredicate::ValidUtf8.mask_bit());
        assert_eq!(
            &utf8[0..9],
            &[0x4c, 0x89, 0xf9, 0x4d, 0x89, 0xfb, 0x49, 0x01, 0xc3]
        );
    }

    #[test]
    fn node_width_extension_width_stays_in_lockstep() {
        for &byte_width in &[1usize, 2, 4, 8] {
            for &operands_signed in &[false, true] {
                let mut bytes = Vec::new();
                append_wrapping_node_width_extension(&mut bytes, byte_width, operands_signed);
                assert_eq!(
                    bytes.len(),
                    wrapping_node_width_extension_width(byte_width),
                    "extension width mismatch at {byte_width} bytes (signed: {operands_signed})"
                );
            }
        }
    }
}

// ============================================================================
// Console byte ops (std read_byte/write_byte) -- the x86_64 flavors.
// ZII-driven like the aarch64 pair: the ByteRead slot is pre-zeroed (tag 0 =
// Eof = the untouched state), the read lands straight in the payload word,
// and only a count > 0 writes tag 1. r14 holds the relocated region base
// (imm64 at +2, the line-read convention).
// ============================================================================

/// Windows import flavor: GetStdHandle(STD_INPUT_HANDLE) + ReadFile(handle,
/// &payload, 1, &bytes_read, NULL). Fixed width; the two rel32 call fixups
/// sit at [`runtime_byte_read_get_std_handle_offset`] and
/// [`runtime_byte_read_read_file_offset`].
pub fn encode_runtime_byte_read_import(
    target_offset: usize,
    payload_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    validate_normalized_win64_get_std_handle_plan()?;
    let file_layout = normalized_win64_file_io_layout()?;
    let tag_disp = disp32(target_offset)?;
    let payload_disp = disp32(target_offset + payload_offset)?;
    let mut bytes = Vec::with_capacity(runtime_byte_read_import_width());
    bytes.extend([0x49, 0xbe]); // mov r14, imm64 (region, relocated)
    bytes.extend(0u64.to_le_bytes());
    append_zero_dword_r14(&mut bytes, tag_disp); // tag = 0 (Eof)
    append_zero_dword_r14(&mut bytes, payload_disp); // payload = 0
    append_sub_rsp(&mut bytes, file_layout.reserve);
    bytes.push(0xb9); // mov ecx, STD_INPUT_HANDLE
    bytes.extend((-10i32).to_le_bytes());
    bytes.push(0xe8); // call GetStdHandle
    debug_assert_eq!(bytes.len(), runtime_byte_read_get_std_handle_offset());
    bytes.extend([0, 0, 0, 0]);
    bytes.extend([0x48, 0x89, 0xc1]); // mov rcx, rax (handle)
    bytes.extend([0x49, 0x8d, 0x96]); // lea rdx, [r14 + payload]
    bytes.extend(payload_disp.to_le_bytes());
    bytes.extend([0x41, 0xb8, 0x01, 0x00, 0x00, 0x00]); // mov r8d, 1
    bytes.extend([0x4c, 0x8d, 0x4c, 0x24, file_layout.transferred_disp]);
    bytes.extend([
        0x48,
        0xc7,
        0x44,
        0x24,
        file_layout.overlapped_disp,
        0,
        0,
        0,
        0,
    ]);
    bytes.push(0xe8); // call ReadFile
    debug_assert_eq!(bytes.len(), runtime_byte_read_read_file_offset());
    bytes.extend([0, 0, 0, 0]);
    bytes.extend([0x8b, 0x44, 0x24, file_layout.transferred_disp]);
    bytes.extend([0x85, 0xc0]); // test eax, eax
    bytes.extend([0x74, 0x0b]); // je +11 (skip the tag store: Eof stays)
    append_one_dword_r14(&mut bytes, tag_disp); // tag = 1 (Byte)
    append_add_rsp(&mut bytes, file_layout.reserve);
    debug_assert_eq!(bytes.len(), runtime_byte_read_import_width());
    Ok(bytes)
}

/// Syscall flavor (linux_x64): read(0, &payload, 1) via the number table.
pub fn encode_runtime_byte_read_syscall(
    target_offset: usize,
    payload_offset: usize,
    number: u32,
    parameter_registers: &[omega_calling_conventions::MachineRegister],
    result_register: omega_calling_conventions::MachineRegister,
    number_register: omega_calling_conventions::MachineRegister,
    supervisor_call: u16,
) -> Result<Vec<u8>, Diagnostic> {
    validate_composite_linux_syscall_plan(
        parameter_registers,
        result_register,
        number_register,
        supervisor_call,
    )?;
    let tag_disp = disp32(target_offset)?;
    let payload_disp = disp32(target_offset + payload_offset)?;
    let mut bytes = Vec::with_capacity(runtime_byte_read_syscall_width());
    bytes.extend([0x49, 0xbe]); // mov r14, imm64 (region, relocated)
    bytes.extend(0u64.to_le_bytes());
    append_zero_dword_r14(&mut bytes, tag_disp);
    append_zero_dword_r14(&mut bytes, payload_disp);
    bytes.extend([0x48, 0x31, 0xff]); // xor rdi, rdi (fd 0)
    bytes.extend([0x49, 0x8d, 0xb6]); // lea rsi, [r14 + payload]
    bytes.extend(payload_disp.to_le_bytes());
    bytes.extend([0xba, 0x01, 0x00, 0x00, 0x00]); // mov edx, 1
    bytes.push(0xb8); // mov eax, number
    bytes.extend(number.to_le_bytes());
    bytes.extend([0x0f, 0x05]); // syscall
    bytes.extend([0x48, 0x85, 0xc0]); // test rax, rax
    bytes.extend([0x7e, 0x0b]); // jle +11 (0 = EOF, negative = error: Eof stays)
    append_one_dword_r14(&mut bytes, tag_disp);
    debug_assert_eq!(bytes.len(), runtime_byte_read_syscall_width());
    Ok(bytes)
}

/// Windows import flavor: GetStdHandle(STD_OUTPUT_HANDLE) + WriteFile(handle,
/// &source, 1, &written, NULL).
pub fn encode_runtime_byte_write_import(source_offset: usize) -> Result<Vec<u8>, Diagnostic> {
    validate_normalized_win64_get_std_handle_plan()?;
    let file_layout = normalized_win64_file_io_layout()?;
    let source_disp = disp32(source_offset)?;
    let mut bytes = Vec::with_capacity(runtime_byte_write_import_width());
    bytes.extend([0x49, 0xbe]); // mov r14, imm64 (source region/literal, relocated)
    bytes.extend(0u64.to_le_bytes());
    append_sub_rsp(&mut bytes, file_layout.reserve);
    bytes.push(0xb9); // mov ecx, STD_OUTPUT_HANDLE
    bytes.extend((-11i32).to_le_bytes());
    bytes.push(0xe8); // call GetStdHandle
    debug_assert_eq!(bytes.len(), runtime_byte_write_get_std_handle_offset());
    bytes.extend([0, 0, 0, 0]);
    bytes.extend([0x48, 0x89, 0xc1]); // mov rcx, rax
    bytes.extend([0x49, 0x8d, 0x96]); // lea rdx, [r14 + source]
    bytes.extend(source_disp.to_le_bytes());
    bytes.extend([0x41, 0xb8, 0x01, 0x00, 0x00, 0x00]); // mov r8d, 1
    bytes.extend([0x4c, 0x8d, 0x4c, 0x24, file_layout.transferred_disp]);
    bytes.extend([
        0x48,
        0xc7,
        0x44,
        0x24,
        file_layout.overlapped_disp,
        0,
        0,
        0,
        0,
    ]);
    bytes.push(0xe8); // call WriteFile
    debug_assert_eq!(bytes.len(), runtime_byte_write_write_file_offset());
    bytes.extend([0, 0, 0, 0]);
    append_add_rsp(&mut bytes, file_layout.reserve);
    debug_assert_eq!(bytes.len(), runtime_byte_write_import_width());
    Ok(bytes)
}

/// Syscall flavor (linux_x64): write(1, &source, 1).
pub fn encode_runtime_byte_write_syscall(
    source_offset: usize,
    number: u32,
    parameter_registers: &[omega_calling_conventions::MachineRegister],
    result_register: omega_calling_conventions::MachineRegister,
    number_register: omega_calling_conventions::MachineRegister,
    supervisor_call: u16,
) -> Result<Vec<u8>, Diagnostic> {
    validate_composite_linux_syscall_plan(
        parameter_registers,
        result_register,
        number_register,
        supervisor_call,
    )?;
    let source_disp = disp32(source_offset)?;
    let mut bytes = Vec::with_capacity(runtime_byte_write_syscall_width());
    bytes.extend([0x49, 0xbe]); // mov r14, imm64 (source, relocated)
    bytes.extend(0u64.to_le_bytes());
    bytes.extend([0xbf, 0x01, 0x00, 0x00, 0x00]); // mov edi, 1 (stdout)
    bytes.extend([0x49, 0x8d, 0xb6]); // lea rsi, [r14 + source]
    bytes.extend(source_disp.to_le_bytes());
    bytes.extend([0xba, 0x01, 0x00, 0x00, 0x00]); // mov edx, 1
    bytes.push(0xb8); // mov eax, number
    bytes.extend(number.to_le_bytes());
    bytes.extend([0x0f, 0x05]); // syscall
    debug_assert_eq!(bytes.len(), runtime_byte_write_syscall_width());
    Ok(bytes)
}

fn validate_composite_linux_syscall_plan(
    parameter_registers: &[omega_calling_conventions::MachineRegister],
    result_register: omega_calling_conventions::MachineRegister,
    number_register: omega_calling_conventions::MachineRegister,
    supervisor_call: u16,
) -> Result<(), Diagnostic> {
    use omega_calling_conventions::MachineRegister::*;
    if parameter_registers != [X86Rdi, X86Rsi, X86Rdx]
        || result_register != X86Rax
        || number_register != X86Rax
        || supervisor_call != 0
    {
        return Err(Diagnostic::error(format!(
            "X86_64 composite runtime-text syscall encoder cannot realize normalized plan parameters={parameter_registers:?}, result={result_register:?}, number={number_register:?}, immediate={supervisor_call}"
        )));
    }
    Ok(())
}

/// `mov dword [r14 + disp32], 0` (11 bytes).
fn append_zero_dword_r14(bytes: &mut Vec<u8>, disp: i32) {
    bytes.extend([0x41, 0xc7, 0x86]);
    bytes.extend(disp.to_le_bytes());
    bytes.extend(0u32.to_le_bytes());
}

/// `mov dword [r14 + disp32], 1` (11 bytes).
fn append_one_dword_r14(bytes: &mut Vec<u8>, disp: i32) {
    bytes.extend([0x41, 0xc7, 0x86]);
    bytes.extend(disp.to_le_bytes());
    bytes.extend(1u32.to_le_bytes());
}

pub fn runtime_byte_read_import_width() -> usize {
    104
}
/// rel32 fixup position of the GetStdHandle call inside the import read.
pub fn runtime_byte_read_get_std_handle_offset() -> usize {
    42
}
/// rel32 fixup position of the ReadFile call inside the import read.
pub fn runtime_byte_read_read_file_offset() -> usize {
    77
}
pub fn runtime_byte_read_syscall_width() -> usize {
    70
}
pub fn runtime_byte_write_import_width() -> usize {
    63
}
pub fn runtime_byte_write_get_std_handle_offset() -> usize {
    20
}
pub fn runtime_byte_write_write_file_offset() -> usize {
    55
}
pub fn runtime_byte_write_syscall_width() -> usize {
    34
}

#[cfg(test)]
mod byte_io_width_tests {
    use super::*;
    use omega_calling_conventions::MachineRegister;

    const PARAMETERS: [MachineRegister; 3] = [
        MachineRegister::X86Rdi,
        MachineRegister::X86Rsi,
        MachineRegister::X86Rdx,
    ];

    #[test]
    fn byte_op_widths_match_emission() {
        for (target_offset, payload_offset) in [(0usize, 4usize), (8, 4), (48, 4)] {
            let import = encode_runtime_byte_read_import(target_offset, payload_offset).unwrap();
            assert_eq!(import.len(), runtime_byte_read_import_width());
            let syscall = encode_runtime_byte_read_syscall(
                target_offset,
                payload_offset,
                0,
                &PARAMETERS,
                MachineRegister::X86Rax,
                MachineRegister::X86Rax,
                0,
            )
            .unwrap();
            assert_eq!(syscall.len(), runtime_byte_read_syscall_width());
        }
        for source_offset in [0usize, 8, 48] {
            let import = encode_runtime_byte_write_import(source_offset).unwrap();
            assert_eq!(import.len(), runtime_byte_write_import_width());
            let syscall = encode_runtime_byte_write_syscall(
                source_offset,
                1,
                &PARAMETERS,
                MachineRegister::X86Rax,
                MachineRegister::X86Rax,
                0,
            )
            .unwrap();
            assert_eq!(syscall.len(), runtime_byte_write_syscall_width());
        }
    }

    #[test]
    fn composite_syscalls_reject_registers_the_encoder_cannot_realize() {
        let noncanonical_parameters = [
            MachineRegister::X86Rcx,
            MachineRegister::X86Rsi,
            MachineRegister::X86Rdx,
        ];
        let diagnostic = encode_runtime_byte_write_syscall(
            0,
            1,
            &noncanonical_parameters,
            MachineRegister::X86Rax,
            MachineRegister::X86Rax,
            0,
        )
        .unwrap_err();

        assert!(
            diagnostic
                .message
                .contains("cannot realize normalized plan")
        );
    }
}
