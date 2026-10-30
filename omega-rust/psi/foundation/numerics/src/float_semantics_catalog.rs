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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
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
    PairBool(FloatSemanticComparison),
    /// `(FloatFormat, FloatMeaning) -> FloatClass`.
    UnaryClass(fn(FloatFormat, &FloatMeaning) -> FloatClass),
    /// Total classification predicates have explicit semantic identity so
    /// checking can derive interval consequences without inspecting a name.
    SinglePredicate(FloatSemanticPredicate),
    /// `(FloatFormat, FloatMeaning) -> bool`.
    UnaryBool(fn(FloatFormat, &FloatMeaning) -> bool),
}

/// IEEE comparison laws owned by the exact selected catalog row.
/// Reflexive value meaning does not make NaN compare equal to itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatSemanticComparison {
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

impl FloatSemanticComparison {
    fn evaluate(self, left: &FloatMeaning, right: &FloatMeaning) -> bool {
        match self {
            Self::Equal => FloatSemantics::equal(left, right),
            Self::NotEqual => FloatSemantics::not_equal(left, right),
            Self::Less => FloatSemantics::less(left, right),
            Self::LessOrEqual => FloatSemantics::less_or_equal(left, right),
            Self::Greater => FloatSemantics::greater(left, right),
            Self::GreaterOrEqual => FloatSemantics::greater_or_equal(left, right),
        }
    }

    /// NaN on either side determines every IEEE comparison independently of
    /// the other operand. An unknown classification supplies no such fact.
    pub const fn unordered_truth(self) -> bool {
        matches!(self, Self::NotEqual)
    }

    /// Complete equal meanings share their NaN classification. Strict order
    /// is false even when that classification is unknown; other comparisons
    /// need it because NaN remains unordered.
    pub const fn same_meaning_truth(self, nan: Option<bool>) -> Option<bool> {
        match self {
            Self::Less | Self::Greater => Some(false),
            _ => match nan {
                Some(true) => Some(self.unordered_truth()),
                Some(false) => Some(!matches!(self, Self::NotEqual)),
                None => None,
            },
        }
    }
}

/// The semantic classification law owned by a selected catalog predicate.
/// An ordered enclosure excludes NaN; endpoint truth alone does not justify
/// arbitrary predicates over its interior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatSemanticPredicate {
    Finite,
    NaN,
    Infinite,
}

impl FloatSemanticPredicate {
    fn evaluate(self, value: &FloatMeaning) -> bool {
        match self {
            Self::Finite => FloatSemantics::is_finite(value),
            Self::NaN => FloatSemantics::is_nan(value),
            Self::Infinite => FloatSemantics::is_infinite(value),
        }
    }

