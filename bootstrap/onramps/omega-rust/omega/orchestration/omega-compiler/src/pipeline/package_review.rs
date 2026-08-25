//! Compiler-owned, in-memory package authority projection.
//!
//! This is deliberately a review surface, not admission evidence. Authored
//! toolchain nominals are bound to exact source commitments, but whole-source,
//! compiler/toolchain, provider-nominal, proof, and trust commitments still
//! live outside this projection.
//! Keeping the type distinct prevents an incomplete checked summary from being
//! persisted as an accepted lock baseline.

mod encoding;
mod recovery;

pub use encoding::{
    PACKAGE_REVIEW_ENCODING_VERSION, PACKAGE_REVIEW_ROW_ENCODING_VERSION,
    PackageReviewEncodingError,
};
pub use recovery::{
    DecodedPackageReviewCanonicalRow, PACKAGE_REVIEW_CANONICAL_ROW_RECOVERY_VERSION,
    PackageReviewCanonicalRowRecoveryError, PackageReviewCanonicalRowRecoveryLimits,
    decode_package_review_canonical_row, decode_package_review_canonical_row_with_limits,
    encode_package_review_canonical_row, encode_package_review_canonical_row_with_limits,
};

use crate::pipeline::CheckedCompilation;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;
use psi_language_semantics::MachineSupplyMode;
use psi_symbols::SymbolHandle;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewTypeIdentity {
    canonical: String,
}

