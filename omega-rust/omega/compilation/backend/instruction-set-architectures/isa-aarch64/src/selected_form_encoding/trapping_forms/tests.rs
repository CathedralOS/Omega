//! Every Trapping realization is checked word for word against Apple clang,
//! executed at its carrier's edges against exact i128 arithmetic, and
//! rejected when its bytes, operands, alternative, or register roles change.
use super::{byte_size, realization};
use crate::aarch64_physical_register_model;
use crate::selected_form_encoding::decoding::{DecodedWord, decode_words};
use crate::selected_form_encoding::{
    encode_aarch64_selected_form, validate_aarch64_selected_form_encoding,
};
use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
    MachineEncodedTrapBehavior, SaturatingCarrier, SelectedInstructionKind, TrappingForm,
    TrappingOperation,
};
use semantic_vocabulary::IntegerSign;

/// Every form over `x1, x2 -> x3` with scratch `x4` (a conversion reads
/// `x1` into `x3` with scratch `x4`), written as assembly by hand from the
/// realization design and assembled independently with Apple clang 17; not
/// derived from this encoder.
const CLANG_TRAPPING_WORDS: [(TrappingOperation, SaturatingCarrier, &[u32]); 72] = [
    (
        TrappingOperation::Add,
        SaturatingCarrier::I8,
        &[0x8b02_0023, 0xeb23_807f, 0x5400_0040, 0xd420_0000],
    ),
    (
        TrappingOperation::Add,
        SaturatingCarrier::I16,
        &[0x8b02_0023, 0xeb23_a07f, 0x5400_0040, 0xd420_0000],
    ),
    (
        TrappingOperation::Add,
        SaturatingCarrier::I32,
        &[0x8b02_0023, 0xeb23_c07f, 0x5400_0040, 0xd420_0000],
    ),
    (
        TrappingOperation::Add,
        SaturatingCarrier::I64,
        &[0xab02_0023, 0x5400_0047, 0xd420_0000],
    ),
    (
        TrappingOperation::Add,
        SaturatingCarrier::U8,
        &[0x8b02_0023, 0xeb23_007f, 0x5400_0040, 0xd420_0000],
    ),
    (
        TrappingOperation::Add,
        SaturatingCarrier::U16,
        &[0x8b02_0023, 0xeb23_207f, 0x5400_0040, 0xd420_0000],
    ),
    (
        TrappingOperation::Add,
        SaturatingCarrier::U32,
        &[0x8b02_0023, 0xeb23_407f, 0x5400_0040, 0xd420_0000],
    ),
    (
        TrappingOperation::Add,
        SaturatingCarrier::U64,
        &[0xab02_0023, 0x5400_0043, 0xd420_0000],
    ),
    (
        TrappingOperation::Subtract,
        SaturatingCarrier::I8,
        &[0xcb02_0023, 0xeb23_807f, 0x5400_0040, 0xd420_0000],
    ),
    (
        TrappingOperation::Subtract,
        SaturatingCarrier::I16,
        &[0xcb02_0023, 0xeb23_a07f, 0x5400_0040, 0xd420_0000],
    ),
    (
        TrappingOperation::Subtract,
        SaturatingCarrier::I32,
        &[0xcb02_0023, 0xeb23_c07f, 0x5400_0040, 0xd420_0000],
    ),
    (
        TrappingOperation::Subtract,
        SaturatingCarrier::I64,
        &[0xeb02_0023, 0x5400_0047, 0xd420_0000],
    ),
    (
        TrappingOperation::Subtract,
        SaturatingCarrier::U8,
        &[0xcb02_0023, 0xeb23_007f, 0x5400_0040, 0xd420_0000],
    ),
    (
        TrappingOperation::Subtract,
        SaturatingCarrier::U16,
        &[0xcb02_0023, 0xeb23_207f, 0x5400_0040, 0xd420_0000],
    ),
    (
        TrappingOperation::Subtract,
        SaturatingCarrier::U32,
        &[0xcb02_0023, 0xeb23_407f, 0x5400_0040, 0xd420_0000],
    ),
    (
        TrappingOperation::Subtract,
        SaturatingCarrier::U64,
        &[0xeb02_0023, 0x5400_0042, 0xd420_0000],
    ),
    (
        TrappingOperation::Multiply,
        SaturatingCarrier::I8,
        &[0x9b02_7c23, 0xeb23_807f, 0x5400_0040, 0xd420_0000],
    ),
    (
        TrappingOperation::Multiply,
        SaturatingCarrier::I16,
        &[0x9b02_7c23, 0xeb23_a07f, 0x5400_0040, 0xd420_0000],
    ),
    (
        TrappingOperation::Multiply,
        SaturatingCarrier::I32,
        &[0x9b02_7c23, 0xeb23_c07f, 0x5400_0040, 0xd420_0000],
    ),
    (
        TrappingOperation::Multiply,
        SaturatingCarrier::I64,
        &[
            0x9b42_7c24,
            0x9b02_7c23,
            0xeb83_fc9f,
            0x5400_0040,
            0xd420_0000,
        ],
    ),
    (
        TrappingOperation::Multiply,
        SaturatingCarrier::U8,
        &[0x9b02_7c23, 0xeb23_007f, 0x5400_0040, 0xd420_0000],
    ),
    (
        TrappingOperation::Multiply,
        SaturatingCarrier::U16,
        &[0x9b02_7c23, 0xeb23_207f, 0x5400_0040, 0xd420_0000],
    ),
    (
        TrappingOperation::Multiply,
        SaturatingCarrier::U32,
        &[0x9b02_7c23, 0xeb23_407f, 0x5400_0040, 0xd420_0000],
    ),
    (
        TrappingOperation::Multiply,
        SaturatingCarrier::U64,
        &[
            0x9bc2_7c24,
            0x9b02_7c23,
            0xf100_009f,
            0x5400_0040,
            0xd420_0000,
        ],
    ),
    (
        TrappingOperation::Divide,
        SaturatingCarrier::I8,
        &[
            0xb500_0042,
            0xd420_0000,
            0x9ac2_0c23,
            0xeb23_807f,
            0x5400_0040,
            0xd420_0000,
        ],
    ),
    (
        TrappingOperation::Divide,
        SaturatingCarrier::I16,
        &[
            0xb500_0042,
            0xd420_0000,
            0x9ac2_0c23,
            0xeb23_a07f,
            0x5400_0040,
            0xd420_0000,
        ],
    ),
    (
        TrappingOperation::Divide,
        SaturatingCarrier::I32,
        &[
            0xb500_0042,
            0xd420_0000,
            0x9ac2_0c23,
            0xeb23_c07f,
            0x5400_0040,
            0xd420_0000,
        ],
    ),
    (
        TrappingOperation::Divide,
        SaturatingCarrier::I64,
        &[
            0xb500_0042,
            0xd420_0000,
            0xb100_045f,
            0xfa41_0820,
            0x5400_0047,
            0xd420_0000,
            0x9ac2_0c23,
        ],
    ),
    (
        TrappingOperation::Divide,
        SaturatingCarrier::U8,
        &[0xb500_0042, 0xd420_0000, 0x9ac2_0823],
    ),
    (
        TrappingOperation::Divide,
        SaturatingCarrier::U16,
        &[0xb500_0042, 0xd420_0000, 0x9ac2_0823],
    ),
    (
        TrappingOperation::Divide,
        SaturatingCarrier::U32,
        &[0xb500_0042, 0xd420_0000, 0x9ac2_0823],
    ),
    (
        TrappingOperation::Divide,
        SaturatingCarrier::U64,
        &[0xb500_0042, 0xd420_0000, 0x9ac2_0823],
    ),
    (
        TrappingOperation::Remainder,
        SaturatingCarrier::I8,
        &[
            0xb500_0042,
            0xd420_0000,
            0x9ac2_0c24,
            0xeb24_809f,
            0x5400_0040,
            0xd420_0000,
            0x9b02_8483,
        ],
    ),
    (
        TrappingOperation::Remainder,
        SaturatingCarrier::I16,
        &[
            0xb500_0042,
            0xd420_0000,
            0x9ac2_0c24,
            0xeb24_a09f,
            0x5400_0040,
            0xd420_0000,
            0x9b02_8483,
        ],
    ),
    (
        TrappingOperation::Remainder,
        SaturatingCarrier::I32,
        &[
            0xb500_0042,
            0xd420_0000,
            0x9ac2_0c24,
            0xeb24_c09f,
            0x5400_0040,
            0xd420_0000,
            0x9b02_8483,
        ],
    ),
    (
        TrappingOperation::Remainder,
        SaturatingCarrier::I64,
        &[
            0xb500_0042,
            0xd420_0000,
            0xb100_045f,
            0xfa41_0820,
            0x5400_0047,
            0xd420_0000,
            0x9ac2_0c24,
            0x9b02_8483,
        ],
    ),
    (
        TrappingOperation::Remainder,
        SaturatingCarrier::U8,
        &[0xb500_0042, 0xd420_0000, 0x9ac2_0824, 0x9b02_8483],
    ),
    (
        TrappingOperation::Remainder,
        SaturatingCarrier::U16,
        &[0xb500_0042, 0xd420_0000, 0x9ac2_0824, 0x9b02_8483],
    ),
    (
        TrappingOperation::Remainder,
        SaturatingCarrier::U32,
        &[0xb500_0042, 0xd420_0000, 0x9ac2_0824, 0x9b02_8483],
    ),
    (
        TrappingOperation::Remainder,
        SaturatingCarrier::U64,
        &[0xb500_0042, 0xd420_0000, 0x9ac2_0824, 0x9b02_8483],
    ),
    (
        TrappingOperation::ShiftLeft,
        SaturatingCarrier::I8,
        &[
            0xf100_205f,
            0x5400_0043,
            0xd420_0000,
            0x9ac2_2023,
            0xeb23_807f,
            0x5400_0040,
            0xd420_0000,
        ],
    ),
    (
        TrappingOperation::ShiftLeft,
        SaturatingCarrier::I16,
        &[
            0xf100_405f,
            0x5400_0043,
            0xd420_0000,
            0x9ac2_2023,
            0xeb23_a07f,
            0x5400_0040,
            0xd420_0000,
        ],
    ),
    (
        TrappingOperation::ShiftLeft,
        SaturatingCarrier::I32,
        &[
            0xf100_805f,
            0x5400_0043,
            0xd420_0000,
            0x9ac2_2023,
            0xeb23_c07f,
            0x5400_0040,
            0xd420_0000,
        ],
    ),
    (
        TrappingOperation::ShiftLeft,
        SaturatingCarrier::I64,
        &[
            0xf101_005f,
            0x5400_0043,
            0xd420_0000,
            0x9ac2_2023,
            0x9ac2_2864,
            0xeb01_009f,
            0x5400_0040,
            0xd420_0000,
        ],
    ),
    (
        TrappingOperation::ShiftLeft,
        SaturatingCarrier::U8,
        &[
            0xf100_205f,
            0x5400_0043,
            0xd420_0000,
            0x9ac2_2023,
            0xeb23_007f,
            0x5400_0040,
            0xd420_0000,
        ],
    ),
    (
        TrappingOperation::ShiftLeft,
        SaturatingCarrier::U16,
        &[
            0xf100_405f,
            0x5400_0043,
            0xd420_0000,
            0x9ac2_2023,
            0xeb23_207f,
            0x5400_0040,
            0xd420_0000,
        ],
    ),
    (
        TrappingOperation::ShiftLeft,
        SaturatingCarrier::U32,
        &[
            0xf100_805f,
            0x5400_0043,
            0xd420_0000,
            0x9ac2_2023,
            0xeb23_407f,
            0x5400_0040,
            0xd420_0000,
        ],
    ),
    (
        TrappingOperation::ShiftLeft,
        SaturatingCarrier::U64,
        &[
            0xf101_005f,
            0x5400_0043,
            0xd420_0000,
            0x9ac2_2023,
            0x9ac2_2464,
            0xeb01_009f,
            0x5400_0040,
            0xd420_0000,
        ],
    ),
    (
        TrappingOperation::ShiftRight,
        SaturatingCarrier::I8,
        &[0xf100_205f, 0x5400_0043, 0xd420_0000, 0x9ac2_2823],
    ),
    (
        TrappingOperation::ShiftRight,
        SaturatingCarrier::I16,
        &[0xf100_405f, 0x5400_0043, 0xd420_0000, 0x9ac2_2823],
    ),
    (
        TrappingOperation::ShiftRight,
        SaturatingCarrier::I32,
        &[0xf100_805f, 0x5400_0043, 0xd420_0000, 0x9ac2_2823],
    ),
    (
        TrappingOperation::ShiftRight,
        SaturatingCarrier::I64,
        &[0xf101_005f, 0x5400_0043, 0xd420_0000, 0x9ac2_2823],
    ),
    (
        TrappingOperation::ShiftRight,
        SaturatingCarrier::U8,
        &[0xf100_205f, 0x5400_0043, 0xd420_0000, 0x9ac2_2423],
    ),
    (
        TrappingOperation::ShiftRight,
        SaturatingCarrier::U16,
        &[0xf100_405f, 0x5400_0043, 0xd420_0000, 0x9ac2_2423],
    ),
    (
        TrappingOperation::ShiftRight,
        SaturatingCarrier::U32,
        &[0xf100_805f, 0x5400_0043, 0xd420_0000, 0x9ac2_2423],
    ),
    (
        TrappingOperation::ShiftRight,
        SaturatingCarrier::U64,
        &[0xf101_005f, 0x5400_0043, 0xd420_0000, 0x9ac2_2423],
    ),
    (
        TrappingOperation::Convert {
            source: IntegerSign::Signed,
        },
        SaturatingCarrier::I8,
        &[0xeb21_803f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3],
    ),
    (
        TrappingOperation::Convert {
            source: IntegerSign::Signed,
        },
        SaturatingCarrier::I16,
        &[0xeb21_a03f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3],
    ),
    (
        TrappingOperation::Convert {
            source: IntegerSign::Signed,
        },
        SaturatingCarrier::I32,
        &[0xeb21_c03f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3],
    ),
    (
        TrappingOperation::Convert {
            source: IntegerSign::Signed,
        },
        SaturatingCarrier::I64,
        &[0xaa01_03e3],
    ),
    (
        TrappingOperation::Convert {
            source: IntegerSign::Signed,
        },
        SaturatingCarrier::U8,
        &[0xf278_dc3f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3],
    ),
    (
        TrappingOperation::Convert {
            source: IntegerSign::Signed,
        },
        SaturatingCarrier::U16,
        &[0xf270_bc3f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3],
    ),
    (
        TrappingOperation::Convert {
            source: IntegerSign::Signed,
        },
        SaturatingCarrier::U32,
        &[0xf260_7c3f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3],
    ),
    (
        TrappingOperation::Convert {
            source: IntegerSign::Signed,
        },
        SaturatingCarrier::U64,
        &[0xf241_003f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3],
    ),
    (
        TrappingOperation::Convert {
            source: IntegerSign::Unsigned,
        },
        SaturatingCarrier::I8,
        &[0xf279_e03f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3],
    ),
    (
        TrappingOperation::Convert {
            source: IntegerSign::Unsigned,
        },
        SaturatingCarrier::I16,
        &[0xf271_c03f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3],
    ),
    (
        TrappingOperation::Convert {
            source: IntegerSign::Unsigned,
        },
        SaturatingCarrier::I32,
        &[0xf261_803f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3],
    ),
    (
        TrappingOperation::Convert {
            source: IntegerSign::Unsigned,
        },
        SaturatingCarrier::I64,
        &[0xf241_003f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3],
    ),
    (
        TrappingOperation::Convert {
            source: IntegerSign::Unsigned,
        },
        SaturatingCarrier::U8,
        &[0xf278_dc3f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3],
    ),
    (
        TrappingOperation::Convert {
            source: IntegerSign::Unsigned,
        },
        SaturatingCarrier::U16,
        &[0xf270_bc3f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3],
    ),
    (
        TrappingOperation::Convert {
            source: IntegerSign::Unsigned,
        },
        SaturatingCarrier::U32,
        &[0xf260_7c3f, 0x5400_0040, 0xd420_0000, 0xaa01_03e3],
    ),
    (
        TrappingOperation::Convert {
            source: IntegerSign::Unsigned,
        },
        SaturatingCarrier::U64,
        &[0xaa01_03e3],
    ),
];

