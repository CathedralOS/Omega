use super::validate;
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
