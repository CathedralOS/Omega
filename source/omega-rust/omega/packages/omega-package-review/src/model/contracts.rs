use super::*;
use psi_symbols::BuiltinFunction;

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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewContractCallTarget {
    Nominal(PackageReviewNominalIdentity),
    /// One exact compiler-installed root builtin-function slot. The spelling
    /// is diagnostic only; package declarations with the same name remain
    /// ordinary nominals.
    BuiltinFunction(BuiltinFunction),
    ByteSequencePredicate(PackageReviewByteSequencePredicate),
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
        }
    }

    pub const fn builtin_function(&self) -> Option<BuiltinFunction> {
        match self {
            Self::BuiltinFunction(function) => Some(*function),
            Self::Nominal(_) | Self::ByteSequencePredicate(_) => None,
        }
    }

    pub const fn byte_sequence_predicate(&self) -> Option<PackageReviewByteSequencePredicate> {
        match self {
            Self::Nominal(_) | Self::BuiltinFunction(_) => None,
            Self::ByteSequencePredicate(predicate) => Some(*predicate),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewContractExpression {
    Boolean(bool),
    Integer(String),
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewPropositionBinderKind {
    Type,
    Const(PackageReviewTypeIdentity),
    Machine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewPropositionBinder {
    pub(crate) kind: PackageReviewPropositionBinderKind,
    pub(crate) bounds: psi_typed_trees::data::DataProperties,
}

impl PartialOrd for PackageReviewPropositionBinder {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PackageReviewPropositionBinder {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.kind.cmp(&other.kind).then_with(|| {
            package_review_data_properties_key(self.bounds)
                .cmp(&package_review_data_properties_key(other.bounds))
        })
    }
}

fn package_review_data_properties_key(
    properties: psi_typed_trees::data::DataProperties,
) -> (u8, Option<(u8, u8, u8, u8)>) {
    let multiplicity = match properties.multiplicity {
        psi_language_semantics::Multiplicity::Unrestricted => 0,
        psi_language_semantics::Multiplicity::Affine => 1,
        psi_language_semantics::Multiplicity::Linear => 2,
    };
    let carry = properties.carry.map(|carry| {
        (
            u8::from(matches!(
                carry.suspension,
                psi_language_semantics::CarrySuspension::Allowed
            )),
            u8::from(matches!(carry.cpu, psi_language_semantics::CarryCpu::Any)),
            u8::from(matches!(
                carry.host_thread,
                psi_language_semantics::CarryHostThread::Any
            )),
            u8::from(matches!(
                carry.address,
                psi_language_semantics::CarryAddress::Movable
            )),
        )
    });
    (multiplicity, carry)
}

impl PackageReviewPropositionBinder {
    pub const fn kind(&self) -> &PackageReviewPropositionBinderKind {
        &self.kind
    }

    pub const fn bounds(&self) -> psi_typed_trees::data::DataProperties {
        self.bounds
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewPropositionBinderValue {
    Type(PackageReviewTypeIdentity),
    Machine(PackageReviewNominalIdentity),
    GenericBinder(u32),
    Integer(String),
    EvidenceProjection {
        source_kind: PackageReviewContractKind,
        source_lane_position: u32,
        declaring_trait: PackageReviewNominalIdentity,
        declaring_trait_arguments: Vec<PackageReviewTypeIdentity>,
        requirement: PackageReviewNominalIdentity,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewPropositionBinderArgument {
    pub(crate) kind: psi_typed_trees::proposition::PropositionBinderArgumentKind,
    pub(crate) value: PackageReviewPropositionBinderValue,
}

impl PackageReviewPropositionBinderArgument {
    pub const fn kind(&self) -> psi_typed_trees::proposition::PropositionBinderArgumentKind {
        self.kind
    }

    pub const fn value(&self) -> &PackageReviewPropositionBinderValue {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewEvidenceRequirement {
    pub(crate) declaring_trait: PackageReviewNominalIdentity,
    pub(crate) declaring_trait_arguments: Vec<PackageReviewTypeIdentity>,
    pub(crate) requirement: PackageReviewNominalIdentity,
}

impl PackageReviewEvidenceRequirement {
    pub const fn declaring_trait(&self) -> &PackageReviewNominalIdentity {
        &self.declaring_trait
    }

    pub fn declaring_trait_arguments(&self) -> &[PackageReviewTypeIdentity] {
        &self.declaring_trait_arguments
    }

    pub const fn requirement(&self) -> &PackageReviewNominalIdentity {
        &self.requirement
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewEvidenceInterface {
    pub(crate) trait_identity: PackageReviewNominalIdentity,
    pub(crate) arguments: Vec<PackageReviewTypeIdentity>,
    pub(crate) requirements: Vec<PackageReviewEvidenceRequirement>,
}

impl PackageReviewEvidenceInterface {
    pub const fn trait_identity(&self) -> &PackageReviewNominalIdentity {
        &self.trait_identity
    }

    pub fn arguments(&self) -> &[PackageReviewTypeIdentity] {
        &self.arguments
    }

    pub fn requirements(&self) -> &[PackageReviewEvidenceRequirement] {
        &self.requirements
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewPropositionEvidence {
    FactOnly,
    Witness(PackageReviewEvidenceInterface),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewPublicPropositionBody {
    Primitive,
    Witness(PackageReviewEvidenceInterface),
    Transparent(PackageReviewContractFact),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewPropositionShape {
    pub(crate) identity: PackageReviewNominalIdentity,
    pub(crate) binders: Vec<PackageReviewPropositionBinder>,
    pub(crate) parameter_types: Vec<PackageReviewTypeIdentity>,
    pub(crate) body: PackageReviewPublicPropositionBody,
}

impl PackageReviewPropositionShape {
    pub const fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }

    pub fn binders(&self) -> &[PackageReviewPropositionBinder] {
        &self.binders
    }

    pub fn parameter_types(&self) -> &[PackageReviewTypeIdentity] {
        &self.parameter_types
    }

    pub const fn body(&self) -> &PackageReviewPublicPropositionBody {
        &self.body
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewConstShape {
    pub(crate) identity: PackageReviewNominalIdentity,
    pub(crate) declared_type: PackageReviewTypeIdentity,
    pub(crate) canonical_value_encoding: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewOperatorShape {
    pub(crate) coordinate: PackageReviewOperatorCoordinate,
    pub(crate) is_boundary: bool,
    pub(crate) spelling: Option<psi_language_core::OperatorSpelling>,
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) type_parameters: Vec<PackageReviewTypeParameter>,
    pub(crate) parameters: Vec<PackageReviewCallableParameter>,
    pub(crate) return_type: PackageReviewTypeIdentity,
    pub(crate) contracts: Vec<PackageReviewCallableContract>,
    pub(crate) published_crash: Vec<PackageReviewCrashRoute>,
}

impl PackageReviewOperatorShape {
    pub const fn coordinate(&self) -> &PackageReviewOperatorCoordinate {
        &self.coordinate
    }

    pub const fn is_boundary(&self) -> bool {
        self.is_boundary
    }

    pub const fn spelling(&self) -> Option<psi_language_core::OperatorSpelling> {
        self.spelling
    }

    pub const fn lifetime_parameter_count(&self) -> usize {
        self.lifetime_parameter_count
    }

    pub fn type_parameters(&self) -> &[PackageReviewTypeParameter] {
        &self.type_parameters
    }

    pub fn parameters(&self) -> &[PackageReviewCallableParameter] {
        &self.parameters
    }

    pub const fn return_type(&self) -> &PackageReviewTypeIdentity {
        &self.return_type
    }

    pub fn contracts(&self) -> &[PackageReviewCallableContract] {
        &self.contracts
    }

    pub fn published_crash(&self) -> &[PackageReviewCrashRoute] {
        &self.published_crash
    }
}

impl PackageReviewConstShape {
    pub const fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }

    pub const fn declared_type(&self) -> &PackageReviewTypeIdentity {
        &self.declared_type
    }

    pub fn canonical_value_encoding(&self) -> &str {
        &self.canonical_value_encoding
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewPropositionApplication {
    pub(crate) declaration: PackageReviewNominalIdentity,
    pub(crate) binders: Vec<PackageReviewPropositionBinder>,
    pub(crate) parameter_types: Vec<PackageReviewTypeIdentity>,
    pub(crate) binder_arguments: Vec<PackageReviewPropositionBinderArgument>,
    pub(crate) arguments: Vec<PackageReviewContractExpression>,
    pub(crate) evidence: PackageReviewPropositionEvidence,
}

impl PackageReviewPropositionApplication {
    pub const fn declaration(&self) -> &PackageReviewNominalIdentity {
        &self.declaration
    }

    pub fn binders(&self) -> &[PackageReviewPropositionBinder] {
        &self.binders
    }

    pub fn parameter_types(&self) -> &[PackageReviewTypeIdentity] {
        &self.parameter_types
    }

    pub fn binder_arguments(&self) -> &[PackageReviewPropositionBinderArgument] {
        &self.binder_arguments
    }

    pub fn arguments(&self) -> &[PackageReviewContractExpression] {
        &self.arguments
    }

    pub const fn evidence(&self) -> &PackageReviewPropositionEvidence {
        &self.evidence
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewPropositionParameterApplication {
    pub(crate) binder_ordinal: u32,
    pub(crate) arguments: Vec<PackageReviewContractExpression>,
}

impl PackageReviewPropositionParameterApplication {
    pub const fn binder_ordinal(&self) -> u32 {
        self.binder_ordinal
    }

    pub fn arguments(&self) -> &[PackageReviewContractExpression] {
        &self.arguments
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewContractFact {
    Expression(PackageReviewContractExpression),
    Membership {
        value: PackageReviewContractExpression,
        domain: PackageReviewNominalIdentity,
    },
    Proposition(PackageReviewPropositionApplication),
    PropositionParameter(PackageReviewPropositionParameterApplication),
}

/// Exact nominal result-arm coordinate guarding one outcome-specific
/// guarantee. The coordinate is absent for unconditional `requires` and
/// `ensures` rows.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewResultCaseIdentity {
    pub(crate) result_data: PackageReviewNominalIdentity,
    pub(crate) result_case: PackageReviewNominalIdentity,
}

impl PackageReviewResultCaseIdentity {
    pub const fn result_data(&self) -> &PackageReviewNominalIdentity {
        &self.result_data
    }

    pub const fn result_case(&self) -> &PackageReviewNominalIdentity {
        &self.result_case
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewCallableContract {
    pub(crate) kind: PackageReviewContractKind,
    pub(crate) result_case: Option<PackageReviewResultCaseIdentity>,
    pub(crate) binding: Option<String>,
    pub(crate) evidence_lane_position: Option<u32>,
    pub(crate) fact: PackageReviewContractFact,
}

impl PackageReviewCallableContract {
    pub const fn kind(&self) -> PackageReviewContractKind {
        self.kind
    }

    pub const fn result_case(&self) -> Option<&PackageReviewResultCaseIdentity> {
        self.result_case.as_ref()
    }

    pub fn binding(&self) -> Option<&str> {
        self.binding.as_deref()
    }

    pub const fn evidence_lane_position(&self) -> Option<u32> {
        self.evidence_lane_position
    }

    pub const fn fact(&self) -> &PackageReviewContractFact {
        &self.fact
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewSynchronousInvocation {
    Parameter(u32),
    Service(PackageReviewNominalIdentity),
}

impl PackageReviewSynchronousInvocation {
    pub const fn parameter(&self) -> Option<u32> {
        match self {
            Self::Parameter(position) => Some(*position),
            Self::Service(_) => None,
        }
    }

    pub const fn service(&self) -> Option<&PackageReviewNominalIdentity> {
        match self {
            Self::Parameter(_) => None,
            Self::Service(service) => Some(service),
        }
    }
}