fn physical() -> ValidatedPhysicalRegisterModel {
    register_model::validate_physical_register_model(aarch64_physical_register_model()).unwrap()
}

fn view(physical: &ValidatedPhysicalRegisterModel, name: &str) -> RegisterViewId {
    physical.model().view_named(name).unwrap().id
}

fn canonical_operands(
    physical: &ValidatedPhysicalRegisterModel,
    form: TrappingForm,
) -> Vec<RegisterViewId> {
    let names: &[&str] = if form.source_count() == 1 {
        &["x1", "x3", "x4"]
    } else {
        &["x1", "x2", "x3", "x4"]
    };
    names.iter().map(|name| view(physical, name)).collect()
}

fn key(form: TrappingForm) -> MachineAlternativeKey {
    MachineAlternativeKey {
        family: MachineAlternativeFamily::TrappingInteger(form),
        variant: 0,
    }
}

fn little_endian(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

#[test]
fn every_trapping_form_matches_clang_and_declares_its_trap() {
    let physical = physical();
    assert_eq!(TrappingForm::ALL.len(), CLANG_TRAPPING_WORDS.len());
    for (operation, carrier, words) in CLANG_TRAPPING_WORDS {
        let form = TrappingForm { operation, carrier };
        let kind = SelectedInstructionKind::TrappingInteger { form };
        let operands = canonical_operands(&physical, form);
        let encoded = encode_aarch64_selected_form(&physical, kind, key(form), &operands)
            .unwrap_or_else(|error| panic!("{form:?} encodes: {error:?}"));
        assert_eq!(encoded.bytes(), little_endian(words), "{form:?}");
        assert_eq!(usize::from(byte_size(form)), 4 * words.len(), "{form:?}");
        let footprint = encoded.footprint();
        let sources = form.source_count();
        assert_eq!(footprint.register_reads, operands[..sources]);
        assert_eq!(footprint.register_writes, operands[sources..]);
        assert_eq!(
            footprint.encoded.implicit_unit_clobbers,
            physical.model().view_named("nzcv").unwrap().units
        );
        assert!(footprint.encoded.implicit_unit_defs.is_empty());
        assert_eq!(
            footprint.encoded.trap,
            MachineEncodedTrapBehavior::TrappingIntegerV1
        );
        assert_eq!(
            footprint.encoded.control,
            MachineEncodedControlEffect::FallThroughOrTrapV1
        );
        // Every trap is the Crash leaf's word, reached only by falling past
        // the one skip branch in front of it.
        let decoded = decode_words(encoded.bytes()).unwrap();
        for (index, word) in decoded.iter().enumerate() {
            if *word == DecodedWord::Crash {
                assert!(matches!(
                    decoded[index - 1],
                    DecodedWord::BranchOverTrap { .. } | DecodedWord::BranchNonZeroOverTrap { .. }
                ));
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Value(u64),
    Trap,
}

#[derive(Default)]
struct Flags {
    zero: bool,
    carry: bool,
    overflow: bool,
}

impl Flags {
    fn subtract(left: u64, right: u64) -> Self {
        let result = left.wrapping_sub(right);
        Self {
            zero: result == 0,
            carry: left >= right,
            overflow: (left as i64).checked_sub(right as i64).is_none(),
        }
    }

    fn add(left: u64, right: u64) -> Self {
        let result = left.wrapping_add(right);
        Self {
            zero: result == 0,
            carry: left.checked_add(right).is_none(),
            overflow: (left as i64).checked_add(right as i64).is_none(),
        }
    }

    fn holds(&self, condition: u8) -> bool {
        match condition {
            0 => self.zero,
            1 => !self.zero,
            2 => self.carry,
            3 => !self.carry,
            6 => self.overflow,
            7 => !self.overflow,
            _ => panic!("condition {condition} is not a Trapping skip condition"),
        }
    }
}

fn extend(value: u64, extension: u8) -> u64 {
    match extension {
        0 => value as u8 as u64,
        1 => value as u16 as u64,
        2 => value as u32 as u64,
        4 => value as i8 as i64 as u64,
        5 => value as i16 as i64 as u64,
        6 => value as i32 as i64 as u64,
        _ => panic!("extension {extension} is not a narrow carrier's"),
    }
}

/// A test-only AArch64 executor over the decoded Trapping vocabulary.
fn execute(decoded: &[DecodedWord], inputs: &[(u8, u64)], result: u8) -> Outcome {
    let mut registers = [0_u64; 32];
    for (register, value) in inputs {
        registers[usize::from(*register)] = *value;
    }
    let mut flags = Flags::default();
    let mut index = 0;
    while index < decoded.len() {
        let mut next = index + 1;
        let read = |register: u8| registers[usize::from(register)];
        match decoded[index] {
            DecodedWord::Crash => return Outcome::Trap,
            DecodedWord::BranchOverTrap { condition } => {
                if flags.holds(condition) {
                    next += 1;
                }
            }
            DecodedWord::BranchNonZeroOverTrap { register } => {
                if read(register) != 0 {
                    next += 1;
                }
            }
            DecodedWord::AddWithFlags {
                left,
                right,
                destination,
            } => {
                flags = Flags::add(read(left), read(right));
                registers[usize::from(destination)] = read(left).wrapping_add(read(right));
            }
            DecodedWord::SubtractWithFlags {
                left,
                right,
                destination,
            } => {
                flags = Flags::subtract(read(left), read(right));
                registers[usize::from(destination)] = read(left).wrapping_sub(read(right));
            }
            DecodedWord::Add {
                left,
                right,
                destination,
            } => {
                registers[usize::from(destination)] = read(left).wrapping_add(read(right));
            }
            DecodedWord::Subtract {
                left,
                right,
                destination,
            } => {
                registers[usize::from(destination)] = read(left).wrapping_sub(read(right));
            }
            DecodedWord::Multiply {
                left,
                right,
                destination,
            } => {
                registers[usize::from(destination)] = read(left).wrapping_mul(read(right));
            }
            DecodedWord::SignedMultiplyHigh {
                left,
                right,
                destination,
            } => {
                let product = i128::from(read(left) as i64) * i128::from(read(right) as i64);
                registers[usize::from(destination)] = (product >> 64) as u64;
            }
            DecodedWord::UnsignedMultiplyHigh {
                left,
                right,
                destination,
            } => {
                let product = u128::from(read(left)) * u128::from(read(right));
                registers[usize::from(destination)] = (product >> 64) as u64;
            }
            DecodedWord::CompareWithSignOf { high, low } => {
                flags = Flags::subtract(read(high), ((read(low) as i64) >> 63) as u64);
            }
            DecodedWord::CompareZero { source } => flags = Flags::subtract(read(source), 0),
            DecodedWord::CompareImmediate { source, immediate } => {
                flags = Flags::subtract(read(source), u64::from(immediate));
            }
            DecodedWord::Compare { left, right } => {
                flags = Flags::subtract(read(left), read(right));
            }
            DecodedWord::CompareExtended {
                left,
                right,
                extension,
            } => {
                flags = Flags::subtract(read(left), extend(read(right), extension));
            }
            DecodedWord::CompareNegativeOne { source } => flags = Flags::add(read(source), 1),
            DecodedWord::ConditionalCompareOneOnEqual { register } => {
                flags = if flags.zero {
                    Flags::subtract(read(register), 1)
                } else {
                    Flags::default()
                };
            }
            DecodedWord::TestHighBits { source, from_bit } => {
                let masked = read(source) & (u64::MAX << from_bit);
                flags = Flags {
                    zero: masked == 0,
                    carry: false,
                    overflow: false,
                };
            }
            DecodedWord::SignedDivide {
                dividend,
                divisor,
                destination,
            } => {
                let (dividend, divisor) = (read(dividend) as i64, read(divisor) as i64);
                registers[usize::from(destination)] = if divisor == 0 {
                    0
                } else {
                    dividend.wrapping_div(divisor) as u64
                };
            }
            DecodedWord::UnsignedDivide {
                dividend,
                divisor,
                destination,
            } => {
                registers[usize::from(destination)] =
                    read(dividend).checked_div(read(divisor)).unwrap_or(0);
            }
            DecodedWord::MultiplySubtract {
                left,
                right,
                minuend,
                destination,
            } => {
                registers[usize::from(destination)] =
                    read(minuend).wrapping_sub(read(left).wrapping_mul(read(right)));
            }
            DecodedWord::ShiftLeftVariable {
                value,
                count,
                destination,
            } => {
                registers[usize::from(destination)] = read(value) << (read(count) & 63);
            }
            DecodedWord::ShiftRightLogicalVariable {
                value,
                count,
                destination,
            } => {
                registers[usize::from(destination)] = read(value) >> (read(count) & 63);
            }
            DecodedWord::ShiftRightArithmeticVariable {
                value,
                count,
                destination,
            } => {
                registers[usize::from(destination)] =
                    ((read(value) as i64) >> (read(count) & 63)) as u64;
            }
            DecodedWord::Copy {
                source,
                destination,
            } => {
                registers[usize::from(destination)] = read(source);
            }
            other => panic!("{other:?} is not a Trapping realization word"),
        }
        index = next;
    }
    Outcome::Value(registers[usize::from(result)])
}

fn bounds(carrier: SaturatingCarrier) -> (i128, i128) {
    let bits = u32::from(carrier.bits());
    if carrier.is_signed() {
        (-(1_i128 << (bits - 1)), (1_i128 << (bits - 1)) - 1)
    } else {
        (0, (1_i128 << bits) - 1)
    }
}

/// The exact Trapping answer: the mathematical result when every predicate
/// of the settled `numerics::integer_policy` row is false, otherwise a trap.
fn reference(form: TrappingForm, left: i128, right: i128) -> Outcome {
    let (minimum, maximum) = bounds(form.carrier);
    let width = i128::from(form.carrier.bits());
    let exact = match form.operation {
        TrappingOperation::Add => Some(left + right),
        TrappingOperation::Subtract => Some(left - right),
        TrappingOperation::Multiply => left.checked_mul(right),
        TrappingOperation::Divide => (right != 0).then(|| left / right),
        TrappingOperation::Remainder => (right != 0)
            .then(|| left / right)
            .filter(|quotient| (minimum..=maximum).contains(quotient))
            .map(|_| left % right),
        TrappingOperation::ShiftLeft => (0..width).contains(&right).then(|| left << right),
        TrappingOperation::ShiftRight => (0..width).contains(&right).then(|| left >> right),
        TrappingOperation::Convert { .. } => Some(left),
    };
    match exact {
        Some(value) if (minimum..=maximum).contains(&value) => Outcome::Value(value as i64 as u64),
        _ => Outcome::Trap,
    }
}

fn register(value: i128) -> u64 {
    value as i64 as u64
}

/// Boundary values of one carrier and their neighbors.
fn edges(carrier: SaturatingCarrier) -> Vec<i128> {
    let (minimum, maximum) = bounds(carrier);
    let mut values = vec![
        minimum,
        minimum + 1,
        -2,
        -1,
        0,
        1,
        2,
        3,
        7,
        maximum - 1,
        maximum,
    ];
    values.extend([maximum / 2, maximum / 2 + 1, minimum / 2, minimum / 2 - 1]);
    values.retain(|value| (minimum..=maximum).contains(value));
    values.sort_unstable();
    values.dedup();
    values
}

/// Counts of every fixed native count type around the width boundaries.
fn counts(width: i128) -> Vec<(i128, u64)> {
    [-129, -1, 0, 1, width - 1, width, width + 1, 63, 64, 65, 200]
        .into_iter()
        .map(|count| (count, register(count)))
        .collect()
}

#[test]
fn every_trapping_form_executes_the_policy_answer_at_its_carriers_edges() {
    let mut checked = 0;
    for form in TrappingForm::ALL {
        let registers: &[u8] = if form.source_count() == 1 {
            &[1, 3, 4]
        } else {
            &[1, 2, 3, 4]
        };
        let decoded = realization(form, registers);
        let result = registers[form.source_count()];
        match form.operation {
            TrappingOperation::Convert { source } => {
                for source_carrier in SaturatingCarrier::ALL
                    .into_iter()
                    .filter(|carrier| carrier.sign() == source)
                {
                    for value in edges(source_carrier) {
                        let actual = execute(&decoded, &[(1, register(value))], result);
                        assert_eq!(actual, reference(form, value, 0), "{form:?} {value}");
                        checked += 1;
                    }
                }
            }
            TrappingOperation::ShiftLeft | TrappingOperation::ShiftRight => {
                for value in edges(form.carrier) {
                    for (count, count_register) in counts(i128::from(form.carrier.bits())) {
                        let actual = execute(
                            &decoded,
                            &[(1, register(value)), (2, count_register)],
                            result,
                        );
                        assert_eq!(
                            actual,
                            reference(form, value, count),
                            "{form:?} {value} << {count}"
                        );
                        checked += 1;
                    }
                }
            }
            _ => {
                for left in edges(form.carrier) {
                    for right in edges(form.carrier) {
                        let actual = execute(
                            &decoded,
                            &[(1, register(left)), (2, register(right))],
                            result,
                        );
                        assert_eq!(
                            actual,
                            reference(form, left, right),
                            "{form:?} {left} {right}"
                        );
                        checked += 1;
                    }
                }
            }
        }
    }
    // Every form runs at least its carrier's edge grid.
    assert!(checked > 5_000, "{checked}");
}

#[test]
fn named_trapping_boundaries_trap_or_return_exactly() {
    let form = |operation, carrier| TrappingForm { operation, carrier };
    let run = |form: TrappingForm, left: i128, right: i128| {
        execute(
            &realization(form, &[1, 2, 3, 4]),
            &[(1, register(left)), (2, register(right))],
            3,
        )
    };
    let convert = |source, carrier, value: i128| {
        execute(
            &realization(
                TrappingForm {
                    operation: TrappingOperation::Convert { source },
                    carrier,
                },
                &[1, 3, 4],
            ),
            &[(1, register(value))],
            3,
        )
    };
    use SaturatingCarrier::{I8, I32, I64, U8, U32, U64};
    use TrappingOperation::{Add, Divide, Multiply, Remainder, ShiftLeft, ShiftRight, Subtract};
    assert_eq!(run(form(Add, U8), 200, 100), Outcome::Trap);
    assert_eq!(run(form(Add, U8), 200, 55), Outcome::Value(255));
    assert_eq!(run(form(Add, I32), i128::from(i32::MAX), 1), Outcome::Trap);
    assert_eq!(run(form(Add, I64), i128::from(i64::MAX), 1), Outcome::Trap);
    assert_eq!(run(form(Add, U64), i128::from(u64::MAX), 1), Outcome::Trap);
    assert_eq!(run(form(Subtract, U32), 0, 1), Outcome::Trap);
    assert_eq!(
        run(form(Subtract, I64), i128::from(i64::MIN), 1),
        Outcome::Trap
    );
    assert_eq!(
        run(form(Multiply, U32), i128::from(u32::MAX), 2),
        Outcome::Trap
    );
    assert_eq!(run(form(Multiply, I64), 1 << 32, 1 << 31), Outcome::Trap);
    assert_eq!(
        run(form(Multiply, I64), -(1 << 32), 1 << 31),
        Outcome::Value(i64::MIN as u64)
    );
    assert_eq!(run(form(Multiply, U64), 1 << 32, 1 << 32), Outcome::Trap);
    for carrier in [I8, I32, I64] {
        let (minimum, _) = bounds(carrier);
        assert_eq!(run(form(Divide, carrier), minimum, -1), Outcome::Trap);
        assert_eq!(run(form(Remainder, carrier), minimum, -1), Outcome::Trap);
        assert_eq!(
            run(form(Divide, carrier), minimum, 1),
            Outcome::Value(register(minimum))
        );
        assert_eq!(run(form(Divide, carrier), -1, -1), Outcome::Value(1));
        assert_eq!(run(form(Divide, carrier), 7, 0), Outcome::Trap);
        assert_eq!(
            run(form(Remainder, carrier), -7, 2),
            Outcome::Value(register(-1))
        );
    }
    assert_eq!(run(form(Divide, U64), 7, 0), Outcome::Trap);
    assert_eq!(
        run(form(Remainder, U64), i128::from(u64::MAX), 10),
        Outcome::Value(5)
    );
    assert_eq!(run(form(ShiftLeft, U32), 1, 32), Outcome::Trap);
    assert_eq!(run(form(ShiftLeft, U32), 1, 31), Outcome::Value(1 << 31));
    assert_eq!(run(form(ShiftLeft, I32), 1, 31), Outcome::Trap);
    assert_eq!(
        run(form(ShiftLeft, I64), -1, 63),
        Outcome::Value(i64::MIN as u64)
    );
    assert_eq!(run(form(ShiftLeft, U64), 3, 63), Outcome::Trap);
    assert_eq!(
        run(form(ShiftRight, I8), -128, 7),
        Outcome::Value(register(-1))
    );
    assert_eq!(run(form(ShiftRight, I8), -128, -1), Outcome::Trap);
    assert_eq!(run(form(ShiftRight, U64), 1, 64), Outcome::Trap);
    assert_eq!(
        convert(IntegerSign::Unsigned, I8, i128::from(u64::MAX)),
        Outcome::Trap
    );
    assert_eq!(convert(IntegerSign::Signed, U64, -1), Outcome::Trap);
    assert_eq!(convert(IntegerSign::Signed, U8, 255), Outcome::Value(255));
    assert_eq!(convert(IntegerSign::Signed, I8, -129), Outcome::Trap);
    assert_eq!(
        convert(IntegerSign::Unsigned, I64, i128::from(i64::MAX) + 1),
        Outcome::Trap
    );
    assert_eq!(
        convert(IntegerSign::Signed, I64, -1),
        Outcome::Value(u64::MAX)
    );
}

#[test]
fn corrupted_trapping_encodings_are_rejected() {
    let physical = physical();
    for form in TrappingForm::ALL {
        let kind = SelectedInstructionKind::TrappingInteger { form };
        let operands = canonical_operands(&physical, form);
        let bytes = encode_aarch64_selected_form(&physical, kind, key(form), &operands)
            .unwrap()
            .bytes()
            .to_vec();
        for bit in 0..bytes.len() * 8 {
            let mut corrupted = bytes.clone();
            corrupted[bit / 8] ^= 1 << (bit % 8);
            assert!(
                validate_aarch64_selected_form_encoding(
                    &physical,
                    kind,
                    key(form),
                    &operands,
                    &corrupted
                )
                .is_err(),
                "{form:?} bit {bit}"
            );
        }
        assert!(
            validate_aarch64_selected_form_encoding(
                &physical,
                kind,
                key(form),
                &operands,
                &bytes[4..]
            )
            .is_err()
        );
        assert!(encode_aarch64_selected_form(&physical, kind, key(form), &operands[1..]).is_err());
        assert!(
            encode_aarch64_selected_form(
                &physical,
                kind,
                MachineAlternativeKey {
                    variant: 1,
                    ..key(form)
                },
                &operands
            )
            .is_err()
        );
        // The early-clobber result may share a register with neither an
        // input nor the scratch.
        let sources = form.source_count();
        for aliased in [0, sources + 1] {
            let mut shared = operands.clone();
            shared[sources] = operands[aliased];
            assert!(encode_aarch64_selected_form(&physical, kind, key(form), &shared).is_err());
        }
    }
    // A sibling carrier's family does not validate these bytes.
    let form = TrappingForm {
        operation: TrappingOperation::Add,
        carrier: SaturatingCarrier::I32,
    };
    let other = TrappingForm {
        carrier: SaturatingCarrier::I16,
        ..form
    };
    let operands = canonical_operands(&physical, form);
    let bytes = encode_aarch64_selected_form(
        &physical,
        SelectedInstructionKind::TrappingInteger { form },
        key(form),
        &operands,
    )
    .unwrap()
    .bytes()
    .to_vec();
    assert!(
        validate_aarch64_selected_form_encoding(
            &physical,
            SelectedInstructionKind::TrappingInteger { form: other },
            key(other),
            &operands,
            &bytes
        )
        .is_err()
    );
}
