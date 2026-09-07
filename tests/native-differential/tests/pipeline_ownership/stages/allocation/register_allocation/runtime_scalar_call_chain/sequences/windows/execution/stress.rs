//! Test ABI interposition; never a production artifact or admission mechanism.

use machine_code::PlacedInternalMachineCallResolution;

fn relative(bytes: &mut Vec<u8>, opcode: u8, target: usize) {
    let next = bytes.len() + 5;
    let displacement = i32::try_from(target as i64 - next as i64).unwrap();
    bytes.push(opcode);
    bytes.extend_from_slice(&displacement.to_le_bytes());
}

pub(super) fn interpose(
    bytes: &mut Vec<u8>,
    calls: &[PlacedInternalMachineCallResolution],
    trace: &mut [[u64; 4]; 3],
) {
    assert_eq!(calls.len(), trace.len());
    for (call, observed) in calls.iter().zip(trace) {
        assert_eq!(call.field_byte_width, 4);
        assert_eq!(
            bytes[usize::try_from(call.opcode_section_offset).unwrap()],
            0xe8
        );
        let hook = bytes.len();
        // MOV R11, trace address; record the four register argument slots.
        bytes.extend_from_slice(&[0x49, 0xbb]);
        bytes.extend_from_slice(&(observed.as_mut_ptr() as u64).to_le_bytes());
        bytes.extend_from_slice(&[
            0x49, 0x89, 0x0b, // [R11] = RCX
            0x49, 0x89, 0x53, 0x08, // [R11+8] = RDX
            0x4d, 0x89, 0x43, 0x10, // [R11+16] = R8
            0x4d, 0x89, 0x4b, 0x18, // [R11+24] = R9
            0x49, 0xba, // MOV R10, poison
        ]);
        bytes.extend_from_slice(&0xdead_beef_cafe_1234_u64.to_le_bytes());
        // At callee entry [RSP] is the return address; [RSP+8..40)
        // is the caller-provided Microsoft home area.
        for offset in [8, 16, 24, 32] {
            bytes.extend_from_slice(&[0x4c, 0x89, 0x54, 0x24, offset]);
        }
        // Tail-enter the unchanged compiled callee: its actual result is
        // consumed by later calls, not supplied by this observation hook.
        relative(
            bytes,
            0xe9,
            usize::try_from(call.callee_section_offset).unwrap(),
        );
        let displacement =
            i32::try_from(hook as i64 - call.next_instruction_section_offset as i64).unwrap();
        let field = usize::try_from(call.field_section_offset).unwrap();
        bytes[field..field + 4].copy_from_slice(&displacement.to_le_bytes());
    }
}

pub(super) fn preservation_wrapper(bytes: &mut Vec<u8>, entry: usize) -> usize {
    let start = bytes.len();
    // Preserve the host's eight nonvolatile GPRs; reserve the wrapper's own
    // 32-byte outgoing home area plus eight bytes for 16-byte call alignment.
    bytes.extend_from_slice(&[
        0x53, 0x55, 0x56, 0x57, 0x41, 0x54, 0x41, 0x55, 0x41, 0x56, 0x41, 0x57,
    ]);
    bytes.extend_from_slice(&[0x48, 0x83, 0xec, 0x28]);
    let registers = [
        (0x48, 3),
        (0x48, 5),
        (0x48, 6),
        (0x48, 7),
        (0x49, 4),
        (0x49, 5),
        (0x49, 6),
        (0x49, 7),
    ];
    for (index, (rex, register)) in registers.iter().enumerate() {
        bytes.extend_from_slice(&[*rex, 0xb8 + register]);
        bytes.extend_from_slice(&(0x1234_5678_0000_0000_u64 + index as u64).to_le_bytes());
    }
    relative(bytes, 0xe8, entry);
    bytes.extend_from_slice(&[0x45, 0x31, 0xd2]); // XOR R10D,R10D: mismatch accumulator
    for (index, (rex, register)) in registers.iter().enumerate() {
        bytes.extend_from_slice(&[0x48, 0xb8]);
        bytes.extend_from_slice(&(0x1234_5678_0000_0000_u64 + index as u64).to_le_bytes());
        bytes.extend_from_slice(&[*rex, 0x39, 0xc0 + register]); // CMP register,RAX
        bytes.extend_from_slice(&[0x0f, 0x95, 0xc0, 0x0f, 0xb6, 0xc0, 0x49, 0x09, 0xc2]);
    }
    bytes.extend_from_slice(&[0x4c, 0x89, 0xd0, 0x48, 0x83, 0xc4, 0x28]); // RAX=R10, release area
    bytes.extend_from_slice(&[
        0x41, 0x5f, 0x41, 0x5e, 0x41, 0x5d, 0x41, 0x5c, 0x5f, 0x5e, 0x5d, 0x5b, 0xc3,
    ]);
    start
}
