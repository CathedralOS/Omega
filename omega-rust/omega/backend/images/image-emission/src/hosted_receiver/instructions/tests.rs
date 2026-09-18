use super::{validate, validate_linux_arm64, validate_x86_64};
use crate::hosted_receiver::HostedReceiverPartitions;

const BSS_ADDRESS: u64 = 0x1000_2000;
const PARTITIONS: HostedReceiverPartitions = HostedReceiverPartitions {
    saved_continuation_offset: 0x20,
    stack_offset: 0x30,
    stack_byte_count: 0x100,
    receiver_offset: 0x140,
    receiver_byte_count: 8,
    end_offset: 0x148,
};

// These are hand-encoded instructions, not output from the bridge generator or
// its relocation patcher. Addresses below are chosen independently of decoding.
fn forward_pages() -> [u32; 16] {
    [
        0xd000_0009, // At 0x10000ff0: ADRP x9, +2 pages -> 0x10002000.
        0x9100_8129, // ADD x9, x9, #0x20 -> saved continuation.
        0x9100_03ea,
        0xa900_792a,
        0xb000_000a, // At 0x10001000: ADRP x10, +1 page.
        0x9104_c14a, // ADD x10, x10, #0x130 -> stack top.
        0x9100_015f,
        0xb000_0000, // ADRP x0, +1 page.
        0x9105_0000, // ADD x0, x0, #0x140 -> receiver.
        0x97ff_ffe7, // BL -25 words, from 0x10001014 to 0x10000fb0.
        0xb000_0009, // ADRP x9, +1 page after callee clobbers.
        0x9100_8129,
        0xa940_792a,
        0x9100_015f,
        0x5280_0000,
        0xd65f_03c0,
    ]
}

