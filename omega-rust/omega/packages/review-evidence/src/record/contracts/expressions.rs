// Denotational contract expression, static argument, evidence lane, and
// operator overload vocabulary for package review.
//
// `PackageReviewCallableSupply` classifies callable supply on the callable
// envelope while projecting external binding identity separately as an
// executable-supply trust row. `PackageReviewContractStaticArgument` preserves
// exact static arguments across generic type/const/machine binders, structured
// const values, concrete machines, and lifetime-free closed conformance
// applications. `PackageReviewContractCallTarget` distinguishes nominal
// callables, root builtin-function slots, byte-sequence predicates, and
// compiler-owned opaque `ProofView` atoms. `PackageReviewOperatorCoordinate`
// identifies ordinary operator overloads by nominal path plus canonical
// parameter and result dispatch identities. `PackageReviewContractExpression`
// represents authenticated semantic contract expressions (layout queries,
// static type equality, exact IEEE float bits, array/constructor/indexed/range
// forms, raw byte sequences, built-in type members, case membership,
// denotational references, atomic loads, zero values, collection
// length/capacity, casts, calls, and first-match value dispatch).

use crate::record::PackageReviewNominalIdentity;
use crate::record::PackageReviewTypeIdentity;
use crate::record::accessors::field_projection_accessors;
use symbols::BuiltinFunction;
mod operator_policy;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PackageReviewCallableRole {
    Boundary,
    Public,
    Build,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewCallableSupply {
    CheckedBody,
    Requirement,
    TopLevelRequirement,
    Boundary,
    AdmissionClaim,
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
    Recast(PackageReviewReferenceAccess),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewLayoutQueryKind {
    Size,
    Alignment,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewContractStaticArgument {
    Type(PackageReviewTypeIdentity),
    GenericTypeBinder(u32),
    GenericType {
        base: PackageReviewTypeIdentity,
        lifetime_arguments: Vec<u32>,
        arguments: Vec<PackageReviewContractStaticArgument>,
    },
    ConstInteger(String),
    ConstBoolean(bool),
    ConstStructured {
        declared_type: PackageReviewTypeIdentity,
        canonical_value_encoding: String,
    },
    GenericConstBinder(u32),
    GenericMachineBinder(u32),
    ConcreteMachine(PackageReviewNominalIdentity),
    ConformanceApplication {
        declaration: PackageReviewNominalIdentity,
        arguments: Vec<PackageReviewContractStaticArgument>,
        trait_identity: PackageReviewNominalIdentity,
        trait_arguments: Vec<PackageReviewTypeIdentity>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewByteSequencePredicate {
    ValidUtf8,
    NoNul,
    AsciiOnly,
    NonEmpty,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewContractCallTarget {
    Nominal(PackageReviewNominalIdentity),
    BuiltinFunction(BuiltinFunction),
    ByteSequencePredicate(PackageReviewByteSequencePredicate),
    ProofView { name: String },
    Runtime { contract: PackageReviewTypeIdentity, callee: Box<PackageReviewContractExpression> },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewContractEvidenceOwner {
    Nominal(PackageReviewNominalIdentity),
    Runtime { contract: PackageReviewTypeIdentity },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewContractEvidenceTerm {
    pub(crate) owner: PackageReviewContractEvidenceOwner,
    pub(crate) kind: PackageReviewContractKind,
    pub(crate) lane_position: u32,
}

impl PackageReviewContractEvidenceTerm {
    field_projection_accessors!(borrowed owner: &PackageReviewContractEvidenceOwner);
    field_projection_accessors!(copied kind: PackageReviewContractKind, lane_position: u32);
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewContractEvidenceArgument {
    pub(crate) lane_position: u32,
    pub(crate) source: PackageReviewContractEvidenceTerm,
    pub(crate) parameter: PackageReviewContractEvidenceTerm,
}

impl PackageReviewContractEvidenceArgument {
    field_projection_accessors!(copied lane_position: u32);
    field_projection_accessors!(borrowed source: &PackageReviewContractEvidenceTerm);
    field_projection_accessors!(borrowed parameter: &PackageReviewContractEvidenceTerm);
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewOperatorCoordinate {
    pub(crate) identity: PackageReviewNominalIdentity,
    pub(crate) parameter_dispatch: String,
    pub(crate) result_dispatch: String,
}

impl PackageReviewOperatorCoordinate {
    field_projection_accessors!(borrowed identity: &PackageReviewNominalIdentity);
    pub fn result_dispatch(&self) -> &str {
        &self.result_dispatch
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewOperatorRealization {
    pub(crate) coordinate: PackageReviewOperatorCoordinate,
    pub(crate) alias: Option<String>,
}

impl PackageReviewOperatorRealization {
    field_projection_accessors!(borrowed coordinate: &PackageReviewOperatorCoordinate);

    pub fn alias(&self) -> Option<&str> {
        self.alias.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewContractOperatorMeaning {
    Builtin,
    Declared(PackageReviewOperatorCoordinate),
}

impl PackageReviewContractCallTarget {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewContractExpression {
    MachineSelection { contract: PackageReviewTypeIdentity, selection: String },
    LayoutQuery {
        kind: PackageReviewLayoutQueryKind,
        operand: PackageReviewTypeIdentity,
    },
    TypeEquality {
        left: PackageReviewTypeIdentity,
        right: PackageReviewTypeIdentity,
        negated: bool,
    },
    Boolean(bool),
    Integer(String),
    Float(PackageReviewFloatLiteral),
    Array(Vec<PackageReviewContractExpression>),
    Constructor {
        data: PackageReviewNominalIdentity,
        case: Option<PackageReviewNominalIdentity>,
        fields: Vec<PackageReviewConstructorField>,
    },
    Indexed {
        meaning: PackageReviewContractOperatorMeaning,
        collection: Box<PackageReviewContractExpression>,
        index: Box<PackageReviewContractExpression>,
    },
    Range {
        start: Option<Box<PackageReviewContractExpression>>,
        end: Option<Box<PackageReviewContractExpression>>,
        end_inclusive: bool,
    },
    ByteSequence(Vec<u8>),
    DomainSubject,
    Parameter(u32),
    Result,
    GenericBinder(u32),
    BuiltinTypeMember(symbols::BuiltinTypeMember),
    Nominal(PackageReviewNominalIdentity),
    CaseMembership {
        subject: Box<PackageReviewContractExpression>,
        case: PackageReviewNominalIdentity,
    },
    Reference {
        access: PackageReviewReferenceAccess,
        target: Box<PackageReviewContractExpression>,
    },
    AtomicLoad {
        value: Box<PackageReviewContractExpression>,
        ordering: PackageReviewAtomicLoadOrdering,
    },
    ZeroValue(PackageReviewTypeIdentity),
    CollectionLength {
        collection: Box<PackageReviewContractExpression>,
    },
    CollectionCapacity {
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
    Call {
        receiver: Option<Box<PackageReviewContractExpression>>,
        target: PackageReviewContractCallTarget,
        static_arguments: Vec<PackageReviewContractStaticArgument>,
        evidence_arguments: Vec<PackageReviewContractEvidenceArgument>,
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
    Match {
        subject: Box<PackageReviewContractExpression>,
        arms: Vec<PackageReviewMatchArm>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewMatchArm {
    pub(crate) pattern: Option<PackageReviewContractExpression>, // `None` is the wildcard arm.
    pub(crate) value: PackageReviewContractExpression,
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

impl PackageReviewConstructorField {}
