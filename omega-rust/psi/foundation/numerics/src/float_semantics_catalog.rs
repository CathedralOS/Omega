//! Closed catalog of the representation-free float semantic definitions the
//! sealed toolchain source declares as `FloatSemantics::<name>`.
//!
//! These rows are identity only. A row names one exact declaration by its
//! source-visible path and its complete normalized signature (parameter
//! kinds in order and result kind), so same-named overloads such as the
//! eight `from_integer` carriers are distinct rows and never collide. The
//! catalog does not bind a semantic-engine implementation: a bare bodyless
//! `machine FloatSemantics::add(...)` in the sealed source is admitted by
//! this identity and remains the proof-only definition the `Float::*` slot
//! contracts cite. Attaching kernel discharge or an executable meaning to a
//! row is the extension point for the FloatMeaning provider work: add the
//! binding beside the row, never key it on the leaf spelling.
//!
//! As with the projection catalog, the row alone grants nothing. Ownership is
//! declaration custody -- the Toolchain-origin
//! [`FLOAT_PROJECTION_CORE_SOURCE`](crate::float_projection::FLOAT_PROJECTION_CORE_SOURCE)
//! file -- checked by symbol resolution before a signature lowers to the
//! catalog declaration and by validation on the typed shape afterwards.

use sha2::{Digest, Sha256};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
}

/// The kind of one parameter or result position of a semantic definition.
/// Kinds are the catalog's signature vocabulary; the sealed toolchain types
/// they stand for (`FloatFormat`, `FloatMeaning`, `FloatClass`) are checked
/// by exact toolchain identity in validation, never by this spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// One exact row: the leaf name under [`FLOAT_SEMANTICS_NAMESPACE`] plus the
/// complete normalized signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FloatSemanticOperation {
    pub name: &'static str,
    pub parameters: &'static [FloatSemanticValueKind],
    pub result: FloatSemanticValueKind,
}

use FloatSemanticValueKind::{Bool, Class, Format, Meaning};

const fn integer(carrier: IntegerCarrier) -> FloatSemanticValueKind {
    FloatSemanticValueKind::Integer(carrier)
}

const fn row(
    name: &'static str,
    parameters: &'static [FloatSemanticValueKind],
    result: FloatSemanticValueKind,
) -> FloatSemanticOperation {
    FloatSemanticOperation {
        name,
        parameters,
        result,
    }
}

const BINARY: &[FloatSemanticValueKind] = &[Format, Meaning, Meaning];
const UNARY: &[FloatSemanticValueKind] = &[Format, Meaning];
const TERNARY: &[FloatSemanticValueKind] = &[Format, Meaning, Meaning, Meaning];
const PAIR: &[FloatSemanticValueKind] = &[Meaning, Meaning];
const SINGLE: &[FloatSemanticValueKind] = &[Meaning];

/// Every row, in the order the sealed source declares them. The position is
/// the row's stable ordinal in [`FloatSemanticContractIdentity`].
pub const FLOAT_SEMANTIC_OPERATIONS: &[FloatSemanticOperation] = &[
    row("add", BINARY, Meaning),
    row("subtract", BINARY, Meaning),
    row("multiply", BINARY, Meaning),
    row("divide", BINARY, Meaning),
    row("negate", UNARY, Meaning),
    row("square_root", UNARY, Meaning),
    row("convert", UNARY, Meaning),
    row(
        "from_integer",
        &[Format, integer(IntegerCarrier::I8)],
        Meaning,
    ),
    row(
        "from_integer",
        &[Format, integer(IntegerCarrier::I16)],
        Meaning,
    ),
    row(
        "from_integer",
        &[Format, integer(IntegerCarrier::I32)],
        Meaning,
    ),
    row(
        "from_integer",
        &[Format, integer(IntegerCarrier::I64)],
        Meaning,
    ),
    row(
        "from_integer",
        &[Format, integer(IntegerCarrier::U8)],
        Meaning,
    ),
    row(
        "from_integer",
        &[Format, integer(IntegerCarrier::U16)],
        Meaning,
    ),
    row(
        "from_integer",
        &[Format, integer(IntegerCarrier::U32)],
        Meaning,
    ),
    row(
        "from_integer",
        &[Format, integer(IntegerCarrier::U64)],
        Meaning,
    ),
    row("to_i8", SINGLE, integer(IntegerCarrier::I8)),
    row("to_i16", SINGLE, integer(IntegerCarrier::I16)),
    row("to_i32", SINGLE, integer(IntegerCarrier::I32)),
    row("to_i64", SINGLE, integer(IntegerCarrier::I64)),
    row("to_u8", SINGLE, integer(IntegerCarrier::U8)),
    row("to_u16", SINGLE, integer(IntegerCarrier::U16)),
    row("to_u32", SINGLE, integer(IntegerCarrier::U32)),
    row("to_u64", SINGLE, integer(IntegerCarrier::U64)),
    row("to_i8_saturating", SINGLE, integer(IntegerCarrier::I8)),
    row("to_i16_saturating", SINGLE, integer(IntegerCarrier::I16)),
    row("to_i32_saturating", SINGLE, integer(IntegerCarrier::I32)),
    row("to_i64_saturating", SINGLE, integer(IntegerCarrier::I64)),
    row("to_u8_saturating", SINGLE, integer(IntegerCarrier::U8)),
    row("to_u16_saturating", SINGLE, integer(IntegerCarrier::U16)),
    row("to_u32_saturating", SINGLE, integer(IntegerCarrier::U32)),
    row("to_u64_saturating", SINGLE, integer(IntegerCarrier::U64)),
    row("multiply_then_add", TERNARY, Meaning),
    row("fused_multiply_add", TERNARY, Meaning),
    row("minimum", PAIR, Meaning),
    row("maximum", PAIR, Meaning),
    row("equal", PAIR, Bool),
    row("not_equal", PAIR, Bool),
    row("less", PAIR, Bool),
    row("less_or_equal", PAIR, Bool),
    row("greater", PAIR, Bool),
    row("greater_or_equal", PAIR, Bool),
    row("classify", UNARY, Class),
    row("is_finite", SINGLE, Bool),
    row("is_nan", SINGLE, Bool),
    row("is_infinite", SINGLE, Bool),
    row("is_normal", UNARY, Bool),
    row("is_subnormal", UNARY, Bool),
    row("add_toward_zero", BINARY, Meaning),
    row("add_toward_positive", BINARY, Meaning),
    row("add_toward_negative", BINARY, Meaning),
    row("subtract_toward_zero", BINARY, Meaning),
    row("subtract_toward_positive", BINARY, Meaning),
    row("subtract_toward_negative", BINARY, Meaning),
    row("multiply_toward_zero", BINARY, Meaning),
    row("multiply_toward_positive", BINARY, Meaning),
    row("multiply_toward_negative", BINARY, Meaning),
    row("divide_toward_zero", BINARY, Meaning),
    row("divide_toward_positive", BINARY, Meaning),
    row("divide_toward_negative", BINARY, Meaning),
    row("square_root_toward_zero", UNARY, Meaning),
    row("square_root_toward_positive", UNARY, Meaning),
    row("square_root_toward_negative", UNARY, Meaning),
    row("fused_multiply_add_toward_zero", TERNARY, Meaning),
    row("fused_multiply_add_toward_positive", TERNARY, Meaning),
    row("fused_multiply_add_toward_negative", TERNARY, Meaning),
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
        FLOAT_SEMANTIC_OPERATIONS, FloatSemanticOperation, FloatSemanticValueKind, IntegerCarrier,
    };
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
}