impl PackageReviewTypeIdentity {
    pub fn canonical(&self) -> &str {
        &self.canonical
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageReviewTypeParameterKind {
    Type,
    Const(PackageReviewTypeIdentity),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewTypeParameter {
    kind: PackageReviewTypeParameterKind,
    bounds: psi_typed_trees::data::DataProperties,
}

/// One generic conformance requirement in a public signature.
///
/// An explicit proof-static binder is alpha-normalized to `binder_ordinal`;
/// `None` retains a binder-free `where T satisfies Trait` requirement without
/// fabricating evidence. The subject is the ordinal of an ordinary type
/// parameter in the containing declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewConformanceBound {
    binder_ordinal: Option<u32>,
    subject_parameter: u32,
    selected_conformance: Option<PackageReviewNominalIdentity>,
    selected_carrier: Option<PackageReviewNominalIdentity>,
    selected_carrier_arguments: Vec<PackageReviewTypeIdentity>,
    trait_identity: PackageReviewNominalIdentity,
    arguments: Vec<PackageReviewTypeIdentity>,
}

impl PackageReviewConformanceBound {
    pub const fn binder_ordinal(&self) -> Option<u32> {
        self.binder_ordinal
    }

    pub const fn subject_parameter(&self) -> u32 {
        self.subject_parameter
    }

    pub const fn selected_conformance(&self) -> Option<&PackageReviewNominalIdentity> {
        self.selected_conformance.as_ref()
    }

    pub const fn selected_carrier(&self) -> Option<&PackageReviewNominalIdentity> {
        self.selected_carrier.as_ref()
    }

    pub fn selected_carrier_arguments(&self) -> &[PackageReviewTypeIdentity] {
        &self.selected_carrier_arguments
    }

    pub const fn trait_identity(&self) -> &PackageReviewNominalIdentity {
        &self.trait_identity
    }

    pub fn arguments(&self) -> &[PackageReviewTypeIdentity] {
        &self.arguments
    }
}

impl PackageReviewTypeParameter {
    pub const fn kind(&self) -> &PackageReviewTypeParameterKind {
        &self.kind
    }

    pub const fn bounds(&self) -> psi_typed_trees::data::DataProperties {
        self.bounds
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewTraitParent {
    kind: psi_typed_trees::trait_definition::TraitCompositionKind,
    identity: PackageReviewNominalIdentity,
    lifetime_arguments: Vec<u32>,
    arguments: Vec<PackageReviewTypeIdentity>,
}

impl PackageReviewTraitParent {
    pub const fn kind(&self) -> psi_typed_trees::trait_definition::TraitCompositionKind {
        self.kind
    }

    pub const fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }

    pub fn lifetime_arguments(&self) -> &[u32] {
        &self.lifetime_arguments
    }

    pub fn arguments(&self) -> &[PackageReviewTypeIdentity] {
        &self.arguments
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewTraitRequirementParameter {
    name: String,
    type_identity: PackageReviewTypeIdentity,
    is_const: bool,
    is_mutable: bool,
    is_self: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewCallableParameter {
    name: String,
    type_identity: PackageReviewTypeIdentity,
    is_const: bool,
    is_mutable: bool,
    is_self: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewCallableConformance {
    trait_identity: PackageReviewNominalIdentity,
    requirement_identity: PackageReviewNominalIdentity,
    arguments: Vec<PackageReviewTypeIdentity>,
    alias: Option<String>,
}

impl PackageReviewCallableConformance {
    pub const fn trait_identity(&self) -> &PackageReviewNominalIdentity {
        &self.trait_identity
    }

    pub const fn requirement_identity(&self) -> &PackageReviewNominalIdentity {
        &self.requirement_identity
    }

    pub fn arguments(&self) -> &[PackageReviewTypeIdentity] {
        &self.arguments
    }

    pub fn alias(&self) -> Option<&str> {
        self.alias.as_deref()
    }
}

impl PackageReviewCallableParameter {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn type_identity(&self) -> &PackageReviewTypeIdentity {
        &self.type_identity
    }

    pub const fn is_const(&self) -> bool {
        self.is_const
    }

    pub const fn is_mutable(&self) -> bool {
        self.is_mutable
    }

    pub const fn is_self(&self) -> bool {
        self.is_self
    }
}

impl PackageReviewTraitRequirementParameter {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn type_identity(&self) -> &PackageReviewTypeIdentity {
        &self.type_identity
    }

    pub const fn is_const(&self) -> bool {
        self.is_const
    }

    pub const fn is_mutable(&self) -> bool {
        self.is_mutable
    }

    pub const fn is_self(&self) -> bool {
        self.is_self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewTraitRequirement {
    identity: PackageReviewNominalIdentity,
    spelling: Option<psi_language_core::OperatorSpelling>,
    /// Body presence is public conformance behavior. The body itself remains
    /// checked source, not a compiler-private IR blob in package evidence.
    has_default_realization: bool,
    lifetime_parameter_count: usize,
    type_parameters: Vec<PackageReviewTypeParameter>,
    parameters: Vec<PackageReviewTraitRequirementParameter>,
    return_type: PackageReviewTypeIdentity,
    contracts: Vec<PackageReviewCallableContract>,
    /// Abstract published crash ceiling for this requirement. Trait
    /// requirements have no checked body sites or calls of their own.
    published_crash: Vec<PackageReviewCrashRoute>,
    service_reach: Vec<PackageReviewNominalIdentity>,
    service_reach_is_installation_bound: bool,
    synchronous_invocations: Vec<PackageReviewSynchronousInvocation>,
    suspends: bool,
    blocks: bool,
    termination: PackageReviewTermination,
}

impl PackageReviewTraitRequirement {
    pub const fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }

    pub const fn spelling(&self) -> Option<psi_language_core::OperatorSpelling> {
        self.spelling
    }

    pub const fn has_default_realization(&self) -> bool {
        self.has_default_realization
    }

    pub const fn lifetime_parameter_count(&self) -> usize {
        self.lifetime_parameter_count
    }

    pub fn type_parameters(&self) -> &[PackageReviewTypeParameter] {
        &self.type_parameters
    }

    pub fn parameters(&self) -> &[PackageReviewTraitRequirementParameter] {
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

    pub fn service_reach(&self) -> &[PackageReviewNominalIdentity] {
        &self.service_reach
    }

    pub const fn service_reach_is_installation_bound(&self) -> bool {
        self.service_reach_is_installation_bound
    }

    pub fn synchronous_invocations(&self) -> &[PackageReviewSynchronousInvocation] {
        &self.synchronous_invocations
    }

    pub const fn suspends(&self) -> bool {
        self.suspends
    }

    pub const fn blocks(&self) -> bool {
        self.blocks
    }

    pub const fn termination(&self) -> &PackageReviewTermination {
        &self.termination
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewTraitShape {
    identity: PackageReviewNominalIdentity,
    is_boundary: bool,
    lifetime_parameter_count: usize,
    type_parameters: Vec<PackageReviewTypeParameter>,
    conformance_bounds: Vec<PackageReviewConformanceBound>,
    parents: Vec<PackageReviewTraitParent>,
    requirements: Vec<PackageReviewTraitRequirement>,
}

impl PackageReviewTraitShape {
    pub const fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }

    pub const fn is_boundary(&self) -> bool {
        self.is_boundary
    }

    pub const fn lifetime_parameter_count(&self) -> usize {
        self.lifetime_parameter_count
    }

    pub fn type_parameters(&self) -> &[PackageReviewTypeParameter] {
        &self.type_parameters
    }

    pub fn conformance_bounds(&self) -> &[PackageReviewConformanceBound] {
        &self.conformance_bounds
    }

    pub fn parents(&self) -> &[PackageReviewTraitParent] {
        &self.parents
    }

    pub fn requirements(&self) -> &[PackageReviewTraitRequirement] {
        &self.requirements
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageReviewDomainClassification {
    ProgressProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewDomainEstablishmentKind {
    CheckedRequirement,
    BoundaryRequirement,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewDomainEstablishmentRoute {
    kind: PackageReviewDomainEstablishmentKind,
    trait_identity: PackageReviewNominalIdentity,
    requirement_identity: PackageReviewNominalIdentity,
}

impl PackageReviewDomainEstablishmentRoute {
    pub const fn kind(&self) -> PackageReviewDomainEstablishmentKind {
        self.kind
    }

    pub const fn trait_identity(&self) -> &PackageReviewNominalIdentity {
        &self.trait_identity
    }

    pub const fn requirement_identity(&self) -> &PackageReviewNominalIdentity {
        &self.requirement_identity
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewDomainShape {
    identity: PackageReviewNominalIdentity,
    type_parameters: Vec<PackageReviewTypeParameter>,
    target_type: PackageReviewTypeIdentity,
    index_arguments: Vec<PackageReviewTypeIdentity>,
    predicate_body: psi_language_semantics::DomainPredicateBody,
    predicate_facts: Vec<PackageReviewContractFact>,
    alias_expansion: Option<Vec<PackageReviewNominalIdentity>>,
    classification: Option<PackageReviewDomainClassification>,
    establishment_routes: Vec<PackageReviewDomainEstablishmentRoute>,
}

impl PackageReviewDomainShape {
    pub const fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }

    pub fn type_parameters(&self) -> &[PackageReviewTypeParameter] {
        &self.type_parameters
    }

    pub const fn target_type(&self) -> &PackageReviewTypeIdentity {
        &self.target_type
    }

    pub fn index_arguments(&self) -> &[PackageReviewTypeIdentity] {
        &self.index_arguments
    }

    pub const fn predicate_body(&self) -> psi_language_semantics::DomainPredicateBody {
        self.predicate_body
    }

    pub fn predicate_facts(&self) -> &[PackageReviewContractFact] {
        &self.predicate_facts
    }

    pub fn alias_expansion(&self) -> Option<&[PackageReviewNominalIdentity]> {
        self.alias_expansion.as_deref()
    }

    pub const fn classification(&self) -> Option<PackageReviewDomainClassification> {
        self.classification
    }

    pub fn establishment_routes(&self) -> &[PackageReviewDomainEstablishmentRoute] {
        &self.establishment_routes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewDataField {
    identity: Option<u64>,
    name: String,
    relevance: psi_language_core::BindingRelevance,
    type_identity: PackageReviewTypeIdentity,
}

impl PackageReviewDataField {
    pub const fn identity(&self) -> Option<u64> {
        self.identity
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn relevance(&self) -> psi_language_core::BindingRelevance {
        self.relevance
    }

    pub const fn type_identity(&self) -> &PackageReviewTypeIdentity {
        &self.type_identity
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageReviewDataMember {
    Field(PackageReviewDataField),
    Variant {
        identity: Option<u64>,
        name: String,
        payload: Vec<PackageReviewDataField>,
        retired_payload_identities: Vec<u64>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewDataShape {
    identity: PackageReviewNominalIdentity,
    supply: psi_language_semantics::DataSupplyMode,
    lifetime_parameter_count: usize,
    type_parameters: Vec<PackageReviewTypeParameter>,
    properties: psi_typed_trees::data::DataProperties,
    zero_gated: bool,
    retired_identities: Vec<u64>,
    members: Vec<PackageReviewDataMember>,
}

/// The representation/ABI commitment retained for an opaque boundary datum.
/// Review projection currently has no sealed realization join, so it can only
/// state that the commitment is absent rather than inventing a layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewRepresentationAbiCommitment {
    Unbound,
}

/// The selected external representation mechanism for an opaque boundary
/// datum. Mechanism selection is not yet joined into checked package review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewRepresentationMechanism {
    Unbound,
}

/// Distinct representation-TCB evidence for one package-owned opaque boundary
/// datum. This row is emitted independently of visibility, claims, and reach:
/// none of those facts can make an externally supplied representation cease to
/// be trusted implementation surface.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewRepresentationTcb {
    declaration: PackageReviewNominalIdentity,
    abi: PackageReviewRepresentationAbiCommitment,
    mechanism: PackageReviewRepresentationMechanism,
}

impl PackageReviewRepresentationTcb {
    pub const fn declaration(&self) -> &PackageReviewNominalIdentity {
        &self.declaration
    }

    pub const fn abi(&self) -> PackageReviewRepresentationAbiCommitment {
        self.abi
    }

    pub const fn mechanism(&self) -> PackageReviewRepresentationMechanism {
        self.mechanism
    }
}

impl PackageReviewDataShape {
    pub const fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }

    pub const fn supply(&self) -> psi_language_semantics::DataSupplyMode {
        self.supply
    }

    pub const fn lifetime_parameter_count(&self) -> usize {
        self.lifetime_parameter_count
    }

    pub fn type_parameters(&self) -> &[PackageReviewTypeParameter] {
        &self.type_parameters
    }

    pub const fn properties(&self) -> psi_typed_trees::data::DataProperties {
        self.properties
    }

    pub const fn zero_gated(&self) -> bool {
        self.zero_gated
    }

    pub fn retired_identities(&self) -> &[u64] {
        &self.retired_identities
    }

    pub fn members(&self) -> &[PackageReviewDataMember] {
        &self.members
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewCallableRole {
    Boundary,
    Public,
    Build,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewContractKind {
    Requires,
    Ensures,
    Boundary,
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
pub enum PackageReviewContractExpression {
    Boolean(bool),
    Integer(String),
    /// The implicit carrier being classified by a domain predicate.
    DomainSubject,
    Parameter(u32),
    Result,
    GenericBinder(u32),
    Nominal(PackageReviewNominalIdentity),
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
    Binary {
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
pub enum PackageReviewPropositionBinderKind {
    Type,
    Const(PackageReviewTypeIdentity),
    Machine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewPropositionBinder {
    kind: PackageReviewPropositionBinderKind,
    bounds: psi_typed_trees::data::DataProperties,
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
    Nominal(PackageReviewNominalIdentity),
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
    kind: psi_typed_trees::proposition::PropositionBinderArgumentKind,
    value: PackageReviewPropositionBinderValue,
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
    declaring_trait: PackageReviewNominalIdentity,
    declaring_trait_arguments: Vec<PackageReviewTypeIdentity>,
    requirement: PackageReviewNominalIdentity,
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
    trait_identity: PackageReviewNominalIdentity,
    arguments: Vec<PackageReviewTypeIdentity>,
    requirements: Vec<PackageReviewEvidenceRequirement>,
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
pub struct PackageReviewPropositionApplication {
    declaration: PackageReviewNominalIdentity,
    binders: Vec<PackageReviewPropositionBinder>,
    parameter_types: Vec<PackageReviewTypeIdentity>,
    binder_arguments: Vec<PackageReviewPropositionBinderArgument>,
    arguments: Vec<PackageReviewContractExpression>,
    evidence: PackageReviewPropositionEvidence,
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
pub enum PackageReviewContractFact {
    Expression(PackageReviewContractExpression),
    Membership {
        value: PackageReviewContractExpression,
        domain: PackageReviewNominalIdentity,
    },
    Proposition(PackageReviewPropositionApplication),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewCallableContract {
    kind: PackageReviewContractKind,
    binding: Option<String>,
    evidence_lane_position: Option<u32>,
    fact: PackageReviewContractFact,
}

impl PackageReviewCallableContract {
    pub const fn kind(&self) -> PackageReviewContractKind {
        self.kind
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageReviewToolchainSourceIdentity {
    digest: [u8; 32],
}

impl PackageReviewToolchainSourceIdentity {
    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewNominalOwner {
    Package(PackageKeyIdentity),
    /// Exact authored toolchain source coordinate and bytes. This binds the
    /// nominal declaration but is not the whole compiler/toolchain commitment
    /// required by sealed admission.
    ToolchainSource(PackageReviewToolchainSourceIdentity),
    /// Compiler-intrinsic identity with no authored source coordinate. Review
    /// keeps this visibly unbound until an exact compiler commitment exists.
    ToolchainUnbound,
    /// Checked lowering retained a nominal reference without an authored
    /// source owner or mandatory compiler derivation origin. Review surfaces
    /// it explicitly; admission must reject it.
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewNominalIdentity {
    owner: PackageReviewNominalOwner,
    path: String,
}

impl PackageReviewNominalIdentity {
    pub const fn owner(&self) -> PackageReviewNominalOwner {
        self.owner
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewSemanticDependencyExposure {
    PrivateImplementation,
    PublicInterface,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewSemanticDependencyKind {
    NominalIdentity,
    Layout,
    OwnershipBehavior,
    AutomaticCleanup,
    AutomaticCleanupMachine,
}

/// One exact declaration whose semantics are carried by a reviewed package's
/// machine without granting that machine authored source authority.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewSemanticDependency {
    consumer: PackageReviewNominalIdentity,
    dependency: PackageReviewNominalIdentity,
    exposure: PackageReviewSemanticDependencyExposure,
    kind: PackageReviewSemanticDependencyKind,
}

impl PackageReviewSemanticDependency {
    pub const fn consumer(&self) -> &PackageReviewNominalIdentity {
        &self.consumer
    }

    pub const fn dependency(&self) -> &PackageReviewNominalIdentity {
        &self.dependency
    }

    pub const fn exposure(&self) -> PackageReviewSemanticDependencyExposure {
        self.exposure
    }

    pub const fn kind(&self) -> PackageReviewSemanticDependencyKind {
        self.kind
    }
}

/// A compiler-owned risk class for authority exposed by a reviewed package.
///
/// The class is attached only after an exact service declaration rejoins its
/// current compiler/toolchain provenance. Package-controlled declaration names
/// never select a class. The containing review projection remains unsealed;
/// admission must still retain the exact toolchain commitment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewDangerousAuthorityClass {
    Filesystem,
    MachineControl,
    PortIo,
    InterruptControl,
    InterruptEntry,
    RootMemory,
    Process,
}

/// One exact reached/invoked service whose compiler-owned metadata marks it as
/// intrinsically dangerous. This is review evidence, not an authority grant.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewDangerousAuthority {
    class: PackageReviewDangerousAuthorityClass,
    service: PackageReviewNominalIdentity,
}

/// A dangerous service present in a checked callable's published ceiling but
/// absent from that callable's checked transitive body reach.
///
/// This is review guidance, not a claim that the declaration is malicious or
/// that bodyless supply failed to realize anything.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewDangerousAuthoritySlack {
    class: PackageReviewDangerousAuthorityClass,
    callable: PackageReviewNominalIdentity,
    service: PackageReviewNominalIdentity,
}

impl PackageReviewDangerousAuthoritySlack {
    pub const fn class(&self) -> PackageReviewDangerousAuthorityClass {
        self.class
    }

    pub const fn callable(&self) -> &PackageReviewNominalIdentity {
        &self.callable
    }

    pub const fn service(&self) -> &PackageReviewNominalIdentity {
        &self.service
    }
}

impl PackageReviewDangerousAuthority {
    pub const fn class(&self) -> PackageReviewDangerousAuthorityClass {
        self.class
    }

    pub const fn service(&self) -> &PackageReviewNominalIdentity {
        &self.service
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewCapabilityFlow {
    capability: PackageReviewNominalIdentity,
    kind: psi_effects::CapabilityFlowKind,
    state: PackageReviewNominalIdentity,
    statement_index: usize,
    call_ordinal: usize,
    via_state: Option<PackageReviewNominalIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewInstallationReach {
    requirement: PackageReviewNominalIdentity,
    upper_bound: Vec<PackageReviewNominalIdentity>,
}

impl PackageReviewInstallationReach {
    pub fn requirement(&self) -> &PackageReviewNominalIdentity {
        &self.requirement
    }

    pub fn upper_bound(&self) -> &[PackageReviewNominalIdentity] {
        &self.upper_bound
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewMutation {
    state: PackageReviewNominalIdentity,
    completeness: psi_facts::WriteFrameCompleteness,
    paths: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewCrashInterface {
    InternalInferred,
    PublishedCeiling,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewCrashPredicate {
    canonical_bytes: Vec<u8>,
}

impl PackageReviewCrashPredicate {
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewCrashRouteGuard {
    Truth,
    Predicate(PackageReviewCrashPredicate),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewCrashRoute {
    cause: psi_checked_trees::CrashCause,
    alternative_guards: Vec<PackageReviewCrashRouteGuard>,
}

impl PackageReviewCrashRoute {
    pub const fn cause(&self) -> psi_checked_trees::CrashCause {
        self.cause
    }

    pub fn alternative_guards(&self) -> &[PackageReviewCrashRouteGuard] {
        &self.alternative_guards
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewPermissionSource {
    StateEntry,
    Statement {
        statement_ordinal: u64,
    },
    Call {
        statement_ordinal: u64,
        call_ordinal: u64,
        target: PackageReviewNominalIdentity,
    },
    StateExit,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewPermissionClaim {
    machine: PackageReviewNominalIdentity,
    state: PackageReviewNominalIdentity,
    source: PackageReviewPermissionSource,
    ordinal: u32,
}

impl PackageReviewPermissionClaim {
    pub fn machine(&self) -> &PackageReviewNominalIdentity {
        &self.machine
    }

    pub fn state(&self) -> &PackageReviewNominalIdentity {
        &self.state
    }

    pub fn source(&self) -> &PackageReviewPermissionSource {
        &self.source
    }

    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewCrashSite {
    state: PackageReviewNominalIdentity,
    statement_ordinal: u32,
    cause: psi_checked_trees::CrashCause,
    path_guard_conjuncts: Vec<PackageReviewCrashPredicate>,
    path_guard_consequences: Vec<PackageReviewCrashPredicate>,
    guard_covering_buckets: Vec<u32>,
    frontier_lower_bound: Vec<PackageReviewPermissionClaim>,
}

impl PackageReviewCrashSite {
    pub fn state(&self) -> &PackageReviewNominalIdentity {
        &self.state
    }

    pub const fn statement_ordinal(&self) -> u32 {
        self.statement_ordinal
    }

    pub const fn cause(&self) -> psi_checked_trees::CrashCause {
        self.cause
    }

    pub fn path_guard_conjuncts(&self) -> &[PackageReviewCrashPredicate] {
        &self.path_guard_conjuncts
    }

    pub fn path_guard_consequences(&self) -> &[PackageReviewCrashPredicate] {
        &self.path_guard_consequences
    }

    pub fn guard_covering_buckets(&self) -> &[u32] {
        &self.guard_covering_buckets
    }

    pub fn frontier_lower_bound(&self) -> &[PackageReviewPermissionClaim] {
        &self.frontier_lower_bound
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewCrashCall {
    state: PackageReviewNominalIdentity,
    statement_ordinal: u32,
    call_ordinal: u32,
    target_machine: PackageReviewNominalIdentity,
    target_state: PackageReviewNominalIdentity,
    path_guard_conjuncts: Vec<PackageReviewCrashPredicate>,
    path_guard_consequences: Vec<PackageReviewCrashPredicate>,
    surviving_buckets: Vec<PackageReviewCrashRoute>,
}

impl PackageReviewCrashCall {
    pub fn state(&self) -> &PackageReviewNominalIdentity {
        &self.state
    }

    pub const fn statement_ordinal(&self) -> u32 {
        self.statement_ordinal
    }

    pub const fn call_ordinal(&self) -> u32 {
        self.call_ordinal
    }

    pub fn target_machine(&self) -> &PackageReviewNominalIdentity {
        &self.target_machine
    }

    pub fn target_state(&self) -> &PackageReviewNominalIdentity {
        &self.target_state
    }

    pub fn path_guard_conjuncts(&self) -> &[PackageReviewCrashPredicate] {
        &self.path_guard_conjuncts
    }

    pub fn path_guard_consequences(&self) -> &[PackageReviewCrashPredicate] {
        &self.path_guard_consequences
    }

    pub fn surviving_buckets(&self) -> &[PackageReviewCrashRoute] {
        &self.surviving_buckets
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewCrash {
    interface: PackageReviewCrashInterface,
    published: Vec<PackageReviewCrashRoute>,
    structural_runtime_requirements: Option<Vec<psi_checked_trees::CheckedBooleanExpression>>,
    checked_sites: Vec<PackageReviewCrashSite>,
    checked_calls: Vec<PackageReviewCrashCall>,
}

impl PackageReviewCrash {
    pub const fn interface(&self) -> PackageReviewCrashInterface {
        self.interface
    }

    pub fn published(&self) -> &[PackageReviewCrashRoute] {
        &self.published
    }

    pub fn structural_runtime_requirements(
        &self,
    ) -> Option<&[psi_checked_trees::CheckedBooleanExpression]> {
        self.structural_runtime_requirements.as_deref()
    }

    pub fn checked_sites(&self) -> &[PackageReviewCrashSite] {
        &self.checked_sites
    }

    pub fn checked_calls(&self) -> &[PackageReviewCrashCall] {
        &self.checked_calls
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewProgressSubject {
    Declaration(PackageReviewNominalIdentity),
    Receiver,
    Parameter(u32),
}

impl PackageReviewProgressSubject {
    pub const fn declaration(&self) -> Option<&PackageReviewNominalIdentity> {
        match self {
            Self::Declaration(identity) => Some(identity),
            Self::Receiver | Self::Parameter(_) => None,
        }
    }

    pub const fn is_receiver(&self) -> bool {
        matches!(self, Self::Receiver)
    }

    pub const fn parameter(&self) -> Option<u32> {
        match self {
            Self::Parameter(position) => Some(*position),
            Self::Declaration(_) | Self::Receiver => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewProgressPremise {
    profile: PackageReviewNominalIdentity,
    subject: PackageReviewProgressSubject,
    projections: Vec<PackageReviewNominalIdentity>,
}

impl PackageReviewProgressPremise {
    pub const fn profile(&self) -> &PackageReviewNominalIdentity {
        &self.profile
    }

    pub const fn subject(&self) -> &PackageReviewProgressSubject {
        &self.subject
    }

    pub fn projections(&self) -> &[PackageReviewNominalIdentity] {
        &self.projections
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageReviewTermination {
    NoGuarantee,
    Terminates {
        premises: Vec<PackageReviewProgressPremise>,
    },
}

impl PackageReviewTermination {
    pub fn premises(&self) -> Option<&[PackageReviewProgressPremise]> {
        match self {
            Self::NoGuarantee => None,
            Self::Terminates { premises } => Some(premises),
        }
    }
}

impl PackageReviewMutation {
    pub fn state(&self) -> &PackageReviewNominalIdentity {
        &self.state
    }

    pub const fn completeness(&self) -> psi_facts::WriteFrameCompleteness {
        self.completeness
    }

    pub fn paths(&self) -> &[String] {
        &self.paths
    }
}

impl PackageReviewCapabilityFlow {
    pub fn capability(&self) -> &PackageReviewNominalIdentity {
        &self.capability
    }

    pub const fn kind(&self) -> psi_effects::CapabilityFlowKind {
        self.kind
    }

    pub fn state(&self) -> &PackageReviewNominalIdentity {
        &self.state
    }

    pub const fn statement_index(&self) -> usize {
        self.statement_index
    }

    pub const fn call_ordinal(&self) -> usize {
        self.call_ordinal
    }

    pub fn via_state(&self) -> Option<&PackageReviewNominalIdentity> {
        self.via_state.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPackageCallableReview {
    role: PackageReviewCallableRole,
    identity: PackageReviewNominalIdentity,
    supply: MachineSupplyMode,
    lifetime_parameter_count: usize,
    type_parameters: Vec<PackageReviewTypeParameter>,
    conformance_bounds: Vec<PackageReviewConformanceBound>,
    parameters: Vec<PackageReviewCallableParameter>,
    return_type: PackageReviewTypeIdentity,
    conformances: Vec<PackageReviewCallableConformance>,
    contracts: Vec<PackageReviewCallableContract>,
    /// `Some` preserves a published ceiling, including an explicitly empty
    /// one. `None` is retained for the current ordinary build-machine form;
    /// admission must not silently reinterpret it as a public empty promise.
    declared_service_reach: Option<Vec<PackageReviewNominalIdentity>>,
    checked_service_reach: PackageReviewCheckedServiceReach,
    unresolved_installation_reaches: Vec<PackageReviewInstallationReach>,
    /// `Some` preserves a published direct synchronous-invocation ceiling,
    /// including an explicitly empty one. Targets retain parameter ordinals
    /// or package-qualified service identities, never display strings.
    declared_synchronous_invocations: Option<Vec<PackageReviewSynchronousInvocation>>,
    realized_synchronous_invocations: Vec<PackageReviewSynchronousInvocation>,
    capability_flows: Vec<PackageReviewCapabilityFlow>,
    checked_may_suspend: bool,
    checked_may_block: bool,
    checked_termination: PackageReviewTermination,
    checked_crash: PackageReviewCrash,
    mutation: Vec<PackageReviewMutation>,
}

/// Whether package review has a checked implementation body from which exact
/// service reach can be reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageReviewCheckedServiceReach {
    NoCheckedBody,
    CheckedBody {
        realized: Vec<PackageReviewNominalIdentity>,
        concrete: Vec<PackageReviewNominalIdentity>,
    },
}

impl PackageReviewCheckedServiceReach {
    pub fn realized(&self) -> Option<&[PackageReviewNominalIdentity]> {
        match self {
            Self::NoCheckedBody => None,
            Self::CheckedBody { realized, .. } => Some(realized),
        }
    }

    pub fn concrete(&self) -> Option<&[PackageReviewNominalIdentity]> {
        match self {
            Self::NoCheckedBody => None,
            Self::CheckedBody { concrete, .. } => Some(concrete),
        }
    }
}

/// One selected provider plan retained for human/LLM review.
///
/// The realizing package is exact and participates in `plan_fingerprint`.
/// That existing 64-bit fingerprint is review/execution compatibility data,
/// not a collision-resistant package-admission identity.
/// Provider type, schema, and requirement labels are paired with exact package
/// owners in the retained schema. A checked-adapter row carries its canonical
/// overload identity and exact machine package owner; compiler consumers have
/// already rejoined both to typed semantics. Provider-selection names and
/// compiler-intrinsic toolchain ownership remain unsealed review data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPackageProviderReview {
    plan_name: String,
    plan_fingerprint: u64,
    realizing_package: Option<PackageKeyIdentity>,
    provider_type: String,
    provider_type_package: Option<PackageKeyIdentity>,
    schema: omega_effects::provider_plan::ServiceSchema,
    target: String,
    rows: Vec<omega_effects::provider_plan::ProviderPlanRow>,
}

impl CheckedPackageProviderReview {
    pub fn plan_name(&self) -> &str {
        &self.plan_name
    }

    pub const fn plan_fingerprint(&self) -> u64 {
        self.plan_fingerprint
    }

    pub const fn realizing_package(&self) -> Option<PackageKeyIdentity> {
        self.realizing_package
    }

    pub fn provider_type(&self) -> &str {
        &self.provider_type
    }

    pub const fn provider_type_package(&self) -> Option<PackageKeyIdentity> {
        self.provider_type_package
    }

    pub fn service_schema(&self) -> &str {
        &self.schema.trait_name
    }

    pub fn schema(&self) -> &omega_effects::provider_plan::ServiceSchema {
        &self.schema
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn rows(&self) -> &[omega_effects::provider_plan::ProviderPlanRow] {
        &self.rows
    }
}

impl CheckedPackageCallableReview {
    pub const fn role(&self) -> PackageReviewCallableRole {
        self.role
    }

    pub fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }

    pub const fn supply(&self) -> MachineSupplyMode {
        self.supply
    }

    pub const fn lifetime_parameter_count(&self) -> usize {
        self.lifetime_parameter_count
    }

    pub fn type_parameters(&self) -> &[PackageReviewTypeParameter] {
        &self.type_parameters
    }

    pub fn conformance_bounds(&self) -> &[PackageReviewConformanceBound] {
        &self.conformance_bounds
    }

    pub fn parameters(&self) -> &[PackageReviewCallableParameter] {
        &self.parameters
    }

    pub const fn return_type(&self) -> &PackageReviewTypeIdentity {
        &self.return_type
    }

    pub fn conformances(&self) -> &[PackageReviewCallableConformance] {
        &self.conformances
    }

    pub fn contracts(&self) -> &[PackageReviewCallableContract] {
        &self.contracts
    }

    pub fn declared_service_reach(&self) -> Option<&[PackageReviewNominalIdentity]> {
        self.declared_service_reach.as_deref()
    }

    pub const fn checked_service_reach(&self) -> &PackageReviewCheckedServiceReach {
        &self.checked_service_reach
    }

    pub fn unresolved_installation_reaches(&self) -> &[PackageReviewInstallationReach] {
        &self.unresolved_installation_reaches
    }

    pub fn declared_synchronous_invocations(
        &self,
    ) -> Option<&[PackageReviewSynchronousInvocation]> {
        self.declared_synchronous_invocations.as_deref()
    }

    pub fn realized_synchronous_invocations(&self) -> &[PackageReviewSynchronousInvocation] {
        &self.realized_synchronous_invocations
    }

    pub fn capability_flows(&self) -> &[PackageReviewCapabilityFlow] {
        &self.capability_flows
    }

    pub const fn checked_may_suspend(&self) -> bool {
        self.checked_may_suspend
    }

    pub const fn checked_may_block(&self) -> bool {
        self.checked_may_block
    }

    pub const fn checked_termination(&self) -> &PackageReviewTermination {
        &self.checked_termination
    }

    pub const fn checked_crash(&self) -> &PackageReviewCrash {
        &self.checked_crash
    }

    pub fn mutation(&self) -> &[PackageReviewMutation] {
        &self.mutation
    }
}

#[derive(Debug, Clone)]
pub struct CheckedPackageReviewProjection {
    package: PackageKeyIdentity,
    target: omega_target::TargetProfile,
    public_traits: Vec<PackageReviewTraitShape>,
    public_domains: Vec<PackageReviewDomainShape>,
    public_data: Vec<PackageReviewDataShape>,
    representation_tcb: Vec<PackageReviewRepresentationTcb>,
    semantic_dependencies: Vec<PackageReviewSemanticDependency>,
    callables: Vec<CheckedPackageCallableReview>,
    dangerous_authorities: Vec<PackageReviewDangerousAuthority>,
    dangerous_authority_slack: Vec<PackageReviewDangerousAuthoritySlack>,
    selected_providers: Vec<CheckedPackageProviderReview>,
    row_sources: PackageReviewCanonicalRowSources,
}

impl PartialEq for CheckedPackageReviewProjection {
    fn eq(&self, other: &Self) -> bool {
        self.package == other.package
            && self.target == other.target
            && self.public_traits == other.public_traits
            && self.public_domains == other.public_domains
            && self.public_data == other.public_data
            && self.representation_tcb == other.representation_tcb
            && self.semantic_dependencies == other.semantic_dependencies
            && self.callables == other.callables
            && self.dangerous_authorities == other.dangerous_authorities
            && self.dangerous_authority_slack == other.dangerous_authority_slack
            && self.selected_providers == other.selected_providers
    }
}

impl Eq for CheckedPackageReviewProjection {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageReviewCanonicalRowSources {
    public_traits: Vec<PackageReviewCanonicalRowSource>,
    public_domains: Vec<PackageReviewCanonicalRowSource>,
    public_data: Vec<PackageReviewCanonicalRowSource>,
    representation_tcb: Vec<PackageReviewCanonicalRowSource>,
    semantic_dependencies: Vec<PackageReviewCanonicalRowSource>,
    callables: Vec<PackageReviewCanonicalRowSource>,
    dangerous_authorities: Vec<PackageReviewCanonicalRowSource>,
    dangerous_authority_slack: Vec<PackageReviewCanonicalRowSource>,
    selected_provider_set: PackageReviewCanonicalRowSource,
}

/// Compiler-internal pairing between one semantic review row and the exact
/// declaration that produced it. Canonical sorting must move both together;
/// source projection may never rediscover the declaration from reduced row
/// identity.
#[derive(Debug, Clone)]
struct ProjectedReviewRow<Row> {
    row: Row,
    declaration: SymbolHandle,
}

#[derive(Debug, Clone)]
struct ProjectedDangerousAuthorityRow {
    row: PackageReviewDangerousAuthority,
    declaration: SymbolHandle,
    exposures: Vec<SymbolHandle>,
}

#[derive(Debug, Clone)]
struct ProjectedDangerousAuthoritySlackRow {
    row: PackageReviewDangerousAuthoritySlack,
    authority_declaration: SymbolHandle,
    callable_declaration: SymbolHandle,
}

#[derive(Debug, Clone)]
struct ProjectedSemanticDependencyRow {
    row: PackageReviewSemanticDependency,
    consumer_declarations: Vec<SymbolHandle>,
    dependency_declarations: Vec<SymbolHandle>,
}

/// Compiler-owned granularity for review-only capability/API comparison.
///
/// Callable rows currently retain the complete callable envelope. Nested
/// contract/reach/flow decomposition can refine that lane without requiring
/// package orchestration to parse compiler encoding bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PackageReviewCanonicalRowKind {
    ProjectionHeader,
    PublicTrait,
    PublicDomain,
    PublicData,
    RepresentationTcb,
    Callable,
    DangerousAuthority,
    SelectedProviderSet,
    /// A trust-bearing bodyless boundary guarantee. This is separate from the
    /// callable API row so admission policy cannot mistake an accepted claim
    /// for checked implementation evidence.
    AcceptedClaim,
    /// A compiler-classified dangerous service is declared by a checked body
    /// but absent from its exact inferred transitive reach.
    DangerousAuthoritySlack,
    SemanticDependency,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PackageReviewCanonicalRowRisk {
    Blocking,
    AuditRecommended,
    OpaqueBlocking,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PackageReviewSourceLocationRole {
    Declaration,
    DerivationOrigin,
    AuthorityDeclaration,
    AuthorityExposure,
    ProviderSelection,
    ProviderSchemaDeclaration,
    ProviderTypeDeclaration,
    ProviderRealization,
    SemanticDependencyConsumer,
    SemanticDependencyDeclaration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PackageReviewSyntheticSourceKind {
    ProjectionHeader,
    EmptySelectedProviderSet,
    UniqueCoveringProviderSelection,
    FreeExternalProviderType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PackageReviewSourceLocationOwner {
    Package(PackageKeyIdentity),
    Toolchain(PackageReviewToolchainSourceIdentity),
}

/// Compiler-validated package-relative source coordinate used only to explain
/// a canonical review row. Absolute resolver/cache paths never enter it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageReviewSourceLocation {
    owner: PackageReviewSourceLocationOwner,
    relative_path: String,
    start_byte: u64,
    end_byte: u64,
    role: PackageReviewSourceLocationRole,
}

impl PackageReviewSourceLocation {
    pub const fn owner(&self) -> PackageReviewSourceLocationOwner {
        self.owner
    }

    pub fn relative_path(&self) -> &str {
        &self.relative_path
    }

    pub const fn start_byte(&self) -> u64 {
        self.start_byte
    }

    pub const fn end_byte(&self) -> u64 {
        self.end_byte
    }

    pub const fn role(&self) -> PackageReviewSourceLocationRole {
        self.role
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewCanonicalRowSource {
    authored_locations: Vec<PackageReviewSourceLocation>,
    compiler_derivations: Vec<PackageReviewSyntheticSourceKind>,
}

impl PackageReviewCanonicalRowSource {
    fn authored(authored_locations: Vec<PackageReviewSourceLocation>) -> Self {
        Self {
            authored_locations,
            compiler_derivations: Vec::new(),
        }
    }

    fn compiler_derived(compiler_derivation: PackageReviewSyntheticSourceKind) -> Self {
        Self {
            authored_locations: Vec::new(),
            compiler_derivations: vec![compiler_derivation],
        }
    }

    fn mixed(
        authored_locations: Vec<PackageReviewSourceLocation>,
        compiler_derivations: Vec<PackageReviewSyntheticSourceKind>,
    ) -> Self {
        Self {
            authored_locations,
            compiler_derivations,
        }
    }

    pub fn authored_locations(&self) -> Option<&[PackageReviewSourceLocation]> {
        (!self.authored_locations.is_empty()).then_some(&self.authored_locations)
    }

    pub fn compiler_derivations(&self) -> &[PackageReviewSyntheticSourceKind] {
        &self.compiler_derivations
    }
}

/// One independently framed canonical row issued by the compiler.
///
/// The key is used only to match one row family across two projections. The
/// complete bytes bind schema, package, target, kind, key, and value. Neither
/// byte sequence is a package certificate or accepted lock artifact.
#[derive(Debug, Clone)]
pub struct PackageReviewCanonicalRow {
    kind: PackageReviewCanonicalRowKind,
    risk: PackageReviewCanonicalRowRisk,
    key_bytes: Vec<u8>,
    canonical_bytes: Vec<u8>,
    source: PackageReviewCanonicalRowSource,
}

impl PartialEq for PackageReviewCanonicalRow {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.risk == other.risk
            && self.key_bytes == other.key_bytes
            && self.canonical_bytes == other.canonical_bytes
    }
}

impl Eq for PackageReviewCanonicalRow {}

impl PackageReviewCanonicalRow {
    pub const fn kind(&self) -> PackageReviewCanonicalRowKind {
        self.kind
    }

    pub const fn risk(&self) -> PackageReviewCanonicalRowRisk {
        self.risk
    }

    pub fn key_bytes(&self) -> &[u8] {
        &self.key_bytes
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn source(&self) -> &PackageReviewCanonicalRowSource {
        &self.source
    }
}

impl CheckedPackageReviewProjection {
    pub const fn package(&self) -> PackageKeyIdentity {
        self.package
    }

    pub const fn target(&self) -> omega_target::TargetProfile {
        self.target
    }

    pub fn public_traits(&self) -> &[PackageReviewTraitShape] {
        &self.public_traits
    }

    pub fn public_domains(&self) -> &[PackageReviewDomainShape] {
        &self.public_domains
    }

    pub fn public_data(&self) -> &[PackageReviewDataShape] {
        &self.public_data
    }

    pub fn representation_tcb(&self) -> &[PackageReviewRepresentationTcb] {
        &self.representation_tcb
    }

    pub fn semantic_dependencies(&self) -> &[PackageReviewSemanticDependency] {
        &self.semantic_dependencies
    }

    pub fn callables(&self) -> &[CheckedPackageCallableReview] {
        &self.callables
    }

    pub fn dangerous_authorities(&self) -> &[PackageReviewDangerousAuthority] {
        &self.dangerous_authorities
    }

    pub fn dangerous_authority_slack(&self) -> &[PackageReviewDangerousAuthoritySlack] {
        &self.dangerous_authority_slack
    }

    pub fn selected_providers(&self) -> &[CheckedPackageProviderReview] {
        &self.selected_providers
    }

    /// Versioned, source-handle-free comparison bytes for this review-only
    /// projection. These bytes are not a package certificate and must not be
    /// persisted as accepted evidence without the source/toolchain/compiler
    /// binding and remaining required admission-projection joins. Terminal
    /// evidence is separately required only for final-realization claims.
    pub fn canonical_review_bytes(&self) -> Result<Vec<u8>, PackageReviewEncodingError> {
        encoding::encode(self)
    }

    /// Independently framed rows for review-only conflict explanation.
    /// Package orchestration compares these bytes but never parses or
    /// reconstructs compiler semantic rows itself.
    pub fn canonical_rows(
        &self,
    ) -> Result<Vec<PackageReviewCanonicalRow>, PackageReviewEncodingError> {
        encoding::encode_rows(self)
    }
}

/// Project the exact checked authority facts that are already safely joined.
///
/// This refuses standalone and target-free compilations, missing checked fact
/// rows, and a non-root build machine. Compiler-generated nominals inherit the
/// exact authored source provenance of their mandatory derivation origin.
/// Truly source-free nominals remain explicit `Unresolved` review rows; a later
/// admission certificate must reject them rather than treating them as empty
/// authority.
pub fn project_checked_package_review(
    compilation: &CheckedCompilation,
) -> Result<CheckedPackageReviewProjection, Vec<Diagnostic>> {
    let package = compilation.package_identity().ok_or_else(|| {
        vec![Diagnostic::error(
            "package review requires package-aware checked compilation",
        )]
    })?;
    let target = compilation.selected_target_profile().ok_or_else(|| {
        vec![Diagnostic::error(
            "package review requires one explicit target selection",
        )]
    })?;
    if !compilation.contract_entailment_stand_downs().is_empty() {
        return Err(compilation
            .contract_entailment_stand_downs()
            .iter()
            .map(|stand_down| {
                Diagnostic::error(format!(
                    "package review rejects unresolved contract-entailment stand-down at machine symbol {}, contract {}, fact {}: {}",
                    stand_down.machine_symbol.arena_index(),
                    stand_down.contract_index,
                    stand_down.fact_index,
                    stand_down.reason.label(),
                ))
            })
            .collect());
    }
    let build_machine = compilation.selected_build_machine_symbol();
    let public_traits = project_public_traits(compilation, package)?;
    let public_domains = project_public_domains(compilation, package)?;
    let public_data = project_public_data(compilation, package)?;
    let representation_tcb = project_representation_tcb(compilation, package)?;
    let semantic_dependencies = project_semantic_dependencies(compilation, package)?;
    let synchronous_invocations = psi_effects::infer_synchronous_invocations(&compilation.typed);
    let mut callables = Vec::new();
    let mut projected_build_machine = false;

    for machine in compilation.machines() {
        let role = if Some(machine.symbol) == build_machine {
            Some(PackageReviewCallableRole::Build)
        } else if machine.supply_mode.is_boundary_declaration() {
            Some(PackageReviewCallableRole::Boundary)
        } else if machine.is_public {
            Some(PackageReviewCallableRole::Public)
        } else {
            None
        };
        let Some(role) = role else {
            continue;
        };
        let owner = nominal_identity(compilation, machine.symbol)?;
        match owner.owner {
            PackageReviewNominalOwner::Package(owner) if owner == package => {}
            PackageReviewNominalOwner::Package(_)
            | PackageReviewNominalOwner::ToolchainSource(_)
            | PackageReviewNominalOwner::ToolchainUnbound => {
                continue;
            }
            PackageReviewNominalOwner::Unresolved => {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` has no managed package owner",
                    owner.path
                ))]);
            }
        }

        callables.push(ProjectedReviewRow {
            row: project_callable(compilation, &synchronous_invocations, machine, role, owner)?,
            declaration: machine.symbol,
        });
        projected_build_machine |= role == PackageReviewCallableRole::Build;
    }

    if build_machine.is_some() && !projected_build_machine {
        return Err(vec![Diagnostic::error(
            "selected build machine is not owned by the reviewed root package",
        )]);
    }

    callables.sort_by(|left, right| {
        left.row
            .identity
            .cmp(&right.row.identity)
            .then(left.row.role.cmp(&right.row.role))
            .then(left.row.contracts.cmp(&right.row.contracts))
    });
    let dangerous_authorities = project_dangerous_authorities(compilation, &callables)?;
    let dangerous_authority_slack = project_dangerous_authority_slack(compilation, &callables)?;
    let selected_providers: Vec<_> = compilation
        .selected_provider_plans()
        .plans()
        .iter()
        .map(|plan| CheckedPackageProviderReview {
            plan_name: plan.name.clone(),
            plan_fingerprint: plan.identity_fingerprint(),
            realizing_package: plan.origin_package_identity,
            provider_type: plan.provider_type.clone(),
            provider_type_package: plan.provider_type_package_identity,
            schema: plan.schema.clone(),
            target: plan.target.clone(),
            rows: plan.rows.clone(),
        })
        .collect();
    let (public_traits, public_trait_sources) = finalize_projected_rows(
        compilation,
        public_traits,
        PackageReviewSourceLocationRole::Declaration,
    )?;
    let (public_domains, public_domain_sources) = finalize_projected_rows(
        compilation,
        public_domains,
        PackageReviewSourceLocationRole::Declaration,
    )?;
    let (public_data, public_data_sources) = finalize_projected_rows(
        compilation,
        public_data,
        PackageReviewSourceLocationRole::Declaration,
    )?;
    let (representation_tcb, representation_tcb_sources) = finalize_projected_rows(
        compilation,
        representation_tcb,
        PackageReviewSourceLocationRole::Declaration,
    )?;
    let (semantic_dependencies, semantic_dependency_sources) =
        finalize_semantic_dependency_rows(compilation, semantic_dependencies)?;
    let (callables, callable_sources) = finalize_projected_rows(
        compilation,
        callables,
        PackageReviewSourceLocationRole::Declaration,
    )?;
    let (dangerous_authorities, dangerous_authority_sources) =
        finalize_dangerous_authority_rows(compilation, dangerous_authorities)?;
    let (dangerous_authority_slack, dangerous_authority_slack_sources) =
        finalize_dangerous_authority_slack_rows(compilation, dangerous_authority_slack)?;
    let row_sources = PackageReviewCanonicalRowSources {
        public_traits: public_trait_sources,
        public_domains: public_domain_sources,
        public_data: public_data_sources,
        representation_tcb: representation_tcb_sources,
        semantic_dependencies: semantic_dependency_sources,
        callables: callable_sources,
        dangerous_authorities: dangerous_authority_sources,
        dangerous_authority_slack: dangerous_authority_slack_sources,
        selected_provider_set: selected_provider_row_source(compilation, &selected_providers)?,
    };
    validate_canonical_row_source_limits(&row_sources)?;

    Ok(CheckedPackageReviewProjection {
        package,
        target,
        public_traits,
        public_domains,
        public_data,
        representation_tcb,
        semantic_dependencies,
        callables,
        dangerous_authorities,
        dangerous_authority_slack,
        selected_providers,
        row_sources,
    })
}

fn project_semantic_dependencies(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedSemanticDependencyRow>, Vec<Diagnostic>> {
    let mut projected: Vec<ProjectedSemanticDependencyRow> = Vec::new();
    for checked in &compilation.facts.flow.semantic_dependencies.rows {
        let consumer = nominal_identity(compilation, checked.consumer_machine)?;
        if !reviewed_package_owns(&consumer, package)? {
            continue;
        }
        let row = PackageReviewSemanticDependency {
            consumer,
            dependency: nominal_identity(compilation, checked.dependency)?,
            exposure: match checked.exposure {
                psi_checked_trees::CheckedSemanticDependencyExposure::PrivateImplementation => {
                    PackageReviewSemanticDependencyExposure::PrivateImplementation
                }
                psi_checked_trees::CheckedSemanticDependencyExposure::PublicInterface => {
                    PackageReviewSemanticDependencyExposure::PublicInterface
                }
            },
            kind: match checked.kind {
                psi_checked_trees::CheckedSemanticDependencyKind::NominalIdentity => {
                    PackageReviewSemanticDependencyKind::NominalIdentity
                }
                psi_checked_trees::CheckedSemanticDependencyKind::Layout => {
                    PackageReviewSemanticDependencyKind::Layout
                }
                psi_checked_trees::CheckedSemanticDependencyKind::OwnershipBehavior => {
                    PackageReviewSemanticDependencyKind::OwnershipBehavior
                }
                psi_checked_trees::CheckedSemanticDependencyKind::AutomaticCleanup => {
                    PackageReviewSemanticDependencyKind::AutomaticCleanup
                }
                psi_checked_trees::CheckedSemanticDependencyKind::AutomaticCleanupMachine => {
                    PackageReviewSemanticDependencyKind::AutomaticCleanupMachine
                }
            },
        };
        if let Some(existing) = projected.iter_mut().find(|existing| existing.row == row) {
            if !existing
                .consumer_declarations
                .contains(&checked.consumer_machine)
            {
                existing
                    .consumer_declarations
                    .push(checked.consumer_machine);
            }
            if !existing
                .dependency_declarations
                .contains(&checked.dependency)
            {
                existing.dependency_declarations.push(checked.dependency);
            }
        } else {
            projected.push(ProjectedSemanticDependencyRow {
                row,
                consumer_declarations: vec![checked.consumer_machine],
                dependency_declarations: vec![checked.dependency],
            });
        }
    }
    projected.sort_by(|left, right| left.row.cmp(&right.row));
    for row in &mut projected {
        row.consumer_declarations
            .sort_by_key(|symbol| symbol.arena_index());
        row.dependency_declarations
            .sort_by_key(|symbol| symbol.arena_index());
    }
    Ok(projected)
}

fn project_representation_tcb(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedReviewRow<PackageReviewRepresentationTcb>>, Vec<Diagnostic>> {
    let mut rows = Vec::new();
    for definition in compilation.data_definitions().iter().filter(|definition| {
        definition.supply_mode == psi_language_semantics::DataSupplyMode::BoundaryOpaque
    }) {
        let declaration = nominal_identity(compilation, definition.symbol)?;
        if !reviewed_package_owns(&declaration, package)? {
            continue;
        }
        rows.push(ProjectedReviewRow {
            row: PackageReviewRepresentationTcb {
                declaration,
                abi: PackageReviewRepresentationAbiCommitment::Unbound,
                mechanism: PackageReviewRepresentationMechanism::Unbound,
            },
            declaration: definition.symbol,
        });
    }
    rows.sort_by(|left, right| left.row.cmp(&right.row));
    rows.dedup_by(|left, right| left.row == right.row && left.declaration == right.declaration);
    Ok(rows)
}

fn project_dangerous_authorities(
    compilation: &CheckedCompilation,
    callables: &[ProjectedReviewRow<CheckedPackageCallableReview>],
) -> Result<Vec<ProjectedDangerousAuthorityRow>, Vec<Diagnostic>> {
    let mut exposed_services = BTreeSet::new();
    for callable in callables.iter().map(|projected| &projected.row) {
        if let Some(services) = callable.declared_service_reach() {
            exposed_services.extend(services.iter().cloned());
        }
        if let Some(services) = callable.checked_service_reach().realized() {
            exposed_services.extend(services.iter().cloned());
        }
        if let Some(services) = callable.checked_service_reach().concrete() {
            exposed_services.extend(services.iter().cloned());
        }
        for reach in callable.unresolved_installation_reaches() {
            exposed_services.extend(reach.upper_bound().iter().cloned());
        }
        if let Some(invocations) = callable.declared_synchronous_invocations() {
            exposed_services.extend(
                invocations
                    .iter()
                    .filter_map(PackageReviewSynchronousInvocation::service)
                    .cloned(),
            );
        }
        exposed_services.extend(
            callable
                .realized_synchronous_invocations()
                .iter()
                .filter_map(PackageReviewSynchronousInvocation::service)
                .cloned(),
        );
    }

    let mut rows = Vec::new();
    for definition in compilation.facts.service_reaches.services.definitions() {
        let service = nominal_identity(compilation, definition.symbol)?;
        if !exposed_services.contains(&service) {
            continue;
        }
        let Some(class) = dangerous_authority_class(compilation, definition) else {
            continue;
        };
        let exposures = callables
            .iter()
            .filter(|callable| callable_exposes_service(&callable.row, &service))
            .map(|callable| callable.declaration)
            .collect();
        rows.push(ProjectedDangerousAuthorityRow {
            row: PackageReviewDangerousAuthority { class, service },
            declaration: definition.symbol,
            exposures,
        });
    }
    rows.sort_by(|left, right| left.row.cmp(&right.row));
    rows.dedup_by(|left, right| {
        left.row == right.row
            && left.declaration == right.declaration
            && left.exposures == right.exposures
    });
    Ok(rows)
}

fn project_dangerous_authority_slack(
    compilation: &CheckedCompilation,
    callables: &[ProjectedReviewRow<CheckedPackageCallableReview>],
) -> Result<Vec<ProjectedDangerousAuthoritySlackRow>, Vec<Diagnostic>> {
    let mut catalog = Vec::new();
    for definition in compilation.facts.service_reaches.services.definitions() {
        let Some(class) = dangerous_authority_class(compilation, definition) else {
            continue;
        };
        catalog.push((
            nominal_identity(compilation, definition.symbol)?,
            class,
            definition.symbol,
        ));
    }
    catalog.sort_by(|left, right| left.0.cmp(&right.0));

    let mut rows = Vec::new();
    for callable in callables {
        let Some(realized) = callable.row.checked_service_reach().realized() else {
            continue;
        };
        let Some(declared) = callable.row.declared_service_reach() else {
            continue;
        };
        for service in declared {
            if realized.contains(service) {
                continue;
            }
            let Ok(index) = catalog.binary_search_by(|entry| entry.0.cmp(service)) else {
                continue;
            };
            let (_, class, authority_declaration) = &catalog[index];
            rows.push(ProjectedDangerousAuthoritySlackRow {
                row: PackageReviewDangerousAuthoritySlack {
                    class: *class,
                    callable: callable.row.identity.clone(),
                    service: service.clone(),
                },
                authority_declaration: *authority_declaration,
                callable_declaration: callable.declaration,
            });
        }
    }
    rows.sort_by(|left, right| left.row.cmp(&right.row));
    rows.dedup_by(|left, right| {
        left.row == right.row
            && left.authority_declaration == right.authority_declaration
            && left.callable_declaration == right.callable_declaration
    });
    Ok(rows)
}

fn finalize_projected_rows<Row>(
    compilation: &CheckedCompilation,
    projected: Vec<ProjectedReviewRow<Row>>,
    role: PackageReviewSourceLocationRole,
) -> Result<(Vec<Row>, Vec<PackageReviewCanonicalRowSource>), Vec<Diagnostic>> {
    let mut rows = Vec::with_capacity(projected.len());
    let mut sources = Vec::with_capacity(projected.len());
    for projected in projected {
        sources.push(PackageReviewCanonicalRowSource::authored(vec![
            canonical_source_location(compilation, projected.declaration, role)?,
        ]));
        rows.push(projected.row);
    }
    Ok((rows, sources))
}

fn finalize_semantic_dependency_rows(
    compilation: &CheckedCompilation,
    projected: Vec<ProjectedSemanticDependencyRow>,
) -> Result<
    (
        Vec<PackageReviewSemanticDependency>,
        Vec<PackageReviewCanonicalRowSource>,
    ),
    Vec<Diagnostic>,
> {
    let mut rows = Vec::with_capacity(projected.len());
    let mut sources = Vec::with_capacity(projected.len());
    for projected in projected {
        let mut locations = Vec::new();
        for declaration in projected.consumer_declarations {
            locations.push(canonical_source_location(
                compilation,
                declaration,
                PackageReviewSourceLocationRole::SemanticDependencyConsumer,
            )?);
        }
        for declaration in projected.dependency_declarations {
            locations.push(canonical_source_location(
                compilation,
                declaration,
                PackageReviewSourceLocationRole::SemanticDependencyDeclaration,
            )?);
        }
        locations.sort();
        locations.dedup();
        sources.push(PackageReviewCanonicalRowSource::authored(locations));
        rows.push(projected.row);
    }
    Ok((rows, sources))
}

fn finalize_dangerous_authority_rows(
    compilation: &CheckedCompilation,
    projected: Vec<ProjectedDangerousAuthorityRow>,
) -> Result<
    (
        Vec<PackageReviewDangerousAuthority>,
        Vec<PackageReviewCanonicalRowSource>,
    ),
    Vec<Diagnostic>,
> {
    let mut rows = Vec::with_capacity(projected.len());
    let mut sources = Vec::with_capacity(projected.len());
    for projected in projected {
        let mut locations = vec![canonical_source_location(
            compilation,
            projected.declaration,
            PackageReviewSourceLocationRole::AuthorityDeclaration,
        )?];
        for exposure in projected.exposures {
            locations.push(canonical_source_location(
                compilation,
                exposure,
                PackageReviewSourceLocationRole::AuthorityExposure,
            )?);
        }
        locations.sort();
        locations.dedup();
        sources.push(PackageReviewCanonicalRowSource::authored(locations));
        rows.push(projected.row);
    }
    Ok((rows, sources))
}

fn finalize_dangerous_authority_slack_rows(
    compilation: &CheckedCompilation,
    projected: Vec<ProjectedDangerousAuthoritySlackRow>,
) -> Result<
    (
        Vec<PackageReviewDangerousAuthoritySlack>,
        Vec<PackageReviewCanonicalRowSource>,
    ),
    Vec<Diagnostic>,
> {
    let mut rows = Vec::with_capacity(projected.len());
    let mut sources = Vec::with_capacity(projected.len());
    for projected in projected {
        let mut locations = vec![
            canonical_source_location(
                compilation,
                projected.authority_declaration,
                PackageReviewSourceLocationRole::AuthorityDeclaration,
            )?,
            canonical_source_location(
                compilation,
                projected.callable_declaration,
                PackageReviewSourceLocationRole::AuthorityExposure,
            )?,
        ];
        locations.sort();
        locations.dedup();
        sources.push(PackageReviewCanonicalRowSource::authored(locations));
        rows.push(projected.row);
    }
    Ok((rows, sources))
}

fn selected_provider_row_source(
    compilation: &CheckedCompilation,
    selected_providers: &[CheckedPackageProviderReview],
) -> Result<PackageReviewCanonicalRowSource, Vec<Diagnostic>> {
    let selected_plans = compilation.selected_provider_plans().plans();
    let provenance = compilation.selected_provider_provenance();
    if selected_plans.len() != selected_providers.len() || selected_plans.len() != provenance.len()
    {
        return Err(vec![Diagnostic::error(
            "selected-provider review provenance is not aligned with the canonical selected plan set",
        )]);
    }
    if selected_plans.is_empty() {
        return Ok(PackageReviewCanonicalRowSource::compiler_derived(
            PackageReviewSyntheticSourceKind::EmptySelectedProviderSet,
        ));
    }

    let mut locations = Vec::new();
    let mut compiler_derivations = Vec::new();
    for (index, plan) in selected_plans.iter().enumerate() {
        let retained = &provenance[index];
        if retained.plan != *plan {
            return Err(vec![Diagnostic::error(format!(
                "selected provider plan `{}` is not aligned with its retained provenance",
                plan.name,
            ))]);
        }

        match &retained.selected_by {
            super::provider_plans::ProviderSelectionProvenance::BuildOverride(declarations)
            | super::provider_plans::ProviderSelectionProvenance::TargetDefault(declarations) => {
                for declaration in declarations {
                    locations.push(canonical_source_span_location(
                        compilation,
                        declaration.source_span,
                        PackageReviewSourceLocationRole::ProviderSelection,
                    )?);
                }
            }
            super::provider_plans::ProviderSelectionProvenance::UniqueCoveringCandidate => {
                compiler_derivations
                    .push(PackageReviewSyntheticSourceKind::UniqueCoveringProviderSelection);
            }
        }

        locations.push(canonical_source_location(
            compilation,
            retained.provider.schema.symbol(),
            PackageReviewSourceLocationRole::ProviderSchemaDeclaration,
        )?);

        if let Some(provider_type) = retained.provider.provider_type {
            locations.push(canonical_source_location(
                compilation,
                provider_type,
                PackageReviewSourceLocationRole::ProviderTypeDeclaration,
            )?);
        } else {
            compiler_derivations.push(PackageReviewSyntheticSourceKind::FreeExternalProviderType);
        }

        for realization in &retained.provider.row_realizations {
            locations.push(canonical_source_location(
                compilation,
                *realization,
                PackageReviewSourceLocationRole::ProviderRealization,
            )?);
        }
    }
    locations.sort();
    locations.dedup();
    compiler_derivations.sort();
    compiler_derivations.dedup();
    Ok(PackageReviewCanonicalRowSource::mixed(
        locations,
        compiler_derivations,
    ))
}

const MAX_PACKAGE_REVIEW_SOURCE_LOCATIONS: usize = 262_144;
const MAX_PACKAGE_REVIEW_SOURCE_LOCATION_PATH_BYTES: usize = 16 * 1024 * 1024;

fn validate_canonical_row_source_limits(
    sources: &PackageReviewCanonicalRowSources,
) -> Result<(), Vec<Diagnostic>> {
    let all = sources
        .public_traits
        .iter()
        .chain(&sources.public_domains)
        .chain(&sources.public_data)
        .chain(&sources.representation_tcb)
        .chain(&sources.semantic_dependencies)
        .chain(&sources.callables)
        .chain(&sources.dangerous_authorities)
        .chain(&sources.dangerous_authority_slack)
        .chain(std::iter::once(&sources.selected_provider_set));
    let mut count = 0usize;
    let mut path_bytes = 0usize;
    for source in all {
        let locations = &source.authored_locations;
        let derivations = &source.compiler_derivations;
        if locations.is_empty() && derivations.is_empty() {
            return Err(vec![Diagnostic::error(
                "package review row has neither authored source locations nor a compiler derivation",
            )]);
        }
        if locations.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(vec![Diagnostic::error(
                "authored package review source locations are not strictly canonical",
            )]);
        }
        if derivations.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(vec![Diagnostic::error(
                "package review compiler derivations are not strictly canonical",
            )]);
        }
        count = count.checked_add(locations.len()).ok_or_else(|| {
            vec![Diagnostic::error(
                "package review source-location count overflow",
            )]
        })?;
        for location in locations {
            path_bytes = path_bytes
                .checked_add(location.relative_path.len())
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "package review source-location path-byte count overflow",
                    )]
                })?;
        }
    }
    if count > MAX_PACKAGE_REVIEW_SOURCE_LOCATIONS {
        return Err(vec![Diagnostic::error(
            "package review exceeds the source-location count ceiling",
        )]);
    }
    if path_bytes > MAX_PACKAGE_REVIEW_SOURCE_LOCATION_PATH_BYTES {
        return Err(vec![Diagnostic::error(
            "package review exceeds the source-location path-byte ceiling",
        )]);
    }
    Ok(())
}

fn canonical_source_location(
    compilation: &CheckedCompilation,
    symbol: SymbolHandle,
    mut role: PackageReviewSourceLocationRole,
) -> Result<PackageReviewSourceLocation, Vec<Diagnostic>> {
    if compilation
        .typed
        .symbols
        .symbol_source_span(symbol)
        .is_none()
    {
        role = PackageReviewSourceLocationRole::DerivationOrigin;
    }
    let span = compilation
        .typed
        .symbols
        .symbol_provenance_source_span(symbol)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "reviewed declaration `{}` has no authored source span",
                compilation.typed.symbols.display_path(symbol, "::")
            ))]
        })?;
    canonical_source_span_location(compilation, span, role)
}

fn canonical_source_span_location(
    compilation: &CheckedCompilation,
    span: psi_source::SourceSpan,
    role: PackageReviewSourceLocationRole,
) -> Result<PackageReviewSourceLocation, Vec<Diagnostic>> {
    let source_file = compilation.typed.symbols.source_file(span).ok_or_else(|| {
        vec![Diagnostic::error(
            "reviewed declaration source span has no retained source file",
        )]
    })?;
    if span.span.start >= span.span.end
        || span.span.end > source_file.source.len()
        || !source_file.source.is_char_boundary(span.span.start)
        || !source_file.source.is_char_boundary(span.span.end)
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed declaration source span is outside `{}`",
            source_file.path.display()
        ))]);
    }
    let owner = match source_file.origin {
        psi_source::SourceOrigin::User => PackageReviewSourceLocationOwner::Package(
            source_file.package_identity.ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "reviewed package source `{}` has no reconciled package identity",
                    source_file.path.display()
                ))]
            })?,
        ),
        psi_source::SourceOrigin::Toolchain => {
            PackageReviewSourceLocationOwner::Toolchain(toolchain_source_identity(source_file)?)
        }
    };
    let relative_path = canonical_review_relative_path(source_file)?;
    Ok(PackageReviewSourceLocation {
        owner,
        relative_path,
        start_byte: u64::try_from(span.span.start).expect("retained source byte offset fits u64"),
        end_byte: u64::try_from(span.span.end).expect("retained source byte offset fits u64"),
        role,
    })
}

fn canonical_review_relative_path(
    source_file: &psi_source::SourceFile,
) -> Result<String, Vec<Diagnostic>> {
    let relative = match source_file.path.strip_prefix(&source_file.package_root) {
        Ok(relative) => relative,
        Err(_)
            if source_file.origin == psi_source::SourceOrigin::Toolchain
                && is_canonical_virtual_toolchain_path(&source_file.path) =>
        {
            source_file.path.as_path()
        }
        Err(_) => {
            return Err(vec![Diagnostic::error(format!(
                "reviewed source `{}` is outside its retained package root `{}`",
                source_file.path.display(),
                source_file.package_root.display()
            ))]);
        }
    };
    let mut path = String::new();
    for component in relative.components() {
        let std::path::Component::Normal(component) = component else {
            return Err(vec![Diagnostic::error(format!(
                "reviewed source `{}` has a non-canonical relative path",
                source_file.path.display()
            ))]);
        };
        let component = component.to_str().ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "reviewed source `{}` has a non-UTF-8 relative path component",
                source_file.path.display()
            ))]
        })?;
        if !path.is_empty() {
            path.push('/');
        }
        path.push_str(component);
    }
    if path.is_empty() {
        return Err(vec![Diagnostic::error(
            "reviewed source location requires a non-empty relative path",
        )]);
    }
    Ok(path)
}

fn callable_exposes_service(
    callable: &CheckedPackageCallableReview,
    service: &PackageReviewNominalIdentity,
) -> bool {
    callable
        .declared_service_reach()
        .is_some_and(|services| services.contains(service))
        || callable
            .checked_service_reach()
            .realized()
            .is_some_and(|services| services.contains(service))
        || callable
            .checked_service_reach()
            .concrete()
            .is_some_and(|services| services.contains(service))
        || callable
            .unresolved_installation_reaches()
            .iter()
            .any(|reach| reach.upper_bound().contains(service))
        || callable
            .declared_synchronous_invocations()
            .is_some_and(|invocations| {
                invocations
                    .iter()
                    .any(|invocation| invocation.service() == Some(service))
            })
        || callable
            .realized_synchronous_invocations()
            .iter()
            .any(|invocation| invocation.service() == Some(service))
}

/// Compiler-owned intrinsic metadata for the standard authority catalog.
/// Both the declaration path and immutable toolchain source coordinate must
/// match. A package-authored lookalike therefore cannot acquire or suppress a
/// risk class by choosing a declaration name.
fn dangerous_authority_class(
    compilation: &CheckedCompilation,
    definition: &psi_language_semantics::ServiceReachDefinition,
) -> Option<PackageReviewDangerousAuthorityClass> {
    let source_file = compilation
        .typed
        .symbols
        .symbol_source_span(definition.symbol)
        .and_then(|span| compilation.typed.symbols.source_file(span))?;
    if source_file.origin != psi_source::SourceOrigin::Toolchain {
        return None;
    }
    let relative_source = source_file
        .path
        .strip_prefix(&source_file.package_root)
        .ok()?;
    match (
        relative_source,
        compilation
            .typed
            .symbols
            .display_path(definition.symbol, "::")
            .as_str(),
    ) {
        (path, "FilesystemHost") if path == std::path::Path::new("filesystem_host.omg") => {
            Some(PackageReviewDangerousAuthorityClass::Filesystem)
        }
        (path, "MachineControl") if path == std::path::Path::new("assembly.omg") => {
            Some(PackageReviewDangerousAuthorityClass::MachineControl)
        }
        (path, "PortIo") if path == std::path::Path::new("assembly.omg") => {
            Some(PackageReviewDangerousAuthorityClass::PortIo)
        }
        (path, "InterruptMaskControl") if path == std::path::Path::new("interrupt.omg") => {
            Some(PackageReviewDangerousAuthorityClass::InterruptControl)
        }
        (path, "InterruptEntry") if path == std::path::Path::new("interrupt.omg") => {
            Some(PackageReviewDangerousAuthorityClass::InterruptEntry)
        }
        (path, "ExtentRootProvider") if path == std::path::Path::new("extent.omg") => {
            Some(PackageReviewDangerousAuthorityClass::RootMemory)
        }
        (path, "Console") if path == std::path::Path::new("console.omg") => {
            Some(PackageReviewDangerousAuthorityClass::Process)
        }
        _ => None,
    }
}

fn project_public_traits(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedReviewRow<PackageReviewTraitShape>>, Vec<Diagnostic>> {
    let mut rows = Vec::new();
    for definition in compilation.traits().iter().filter(|row| row.is_public) {
        let identity = nominal_identity(compilation, definition.symbol)?;
        if !reviewed_package_owns(&identity, package)? {
            continue;
        }
        if !compilation.trait_invariants(definition).is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "public trait `{}` uses invariants not yet represented by package review",
                identity.path
            ))]);
        }

        let parameters = compilation.trait_type_parameters(definition);
        let (mut trait_binders, type_parameters) =
            project_type_parameters(compilation, parameters, "trait", &identity.path)?;
        trait_binders.insert(0, (definition.symbol, "trait-self".to_owned()));
        let conformance_bounds = project_conformance_bounds(
            compilation,
            &definition.conformance_bounds,
            parameters,
            &trait_binders,
            &definition.lifetime_parameters,
            "public trait",
            &identity.path,
        )?;
        let parents = compilation
            .trait_requirements(definition)
            .iter()
            .map(|parent| {
                project_trait_parent(
                    compilation,
                    parent,
                    &trait_binders,
                    &definition.lifetime_parameters,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let requirements = compilation
            .trait_machine_signatures(definition)
            .iter()
            .map(|requirement| {
                project_trait_requirement(
                    compilation,
                    definition.symbol,
                    requirement,
                    &trait_binders,
                    parameters.len(),
                    &definition.lifetime_parameters,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        rows.push(ProjectedReviewRow {
            row: PackageReviewTraitShape {
                identity,
                is_boundary: definition.is_boundary,
                lifetime_parameter_count: definition.lifetime_parameters.len(),
                type_parameters,
                conformance_bounds,
                parents,
                requirements,
            },
            declaration: definition.symbol,
        });
    }
    rows.sort_by(|left, right| left.row.identity.cmp(&right.row.identity));
    Ok(rows)
}

fn project_trait_parent(
    compilation: &CheckedCompilation,
    parent: &psi_typed_trees::trait_definition::TraitRequirement,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[psi_typed_trees::name::Identifier],
) -> Result<PackageReviewTraitParent, Vec<Diagnostic>> {
    let matches = compilation
        .traits()
        .iter()
        .filter(|candidate| candidate.symbol == parent.symbol)
        .collect::<Vec<_>>();
    let [definition] = matches.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "package review trait parent resolves to {} declarations; expected exactly one",
            matches.len()
        ))]);
    };
    if parent.lifetime_arguments.len() != definition.lifetime_parameters.len() {
        return Err(vec![Diagnostic::error(format!(
            "package review trait parent `{}` has {} resolved lifetime arguments; expected {}",
            parent.name,
            parent.lifetime_arguments.len(),
            definition.lifetime_parameters.len(),
        ))]);
    }
    Ok(PackageReviewTraitParent {
        kind: if definition.is_boundary {
            psi_typed_trees::trait_definition::TraitCompositionKind::ServiceReach
        } else {
            psi_typed_trees::trait_definition::TraitCompositionKind::Policy
        },
        identity: nominal_identity(compilation, definition.symbol)?,
        lifetime_arguments: parent
            .lifetime_arguments
            .iter()
            .map(|argument| lifetime_binder_ordinal(argument, lifetime_binders, "trait parent"))
            .collect::<Result<Vec<_>, _>>()?,
        arguments: compilation
            .type_reference_table
            .type_reference_handles(parent.arguments)
            .iter()
            .map(|argument| {
                review_signature_type_identity_with_binders(
                    compilation,
                    *argument,
                    binders,
                    lifetime_binders,
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn project_trait_requirement(
    compilation: &CheckedCompilation,
    trait_symbol: SymbolHandle,
    requirement: &psi_typed_trees::signature::StateSignature,
    trait_binders: &[(SymbolHandle, String)],
    trait_parameter_count: usize,
    trait_lifetime_parameters: &[psi_typed_trees::name::Identifier],
) -> Result<PackageReviewTraitRequirement, Vec<Diagnostic>> {
    let identity = nominal_identity(compilation, requirement.symbol)?;
    let parameters = compilation.state_signature_type_parameters(requirement);
    let (binders, type_parameters) = project_type_parameters_after(
        compilation,
        parameters,
        "trait requirement",
        &identity.path,
        trait_binders,
        trait_parameter_count,
    )?;
    let mut lifetime_binders = trait_lifetime_parameters.to_vec();
    lifetime_binders.extend(requirement.lifetime_parameters.iter().cloned());
    // Preserve the more specific public-progress validation before the same
    // membership facts enter the general contract lane.
    let termination = project_trait_requirement_termination(compilation, requirement)?;
    let contract_parameters = compilation.state_signature_parameters(requirement);
    let contract_context = ContractProjectionContext {
        subject_kind: "public trait requirement",
        subject_name: &identity.path,
        owner: psi_checked_trees::ContractProofFactOwner::StateSignature {
            owner_symbol: trait_symbol,
            state_symbol: requirement.symbol,
        },
        point: psi_facts::ProgramPoint::State {
            machine_symbol: trait_symbol,
            state_symbol: requirement.symbol,
        },
        parameters: contract_parameters,
        domain_symbol: None,
    };
    let contracts =
        project_trait_requirement_contracts(compilation, requirement, &contract_context, &binders)?;
    let mut crash_capsules = compilation
        .facts
        .contract_plans
        .crash_capsules
        .iter()
        .filter(|capsule| {
            capsule.target_machine() == trait_symbol && capsule.target_state() == requirement.symbol
        });
    let crash_capsule = crash_capsules.next().ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "public trait requirement `{}` has no exact checked crash capsule",
            identity.path
        ))]
    })?;
    if crash_capsules.next().is_some() {
        return Err(vec![Diagnostic::error(format!(
            "public trait requirement `{}` has duplicate checked crash capsules",
            identity.path
        ))]);
    }
    let published_crash = project_crash_routes(crash_capsule.published_buckets());
    Ok(PackageReviewTraitRequirement {
        identity,
        spelling: requirement.spelling,
        has_default_realization: requirement.is_default,
        lifetime_parameter_count: requirement.lifetime_parameters.len(),
        type_parameters,
        parameters: compilation
            .state_signature_parameters(requirement)
            .iter()
            .map(|parameter| {
                Ok(PackageReviewTraitRequirementParameter {
                    name: parameter.name.as_str().to_owned(),
                    type_identity: review_signature_type_identity_with_binders(
                        compilation,
                        parameter.type_reference,
                        &binders,
                        &lifetime_binders,
                    )?,
                    is_const: parameter.is_const,
                    is_mutable: parameter.is_mutable,
                    is_self: parameter.is_self,
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?,
        return_type: review_signature_type_identity_with_binders(
            compilation,
            requirement.return_type,
            &binders,
            &lifetime_binders,
        )?,
        contracts,
        published_crash,
        service_reach: project_service_row(compilation, requirement.service_reach_row)?,
        service_reach_is_installation_bound: requirement.service_reach_is_installation_bound,
        synchronous_invocations: project_synchronous_invocations(
            compilation,
            &psi_effects::declared_signature_invocations(compilation, requirement),
        )?,
        suspends: requirement.suspends,
        blocks: requirement.blocks,
        termination,
    })
}

fn project_public_domains(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedReviewRow<PackageReviewDomainShape>>, Vec<Diagnostic>> {
    let mut rows = Vec::new();
    for definition in compilation
        .domain_definitions()
        .iter()
        .filter(|row| row.is_public)
    {
        let identity = nominal_identity(compilation, definition.symbol)?;
        if !reviewed_package_owns(&identity, package)? {
            continue;
        }
        if !definition.semantic_roles.is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "public domain `{}` uses semantic roles not yet represented by package review",
                identity.path
            ))]);
        }
        if !compilation.domain_operators(definition).is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "public domain `{}` uses operators not yet represented by package review",
                identity.path
            ))]);
        }

        let parameters = compilation.domain_type_parameters(definition);
        let (binders, type_parameters) =
            project_type_parameters(compilation, parameters, "domain", &identity.path)?;
        let predicate_facts =
            project_domain_predicate_facts(compilation, definition, &identity, &binders)?;
        let alias_expansion = definition
            .alias
            .as_ref()
            .map(|_| project_domain_alias_expansion(compilation, definition.symbol))
            .transpose()?;
        let classification = definition
            .classification
            .map(|classification| match classification {
                psi_language_semantics::DomainClassification::ProgressProfile => {
                    PackageReviewDomainClassification::ProgressProfile
                }
            });
        let mut establishment_routes = definition
            .establishment_routes
            .iter()
            .map(|route| project_domain_establishment_route(compilation, *route))
            .collect::<Result<Vec<_>, _>>()?;
        establishment_routes.sort();
        establishment_routes.dedup();
        rows.push(ProjectedReviewRow {
            row: PackageReviewDomainShape {
                identity,
                type_parameters,
                target_type: review_type_identity_with_binders(
                    compilation,
                    definition.target_type,
                    &binders,
                ),
                index_arguments: definition
                    .index_arguments
                    .iter()
                    .map(|argument| {
                        review_type_identity_with_binders(compilation, *argument, &binders)
                    })
                    .collect(),
                predicate_body: definition.predicate_body,
                predicate_facts,
                alias_expansion,
                classification,
                establishment_routes,
            },
            declaration: definition.symbol,
        });
    }
    rows.sort_by(|left, right| left.row.identity.cmp(&right.row.identity));
    Ok(rows)
}

fn project_domain_predicate_facts(
    compilation: &CheckedCompilation,
    definition: &psi_typed_trees::domain::DomainDefinition,
    identity: &PackageReviewNominalIdentity,
    binders: &[(SymbolHandle, String)],
) -> Result<Vec<PackageReviewContractFact>, Vec<Diagnostic>> {
    use psi_typed_trees::domain::ProofFact;

    let context = ContractProjectionContext {
        subject_kind: "public domain",
        subject_name: &identity.path,
        owner: psi_checked_trees::ContractProofFactOwner::Unknown,
        point: psi_facts::ProgramPoint::Definition {
            symbol: definition.symbol,
        },
        parameters: &[],
        domain_symbol: Some(definition.symbol),
    };
    let reviewed_package = compilation.package_identity().ok_or_else(|| {
        vec![Diagnostic::error(
            "domain predicate review requires package-aware checked compilation",
        )]
    })?;
    let mut projected = Vec::new();
    for offset in 0..definition.facts.count() {
        let fact_handle = psi_arena::Handle::from_parts(
            definition
                .facts
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("domain predicate fact handle index overflow"),
            definition.facts.start().generation(),
        );
        require_exact_checked_domain_fact(compilation, definition.symbol, fact_handle, identity)?;
        let fact = match compilation.proof_facts.get(fact_handle) {
            ProofFact::Expression(expression) => {
                PackageReviewContractFact::Expression(project_contract_expression(
                    compilation,
                    &context,
                    binders,
                    *expression,
                    Some(fact_handle),
                    0,
                )?)
            }
            ProofFact::Membership(membership) => {
                let domain = compilation
                    .domain_definitions()
                    .iter()
                    .find(|domain| domain.symbol == membership.domain_symbol)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(format!(
                            "public domain `{}` predicate refers to an unresolved domain",
                            identity.path
                        ))]
                    })?;
                let domain_identity = nominal_identity(compilation, domain.symbol)?;
                if reviewed_package_owns(&domain_identity, reviewed_package)? && !domain.is_public {
                    return Err(vec![Diagnostic::error(format!(
                        "public domain `{}` predicate exposes non-public domain `{}`",
                        identity.path, domain.name
                    ))]);
                }
                PackageReviewContractFact::Membership {
                    value: project_contract_expression(
                        compilation,
                        &context,
                        binders,
                        membership.value,
                        Some(fact_handle),
                        0,
                    )?,
                    domain: domain_identity,
                }
            }
            ProofFact::Proposition(application) => project_contract_proposition(
                compilation,
                &context,
                binders,
                application,
                Some(fact_handle),
                &[],
                &[],
                &mut Vec::new(),
                0,
            )?,
        };
        projected.push(fact);
    }
    projected.sort();
    projected.dedup();
    Ok(projected)
}

fn require_exact_checked_domain_fact(
    compilation: &CheckedCompilation,
    domain_symbol: SymbolHandle,
    fact_handle: psi_arena::Handle<psi_typed_trees::domain::ProofFact>,
    identity: &PackageReviewNominalIdentity,
) -> Result<(), Vec<Diagnostic>> {
    let point = psi_facts::ProgramPoint::Definition {
        symbol: domain_symbol,
    };
    let matching_rows = compilation
        .facts
        .semantic
        .facts
        .iter()
        .filter_map(|(handle, fact)| {
            (fact.point == point
                && fact.origin == psi_facts::FactOrigin::DomainDefinition { domain_symbol }
                && fact.evidence == psi_facts::QualificationEvidence::default()
                && semantic_fact_matches_domain_fact(compilation, fact, fact_handle))
            .then_some(handle)
        })
        .collect::<Vec<_>>();
    if matching_rows.len() != 1 {
        return Err(vec![Diagnostic::error(format!(
            "public domain `{}` predicate fact has {} exact checked definition rows; expected one",
            identity.path,
            matching_rows.len()
        ))]);
    }
    let matching_records = compilation
        .facts
        .semantic
        .domain_definition_facts
        .iter()
        .filter(|(_, record)| {
            record.domain_symbol == domain_symbol
                && record.fact == fact_handle
                && record.semantic_fact == matching_rows[0]
        })
        .count();
    if matching_records != 1 {
        return Err(vec![Diagnostic::error(format!(
            "public domain `{}` predicate fact has {matching_records} exact checked ownership records; expected one",
            identity.path
        ))]);
    }
    Ok(())
}

fn semantic_fact_matches_domain_fact(
    compilation: &CheckedCompilation,
    semantic_fact: &psi_facts::Fact,
    fact_handle: psi_arena::Handle<psi_typed_trees::domain::ProofFact>,
) -> bool {
    use psi_facts::FactPayload;
    use psi_typed_trees::domain::ProofFact;

    match (
        compilation.proof_facts.get(fact_handle),
        semantic_fact.payload,
    ) {
        (ProofFact::Expression(expected), FactPayload::BooleanExpression(actual)) => {
            *expected == actual
        }
        (
            ProofFact::Membership(expected),
            FactPayload::DomainMembership {
                value,
                domain,
                domain_symbol,
            },
        ) => {
            expected.value == value
                && expected.domain == domain
                && expected.domain_symbol == domain_symbol
        }
        (
            ProofFact::Proposition(expected),
            FactPayload::PropositionApplication { fact, proposition },
        ) => fact == fact_handle && proposition == expected.proposition,
        _ => false,
    }
}

fn project_domain_alias_expansion(
    compilation: &CheckedCompilation,
    domain_symbol: SymbolHandle,
) -> Result<Vec<PackageReviewNominalIdentity>, Vec<Diagnostic>> {
    fn expand(
        compilation: &CheckedCompilation,
        domain_symbol: SymbolHandle,
        stack: &mut Vec<SymbolHandle>,
        atoms: &mut Vec<PackageReviewNominalIdentity>,
    ) -> Result<(), Vec<Diagnostic>> {
        if stack.contains(&domain_symbol) {
            return Err(vec![Diagnostic::error(
                "package review encountered a cycle in checked domain alias expansion",
            )]);
        }
        let definitions = compilation
            .domain_definitions()
            .iter()
            .filter(|candidate| candidate.symbol == domain_symbol)
            .collect::<Vec<_>>();
        let [definition] = definitions.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "package review domain alias resolves to {} declarations; expected exactly one",
                definitions.len()
            ))]);
        };
        let Some(alias) = definition.alias.as_ref() else {
            atoms.push(nominal_identity(compilation, definition.symbol)?);
            return Ok(());
        };
        stack.push(domain_symbol);
        for constituent in &alias.constituents {
            let label = compilation
                .domain_path_members(constituent.domain)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            if label == "Carry::Portable" {
                atoms.extend(
                    psi_language_semantics::CarryPermission::ALL.map(|permission| {
                        PackageReviewNominalIdentity {
                            owner: PackageReviewNominalOwner::ToolchainUnbound,
                            path: permission.name().to_owned(),
                        }
                    }),
                );
            } else if let Some(permission) =
                psi_language_semantics::CarryPermission::from_name(&label)
            {
                atoms.push(PackageReviewNominalIdentity {
                    owner: PackageReviewNominalOwner::ToolchainUnbound,
                    path: permission.name().to_owned(),
                });
            } else {
                if !constituent.domain_symbol.is_valid() {
                    return Err(vec![Diagnostic::error(format!(
                        "package review domain alias has unresolved constituent `{label}`"
                    ))]);
                }
                expand(compilation, constituent.domain_symbol, stack, atoms)?;
            }
        }
        stack.pop();
        Ok(())
    }

    let mut atoms = Vec::new();
    expand(compilation, domain_symbol, &mut Vec::new(), &mut atoms)?;
    atoms.sort();
    atoms.dedup();
    if atoms.is_empty() {
        return Err(vec![Diagnostic::error(
            "package review domain alias has an empty canonical expansion",
        )]);
    }
    Ok(atoms)
}

fn project_domain_establishment_route(
    compilation: &CheckedCompilation,
    route: psi_language_semantics::DomainEstablishmentRoute,
) -> Result<PackageReviewDomainEstablishmentRoute, Vec<Diagnostic>> {
    let (kind, trait_symbol, requirement_symbol, expects_boundary) = match route {
        psi_language_semantics::DomainEstablishmentRoute::CheckedRequirement {
            trait_definition,
            requirement,
        } => (
            PackageReviewDomainEstablishmentKind::CheckedRequirement,
            trait_definition,
            requirement,
            false,
        ),
        psi_language_semantics::DomainEstablishmentRoute::BoundaryRequirement {
            boundary_trait,
            requirement,
        } => (
            PackageReviewDomainEstablishmentKind::BoundaryRequirement,
            boundary_trait,
            requirement,
            true,
        ),
    };
    let owners = compilation
        .traits()
        .iter()
        .filter(|candidate| candidate.symbol == trait_symbol)
        .collect::<Vec<_>>();
    let [owner] = owners.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "package review domain establishment route resolves to {} trait declarations; expected exactly one",
            owners.len()
        ))]);
    };
    if owner.is_boundary != expects_boundary {
        return Err(vec![Diagnostic::error(
            "package review domain establishment route kind disagrees with its exact trait declaration",
        )]);
    }
    let requirements = compilation
        .trait_machine_signatures(owner)
        .iter()
        .filter(|candidate| candidate.symbol == requirement_symbol)
        .collect::<Vec<_>>();
    let [requirement] = requirements.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "package review domain establishment route resolves to {} requirements under its exact trait; expected exactly one",
            requirements.len()
        ))]);
    };
    Ok(PackageReviewDomainEstablishmentRoute {
        kind,
        trait_identity: nominal_identity(compilation, owner.symbol)?,
        requirement_identity: nominal_identity(compilation, requirement.symbol)?,
    })
}

fn project_public_data(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedReviewRow<PackageReviewDataShape>>, Vec<Diagnostic>> {
    let mut rows = Vec::new();
    for definition in compilation
        .data_definitions()
        .iter()
        .filter(|row| row.is_public)
    {
        let identity = nominal_identity(compilation, definition.symbol)?;
        if !reviewed_package_owns(&identity, package)? {
            continue;
        }
        if definition.quotient.is_some() {
            return Err(vec![Diagnostic::error(format!(
                "public data `{}` uses quotient semantics not yet represented by package review",
                identity.path
            ))]);
        }
        if !definition.where_facts.is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "public data `{}` uses proof facts not yet represented by package review",
                identity.path
            ))]);
        }

        let parameters = compilation.data_type_parameters(definition);
        let (binders, type_parameters) =
            project_type_parameters(compilation, parameters, "data", &identity.path)?;

        let members = compilation
            .data_members(definition)
            .iter()
            .map(
                |member| -> Result<PackageReviewDataMember, Vec<Diagnostic>> {
                    Ok(match member {
                        psi_typed_trees::data::DataMember::Field(field) => {
                            PackageReviewDataMember::Field(project_data_field(
                                compilation,
                                field,
                                &binders,
                                &definition.lifetime_parameters,
                            )?)
                        }
                        psi_typed_trees::data::DataMember::Variant(variant) => {
                            let mut retired_payload_identities =
                                variant.retired_payload_identities.clone();
                            retired_payload_identities.sort_unstable();
                            retired_payload_identities.dedup();
                            PackageReviewDataMember::Variant {
                                identity: variant.identity,
                                name: variant.name.as_str().to_owned(),
                                payload: compilation
                                    .data_payload_fields(variant)
                                    .iter()
                                    .map(|field| {
                                        project_data_field(
                                            compilation,
                                            field,
                                            &binders,
                                            &definition.lifetime_parameters,
                                        )
                                    })
                                    .collect::<Result<Vec<_>, _>>()?,
                                retired_payload_identities,
                            }
                        }
                    })
                },
            )
            .collect::<Result<Vec<_>, _>>()?;
        let mut retired_identities = definition.retired_identities.clone();
        retired_identities.sort_unstable();
        retired_identities.dedup();
        rows.push(ProjectedReviewRow {
            row: PackageReviewDataShape {
                identity,
                supply: definition.supply_mode,
                lifetime_parameter_count: definition.lifetime_parameters.len(),
                type_parameters,
                properties: definition.properties,
                zero_gated: definition.zero_gated,
                retired_identities,
                members,
            },
            declaration: definition.symbol,
        });
    }
    rows.sort_by(|left, right| left.row.identity.cmp(&right.row.identity));
    Ok(rows)
}

fn project_type_parameters(
    compilation: &CheckedCompilation,
    parameters: &[psi_typed_trees::data::TypeParameter],
    declaration_kind: &str,
    declaration_path: &str,
) -> Result<(Vec<(SymbolHandle, String)>, Vec<PackageReviewTypeParameter>), Vec<Diagnostic>> {
    project_type_parameters_after(
        compilation,
        parameters,
        declaration_kind,
        declaration_path,
        &[],
        0,
    )
}

fn project_type_parameters_after(
    compilation: &CheckedCompilation,
    parameters: &[psi_typed_trees::data::TypeParameter],
    declaration_kind: &str,
    declaration_path: &str,
    preceding_binders: &[(SymbolHandle, String)],
    ordinal_offset: usize,
) -> Result<(Vec<(SymbolHandle, String)>, Vec<PackageReviewTypeParameter>), Vec<Diagnostic>> {
    let mut binders = preceding_binders.to_vec();
    binders.extend(parameters.iter().enumerate().map(|(ordinal, parameter)| {
        (
            parameter.symbol,
            format!("type-parameter:{}", ordinal_offset + ordinal),
        )
    }));
    let mut projected = Vec::with_capacity(parameters.len());
    for parameter in parameters {
        let kind = match parameter.kind {
            psi_typed_trees::data::TypeParameterKind::Type => PackageReviewTypeParameterKind::Type,
            psi_typed_trees::data::TypeParameterKind::Const { type_reference } => {
                PackageReviewTypeParameterKind::Const(review_type_identity_with_binders(
                    compilation,
                    type_reference,
                    &binders,
                ))
            }
            psi_typed_trees::data::TypeParameterKind::Machine { .. }
            | psi_typed_trees::data::TypeParameterKind::Proposition { .. } => {
                return Err(vec![Diagnostic::error(format!(
                    "public {declaration_kind} `{declaration_path}` uses a static machine or proposition parameter not yet represented by package review",
                ))]);
            }
        };
        projected.push(PackageReviewTypeParameter {
            kind,
            bounds: parameter.bounds,
        });
    }
    Ok((binders, projected))
}

fn project_conformance_bounds(
    compilation: &CheckedCompilation,
    bounds: &[psi_typed_trees::machine::GenericConformanceBound],
    parameters: &[psi_typed_trees::data::TypeParameter],
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[psi_typed_trees::name::Identifier],
    declaration_kind: &str,
    declaration_path: &str,
) -> Result<Vec<PackageReviewConformanceBound>, Vec<Diagnostic>> {
    let mut projected = Vec::with_capacity(bounds.len());
    let mut next_binder_ordinal = 0usize;
    for bound in bounds {
        let binder_ordinal = if let Some(binder) = bound.binder {
            if !binder.is_valid() {
                return Err(vec![Diagnostic::error(format!(
                    "{declaration_kind} `{declaration_path}` has an unresolved conformance evidence binder"
                ))]);
            }
            let ordinal = u32::try_from(next_binder_ordinal).map_err(|_| {
                vec![Diagnostic::error(format!(
                    "{declaration_kind} `{declaration_path}` has too many conformance binders for portable review evidence"
                ))]
            })?;
            next_binder_ordinal += 1;
            Some(ordinal)
        } else {
            None
        };
        let Some(subject_parameter) = parameters
            .iter()
            .position(|parameter| parameter.symbol == bound.subject)
        else {
            return Err(vec![Diagnostic::error(format!(
                "{declaration_kind} `{declaration_path}` has a conformance subject outside its type-parameter telescope"
            ))]);
        };
        let selected = bound.conformance.map(|symbol| {
            compilation
                .conformances()
                .iter()
                .filter(|declaration| declaration.symbol == symbol)
                .collect::<Vec<_>>()
        });
        let (
            selected_conformance,
            selected_carrier,
            selected_carrier_arguments,
            trait_symbol,
            trait_arguments,
        ) = match selected {
            None => (
                None,
                None,
                Vec::new(),
                bound.carrier,
                bound.arguments.clone(),
            ),
            Some(selected) => {
                let [selected] = selected.as_slice() else {
                    return Err(vec![Diagnostic::error(format!(
                        "{declaration_kind} `{declaration_path}` resolves its selected conformance to {} declarations; expected exactly one",
                        selected.len()
                    ))]);
                };
                if !selected.lifetime_parameters.is_empty()
                    || !compilation.conformance_type_parameters(selected).is_empty()
                {
                    return Err(vec![Diagnostic::error(format!(
                        "{declaration_kind} `{declaration_path}` selects generic conformance `{}` whose application telescope is not yet represented by package review",
                        selected
                            .alias
                            .as_ref()
                            .map_or("<unnamed>", |name| name.as_str())
                    ))]);
                }
                if selected.carrier_symbol != bound.carrier {
                    return Err(vec![Diagnostic::error(format!(
                        "{declaration_kind} `{declaration_path}` selected conformance carrier does not match its exact bound carrier"
                    ))]);
                }
                let matching_carriers = compilation
                    .data_definitions()
                    .iter()
                    .filter(|definition| definition.symbol == selected.carrier_symbol)
                    .collect::<Vec<_>>();
                let [carrier] = matching_carriers.as_slice() else {
                    return Err(vec![Diagnostic::error(format!(
                        "{declaration_kind} `{declaration_path}` resolves its selected conformance carrier to {} data declarations; expected exactly one",
                        matching_carriers.len()
                    ))]);
                };
                if !carrier.is_public {
                    return Err(vec![Diagnostic::error(format!(
                        "{declaration_kind} `{declaration_path}` exposes non-public selected-conformance carrier `{}`",
                        carrier.name
                    ))]);
                }
                (
                    Some(nominal_identity(compilation, selected.symbol)?),
                    Some(nominal_identity(compilation, selected.carrier_symbol)?),
                    bound
                        .arguments
                        .iter()
                        .map(|argument| {
                            review_signature_type_identity_with_binders(
                                compilation,
                                *argument,
                                binders,
                                lifetime_binders,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                    selected.trait_symbol,
                    compilation
                        .type_reference_table
                        .type_reference_handles(selected.arguments)
                        .to_vec(),
                )
            }
        };
        let matching_traits = compilation
            .traits()
            .iter()
            .filter(|definition| definition.symbol == trait_symbol)
            .collect::<Vec<_>>();
        let [trait_definition] = matching_traits.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "{declaration_kind} `{declaration_path}` conformance bound resolves to {} traits; expected exactly one",
                matching_traits.len()
            ))]);
        };
        if !trait_definition.is_public {
            return Err(vec![Diagnostic::error(format!(
                "{declaration_kind} `{declaration_path}` exposes non-public conformance trait `{}`",
                trait_definition.name
            ))]);
        }
        if !trait_definition.lifetime_parameters.is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "{declaration_kind} `{declaration_path}` uses lifetime-parameterized conformance trait `{}` without retained lifetime arguments",
                trait_definition.name
            ))]);
        }
        projected.push(PackageReviewConformanceBound {
            binder_ordinal,
            subject_parameter: u32::try_from(subject_parameter).map_err(|_| {
                vec![Diagnostic::error(format!(
                    "{declaration_kind} `{declaration_path}` conformance subject exceeds the portable review parameter range"
                ))]
            })?,
            selected_conformance,
            selected_carrier,
            selected_carrier_arguments,
            trait_identity: nominal_identity(compilation, trait_definition.symbol)?,
            arguments: trait_arguments
                .iter()
                .map(|argument| {
                    review_signature_type_identity_with_binders(
                        compilation,
                        *argument,
                        binders,
                        lifetime_binders,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
        });
    }
    Ok(projected)
}

fn project_data_field(
    compilation: &CheckedCompilation,
    field: &psi_typed_trees::data::DataField,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[psi_typed_trees::name::Identifier],
) -> Result<PackageReviewDataField, Vec<Diagnostic>> {
    Ok(PackageReviewDataField {
        identity: field.identity,
        name: field.name.as_str().to_owned(),
        relevance: field.relevance,
        type_identity: review_signature_type_identity_with_binders(
            compilation,
            field.type_reference,
            binders,
            lifetime_binders,
        )?,
    })
}

fn review_type_identity_with_binders(
    compilation: &CheckedCompilation,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
) -> PackageReviewTypeIdentity {
    PackageReviewTypeIdentity {
        canonical: compilation
            .package_qualified_type_identity_with_binders(type_reference, binders)
            .into_string(),
    }
}

fn review_type_identity_with_binders_and_substitutions(
    compilation: &CheckedCompilation,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    substitutions: &[(SymbolHandle, psi_typed_trees::types::TypeReferenceHandle)],
) -> PackageReviewTypeIdentity {
    PackageReviewTypeIdentity {
        canonical: compilation
            .package_qualified_type_identity_with_binders_and_substitutions(
                type_reference,
                binders,
                substitutions,
            )
            .into_string(),
    }
}

/// Public signature identity layers erased borrow-region relationships over
/// the ordinary package-qualified runtime type identity. General structural
/// type identity intentionally erases these tags; package compatibility may
/// not, because changing which input owns an output loan changes the callable
/// contract without changing layout or monomorphization.
fn review_signature_type_identity_with_binders(
    compilation: &CheckedCompilation,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[psi_typed_trees::name::Identifier],
) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
    let runtime = compilation
        .package_qualified_type_identity_with_binders(type_reference, binders)
        .into_string();
    let lifetime = review_lifetime_topology(compilation, type_reference, lifetime_binders)?;
    Ok(PackageReviewTypeIdentity {
        canonical: framed_identity("signature-type", &[runtime, lifetime]),
    })
}

fn review_lifetime_topology(
    compilation: &CheckedCompilation,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    lifetime_binders: &[psi_typed_trees::name::Identifier],
) -> Result<String, Vec<Diagnostic>> {
    use psi_typed_trees::types::{TypeConstraintNode, TypeReferenceNode};

    let topology = match compilation
        .type_reference_table
        .type_reference(type_reference)
    {
        TypeReferenceNode::Reference {
            referee, lifetime, ..
        } => {
            let lifetime = match lifetime {
                Some(lifetime) => format!(
                    "binder:{}",
                    lifetime_binder_ordinal(lifetime, lifetime_binders, "public type")?
                ),
                None => "elided".to_owned(),
            };
            framed_identity(
                "reference",
                &[
                    lifetime,
                    review_lifetime_topology(compilation, *referee, lifetime_binders)?,
                ],
            )
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let mut constraint_topologies = compilation
                .type_reference_table
                .constraints(*constraints)
                .iter()
                .filter_map(|constraint| match constraint {
                    TypeConstraintNode::Domain(domain) if !domain.arguments.is_empty() => Some(
                        domain
                            .arguments
                            .iter()
                            .map(|argument| {
                                review_lifetime_topology(compilation, *argument, lifetime_binders)
                            })
                            .collect::<Result<Vec<_>, _>>()
                            .map(|arguments| framed_identity(domain.as_str(), &arguments)),
                    ),
                    _ => None,
                })
                .collect::<Result<Vec<_>, _>>()?;
            constraint_topologies.sort();
            constraint_topologies.dedup();
            let mut children = vec![review_lifetime_topology(
                compilation,
                *base_type,
                lifetime_binders,
            )?];
            children.extend(constraint_topologies);
            framed_identity("constrained", &children)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => framed_identity(
            "array",
            &[review_lifetime_topology(
                compilation,
                *element_type,
                lifetime_binders,
            )?],
        ),
        TypeReferenceNode::Slice { element_type } => framed_identity(
            "slice",
            &[review_lifetime_topology(
                compilation,
                *element_type,
                lifetime_binders,
            )?],
        ),
        TypeReferenceNode::Generic {
            lifetime_arguments,
            arguments,
            ..
        } => {
            let mut children = lifetime_arguments
                .iter()
                .map(|lifetime| {
                    lifetime_binder_ordinal(lifetime, lifetime_binders, "public type")
                        .map(|ordinal| format!("binder:{ordinal}"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            children.extend(
                compilation
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .iter()
                    .map(|argument| {
                        review_lifetime_topology(compilation, *argument, lifetime_binders)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            );
            framed_identity("generic", &children)
        }
        TypeReferenceNode::Named { .. } => "named".to_owned(),
        TypeReferenceNode::DynamicTrait { .. } => "dynamic-trait".to_owned(),
        TypeReferenceNode::ConstExpression(_) => "const-expression".to_owned(),
        TypeReferenceNode::Unit => "unit".to_owned(),
    };
    Ok(topology)
}

fn lifetime_binder_ordinal(
    lifetime: &psi_typed_trees::name::Identifier,
    lifetime_binders: &[psi_typed_trees::name::Identifier],
    context: &str,
) -> Result<u32, Vec<Diagnostic>> {
    let Some(ordinal) = lifetime_binders
        .iter()
        .position(|candidate| candidate == lifetime)
    else {
        return Err(vec![Diagnostic::error(format!(
            "{context} refers to unresolved lifetime `'{}'",
            lifetime.as_str()
        ))]);
    };
    u32::try_from(ordinal).map_err(|_| {
        vec![Diagnostic::error(format!(
            "{context} lifetime binder ordinal exceeds the portable package-review limit"
        ))]
    })
}

fn framed_identity(label: &str, children: &[String]) -> String {
    use std::fmt::Write as _;

    let mut identity = String::new();
    let _ = write!(identity, "{}:{label}", label.len());
    for child in children {
        let _ = write!(identity, "{}:{child}", child.len());
    }
    identity
}

fn reviewed_package_owns(
    identity: &PackageReviewNominalIdentity,
    package: PackageKeyIdentity,
) -> Result<bool, Vec<Diagnostic>> {
    match identity.owner {
        PackageReviewNominalOwner::Package(owner) => Ok(owner == package),
        PackageReviewNominalOwner::ToolchainSource(_)
        | PackageReviewNominalOwner::ToolchainUnbound => Ok(false),
        PackageReviewNominalOwner::Unresolved => Err(vec![Diagnostic::error(format!(
            "reviewed public declaration `{}` has no managed package owner",
            identity.path
        ))]),
    }
}

fn project_callable(
    compilation: &CheckedCompilation,
    synchronous_invocations: &psi_effects::InvocationInferencePlan,
    machine: &psi_typed_trees::machine::Machine,
    role: PackageReviewCallableRole,
    identity: PackageReviewNominalIdentity,
) -> Result<CheckedPackageCallableReview, Vec<Diagnostic>> {
    let subject = identity.path.as_str();
    let Some(entry) = compilation.machine_states(machine).first() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{subject}` has no canonical entry signature"
        ))]);
    };
    let machine_type_parameters = compilation.machine_type_parameters(machine);
    let (binders, type_parameters) =
        project_type_parameters(compilation, machine_type_parameters, "callable", subject)?;
    let conformance_bounds = project_conformance_bounds(
        compilation,
        &machine.conformance_bounds,
        machine_type_parameters,
        &binders,
        &machine.lifetime_parameters,
        "reviewed callable",
        subject,
    )?;
    let parameters = compilation
        .state_parameters(entry)
        .iter()
        .map(|parameter| {
            Ok(PackageReviewCallableParameter {
                name: parameter.name.as_str().to_owned(),
                type_identity: review_signature_type_identity_with_binders(
                    compilation,
                    parameter.type_reference,
                    &binders,
                    &machine.lifetime_parameters,
                )?,
                is_const: parameter.is_const,
                is_mutable: parameter.is_mutable,
                is_self: parameter.is_self,
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    let return_type = review_signature_type_identity_with_binders(
        compilation,
        entry.return_type,
        &binders,
        &machine.lifetime_parameters,
    )?;
    let conformances = project_callable_conformances(compilation, machine, &binders)?;
    let contracts = project_callable_contracts(compilation, machine, entry, &binders)?;
    let service_reach = exactly_one(
        compilation
            .facts
            .service_reaches
            .machines()
            .iter()
            .filter(|fact| fact.machine == machine.symbol),
        subject,
        "service-reach",
    )?;
    let realized = exactly_one(
        compilation
            .facts
            .contract_plans
            .realized_envelopes
            .iter()
            .filter(|envelope| envelope.machine == machine.symbol),
        subject,
        "realized contract envelope",
    )?;
    let checked_invocation = exactly_one(
        compilation
            .facts
            .synchronous_invocations
            .machines
            .iter()
            .filter(|fact| fact.machine == machine.symbol),
        subject,
        "synchronous-invocation",
    )?;
    let invocation_summary = synchronous_invocations
        .for_machine(machine.symbol)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "reviewed callable `{subject}` has no exact inferred synchronous-invocation row"
            ))]
        })?;

    let declared_service_reach = match service_reach.interface {
        psi_language_semantics::ServiceReachInterface::PublishedCeiling(row) => {
            Some(project_service_row(compilation, row)?)
        }
        psi_language_semantics::ServiceReachInterface::InternalInferred
            if role == PackageReviewCallableRole::Build =>
        {
            None
        }
        psi_language_semantics::ServiceReachInterface::InternalInferred => {
            return Err(vec![Diagnostic::error(format!(
                "reviewed callable `{subject}` has no published service-reach ceiling"
            ))]);
        }
    };
    match machine.supply_mode {
        MachineSupplyMode::CheckedBody if !machine.body_is_present => {
            return Err(vec![Diagnostic::error(format!(
                "reviewed callable `{subject}` is classified as checked supply but has no retained body"
            ))]);
        }
        MachineSupplyMode::Accepted
        | MachineSupplyMode::Requirement
        | MachineSupplyMode::ExternalRealization { .. }
            if machine.body_is_present =>
        {
            return Err(vec![Diagnostic::error(format!(
                "reviewed callable `{subject}` has bodyless supply but retains a body"
            ))]);
        }
        MachineSupplyMode::CheckedBody
        | MachineSupplyMode::Boundary
        | MachineSupplyMode::Accepted
        | MachineSupplyMode::Requirement
        | MachineSupplyMode::ExternalRealization { .. } => {}
    }
    let has_checked_body = machine.body_is_present
        && matches!(
            machine.supply_mode,
            MachineSupplyMode::CheckedBody | MachineSupplyMode::Boundary
        );
    let checked_service_reach = if has_checked_body {
        let realized = project_service_row(compilation, service_reach.inferred_transitive)?;
        let concrete = project_service_row(compilation, service_reach.concrete_transitive)?;
        PackageReviewCheckedServiceReach::CheckedBody { realized, concrete }
    } else {
        PackageReviewCheckedServiceReach::NoCheckedBody
    };
    let declared_synchronous_invocations = match checked_invocation.plan.interface {
        psi_language_semantics::SynchronousInvocationInterface::PublishedCeiling => Some(
            project_synchronous_invocations(compilation, &invocation_summary.published)?,
        ),
        psi_language_semantics::SynchronousInvocationInterface::InternalInferred
            if role == PackageReviewCallableRole::Build =>
        {
            None
        }
        psi_language_semantics::SynchronousInvocationInterface::InternalInferred => {
            return Err(vec![Diagnostic::error(format!(
                "reviewed callable `{subject}` has no published synchronous-invocation ceiling"
            ))]);
        }
    };
    let realized_synchronous_invocations =
        project_synchronous_invocations(compilation, &invocation_summary.inferred_transitive)?;
    let mut capability_flows = realized
        .capabilities
        .iter()
        .map(|flow| project_capability_flow(compilation, flow))
        .collect::<Result<Vec<_>, _>>()?;
    capability_flows.sort_by(|left, right| {
        left.capability
            .cmp(&right.capability)
            .then(left.kind.as_str().cmp(right.kind.as_str()))
            .then(left.state.cmp(&right.state))
            .then(left.statement_index.cmp(&right.statement_index))
            .then(left.call_ordinal.cmp(&right.call_ordinal))
            .then(left.via_state.cmp(&right.via_state))
    });

    Ok(CheckedPackageCallableReview {
        role,
        identity,
        supply: machine.supply_mode,
        lifetime_parameter_count: machine.lifetime_parameters.len(),
        type_parameters,
        conformance_bounds,
        parameters,
        return_type,
        conformances,
        contracts,
        declared_service_reach,
        checked_service_reach,
        unresolved_installation_reaches: project_installation_reaches(
            compilation,
            &service_reach.unresolved_installation_reaches,
        )?,
        declared_synchronous_invocations,
        realized_synchronous_invocations,
        capability_flows,
        checked_may_suspend: realized.checked_may_suspend,
        checked_may_block: realized.checked_may_block,
        checked_termination: project_termination(compilation, &realized.checked_termination)?,
        checked_crash: project_crash(compilation, &realized.checked_crash)?,
        mutation: project_mutation(compilation, &realized.mutation)?,
    })
}

struct ContractProjectionContext<'a> {
    subject_kind: &'static str,
    subject_name: &'a str,
    owner: psi_checked_trees::ContractProofFactOwner,
    point: psi_facts::ProgramPoint,
    parameters: &'a [psi_typed_trees::signature::StateParameter],
    domain_symbol: Option<SymbolHandle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContractProjectionPolicy {
    Callable,
    PublicTraitRequirement,
}

fn project_callable_contracts(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
    entry: &psi_typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
) -> Result<Vec<PackageReviewCallableContract>, Vec<Diagnostic>> {
    let parameters = compilation.state_parameters(entry);
    let context = ContractProjectionContext {
        subject_kind: "callable",
        subject_name: machine.name.as_str(),
        owner: psi_checked_trees::ContractProofFactOwner::Machine {
            machine_symbol: machine.symbol,
        },
        point: psi_facts::ProgramPoint::Machine {
            machine_symbol: machine.symbol,
        },
        parameters,
        domain_symbol: None,
    };
    project_contracts(
        compilation,
        compilation.machine_contracts(machine),
        &context,
        binders,
        ContractProjectionPolicy::Callable,
    )
}

fn project_trait_requirement_contracts(
    compilation: &CheckedCompilation,
    requirement: &psi_typed_trees::signature::StateSignature,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
) -> Result<Vec<PackageReviewCallableContract>, Vec<Diagnostic>> {
    project_contracts(
        compilation,
        compilation.state_signature_contracts(requirement),
        context,
        binders,
        ContractProjectionPolicy::PublicTraitRequirement,
    )
}

fn project_contracts(
    compilation: &CheckedCompilation,
    contracts: &[psi_typed_trees::signature::SignatureContract],
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
    policy: ContractProjectionPolicy,
) -> Result<Vec<PackageReviewCallableContract>, Vec<Diagnostic>> {
    use psi_typed_trees::{domain::ProofFact, signature::SignatureContractKind};

    let reviewed_package = compilation.package_identity().ok_or_else(|| {
        vec![Diagnostic::error(
            "contract review requires package-aware checked compilation",
        )]
    })?;
    let mut projected = Vec::new();
    for contract in contracts {
        if policy == ContractProjectionPolicy::PublicTraitRequirement && contract.binding.is_some()
        {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` uses a named contract not yet represented by public-trait review",
                context.subject_kind, context.subject_name
            ))]);
        }
        let kind = match contract.kind {
            SignatureContractKind::Requires => PackageReviewContractKind::Requires,
            SignatureContractKind::Ensures => PackageReviewContractKind::Ensures,
            SignatureContractKind::Boundary
                if policy == ContractProjectionPolicy::PublicTraitRequirement =>
            {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` uses a boundary contract not yet represented by public-trait review",
                    context.subject_kind, context.subject_name
                ))]);
            }
            SignatureContractKind::Boundary => PackageReviewContractKind::Boundary,
            SignatureContractKind::Crashes { .. } => continue,
        };
        if contract.facts.is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` has an empty public {:?} contract",
                context.subject_kind, context.subject_name, kind
            ))]);
        }
        for offset in 0..contract.facts.count() {
            let fact_handle = psi_arena::Handle::from_parts(
                contract
                    .facts
                    .start()
                    .arena_index()
                    .checked_add(offset)
                    .expect("proof fact handle index overflow"),
                contract.facts.start().generation(),
            );
            let checked_fact = checked_contract_fact(compilation, context, fact_handle, kind)?;
            let fact = match compilation.proof_facts.get(fact_handle) {
                ProofFact::Expression(expression) => {
                    PackageReviewContractFact::Expression(project_contract_expression(
                        compilation,
                        context,
                        binders,
                        *expression,
                        Some(fact_handle),
                        0,
                    )?)
                }
                ProofFact::Membership(membership) => {
                    let domain = compilation
                        .domain_definitions()
                        .iter()
                        .find(|domain| domain.symbol == membership.domain_symbol)
                        .ok_or_else(|| {
                            vec![Diagnostic::error(format!(
                                "reviewed {} `{}` contract refers to an unresolved domain",
                                context.subject_kind, context.subject_name
                            ))]
                        })?;
                    let domain_identity = nominal_identity(compilation, domain.symbol)?;
                    if reviewed_package_owns(&domain_identity, reviewed_package)?
                        && !domain.is_public
                    {
                        return Err(vec![Diagnostic::error(format!(
                            "reviewed {} `{}` exposes non-public domain `{}` in its contract",
                            context.subject_kind, context.subject_name, domain.name
                        ))]);
                    }
                    PackageReviewContractFact::Membership {
                        value: project_contract_expression(
                            compilation,
                            context,
                            binders,
                            membership.value,
                            Some(fact_handle),
                            0,
                        )?,
                        domain: domain_identity,
                    }
                }
                ProofFact::Proposition(application) => project_contract_proposition(
                    compilation,
                    context,
                    binders,
                    application,
                    Some(fact_handle),
                    &[],
                    &[],
                    &mut Vec::new(),
                    0,
                )?,
            };
            let evidence_lane_position = validate_checked_contract_evidence(
                compilation,
                context,
                contract.binding.as_ref(),
                checked_fact,
                &fact,
            )?;
            projected.push(PackageReviewCallableContract {
                kind,
                binding: match kind {
                    PackageReviewContractKind::Ensures => contract
                        .binding
                        .as_ref()
                        .map(|binding| binding.as_str().to_owned()),
                    PackageReviewContractKind::Requires | PackageReviewContractKind::Boundary => {
                        None
                    }
                },
                evidence_lane_position,
                fact,
            });
        }
    }
    projected.sort();
    projected.dedup();
    Ok(projected)
}

fn checked_contract_fact<'a>(
    compilation: &'a CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    fact: psi_arena::Handle<psi_typed_trees::domain::ProofFact>,
    kind: PackageReviewContractKind,
) -> Result<&'a psi_checked_trees::ContractProofFact, Vec<Diagnostic>> {
    let checked_kind = match kind {
        PackageReviewContractKind::Requires => psi_checked_trees::ContractProofFactKind::Requires,
        PackageReviewContractKind::Ensures => psi_checked_trees::ContractProofFactKind::Ensures,
        PackageReviewContractKind::Boundary => psi_checked_trees::ContractProofFactKind::Boundary,
    };
    let matching = compilation
        .facts
        .proof
        .contract_facts
        .iter()
        .filter_map(|(_, checked)| {
            (checked.fact == fact && checked.kind == checked_kind && checked.owner == context.owner)
                .then_some(checked)
        })
        .collect::<Vec<_>>();
    let [checked] = matching.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract fact has {} checked owner rows; expected one",
            context.subject_kind,
            context.subject_name,
            matching.len()
        ))]);
    };
    Ok(*checked)
}

fn validate_checked_contract_evidence(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    binding: Option<&psi_typed_trees::name::Identifier>,
    checked: &psi_checked_trees::ContractProofFact,
    projected: &PackageReviewContractFact,
) -> Result<Option<u32>, Vec<Diagnostic>> {
    let Some(binding) = binding else {
        if checked.evidence_term.is_some() {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` has an unnamed contract with a checked evidence term",
                context.subject_kind, context.subject_name
            ))]);
        }
        return Ok(None);
    };
    let Some(term_handle) = checked.evidence_term else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` named contract `{}` has no checked evidence term",
            context.subject_kind, context.subject_name, binding
        ))]);
    };
    let term = compilation.facts.proof.evidence_terms.get(term_handle);
    if term.name != binding.as_str() || term.owner != checked.owner || term.kind != checked.kind {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` named contract `{}` does not match its checked evidence term",
            context.subject_kind, context.subject_name, binding
        ))]);
    }
    let PackageReviewContractFact::Proposition(application) = projected else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` named contract `{}` is not a proposition",
            context.subject_kind, context.subject_name, binding
        ))]);
    };
    if nominal_identity(compilation, term.proposition.declaration)? != application.declaration {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` named contract `{}` changed proposition endpoint during checked lowering",
            context.subject_kind, context.subject_name, binding
        ))]);
    }
    let PackageReviewPropositionEvidence::Witness(interface) = &application.evidence else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` named contract `{}` does not expose witness evidence",
            context.subject_kind, context.subject_name, binding
        ))]);
    };
    let Some(checked_interface) = term.evidence_interface.as_ref() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` named contract `{}` has no exact checked witness interface",
            context.subject_kind, context.subject_name, binding
        ))]);
    };
    if nominal_identity(compilation, checked_interface.trait_symbol)? != interface.trait_identity {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` named contract `{}` changed witness trait during checked lowering",
            context.subject_kind, context.subject_name, binding
        ))]);
    }
    let mut checked_requirements = checked_interface
        .requirements
        .iter()
        .map(|requirement| {
            Ok((
                nominal_identity(compilation, requirement.declaring_trait)?,
                nominal_identity(compilation, requirement.requirement)?,
            ))
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    checked_requirements.sort();
    let mut projected_requirements = interface
        .requirements
        .iter()
        .map(|requirement| {
            (
                requirement.declaring_trait.clone(),
                requirement.requirement.clone(),
            )
        })
        .collect::<Vec<_>>();
    projected_requirements.sort();
    if checked_requirements != projected_requirements {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` named contract `{}` changed witness requirements during checked lowering",
            context.subject_kind, context.subject_name, binding
        ))]);
    }
    portable_parameter_position(term.lane_position).map(Some)
}

fn project_contract_proposition(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    callable_binders: &[(SymbolHandle, String)],
    application: &psi_typed_trees::proposition::PropositionApplication,
    checked_fact: Option<psi_arena::Handle<psi_typed_trees::domain::ProofFact>>,
    binder_substitutions: &[(SymbolHandle, PackageReviewPropositionBinderArgument)],
    value_substitutions: &[(SymbolHandle, PackageReviewContractExpression)],
    visiting: &mut Vec<SymbolHandle>,
    depth: usize,
) -> Result<PackageReviewContractFact, Vec<Diagnostic>> {
    use psi_typed_trees::proposition::{PropositionBody, PropositionFormula};

    if depth >= 64 {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` proposition expansion exceeds the package-review depth limit",
            context.subject_kind, context.subject_name
        ))]);
    }
    if visiting.contains(&application.proposition) {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` proposition expansion is cyclic",
            context.subject_kind, context.subject_name
        ))]);
    }
    let declaration = compilation
        .propositions()
        .iter()
        .find(|candidate| candidate.symbol == application.proposition)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "reviewed {} `{}` contract refers to an unresolved or generic proposition endpoint",
                context.subject_kind, context.subject_name
            ))]
        })?;
    let declaration_binders = compilation.proposition_binders(declaration);
    let declaration_parameters = compilation.proposition_parameters(declaration);
    if declaration_binders.len() != application.binder_arguments.len()
        || declaration_parameters.len()
            != compilation
                .expression_table
                .expression_handles(application.arguments)
                .len()
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` proposition `{}` has inconsistent checked arity",
            context.subject_kind, context.subject_name, declaration.name
        ))]);
    }
    let binder_arguments = declaration_binders
        .iter()
        .zip(&application.binder_arguments)
        .map(|(binder, argument)| {
            let expected = match binder.kind {
                psi_typed_trees::proposition::PropositionBinderKind::Type => {
                    psi_typed_trees::proposition::PropositionBinderArgumentKind::Type
                }
                psi_typed_trees::proposition::PropositionBinderKind::Const { .. } => {
                    psi_typed_trees::proposition::PropositionBinderArgumentKind::Const
                }
                psi_typed_trees::proposition::PropositionBinderKind::Machine => {
                    psi_typed_trees::proposition::PropositionBinderArgumentKind::Machine
                }
            };
            if argument.kind != expected {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` proposition `{}` binder kind changed during typing",
                    context.subject_kind, context.subject_name, declaration.name
                ))]);
            }
            project_proposition_binder_argument(
                compilation,
                context,
                callable_binders,
                argument,
                binder_substitutions,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let arguments = compilation
        .expression_table
        .expression_handles(application.arguments)
        .iter()
        .map(|argument| {
            project_contract_expression_with_substitutions(
                compilation,
                context,
                callable_binders,
                *argument,
                value_substitutions,
                checked_fact,
                0,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    match &declaration.body {
        PropositionBody::Primitive | PropositionBody::Witness { .. } => Ok(
            PackageReviewContractFact::Proposition(project_proposition_endpoint(
                compilation,
                declaration,
                binder_arguments,
                arguments,
            )?),
        ),
        PropositionBody::Transparent { proposition } => {
            visiting.push(declaration.symbol);
            let mut nested_binders = binder_substitutions.to_vec();
            nested_binders.extend(
                declaration_binders
                    .iter()
                    .zip(&binder_arguments)
                    .map(|(binder, argument)| (binder.symbol, argument.clone())),
            );
            let mut nested_values = value_substitutions.to_vec();
            nested_values.extend(
                declaration_parameters
                    .iter()
                    .zip(&arguments)
                    .map(|(parameter, argument)| (parameter.symbol, argument.clone())),
            );
            for (binder, argument) in declaration_binders.iter().zip(&binder_arguments) {
                if let Some(value) = proposition_binder_value_expression(argument) {
                    nested_values.push((binder.symbol, value));
                }
            }
            let projected = match proposition {
                PropositionFormula::Application(expansion) => project_contract_proposition(
                    compilation,
                    context,
                    callable_binders,
                    expansion,
                    checked_fact,
                    &nested_binders,
                    &nested_values,
                    visiting,
                    depth + 1,
                ),
                PropositionFormula::BooleanExpression(expression) => {
                    project_contract_expression_with_substitutions(
                        compilation,
                        context,
                        callable_binders,
                        *expression,
                        &nested_values,
                        checked_fact,
                        0,
                    )
                    .map(PackageReviewContractFact::Expression)
                }
            };
            visiting.pop();
            projected
        }
    }
}

fn project_proposition_binder_argument(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    callable_binders: &[(SymbolHandle, String)],
    argument: &psi_typed_trees::proposition::PropositionBinderArgument,
    substitutions: &[(SymbolHandle, PackageReviewPropositionBinderArgument)],
) -> Result<PackageReviewPropositionBinderArgument, Vec<Diagnostic>> {
    if let Some((_, substitution)) = substitutions
        .iter()
        .rev()
        .find(|(symbol, _)| *symbol == argument.symbol)
    {
        if substitution.kind != argument.kind {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` proposition binder substitution changes kind",
                context.subject_kind, context.subject_name
            ))]);
        }
        return Ok(substitution.clone());
    }
    let value = if let Some(projection) = argument.evidence_projection.as_ref() {
        project_proposition_evidence_projection(compilation, context, projection)?
    } else if let Some(literal) = &argument.const_literal {
        PackageReviewPropositionBinderValue::Integer(literal.text().to_owned())
    } else if let Some(position) = callable_binders
        .iter()
        .position(|(symbol, _)| *symbol == argument.symbol)
    {
        PackageReviewPropositionBinderValue::GenericBinder(portable_parameter_position(position)?)
    } else if argument.symbol.is_valid() {
        PackageReviewPropositionBinderValue::Nominal(nominal_identity(
            compilation,
            argument.symbol,
        )?)
    } else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` proposition contains an unresolved binder argument",
            context.subject_kind, context.subject_name
        ))]);
    };
    Ok(PackageReviewPropositionBinderArgument {
        kind: argument.kind,
        value,
    })
}

fn project_proposition_evidence_projection(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    projection: &psi_typed_trees::expression::EvidenceProjection,
) -> Result<PackageReviewPropositionBinderValue, Vec<Diagnostic>> {
    let matching_terms = compilation
        .facts
        .proof
        .evidence_terms
        .iter()
        .filter_map(|(handle, term)| {
            (term.owner == context.owner
                && term.kind == psi_checked_trees::ContractProofFactKind::Requires
                && term.name == projection.term.as_str())
            .then_some((handle, term))
        })
        .collect::<Vec<_>>();
    let [(term_handle, term)] = matching_terms.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` evidence projection `{}.{}` resolves to {} checked source terms; expected one",
            context.subject_kind,
            context.subject_name,
            projection.term,
            projection.member,
            matching_terms.len()
        ))]);
    };
    let Some(checked_interface) = term.evidence_interface.as_ref() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` evidence projection `{}.{}` has no exact checked source interface",
            context.subject_kind, context.subject_name, projection.term, projection.member
        ))]);
    };
    let matching_requirements = checked_interface
        .requirements
        .iter()
        .filter(|requirement| {
            compilation.symbols.name(requirement.requirement) == projection.member.as_str()
        })
        .collect::<Vec<_>>();
    let [checked_requirement] = matching_requirements.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` evidence projection `{}.{}` resolves to {} checked requirement rows; expected one",
            context.subject_kind,
            context.subject_name,
            projection.term,
            projection.member,
            matching_requirements.len()
        ))]);
    };
    if !compilation
        .facts
        .proof
        .proposition_vocabulary
        .applications
        .iter()
        .flat_map(|application| &application.binder_arguments)
        .filter_map(|argument| argument.evidence_projection.as_ref())
        .any(|retained| {
            retained.term == *term_handle
                && retained.declaring_trait == checked_requirement.declaring_trait
                && retained.requirement == checked_requirement.requirement
        })
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` evidence projection `{}.{}` has no retained checked projection row",
            context.subject_kind, context.subject_name, projection.term, projection.member
        ))]);
    }

    let declaration = compilation
        .propositions()
        .iter()
        .find(|candidate| candidate.symbol == term.proposition.declaration)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "reviewed {} `{}` evidence projection `{}.{}` has an unresolved source proposition endpoint",
                context.subject_kind, context.subject_name, projection.term, projection.member
            ))]
        })?;
    let psi_typed_trees::proposition::PropositionBody::Witness { evidence } = &declaration.body
    else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` evidence projection `{}.{}` does not originate from witness evidence",
            context.subject_kind, context.subject_name, projection.term, projection.member
        ))]);
    };
    let proposition_binders = compilation
        .proposition_binders(declaration)
        .iter()
        .enumerate()
        .map(|(position, binder)| (binder.symbol, format!("proposition-binder:{position}")))
        .collect::<Vec<_>>();
    let interface = project_evidence_interface(compilation, *evidence, &proposition_binders)?;
    if nominal_identity(compilation, checked_interface.trait_symbol)? != interface.trait_identity {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` evidence projection `{}.{}` changed source interface during checked lowering",
            context.subject_kind, context.subject_name, projection.term, projection.member
        ))]);
    }
    let declaring_trait = nominal_identity(compilation, checked_requirement.declaring_trait)?;
    let requirement = nominal_identity(compilation, checked_requirement.requirement)?;
    let matching_projected = interface
        .requirements
        .iter()
        .filter(|candidate| {
            candidate.declaring_trait == declaring_trait && candidate.requirement == requirement
        })
        .collect::<Vec<_>>();
    let [projected_requirement] = matching_projected.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` evidence projection `{}.{}` resolves to {} structural interface rows; expected one",
            context.subject_kind,
            context.subject_name,
            projection.term,
            projection.member,
            matching_projected.len()
        ))]);
    };
    Ok(PackageReviewPropositionBinderValue::EvidenceProjection {
        source_kind: PackageReviewContractKind::Requires,
        source_lane_position: portable_parameter_position(term.lane_position)?,
        declaring_trait,
        declaring_trait_arguments: projected_requirement.declaring_trait_arguments.clone(),
        requirement,
    })
}

