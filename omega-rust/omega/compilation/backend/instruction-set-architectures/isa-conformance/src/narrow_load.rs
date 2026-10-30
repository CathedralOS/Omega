//! One narrow-load body over each instruction set's roster and bytes.
//!
//! A narrow load reads one or two bytes through a pointer operand and writes a
//! full, zero-extended register. The width and offset sweep, the footprint it
//! must bind, the exact opcodes and the refusals every stream owes are stated
//! once here; an instruction set answers with its roster and its bytes.

use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, SelectedInstructionKind,
};

/// What one validated narrow-load encoding must report; every assertion on it
/// is stated here, so neither target can bind a weaker effect.
#[derive(Debug, Clone, PartialEq)]
pub struct Footprint {
    pub bytes: Vec<u8>,
    pub register_reads: Vec<RegisterViewId>,
    pub register_writes: Vec<RegisterViewId>,
    pub memory: MachineEncodedMemoryEffect,
    pub stack: MachineEncodedStackEffect,
    pub trap: MachineEncodedTrapBehavior,
    pub flags_untouched: bool,
}

/// The base and destination views one instruction set admits, and the byte
/// offsets its addressing mode can spell at a given width.
pub struct Roster<'a> {
    pub bases: &'a [&'a str],
    pub destination: &'a str,
    pub offsets: fn(u16) -> Vec<u32>,
}