    fn ordered_interval(self, minimum: &FloatMeaning, maximum: &FloatMeaning) -> Option<bool> {
        if !FloatSemantics::less_or_equal(minimum, maximum) {
            return None;
        }
        match self {
            Self::NaN => Some(false),
            Self::Finite | Self::Infinite if minimum.is_finite() && maximum.is_finite() => {
                Some(self == Self::Finite)
            }
            Self::Finite | Self::Infinite if minimum == maximum => Some(self.evaluate(minimum)),
            _ => None,
        }
    }
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
                Some(FloatSemanticResult::Bool(
                    kernel.evaluate(left.as_meaning()?, right.as_meaning()?),
                ))
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
            Self::SinglePredicate(predicate) => {
                let [value] = arguments else {
                    return None;
                };
                Some(FloatSemanticResult::Bool(
                    predicate.evaluate(value.as_meaning()?),
                ))
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
    BinaryMeaning, FromInteger, PairBool, PairMeaning, SinglePredicate, TernaryMeaning,
    ToIntegerExact, ToIntegerSaturating, UnaryBool, UnaryClass, UnaryMeaning,
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

macro_rules! binary {
    ($name:ident) => {
        row(
            stringify!($name),
            BINARY,
            Meaning,
            BinaryMeaning(FloatSemantics::$name),
        )
    };
}
macro_rules! unary {
    ($name:ident) => {
        row(
            stringify!($name),
            UNARY,
            Meaning,
            UnaryMeaning(FloatSemantics::$name),
        )
    };
}
macro_rules! ternary {
    ($name:ident) => {
        row(
            stringify!($name),
            TERNARY,
            Meaning,
            TernaryMeaning(FloatSemantics::$name),
        )
    };
}
macro_rules! pair {
    ($name:ident) => {
        row(
            stringify!($name),
            PAIR,
            Meaning,
            PairMeaning(FloatSemantics::$name),
        )
    };
}
macro_rules! unary_bool {
    ($name:ident) => {
        row(
            stringify!($name),
            UNARY,
            Bool,
            UnaryBool(FloatSemantics::$name),
        )
    };
}
macro_rules! from_integer {
    ($carrier:ident) => {
        row(
            "from_integer",
            &[Format, integer(IntegerCarrier::$carrier)],
            Meaning,
            FromInteger(FloatSemantics::from_integer),
        )
    };
}
macro_rules! exact {
    ($name:literal, $carrier:ident) => {
        row(
            $name,
            SINGLE,
            integer(IntegerCarrier::$carrier),
            ToIntegerExact,
        )
    };
}
macro_rules! saturating {
    ($name:literal, $carrier:ident) => {
        row(
            $name,
            SINGLE,
            integer(IntegerCarrier::$carrier),
            ToIntegerSaturating,
        )
    };
}
macro_rules! compare {
    ($name:literal, $relation:ident) => {
        row(
            $name,
            PAIR,
            Bool,
            PairBool(FloatSemanticComparison::$relation),
        )
    };
}
macro_rules! predicate {
    ($name:literal, $kind:ident) => {
        row(
            $name,
            SINGLE,
            Bool,
            SinglePredicate(FloatSemanticPredicate::$kind),
        )
    };
}

/// Every row, in the order the sealed source declares them. The position is
/// the row's stable ordinal in [`FloatSemanticContractIdentity`]. The third
/// argument of each `row` literal is the kernel-discharge binding: the exact
/// `FloatSemantics` definition an application of that row evaluates through.
pub const FLOAT_SEMANTIC_OPERATIONS: &[FloatSemanticOperation] = &[
    binary!(add),
    binary!(subtract),
    binary!(multiply),
    binary!(divide),
    unary!(negate),
    unary!(square_root),
    unary!(convert),
    from_integer!(I8),
    from_integer!(I16),
    from_integer!(I32),
    from_integer!(I64),
    from_integer!(U8),
    from_integer!(U16),
    from_integer!(U32),
    from_integer!(U64),
    exact!("to_i8", I8),
    exact!("to_i16", I16),
    exact!("to_i32", I32),
    exact!("to_i64", I64),
    exact!("to_u8", U8),
    exact!("to_u16", U16),
    exact!("to_u32", U32),
    exact!("to_u64", U64),
    saturating!("to_i8_saturating", I8),
    saturating!("to_i16_saturating", I16),
    saturating!("to_i32_saturating", I32),
    saturating!("to_i64_saturating", I64),
    saturating!("to_u8_saturating", U8),
    saturating!("to_u16_saturating", U16),
    saturating!("to_u32_saturating", U32),
    saturating!("to_u64_saturating", U64),
    ternary!(multiply_then_add),
    ternary!(fused_multiply_add),
    pair!(minimum),
    pair!(maximum),
    compare!("equal", Equal),
    compare!("not_equal", NotEqual),
    compare!("less", Less),
    compare!("less_or_equal", LessOrEqual),
    compare!("greater", Greater),
    compare!("greater_or_equal", GreaterOrEqual),
    row(
        "classify",
        UNARY,
        Class,
        UnaryClass(FloatSemantics::classify),
    ),
    predicate!("is_finite", Finite),
    predicate!("is_nan", NaN),
    predicate!("is_infinite", Infinite),
    unary_bool!(is_normal),
    unary_bool!(is_subnormal),
    binary!(add_toward_zero),
    binary!(add_toward_positive),
    binary!(add_toward_negative),
    binary!(subtract_toward_zero),
    binary!(subtract_toward_positive),
    binary!(subtract_toward_negative),
    binary!(multiply_toward_zero),
    binary!(multiply_toward_positive),
    binary!(multiply_toward_negative),
    binary!(divide_toward_zero),
    binary!(divide_toward_positive),
    binary!(divide_toward_negative),
    unary!(square_root_toward_zero),
    unary!(square_root_toward_positive),
    unary!(square_root_toward_negative),
    ternary!(fused_multiply_add_toward_zero),
    ternary!(fused_multiply_add_toward_positive),
    ternary!(fused_multiply_add_toward_negative),
];

/// The leaf names the builtin call dispatch spells by name alone — the
/// rows a plain leaf selects rather than a signature-matched operator
/// path. Grouped by call arity; a row's leading `Format` parameter is
/// the format selection, not a call operand.
pub const FLOAT_NAMED_CALL_LEAVES: &[&str] = &[
    "negate",
    "square_root",
    "classify",
    "is_finite",
    "is_nan",
    "is_infinite",
    "is_normal",
    "is_subnormal",
    "square_root_toward_zero",
    "square_root_toward_positive",
    "square_root_toward_negative",
    "minimum",
    "maximum",
    "add_toward_zero",
    "add_toward_positive",
    "add_toward_negative",
    "subtract_toward_zero",
    "subtract_toward_positive",
    "subtract_toward_negative",
    "multiply_toward_zero",
    "multiply_toward_positive",
    "multiply_toward_negative",
    "divide_toward_zero",
    "divide_toward_positive",
    "divide_toward_negative",
    "multiply_then_add",
    "fused_multiply_add",
    "fused_multiply_add_toward_zero",
    "fused_multiply_add_toward_positive",
    "fused_multiply_add_toward_negative",
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

    /// The call-operand count for a leaf the builtin call dispatch spells
    /// by name alone, or `None` when `name` is not one of
    /// [`FLOAT_NAMED_CALL_LEAVES`]. The count comes from the row's own
    /// signature: a leading `Format` parameter is the row's format
    /// selection, not a call operand.
    pub fn named_call_arity(name: &str) -> Option<usize> {
        if !FLOAT_NAMED_CALL_LEAVES.contains(&name) {
            return None;
        }
        FLOAT_SEMANTIC_OPERATIONS
            .iter()
            .find(|row| row.name == name)
            .map(|row| {
                row.parameters.len()
                    - usize::from(row.parameters.first() == Some(&FloatSemanticValueKind::Format))
            })
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

    /// The image of an admitted Integer enclosure under this exact selected
    /// conversion. The catalog's FromInteger kernel is monotone; discharge
    /// each endpoint through that same kernel and retain its rounded meaning.
    /// Fixed source carriers and sealed target formats remain independent.
    pub fn integer_operand_image(
        &self,
        format: FloatFormat,
        carrier: IntegerCarrier,
        minimum: &BigInt,
        maximum: &BigInt,
    ) -> Option<(FloatMeaning, FloatMeaning)> {
        if !matches!(self.kernel, FloatSemanticKernel::FromInteger(_))
            || self.result != FloatSemanticValueKind::Meaning
            || self.parameters
                != [
                    FloatSemanticValueKind::Format,
                    FloatSemanticValueKind::Integer(carrier),
                ]
            || minimum > maximum
        {
            return None;
        }
        let endpoint = |value: &BigInt| {
            let discharge = self.kernel_discharge(&[
                FloatSemanticOperand::Format(format),
                FloatSemanticOperand::Integer(value.clone()),
            ])?;
            let FloatSemanticResult::Meaning(meaning) = discharge.result else {
                return None;
            };
            Some(meaning)
        };
        Some((endpoint(minimum)?, endpoint(maximum)?))
    }

    /// A classification predicate whose truth holds throughout an ordered
    /// meaning enclosure. This uses the selected kernel's classification law,
    /// never a named call or arbitrary endpoint agreement.
    pub fn ordered_interval_predicate(
        &self,
        minimum: &FloatMeaning,
        maximum: &FloatMeaning,
    ) -> Option<bool> {
        if self.parameters != [FloatSemanticValueKind::Meaning]
            || self.result != FloatSemanticValueKind::Bool
        {
            return None;
        }
        let FloatSemanticKernel::SinglePredicate(predicate) = self.kernel else {
            return None;
        };
        predicate.ordered_interval(minimum, maximum)
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
        let mut hasher =
            crate::digest_encoder::DomainDigestEncoder::new(FLOAT_SEMANTICS_CONTRACT_DOMAIN);
        hasher.raw(&FLOAT_SEMANTICS_CATALOG_VERSION.to_le_bytes());
        hasher.raw(format!("toolchain::{}::{}", FLOAT_SEMANTICS_NAMESPACE, self.name).as_bytes());
        hasher.byte(0);
        for parameter in self.parameters {
            hasher.raw(parameter.spelling().as_bytes());
            hasher.byte(0);
        }
        hasher.raw(b"->");
        hasher.raw(self.result.spelling().as_bytes());
        FloatSemanticContractIdentity {
            row: self.ordinal(),
            catalog_version: FLOAT_SEMANTICS_CATALOG_VERSION,
            commitment: hasher.finish(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FLOAT_SEMANTIC_OPERATIONS, FloatSemanticComparison, FloatSemanticOperand,
        FloatSemanticOperation, FloatSemanticResult, FloatSemanticValueKind, IntegerCarrier,
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
    fn completed_comparison_laws_agree_with_ieee_kernels() {
        let comparisons = [
            FloatSemanticComparison::Equal,
            FloatSemanticComparison::NotEqual,
            FloatSemanticComparison::Less,
            FloatSemanticComparison::LessOrEqual,
            FloatSemanticComparison::Greater,
            FloatSemanticComparison::GreaterOrEqual,
        ];
        let values = [
            FloatMeaning::from_f32(0.0),
            FloatMeaning::from_f32(-0.0),
            FloatMeaning::from_f32(1.0),
            FloatMeaning::from_f32(-1.0),
            FloatMeaning::from_f32(f32::MIN_POSITIVE),
            FloatMeaning::from_f32(f32::MAX),
            FloatMeaning::from_f32(f32::INFINITY),
            FloatMeaning::from_f32(f32::NEG_INFINITY),
            FloatMeaning::NaN,
        ];
        for comparison in comparisons {
            for value in &values {
                assert_eq!(
                    comparison.same_meaning_truth(Some(FloatSemantics::is_nan(value))),
                    Some(comparison.evaluate(value, value))
                );
                assert_eq!(
                    comparison.unordered_truth(),
                    comparison.evaluate(value, &FloatMeaning::NaN)
                );
                assert_eq!(
                    comparison.unordered_truth(),
                    comparison.evaluate(&FloatMeaning::NaN, value)
                );
            }
        }
        assert_eq!(
            FloatSemanticComparison::Equal.same_meaning_truth(None),
            None
        );
        assert_eq!(
            FloatSemanticComparison::NotEqual.same_meaning_truth(None),
            None
        );
        assert_eq!(
            FloatSemanticComparison::Less.same_meaning_truth(None),
            Some(false)
        );
        assert_eq!(
            FloatSemanticComparison::Greater.same_meaning_truth(None),
            Some(false)
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
    fn integer_carrier_images_preserve_rounded_bounds_and_signature() {
        for carrier in [
            IntegerCarrier::I8,
            IntegerCarrier::I16,
            IntegerCarrier::I32,
            IntegerCarrier::I64,
            IntegerCarrier::U8,
            IntegerCarrier::U16,
            IntegerCarrier::U32,
            IntegerCarrier::U64,
        ] {
            let conversion = row_named(
                "from_integer",
                &[
                    FloatSemanticValueKind::Format,
                    FloatSemanticValueKind::Integer(carrier),
                ],
                FloatSemanticValueKind::Meaning,
            );
            let (minimum, maximum) = carrier.format().bounds();
            for format in [FloatFormat::BINARY32, FloatFormat::BINARY64] {
                let (lower, upper) = conversion
                    .integer_operand_image(format, carrier, &minimum, &maximum)
                    .expect("every builtin Integer image is finite");
                assert!(FloatSemantics::is_finite(&lower));
                assert!(FloatSemantics::is_finite(&upper));
                assert!(FloatSemantics::less_or_equal(&lower, &upper));
                // The image retains rounding: u64::MAX rounds up to 2^64.
                if carrier == IntegerCarrier::U64 {
                    assert_eq!(upper, FloatMeaning::from_f64(18446744073709551616.0));
                }
            }
            assert!(
                conversion
                    .integer_operand_image(
                        FloatFormat::BINARY32,
                        carrier,
                        &minimum,
                        &maximum.add(&BigInt::from_u64(1))
                    )
                    .is_none()
            );
            assert!(
                conversion
                    .integer_operand_image(FloatFormat::BINARY32, carrier, &maximum, &minimum)
                    .is_none()
            );
            let other = if carrier == IntegerCarrier::I8 {
                IntegerCarrier::U8
            } else {
                IntegerCarrier::I8
            };
            assert!(
                conversion
                    .integer_operand_image(
                        FloatFormat::BINARY32,
                        other,
                        &BigInt::zero(),
                        &BigInt::from_u64(1)
                    )
                    .is_none()
            );
            assert!(
                conversion
                    .integer_operand_image(
                        FloatFormat {
                            precision: 8,
                            ..FloatFormat::BINARY32
                        },
                        carrier,
                        &minimum,
                        &maximum
                    )
                    .is_none()
            );
        }
    }

    #[test]
    fn ordered_predicate_images_follow_classification_laws() {
        let finite = row_named(
            "is_finite",
            &[FloatSemanticValueKind::Meaning],
            FloatSemanticValueKind::Bool,
        );
        let infinite = row_named(
            "is_infinite",
            &[FloatSemanticValueKind::Meaning],
            FloatSemanticValueKind::Bool,
        );
        let nan = row_named(
            "is_nan",
            &[FloatSemanticValueKind::Meaning],
            FloatSemanticValueKind::Bool,
        );
        let lower = FloatMeaning::from_f64(-1.0);
        let upper = FloatMeaning::from_f64(1.0);
        assert_eq!(
            finite.ordered_interval_predicate(&lower, &upper),
            Some(true)
        );
        assert_eq!(
            infinite.ordered_interval_predicate(&lower, &upper),
            Some(false)
        );
        assert_eq!(nan.ordered_interval_predicate(&lower, &upper), Some(false));
        let negative_infinity = FloatMeaning::from_f64(f64::NEG_INFINITY);
        let positive_infinity = FloatMeaning::from_f64(f64::INFINITY);
        // Equal endpoint predicates do not establish the interior: this span
        // contains finite values as well as both infinities.
        assert_eq!(
            finite.ordered_interval_predicate(&negative_infinity, &positive_infinity),
            None
        );
        assert_eq!(
            infinite.ordered_interval_predicate(&negative_infinity, &positive_infinity),
            None
        );
        assert_eq!(
            infinite.ordered_interval_predicate(&positive_infinity, &positive_infinity),
            Some(true)
        );
        assert_eq!(finite.ordered_interval_predicate(&upper, &lower), None);
        for predicate in [finite, infinite, nan] {
            assert_eq!(
                predicate.ordered_interval_predicate(&FloatMeaning::NaN, &FloatMeaning::NaN),
                None
            );
        }
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