fn proposition_binder_value_expression(
    argument: &PackageReviewPropositionBinderArgument,
) -> Option<PackageReviewContractExpression> {
    match &argument.value {
        PackageReviewPropositionBinderValue::Nominal(identity) => {
            Some(PackageReviewContractExpression::Nominal(identity.clone()))
        }
        PackageReviewPropositionBinderValue::GenericBinder(position) => {
            Some(PackageReviewContractExpression::GenericBinder(*position))
        }
        PackageReviewPropositionBinderValue::Integer(value) => {
            Some(PackageReviewContractExpression::Integer(value.clone()))
        }
        PackageReviewPropositionBinderValue::EvidenceProjection { .. } => None,
    }
}

fn project_proposition_endpoint(
    compilation: &CheckedCompilation,
    declaration: &psi_typed_trees::proposition::PropositionDefinition,
    binder_arguments: Vec<PackageReviewPropositionBinderArgument>,
    arguments: Vec<PackageReviewContractExpression>,
) -> Result<PackageReviewPropositionApplication, Vec<Diagnostic>> {
    use psi_typed_trees::proposition::{PropositionBinderKind, PropositionBody};

    let declaration_binders = compilation.proposition_binders(declaration);
    let binder_symbols = declaration_binders
        .iter()
        .enumerate()
        .map(|(position, binder)| (binder.symbol, format!("proposition-binder:{position}")))
        .collect::<Vec<_>>();
    let binders = declaration_binders
        .iter()
        .map(|binder| {
            Ok(PackageReviewPropositionBinder {
                kind: match binder.kind {
                    PropositionBinderKind::Type => PackageReviewPropositionBinderKind::Type,
                    PropositionBinderKind::Const { type_reference } => {
                        PackageReviewPropositionBinderKind::Const(
                            review_type_identity_with_binders(
                                compilation,
                                type_reference,
                                &binder_symbols,
                            ),
                        )
                    }
                    PropositionBinderKind::Machine => PackageReviewPropositionBinderKind::Machine,
                },
                bounds: binder.bounds,
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    let parameter_types = compilation
        .proposition_parameters(declaration)
        .iter()
        .map(|parameter| {
            review_type_identity_with_binders(
                compilation,
                parameter.type_reference,
                &binder_symbols,
            )
        })
        .collect();
    let evidence = match declaration.body {
        PropositionBody::Primitive => PackageReviewPropositionEvidence::FactOnly,
        PropositionBody::Witness { evidence } => PackageReviewPropositionEvidence::Witness(
            project_evidence_interface(compilation, evidence, &binder_symbols)?,
        ),
        PropositionBody::Transparent { .. } => unreachable!("transparent endpoint was expanded"),
    };
    Ok(PackageReviewPropositionApplication {
        declaration: nominal_identity(compilation, declaration.symbol)?,
        binders,
        parameter_types,
        binder_arguments,
        arguments,
        evidence,
    })
}

fn project_evidence_interface(
    compilation: &CheckedCompilation,
    evidence: psi_typed_trees::types::TypeReferenceHandle,
    proposition_binders: &[(SymbolHandle, String)],
) -> Result<PackageReviewEvidenceInterface, Vec<Diagnostic>> {
    use psi_typed_trees::types::TypeReferenceNode;

    let (trait_symbol, arguments) = match compilation.type_reference_table.type_reference(evidence)
    {
        TypeReferenceNode::Named { symbol, .. } => (*symbol, Vec::new()),
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => (
            *base_symbol,
            compilation
                .type_reference_table
                .type_reference_handles(*arguments)
                .to_vec(),
        ),
        _ => {
            return Err(vec![Diagnostic::error(
                "reviewed witness proposition uses a non-nominal evidence interface",
            )]);
        }
    };
    let definition = compilation
        .traits()
        .iter()
        .find(|candidate| candidate.symbol == trait_symbol)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "reviewed witness proposition has an unresolved evidence trait",
            )]
        })?;
    if !definition.lifetime_parameters.is_empty() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed witness proposition uses lifetime-parameterized evidence trait `{}` without retained lifetime arguments",
            definition.name
        ))]);
    }
    let projected_arguments = arguments
        .iter()
        .map(|argument| {
            review_type_identity_with_binders_and_substitutions(
                compilation,
                *argument,
                proposition_binders,
                &[],
            )
        })
        .collect::<Vec<_>>();
    let mut requirements = Vec::new();
    collect_evidence_requirements(
        compilation,
        trait_symbol,
        &arguments,
        proposition_binders,
        &[],
        &mut Vec::new(),
        &mut requirements,
    )?;
    requirements.sort();
    requirements.dedup();
    Ok(PackageReviewEvidenceInterface {
        trait_identity: nominal_identity(compilation, trait_symbol)?,
        arguments: projected_arguments,
        requirements,
    })
}

