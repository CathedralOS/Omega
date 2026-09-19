//! Closed catalog of the representation-free float semantic definitions the
//! sealed toolchain source declares as `FloatSemantics::<name>`.
//!
//! A row names one exact declaration by its source-visible path and its
//! complete normalized signature (parameter kinds in order and result kind),
//! so same-named overloads such as the eight `from_integer` carriers are
//! distinct rows and never collide. A bare bodyless
//! `machine FloatSemantics::add(...)` in the sealed source is admitted by
//! this identity and remains the proof-only definition the `Float::*` slot
//! contracts cite.
//!
//! Beside that identity every row carries its kernel-discharge binding: the
//! executable `FloatSemantics` definition that evaluates one application of
//! the row, with the operand and result shape that application takes. The
//! binding is stored beside the row literal and selected through the
//! complete signature by [`FloatSemanticOperation::from_source_identity`] --
//! it is never keyed on the leaf spelling, so a same-named overload cannot
//! pick up another carrier's discharge. [`FloatSemanticOperation::kernel_discharge`]
//! evaluates the application and pairs the result with the row's
//! [`FloatSemanticContractIdentity`], the artifact-relative identity an
//! independent verifier rejoins; the semantic-application proof value that
//! would carry that tuple through Terminal is the remaining open stage.
//!
//! As with the projection catalog, the row alone grants nothing. Ownership is
//! declaration custody -- the Toolchain-origin
//! [`FLOAT_PROJECTION_CORE_SOURCE`](crate::float_projection::FLOAT_PROJECTION_CORE_SOURCE)
//! file -- checked by symbol resolution before a signature lowers to the
//! catalog declaration and by validation on the typed shape afterwards.

use crate::bignum::BigInt;
use crate::float_semantics::{
    FloatClass, FloatFormat, FloatMeaning, FloatSemantics, IntegerFormat,
};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

/// Immutable version of the semantic-operation catalog. This is semantic
/// identity, not a display or codec version.
pub const FLOAT_SEMANTICS_CATALOG_VERSION: u16 = 1;
/// The single namespace every row lives under.
pub const FLOAT_SEMANTICS_NAMESPACE: &str = "FloatSemantics";
/// Sealed toolchain source (relative to the core package root) declaring the
/// `FloatFormat` selector the rows take as their first parameter.
pub const FLOAT_FORMAT_CORE_SOURCE: &str = "float_format.omg";
const FLOAT_SEMANTICS_CONTRACT_DOMAIN: &[u8] = b"psi-float-semantics-contract\0";

/// An integer carrier a semantic definition converts from or to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IntegerCarrier {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
}

impl IntegerCarrier {
    pub const fn spelling(self) -> &'static str {
        match self {
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
        }
    }

    pub fn from_spelling(spelling: &str) -> Option<Self> {
        Some(match spelling {
            "i8" => Self::I8,
            "i16" => Self::I16,
            "i32" => Self::I32,
            "i64" => Self::I64,
            "u8" => Self::U8,
            "u16" => Self::U16,
            "u32" => Self::U32,
            "u64" => Self::U64,
            _ => return None,
        })
    }

    /// The width and signedness format this carrier declares; its bounds are
    /// what an integer operand must satisfy in a discharged application.
    pub const fn format(self) -> IntegerFormat {
        match self {
            Self::I8 => IntegerFormat::I8,
            Self::I16 => IntegerFormat::I16,
            Self::I32 => IntegerFormat::I32,
            Self::I64 => IntegerFormat::I64,
            Self::U8 => IntegerFormat::U8,
            Self::U16 => IntegerFormat::U16,
            Self::U32 => IntegerFormat::U32,
            Self::U64 => IntegerFormat::U64,
        }
    }
}

/// The kind of one parameter or result position of a semantic definition.
/// Kinds are the catalog's signature vocabulary; the sealed toolchain types
/// they stand for (`FloatFormat`, `FloatMeaning`, `FloatClass`) are checked
/// by exact toolchain identity in validation, never by this spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FloatSemanticValueKind {
    /// The sealed `FloatFormat` selector.
    Format,
    /// The sealed proof-only `FloatMeaning` carrier.
    Meaning,
    /// The sealed `FloatClass` classification.
    Class,
    /// Builtin `bool`.
    Bool,
    /// One builtin integer carrier.
    Integer(IntegerCarrier),
}

impl FloatSemanticValueKind {
    /// Classify a source-visible type spelling. Symbol resolution uses this
    /// on the authored signature; the exact toolchain identity behind the
    /// sealed spellings is validation's separate check.
    pub fn from_spelling(spelling: &str) -> Option<Self> {
        Some(match spelling {
            "FloatFormat" => Self::Format,
            "FloatMeaning" => Self::Meaning,
            "FloatClass" => Self::Class,
            "bool" => Self::Bool,
            other => Self::Integer(IntegerCarrier::from_spelling(other)?),
        })
    }

    pub const fn spelling(self) -> &'static str {
        match self {
            Self::Format => "FloatFormat",
            Self::Meaning => "FloatMeaning",
            Self::Class => "FloatClass",
            Self::Bool => "bool",
            Self::Integer(carrier) => carrier.spelling(),
        }
    }
}

/// Source-free identity of one sealed semantic-definition row and the catalog
/// against which its complete signature was checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FloatSemanticContractIdentity {
    pub row: u8,
    pub catalog_version: u16,
    pub commitment: [u8; 32],
}