fn bytes(words: &[u32; 16]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

#[test]
fn bridge_instructions_reconstruct_signed_pages_and_call_targets() {
    validate(
        &bytes(&forward_pages()),
        0x1000_0ff0,
        0x1000_0fb0,
        BSS_ADDRESS,
        PARTITIONS,
    )
    .expect("page crossing with independently known backward call");

    let mut backward = forward_pages();
    for (instruction, register) in [(0, 9), (4, 10), (7, 0), (10, 9)] {
        backward[instruction] = 0xf0ff_ffe0 | register; // ADRP -1 page.
    }
    validate(
        &bytes(&backward),
        0x1000_3000,
        0x1000_2fc0,
        BSS_ADDRESS,
        PARTITIONS,
    )
    .expect("negative page displacement and backward call");
    backward[9] = 0x9400_0007; // BL +7 words from bridge+36 to bridge+64.
    validate(
        &bytes(&backward),
        0x1000_3000,
        0x1000_3040,
        BSS_ADDRESS,
        PARTITIONS,
    )
    .expect("forward call displacement");
}

#[test]
fn bridge_instructions_reject_every_changed_address_pair_and_call_bit() {
    let original = forward_pages();
    for instruction in [0, 1, 4, 5, 7, 8, 9, 10, 11] {
        for bit in 0..32 {
            let mut changed = original;
            changed[instruction] ^= 1 << bit;
            assert!(
                validate(
                    &bytes(&changed),
                    0x1000_0ff0,
                    0x1000_0fb0,
                    BSS_ADDRESS,
                    PARTITIONS
                )
                .is_err(),
                "instruction {instruction}, bit {bit}"
            );
        }
    }
}

#[test]
fn bridge_instructions_reject_changed_sp_lr_memory_and_return_flow() {
    let original = forward_pages();
    for instruction in [2, 3, 6, 12, 13, 14, 15] {
        for bit in 0..32 {
            let mut changed = original;
            changed[instruction] ^= 1 << bit;
            assert!(
                validate(
                    &bytes(&changed),
                    0x1000_0ff0,
                    0x1000_0fb0,
                    BSS_ADDRESS,
                    PARTITIONS
                )
                .is_err(),
                "instruction {instruction}, bit {bit}"
            );
        }
    }
    let mut reordered = original;
    reordered.swap(3, 6);
    assert!(
        validate(
            &bytes(&reordered),
            0x1000_0ff0,
            0x1000_0fb0,
            BSS_ADDRESS,
            PARTITIONS
        )
        .is_err(),
        "switching SP before saving the physical continuation changes custody"
    );
}

#[test]
fn bridge_instructions_reject_redirected_expected_storage_or_entry() {
    let encoded = bytes(&forward_pages());
    for mutation in 0..4 {
        let mut partitions = PARTITIONS;
        match mutation {
            0 => partitions.saved_continuation_offset += 16,
            1 => partitions.stack_offset += 16,
            2 => partitions.stack_byte_count += 16,
            _ => partitions.receiver_offset += 16,
        }
        assert!(validate(&encoded, 0x1000_0ff0, 0x1000_0fb0, BSS_ADDRESS, partitions).is_err());
    }
    assert!(validate(&encoded, 0x1000_0ff0, 0x1000_0fb4, BSS_ADDRESS, PARTITIONS).is_err());
    assert!(
        validate(
            &encoded,
            0x1000_0ff0,
            0x1000_0fb0,
            BSS_ADDRESS + 0x1000,
            PARTITIONS
        )
        .is_err()
    );
}

#[test]
fn bridge_instructions_reject_malformed_lengths_alignment_and_address_overflow() {
    let encoded = bytes(&forward_pages());
    for length in [0, 1, 3, 4, 60, 63] {
        assert!(
            validate(
                &encoded[..length],
                0x1000_0ff0,
                0x1000_0fb0,
                BSS_ADDRESS,
                PARTITIONS
            )
            .is_err()
        );
    }
    let mut longer = encoded.clone();
    longer.extend([0; 4]);
    assert!(validate(&longer, 0x1000_0ff0, 0x1000_0fb0, BSS_ADDRESS, PARTITIONS).is_err());
    for address in [0x1000_0ff1, u64::MAX - 3] {
        assert!(validate(&encoded, address, 0x1000_0fb0, BSS_ADDRESS, PARTITIONS).is_err());
    }
    assert!(
        validate(
            &encoded,
            0x1000_0ff0,
            0x1000_0fb0,
            u64::MAX - 15,
            PARTITIONS
        )
        .is_err()
    );
    let mut overflowing = PARTITIONS;
    overflowing.stack_byte_count = u64::MAX;
    assert!(validate(&encoded, 0x1000_0ff0, 0x1000_0fb0, BSS_ADDRESS, overflowing).is_err());
}

// Hand-encoded Linux x86-64 bridge: `mov [rip+scratch], rsp; lea rsp,
// [rip+stack_top]; lea rdi, [rip+receiver]; call rel32; xor edi, edi;
// mov eax, 231; syscall; ud2`. Displacements are computed independently of
// the writer and relocation patcher, exactly as the final image resolves them.
fn linux_shim(
    address: u64,
    selected_entry: u64,
    bss: u64,
    partitions: HostedReceiverPartitions,
) -> Vec<u8> {
    let rel32 = |field: usize, target: u64| -> i32 {
        i32::try_from(target as i128 - (address as i128 + field as i128 + 4)).unwrap()
    };
    let continuation = bss + partitions.saved_continuation_offset;
    let stack_top = bss + partitions.stack_offset + partitions.stack_byte_count;
    let receiver = bss + partitions.receiver_offset;
    let mut bytes = vec![0x48, 0x89, 0x25];
    bytes.extend(rel32(3, continuation).to_le_bytes());
    bytes.extend([0x48, 0x8d, 0x25]);
    bytes.extend(rel32(10, stack_top).to_le_bytes());
    bytes.extend([0x48, 0x8d, 0x3d]);
    bytes.extend(rel32(17, receiver).to_le_bytes());
    bytes.extend([0xe8]);
    bytes.extend(rel32(22, selected_entry).to_le_bytes());
    bytes.extend([0x31, 0xff, 0xb8, 0xe7, 0, 0, 0, 0x0f, 0x05, 0x0f, 0x0b]);
    debug_assert_eq!(bytes.len(), 37);
    bytes
}

#[test]
fn linux_bridge_instructions_reconstruct_rip_relative_storage_and_call() {
    let shim = linux_shim(0x1000_0ff0, 0x1000_0fb0, BSS_ADDRESS, PARTITIONS);
    validate_x86_64(&shim, 0x1000_0ff0, 0x1000_0fb0, BSS_ADDRESS, PARTITIONS)
        .expect("exact Linux bridge bytes and resolved targets");

    // Backward and forward rel32 displacements both reconstruct.
    let shim = linux_shim(0x1000_3000, 0x1000_2fc0, BSS_ADDRESS, PARTITIONS);
    validate_x86_64(&shim, 0x1000_3000, 0x1000_2fc0, BSS_ADDRESS, PARTITIONS)
        .expect("negative displacement reconstruction");
    let shim = linux_shim(0x1000_3000, 0x1000_4000, BSS_ADDRESS, PARTITIONS);
    validate_x86_64(&shim, 0x1000_3000, 0x1000_4000, BSS_ADDRESS, PARTITIONS)
        .expect("forward call displacement");
}

#[test]
fn linux_bridge_instructions_reject_every_mutated_byte() {
    let address = 0x1000_0ff0;
    let entry = 0x1000_0fb0;
    let original = linux_shim(address, entry, BSS_ADDRESS, PARTITIONS);
    for bit in 0..(original.len() * 8) {
        let mut hostile = original.clone();
        hostile[bit / 8] ^= 1 << (bit % 8);
        assert!(
            validate_x86_64(&hostile, address, entry, BSS_ADDRESS, PARTITIONS).is_err(),
            "bit {bit}"
        );
    }
    for length in [0, 3, 7, 21, 26, 36] {
        assert!(
            validate_x86_64(&original[..length], address, entry, BSS_ADDRESS, PARTITIONS).is_err()
        );
    }
    let mut longer = original.clone();
    longer.push(0x0b);
    assert!(validate_x86_64(&longer, address, entry, BSS_ADDRESS, PARTITIONS).is_err());
}

#[test]
fn linux_bridge_instructions_reject_redirected_storage_or_entry() {
    let address = 0x1000_0ff0;
    let entry = 0x1000_0fb0;
    let shim = linux_shim(address, entry, BSS_ADDRESS, PARTITIONS);
    for mutation in 0..4 {
        let mut partitions = PARTITIONS;
        match mutation {
            0 => partitions.saved_continuation_offset += 16,
            1 => partitions.stack_offset += 16,
            2 => partitions.stack_byte_count += 16,
            _ => partitions.receiver_offset += 16,
        }
        assert!(validate_x86_64(&shim, address, entry, BSS_ADDRESS, partitions).is_err());
    }
    // A shim aimed at a different continuation must not satisfy this entry.
    assert!(validate_x86_64(&shim, address, entry + 0x40, BSS_ADDRESS, PARTITIONS).is_err());
    let redirected = linux_shim(address, entry + 0x40, BSS_ADDRESS, PARTITIONS);
    assert!(validate_x86_64(&redirected, address, entry, BSS_ADDRESS, PARTITIONS).is_err());
    let moved = linux_shim(address, entry, BSS_ADDRESS + 0x1000, PARTITIONS);
    assert!(validate_x86_64(&moved, address, entry, BSS_ADDRESS, PARTITIONS).is_err());
    // A shim encoded for a different base must not replay at this address.
    assert!(validate_x86_64(&moved, address + 0x40, entry, BSS_ADDRESS, PARTITIONS).is_err());
    // BSS arithmetic overflow fails closed.
    let mut overflowing = PARTITIONS;
    overflowing.stack_byte_count = u64::MAX;
    assert!(validate_x86_64(&shim, address, entry, BSS_ADDRESS, overflowing).is_err());
}

// Hand-encoded Linux ARM64 bridge: adrp/add the continuation residence,
// observe the incoming sp, load the head-word contract input, stp the pair,
// adrp/add the private stack top, switch sp, adrp/add the receiver into x0,
// bl to the semantic continuation, then exit_group(94) with w0 = 0 and a brk
// fence. Immediates are computed independently of the writer and relocator.
fn linux_arm64_shim(
    address: u64,
    selected_entry: u64,
    bss: u64,
    partitions: HostedReceiverPartitions,
) -> [u32; 15] {
    let adrp = |instruction: usize, register: u32, target: u64| -> u32 {
        let pc = (address + (instruction * 4) as u64) & !0xfff;
        let pages = ((target & !0xfff) as i64 - pc as i64) / 4096;
        0x9000_0000
            | (((pages as u32) & 0x3) << 29)
            | ((((pages as u32) >> 2) & 0x7_ffff) << 5)
            | register
    };
    let add_lo12 = |register: u32, target: u64| -> u32 {
        0x9100_0000 | (((target as u32) & 0xfff) << 10) | (register << 5) | register
    };
    let continuation = bss + partitions.saved_continuation_offset;
    let stack_top = bss + partitions.stack_offset + partitions.stack_byte_count;
    let receiver = bss + partitions.receiver_offset;
    let call_words = ((selected_entry as i64 - (address + 40) as i64) / 4) as u32;
    [
        adrp(0, 9, continuation),
        add_lo12(9, continuation),
        0x9100_03ea,
        0xf940_014b,
        0xa900_2d2a,
        adrp(5, 10, stack_top),
        add_lo12(10, stack_top),
        0x9100_015f,
        adrp(8, 0, receiver),
        add_lo12(0, receiver),
        0x9400_0000 | (call_words & 0x03ff_ffff),
        0x5280_0000,
        0xd280_0bc8,
        0xd400_0001,
        0xd420_0000,
    ]
}

fn linux_arm64_bytes(words: &[u32; 15]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

#[test]
fn linux_arm64_bridge_instructions_reconstruct_signed_pages_and_call() {
    let shim = linux_arm64_shim(0x1000_0ff0, 0x1000_0fb0, BSS_ADDRESS, PARTITIONS);
    validate_linux_arm64(
        &linux_arm64_bytes(&shim),
        0x1000_0ff0,
        0x1000_0fb0,
        BSS_ADDRESS,
        PARTITIONS,
    )
    .expect("exact Linux ARM64 bridge words and resolved targets");

    // Negative page displacements and a backward call both reconstruct.
    let shim = linux_arm64_shim(0x1000_3000, 0x1000_2fc0, BSS_ADDRESS, PARTITIONS);
    validate_linux_arm64(
        &linux_arm64_bytes(&shim),
        0x1000_3000,
        0x1000_2fc0,
        BSS_ADDRESS,
        PARTITIONS,
    )
    .expect("negative page displacement and backward call");
    let shim = linux_arm64_shim(0x1000_3000, 0x1000_4000, BSS_ADDRESS, PARTITIONS);
    validate_linux_arm64(
        &linux_arm64_bytes(&shim),
        0x1000_3000,
        0x1000_4000,
        BSS_ADDRESS,
        PARTITIONS,
    )
    .expect("forward call displacement");
}

#[test]
fn linux_arm64_bridge_instructions_reject_every_mutated_word() {
    let address = 0x1000_0ff0;
    let entry = 0x1000_0fb0;
    let original = linux_arm64_shim(address, entry, BSS_ADDRESS, PARTITIONS);
    for instruction in 0..15 {
        for bit in 0..32 {
            let mut changed = original;
            changed[instruction] ^= 1 << bit;
            assert!(
                validate_linux_arm64(
                    &linux_arm64_bytes(&changed),
                    address,
                    entry,
                    BSS_ADDRESS,
                    PARTITIONS
                )
                .is_err(),
                "instruction {instruction}, bit {bit}"
            );
        }
    }
    // Storing the continuation after the SP switch would lose input custody.
    let mut reordered = original;
    reordered.swap(4, 7);
    assert!(
        validate_linux_arm64(
            &linux_arm64_bytes(&reordered),
            address,
            entry,
            BSS_ADDRESS,
            PARTITIONS
        )
        .is_err(),
        "switching SP before saving the physical continuation changes custody"
    );
    for length in [0, 4, 40, 56, 59] {
        assert!(
            validate_linux_arm64(
                &linux_arm64_bytes(&original)[..length],
                address,
                entry,
                BSS_ADDRESS,
                PARTITIONS
            )
            .is_err()
        );
    }
    let mut longer = linux_arm64_bytes(&original);
    longer.extend([0; 4]);
    assert!(validate_linux_arm64(&longer, address, entry, BSS_ADDRESS, PARTITIONS).is_err());
}

#[test]
fn linux_arm64_bridge_instructions_reject_redirected_storage_or_entry() {
    let address = 0x1000_0ff0;
    let entry = 0x1000_0fb0;
    let shim = linux_arm64_bytes(&linux_arm64_shim(address, entry, BSS_ADDRESS, PARTITIONS));
    for mutation in 0..4 {
        let mut partitions = PARTITIONS;
        match mutation {
            0 => partitions.saved_continuation_offset += 16,
            1 => partitions.stack_offset += 16,
            2 => partitions.stack_byte_count += 16,
            _ => partitions.receiver_offset += 16,
        }
        assert!(validate_linux_arm64(&shim, address, entry, BSS_ADDRESS, partitions).is_err());
    }
    // A shim aimed at a different continuation must not satisfy this entry.
    assert!(validate_linux_arm64(&shim, address, entry + 0x40, BSS_ADDRESS, PARTITIONS).is_err());
    let redirected = linux_arm64_bytes(&linux_arm64_shim(
        address,
        entry + 0x40,
        BSS_ADDRESS,
        PARTITIONS,
    ));
    assert!(validate_linux_arm64(&redirected, address, entry, BSS_ADDRESS, PARTITIONS).is_err());
    let moved = linux_arm64_bytes(&linux_arm64_shim(
        address,
        entry,
        BSS_ADDRESS + 0x1000,
        PARTITIONS,
    ));
    assert!(validate_linux_arm64(&moved, address, entry, BSS_ADDRESS, PARTITIONS).is_err());
    for hostile in [address + 1, u64::MAX - 3] {
        assert!(validate_linux_arm64(&shim, hostile, entry, BSS_ADDRESS, PARTITIONS).is_err());
    }
    // BSS arithmetic overflow fails closed.
    let mut overflowing = PARTITIONS;
    overflowing.stack_byte_count = u64::MAX;
    assert!(validate_linux_arm64(&shim, address, entry, BSS_ADDRESS, overflowing).is_err());
}