fn collect_evidence_requirements(
    compilation: &CheckedCompilation,
    trait_symbol: SymbolHandle,
    trait_arguments: &[psi_typed_trees::types::TypeReferenceHandle],
    proposition_binders: &[(SymbolHandle, String)],
    inherited_substitutions: &[(SymbolHandle, psi_typed_trees::types::TypeReferenceHandle)],
    visited: &mut Vec<(PackageReviewNominalIdentity, Vec<PackageReviewTypeIdentity>)>,
    requirements: &mut Vec<PackageReviewEvidenceRequirement>,
) -> Result<(), Vec<Diagnostic>> {
    let definition = compilation
        .traits()
        .iter()
        .find(|candidate| candidate.symbol == trait_symbol)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "reviewed evidence interface inherits an unresolved trait",
            )]
        })?;
    if !definition.lifetime_parameters.is_empty() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed evidence interface inherits lifetime-parameterized trait `{}`",
            definition.name
        ))]);
    }
    let type_parameters = compilation.trait_type_parameters(definition);
    if type_parameters.len() != trait_arguments.len() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed evidence trait `{}` has inconsistent instantiated arity",
            definition.name
        ))]);
    }
    if type_parameters.iter().any(|parameter| {
        !matches!(
            parameter.kind,
            psi_typed_trees::data::TypeParameterKind::Type
                | psi_typed_trees::data::TypeParameterKind::Const { .. }
        )
    }) {
        return Err(vec![Diagnostic::error(format!(
            "reviewed evidence trait `{}` uses a static machine or proposition parameter not yet represented by package review",
            definition.name
        ))]);
    }
    let argument_identities = trait_arguments
        .iter()
        .map(|argument| {
            review_type_identity_with_binders_and_substitutions(
                compilation,
                *argument,
                proposition_binders,
                inherited_substitutions,
            )
        })
        .collect::<Vec<_>>();
    let visit = (
        nominal_identity(compilation, trait_symbol)?,
        argument_identities.clone(),
    );
    if visited.contains(&visit) {
        return Ok(());
    }
    visited.push(visit);

    for requirement in compilation.trait_machine_signatures(definition) {
        requirements.push(PackageReviewEvidenceRequirement {
            declaring_trait: nominal_identity(compilation, trait_symbol)?,
            declaring_trait_arguments: argument_identities.clone(),
            requirement: nominal_identity(compilation, requirement.symbol)?,
        });
    }

    let mut substitutions = inherited_substitutions.to_vec();
    substitutions.extend(
        type_parameters
            .iter()
            .zip(trait_arguments)
            .map(|(parameter, argument)| (parameter.symbol, *argument)),
    );
    for parent in compilation.trait_requirements(definition) {
        if !parent.lifetime_arguments.is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "reviewed evidence trait `{}` has a parent with lifetime arguments not yet represented by package review",
                definition.name
            ))]);
        }
        let parent_arguments = compilation
            .type_reference_table
            .type_reference_handles(parent.arguments);
        collect_evidence_requirements(
            compilation,
            parent.symbol,
            parent_arguments,
            proposition_binders,
            &substitutions,
            visited,
            requirements,
        )?;
    }
    Ok(())
}