/// One exact row: the leaf name under [`FLOAT_SEMANTICS_NAMESPACE`], the
/// complete normalized signature, and the kernel-discharge binding stored
/// beside them.
///
/// Identity is the signature alone, so equality, ordering, and hashing read
/// `name`, `parameters`, and `result` and never the bound kernel: the kernel
/// is determined by the row, not a part of what makes two rows the same row.
#[derive(Debug, Clone, Copy)]
pub struct FloatSemanticOperation {
    pub name: &'static str,
    pub parameters: &'static [FloatSemanticValueKind],
    pub result: FloatSemanticValueKind,
    kernel: FloatSemanticKernel,
}

impl PartialEq for FloatSemanticOperation {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.parameters == other.parameters
            && self.result == other.result
    }
}

impl Eq for FloatSemanticOperation {}

impl PartialOrd for FloatSemanticOperation {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FloatSemanticOperation {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name
            .cmp(other.name)
            .then_with(|| self.parameters.cmp(other.parameters))
            .then_with(|| self.result.cmp(&other.result))
    }
}

impl Hash for FloatSemanticOperation {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.parameters.hash(state);
        self.result.hash(state);
    }
}

/// One evaluated operand in an application of a semantic-definition row. The
/// positions and kinds a row accepts come from its complete catalog
/// signature; `Format` operands must name one of the two sealed IEEE binary
/// format records and `Integer` operands must fit the declared carrier.
#[derive(Debug, Clone, PartialEq)]
pub enum FloatSemanticOperand {
    Format(FloatFormat),
    Meaning(FloatMeaning),
    Integer(BigInt),
}

/// The discharged result of applying one catalog row to evaluated operands.
///
/// `Meaning` results are proof-only values: their `==` is the payload-erased
/// meaning equality (`NaN` reflexive, signed zeros distinct) that a
/// `FloatMeaningEqual` proposition names. `Bool` results are the IEEE
/// comparison or classification predicate the row declares — runtime
/// comparison stays distinct from meaning equality.
#[derive(Debug, Clone, PartialEq)]
pub enum FloatSemanticResult {
    Meaning(FloatMeaning),
    Class(FloatClass),
    Bool(bool),
    Integer(BigInt),
}

/// One discharged application: the row's contract identity, which an
/// independent verifier rejoins from the artifact, paired with the result
/// the bound kernel produced. No evaluation can outrun this tuple — an
/// application that discharges under contract `C` proves only `C`'s row.
#[derive(Debug, Clone, PartialEq)]
pub struct FloatSemanticDischarge {
    pub contract: FloatSemanticContractIdentity,
    pub result: FloatSemanticResult,
}

/// The kernel-discharge binding stored beside a row: which executable
/// `FloatSemantics` definition evaluates one application, with the
/// operand/result shape fixed by the variant. Stored as function pointers so
/// the row literal names its kernel directly; the binding still reaches a
/// caller only through a row already selected by complete signature.
#[derive(Debug, Clone, Copy)]
pub enum FloatSemanticKernel {
    /// `(FloatFormat, FloatMeaning, FloatMeaning) -> FloatMeaning`.
    BinaryMeaning(fn(FloatFormat, &FloatMeaning, &FloatMeaning) -> FloatMeaning),
    /// `(FloatFormat, FloatMeaning) -> FloatMeaning`.
    UnaryMeaning(fn(FloatFormat, &FloatMeaning) -> FloatMeaning),
    /// `(FloatFormat, FloatMeaning, FloatMeaning, FloatMeaning) -> FloatMeaning`.
    TernaryMeaning(fn(FloatFormat, &FloatMeaning, &FloatMeaning, &FloatMeaning) -> FloatMeaning),
    /// `(FloatFormat, integer) -> FloatMeaning`; the row fixes the carrier.
    FromInteger(fn(FloatFormat, &BigInt) -> FloatMeaning),
    /// `FloatMeaning -> integer`; exact conversion, contract-gated per row.
    ToIntegerExact,
    /// `FloatMeaning -> integer`; saturating conversion.
    ToIntegerSaturating,
    /// `(FloatMeaning, FloatMeaning) -> FloatMeaning`.
    PairMeaning(fn(&FloatMeaning, &FloatMeaning) -> FloatMeaning),
    /// `(FloatMeaning, FloatMeaning) -> bool`.
    PairBool(fn(&FloatMeaning, &FloatMeaning) -> bool),
    /// `(FloatFormat, FloatMeaning) -> FloatClass`.
    UnaryClass(fn(FloatFormat, &FloatMeaning) -> FloatClass),
    /// `FloatMeaning -> bool`.
    SingleBool(fn(&FloatMeaning) -> bool),
    /// `(FloatFormat, FloatMeaning) -> bool`.
    UnaryBool(fn(FloatFormat, &FloatMeaning) -> bool),
}

impl FloatSemanticOperand {
    /// Whether this operand can stand at a position declared `kind`. The
    /// sealed formats are the only ones the kernel can evaluate, and an
    /// integer operand outside its declared carrier's bounds cannot be a
    /// value of that carrier.
    fn matches_kind(&self, kind: FloatSemanticValueKind) -> bool {
        match (self, kind) {
            (Self::Format(format), FloatSemanticValueKind::Format) => {
                *format == FloatFormat::BINARY32 || *format == FloatFormat::BINARY64
            }
            (Self::Meaning(_), FloatSemanticValueKind::Meaning) => true,
            (Self::Integer(value), FloatSemanticValueKind::Integer(carrier)) => {
                carrier.format().contains(value)
            }
            _ => false,
        }
    }

    fn as_format(&self) -> Option<FloatFormat> {
        match self {
            Self::Format(format) => Some(*format),
            _ => None,
        }
    }

    fn as_meaning(&self) -> Option<&FloatMeaning> {
        match self {
            Self::Meaning(meaning) => Some(meaning),
            _ => None,
        }
    }

