use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewCallableRole {
    Boundary,
    Public,
    Build,
}

/// Source-handle-free supply classification retained on the callable envelope.
/// Exact external binding identity is projected separately as an executable-
/// supply trust row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewCallableSupply {
    CheckedBody,
    Requirement,
    Boundary,
    Accepted,
    ExternalRealization,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewContractKind {
    Requires,
    Ensures,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewContractBinaryOperator {
    Add,
    And,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    Divide,
    Equal,
    Greater,
    GreaterOrEqual,
    Less,
    LessOrEqual,
    Modulo,
    Multiply,
    NotEqual,
    Or,
    ShiftLeft,
    ShiftRight,
    Subtract,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewContractUnaryOperator {
    BitwiseNot,
    LogicalNot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewReferenceAccess {
    Shared,
    Mutable,
    WriteOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewAtomicLoadOrdering {
    NoOrdering,
    Receive,
    GlobalOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewArithmeticDomain {
    Exact,
    Wrapping,
    Saturating,
    Trapping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewCastForm {
    Value,
    RecastShared,
    RecastMutable,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewContractStaticArgument {
    /// One exact concrete type identity.
    Type(PackageReviewTypeIdentity),
    /// One type parameter from the containing declaration's canonical static
    /// telescope. The ordinal spans every static parameter category.
    GenericTypeBinder(u32),
    /// One exact generic data-family application whose base declaration and
    /// recursively categorized static arguments rejoin the checked data
    /// telescope. Lifetime arguments are caller-binder ordinals.
    GenericType {
        base: PackageReviewTypeIdentity,
        lifetime_arguments: Vec<u32>,
        arguments: Vec<PackageReviewContractStaticArgument>,
    },
    /// One parser-canonical integer literal in an exact const-parameter slot.
    ConstInteger(String),
    /// One const parameter from the containing declaration's canonical static
    /// telescope. The ordinal spans every static parameter category.
    GenericConstBinder(u32),
    /// One machine parameter from the containing declaration's canonical
    /// static telescope. The ordinal spans every static parameter category.
    GenericMachineBinder(u32),
    /// The exact selected concrete machine entry, including package or
    /// compiler/toolchain ownership.
    ConcreteMachine(PackageReviewNominalIdentity),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewByteSequencePredicate {
    ValidUtf8,
    NoNul,
    AsciiOnly,
    NonEmpty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewCollectionViewOperation {
    SharedSlice,
    MutableSlice,
    TextView,
    Bytes,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewContractCallTarget {
    Nominal(PackageReviewNominalIdentity),
    /// One exact compiler-installed root builtin-function slot. The spelling
    /// is diagnostic only; package declarations with the same name remain
    /// ordinary nominals.
    BuiltinFunction(BuiltinFunction),
    ByteSequencePredicate(PackageReviewByteSequencePredicate),
    /// One exact compiler-owned collection/text view operation. Package
    /// callables with the same spelling remain ordinary nominals.
    CollectionView(PackageReviewCollectionViewOperation),
}

/// Stable identity of one ordinary operator overload. The nominal path names
/// the declaration family; the compiler's canonical parameter and
/// result-dispatch identities distinguish overloads by the same rules used by
/// checked selection. Source names, arena handles, and return refinements that
/// do not participate in dispatch are not coordinates.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewOperatorCoordinate {
    pub(crate) identity: PackageReviewNominalIdentity,
    pub(crate) parameter_dispatch: String,
    pub(crate) result_dispatch: String,
}

impl PackageReviewOperatorCoordinate {
    pub const fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }

    pub fn parameter_dispatch(&self) -> &str {
        &self.parameter_dispatch
    }

    pub fn result_dispatch(&self) -> &str {
        &self.result_dispatch
    }
}

/// One exact operator requirement realized by a reviewed callable.
///
/// The coordinate identifies the selected overload. The alias preserves the
/// authored local conformance name used by the checked body; it is not part of
/// the operator declaration coordinate.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewOperatorRealization {
    pub(crate) coordinate: PackageReviewOperatorCoordinate,
    pub(crate) alias: Option<String>,
}

impl PackageReviewOperatorRealization {
    pub const fn coordinate(&self) -> &PackageReviewOperatorCoordinate {
        &self.coordinate
    }

    pub fn alias(&self) -> Option<&str> {
        self.alias.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewContractOperatorMeaning {
    Builtin,
    Declared(PackageReviewOperatorCoordinate),
}

impl PackageReviewContractCallTarget {
    pub const fn nominal(&self) -> Option<&PackageReviewNominalIdentity> {
        match self {
            Self::Nominal(identity) => Some(identity),
            Self::BuiltinFunction(_) => None,
            Self::ByteSequencePredicate(_) => None,
            Self::CollectionView(_) => None,
        }
    }

    pub const fn builtin_function(&self) -> Option<BuiltinFunction> {
        match self {
            Self::BuiltinFunction(function) => Some(*function),
            Self::Nominal(_) | Self::ByteSequencePredicate(_) | Self::CollectionView(_) => None,
        }
    }

    pub const fn byte_sequence_predicate(&self) -> Option<PackageReviewByteSequencePredicate> {
        match self {
            Self::Nominal(_) | Self::BuiltinFunction(_) | Self::CollectionView(_) => None,
            Self::ByteSequencePredicate(predicate) => Some(*predicate),
        }
    }

    pub const fn collection_view(&self) -> Option<PackageReviewCollectionViewOperation> {
        match self {
            Self::CollectionView(operation) => Some(*operation),
            Self::Nominal(_) | Self::BuiltinFunction(_) | Self::ByteSequencePredicate(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewContractExpression {
    Boolean(bool),
    Integer(String),
    /// One width-landed IEEE literal. Exact checked bits, including signed
    /// zero, are semantic identity; decimal source spelling is not.
    Float(PackageReviewFloatLiteral),
    /// Ordered structural elements of one checked array literal.
    Array(Vec<PackageReviewContractExpression>),
    Constructor {
        data: PackageReviewNominalIdentity,
        case: Option<PackageReviewNominalIdentity>,
        fields: Vec<PackageReviewConstructorField>,
    },
    /// One checked indexing or slicing operator application. The selected
    /// operator meaning remains distinct from its structural operands.
    Indexed {
        meaning: PackageReviewContractOperatorMeaning,
        collection: Box<PackageReviewContractExpression>,
        index: Box<PackageReviewContractExpression>,
    },
    /// Structural range operand used by an indexed contract expression.
    /// Missing endpoints are explicit; inclusive and exclusive ends remain
    /// distinct checked forms.
    Range {
        start: Option<Box<PackageReviewContractExpression>>,
        end: Option<Box<PackageReviewContractExpression>>,
        end_inclusive: bool,
    },
    /// Exact decoded octets of an Omega quoted literal. No text encoding is
    /// implied by this row.
    ByteSequence(Vec<u8>),
    /// The implicit carrier being classified by a domain predicate.
    DomainSubject,
    Parameter(u32),
    Result,
    GenericBinder(u32),
    Nominal(PackageReviewNominalIdentity),
    /// One explicit denotational reference formation. Runtime loan identity
    /// and source lifetime spelling are not package contract identity.
    Reference {
        access: PackageReviewReferenceAccess,
        target: Box<PackageReviewContractExpression>,
    },
    /// One denotational atomic load retained with its exact checked ordering.
    /// Mutation-bearing atomic operations remain outside contract identity.
    AtomicLoad {
        value: Box<PackageReviewContractExpression>,
        ordering: PackageReviewAtomicLoadOrdering,
    },
    /// Proof-only observation of one exact type's normalized all-zero home
    /// representation. The checker rejects quotient targets before review.
    ZeroValue(PackageReviewTypeIdentity),
    /// Compiler-owned `len` projection on a fixed array or slice. The exact
    /// checked authored-selection occurrence must identify this intrinsic;
    /// same-spelled package fields remain ordinary nominal members.
    CollectionLength {
        collection: Box<PackageReviewContractExpression>,
    },
    Member {
        receiver: Box<PackageReviewContractExpression>,
        member: PackageReviewNominalIdentity,
        case_variant: Option<PackageReviewNominalIdentity>,
    },
    Cast {
        value: Box<PackageReviewContractExpression>,
        target: PackageReviewTypeIdentity,
        arithmetic_domain: PackageReviewArithmeticDomain,
        semantic_domain: Option<PackageReviewNominalIdentity>,
        semantic_domain_arguments: Vec<PackageReviewTypeIdentity>,
        form: PackageReviewCastForm,
    },
    /// One checked, denotational call edge in a public proposition. The
    /// package source-consumption commitment separately pins the selected
    /// callable's implementation; this row does not pretend that a callable
    /// signature identifies its body.
    Call {
        receiver: Option<Box<PackageReviewContractExpression>>,
        target: PackageReviewContractCallTarget,
        static_arguments: Vec<PackageReviewContractStaticArgument>,
        arguments: Vec<PackageReviewContractExpression>,
    },
    Binary {
        meaning: PackageReviewContractOperatorMeaning,
        operator: PackageReviewContractBinaryOperator,
        left: Box<PackageReviewContractExpression>,
        right: Box<PackageReviewContractExpression>,
    },
    Unary {
        operator: PackageReviewContractUnaryOperator,
        operand: Box<PackageReviewContractExpression>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewFloatLiteral {
    F32(u32),
    F64(u64),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewConstructorField {
    pub(crate) field: PackageReviewNominalIdentity,
    pub(crate) value: PackageReviewContractExpression,
}

impl PackageReviewConstructorField {
    pub const fn field(&self) -> &PackageReviewNominalIdentity {
        &self.field
    }

    pub const fn value(&self) -> &PackageReviewContractExpression {
        &self.value
    }
}