fn project_contract_expression(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
    expression: psi_typed_trees::expression::ExpressionHandle,
    checked_fact: Option<psi_arena::Handle<psi_typed_trees::domain::ProofFact>>,
    depth: usize,
) -> Result<PackageReviewContractExpression, Vec<Diagnostic>> {
    project_contract_expression_with_substitutions(
        compilation,
        context,
        binders,
        expression,
        &[],
        checked_fact,
        depth,
    )
}

fn project_contract_expression_with_substitutions(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
    expression: psi_typed_trees::expression::ExpressionHandle,
    substitutions: &[(SymbolHandle, PackageReviewContractExpression)],
    checked_fact: Option<psi_arena::Handle<psi_typed_trees::domain::ProofFact>>,
    depth: usize,
) -> Result<PackageReviewContractExpression, Vec<Diagnostic>> {
    use psi_typed_trees::expression::ExpressionNode;

    if depth >= 256 {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract expression exceeds the package-review depth limit",
            context.subject_kind, context.subject_name
        ))]);
    }
    let child = |expression| {
        project_contract_expression_with_substitutions(
            compilation,
            context,
            binders,
            expression,
            substitutions,
            checked_fact,
            depth + 1,
        )
    };
    match compilation.expression_table.expression(expression) {
        ExpressionNode::Boolean(value) => Ok(PackageReviewContractExpression::Boolean(*value)),
        ExpressionNode::Integer(value) => Ok(PackageReviewContractExpression::Integer(
            value.text().to_owned(),
        )),
        ExpressionNode::Binary(binary) => Ok(PackageReviewContractExpression::Binary {
            operator: project_contract_binary_operator(binary.operator),
            left: Box::new(child(binary.left)?),
            right: Box::new(child(binary.right)?),
        }),
        ExpressionNode::Unary(unary) => Ok(PackageReviewContractExpression::Unary {
            operator: project_contract_unary_operator(unary.operator),
            operand: Box::new(child(unary.operand)?),
        }),
        ExpressionNode::Name(path) => project_contract_name_expression(
            compilation,
            context,
            binders,
            expression,
            path,
            substitutions,
            checked_fact,
        ),
        ExpressionNode::Member(_) => {
            let Some(checked_fact) = checked_fact else {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` uses a proposition-argument member expression without an exact checked place join",
                    context.subject_kind, context.subject_name
                ))]);
            };
            let Some((root_expression, source_members)) =
                contract_member_path_source(compilation, expression)
            else {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` uses a computed member expression not yet represented by package review",
                    context.subject_kind, context.subject_name
                ))]);
            };
            let root = contract_member_path_root(compilation, context, root_expression)
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "reviewed {} `{}` contract member path has no exact semantic root",
                        context.subject_kind, context.subject_name
                    ))]
                })?;
            let receiver = child(root_expression)?;
            checked_contract_member_path(
                compilation,
                context,
                checked_fact,
                expression,
                root,
                &source_members,
            )?
            .into_iter()
            .try_fold(receiver, |receiver, (case_variant, member_symbol)| {
                project_contract_member_expression(
                    compilation,
                    context,
                    receiver,
                    member_symbol,
                    case_variant,
                )
            })
        }
        ExpressionNode::Cast(cast) => {
            let semantic_domain = if cast.semantic_domain_symbol.is_valid() {
                let domain = compilation
                    .domain_definitions()
                    .iter()
                    .find(|domain| domain.symbol == cast.semantic_domain_symbol)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(format!(
                            "reviewed {} `{}` cast refers to an unresolved semantic domain",
                            context.subject_kind, context.subject_name
                        ))]
                    })?;
                let identity = nominal_identity(compilation, domain.symbol)?;
                let reviewed_package = compilation.package_identity().ok_or_else(|| {
                    vec![Diagnostic::error(
                        "package review requires package-aware checked compilation",
                    )]
                })?;
                if reviewed_package_owns(&identity, reviewed_package)? && !domain.is_public {
                    return Err(vec![Diagnostic::error(format!(
                        "reviewed {} `{}` exposes non-public semantic domain `{}` in a cast",
                        context.subject_kind, context.subject_name, domain.name
                    ))]);
                }
                Some(identity)
            } else {
                None
            };
            Ok(PackageReviewContractExpression::Cast {
                value: Box::new(child(cast.value)?),
                target: review_type_identity_with_binders(compilation, cast.target_type, binders),
                arithmetic_domain: match cast.domain {
                    psi_numerics::arithmetic::ArithmeticDomain::Exact => {
                        PackageReviewArithmeticDomain::Exact
                    }
                    psi_numerics::arithmetic::ArithmeticDomain::Wrapping => {
                        PackageReviewArithmeticDomain::Wrapping
                    }
                    psi_numerics::arithmetic::ArithmeticDomain::Saturating => {
                        PackageReviewArithmeticDomain::Saturating
                    }
                    psi_numerics::arithmetic::ArithmeticDomain::Trapping => {
                        PackageReviewArithmeticDomain::Trapping
                    }
                },
                semantic_domain,
                semantic_domain_arguments: compilation
                    .type_reference_table
                    .type_reference_handles(cast.semantic_domain_arguments)
                    .iter()
                    .map(|argument| {
                        review_type_identity_with_binders(compilation, *argument, binders)
                    })
                    .collect(),
                form: match cast.form {
                    psi_language_core::cast_form::CastForm::Value => PackageReviewCastForm::Value,
                    psi_language_core::cast_form::CastForm::RecastShared => {
                        PackageReviewCastForm::RecastShared
                    }
                    psi_language_core::cast_form::CastForm::RecastMutable => {
                        PackageReviewCastForm::RecastMutable
                    }
                },
            })
        }
        _ => Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` uses a contract expression form not yet represented by package review",
            context.subject_kind, context.subject_name
        ))]),
    }
}

fn project_contract_name_expression(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
    expression: psi_typed_trees::expression::ExpressionHandle,
    path: &psi_typed_trees::expression::TableNamePath,
    substitutions: &[(SymbolHandle, PackageReviewContractExpression)],
    checked_fact: Option<psi_arena::Handle<psi_typed_trees::domain::ProofFact>>,
) -> Result<PackageReviewContractExpression, Vec<Diagnostic>> {
    let members = compilation.expression_table.name_path_members(path.members);
    let root_symbol = path.head_symbol;
    let root_name = members.first();
    let substitution_root = substitutions
        .iter()
        .rev()
        .find_map(|(symbol, substitution)| {
            (*symbol == root_symbol || (members.len() == 1 && *symbol == path.symbol))
                .then(|| substitution.clone())
        });
    let parameter_position = context.parameters.iter().position(|parameter| {
        parameter.symbol == root_symbol || root_name.is_some_and(|name| name == &parameter.name)
    });
    let is_domain_subject =
        context.domain_symbol.is_some() && root_name.is_some_and(|name| name.as_str() == "self");
    let binder_position = binders
        .iter()
        .position(|(symbol, _)| *symbol == root_symbol);
    let root = if let Some(substitution) = substitution_root {
        Some(substitution)
    } else if is_domain_subject {
        Some(PackageReviewContractExpression::DomainSubject)
    } else if let Some(position) = parameter_position {
        Some(PackageReviewContractExpression::Parameter(
            portable_parameter_position(position)?,
        ))
    } else if root_name.is_some_and(|name| name.as_str() == "result") {
        Some(PackageReviewContractExpression::Result)
    } else if let Some(position) = binder_position {
        Some(PackageReviewContractExpression::GenericBinder(
            portable_parameter_position(position)?,
        ))
    } else {
        None
    };

    let Some(projected) = root else {
        if !path.symbol.is_valid() {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` contract contains an unresolved name expression",
                context.subject_kind, context.subject_name
            ))]);
        }
        return nominal_identity(compilation, path.symbol)
            .map(PackageReviewContractExpression::Nominal);
    };
    if members.len() == 1 {
        return Ok(projected);
    }
    let Some(checked_fact) = checked_fact else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` uses a proposition-argument name-path member without an exact checked place join",
            context.subject_kind, context.subject_name
        ))]);
    };
    let semantic_root = is_domain_subject
        .then_some(psi_facts::PlaceRoot::Expression(expression))
        .or_else(|| {
            parameter_position
                .map(|position| psi_facts::PlaceRoot::Symbol(context.parameters[position].symbol))
        })
        .or_else(|| {
            root_symbol
                .is_valid()
                .then_some(psi_facts::PlaceRoot::Symbol(root_symbol))
        })
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "reviewed {} `{}` contract name-path member has no exact semantic root",
                context.subject_kind, context.subject_name
            ))]
        })?;
    checked_contract_member_path(
        compilation,
        context,
        checked_fact,
        expression,
        semantic_root,
        &members[1..],
    )?
    .into_iter()
    .try_fold(projected, |receiver, (case_variant, member_symbol)| {
        project_contract_member_expression(
            compilation,
            context,
            receiver,
            member_symbol,
            case_variant,
        )
    })
}

fn project_contract_member_expression(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    receiver: PackageReviewContractExpression,
    member_symbol: SymbolHandle,
    case_variant_symbol: Option<SymbolHandle>,
) -> Result<PackageReviewContractExpression, Vec<Diagnostic>> {
    if !member_symbol.is_valid() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract contains an unresolved member expression",
            context.subject_kind, context.subject_name
        ))]);
    }
    Ok(PackageReviewContractExpression::Member {
        receiver: Box::new(receiver),
        member: nominal_identity(compilation, member_symbol)?,
        case_variant: case_variant_symbol
            .map(|symbol| nominal_identity(compilation, symbol))
            .transpose()?,
    })
}

fn contract_member_path_source(
    compilation: &CheckedCompilation,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Option<(
    psi_typed_trees::expression::ExpressionHandle,
    Vec<psi_typed_trees::name::Identifier>,
)> {
    use psi_typed_trees::expression::ExpressionNode;

    match compilation.expression_table.expression(expression) {
        ExpressionNode::Member(member) => {
            let (root, mut members) = contract_member_path_source(compilation, member.receiver)?;
            members.push(member.member.clone());
            Some((root, members))
        }
        ExpressionNode::Name(path)
            if compilation
                .expression_table
                .name_path_members(path.members)
                .len()
                == 1 =>
        {
            Some((expression, Vec::new()))
        }
        _ => None,
    }
}

fn contract_member_path_root(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Option<psi_facts::PlaceRoot> {
    let psi_typed_trees::expression::ExpressionNode::Name(path) =
        compilation.expression_table.expression(expression)
    else {
        return None;
    };
    let resolved = path
        .head_symbol
        .is_valid()
        .then_some(path.head_symbol)
        .or_else(|| path.symbol.is_valid().then_some(path.symbol));
    if let Some(symbol) = resolved {
        return Some(psi_facts::PlaceRoot::Symbol(symbol));
    }
    let [name] = compilation.expression_table.name_path_members(path.members) else {
        return None;
    };
    if context.domain_symbol.is_some() && name.as_str() == "self" {
        return Some(psi_facts::PlaceRoot::Expression(expression));
    }
    context
        .parameters
        .iter()
        .find(|parameter| parameter.name == *name)
        .map(|parameter| psi_facts::PlaceRoot::Symbol(parameter.symbol))
}

fn checked_contract_member_path(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    checked_fact: psi_arena::Handle<psi_typed_trees::domain::ProofFact>,
    expression: psi_typed_trees::expression::ExpressionHandle,
    root: psi_facts::PlaceRoot,
    source_members: &[psi_typed_trees::name::Identifier],
) -> Result<Vec<(Option<SymbolHandle>, SymbolHandle)>, Vec<Diagnostic>> {
    use psi_facts::{FactPayload, FactPlace};

    if let Some(domain_symbol) = context.domain_symbol {
        let mut candidates = Vec::new();
        for (_, record) in compilation
            .facts
            .semantic
            .domain_definition_facts
            .iter()
            .filter(|(_, record)| {
                record.domain_symbol == domain_symbol && record.fact == checked_fact
            })
        {
            for dependency in record
                .dependencies
                .iter()
                .filter(|dependency| dependency.expression == expression)
            {
                let Some((_, place)) = compilation
                    .facts
                    .semantic
                    .places
                    .iter()
                    .find(|(handle, _)| *handle == dependency.place)
                else {
                    continue;
                };
                if place.root != root {
                    continue;
                }
                if let Some(selected) =
                    checked_member_segments(compilation, place.segments, source_members)
                {
                    candidates.push(selected);
                }
            }
        }
        let [selected] = candidates.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` contract member path resolves to {} exact checked dependency records; expected one",
                context.subject_kind,
                context.subject_name,
                candidates.len()
            ))]);
        };
        return Ok(selected.clone());
    }

    let mut candidates = Vec::new();
    for (_, semantic_fact) in compilation.facts.semantic.facts.iter() {
        let contract_fact_matches = matches!(
            semantic_fact.payload,
            FactPayload::ContractBooleanExpression { fact, .. }
                | FactPayload::ContractDomainMembership { fact, .. }
                if fact == checked_fact
        );
        if semantic_fact.point != context.point || !contract_fact_matches {
            continue;
        }
        let FactPlace::Place(place_handle) = semantic_fact.place else {
            continue;
        };
        let place = compilation.facts.semantic.places.get(place_handle);
        if place.root != root {
            continue;
        }
        let Some(selected) = checked_member_segments(compilation, place.segments, source_members)
        else {
            continue;
        };
        if !candidates.contains(&selected) {
            candidates.push(selected);
        }
    }
    let [selected] = candidates.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract member path resolves to {} exact checked place rows; expected one",
            context.subject_kind,
            context.subject_name,
            candidates.len()
        ))]);
    };
    Ok(selected.clone())
}

