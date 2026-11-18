#![forbid(unsafe_code)]

/*
We define the target-neutral step program (`Step` and `plan`) that both the
x86-64 and AArch64 selected-form encoders lower when realizing variable-width
encoded scalar loads and stores (`LoadEncodedScalar` and `StoreEncodedScalar`).
A load walks the admitted byte count from high byte to low byte and sign-extends
the leading byte through constant shifts when the codec word marks the scalar
signed; a store walks low-to-high under the same byte count. Each instruction
set supplies only its concrete instruction encodings and terminal load step,
resolving step-index branch targets in its own stream unit (bytes on x86-64,
32-bit words on AArch64).
*/

/*
We state the encoded-scalar byte-loop program once in this crate so that two
architectures with different instruction widths and condition-flag rules cannot
drift in the byte addresses they access, the order in which they walk multi-byte
scalars, or how signed and unsigned widths extend into a 64-bit destination
register. Keeping the step vocabulary and control flow shared reduces each ISA's
encoded-scalar implementation to a direct one-to-one step table and decoder.
*/

/*
`isa-aarch64` and `isa-x86_64` consume `Step` and `plan` in their
`selected_form_encoding` modules to emit and validate encoded-scalar instruction
streams. Under the `test-support` feature, our `replay` module executes a
decoded `Step` stream against a 32-register state array and byte buffer, while
`harness` drives the shared footprint, sign-extension, bit-flip rejection, and
scratch-aliasing test sweeps for both architectures.
*/

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Zero(u8),
    Copy(u8, u8),
    ByteCount(u8, u8),
    SubOne(u8),
    AddOne(u8),
    LoadByte(u8, u8, u8),
    StoreByte(u8, u8, u8),
    SignByte(u8),
    ShiftLeft8(u8),
    ShiftRight8(u8),
    Or(u8, u8),
    Compare(u8, u8),
    TestSigned(u8, usize),
    ZeroBranch(u8, usize),
    LessBranch(usize),
    Branch(usize),
}

pub fn plan(load: bool, registers: [u8; 6], finish: Step) -> Vec<Step> {
    if load {
        let [address, codec, cursor, count, byte, result] = registers;
        vec![
            Step::ByteCount(count, codec),
            Step::Copy(cursor, count),
            Step::Zero(byte),
            Step::Zero(result),
            Step::SubOne(cursor),
            Step::LoadByte(result, address, cursor),
            Step::TestSigned(codec, 8),
            Step::SignByte(result),
            Step::ZeroBranch(cursor, 15),
            Step::SubOne(cursor),
            Step::ShiftLeft8(result),
            Step::LoadByte(byte, address, cursor),
            Step::Or(result, byte),
            Step::ZeroBranch(cursor, 15),
            Step::Branch(9),
            finish,
        ]
    } else {
        let [address, codec, value, cursor, count, payload] = registers;
        vec![
            Step::Zero(cursor),
            Step::ByteCount(count, codec),
            Step::Copy(payload, value),
            Step::StoreByte(payload, address, cursor),
            Step::ShiftRight8(payload),
            Step::AddOne(cursor),
            Step::Compare(cursor, count),
            Step::LessBranch(3),
        ]
    }
}

#[cfg(feature = "test-support")]
pub mod harness;

#[cfg(feature = "test-support")]
pub mod replay {
    use crate::Step;

    pub fn execute(
        bytes: &[u8],
        slots: usize,
        decode: impl Fn(&[u8], usize) -> (Step, usize),
        state: &mut [u64; 32],
        memory: &mut [u8],
    ) -> Vec<usize> {
        let mut position = 0;
        let mut less = false;
        let mut accesses = Vec::new();
        for _ in 0..128 {
            if position == slots {
                return accesses;
            }
            let (step, size) = decode(bytes, position);
            position += size;
            match step {
                Step::Zero(destination) => state[destination as usize] = 0,
                Step::Copy(destination, source) => {
                    state[destination as usize] = state[source as usize]
                }
                Step::ByteCount(destination, source) => {
                    state[destination as usize] = state[source as usize] & 255
                }
                Step::SubOne(register) => state[register as usize] -= 1,
                Step::AddOne(register) => state[register as usize] += 1,
                Step::LoadByte(destination, base, offset) => {
                    let address = (state[base as usize] + state[offset as usize]) as usize;
                    accesses.push(address);
                    state[destination as usize] = u64::from(memory[address]);
                }
                Step::StoreByte(source, base, offset) => {
                    let address = (state[base as usize] + state[offset as usize]) as usize;
                    accesses.push(address);
                    memory[address] = state[source as usize] as u8;
                }
                Step::SignByte(register) => {
                    state[register as usize] = state[register as usize] as i8 as i64 as u64
                }
                Step::ShiftLeft8(register) => state[register as usize] <<= 8,
                Step::ShiftRight8(register) => state[register as usize] >>= 8,
                Step::Or(destination, source) => {
                    state[destination as usize] |= state[source as usize]
                }
                Step::Compare(left, right) => less = state[left as usize] < state[right as usize],
                Step::TestSigned(register, target) => {
                    if state[register as usize] & 256 == 0 {
                        position = target;
                    }
                }
                Step::ZeroBranch(register, target) => {
                    if state[register as usize] == 0 {
                        position = target;
                    }
                }
                Step::LessBranch(target) => {
                    if less {
                        position = target;
                    }
                }
                Step::Branch(target) => position = target,
            }
        }
        panic!("codec loop did not terminate");
    }
}
