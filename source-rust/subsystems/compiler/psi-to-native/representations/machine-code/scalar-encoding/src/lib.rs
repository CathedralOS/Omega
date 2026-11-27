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
streams. Under the `test-support` feature, `harness` drives the shared bit-flip
rejection and scratch-aliasing test sweep.
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