fn checked_member_segments(
    compilation: &CheckedCompilation,
    segments: psi_arena::HandleSpan<psi_facts::PlaceSegment>,
    source_members: &[psi_typed_trees::name::Identifier],
) -> Option<Vec<(Option<SymbolHandle>, SymbolHandle)>> {
    use psi_facts::PlaceSegment;

    let mut selected = Vec::new();
    let mut pending_case = None;
    for segment in compilation
        .facts
        .semantic
        .place_segments
        .span_or_empty(segments)
    {
        match *segment {
            PlaceSegment::Case { variant } if pending_case.is_none() => {
                pending_case = Some(variant);
            }
            PlaceSegment::Field { symbol } if symbol.is_valid() => {
                selected.push((pending_case.take(), symbol));
            }
            _ => return None,
        }
    }
    if pending_case.is_some() || selected.len() != source_members.len() {
        return None;
    }
    if selected
        .iter()
        .zip(source_members)
        .any(|((_, symbol), name)| compilation.symbols.name(*symbol) != name.as_str())
    {
        return None;
    }
    Some(selected)
}

fn portable_parameter_position(position: usize) -> Result<u32, Vec<Diagnostic>> {
    u32::try_from(position).map_err(|_| {
        vec![Diagnostic::error(
            "package review contract parameter ordinal exceeds the portable identity range",
        )]
    })
}