    fn as_integer(&self) -> Option<&BigInt> {
        match self {
            Self::Integer(value) => Some(value),
            _ => None,
        }
    }
}

impl FloatSemanticResult {
    /// Whether this result satisfies the row's declared result kind, carrier
    /// bounds included for integer results.
    fn matches_kind(&self, kind: FloatSemanticValueKind) -> bool {
        match (self, kind) {
            (Self::Meaning(_), FloatSemanticValueKind::Meaning) => true,
            (Self::Class(_), FloatSemanticValueKind::Class) => true,
            (Self::Bool(_), FloatSemanticValueKind::Bool) => true,
            (Self::Integer(value), FloatSemanticValueKind::Integer(carrier)) => {
                carrier.format().contains(value)
            }
            _ => false,
        }
    }
}

impl FloatSemanticKernel {
    /// Evaluate the application. `result_kind` is the row's declared result;
    /// integer conversions take their target carrier from it. A `None` here
    /// means the kernel shape and the row signature disagree — a table defect
    /// the consistency tests reject — since callers have already matched
    /// operand kinds against the row.
    fn evaluate(
        self,
        result_kind: FloatSemanticValueKind,
        arguments: &[FloatSemanticOperand],
    ) -> Option<FloatSemanticResult> {
        match self {
            Self::BinaryMeaning(kernel) => {
                let [format, left, right] = arguments else {
                    return None;
                };
                Some(FloatSemanticResult::Meaning(kernel(
                    format.as_format()?,
                    left.as_meaning()?,
                    right.as_meaning()?,
                )))
            }
            Self::UnaryMeaning(kernel) => {
                let [format, value] = arguments else {
                    return None;
                };
                Some(FloatSemanticResult::Meaning(kernel(
                    format.as_format()?,
                    value.as_meaning()?,
                )))
            }
            Self::TernaryMeaning(kernel) => {
                let [format, left, right, addend] = arguments else {
                    return None;
                };
                Some(FloatSemanticResult::Meaning(kernel(
                    format.as_format()?,
                    left.as_meaning()?,
                    right.as_meaning()?,
                    addend.as_meaning()?,
                )))
            }
            Self::FromInteger(kernel) => {
                let [format, value] = arguments else {
                    return None;
                };
                Some(FloatSemanticResult::Meaning(kernel(
                    format.as_format()?,
                    value.as_integer()?,
                )))
            }
            Self::ToIntegerExact | Self::ToIntegerSaturating => {
                let [value] = arguments else {
                    return None;
                };
                let FloatSemanticValueKind::Integer(carrier) = result_kind else {
                    return None;
                };
                let value = value.as_meaning()?;
                let converted = match self {
                    Self::ToIntegerExact => {
                        FloatSemantics::to_integer_exact(value, carrier.format()).ok()
                    }
                    Self::ToIntegerSaturating => Some(FloatSemantics::to_integer_saturating(
                        value,
                        carrier.format(),
                    )),
                    _ => unreachable!("covered by the outer match arms"),
                };
                converted.map(FloatSemanticResult::Integer)
            }
            Self::PairMeaning(kernel) => {
                let [left, right] = arguments else {
                    return None;
                };
                Some(FloatSemanticResult::Meaning(kernel(
                    left.as_meaning()?,
                    right.as_meaning()?,
                )))
            }
            Self::PairBool(kernel) => {
                let [left, right] = arguments else {
                    return None;
                };
                Some(FloatSemanticResult::Bool(kernel(
                    left.as_meaning()?,
                    right.as_meaning()?,
                )))
            }
            Self::UnaryClass(kernel) => {
                let [format, value] = arguments else {
                    return None;
                };
                Some(FloatSemanticResult::Class(kernel(
                    format.as_format()?,
                    value.as_meaning()?,
                )))
            }
            Self::SingleBool(kernel) => {
                let [value] = arguments else {
                    return None;
                };
                Some(FloatSemanticResult::Bool(kernel(value.as_meaning()?)))
            }
            Self::UnaryBool(kernel) => {
                let [format, value] = arguments else {
                    return None;
                };
                Some(FloatSemanticResult::Bool(kernel(
                    format.as_format()?,
                    value.as_meaning()?,
                )))
            }
        }
    }
}

use FloatSemanticKernel::{
    BinaryMeaning, FromInteger, PairBool, PairMeaning, SingleBool, TernaryMeaning, ToIntegerExact,
    ToIntegerSaturating, UnaryBool, UnaryClass, UnaryMeaning,
};
use FloatSemanticValueKind::{Bool, Class, Format, Meaning};

const fn integer(carrier: IntegerCarrier) -> FloatSemanticValueKind {
    FloatSemanticValueKind::Integer(carrier)
}

const fn row(
    name: &'static str,
    parameters: &'static [FloatSemanticValueKind],
    result: FloatSemanticValueKind,
    kernel: FloatSemanticKernel,
) -> FloatSemanticOperation {
    FloatSemanticOperation {
        name,
        parameters,
        result,
        kernel,
    }
}

const BINARY: &[FloatSemanticValueKind] = &[Format, Meaning, Meaning];
const UNARY: &[FloatSemanticValueKind] = &[Format, Meaning];
const TERNARY: &[FloatSemanticValueKind] = &[Format, Meaning, Meaning, Meaning];
const PAIR: &[FloatSemanticValueKind] = &[Meaning, Meaning];
const SINGLE: &[FloatSemanticValueKind] = &[Meaning];

