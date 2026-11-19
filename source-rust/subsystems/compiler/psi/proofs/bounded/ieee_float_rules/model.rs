//! Independent test oracle: all encodings of sign:1 / exponent:2 / fraction:2.
//! Bias is 1. Finite values are exact multiples of 1/4; exponent 3 is special.
//! No host floating arithmetic, FloatSemantics, or production normalizer is used.

use std::cmp::Ordering;

pub const ENCODINGS: usize = 32;
pub const TRIPLES_PER_FORMAT: usize = ENCODINGS * ENCODINGS * ENCODINGS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    F32,
    F64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Finite {
        quarters: i8,
    },
    Infinity {
        negative: bool,
    },
    NaN {
        negative: bool,
        quiet: bool,
        payload: u8,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Value {
    pub encoding: u8,
    pub class: Class,
    pub f32_bits: u32,
    pub f64_bits: u64,
}

impl Value {
    pub fn from_encoding(encoding: u8) -> Self {
        assert!(encoding < 32);
        let negative = encoding & 16 != 0;
        let exponent = (encoding >> 2) & 3;
        let fraction = encoding & 3;
        let class = if exponent == 3 {
            if fraction == 0 {
                Class::Infinity { negative }
            } else {
                Class::NaN {
                    negative,
                    quiet: fraction & 2 != 0,
                    payload: fraction & 1,
                }
            }
        } else {
            let magnitude = if exponent == 0 {
                fraction
            } else {
                (4 + fraction) << (exponent - 1)
            };
            Class::Finite {
                quarters: if negative {
                    -(magnitude as i8)
                } else {
                    magnitude as i8
                },
            }
        };
        Self {
            encoding,
            class,
            f32_bits: embed(negative, exponent, fraction, 8, 23) as u32,
            f64_bits: embed(negative, exponent, fraction, 11, 52),
        }
    }

    /// F32 bits occupy the low 32 bits. No conversion evaluates a signaling NaN.
    pub fn bits(self, format: Format) -> u64 {
        match format {
            Format::F32 => u64::from(self.f32_bits),
            Format::F64 => self.f64_bits,
        }
    }
}

fn embed(
    negative: bool,
    exponent: u8,
    fraction: u8,
    exponent_bits: u32,
    fraction_bits: u32,
) -> u64 {
    let sign = u64::from(negative) << (exponent_bits + fraction_bits);
    let max_exponent = (1u64 << exponent_bits) - 1;
    if exponent == 3 {
        // The model's high fraction bit maps to the IEEE quiet bit; the lower
        // payload bit maps immediately below it. Both NaN classes stay nonzero.
        return sign
            | (max_exponent << fraction_bits)
            | (u64::from(fraction) << (fraction_bits - 2));
    }
    let magnitude = if exponent == 0 {
        u32::from(fraction)
    } else {
        u32::from(4 + fraction) << (exponent - 1)
    };
    if magnitude == 0 {
        return sign;
    } // Preserve both signed zeros.
    let leading = 31 - magnitude.leading_zeros();
    let bias = (1i32 << (exponent_bits - 1)) - 1;
    let ieee_exponent = (leading as i32 - 2 + bias) as u64;
    let ieee_fraction = u64::from(magnitude - (1 << leading)) << (fraction_bits - leading);
    sign | (ieee_exponent << fraction_bits) | ieee_fraction
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Relations {
    pub equal: bool,
    pub not_equal: bool,
    pub less: bool,
    pub less_or_equal: bool,
    pub greater: bool,
    pub greater_or_equal: bool,
}

fn compare(left: Class, right: Class) -> Relations {
    use Class::*;
    let order = match (left, right) {
        (NaN { .. }, _) | (_, NaN { .. }) => None,
        (Finite { quarters: a }, Finite { quarters: b }) => Some(a.cmp(&b)),
        (Infinity { negative: a }, Infinity { negative: b }) => Some(b.cmp(&a)),
        (Infinity { negative }, _) => Some(if negative {
            Ordering::Less
        } else {
            Ordering::Greater
        }),
        (_, Infinity { negative }) => Some(if negative {
            Ordering::Greater
        } else {
            Ordering::Less
        }),
    };
    let equal = order == Some(Ordering::Equal);
    let less = order == Some(Ordering::Less);
    let greater = order == Some(Ordering::Greater);
    Relations {
        equal,
        not_equal: !equal,
        less,
        less_or_equal: less || equal,
        greater,
        greater_or_equal: greater || equal,
    }
}

/// The same exact relation table applies to both embeddings: every finite value
/// is represented exactly in both formats. Encodings still distinguish +/-0.
pub struct Model {
    pub values: [Value; ENCODINGS],
    pub pairs: [[Relations; ENCODINGS]; ENCODINGS],
}

impl Model {
    pub fn new() -> Self {
        let values = std::array::from_fn(|i| Value::from_encoding(i as u8));
        let pairs = std::array::from_fn(|a| {
            std::array::from_fn(|b| compare(values[a].class, values[b].class))
        });
        Self { values, pairs }
    }

    /// Exactly 32768 triples, including every NaN payload/sign and signed zero.
    pub fn triples(&self, format: Format) -> impl ExactSizeIterator<Item = Triple> + '_ {
        (0..TRIPLES_PER_FORMAT).map(move |index| {
            let [a, b, c] = [index / 1024, (index / 32) % 32, index % 32];
            Triple {
                format,
                values: [self.values[a], self.values[b], self.values[c]],
                ab: self.pairs[a][b],
                bc: self.pairs[b][c],
                ac: self.pairs[a][c],
            }
        })
    }
}

impl Default for Model {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Triple {
    pub format: Format,
    pub values: [Value; 3],
    pub ab: Relations,
    pub bc: Relations,
    pub ac: Relations,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ordered {
    Less,
    LessOrEqual,
}

impl Ordered {
    pub fn holds(self, relations: Relations) -> bool {
        match self {
            Self::Less => relations.less,
            Self::LessOrEqual => relations.less_or_equal,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Chain {
    pub left: Ordered,
    pub right: Ordered,
    pub conclusion: Ordered,
    pub premises_hold: [bool; 2],
    pub conclusion_holds: bool,
}

impl Triple {
    /// All four positive-order compositions. A strict premise requires a strict
    /// conclusion; two non-strict premises establish only non-strict order.
    /// False premises remain visible: never mint their proof from this oracle.
    pub fn chains(self) -> [Chain; 4] {
        use Ordered::{Less, LessOrEqual};
        [
            (LessOrEqual, LessOrEqual),
            (LessOrEqual, Less),
            (Less, LessOrEqual),
            (Less, Less),
        ]
        .map(|(left, right)| {
            let conclusion = if left == Less || right == Less {
                Less
            } else {
                LessOrEqual
            };
            Chain {
                left,
                right,
                conclusion,
                premises_hold: [left.holds(self.ab), right.holds(self.bc)],
                conclusion_holds: conclusion.holds(self.ac),
            }
        })
    }
}

// Usage in the real kernel test (this scaffold does not assert kernel results):
// let model = Model::new();
// for format in [Format::F32, Format::F64] {
//     for triple in model.triples(format) {
//         let bits = triple.values.map(|value| value.bits(format));
//         // Check the actual kernel's six literal judgments against ab/bc/ac.
//         for chain in triple.chains() {
//             // Construct genuine premise certificates when both premises hold;
//             // use independent malformed/missing-premise controls otherwise.
//             // Compare kernel acceptance with this explicit oracle, not itself.
//         }
//     }
// }