const fn project_contract_binary_operator(
    operator: psi_typed_trees::expression::BinaryOperator,
) -> PackageReviewContractBinaryOperator {
    use psi_typed_trees::expression::BinaryOperator;
    match operator {
        BinaryOperator::Add => PackageReviewContractBinaryOperator::Add,
        BinaryOperator::And => PackageReviewContractBinaryOperator::And,
        BinaryOperator::BitwiseAnd => PackageReviewContractBinaryOperator::BitwiseAnd,
        BinaryOperator::BitwiseOr => PackageReviewContractBinaryOperator::BitwiseOr,
        BinaryOperator::BitwiseXor => PackageReviewContractBinaryOperator::BitwiseXor,
        BinaryOperator::Divide => PackageReviewContractBinaryOperator::Divide,
        BinaryOperator::Equal => PackageReviewContractBinaryOperator::Equal,
        BinaryOperator::Greater => PackageReviewContractBinaryOperator::Greater,
        BinaryOperator::GreaterOrEqual => PackageReviewContractBinaryOperator::GreaterOrEqual,
        BinaryOperator::Less => PackageReviewContractBinaryOperator::Less,
        BinaryOperator::LessOrEqual => PackageReviewContractBinaryOperator::LessOrEqual,
        BinaryOperator::Modulo => PackageReviewContractBinaryOperator::Modulo,
        BinaryOperator::Multiply => PackageReviewContractBinaryOperator::Multiply,
        BinaryOperator::NotEqual => PackageReviewContractBinaryOperator::NotEqual,
        BinaryOperator::Or => PackageReviewContractBinaryOperator::Or,
        BinaryOperator::ShiftLeft => PackageReviewContractBinaryOperator::ShiftLeft,
        BinaryOperator::ShiftRight => PackageReviewContractBinaryOperator::ShiftRight,
        BinaryOperator::Subtract => PackageReviewContractBinaryOperator::Subtract,
    }
}

const fn project_contract_unary_operator(
    operator: psi_typed_trees::expression::UnaryOperator,
) -> PackageReviewContractUnaryOperator {
    match operator {
        psi_typed_trees::expression::UnaryOperator::BitwiseNot => {
            PackageReviewContractUnaryOperator::BitwiseNot
        }
        psi_typed_trees::expression::UnaryOperator::LogicalNot => {
            PackageReviewContractUnaryOperator::LogicalNot
        }
    }
}