/// Every row, in the order the sealed source declares them. The position is
/// the row's stable ordinal in [`FloatSemanticContractIdentity`]. The third
/// argument of each `row` literal is the kernel-discharge binding: the exact
/// `FloatSemantics` definition an application of that row evaluates through.
pub const FLOAT_SEMANTIC_OPERATIONS: &[FloatSemanticOperation] = &[
    row("add", BINARY, Meaning, BinaryMeaning(FloatSemantics::add)),
    row(
        "subtract",
        BINARY,
        Meaning,
        BinaryMeaning(FloatSemantics::subtract),
    ),
    row(
        "multiply",
        BINARY,
        Meaning,
        BinaryMeaning(FloatSemantics::multiply),
    ),
    row(
        "divide",
        BINARY,
        Meaning,
        BinaryMeaning(FloatSemantics::divide),
    ),
    row(
        "negate",
        UNARY,
        Meaning,
        UnaryMeaning(FloatSemantics::negate),
    ),
    row(
        "square_root",
        UNARY,
        Meaning,
        UnaryMeaning(FloatSemantics::square_root),
    ),
    row(
        "convert",
        UNARY,
        Meaning,
        UnaryMeaning(FloatSemantics::convert),
    ),
    row(
        "from_integer",
        &[Format, integer(IntegerCarrier::I8)],
        Meaning,
        FromInteger(FloatSemantics::from_integer),
    ),
    row(
        "from_integer",
        &[Format, integer(IntegerCarrier::I16)],
        Meaning,
        FromInteger(FloatSemantics::from_integer),
    ),
    row(
        "from_integer",
        &[Format, integer(IntegerCarrier::I32)],
        Meaning,
        FromInteger(FloatSemantics::from_integer),
    ),
    row(
        "from_integer",
        &[Format, integer(IntegerCarrier::I64)],
        Meaning,
        FromInteger(FloatSemantics::from_integer),
    ),
    row(
        "from_integer",
        &[Format, integer(IntegerCarrier::U8)],
        Meaning,
        FromInteger(FloatSemantics::from_integer),
    ),
    row(
        "from_integer",
        &[Format, integer(IntegerCarrier::U16)],
        Meaning,
        FromInteger(FloatSemantics::from_integer),
    ),
    row(
        "from_integer",
        &[Format, integer(IntegerCarrier::U32)],
        Meaning,
        FromInteger(FloatSemantics::from_integer),
    ),
    row(
        "from_integer",
        &[Format, integer(IntegerCarrier::U64)],
        Meaning,
        FromInteger(FloatSemantics::from_integer),
    ),
    row("to_i8", SINGLE, integer(IntegerCarrier::I8), ToIntegerExact),
    row(
        "to_i16",
        SINGLE,
        integer(IntegerCarrier::I16),
        ToIntegerExact,
    ),
    row(
        "to_i32",
        SINGLE,
        integer(IntegerCarrier::I32),
        ToIntegerExact,
    ),
    row(
        "to_i64",
        SINGLE,
        integer(IntegerCarrier::I64),
        ToIntegerExact,
    ),
    row("to_u8", SINGLE, integer(IntegerCarrier::U8), ToIntegerExact),
    row(
        "to_u16",
        SINGLE,
        integer(IntegerCarrier::U16),
        ToIntegerExact,
    ),
    row(
        "to_u32",
        SINGLE,
        integer(IntegerCarrier::U32),
        ToIntegerExact,
    ),
    row(
        "to_u64",
        SINGLE,
        integer(IntegerCarrier::U64),
        ToIntegerExact,
    ),
    row(
        "to_i8_saturating",
        SINGLE,
        integer(IntegerCarrier::I8),
        ToIntegerSaturating,
    ),
    row(
        "to_i16_saturating",
        SINGLE,
        integer(IntegerCarrier::I16),
        ToIntegerSaturating,
    ),
    row(
        "to_i32_saturating",
        SINGLE,
        integer(IntegerCarrier::I32),
        ToIntegerSaturating,
    ),
    row(
        "to_i64_saturating",
        SINGLE,
        integer(IntegerCarrier::I64),
        ToIntegerSaturating,
    ),
    row(
        "to_u8_saturating",
        SINGLE,
        integer(IntegerCarrier::U8),
        ToIntegerSaturating,
    ),
    row(
        "to_u16_saturating",
        SINGLE,
        integer(IntegerCarrier::U16),
        ToIntegerSaturating,
    ),
    row(
        "to_u32_saturating",
        SINGLE,
        integer(IntegerCarrier::U32),
        ToIntegerSaturating,
    ),
    row(
        "to_u64_saturating",
        SINGLE,
        integer(IntegerCarrier::U64),
        ToIntegerSaturating,
    ),
    row(
        "multiply_then_add",
        TERNARY,
        Meaning,
        TernaryMeaning(FloatSemantics::multiply_then_add),
    ),
    row(
        "fused_multiply_add",
        TERNARY,
        Meaning,
        TernaryMeaning(FloatSemantics::fused_multiply_add),
    ),
    row(
        "minimum",
        PAIR,
        Meaning,
        PairMeaning(FloatSemantics::minimum),
    ),
    row(
        "maximum",
        PAIR,
        Meaning,
        PairMeaning(FloatSemantics::maximum),
    ),
    row("equal", PAIR, Bool, PairBool(FloatSemantics::equal)),
    row("not_equal", PAIR, Bool, PairBool(FloatSemantics::not_equal)),
    row("less", PAIR, Bool, PairBool(FloatSemantics::less)),
    row(
        "less_or_equal",
        PAIR,
        Bool,
        PairBool(FloatSemantics::less_or_equal),
    ),
    row("greater", PAIR, Bool, PairBool(FloatSemantics::greater)),
    row(
        "greater_or_equal",
        PAIR,
        Bool,
        PairBool(FloatSemantics::greater_or_equal),
    ),
    row(
        "classify",
        UNARY,
        Class,
        UnaryClass(FloatSemantics::classify),
    ),
    row(
        "is_finite",
        SINGLE,
        Bool,
        SingleBool(FloatSemantics::is_finite),
    ),
    row("is_nan", SINGLE, Bool, SingleBool(FloatSemantics::is_nan)),
    row(
        "is_infinite",
        SINGLE,
        Bool,
        SingleBool(FloatSemantics::is_infinite),
    ),
    row(
        "is_normal",
        UNARY,
        Bool,
        UnaryBool(FloatSemantics::is_normal),
    ),
    row(
        "is_subnormal",
        UNARY,
        Bool,
        UnaryBool(FloatSemantics::is_subnormal),
    ),
    row(
        "add_toward_zero",
        BINARY,
        Meaning,
        BinaryMeaning(FloatSemantics::add_toward_zero),
    ),
    row(
        "add_toward_positive",
        BINARY,
        Meaning,
        BinaryMeaning(FloatSemantics::add_toward_positive),
    ),
    row(
        "add_toward_negative",
        BINARY,
        Meaning,
        BinaryMeaning(FloatSemantics::add_toward_negative),
    ),
    row(
        "subtract_toward_zero",
        BINARY,
        Meaning,
        BinaryMeaning(FloatSemantics::subtract_toward_zero),
    ),
    row(
        "subtract_toward_positive",
        BINARY,
        Meaning,
        BinaryMeaning(FloatSemantics::subtract_toward_positive),
    ),
    row(
        "subtract_toward_negative",
        BINARY,
        Meaning,
        BinaryMeaning(FloatSemantics::subtract_toward_negative),
    ),
    row(
        "multiply_toward_zero",
        BINARY,
        Meaning,
        BinaryMeaning(FloatSemantics::multiply_toward_zero),
    ),
    row(
        "multiply_toward_positive",
        BINARY,
        Meaning,
        BinaryMeaning(FloatSemantics::multiply_toward_positive),
    ),
    row(
        "multiply_toward_negative",
        BINARY,
        Meaning,
        BinaryMeaning(FloatSemantics::multiply_toward_negative),
    ),
    row(
        "divide_toward_zero",
        BINARY,
        Meaning,
        BinaryMeaning(FloatSemantics::divide_toward_zero),
    ),
    row(
        "divide_toward_positive",
        BINARY,
        Meaning,
        BinaryMeaning(FloatSemantics::divide_toward_positive),
    ),
    row(
        "divide_toward_negative",
        BINARY,
        Meaning,
        BinaryMeaning(FloatSemantics::divide_toward_negative),
    ),
    row(
        "square_root_toward_zero",
        UNARY,
        Meaning,
        UnaryMeaning(FloatSemantics::square_root_toward_zero),
    ),
    row(
        "square_root_toward_positive",
        UNARY,
        Meaning,
        UnaryMeaning(FloatSemantics::square_root_toward_positive),
    ),
    row(
        "square_root_toward_negative",
        UNARY,
        Meaning,
        UnaryMeaning(FloatSemantics::square_root_toward_negative),
    ),
    row(
        "fused_multiply_add_toward_zero",
        TERNARY,
        Meaning,
        TernaryMeaning(FloatSemantics::fused_multiply_add_toward_zero),
    ),
    row(
        "fused_multiply_add_toward_positive",
        TERNARY,
        Meaning,
        TernaryMeaning(FloatSemantics::fused_multiply_add_toward_positive),
    ),
    row(
        "fused_multiply_add_toward_negative",
        TERNARY,
        Meaning,
        TernaryMeaning(FloatSemantics::fused_multiply_add_toward_negative),
    ),
];

