/*
We provide the shared conformance sweeps (`exact_footprint_and_extension` and
`rejects_every_single_bit_mutation`) that both `isa-aarch64` and `isa-x86_64`
run over their `LoadEncodedScalar` and `StoreEncodedScalar` encoders and
validators. Each architecture supplies its six admitted operand register views,
its `encode` and `validate` functions, and its decoded-stream replay callback;
we verify every admitted signed, unsigned, and out-of-band codec word, exact
byte access order, sign/zero extension, single-bit stream corruption refusal,
and scratch-register aliasing rejection.
*/

use register_model::RegisterViewId;
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, SelectedInstructionKind,
};

pub const fn kind(load: bool) -> SelectedInstructionKind {
    if load {
        SelectedInstructionKind::LoadEncodedScalar
    } else {
        SelectedInstructionKind::StoreEncodedScalar
    }
}

pub const fn alternative(load: bool) -> MachineAlternativeKey {
    MachineAlternativeKey {
        family: if load {
            MachineAlternativeFamily::LoadEncodedScalar
        } else {
            MachineAlternativeFamily::StoreEncodedScalar
        },
        variant: 0,
    }
}

fn codecs() -> impl Iterator<Item = u64> {
    (1..=8u64)
        .flat_map(|width| [width, width | 256])
        .chain([516, 520, 1025, 2056])
}

fn emit<E: std::fmt::Debug>(
    encode: &impl Fn(
        SelectedInstructionKind,
        MachineAlternativeKey,
        &[RegisterViewId; 6],
    ) -> Result<Vec<u8>, E>,
    load: bool,
    operands: &[RegisterViewId; 6],
) -> Vec<u8> {
    encode(kind(load), alternative(load), operands).expect("admitted operands must encode")
}

pub fn exact_footprint_and_extension<E: std::fmt::Debug>(
    registers: [u8; 6],
    operands: [RegisterViewId; 6],
    encode: impl Fn(
        SelectedInstructionKind,
        MachineAlternativeKey,
        &[RegisterViewId; 6],
    ) -> Result<Vec<u8>, E>,
    execute: impl Fn(&[u8], &mut [u64; 32], &mut [u8]) -> Vec<usize>,
) {
    for codec in codecs() {
        let width = codec & 255;
        let signed = codec & 256 != 0;
        let payload = if codec == 1025 {
            1
        } else {
            0x807f_6655_4433_2280u64
        };
        let mut memory = [85u8; 10];
        let mut state = [0u64; 32];
        state[registers[0] as usize] = 1;
        state[registers[1] as usize] = codec;
        state[registers[2] as usize] = payload;
        let original = state;
        let store = emit(&encode, false, &operands);
        let writes = execute(&store, &mut state, &mut memory);
        assert_eq!(writes, (1..=width as usize).collect::<Vec<_>>());
        assert_eq!(memory[0], 85);
        assert_eq!(memory[width as usize + 1], 85);
        assert_eq!(
            &memory[1..=width as usize],
            &payload.to_le_bytes()[..width as usize]
        );
        for input in 0..3 {
            assert_eq!(
                state[registers[input] as usize],
                original[registers[input] as usize]
            );
        }
        let load = emit(&encode, true, &operands);
        let reads = execute(&load, &mut state, &mut memory);
        assert_eq!(reads, (1..=width as usize).rev().collect::<Vec<_>>());
        let shift = 64 - width * 8;
        let expected = if signed {
            ((payload << shift) as i64 >> shift) as u64
        } else {
            (payload << shift) >> shift
        };
        assert_eq!(state[registers[5] as usize], expected);
        for input in 0..2 {
            assert_eq!(
                state[registers[input] as usize],
                original[registers[input] as usize]
            );
        }
    }
}

pub fn rejects_every_single_bit_mutation<E: std::fmt::Debug>(
    operands: [RegisterViewId; 6],
    encode: impl Fn(
        SelectedInstructionKind,
        MachineAlternativeKey,
        &[RegisterViewId; 6],
    ) -> Result<Vec<u8>, E>,
    refused: impl Fn(
        SelectedInstructionKind,
        MachineAlternativeKey,
        &[RegisterViewId; 6],
        &[u8],
    ) -> bool,
) {
    for load in [true, false] {
        let kind = kind(load);
        let alternative = alternative(load);
        let encoded = emit(&encode, load, &operands);
        for bit in 0..encoded.len() * 8 {
            let mut changed = encoded.clone();
            changed[bit / 8] ^= 1 << (bit % 8);
            assert!(
                refused(kind, alternative, &operands, &changed),
                "{kind:?} bit {bit}"
            );
        }
        let input_count = if load { 2 } else { 3 };
        for scratch in input_count..6 {
            for prior in 0..scratch {
                let mut aliased = operands;
                aliased[scratch] = operands[prior];
                assert!(
                    encode(kind, alternative, &aliased).is_err(),
                    "{kind:?} scratch {scratch} aliases operand {prior}"
                );
                assert!(
                    refused(kind, alternative, &aliased, &encoded),
                    "{kind:?} scratch {scratch} aliasing operand {prior} was accepted"
                );
            }
        }
        for input in 1..input_count {
            let mut shared = operands;
            shared[input] = operands[0];
            let encoded = emit(&encode, load, &shared);
            assert!(
                !refused(kind, alternative, &shared, &encoded),
                "{kind:?} shared input {input} was refused"
            );
        }
    }
}
