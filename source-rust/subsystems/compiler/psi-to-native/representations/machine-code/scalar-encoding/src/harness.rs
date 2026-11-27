/*
We provide the shared conformance sweep (`rejects_every_single_bit_mutation`)
that an instruction set runs over its `LoadEncodedScalar` and
`StoreEncodedScalar` encoder and validator. The caller supplies its six
admitted operand register views and its `encode` and `validate` functions; we
verify single-bit stream corruption refusal, scratch-register aliasing
rejection, and acceptance of shared input registers.
*/

use register_model::RegisterViewId;
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, SelectedInstructionKind,
};

const fn kind(load: bool) -> SelectedInstructionKind {
    if load {
        SelectedInstructionKind::LoadEncodedScalar
    } else {
        SelectedInstructionKind::StoreEncodedScalar
    }
}

const fn alternative(load: bool) -> MachineAlternativeKey {
    MachineAlternativeKey {
        family: if load {
            MachineAlternativeFamily::LoadEncodedScalar
        } else {
            MachineAlternativeFamily::StoreEncodedScalar
        },
        variant: 0,
    }
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