impl FloatSemanticOperation {
    /// Whether the namespace names this catalog at all. A row is never
    /// selected by leaf spelling alone: see [`Self::from_source_identity`].
    pub fn namespace_matches(namespace: &str) -> bool {
        namespace == FLOAT_SEMANTICS_NAMESPACE
    }

    /// Select the one row whose path and complete normalized signature match.
    /// Overloads sharing a leaf (`from_integer`) differ by carrier, so the
    /// signature is part of the key.
    pub fn from_source_identity(
        namespace: &str,
        name: &str,
        parameters: &[FloatSemanticValueKind],
        result: FloatSemanticValueKind,
    ) -> Option<&'static FloatSemanticOperation> {
        if !Self::namespace_matches(namespace) {
            return None;
        }
        FLOAT_SEMANTIC_OPERATIONS
            .iter()
            .find(|row| row.name == name && row.parameters == parameters && row.result == result)
    }

    /// Whether any row carries this leaf name; used only to word a rejection
    /// for a signature that names the catalog but matches no exact row.
    pub fn names_a_row(namespace: &str, name: &str) -> bool {
        Self::namespace_matches(namespace)
            && FLOAT_SEMANTIC_OPERATIONS.iter().any(|row| row.name == name)
    }

    /// The kernel-discharge binding stored beside this row.
    pub const fn kernel(&self) -> FloatSemanticKernel {
        self.kernel
    }

    /// Evaluate one application of this row through its bound kernel.
    ///
    /// The operand list must match the row's complete signature position by
    /// position: `Format` operands name one of the two sealed IEEE binary
    /// format records, and `Integer` operands fit the declared carrier's
    /// bounds. `None` reports a malformed application or an operation the
    /// exact semantics leaves undefined on these operands — for example an
    /// exact `to_*` conversion of NaN, infinity, or an out-of-carrier value —
    /// and discharges no obligation. A `Some` pairs the kernel's result with
    /// the row's [`FloatSemanticContractIdentity`], so a discharged
    /// application cites the artifact-relative identity the rooted checker
    /// emitted for the declaration. This is the binding a provider consumes
    /// through `semantic_operations::exact_toolchain_float_semantic_contract`;
    /// comparing a `Meaning` result with `==` is the payload-erased meaning
    /// equality a `FloatMeaningEqual` proposition names.
    pub fn kernel_discharge(
        &self,
        arguments: &[FloatSemanticOperand],
    ) -> Option<FloatSemanticDischarge> {
        if arguments.len() != self.parameters.len()
            || !arguments
                .iter()
                .zip(self.parameters.iter())
                .all(|(operand, kind)| operand.matches_kind(*kind))
        {
            return None;
        }
        let result = self.kernel.evaluate(self.result, arguments)?;
        if !result.matches_kind(self.result) {
            return None;
        }
        Some(FloatSemanticDischarge {
            contract: self.contract_identity(),
            result,
        })
    }

    /// The reverse lookup an artifact-verifier performs: the identity carried
    /// beside a discharged application selects the one row whose ordinal,
    /// catalog version, and recomputed commitment all match. An identity that
    /// names no live row, or a row whose recomputed commitment drifted,
    /// rejoins to nothing.
    pub fn for_contract_identity(
        identity: &FloatSemanticContractIdentity,
    ) -> Option<&'static FloatSemanticOperation> {
        if identity.catalog_version != FLOAT_SEMANTICS_CATALOG_VERSION {
            return None;
        }
        let row = FLOAT_SEMANTIC_OPERATIONS.get(usize::from(identity.row))?;
        if row.contract_identity() == *identity {
            Some(row)
        } else {
            None
        }
    }

    /// The row's stable ordinal in the catalog order.
    pub fn ordinal(&self) -> u8 {
        FLOAT_SEMANTIC_OPERATIONS
            .iter()
            .position(|row| row == self)
            .and_then(|position| u8::try_from(position).ok())
            .expect("every row is in the catalog")
    }

    pub fn contract_identity(&self) -> FloatSemanticContractIdentity {
        let mut hasher = Sha256::new();
        hasher.update(FLOAT_SEMANTICS_CONTRACT_DOMAIN);
        hasher.update(FLOAT_SEMANTICS_CATALOG_VERSION.to_le_bytes());
        hasher
            .update(format!("toolchain::{}::{}", FLOAT_SEMANTICS_NAMESPACE, self.name).as_bytes());
        hasher.update([0]);
        for parameter in self.parameters {
            hasher.update(parameter.spelling().as_bytes());
            hasher.update([0]);
        }
        hasher.update(b"->");
        hasher.update(self.result.spelling().as_bytes());
        FloatSemanticContractIdentity {
            row: self.ordinal(),
            catalog_version: FLOAT_SEMANTICS_CATALOG_VERSION,
            commitment: hasher.finalize().into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FLOAT_SEMANTIC_OPERATIONS, FloatSemanticOperand, FloatSemanticOperation,
        FloatSemanticResult, FloatSemanticValueKind, IntegerCarrier,
    };
    use crate::bignum::BigInt;
    use crate::float_semantics::{FloatFormat, FloatMeaning, FloatSemantics};
    use std::collections::HashSet;

    #[test]
    fn rows_are_distinct_by_complete_signature_and_overloads_do_not_collide() {
        assert_eq!(FLOAT_SEMANTIC_OPERATIONS.len(), 65);
        let keys = FLOAT_SEMANTIC_OPERATIONS
            .iter()
            .map(|row| (row.name, row.parameters, row.result))
            .collect::<HashSet<_>>();
        assert_eq!(keys.len(), FLOAT_SEMANTIC_OPERATIONS.len());
        let commitments = FLOAT_SEMANTIC_OPERATIONS
            .iter()
            .map(|row| row.contract_identity().commitment)
            .collect::<HashSet<_>>();
        assert_eq!(commitments.len(), FLOAT_SEMANTIC_OPERATIONS.len());
        let i8_row = FloatSemanticOperation::from_source_identity(
            "FloatSemantics",
            "from_integer",
            &[
                FloatSemanticValueKind::Format,
                FloatSemanticValueKind::Integer(IntegerCarrier::I8),
            ],
            FloatSemanticValueKind::Meaning,
        )
        .expect("i8 overload");
        let u64_row = FloatSemanticOperation::from_source_identity(
            "FloatSemantics",
            "from_integer",
            &[
                FloatSemanticValueKind::Format,
                FloatSemanticValueKind::Integer(IntegerCarrier::U64),
            ],
            FloatSemanticValueKind::Meaning,
        )
        .expect("u64 overload");
        assert_ne!(i8_row.ordinal(), u64_row.ordinal());
        assert_ne!(i8_row.contract_identity(), u64_row.contract_identity());
    }

    #[test]
    fn leaf_spelling_alone_selects_nothing() {
        assert!(
            FloatSemanticOperation::from_source_identity(
                "Other",
                "add",
                &[
                    FloatSemanticValueKind::Format,
                    FloatSemanticValueKind::Meaning,
                    FloatSemanticValueKind::Meaning
                ],
                FloatSemanticValueKind::Meaning,
            )
            .is_none()
        );
        assert!(
            FloatSemanticOperation::from_source_identity(
                "FloatSemantics",
                "add",
                &[
                    FloatSemanticValueKind::Meaning,
                    FloatSemanticValueKind::Meaning
                ],
                FloatSemanticValueKind::Meaning,
            )
            .is_none(),
            "a drifted signature matches no row"
        );
        assert!(FloatSemanticOperation::names_a_row("FloatSemantics", "add"));
        assert!(!FloatSemanticOperation::names_a_row(
            "FloatSemantics",
            "hypot"
        ));
    }

    fn signature_operands(row: &FloatSemanticOperation) -> Vec<FloatSemanticOperand> {
        row.parameters
            .iter()
            .map(|kind| match kind {
                FloatSemanticValueKind::Format => {
                    FloatSemanticOperand::Format(FloatFormat::BINARY32)
                }
                FloatSemanticValueKind::Meaning => {
                    FloatSemanticOperand::Meaning(FloatMeaning::from_f32(1.0))
                }
                // No row takes FloatClass or Bool as a parameter.
                FloatSemanticValueKind::Integer(_) => {
                    FloatSemanticOperand::Integer(BigInt::from_u64(1))
                }
                other => unreachable!("{other:?} is not a declared parameter kind"),
            })
            .collect()
    }

    fn row_named(
        name: &str,
        parameters: &[FloatSemanticValueKind],
        result: FloatSemanticValueKind,
    ) -> &'static FloatSemanticOperation {
        FloatSemanticOperation::from_source_identity("FloatSemantics", name, parameters, result)
            .expect("catalog row")
    }

    #[test]
    fn contract_identities_rejoin_to_their_own_rows_only() {
        for row in FLOAT_SEMANTIC_OPERATIONS {
            let identity = row.contract_identity();
            let rejoined =
                FloatSemanticOperation::for_contract_identity(&identity).expect("rejoin");
            assert_eq!(
                rejoined.ordinal(),
                row.ordinal(),
                "{} rejoins to the declared row ordinal",
                row.name
            );
            assert_eq!(rejoined.contract_identity(), identity);
            let mut drifted = identity;
            drifted.commitment[0] ^= 1;
            assert!(FloatSemanticOperation::for_contract_identity(&drifted).is_none());
            let mut stale_version = identity;
            stale_version.catalog_version += 1;
            assert!(FloatSemanticOperation::for_contract_identity(&stale_version).is_none());
        }
        let mut out_of_range = FLOAT_SEMANTIC_OPERATIONS[0].contract_identity();
        out_of_range.row = u8::MAX;
        assert!(FloatSemanticOperation::for_contract_identity(&out_of_range).is_none());
    }

    #[test]
    fn every_row_discharges_a_signature_shaped_application() {
        for row in FLOAT_SEMANTIC_OPERATIONS {
            let arguments = signature_operands(row);
            let discharge = row
                .kernel_discharge(&arguments)
                .unwrap_or_else(|| panic!("{} failed to discharge", row.name));
            assert_eq!(discharge.contract, row.contract_identity());
            assert!(discharge.result.matches_kind(row.result));
        }
    }

    #[test]
    fn discharged_results_match_the_kernel_directly() {
        let left = FloatMeaning::from_f32(0.1);
        let right = FloatMeaning::from_f32(0.2);
        let add = row_named(
            "add",
            &[
                FloatSemanticValueKind::Format,
                FloatSemanticValueKind::Meaning,
                FloatSemanticValueKind::Meaning,
            ],
            FloatSemanticValueKind::Meaning,
        );
        let discharge = add
            .kernel_discharge(&[
                FloatSemanticOperand::Format(FloatFormat::BINARY32),
                FloatSemanticOperand::Meaning(left.clone()),
                FloatSemanticOperand::Meaning(right.clone()),
            ])
            .expect("add discharges");
        assert_eq!(
            discharge.result,
            FloatSemanticResult::Meaning(FloatSemantics::add(FloatFormat::BINARY32, &left, &right))
        );
        // The same operand list cannot discharge another row: signature is
        // part of the identity, so `equal` (a PAIR shape) is not reached.
        let equal = row_named(
            "equal",
            &[
                FloatSemanticValueKind::Meaning,
                FloatSemanticValueKind::Meaning,
            ],
            FloatSemanticValueKind::Bool,
        );
        assert_eq!(
            equal
                .kernel_discharge(&[
                    FloatSemanticOperand::Meaning(left.clone()),
                    FloatSemanticOperand::Meaning(right.clone()),
                ])
                .expect("equal discharges")
                .result,
            FloatSemanticResult::Bool(FloatSemantics::equal(&left, &right))
        );
    }

    #[test]
    fn discharge_keeps_meaning_equality_distinct_from_ieee_comparison() {
        let add = row_named(
            "add",
            &[
                FloatSemanticValueKind::Format,
                FloatSemanticValueKind::Meaning,
                FloatSemanticValueKind::Meaning,
            ],
            FloatSemanticValueKind::Meaning,
        );
        let discharged_nan = add
            .kernel_discharge(&[
                FloatSemanticOperand::Format(FloatFormat::BINARY32),
                FloatSemanticOperand::Meaning(FloatMeaning::NaN),
                FloatSemanticOperand::Meaning(FloatMeaning::from_f32(1.0)),
            ])
            .expect("add(NaN, 1) discharges")
            .result;
        // Meaning equality is the payload-erased sum: NaN is reflexive there.
        assert_eq!(
            discharged_nan,
            FloatSemanticResult::Meaning(FloatMeaning::NaN)
        );
        let equal = row_named(
            "equal",
            &[
                FloatSemanticValueKind::Meaning,
                FloatSemanticValueKind::Meaning,
            ],
            FloatSemanticValueKind::Bool,
        );
        // The IEEE comparison kernel keeps NaN unordered.
        assert_eq!(
            equal
                .kernel_discharge(&[
                    FloatSemanticOperand::Meaning(FloatMeaning::NaN),
                    FloatSemanticOperand::Meaning(FloatMeaning::NaN),
                ])
                .expect("equal(NaN, NaN) discharges")
                .result,
            FloatSemanticResult::Bool(false)
        );
    }

    #[test]
    fn discharge_preserves_the_signed_zero_distinction() {
        let add = row_named(
            "add",
            &[
                FloatSemanticValueKind::Format,
                FloatSemanticValueKind::Meaning,
                FloatSemanticValueKind::Meaning,
            ],
            FloatSemanticValueKind::Meaning,
        );
        let discharged = add
            .kernel_discharge(&[
                FloatSemanticOperand::Format(FloatFormat::BINARY32),
                FloatSemanticOperand::Meaning(FloatMeaning::from_f32(0.0)),
                FloatSemanticOperand::Meaning(FloatMeaning::from_f32(-0.0)),
            ])
            .expect("add(+0, -0) discharges")
            .result;
        // IEEE +0 + -0 lands on +0, and meaning equality keeps the sign.
        assert_eq!(
            discharged,
            FloatSemanticResult::Meaning(FloatMeaning::Zero { negative: false })
        );
        assert_ne!(
            discharged,
            FloatSemanticResult::Meaning(FloatMeaning::Zero { negative: true })
        );
        // The IEEE comparison kernel still equates the two zeros.
        let equal = row_named(
            "equal",
            &[
                FloatSemanticValueKind::Meaning,
                FloatSemanticValueKind::Meaning,
            ],
            FloatSemanticValueKind::Bool,
        );
        assert_eq!(
            equal
                .kernel_discharge(&[
                    FloatSemanticOperand::Meaning(FloatMeaning::from_f32(0.0)),
                    FloatSemanticOperand::Meaning(FloatMeaning::from_f32(-0.0)),
                ])
                .expect("equal(+0, -0) discharges")
                .result,
            FloatSemanticResult::Bool(true)
        );
    }

    #[test]
    fn discharge_enforces_declared_carriers_and_conversion_domains() {
        let from_i8 = row_named(
            "from_integer",
            &[
                FloatSemanticValueKind::Format,
                FloatSemanticValueKind::Integer(IntegerCarrier::I8),
            ],
            FloatSemanticValueKind::Meaning,
        );
        // An i8 operand cannot carry 300 or -200; the application is malformed.
        for value in [300_u64, 200_u64] {
            let literal = if value == 200_u64 {
                BigInt::from_u64(value).negate()
            } else {
                BigInt::from_u64(value)
            };
            assert!(
                from_i8
                    .kernel_discharge(&[
                        FloatSemanticOperand::Format(FloatFormat::BINARY32),
                        FloatSemanticOperand::Integer(literal),
                    ])
                    .is_none(),
                "out-of-carrier integer operand must not discharge"
            );
        }
        let discharged = from_i8
            .kernel_discharge(&[
                FloatSemanticOperand::Format(FloatFormat::BINARY32),
                FloatSemanticOperand::Integer(BigInt::from_u64(100)),
            ])
            .expect("in-carrier from_integer discharges")
            .result;
        assert_eq!(
            discharged,
            FloatSemanticResult::Meaning(FloatMeaning::from_f32(100.0))
        );

        let to_i8 = row_named(
            "to_i8",
            &[FloatSemanticValueKind::Meaning],
            FloatSemanticValueKind::Integer(IntegerCarrier::I8),
        );
        // Exact conversion of a value outside the carrier is undefined; a
        // saturating row discharges instead.
        assert!(
            to_i8
                .kernel_discharge(&[FloatSemanticOperand::Meaning(FloatMeaning::from_f32(300.0))])
                .is_none()
        );
        let to_i8_saturating = row_named(
            "to_i8_saturating",
            &[FloatSemanticValueKind::Meaning],
            FloatSemanticValueKind::Integer(IntegerCarrier::I8),
        );
        assert_eq!(
            to_i8_saturating
                .kernel_discharge(&[FloatSemanticOperand::Meaning(FloatMeaning::from_f32(300.0))])
                .expect("saturating conversion discharges")
                .result,
            FloatSemanticResult::Integer(BigInt::from_u64(127))
        );
    }

    #[test]
    fn malformed_applications_do_not_discharge() {
        let add = row_named(
            "add",
            &[
                FloatSemanticValueKind::Format,
                FloatSemanticValueKind::Meaning,
                FloatSemanticValueKind::Meaning,
            ],
            FloatSemanticValueKind::Meaning,
        );
        let one = FloatMeaning::from_f32(1.0);
        // Wrong arity and wrong operand kinds both reject.
        assert!(
            add.kernel_discharge(&[
                FloatSemanticOperand::Format(FloatFormat::BINARY32),
                FloatSemanticOperand::Meaning(one.clone()),
            ])
            .is_none()
        );
        assert!(
            add.kernel_discharge(&[
                FloatSemanticOperand::Meaning(one.clone()),
                FloatSemanticOperand::Meaning(one.clone()),
                FloatSemanticOperand::Meaning(one),
            ])
            .is_none(),
            "a Meaning cannot stand at the declared Format position"
        );
        // A format record outside the two sealed IEEE binaries discharges
        // nothing: the kernel has no meaning for it.
        let other_format = FloatFormat {
            precision: 8,
            ..FloatFormat::BINARY32
        };
        assert!(
            add.kernel_discharge(&[
                FloatSemanticOperand::Format(other_format),
                FloatSemanticOperand::Meaning(FloatMeaning::from_f32(1.0)),
                FloatSemanticOperand::Meaning(FloatMeaning::from_f32(2.0)),
            ])
            .is_none()
        );
    }
}