fn project_callable_conformances(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
    binders: &[(SymbolHandle, String)],
) -> Result<Vec<PackageReviewCallableConformance>, Vec<Diagnostic>> {
    let mut projected = Vec::new();
    for conformance in compilation.machine_trait_conformances(machine) {
        if conformance.external_binding.is_some() {
            return Err(vec![Diagnostic::error(format!(
                "reviewed callable `{}` uses an external trait realization not yet represented by package review",
                machine.name
            ))]);
        }
        let Some(trait_definition) = compilation
            .traits()
            .iter()
            .find(|definition| definition.symbol == conformance.symbol)
        else {
            return Err(vec![Diagnostic::error(format!(
                "reviewed callable `{}` realizes an operator or unresolved trait requirement not yet represented by package review",
                machine.name
            ))]);
        };
        if !trait_definition.is_public {
            return Err(vec![Diagnostic::error(format!(
                "reviewed callable `{}` realizes non-public trait `{}` whose complete contract is absent from package review",
                machine.name, trait_definition.name
            ))]);
        }
        if !trait_definition.lifetime_parameters.is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "reviewed callable `{}` realizes lifetime-parameterized trait `{}` without retained conformance lifetime arguments",
                machine.name, trait_definition.name
            ))]);
        }
        let Some(requirement_name) = conformance.requirement.as_ref() else {
            return Err(vec![Diagnostic::error(format!(
                "reviewed callable `{}` has a trait realization without an exact requirement",
                machine.name
            ))]);
        };
        let implementation_dispatch = compilation.normalized_result_dispatch_set(
            compilation
                .machine_states(machine)
                .first()
                .expect("reviewed callable entry was checked before conformances")
                .return_type,
        );
        let named = compilation
            .trait_machine_signatures(trait_definition)
            .iter()
            .filter(|requirement| requirement.name == *requirement_name)
            .collect::<Vec<_>>();
        let matching = if named.len() == 1 {
            named
        } else {
            named
                .into_iter()
                .filter(|requirement| {
                    compilation.normalized_result_dispatch_set(requirement.return_type)
                        == implementation_dispatch
                })
                .collect()
        };
        let [requirement] = matching.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "reviewed callable `{}` trait realization `{}::{}` resolves to {} exact requirement overloads; expected one",
                machine.name,
                trait_definition.name,
                requirement_name,
                matching.len()
            ))]);
        };
        projected.push(PackageReviewCallableConformance {
            trait_identity: nominal_identity(compilation, trait_definition.symbol)?,
            requirement_identity: nominal_identity(compilation, requirement.symbol)?,
            arguments: compilation
                .type_reference_table
                .type_reference_handles(conformance.arguments)
                .iter()
                .map(|argument| {
                    review_signature_type_identity_with_binders(
                        compilation,
                        *argument,
                        binders,
                        &machine.lifetime_parameters,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
            alias: conformance
                .alias
                .as_ref()
                .map(|alias| alias.as_str().to_owned()),
        });
    }
    projected.sort();
    if projected.windows(2).any(|rows| rows[0] == rows[1]) {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` contains a duplicate exact trait realization",
            machine.name
        ))]);
    }
    Ok(projected)
}

fn project_synchronous_invocations(
    compilation: &CheckedCompilation,
    invocations: &[psi_effects::InvocationTarget],
) -> Result<Vec<PackageReviewSynchronousInvocation>, Vec<Diagnostic>> {
    let mut projected = invocations
        .iter()
        .copied()
        .map(|invocation| match invocation {
            psi_effects::InvocationTarget::Parameter(position) => {
                Ok(PackageReviewSynchronousInvocation::Parameter(position))
            }
            psi_effects::InvocationTarget::Service(symbol) => Ok(
                PackageReviewSynchronousInvocation::Service(nominal_identity(compilation, symbol)?),
            ),
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    projected.sort();
    projected.dedup();
    Ok(projected)
}

fn project_installation_reaches(
    compilation: &CheckedCompilation,
    requirements: &[psi_effects::InstallationReachRequirement],
) -> Result<Vec<PackageReviewInstallationReach>, Vec<Diagnostic>> {
    let mut projected = requirements
        .iter()
        .map(|requirement| {
            Ok(PackageReviewInstallationReach {
                requirement: installation_requirement_identity(
                    compilation,
                    requirement.requirement,
                )?,
                upper_bound: project_service_row(compilation, requirement.upper_bound)?,
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    projected.sort();
    projected.dedup();
    Ok(projected)
}

fn installation_requirement_identity(
    compilation: &CheckedCompilation,
    symbol: SymbolHandle,
) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
    let trait_matches = compilation
        .traits()
        .iter()
        .flat_map(|owner| {
            compilation
                .trait_machine_signatures(owner)
                .iter()
                .filter(move |requirement| requirement.symbol == symbol)
                .map(move |requirement| (owner, requirement))
        })
        .collect::<Vec<_>>();
    let machine_matches = compilation
        .machines()
        .iter()
        .filter(|machine| machine.symbol == symbol)
        .collect::<Vec<_>>();
    match (trait_matches.as_slice(), machine_matches.as_slice()) {
        ([(owner, requirement)], []) => Ok(PackageReviewNominalIdentity {
            owner: nominal_owner(compilation, owner.symbol)?,
            path: compilation
                .normalized_trait_requirement_overload_identity(owner, requirement)
                .identity(),
        }),
        ([], [machine])
            if machine.service_reach_is_installation_bound
                && machine.supply_mode == MachineSupplyMode::Boundary =>
        {
            let path = compilation
                .normalized_machine_overload_identity(machine)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "package review installation reach has no normalized machine overload identity",
                    )]
                })?
                .identity();
            Ok(PackageReviewNominalIdentity {
                owner: nominal_owner(compilation, machine.symbol)?,
                path,
            })
        }
        _ => Err(vec![Diagnostic::error(format!(
            "package review installation reach resolves to {} trait requirements and {} boundary machines; expected exactly one",
            trait_matches.len(),
            machine_matches.len(),
        ))]),
    }
}

fn project_mutation(
    compilation: &CheckedCompilation,
    plans: &[psi_checked_trees::StateWriteFramePlan],
) -> Result<Vec<PackageReviewMutation>, Vec<Diagnostic>> {
    let mut projected = plans
        .iter()
        .map(|plan| {
            Ok(PackageReviewMutation {
                state: nominal_identity(compilation, plan.state)?,
                completeness: plan.frame.completeness(),
                paths: plan.frame.paths().to_vec(),
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    projected.sort_by(|left, right| {
        left.state
            .cmp(&right.state)
            .then_with(|| {
                mutation_completeness_tag(left.completeness)
                    .cmp(&mutation_completeness_tag(right.completeness))
            })
            .then_with(|| left.paths.cmp(&right.paths))
    });
    projected.dedup();
    Ok(projected)
}

fn project_termination(
    compilation: &CheckedCompilation,
    guarantee: &psi_language_semantics::TerminationGuarantee,
) -> Result<PackageReviewTermination, Vec<Diagnostic>> {
    project_termination_with_subject(compilation, guarantee, |root| {
        nominal_identity(compilation, root).map(PackageReviewProgressSubject::Declaration)
    })
}

fn project_trait_requirement_termination(
    compilation: &CheckedCompilation,
    requirement: &psi_typed_trees::signature::StateSignature,
) -> Result<PackageReviewTermination, Vec<Diagnostic>> {
    let parameters = compilation.state_signature_parameters(requirement);
    if let psi_language_semantics::TerminationGuarantee::Terminates { premises } =
        &requirement.termination_guarantee
    {
        for premise in premises {
            let profile = compilation
                .domain_definitions()
                .iter()
                .find(|domain| domain.semantic_id == premise.profile)
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "public trait requirement `{}` has an unknown termination profile",
                        requirement.name
                    ))]
                })?;
            if !profile.is_public {
                return Err(vec![Diagnostic::error(format!(
                    "public trait requirement `{}` exposes non-public progress profile `{}`",
                    requirement.name, profile.name
                ))]);
            }
        }
    }
    project_termination_with_subject(compilation, &requirement.termination_guarantee, |root| {
        let parameter = parameters
                .iter()
                .find(|parameter| parameter.symbol == root)
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "public trait requirement `{}` has a termination premise outside its parameter telescope",
                        requirement.name
                    ))]
                })?;
        if parameter.is_self {
            return Ok(PackageReviewProgressSubject::Receiver);
        }
        let position = parameters
            .iter()
            .filter(|candidate| !candidate.is_self)
            .position(|candidate| candidate.symbol == root)
            .expect("matched non-self requirement parameter must have an ordinal");
        let position = u32::try_from(position).map_err(|_| {
                vec![Diagnostic::error(format!(
                    "public trait requirement `{}` has too many parameters for portable review evidence",
                    requirement.name
                ))]
            })?;
        Ok(PackageReviewProgressSubject::Parameter(position))
    })
}

fn project_termination_with_subject(
    compilation: &CheckedCompilation,
    guarantee: &psi_language_semantics::TerminationGuarantee,
    mut project_subject: impl FnMut(
        SymbolHandle,
    ) -> Result<PackageReviewProgressSubject, Vec<Diagnostic>>,
) -> Result<PackageReviewTermination, Vec<Diagnostic>> {
    let psi_language_semantics::TerminationGuarantee::Terminates { premises } = guarantee else {
        return Ok(PackageReviewTermination::NoGuarantee);
    };
    let mut projected = premises
        .iter()
        .map(|premise| {
            let profile = compilation
                .domain_definitions()
                .iter()
                .find(|domain| domain.semantic_id == premise.profile)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "package review termination premise has an unknown progress-profile identity",
                    )]
                })?;
            if profile.classification
                != Some(psi_language_semantics::DomainClassification::ProgressProfile)
            {
                return Err(vec![Diagnostic::error(
                    "package review termination premise does not name a closed progress-profile domain",
                )]);
            }
            let profile = nominal_identity(compilation, profile.symbol)?;
            let subject = project_subject(premise.subject.root)?;
            let projections = premise
                .subject
                .projections
                .iter()
                .map(|projection| nominal_identity(compilation, *projection))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(PackageReviewProgressPremise {
                profile,
                subject,
                projections,
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    projected.sort();
    projected.dedup();
    Ok(PackageReviewTermination::Terminates {
        premises: projected,
    })
}

fn project_crash(
    compilation: &CheckedCompilation,
    plan: &psi_checked_trees::CrashPlan,
) -> Result<PackageReviewCrash, Vec<Diagnostic>> {
    let interface = match plan.interface() {
        psi_checked_trees::CrashInterface::InternalInferred => {
            PackageReviewCrashInterface::InternalInferred
        }
        psi_checked_trees::CrashInterface::PublishedCeiling => {
            PackageReviewCrashInterface::PublishedCeiling
        }
    };
    let published = project_crash_routes(plan.published());
    let mut checked_sites = plan
        .checked_sites()
        .iter()
        .map(|site| {
            let location = site.location();
            let mut frontier_lower_bound = site
                .frontier_lower_bound()
                .iter()
                .map(|claim| project_permission_claim(compilation, *claim))
                .collect::<Result<Vec<_>, _>>()?;
            frontier_lower_bound.sort();
            frontier_lower_bound.dedup();
            Ok(PackageReviewCrashSite {
                state: nominal_identity(compilation, location.state())?,
                statement_ordinal: location.statement_ordinal(),
                cause: site.cause(),
                path_guard_conjuncts: project_crash_predicates(site.path_guard_conjuncts()),
                path_guard_consequences: project_crash_predicates(site.path_guard_consequences()),
                guard_covering_buckets: site
                    .guard_covering_buckets()
                    .iter()
                    .map(|bucket| bucket.get())
                    .collect(),
                frontier_lower_bound,
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    checked_sites.sort();
    checked_sites.dedup();

    let mut checked_calls = plan
        .checked_calls()
        .iter()
        .map(|call| {
            let location = call.location();
            Ok(PackageReviewCrashCall {
                state: nominal_identity(compilation, location.state())?,
                statement_ordinal: location.statement_ordinal(),
                call_ordinal: location.call_ordinal(),
                target_machine: nominal_identity(compilation, call.target_machine())?,
                target_state: nominal_identity(compilation, call.target_state())?,
                path_guard_conjuncts: project_crash_predicates(call.path_guard_conjuncts()),
                path_guard_consequences: project_crash_predicates(call.path_guard_consequences()),
                surviving_buckets: project_crash_routes(call.surviving_buckets()),
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    checked_calls.sort();
    checked_calls.dedup();

    Ok(PackageReviewCrash {
        interface,
        published,
        structural_runtime_requirements: plan.structural_runtime_requirements().map(<[_]>::to_vec),
        checked_sites,
        checked_calls,
    })
}

fn project_crash_routes(
    routes: &[psi_checked_trees::CrashRouteBucket],
) -> Vec<PackageReviewCrashRoute> {
    let mut projected = routes
        .iter()
        .map(|route| PackageReviewCrashRoute {
            cause: route.cause(),
            alternative_guards: route
                .alternative_guards()
                .iter()
                .map(|guard| match guard {
                    psi_checked_trees::CrashRouteGuard::Truth => {
                        PackageReviewCrashRouteGuard::Truth
                    }
                    psi_checked_trees::CrashRouteGuard::Predicate(predicate) => {
                        PackageReviewCrashRouteGuard::Predicate(project_crash_predicate(predicate))
                    }
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    projected.sort();
    projected.dedup();
    projected
}

fn project_crash_predicates(
    predicates: &[psi_checked_trees::CrashPredicateIdentity],
) -> Vec<PackageReviewCrashPredicate> {
    let mut projected = predicates
        .iter()
        .map(project_crash_predicate)
        .collect::<Vec<_>>();
    projected.sort();
    projected.dedup();
    projected
}

fn project_crash_predicate(
    predicate: &psi_checked_trees::CrashPredicateIdentity,
) -> PackageReviewCrashPredicate {
    PackageReviewCrashPredicate {
        canonical_bytes: predicate.canonical_bytes().to_vec(),
    }
}

fn project_permission_claim(
    compilation: &CheckedCompilation,
    claim: psi_language_semantics::PermissionClaimIdentity,
) -> Result<PackageReviewPermissionClaim, Vec<Diagnostic>> {
    let psi_language_semantics::PermissionClaimIdentity::Established {
        machine_symbol,
        state_symbol,
        source,
        ordinal,
    } = claim
    else {
        return Err(vec![Diagnostic::error(
            "package review crash frontier contains an unidentified permission claim",
        )]);
    };
    let source = match source {
        psi_language_semantics::PermissionEventSource::StateEntry => {
            PackageReviewPermissionSource::StateEntry
        }
        psi_language_semantics::PermissionEventSource::Statement { statement_index } => {
            PackageReviewPermissionSource::Statement {
                statement_ordinal: portable_ordinal(statement_index)?,
            }
        }
        psi_language_semantics::PermissionEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => PackageReviewPermissionSource::Call {
            statement_ordinal: portable_ordinal(statement_index)?,
            call_ordinal: portable_ordinal(call_ordinal)?,
            target: nominal_identity(compilation, target_symbol)?,
        },
        psi_language_semantics::PermissionEventSource::StateExit => {
            PackageReviewPermissionSource::StateExit
        }
    };
    Ok(PackageReviewPermissionClaim {
        machine: nominal_identity(compilation, machine_symbol)?,
        state: nominal_identity(compilation, state_symbol)?,
        source,
        ordinal,
    })
}

fn portable_ordinal(ordinal: usize) -> Result<u64, Vec<Diagnostic>> {
    u64::try_from(ordinal).map_err(|_| {
        vec![Diagnostic::error(
            "package review semantic ordinal exceeds the portable identity range",
        )]
    })
}

const fn mutation_completeness_tag(completeness: psi_facts::WriteFrameCompleteness) -> u8 {
    match completeness {
        psi_facts::WriteFrameCompleteness::Complete => 1,
        psi_facts::WriteFrameCompleteness::Opaque => 2,
    }
}

fn project_service_row(
    compilation: &CheckedCompilation,
    row: psi_language_semantics::ServiceReachRowId,
) -> Result<Vec<PackageReviewNominalIdentity>, Vec<Diagnostic>> {
    let services = compilation.facts.service_reaches.rows.services(row);
    if services.is_empty() && row != psi_language_semantics::ServiceReachRowTable::EMPTY_ROW {
        return Err(vec![Diagnostic::error(
            "package review encountered an unknown service-reach row identity",
        )]);
    }
    let mut projected = services
        .iter()
        .map(|service| {
            let definition = compilation
                .facts
                .service_reaches
                .services
                .definition(*service)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "package review encountered an unknown boundary-service identity",
                    )]
                })?;
            nominal_identity(compilation, definition.symbol)
        })
        .collect::<Result<Vec<_>, _>>()?;
    projected.sort();
    projected.dedup();
    Ok(projected)
}

fn project_capability_flow(
    compilation: &CheckedCompilation,
    flow: &psi_effects::CapabilityFlowFact,
) -> Result<PackageReviewCapabilityFlow, Vec<Diagnostic>> {
    Ok(PackageReviewCapabilityFlow {
        capability: nominal_identity(compilation, flow.capability_symbol)?,
        kind: flow.kind,
        state: nominal_identity(compilation, flow.state_symbol)?,
        statement_index: flow.statement_index,
        call_ordinal: flow.call_ordinal,
        via_state: flow
            .via_state_symbol
            .is_valid()
            .then(|| nominal_identity(compilation, flow.via_state_symbol))
            .transpose()?,
    })
}

fn nominal_identity(
    compilation: &CheckedCompilation,
    symbol: SymbolHandle,
) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
    let owner = nominal_owner(compilation, symbol)?;
    let path = compilation.typed.symbols.display_path(symbol, "::");
    if path.is_empty() {
        return Err(vec![Diagnostic::error(
            "package review encountered a symbol without a stable declaration path",
        )]);
    }
    Ok(PackageReviewNominalIdentity { owner, path })
}

fn nominal_owner(
    compilation: &CheckedCompilation,
    symbol: SymbolHandle,
) -> Result<PackageReviewNominalOwner, Vec<Diagnostic>> {
    if let Some(package) = compilation.typed.symbols.symbol_package_identity(symbol) {
        return Ok(PackageReviewNominalOwner::Package(package));
    }
    let Some(source_file) = compilation
        .typed
        .symbols
        .symbol_source_span(symbol)
        .and_then(|span| compilation.typed.symbols.source_file(span))
    else {
        return Ok(PackageReviewNominalOwner::Unresolved);
    };
    match source_file.origin {
        psi_source::SourceOrigin::Toolchain => Ok(PackageReviewNominalOwner::ToolchainSource(
            toolchain_source_identity(source_file)?,
        )),
        psi_source::SourceOrigin::User => Ok(PackageReviewNominalOwner::Unresolved),
    }
}

fn toolchain_source_identity(
    source_file: &psi_source::SourceFile,
) -> Result<PackageReviewToolchainSourceIdentity, Vec<Diagnostic>> {
    let custody_entry = super::package_source_consumption::canonical_source_entry(source_file)?;
    let mut digest = Sha256::new();
    digest.update(b"OMEGA-PACKAGE-REVIEW-TOOLCHAIN-SOURCE\0");
    digest.update(
        u64::try_from(custody_entry.len())
            .expect("canonical source custody entry length fits u64")
            .to_le_bytes(),
    );
    digest.update(custody_entry);
    Ok(PackageReviewToolchainSourceIdentity {
        digest: digest.finalize().into(),
    })
}

fn is_canonical_virtual_toolchain_path(path: &std::path::Path) -> bool {
    let mut components = path.components();
    let Some(std::path::Component::Normal(component)) = components.next() else {
        return false;
    };
    if components.next().is_some() {
        return false;
    }
    component.to_str().is_some_and(|component| {
        component.len() >= 3 && component.starts_with('<') && component.ends_with('>')
    })
}

fn exactly_one<'item, Item>(
    mut matches: impl Iterator<Item = &'item Item>,
    subject: &str,
    fact_kind: &str,
) -> Result<&'item Item, Vec<Diagnostic>> {
    let first = matches.next().ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "reviewed callable `{subject}` has no exact checked {fact_kind} row"
        ))]
    })?;
    if matches.next().is_some() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{subject}` has duplicate checked {fact_kind} rows"
        ))]);
    }
    Ok(first)
}

#[cfg(test)]
mod tests {
    use super::toolchain_source_identity;
    use psi_source::{SourceFile, SourceId, SourceOrigin};
    use std::path::PathBuf;
    use std::sync::Arc;

    fn toolchain_source(relative_path: &str, source: &str) -> SourceFile {
        namespaced_toolchain_source("std", relative_path, source)
    }

    fn namespaced_toolchain_source(
        namespace: &str,
        relative_path: &str,
        source: &str,
    ) -> SourceFile {
        let package_root = PathBuf::from("toolchain").join(namespace);
        SourceFile {
            source_id: SourceId(0),
            path: package_root.join(relative_path),
            package_root,
            package_identity: None,
            origin: SourceOrigin::Toolchain,
            source: Arc::from(source),
        }
    }

    fn virtual_toolchain_source(path: &str, source: &str) -> SourceFile {
        SourceFile {
            source_id: SourceId(0),
            path: PathBuf::from(path),
            package_root: PathBuf::from("toolchain/std"),
            package_identity: None,
            origin: SourceOrigin::Toolchain,
            source: Arc::from(source),
        }
    }

    #[test]
    fn toolchain_source_identity_is_framed_over_path_and_exact_bytes() {
        let first = toolchain_source_identity(&toolchain_source("service.omg", "trait Host {}"))
            .expect("canonical toolchain source identity");
        let repeated = toolchain_source_identity(&toolchain_source("service.omg", "trait Host {}"))
            .expect("repeated canonical toolchain source identity");
        let changed_path =
            toolchain_source_identity(&toolchain_source("other.omg", "trait Host {}"))
                .expect("changed-path toolchain source identity");
        let changed_source =
            toolchain_source_identity(&toolchain_source("service.omg", "trait Host { }"))
                .expect("changed-source toolchain source identity");

        assert_eq!(first, repeated);
        assert_ne!(first, changed_path);
        assert_ne!(first, changed_source);
        assert_ne!(
            first,
            toolchain_source_identity(&namespaced_toolchain_source(
                "core",
                "service.omg",
                "trait Host {}",
            ))
            .expect("changed-namespace toolchain source identity")
        );
        assert_ne!(first.digest(), [0; 32]);
    }

    #[test]
    fn toolchain_source_identity_accepts_only_canonical_virtual_coordinates() {
        let virtual_source =
            toolchain_source_identity(&virtual_toolchain_source("<build-prelude>", "data Build"))
                .expect("canonical virtual toolchain source identity");
        assert_ne!(virtual_source.digest(), [0; 32]);

        let error = toolchain_source_identity(&virtual_toolchain_source(
            "virtual/<build-prelude>",
            "data Build",
        ))
        .expect_err("nested virtual path outside the toolchain root must reject");
        assert!(error[0].message.contains("outside its canonical root"));
    }
}
