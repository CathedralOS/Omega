use super::{
    BINARY_RIGHT_OPERAND_PUSH_WIDTH, MOV_RAX_R10_WIDTH, Reg64, append_load_rax_from_r14,
    append_load_reg_from_r14, append_lock_cmpxchg_r10_to_r14, append_lock_xadd_r10_to_r14,
    append_mov_r14_imm64, append_mov_rax_r10, append_mov_reg_reg, append_negate_r10,
    append_pop_r10, append_push_r10, append_runtime_binary_operation, append_runtime_value_operand,
    append_store_r10_to_r14, append_xchg_r10_to_r14, load_rax_from_r14_width, load_width,
    lock_cmpxchg_r10_to_r14_width, lock_xadd_r10_to_r14_width, negate_r10_width,
    runtime_binary_operation_width, runtime_value_operand_width, store_width,
};
use omega_target_operations::{
    RuntimeValueOperandHandle, RuntimeValueOperandSource, StateGuardOperator,
};
use psi_diagnostics::Diagnostic;

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
    global_order: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_atomic_store_from_operand_width(
        runtime_value_operands,
        byte_size,
        value,
    ));
    append_mov_r14_imm64(&mut bytes, 0);
    append_runtime_value_operand(runtime_value_operands, &mut bytes, Reg64::R10, value)?;
    if global_order {
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

#[cfg(test)]
mod atomic_tests {
    use super::*;

    fn operands(values: &[i64]) -> psi_arena::Arena<omega_target_operations::RuntimeValueOperand> {
        let mut operands = psi_arena::Arena::default();
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
    fn global_order_store_uses_implicitly_locked_xchg() {
        let operands = operands(&[42]);
        let value = RuntimeValueOperandHandle::from_parts(0, 1);
        let no_ordering = encode_atomic_store_from_operand(&operands, 8, 4, value, false).unwrap();
        let global_order = encode_atomic_store_from_operand(&operands, 8, 4, value, true).unwrap();
        assert_eq!(&no_ordering[20..23], &[0x45, 0x89, 0x96]);
        assert_eq!(&global_order[20..23], &[0x45, 0x87, 0x96]);
        assert_eq!(no_ordering.len(), global_order.len());
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