/// One instruction set's pinned encodings and the requests it must refuse.
pub struct Exact<'a> {
    /// The two operand views both widths are encoded with.
    pub operands: [&'a str; 2],
    /// The pinned stream per width, in the order `(width, bytes)`.
    pub expected: [(u16, &'a [u8]); 2],
    /// The opcode byte that distinguishes zero from sign extension, and its
    /// signed replacement for a one-byte and a two-byte load.
    pub signed_opcode: (usize, [u8; 2]),
    /// Views the instruction set cannot admit as either operand.
    pub invalid_views: &'a [&'a str],
    /// `(width, byte_offset)` requests the addressing mode cannot spell.
    pub invalid_offsets: &'a [(u16, u32)],
}

/// Sweep both widths over every admitted base and offset, require the footprint,
/// the round trip and every refusal a narrow load owes to its stream, then
/// require both widths' pinned opcodes and the refusal of every substituted
/// opcode, view and offset the instruction set cannot spell.
pub fn conformance<E: std::fmt::Debug>(
    physical: &ValidatedPhysicalRegisterModel,
    roster: &Roster<'_>,
    exact: &Exact<'_>,
    encode: impl Fn(
        SelectedInstructionKind,
        MachineAlternativeKey,
        &[RegisterViewId],
        i64,
    ) -> Result<Footprint, E>,
    validate: impl Fn(
        SelectedInstructionKind,
        MachineAlternativeKey,
        &[RegisterViewId],
        i64,
        &[u8],
    ) -> Result<Footprint, E>,
) {
    let rejects = |kind, alternative, operands: &[RegisterViewId], displacement, bytes: &[u8]| {
        validate(kind, alternative, operands, displacement, bytes).is_err()
    };
    for width in [1_u16, 2] {
        let family = family(width);
        let alternative = MachineAlternativeKey { family, variant: 0 };
        for &byte_offset in &(roster.offsets)(width) {
            let kind = load_kind(width, byte_offset);
            let displacement = i64::from(byte_offset);
            for base_name in roster.bases {
                let base = view(physical, base_name);
                let destination = view(physical, roster.destination);
                let operands = [base, destination];
                let reject = |operands: &[RegisterViewId], displacement, bytes: &[u8]| {
                    rejects(kind, alternative, operands, displacement, bytes)
                };
                let encoded = encode(kind, alternative, &operands, displacement)
                    .expect("admitted operands must encode");
                let expected = Footprint {
                    bytes: encoded.bytes.clone(),
                    register_reads: vec![base],
                    register_writes: vec![destination],
                    memory: MachineEncodedMemoryEffect::ReadPointerV1 {
                        pointer_operand: 0,
                        byte_count: width,
                    },
                    stack: MachineEncodedStackEffect::UnchangedV1,
                    trap: MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                    flags_untouched: true,
                };
                assert_eq!(encoded, expected, "{kind:?} {base_name}");
                let validated =
                    validate(kind, alternative, &operands, displacement, &encoded.bytes)
                        .expect("the emitted stream must validate");
                assert_eq!(
                    validated, encoded,
                    "{kind:?} {base_name} must validate to the encoding it replayed"
                );
                for bit in 0..encoded.bytes.len() * 8 {
                    let mut changed = encoded.bytes.clone();
                    changed[bit / 8] ^= 1 << (bit % 8);
                    assert!(
                        reject(&operands, displacement, &changed),
                        "{kind:?} {base_name} accepted changed bit {bit}"
                    );
                }
                for end in 0..encoded.bytes.len() {
                    assert!(
                        reject(&operands, displacement, &encoded.bytes[..end]),
                        "{kind:?} {base_name} accepted truncated prefix {end}"
                    );
                }
                let mut extended = encoded.bytes.clone();
                extended.push(0x90);
                assert!(
                    reject(&operands, displacement, &extended),
                    "{kind:?} {base_name} accepted an appended byte"
                );
                for changed_operands in [
                    [destination, base],
                    [destination, destination],
                    [base, base],
                ] {
                    assert!(
                        reject(&changed_operands, displacement, &encoded.bytes),
                        "{kind:?} {base_name} accepted substituted operands"
                    );
                }
                for neighbour in [displacement + 1, displacement + i64::from(width)] {
                    assert!(
                        reject(&operands, neighbour, &encoded.bytes),
                        "{kind:?} {base_name} accepted neighbour displacement {neighbour}"
                    );
                }
                assert!(
                    rejects(
                        load_kind(if width == 1 { 2 } else { 1 }, byte_offset),
                        alternative,
                        &operands,
                        displacement,
                        &encoded.bytes
                    ),
                    "{kind:?} {base_name} accepted the other width's kind"
                );
                assert!(
                    rejects(
                        kind,
                        MachineAlternativeKey { family, variant: 1 },
                        &operands,
                        displacement,
                        &encoded.bytes
                    ),
                    "{kind:?} {base_name} accepted an undeclared variant"
                );
                assert!(
                    reject(&operands[..1], displacement, &encoded.bytes),
                    "{kind:?} {base_name} accepted a shorter operand list"
                );
                // The complete address is read before the result is defined.
                assert!(
                    encode(kind, alternative, &[base, base], displacement).is_ok(),
                    "{kind:?} {base_name} refused its complete address read"
                );
            }
        }
    }

    let operands = exact.operands.map(|name| view(physical, name));
    for (width, expected) in exact.expected {
        let family = family(width);
        let alternative = MachineAlternativeKey { family, variant: 0 };
        let kind = load_kind(width, 0);
        let encoded =
            encode(kind, alternative, &operands, 0).expect("admitted operands must encode");
        assert_eq!(
            encoded.bytes, expected,
            "{kind:?} must emit its pinned stream"
        );
        let (index, [one_byte, two_bytes]) = exact.signed_opcode;
        let mut signed = expected.to_vec();
        signed[index] = if width == 1 { one_byte } else { two_bytes };
        assert!(
            validate(kind, alternative, &operands, 0, &signed).is_err(),
            "{kind:?} accepted the sign-extending opcode"
        );
        for name in exact.invalid_views {
            let invalid = view(physical, name);
            for changed in [[invalid, operands[1]], [operands[0], invalid]] {
                assert!(
                    encode(kind, alternative, &changed, 0).is_err(),
                    "{kind:?} admitted the view {name}"
                );
            }
        }
    }
    for &(width, byte_offset) in exact.invalid_offsets {
        let kind = load_kind(width, byte_offset);
        let alternative = MachineAlternativeKey {
            family: family(width),
            variant: 0,
        };
        assert!(
            encode(kind, alternative, &operands, i64::from(byte_offset)).is_err(),
            "a {width}-byte load admitted the offset {byte_offset}"
        );
    }
}

fn family(width: u16) -> MachineAlternativeFamily {
    match width {
        1 => MachineAlternativeFamily::Load8,
        _ => MachineAlternativeFamily::Load16,
    }
}

fn load_kind(width: u16, byte_offset: u32) -> SelectedInstructionKind {
    match width {
        1 => SelectedInstructionKind::Load8 { byte_offset },
        _ => SelectedInstructionKind::Load16 { byte_offset },
    }
}

fn view(physical: &ValidatedPhysicalRegisterModel, name: &str) -> RegisterViewId {
    physical
        .model()
        .view_named(name)
        .expect("the canonical model names every admitted view")
        .id
}
