#![forbid(unsafe_code)]

//! The encoded-scalar codec program both ISA encoders lay out.
//!
//! A load walks the admitted byte count high-to-low and sign-extends the first
//! byte through constant shifts; a store walks low-to-high under the same count.
//! The streams differ — x86 bytes are variable-length, AArch64 words are fixed —
//! but the program a stream realizes is one program, so it is stated here once
//! and each ISA supplies only its instruction spellings and its terminal step.
//! Plan branch targets are step indices; each ISA resolves them in its own unit.

/// One step of the encoded-scalar codec program.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Zero(u8),
    Copy(u8, u8),
    /// The admitted byte count: the low byte of the codec word.
    ByteCount(u8, u8),
    SubOne(u8),
    AddOne(u8),
    /// One byte through a base register and a byte cursor.
    LoadByte(u8, u8, u8),
    /// One byte through a base register and a byte cursor.
    StoreByte(u8, u8, u8),
    SignByte(u8),
    ShiftLeft8(u8),
    ShiftRight8(u8),
    Or(u8, u8),
    Compare(u8, u8),
    /// Branch unless the codec word marks the scalar signed.
    TestSigned(u8, usize),
    /// Branch when a register reached zero.
    ZeroBranch(u8, usize),
    /// Branch when the byte loop's comparison said less.
    LessBranch(usize),
    Branch(usize),
}

/// The program the encoded scalar form runs over its six admitted registers, in
/// operand order.
///
/// A load admits `[address, codec, cursor, count, byte, result]` and needs
/// `finish` as the step its exit label lands on — x86 clears the cursor it
/// counted down, AArch64 compares so the emitted stream writes the condition
/// flags the form's footprint claims. A store admits
/// `[address, codec, value, cursor, count, payload]` and ends in its own
/// compare-and-branch, so it ignores `finish`.
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

/// The two sweeps both instruction sets' encoded-scalar tests run.
#[cfg(feature = "test-support")]
pub mod harness;

/// Replay of a decoded codec stream: the ISA crates' encoder tests run the
/// program a stream decodes to, and check the bytes it touches.
#[cfg(feature = "test-support")]
pub mod replay {
    use crate::Step;

    /// Run `bytes` through `decode` against the register file and memory, and
    /// report the byte addresses the program touched, in order.
    ///
    /// `decode` reports the step at a stream position and the position that
    /// follows it; `slots` is the stream length in that same unit. x86 positions
    /// are byte offsets and AArch64 positions are word indices, so the unit
    /// belongs to the decoder that reads the stream.
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
