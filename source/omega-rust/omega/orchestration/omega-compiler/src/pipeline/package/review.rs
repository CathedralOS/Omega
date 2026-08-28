//! Compiler-owned, in-memory package authority projection.
//!
//! This is deliberately a review surface, not admission evidence. Authored
//! toolchain nominals are bound to exact source commitments, but whole-source,
//! compiler/toolchain, provider-nominal, proof, and trust commitments still
//! live outside this projection.
//! Keeping the type distinct prevents an incomplete checked summary from being
//! persisted as an accepted lock baseline.

#[path = "review/encoding.rs"]
mod encoding;
#[path = "review/obligation_ledger.rs"]
mod obligation_ledger;
#[path = "review/recovery.rs"]
mod recovery;

pub use encoding::{
    PACKAGE_REVIEW_ENCODING_VERSION, PACKAGE_REVIEW_ROW_ENCODING_VERSION,
    PackageReviewEncodingError,
};
pub use obligation_ledger::{
    ORDINARY_PACKAGE_OBLIGATION_LEDGER_ENCODING_VERSION,
    ORDINARY_PACKAGE_OBLIGATION_SCHEMA_VERSION, OrdinaryPackageObligationLedger,
    OrdinaryPackageObligationLedgerFingerprint, OrdinaryPackageObligationLedgerRecoveryError,
    OrdinaryPackageObligationRow, OrdinaryPackageObligationSchemaIdentity,
    decode_ordinary_package_obligation_ledger, encode_ordinary_package_obligation_ledger,
    ordinary_package_obligation_ledger_fingerprint,
    ordinary_package_obligation_ledger_from_compiler_rows,
    reconstruct_ordinary_package_obligation_ledger, recover_ordinary_package_obligation_ledger,
    validate_ordinary_package_obligation_ledger,
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
    Machine(PackageReviewMachineParameterContract),
    Proposition(PackageReviewPropositionParameterSignature),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewPropositionParameterSignature {
    parameters: Vec<PackageReviewPropositionParameterValue>,
}

impl PackageReviewPropositionParameterSignature {
    pub fn parameters(&self) -> &[PackageReviewPropositionParameterValue] {
        &self.parameters
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewPropositionParameterValue {
    type_identity: PackageReviewTypeIdentity,
}

impl PackageReviewPropositionParameterValue {
    pub const fn type_identity(&self) -> &PackageReviewTypeIdentity {
        &self.type_identity
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageReviewMachineParameterContract {
    Structural(PackageReviewMachineParameterSignature),
    Nominal {
        trait_identity: PackageReviewNominalIdentity,
        requirement_identity: PackageReviewNominalIdentity,
    },
    RequirementIdentity,
}

impl PackageReviewMachineParameterContract {
    pub const fn structural(&self) -> Option<&PackageReviewMachineParameterSignature> {
        match self {
            Self::Structural(signature) => Some(signature),
            Self::Nominal { .. } | Self::RequirementIdentity => None,
        }
    }

    pub const fn nominal(
        &self,
    ) -> Option<(&PackageReviewNominalIdentity, &PackageReviewNominalIdentity)> {
        match self {
            Self::Structural(_) | Self::RequirementIdentity => None,
            Self::Nominal {
                trait_identity,
                requirement_identity,
            } => Some((trait_identity, requirement_identity)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewMachineParameterSignature {
    lifetime_parameter_count: usize,
    type_parameters: Vec<PackageReviewTypeParameter>,
    parameters: Vec<PackageReviewMachineParameterValue>,
    return_type: PackageReviewTypeIdentity,
    contracts: Vec<PackageReviewCallableContract>,
    published_crash: Vec<PackageReviewCrashRoute>,
    service_reach: Vec<PackageReviewNominalIdentity>,
    service_reach_is_installation_bound: bool,
    synchronous_invocations: Vec<PackageReviewSynchronousInvocation>,
    suspends: bool,
    blocks: bool,
    termination: PackageReviewTermination,
}

impl PackageReviewMachineParameterSignature {
    pub const fn lifetime_parameter_count(&self) -> usize {
        self.lifetime_parameter_count
    }

    pub fn type_parameters(&self) -> &[PackageReviewTypeParameter] {
        &self.type_parameters
    }

    pub fn parameters(&self) -> &[PackageReviewMachineParameterValue] {
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
pub struct PackageReviewMachineParameterValue {
    name: String,
    type_identity: PackageReviewTypeIdentity,
    is_const: bool,
    is_mutable: bool,
    is_self: bool,
}

impl PackageReviewMachineParameterValue {
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
    selected_lifetime_arguments: Vec<u32>,
    selected_arguments: Vec<PackageReviewContractStaticArgument>,
    selected_subject: Option<PackageReviewContractStaticArgument>,
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

    pub fn selected_lifetime_arguments(&self) -> &[u32] {
        &self.selected_lifetime_arguments
    }

    pub fn selected_arguments(&self) -> &[PackageReviewContractStaticArgument] {
        &self.selected_arguments
    }

    pub const fn selected_subject(&self) -> Option<&PackageReviewContractStaticArgument> {
        self.selected_subject.as_ref()
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

/// Closed structural identity of executable code supplied outside Omega.
/// String fields are foreign ABI identifiers, not package-authored policy or
/// capability classifications.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewExternalBinding {
    Import { library: String, symbol: String },
    Syscall { number: i64 },
    CompilerIntrinsic,
    VtableSlot { index: i64 },
    VtableField { field: String },
    TableFunction { field: String },
}

/// One trust-bearing association between an exact reviewed callable,
/// requirement application, and externally supplied executable mechanism.
/// This is not Terminal evidence and makes no implementation-correctness or
/// audit claim.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewExternalRequirement {
    Trait(PackageReviewCallableConformance),
    Operator(PackageReviewOperatorCoordinate),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewExternalExecutableSupply {
    callable: PackageReviewNominalIdentity,
    requirement: PackageReviewExternalRequirement,
    binding: PackageReviewExternalBinding,
}

impl PackageReviewExternalExecutableSupply {
    pub const fn callable(&self) -> &PackageReviewNominalIdentity {
        &self.callable
    }

    pub const fn requirement(&self) -> &PackageReviewExternalRequirement {
        &self.requirement
    }

    pub const fn conformance(&self) -> Option<&PackageReviewCallableConformance> {
        match &self.requirement {
            PackageReviewExternalRequirement::Trait(conformance) => Some(conformance),
            PackageReviewExternalRequirement::Operator(_) => None,
        }
    }

    pub const fn operator(&self) -> Option<&PackageReviewOperatorCoordinate> {
        match &self.requirement {
            PackageReviewExternalRequirement::Trait(_) => None,
            PackageReviewExternalRequirement::Operator(operator) => Some(operator),
        }
    }

    pub const fn binding(&self) -> &PackageReviewExternalBinding {
        &self.binding
    }
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

/// The carrier named by one public complete conformance. A generic carrier is
/// represented by its alpha-normalized conformance-telescope ordinal; a
/// concrete carrier keeps its exact package/toolchain nominal identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageReviewConformanceSubject {
    Subjectless,
    TypeParameter(u32),
    Nominal(PackageReviewNominalIdentity),
}

/// One independently nameable, public, complete conformance declaration.
///
/// `interface` is the complete normalized inherited requirement map proven by
/// checked lowering. Realization machine names, bodies, and physical code
/// identity are deliberately absent: they are private implementation, not a
/// receiver-nameable package contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewConformanceShape {
    identity: PackageReviewNominalIdentity,
    lifetime_parameter_count: usize,
    type_parameters: Vec<PackageReviewTypeParameter>,
    subject: PackageReviewConformanceSubject,
    interface: PackageReviewEvidenceInterface,
}

impl PackageReviewConformanceShape {
    pub const fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }

    pub const fn lifetime_parameter_count(&self) -> usize {
        self.lifetime_parameter_count
    }

    pub fn type_parameters(&self) -> &[PackageReviewTypeParameter] {
        &self.type_parameters
    }

    pub const fn subject(&self) -> &PackageReviewConformanceSubject {
        &self.subject
    }

    pub const fn interface(&self) -> &PackageReviewEvidenceInterface {
        &self.interface
    }
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

/// One compiler-owned semantic role contributed by a public domain.
///
/// The role vocabulary is closed compiler semantics. The declaration's
/// compiler-private semantic-domain ID is validated during projection but does
/// not cross the canonical package-review boundary; the package-qualified
/// domain identity is the persistent subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewDomainSemanticRole {
    DenotationDimension,
    ArithmeticPolicy,
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewDomainAliasAtom {
    Declared(PackageReviewNominalIdentity),
    Carry(psi_language_semantics::CarryPermission),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewDomainShape {
    identity: PackageReviewNominalIdentity,
    type_parameters: Vec<PackageReviewTypeParameter>,
    target_type: PackageReviewTypeIdentity,
    index_arguments: Vec<PackageReviewTypeIdentity>,
    predicate_body: psi_language_semantics::DomainPredicateBody,
    predicate_facts: Vec<PackageReviewContractFact>,
    alias_expansion: Option<Vec<PackageReviewDomainAliasAtom>>,
    classification: Option<PackageReviewDomainClassification>,
    semantic_roles: Vec<PackageReviewDomainSemanticRole>,
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

    pub fn alias_expansion(&self) -> Option<&[PackageReviewDomainAliasAtom]> {
        self.alias_expansion.as_deref()
    }

    pub const fn classification(&self) -> Option<PackageReviewDomainClassification> {
        self.classification
    }

    pub fn semantic_roles(&self) -> &[PackageReviewDomainSemanticRole] {
        &self.semantic_roles
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

/// Closed semantic form of one public data declaration. Quotient identity is
/// the carrier family plus relation declaration; the proof implementation that
/// licensed formation is intentionally not API identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageReviewDataKind {
    Ordinary,
    Quotient {
        carrier: PackageReviewTypeIdentity,
        relation: PackageReviewNominalIdentity,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewDataShape {
    identity: PackageReviewNominalIdentity,
    kind: PackageReviewDataKind,
    supply: psi_language_semantics::DataSupplyMode,
    lifetime_parameter_count: usize,
    type_parameters: Vec<PackageReviewTypeParameter>,
    properties: psi_typed_trees::data::DataProperties,
    zero_gated: bool,
    invariants: Vec<PackageReviewContractFact>,
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

    pub const fn kind(&self) -> &PackageReviewDataKind {
        &self.kind
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

    pub fn invariants(&self) -> &[PackageReviewContractFact] {
        &self.invariants
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
    ByteSequencePredicate(PackageReviewByteSequencePredicate),
}

/// Stable identity of one ordinary operator overload. The nominal path names
/// the declaration family; the compiler's canonical parameter and
/// result-dispatch identities distinguish overloads by the same rules used by
/// checked selection. Source names, arena handles, and return refinements that
/// do not participate in dispatch are not coordinates.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewOperatorCoordinate {
    identity: PackageReviewNominalIdentity,
    parameter_dispatch: String,
    result_dispatch: String,
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewContractOperatorMeaning {
    Builtin,
    Declared(PackageReviewOperatorCoordinate),
}

impl PackageReviewContractCallTarget {
    pub const fn nominal(&self) -> Option<&PackageReviewNominalIdentity> {
        match self {
            Self::Nominal(identity) => Some(identity),
            Self::ByteSequencePredicate(_) => None,
        }
    }

    pub const fn byte_sequence_predicate(&self) -> Option<PackageReviewByteSequencePredicate> {
        match self {
            Self::Nominal(_) => None,
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
    field: PackageReviewNominalIdentity,
    value: PackageReviewContractExpression,
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
pub enum PackageReviewPublicPropositionBody {
    Primitive,
    Witness(PackageReviewEvidenceInterface),
    Transparent(PackageReviewContractFact),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewPropositionShape {
    identity: PackageReviewNominalIdentity,
    binders: Vec<PackageReviewPropositionBinder>,
    parameter_types: Vec<PackageReviewTypeIdentity>,
    body: PackageReviewPublicPropositionBody,
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
    identity: PackageReviewNominalIdentity,
    declared_type: PackageReviewTypeIdentity,
    canonical_value_encoding: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewOperatorShape {
    coordinate: PackageReviewOperatorCoordinate,
    is_boundary: bool,
    spelling: Option<psi_language_core::OperatorSpelling>,
    lifetime_parameter_count: usize,
    type_parameters: Vec<PackageReviewTypeParameter>,
    parameters: Vec<PackageReviewCallableParameter>,
    return_type: PackageReviewTypeIdentity,
    contracts: Vec<PackageReviewCallableContract>,
    published_crash: Vec<PackageReviewCrashRoute>,
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
pub struct PackageReviewPropositionParameterApplication {
    binder_ordinal: u32,
    arguments: Vec<PackageReviewContractExpression>,
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
    result_data: PackageReviewNominalIdentity,
    result_case: PackageReviewNominalIdentity,
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
    kind: PackageReviewContractKind,
    result_case: Option<PackageReviewResultCaseIdentity>,
    binding: Option<String>,
    evidence_lane_position: Option<u32>,
    fact: PackageReviewContractFact,
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
    /// Exact structural guard for an abstract public-operator crash ceiling.
    /// Unlike runtime crash-predicate bytes, this retains selected nominal
    /// package identity for calls, members, and declared overloads.
    Expression(PackageReviewContractExpression),
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
    supply: PackageReviewCallableSupply,
    lifetime_parameter_count: usize,
    type_parameters: Vec<PackageReviewTypeParameter>,
    conformance_bounds: Vec<PackageReviewConformanceBound>,
    parameters: Vec<PackageReviewCallableParameter>,
    return_type: PackageReviewTypeIdentity,
    conformances: Vec<PackageReviewCallableConformance>,
    operator_realizations: Vec<PackageReviewOperatorCoordinate>,
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
    /// Exact checked operational summary. Published callable surfaces expose
    /// their authored may-ceiling; the build-machine lane may remain inferred.
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

/// Exact declarations bound to one selected provider realization row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPackageProviderRowIdentity {
    requirement: PackageReviewNominalIdentity,
    realization: PackageReviewNominalIdentity,
}

impl CheckedPackageProviderRowIdentity {
    pub const fn requirement(&self) -> &PackageReviewNominalIdentity {
        &self.requirement
    }

    pub const fn realization(&self) -> &PackageReviewNominalIdentity {
        &self.realization
    }
}

/// One selected provider plan retained for human/LLM review.
///
/// The realizing package is exact and participates in `plan_fingerprint`.
/// That existing 64-bit fingerprint is review/execution compatibility data,
/// not a collision-resistant package-admission identity.
/// Schema, provider type, row requirement, and realizing machine retain exact
/// package-qualified or authored-toolchain declaration identities, and review
/// rejects if those owners disagree with the selected plan. Readable provider-
/// plan strings remain execution/audit data and are not asked to stand in for
/// those declarations. The authored toolchain-source commitment does not yet
/// seal whole-compiler or source-free compiler-intrinsic identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPackageProviderReview {
    plan_name: String,
    plan_fingerprint: u64,
    realizing_package: Option<PackageKeyIdentity>,
    schema_declaration: PackageReviewNominalIdentity,
    provider_type: String,
    provider_type_package: Option<PackageKeyIdentity>,
    provider_type_declaration: Option<PackageReviewNominalIdentity>,
    schema: omega_effects::provider_plan::ServiceSchema,
    target: String,
    rows: Vec<omega_effects::provider_plan::ProviderPlanRow>,
    row_declarations: Vec<CheckedPackageProviderRowIdentity>,
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

    pub const fn schema_declaration(&self) -> &PackageReviewNominalIdentity {
        &self.schema_declaration
    }

    pub fn provider_type(&self) -> &str {
        &self.provider_type
    }

    pub const fn provider_type_package(&self) -> Option<PackageKeyIdentity> {
        self.provider_type_package
    }

    pub const fn provider_type_declaration(&self) -> Option<&PackageReviewNominalIdentity> {
        self.provider_type_declaration.as_ref()
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

    pub fn row_declarations(&self) -> &[CheckedPackageProviderRowIdentity] {
        &self.row_declarations
    }
}

impl CheckedPackageCallableReview {
    pub const fn role(&self) -> PackageReviewCallableRole {
        self.role
    }

    pub fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }

    pub const fn supply(&self) -> PackageReviewCallableSupply {
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

    pub fn operator_realizations(&self) -> &[PackageReviewOperatorCoordinate] {
        &self.operator_realizations
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
    public_conformances: Vec<PackageReviewConformanceShape>,
    public_domains: Vec<PackageReviewDomainShape>,
    public_propositions: Vec<PackageReviewPropositionShape>,
    public_consts: Vec<PackageReviewConstShape>,
    public_operators: Vec<PackageReviewOperatorShape>,
    public_data: Vec<PackageReviewDataShape>,
    representation_tcb: Vec<PackageReviewRepresentationTcb>,
    semantic_dependencies: Vec<PackageReviewSemanticDependency>,
    callables: Vec<CheckedPackageCallableReview>,
    external_executable_supply: Vec<PackageReviewExternalExecutableSupply>,
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
            && self.public_conformances == other.public_conformances
            && self.public_domains == other.public_domains
            && self.public_propositions == other.public_propositions
            && self.public_consts == other.public_consts
            && self.public_operators == other.public_operators
            && self.public_data == other.public_data
            && self.representation_tcb == other.representation_tcb
            && self.semantic_dependencies == other.semantic_dependencies
            && self.callables == other.callables
            && self.external_executable_supply == other.external_executable_supply
            && self.dangerous_authorities == other.dangerous_authorities
            && self.dangerous_authority_slack == other.dangerous_authority_slack
            && self.selected_providers == other.selected_providers
    }
}

impl Eq for CheckedPackageReviewProjection {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageReviewCanonicalRowSources {
    public_traits: Vec<PackageReviewCanonicalRowSource>,
    public_conformances: Vec<PackageReviewCanonicalRowSource>,
    public_domains: Vec<PackageReviewCanonicalRowSource>,
    public_propositions: Vec<PackageReviewCanonicalRowSource>,
    public_consts: Vec<PackageReviewCanonicalRowSource>,
    public_operators: Vec<PackageReviewCanonicalRowSource>,
    public_data: Vec<PackageReviewCanonicalRowSource>,
    representation_tcb: Vec<PackageReviewCanonicalRowSource>,
    semantic_dependencies: Vec<PackageReviewCanonicalRowSource>,
    callables: Vec<PackageReviewCanonicalRowSource>,
    external_executable_supply: Vec<PackageReviewCanonicalRowSource>,
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
    nested_source_locations: Vec<ProjectedNestedSourceLocation>,
}

#[derive(Debug, Clone, Copy)]
struct ProjectedNestedSourceLocation {
    source_span: psi_source::SourceSpan,
    role: PackageReviewSourceLocationRole,
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
    PublicProposition,
    PublicConst,
    PublicOperator,
    PublicConformance,
    /// Opaque executable code supplied through one exact external binding.
    /// This is a blocking trust/TCB disclosure, not Terminal evidence.
    ExternalExecutableSupply,
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
    ProviderRequirementDeclaration,
    ProviderRealization,
    SemanticDependencyConsumer,
    SemanticDependencyDeclaration,
    TraitParent,
    ContractClause,
    BodyCall,
    Suspension,
    Blocking,
    ServiceReach,
    SynchronousInvocation,
    ExternalBinding,
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

    pub fn public_conformances(&self) -> &[PackageReviewConformanceShape] {
        &self.public_conformances
    }

    pub fn public_domains(&self) -> &[PackageReviewDomainShape] {
        &self.public_domains
    }

    pub fn public_propositions(&self) -> &[PackageReviewPropositionShape] {
        &self.public_propositions
    }

    pub fn public_consts(&self) -> &[PackageReviewConstShape] {
        &self.public_consts
    }

    pub fn public_operators(&self) -> &[PackageReviewOperatorShape] {
        &self.public_operators
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

    pub fn external_executable_supply(&self) -> &[PackageReviewExternalExecutableSupply] {
        &self.external_executable_supply
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
    let derived_operator_realizations =
        psi_typed_trees_to_checked_trees::derive_checked_operator_realization_contracts(
            &compilation.typed,
        );
    if derived_operator_realizations != compilation.facts.operators.operator_realization_contracts {
        return Err(vec![Diagnostic::error(format!(
            "retained checked operator-realization contracts do not equal compiler rederivation (retained {} rows, derived {} rows)",
            compilation
                .facts
                .operators
                .operator_realization_contracts
                .len(),
            derived_operator_realizations.len(),
        ))]);
    }
    let build_machine = compilation.selected_build_machine_symbol();
    let public_traits = project_public_traits(compilation, package)?;
    let public_conformances = project_public_conformances(compilation, package)?;
    let public_domains = project_public_domains(compilation, package)?;
    let public_propositions = project_public_propositions(compilation, package)?;
    let public_consts = project_public_consts(compilation, package)?;
    let public_operators = project_public_operators(compilation, package)?;
    let public_data = project_public_data(compilation, package)?;
    let representation_tcb = project_representation_tcb(compilation, package)?;
    let semantic_dependencies = project_semantic_dependencies(compilation, package)?;
    let mut callables = Vec::new();
    let mut external_executable_supply = Vec::new();
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
            | PackageReviewNominalOwner::ToolchainSource(_) => {
                continue;
            }
            PackageReviewNominalOwner::Unresolved => {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` has no managed package owner",
                    owner.path
                ))]);
            }
        }

        let (callable, executable_supply) = project_callable(compilation, machine, role, owner)?;
        let mut contract_locations =
            project_contract_clause_source_locations(compilation.machine_contracts(machine));
        contract_locations.extend(project_machine_invocation_source_locations(
            compilation,
            machine,
        )?);
        contract_locations.extend(project_machine_service_reach_source_locations(
            compilation,
            machine,
        )?);
        contract_locations.extend(project_machine_operational_source_locations(
            compilation,
            machine,
        )?);
        collect_type_parameter_source_locations(
            compilation,
            compilation.machine_type_parameters(machine),
            &mut contract_locations,
        )?;
        contract_locations.extend(
            psi_typed_trees_to_checked_trees::derive_checked_body_call_source_spans(
                &compilation.typed,
                &compilation.facts,
                machine.symbol,
            )?
            .into_iter()
            .map(|source_span| ProjectedNestedSourceLocation {
                source_span,
                role: PackageReviewSourceLocationRole::BodyCall,
            }),
        );
        external_executable_supply.extend(executable_supply);
        callables.push(ProjectedReviewRow {
            row: callable,
            declaration: machine.symbol,
            nested_source_locations: contract_locations,
        });
        projected_build_machine |= role == PackageReviewCallableRole::Build;
    }

    // External executable supply is trust-bearing even when the leaf is a
    // private implementation detail. Public/build leaves were projected with
    // their callable envelopes above; project every remaining package-owned
    // external leaf without manufacturing a public callable row.
    for machine in compilation.machines() {
        if !matches!(
            machine.supply_mode,
            MachineSupplyMode::ExternalRealization { .. }
        ) || machine.is_public
            || Some(machine.symbol) == build_machine
        {
            continue;
        }
        let owner = nominal_identity(compilation, machine.symbol)?;
        match owner.owner {
            PackageReviewNominalOwner::Package(owner_package) if owner_package == package => {}
            PackageReviewNominalOwner::Package(_)
            | PackageReviewNominalOwner::ToolchainSource(_) => continue,
            PackageReviewNominalOwner::Unresolved => {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed external callable `{}` has no managed package owner",
                    owner.path
                ))]);
            }
        }
        external_executable_supply.extend(project_private_external_executable_supply(
            compilation,
            machine,
            &owner,
        )?);
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
    external_executable_supply.sort_by(|left, right| left.row.cmp(&right.row));
    if external_executable_supply
        .windows(2)
        .any(|rows| rows[0].row == rows[1].row)
    {
        return Err(vec![Diagnostic::error(
            "package review contains a duplicate exact external executable-supply row",
        )]);
    }
    let dangerous_authorities = project_dangerous_authorities(compilation, &callables)?;
    let dangerous_authority_slack = project_dangerous_authority_slack(compilation, &callables)?;
    let selected_plans = compilation.selected_provider_plans().plans();
    let selected_provider_provenance = compilation.selected_provider_provenance();
    if selected_plans.len() != selected_provider_provenance.len() {
        return Err(vec![Diagnostic::error(
            "selected-provider review provenance is not aligned with the canonical selected plan set",
        )]);
    }
    let mut selected_providers = Vec::with_capacity(selected_plans.len());
    for (plan, retained) in selected_plans.iter().zip(selected_provider_provenance) {
        if retained.plan != *plan
            || retained.provider.row_requirements.len() != plan.rows.len()
            || retained.provider.row_realizations.len() != plan.rows.len()
        {
            return Err(vec![Diagnostic::error(format!(
                "selected provider plan `{}` has incomplete or misaligned declaration provenance",
                plan.name,
            ))]);
        }
        let row_declarations = retained
            .provider
            .row_requirements
            .iter()
            .zip(&retained.provider.row_realizations)
            .map(|(requirement, realization)| {
                Ok(CheckedPackageProviderRowIdentity {
                    requirement: provider_requirement_identity(
                        compilation,
                        retained.provider.schema,
                        *requirement,
                    )?,
                    realization: nominal_identity(compilation, *realization)?,
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        let schema_declaration = nominal_identity(compilation, retained.provider.schema.symbol())?;
        validate_selected_provider_declaration_owner(
            &schema_declaration,
            plan.schema.trait_package_identity,
            &plan.name,
            "service schema",
        )?;
        let provider_type_declaration = retained
            .provider
            .provider_type
            .map(|symbol| nominal_identity(compilation, symbol))
            .transpose()?;
        match provider_type_declaration.as_ref() {
            Some(declaration) => validate_selected_provider_declaration_owner(
                declaration,
                plan.provider_type_package_identity,
                &plan.name,
                "provider type",
            )?,
            None if plan.provider_type.is_empty()
                && plan.provider_type_package_identity.is_none() => {}
            None => {
                return Err(vec![Diagnostic::error(format!(
                    "selected provider plan `{}` has provider-type identity without one exact declaration",
                    plan.name,
                ))]);
            }
        }
        for (row, declarations) in plan.rows.iter().zip(&row_declarations) {
            let mut methods = plan
                .schema
                .methods
                .iter()
                .filter(|method| method.requirement_identity == row.requirement_identity);
            let Some(method) = methods.next() else {
                return Err(vec![Diagnostic::error(format!(
                    "selected provider plan `{}` row `{}` has no exact schema method",
                    plan.name, row.requirement_identity,
                ))]);
            };
            if methods.next().is_some() {
                return Err(vec![Diagnostic::error(format!(
                    "selected provider plan `{}` row `{}` has duplicate schema methods",
                    plan.name, row.requirement_identity,
                ))]);
            }
            validate_selected_provider_declaration_owner(
                &declarations.requirement,
                method.requirement_owner_package_identity,
                &plan.name,
                "row requirement",
            )?;
            validate_selected_provider_declaration_owner(
                &declarations.realization,
                plan.origin_package_identity,
                &plan.name,
                "row realization",
            )?;
        }
        selected_providers.push(CheckedPackageProviderReview {
            plan_name: plan.name.clone(),
            plan_fingerprint: plan.identity_fingerprint(),
            realizing_package: plan.origin_package_identity,
            schema_declaration,
            provider_type: plan.provider_type.clone(),
            provider_type_package: plan.provider_type_package_identity,
            provider_type_declaration,
            schema: plan.schema.clone(),
            target: plan.target.clone(),
            rows: plan.rows.clone(),
            row_declarations,
        });
    }
    let (public_traits, public_trait_sources) = finalize_projected_rows(
        compilation,
        public_traits,
        PackageReviewSourceLocationRole::Declaration,
    )?;
    let (public_conformances, public_conformance_sources) = finalize_projected_rows(
        compilation,
        public_conformances,
        PackageReviewSourceLocationRole::Declaration,
    )?;
    let (public_domains, public_domain_sources) = finalize_projected_rows(
        compilation,
        public_domains,
        PackageReviewSourceLocationRole::Declaration,
    )?;
    let (public_propositions, public_proposition_sources) = finalize_projected_rows(
        compilation,
        public_propositions,
        PackageReviewSourceLocationRole::Declaration,
    )?;
    let (public_consts, public_const_sources) = finalize_projected_rows(
        compilation,
        public_consts,
        PackageReviewSourceLocationRole::Declaration,
    )?;
    let (public_operators, public_operator_sources) = finalize_projected_rows(
        compilation,
        public_operators,
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
    let (external_executable_supply, external_executable_supply_sources) = finalize_projected_rows(
        compilation,
        external_executable_supply,
        PackageReviewSourceLocationRole::Declaration,
    )?;
    let (dangerous_authorities, dangerous_authority_sources) =
        finalize_dangerous_authority_rows(compilation, dangerous_authorities)?;
    let (dangerous_authority_slack, dangerous_authority_slack_sources) =
        finalize_dangerous_authority_slack_rows(compilation, dangerous_authority_slack)?;
    let row_sources = PackageReviewCanonicalRowSources {
        public_traits: public_trait_sources,
        public_conformances: public_conformance_sources,
        public_domains: public_domain_sources,
        public_propositions: public_proposition_sources,
        public_consts: public_const_sources,
        public_operators: public_operator_sources,
        public_data: public_data_sources,
        representation_tcb: representation_tcb_sources,
        semantic_dependencies: semantic_dependency_sources,
        callables: callable_sources,
        external_executable_supply: external_executable_supply_sources,
        dangerous_authorities: dangerous_authority_sources,
        dangerous_authority_slack: dangerous_authority_slack_sources,
        selected_provider_set: selected_provider_row_source(compilation, &selected_providers)?,
    };
    validate_canonical_row_source_limits(&row_sources)?;

    Ok(CheckedPackageReviewProjection {
        package,
        target,
        public_traits,
        public_conformances,
        public_domains,
        public_propositions,
        public_consts,
        public_operators,
        public_data,
        representation_tcb,
        semantic_dependencies,
        callables,
        external_executable_supply,
        dangerous_authorities,
        dangerous_authority_slack,
        selected_providers,
        row_sources,
    })
}

fn validate_selected_provider_declaration_owner(
    declaration: &PackageReviewNominalIdentity,
    expected_package: Option<PackageKeyIdentity>,
    plan_name: &str,
    role: &str,
) -> Result<(), Vec<Diagnostic>> {
    let matches = match (expected_package, declaration.owner) {
        (Some(expected), PackageReviewNominalOwner::Package(actual)) => expected == actual,
        (None, PackageReviewNominalOwner::ToolchainSource(_)) => true,
        (Some(_), PackageReviewNominalOwner::ToolchainSource(_))
        | (None, PackageReviewNominalOwner::Package(_))
        | (_, PackageReviewNominalOwner::Unresolved) => false,
    };
    if matches {
        Ok(())
    } else {
        Err(vec![Diagnostic::error(format!(
            "selected provider plan `{plan_name}` {role} `{}` disagrees with its exact package/toolchain ownership",
            declaration.path,
        ))])
    }
}

fn project_semantic_dependencies(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedSemanticDependencyRow>, Vec<Diagnostic>> {
    let derived = psi_typed_trees_to_checked_trees::derive_checked_semantic_dependencies(
        &compilation.typed,
        &compilation.facts,
    );
    if derived != compilation.facts.flow.semantic_dependencies {
        return Err(vec![Diagnostic::error(format!(
            "retained checked semantic-dependency evidence does not equal compiler rederivation (retained {} rows, derived {} rows)",
            compilation.facts.flow.semantic_dependencies.rows.len(),
            derived.rows.len(),
        ))]);
    }

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
            nested_source_locations: Vec::new(),
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
        let mut locations = vec![canonical_source_location(
            compilation,
            projected.declaration,
            role,
        )?];
        for nested in projected.nested_source_locations {
            locations.push(canonical_source_span_location(
                compilation,
                nested.source_span,
                nested.role,
            )?);
        }
        locations.sort();
        locations.dedup();
        sources.push(PackageReviewCanonicalRowSource::authored(locations));
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

        for requirement in &retained.provider.row_requirements {
            locations.push(canonical_source_location(
                compilation,
                *requirement,
                PackageReviewSourceLocationRole::ProviderRequirementDeclaration,
            )?);
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
        .chain(&sources.public_conformances)
        .chain(&sources.public_domains)
        .chain(&sources.public_propositions)
        .chain(&sources.public_consts)
        .chain(&sources.public_operators)
        .chain(&sources.public_data)
        .chain(&sources.representation_tcb)
        .chain(&sources.semantic_dependencies)
        .chain(&sources.callables)
        .chain(&sources.external_executable_supply)
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
        let parameters = compilation.trait_type_parameters(definition);
        let (mut trait_binders, type_parameters) = project_type_parameters(
            compilation,
            parameters,
            "trait",
            &identity.path,
            &definition.lifetime_parameters,
        )?;
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
        let mut nested_source_locations = Vec::new();
        collect_type_parameter_source_locations(
            compilation,
            parameters,
            &mut nested_source_locations,
        )?;
        nested_source_locations.extend(compilation.trait_requirements(definition).iter().map(
            |parent| ProjectedNestedSourceLocation {
                source_span: parent.source_span,
                role: PackageReviewSourceLocationRole::TraitParent,
            },
        ));
        for requirement in compilation.trait_machine_signatures(definition) {
            nested_source_locations.extend(project_contract_clause_source_locations(
                compilation.state_signature_contracts(requirement),
            ));
            nested_source_locations.extend(project_signature_invocation_source_locations(
                compilation,
                requirement,
            )?);
            nested_source_locations.extend(project_signature_service_reach_source_locations(
                compilation,
                definition.symbol,
                requirement,
            )?);
            nested_source_locations.extend(project_signature_operational_source_locations(
                compilation,
                definition.symbol,
                requirement,
            )?);
            collect_type_parameter_source_locations(
                compilation,
                compilation.state_signature_type_parameters(requirement),
                &mut nested_source_locations,
            )?;
        }
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
            nested_source_locations,
        });
    }
    rows.sort_by(|left, right| left.row.identity.cmp(&right.row.identity));
    Ok(rows)
}

fn project_public_conformances(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedReviewRow<PackageReviewConformanceShape>>, Vec<Diagnostic>> {
    use psi_typed_trees::trait_definition::{ConformanceImplementation, ConformanceSubject};

    let mut projected = Vec::new();
    for conformance in compilation
        .conformances()
        .iter()
        .filter(|conformance| conformance.is_public)
    {
        let identity = nominal_identity(compilation, conformance.symbol)?;
        if !reviewed_package_owns(&identity, package)? {
            continue;
        }
        let parameters = compilation.conformance_type_parameters(conformance);
        let (binders, type_parameters) = project_type_parameters(
            compilation,
            parameters,
            "conformance",
            &identity.path,
            &conformance.lifetime_parameters,
        )?;
        let subject = match &conformance.subject {
            ConformanceSubject::Subjectless => PackageReviewConformanceSubject::Subjectless,
            ConformanceSubject::Carrier(_) => {
                if let Some(ordinal) = parameters
                    .iter()
                    .position(|parameter| parameter.symbol == conformance.carrier_symbol)
                {
                    if !matches!(
                        parameters[ordinal].kind,
                        psi_typed_trees::data::TypeParameterKind::Type
                    ) {
                        return Err(vec![Diagnostic::error(format!(
                            "public conformance `{}` uses a non-type static parameter as its subject",
                            identity.path
                        ))]);
                    }
                    PackageReviewConformanceSubject::TypeParameter(
                        u32::try_from(ordinal).map_err(|_| {
                            vec![Diagnostic::error(format!(
                                "public conformance `{}` subject exceeds the portable review parameter range",
                                identity.path
                            ))]
                        })?,
                    )
                } else {
                    let carriers = compilation
                        .data_definitions()
                        .iter()
                        .filter(|definition| definition.symbol == conformance.carrier_symbol)
                        .collect::<Vec<_>>();
                    let [carrier] = carriers.as_slice() else {
                        return Err(vec![Diagnostic::error(format!(
                            "public conformance `{}` resolves its subject to {} data declarations; expected exactly one nominal data carrier or one telescope parameter",
                            identity.path,
                            carriers.len()
                        ))]);
                    };
                    if !carrier.is_public {
                        return Err(vec![Diagnostic::error(format!(
                            "public conformance `{}` exposes private carrier `{}`",
                            identity.path, carrier.name
                        ))]);
                    }
                    PackageReviewConformanceSubject::Nominal(nominal_identity(
                        compilation,
                        conformance.carrier_symbol,
                    )?)
                }
            }
        };
        let traits = compilation
            .traits()
            .iter()
            .filter(|definition| definition.symbol == conformance.trait_symbol)
            .collect::<Vec<_>>();
        let [trait_definition] = traits.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "public conformance `{}` resolves its trait to {} declarations; expected exactly one",
                identity.path,
                traits.len()
            ))]);
        };
        if !trait_definition.is_public {
            return Err(vec![Diagnostic::error(format!(
                "public conformance `{}` exposes private trait `{}`",
                identity.path, trait_definition.name
            ))]);
        }
        if !trait_definition.lifetime_parameters.is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "public conformance `{}` selects lifetime-parameterized trait `{}` without retained lifetime arguments",
                identity.path, trait_definition.name
            ))]);
        }
        let trait_arguments = compilation
            .type_reference_table
            .type_reference_handles(conformance.arguments)
            .to_vec();
        let mut requirements = Vec::new();
        collect_evidence_requirements(
            compilation,
            conformance.trait_symbol,
            &trait_arguments,
            &binders,
            Some(&conformance.lifetime_parameters),
            &[],
            &mut Vec::new(),
            &mut requirements,
        )?;
        requirements.sort();
        requirements.dedup();
        let trait_identity = nominal_identity(compilation, conformance.trait_symbol)?;
        let interface_arguments = trait_arguments
            .iter()
            .map(|argument| {
                review_signature_type_identity_with_binders(
                    compilation,
                    *argument,
                    &binders,
                    &conformance.lifetime_parameters,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let interface = PackageReviewEvidenceInterface {
            trait_identity,
            arguments: interface_arguments,
            requirements,
        };
        let mut realized = match &conformance.implementation {
            ConformanceImplementation::AttachedRequirementMachines => {
                interface.requirements.clone()
            }
            ConformanceImplementation::Closed { rows } => Vec::with_capacity(rows.len()),
        };
        let closed_rows = match &conformance.implementation {
            ConformanceImplementation::AttachedRequirementMachines => &[][..],
            ConformanceImplementation::Closed { rows } => rows.as_slice(),
        };
        for row in closed_rows {
            if !row.realization_machine.is_valid() || !row.realization_state.is_valid() {
                return Err(vec![Diagnostic::error(format!(
                    "public conformance `{}` has an incomplete checked realization row",
                    identity.path
                ))]);
            }
            let declaring_trait = nominal_identity(compilation, row.declaring_trait)?;
            let definition = compilation
                .traits()
                .iter()
                .find(|candidate| candidate.symbol == row.declaring_trait)
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "public conformance `{}` has a row with an unresolved declaring trait",
                        identity.path
                    ))]
                })?;
            let requirement = compilation
                .trait_machine_signatures(definition)
                .iter()
                .find(|candidate| candidate.symbol == row.requirement)
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "public conformance `{}` has a row outside its declaring trait's requirement sequence",
                        identity.path
                    ))]
                })?;
            let requirement = trait_requirement_identity(compilation, definition, requirement)?;
            let matching = interface
                .requirements
                .iter()
                .filter(|candidate| {
                    candidate.declaring_trait == declaring_trait
                        && candidate.requirement == requirement
                })
                .collect::<Vec<_>>();
            let [matching] = matching.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "public conformance `{}` cannot assign one normalized row uniquely to its inherited evidence interface",
                    identity.path
                ))]);
            };
            realized.push((*matching).clone());
        }
        realized.sort();
        if realized.windows(2).any(|pair| pair[0] == pair[1]) || realized != interface.requirements
        {
            return Err(vec![Diagnostic::error(format!(
                "public conformance `{}` does not retain one complete normalized row for every inherited requirement",
                identity.path
            ))]);
        }
        projected.push(ProjectedReviewRow {
            row: PackageReviewConformanceShape {
                identity,
                lifetime_parameter_count: conformance.lifetime_parameters.len(),
                type_parameters,
                subject,
                interface,
            },
            declaration: conformance.symbol,
            nested_source_locations: {
                let mut locations = Vec::new();
                collect_type_parameter_source_locations(compilation, parameters, &mut locations)?;
                locations
            },
        });
    }
    projected.sort_by(|left, right| left.row.identity.cmp(&right.row.identity));
    Ok(projected)
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
    let owner = compilation
        .traits()
        .iter()
        .find(|definition| definition.symbol == trait_symbol)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "package review trait requirement has no exact declaring trait",
            )]
        })?;
    let identity = trait_requirement_identity(compilation, owner, requirement)?;
    let parameters = compilation.state_signature_type_parameters(requirement);
    let mut lifetime_binders = trait_lifetime_parameters.to_vec();
    lifetime_binders.extend(requirement.lifetime_parameters.iter().cloned());
    let (binders, type_parameters) = project_type_parameters_after(
        compilation,
        parameters,
        "trait requirement",
        &identity.path,
        trait_binders,
        trait_parameter_count,
        &lifetime_binders,
        0,
    )?;
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
        data_symbol: None,
        lifetime_binders: &lifetime_binders,
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

fn project_public_propositions(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedReviewRow<PackageReviewPropositionShape>>, Vec<Diagnostic>> {
    use psi_typed_trees::proposition::{PropositionBody, PropositionFormula};

    let mut rows = Vec::new();
    for declaration in compilation
        .propositions()
        .iter()
        .filter(|declaration| declaration.is_public)
    {
        let identity = nominal_identity(compilation, declaration.symbol)?;
        if !reviewed_package_owns(&identity, package)? {
            continue;
        }
        let (binders, parameter_types) = project_proposition_signature(compilation, declaration)?;
        let body = match &declaration.body {
            PropositionBody::Primitive | PropositionBody::Witness { .. } => {
                let matching = compilation
                    .facts
                    .proof
                    .proposition_vocabulary
                    .declarations
                    .iter()
                    .filter(|checked| checked.symbol == declaration.symbol)
                    .collect::<Vec<_>>();
                let [checked] = matching.as_slice() else {
                    return Err(vec![Diagnostic::error(format!(
                        "public proposition `{}` has {} checked declaration rows; expected one",
                        identity.path,
                        matching.len()
                    ))]);
                };
                if !checked.is_public {
                    return Err(vec![Diagnostic::error(format!(
                        "public proposition `{}` lost visibility during checked lowering",
                        identity.path
                    ))]);
                }
                match declaration.body {
                    PropositionBody::Primitive => PackageReviewPublicPropositionBody::Primitive,
                    PropositionBody::Witness { evidence } => {
                        let declaration_binders = compilation.proposition_binders(declaration);
                        let binder_symbols = declaration_binders
                            .iter()
                            .enumerate()
                            .map(|(position, binder)| {
                                (binder.symbol, format!("proposition-binder:{position}"))
                            })
                            .collect::<Vec<_>>();
                        PackageReviewPublicPropositionBody::Witness(project_evidence_interface(
                            compilation,
                            evidence,
                            &binder_symbols,
                        )?)
                    }
                    PropositionBody::Transparent { .. } => unreachable!(),
                }
            }
            PropositionBody::Transparent { proposition } => {
                let parameters = compilation.proposition_parameters(declaration);
                let declaration_binders = compilation.proposition_binders(declaration);
                let binder_symbols = declaration_binders
                    .iter()
                    .enumerate()
                    .map(|(position, binder)| {
                        (binder.symbol, format!("proposition-binder:{position}"))
                    })
                    .collect::<Vec<_>>();
                let context = ContractProjectionContext {
                    subject_kind: "public proposition",
                    subject_name: &identity.path,
                    owner: psi_checked_trees::ContractProofFactOwner::Unknown,
                    point: psi_facts::ProgramPoint::Definition {
                        symbol: declaration.symbol,
                    },
                    parameters,
                    domain_symbol: None,
                    data_symbol: None,
                    lifetime_binders: &[],
                };
                let mut visiting = vec![declaration.symbol];
                let expansion = match proposition {
                    PropositionFormula::Application(application) => project_contract_proposition(
                        compilation,
                        &context,
                        &binder_symbols,
                        application,
                        None,
                        &[],
                        &[],
                        &mut visiting,
                        0,
                    )?,
                    PropositionFormula::BooleanExpression(expression) => {
                        PackageReviewContractFact::Expression(project_contract_expression(
                            compilation,
                            &context,
                            &binder_symbols,
                            *expression,
                            None,
                            0,
                        )?)
                    }
                };
                PackageReviewPublicPropositionBody::Transparent(expansion)
            }
        };
        rows.push(ProjectedReviewRow {
            row: PackageReviewPropositionShape {
                identity,
                binders,
                parameter_types,
                body,
            },
            declaration: declaration.symbol,
            nested_source_locations: Vec::new(),
        });
    }
    rows.sort_by(|left, right| left.row.identity.cmp(&right.row.identity));
    Ok(rows)
}

fn project_public_consts(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedReviewRow<PackageReviewConstShape>>, Vec<Diagnostic>> {
    let mut rows = Vec::new();
    for declaration in compilation
        .const_declarations()
        .iter()
        .filter(|declaration| declaration.is_public)
    {
        let identity = nominal_identity(compilation, declaration.symbol)?;
        if !reviewed_package_owns(&identity, package)? {
            continue;
        }
        let Some(canonical_value_encoding) = declaration.canonical_value_encoding.clone() else {
            return Err(vec![Diagnostic::error(format!(
                "public const `{}` has no canonical declaration value",
                identity.path
            ))]);
        };
        rows.push(ProjectedReviewRow {
            row: PackageReviewConstShape {
                identity,
                declared_type: review_type_identity_with_binders(
                    compilation,
                    declaration.declared_type,
                    &[],
                )?,
                canonical_value_encoding,
            },
            declaration: declaration.symbol,
            nested_source_locations: Vec::new(),
        });
    }
    rows.sort_by(|left, right| left.row.identity.cmp(&right.row.identity));
    Ok(rows)
}

fn project_operator_coordinate(
    compilation: &CheckedCompilation,
    declaration: &psi_typed_trees::operator::OperatorDefinition,
) -> Result<PackageReviewOperatorCoordinate, Vec<Diagnostic>> {
    let identity = nominal_identity(compilation, declaration.symbol)?;
    let overload = compilation.normalized_operator_overload_identity(declaration);
    Ok(PackageReviewOperatorCoordinate {
        identity,
        parameter_dispatch: overload.parameters().to_owned(),
        // Only explicitly named boundary requirements participate in
        // expected-result dispatch. Fixed tokens and ordinary named operators
        // remain operand-directed; their complete return type stays in the
        // row value so a change is one changed declaration, not remove/add.
        result_dispatch: if declaration.is_boundary && declaration.spelling.is_none() {
            overload.result_dispatch().identity()
        } else {
            String::new()
        },
    })
}

fn project_public_operators(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedReviewRow<PackageReviewOperatorShape>>, Vec<Diagnostic>> {
    let derived = psi_typed_trees_to_checked_trees::derive_checked_operator_crash_contracts(
        &compilation.typed,
    );
    if derived != compilation.facts.operators.operator_crash_contracts {
        return Err(vec![Diagnostic::error(format!(
            "retained checked operator-crash evidence does not equal compiler rederivation (retained {} rows, derived {} rows)",
            compilation.facts.operators.operator_crash_contracts.len(),
            derived.len(),
        ))]);
    }
    let mut rows = Vec::new();
    let operators = compilation.operators().iter().chain(
        compilation
            .domain_definitions()
            .iter()
            .flat_map(|domain| compilation.domain_operators(domain)),
    );
    for declaration in operators.filter(|declaration| declaration.is_public) {
        let coordinate = project_operator_coordinate(compilation, declaration)?;
        if !reviewed_package_owns(&coordinate.identity, package)? {
            continue;
        }
        let declaration_path = coordinate.identity.path.as_str();
        let declaration_type_parameters = compilation.operator_type_parameters(declaration);
        let (binders, type_parameters) = project_type_parameters(
            compilation,
            declaration_type_parameters,
            "operator",
            declaration_path,
            &declaration.lifetime_parameters,
        )?;
        let parameters = compilation
            .operator_parameters(declaration)
            .iter()
            .map(|parameter| {
                Ok(PackageReviewCallableParameter {
                    name: parameter.name.as_str().to_owned(),
                    type_identity: review_signature_type_identity_with_binders(
                        compilation,
                        parameter.type_reference,
                        &binders,
                        &declaration.lifetime_parameters,
                    )?,
                    is_const: parameter.is_const,
                    is_mutable: parameter.is_mutable,
                    is_self: parameter.is_self,
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        let context = ContractProjectionContext {
            subject_kind: "public operator",
            subject_name: declaration_path,
            owner: psi_checked_trees::ContractProofFactOwner::OperatorDeclaration {
                operator_symbol: declaration.symbol,
            },
            point: psi_facts::ProgramPoint::Definition {
                symbol: declaration.symbol,
            },
            parameters: compilation.operator_parameters(declaration),
            domain_symbol: None,
            data_symbol: None,
            lifetime_binders: &declaration.lifetime_parameters,
        };
        let contracts = project_contracts(
            compilation,
            compilation.operator_contracts(declaration),
            &context,
            &binders,
        )?;
        let matching_crash = compilation
            .facts
            .operators
            .operator_crash_contracts
            .iter()
            .filter(|checked| checked.operator_symbol() == declaration.symbol)
            .collect::<Vec<_>>();
        let [checked_crash] = matching_crash.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "public operator `{declaration_path}` has {} exact checked crash-contract rows; expected one",
                matching_crash.len(),
            ))]);
        };
        let published_crash =
            project_operator_crash_routes(compilation, checked_crash, &context, &binders)?;
        let mut nested_source_locations =
            project_contract_clause_source_locations(compilation.operator_contracts(declaration));
        collect_type_parameter_source_locations(
            compilation,
            declaration_type_parameters,
            &mut nested_source_locations,
        )?;
        rows.push(ProjectedReviewRow {
            row: PackageReviewOperatorShape {
                coordinate,
                is_boundary: declaration.is_boundary,
                spelling: declaration.spelling,
                lifetime_parameter_count: declaration.lifetime_parameters.len(),
                type_parameters,
                parameters,
                return_type: review_signature_type_identity_with_binders(
                    compilation,
                    declaration.return_type,
                    &binders,
                    &declaration.lifetime_parameters,
                )?,
                contracts,
                published_crash,
            },
            declaration: declaration.symbol,
            nested_source_locations,
        });
    }
    rows.sort_by(|left, right| left.row.coordinate.cmp(&right.row.coordinate));
    if rows
        .windows(2)
        .any(|pair| pair[0].row.coordinate == pair[1].row.coordinate)
    {
        return Err(vec![Diagnostic::error(
            "public operator review produced a duplicate overload coordinate",
        )]);
    }
    Ok(rows)
}

fn project_operator_crash_routes(
    compilation: &CheckedCompilation,
    checked: &psi_checked_trees::CheckedOperatorCrashContract,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
) -> Result<Vec<PackageReviewCrashRoute>, Vec<Diagnostic>> {
    use psi_typed_trees::domain::ProofFact;

    checked
        .buckets()
        .iter()
        .map(|bucket| {
            let alternative_guards = if bucket.is_unconditional() {
                if !bucket.facts().is_empty() {
                    return Err(vec![Diagnostic::error(format!(
                        "public operator `{}` has an unconditional checked crash bucket with retained guarded facts",
                        context.subject_name
                    ))]);
                }
                vec![PackageReviewCrashRouteGuard::Truth]
            } else {
                let mut guards = bucket
                    .facts()
                    .iter()
                    .map(|fact| {
                        let ProofFact::Expression(expression) = compilation.proof_facts.get(*fact)
                        else {
                            return Err(vec![Diagnostic::error(format!(
                                "public operator `{}` has a non-expression checked crash route",
                                context.subject_name
                            ))]);
                        };
                        project_contract_expression(
                            compilation,
                            context,
                            binders,
                            *expression,
                            Some(*fact),
                            0,
                        )
                        .map(PackageReviewCrashRouteGuard::Expression)
                    })
                    .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
                guards.sort();
                guards.dedup();
                if guards.is_empty() {
                    return Err(vec![Diagnostic::error(format!(
                        "public operator `{}` has an empty guarded checked crash bucket",
                        context.subject_name
                    ))]);
                }
                guards
            };
            Ok(PackageReviewCrashRoute {
                cause: bucket.cause(),
                alternative_guards,
            })
        })
        .collect()
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
        let parameters = compilation.domain_type_parameters(definition);
        let (binders, type_parameters) =
            project_type_parameters(compilation, parameters, "domain", &identity.path, &[])?;
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
        let semantic_roles = project_domain_semantic_roles(definition, &identity)?;
        rows.push(ProjectedReviewRow {
            row: PackageReviewDomainShape {
                identity,
                type_parameters,
                target_type: review_type_identity_with_binders(
                    compilation,
                    definition.target_type,
                    &binders,
                )?,
                index_arguments: definition
                    .index_arguments
                    .iter()
                    .map(|argument| {
                        review_type_identity_with_binders(compilation, *argument, &binders)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                predicate_body: definition.predicate_body,
                predicate_facts,
                alias_expansion,
                classification,
                semantic_roles,
                establishment_routes,
            },
            declaration: definition.symbol,
            nested_source_locations: {
                let mut locations = Vec::new();
                collect_type_parameter_source_locations(compilation, parameters, &mut locations)?;
                locations
            },
        });
    }
    rows.sort_by(|left, right| left.row.identity.cmp(&right.row.identity));
    Ok(rows)
}

fn project_domain_semantic_roles(
    definition: &psi_typed_trees::domain::DomainDefinition,
    identity: &PackageReviewNominalIdentity,
) -> Result<Vec<PackageReviewDomainSemanticRole>, Vec<Diagnostic>> {
    let mut roles = Vec::new();
    for (role, semantic_identity) in [
        (
            PackageReviewDomainSemanticRole::DenotationDimension,
            definition.semantic_roles.denotation_dimension,
        ),
        (
            PackageReviewDomainSemanticRole::ArithmeticPolicy,
            definition.semantic_roles.arithmetic_policy,
        ),
    ] {
        let Some(semantic_identity) = semantic_identity else {
            continue;
        };
        if semantic_identity != definition.semantic_id {
            return Err(vec![Diagnostic::error(format!(
                "public domain `{}` semantic role does not name its exact typed semantic identity",
                identity.path
            ))]);
        }
        roles.push(role);
    }
    Ok(roles)
}

fn project_domain_predicate_facts(
    compilation: &CheckedCompilation,
    definition: &psi_typed_trees::domain::DomainDefinition,
    identity: &PackageReviewNominalIdentity,
    binders: &[(SymbolHandle, String)],
) -> Result<Vec<PackageReviewContractFact>, Vec<Diagnostic>> {
    let context = ContractProjectionContext {
        subject_kind: "public domain",
        subject_name: &identity.path,
        owner: psi_checked_trees::ContractProofFactOwner::Unknown,
        point: psi_facts::ProgramPoint::Definition {
            symbol: definition.symbol,
        },
        parameters: &[],
        domain_symbol: Some(definition.symbol),
        data_symbol: None,
        lifetime_binders: &[],
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
        projected.push(project_definition_contract_fact(
            compilation,
            &context,
            binders,
            fact_handle,
            reviewed_package,
        )?);
    }
    projected.sort();
    projected.dedup();
    Ok(projected)
}

fn project_definition_contract_fact(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
    fact_handle: psi_arena::Handle<psi_typed_trees::domain::ProofFact>,
    reviewed_package: PackageKeyIdentity,
) -> Result<PackageReviewContractFact, Vec<Diagnostic>> {
    use psi_typed_trees::domain::ProofFact;

    match compilation.proof_facts.get(fact_handle) {
        ProofFact::Expression(expression) => Ok(PackageReviewContractFact::Expression(
            project_contract_expression(
                compilation,
                context,
                binders,
                *expression,
                Some(fact_handle),
                0,
            )?,
        )),
        ProofFact::Membership(membership) => {
            let domain = compilation
                .domain_definitions()
                .iter()
                .find(|domain| domain.symbol == membership.domain_symbol)
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "{} `{}` predicate refers to an unresolved domain",
                        context.subject_kind, context.subject_name
                    ))]
                })?;
            let domain_identity = nominal_identity(compilation, domain.symbol)?;
            if reviewed_package_owns(&domain_identity, reviewed_package)? && !domain.is_public {
                return Err(vec![Diagnostic::error(format!(
                    "{} `{}` predicate exposes non-public domain `{}`",
                    context.subject_kind, context.subject_name, domain.name
                ))]);
            }
            Ok(PackageReviewContractFact::Membership {
                value: project_contract_expression(
                    compilation,
                    context,
                    binders,
                    membership.value,
                    Some(fact_handle),
                    0,
                )?,
                domain: domain_identity,
            })
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
        ),
    }
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
                && semantic_fact_matches_definition_fact(compilation, fact, fact_handle))
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
    let retained_records = compilation
        .facts
        .semantic
        .domain_definition_facts
        .iter()
        .filter(|(_, record)| record.domain_symbol == domain_symbol && record.fact == fact_handle)
        .map(|(_, record)| record)
        .collect::<Vec<_>>();
    let matching_records = retained_records
        .iter()
        .filter(|record| record.semantic_fact == matching_rows[0])
        .count();
    if retained_records.len() != 1 || matching_records != 1 {
        return Err(vec![Diagnostic::error(format!(
            "public domain `{}` predicate fact has {matching_records} exact checked ownership records among {} retained records; expected exactly one retained record",
            identity.path,
            retained_records.len(),
        ))]);
    }
    Ok(())
}

fn semantic_fact_matches_definition_fact(
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
) -> Result<Vec<PackageReviewDomainAliasAtom>, Vec<Diagnostic>> {
    fn expand(
        compilation: &CheckedCompilation,
        domain_symbol: SymbolHandle,
        stack: &mut Vec<SymbolHandle>,
        atoms: &mut Vec<PackageReviewDomainAliasAtom>,
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
            atoms.push(PackageReviewDomainAliasAtom::Declared(nominal_identity(
                compilation,
                definition.symbol,
            )?));
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
            if !constituent.domain_symbol.is_valid() && label == "Carry::Portable" {
                atoms.extend(
                    psi_language_semantics::CarryPermission::ALL
                        .map(PackageReviewDomainAliasAtom::Carry),
                );
            } else if !constituent.domain_symbol.is_valid()
                && let Some(permission) = psi_language_semantics::CarryPermission::from_name(&label)
            {
                atoms.push(PackageReviewDomainAliasAtom::Carry(permission));
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
        requirement_identity: trait_requirement_identity(compilation, owner, requirement)?,
    })
}

fn project_data_invariant_facts(
    compilation: &CheckedCompilation,
    definition: &psi_typed_trees::data::DataDefinition,
    identity: &PackageReviewNominalIdentity,
    binders: &[(SymbolHandle, String)],
) -> Result<Vec<PackageReviewContractFact>, Vec<Diagnostic>> {
    let context = ContractProjectionContext {
        subject_kind: "public data",
        subject_name: &identity.path,
        owner: psi_checked_trees::ContractProofFactOwner::Unknown,
        point: psi_facts::ProgramPoint::Definition {
            symbol: definition.symbol,
        },
        parameters: &[],
        domain_symbol: None,
        data_symbol: Some(definition.symbol),
        lifetime_binders: &definition.lifetime_parameters,
    };
    let reviewed_package = compilation.package_identity().ok_or_else(|| {
        vec![Diagnostic::error(
            "data invariant review requires package-aware checked compilation",
        )]
    })?;
    let mut projected = Vec::new();
    for offset in 0..definition.where_facts.count() {
        let fact_handle = psi_arena::Handle::from_parts(
            definition
                .where_facts
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("data invariant fact handle index overflow"),
            definition.where_facts.start().generation(),
        );
        require_exact_checked_data_fact(compilation, definition.symbol, fact_handle, identity)?;
        projected.push(project_definition_contract_fact(
            compilation,
            &context,
            binders,
            fact_handle,
            reviewed_package,
        )?);
    }
    projected.sort();
    projected.dedup();
    Ok(projected)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecheckedDataDefinitionFact {
    data_symbol: SymbolHandle,
    fact: psi_arena::Handle<psi_typed_trees::domain::ProofFact>,
    semantic_fact: RecheckedSemanticFact,
    dependencies: Vec<RecheckedDataDefinitionFactDependency>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecheckedDataDefinitionFactDependency {
    expression: psi_typed_trees::expression::ExpressionHandle,
    place: RecheckedFactPlace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecheckedSemanticFact {
    place: RecheckedSemanticFactPlace,
    point: psi_facts::ProgramPoint,
    origin: psi_facts::FactOrigin,
    evidence: psi_facts::QualificationEvidence,
    payload: psi_facts::FactPayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RecheckedSemanticFactPlace {
    Unknown,
    Place(RecheckedFactPlace),
    Symbol(SymbolHandle),
    Expression(psi_typed_trees::expression::ExpressionHandle),
    TypeReference(psi_typed_trees::types::TypeReferenceHandle),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecheckedFactPlace {
    root: psi_facts::PlaceRoot,
    segments: Vec<psi_facts::PlaceSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecheckedDataDefinitionEvidence {
    definitions: Vec<RecheckedDataDefinitionFact>,
    semantic_facts: Vec<RecheckedSemanticFact>,
    refs: Vec<RecheckedSemanticFact>,
    contexts: Vec<RecheckedDataFactContext>,
    symbol_sets: Vec<RecheckedDataSymbolFactSet>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecheckedDataFactContext {
    point: psi_facts::ProgramPoint,
    facts: Vec<RecheckedSemanticFact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecheckedDataSymbolFactSet {
    symbol: SymbolHandle,
    facts: Vec<RecheckedSemanticFact>,
}

fn require_rederived_data_definition_facts(
    compilation: &CheckedCompilation,
) -> Result<(), Vec<Diagnostic>> {
    let rederived = psi_facts::build_definition_fact_plan(&compilation.typed);
    let data_symbols = compilation
        .data_definitions()
        .iter()
        .map(|definition| definition.symbol)
        .collect::<Vec<_>>();
    let Some(expected) = rechecked_data_definition_evidence(&rederived, &data_symbols) else {
        return Err(vec![Diagnostic::error(
            "compiler-rederived data invariant evidence is internally malformed",
        )]);
    };
    let Some(retained) =
        rechecked_data_definition_evidence(&compilation.facts.semantic, &data_symbols)
    else {
        return Err(vec![Diagnostic::error(
            "retained checked data invariant evidence is internally malformed",
        )]);
    };
    if retained != expected {
        return Err(vec![Diagnostic::error(
            "retained checked data invariant evidence disagrees with the compiler-rederived typed program",
        )]);
    }
    Ok(())
}

fn rechecked_data_definition_evidence(
    facts: &psi_facts::FactPlan,
    data_symbols: &[SymbolHandle],
) -> Option<RecheckedDataDefinitionEvidence> {
    fact_plan_arena_links_are_well_formed(facts).then_some(())?;
    let definitions = facts
        .data_definition_facts
        .iter()
        .map(|(_, record)| {
            let semantic_fact = rechecked_semantic_fact(facts, record.semantic_fact)?;
            let dependencies = record
                .dependencies
                .iter()
                .map(|dependency| {
                    Some(RecheckedDataDefinitionFactDependency {
                        expression: dependency.expression,
                        place: rechecked_fact_place(facts, dependency.place)?,
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            Some(RecheckedDataDefinitionFact {
                data_symbol: record.data_symbol,
                fact: record.fact,
                semantic_fact,
                dependencies,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let semantic_facts = facts
        .facts
        .iter()
        .filter_map(|(_, fact)| {
            matches!(fact.origin, psi_facts::FactOrigin::DataDefinition { .. })
                .then_some(rechecked_semantic_fact_value(facts, fact))
        })
        .collect::<Option<Vec<_>>>()?;
    let refs = facts
        .refs
        .iter()
        .filter_map(|(_, fact_ref)| {
            let fact = facts
                .facts
                .iter()
                .find_map(|(handle, fact)| (handle == fact_ref.fact).then_some(fact))?;
            matches!(fact.origin, psi_facts::FactOrigin::DataDefinition { .. })
                .then_some(rechecked_semantic_fact_value(facts, fact))
        })
        .collect::<Option<Vec<_>>>()?;
    let contexts = facts
        .contexts
        .iter()
        .filter_map(|(_, context)| {
            let at_data_definition = matches!(
                context.point,
                psi_facts::ProgramPoint::Definition { symbol }
                    if data_symbols.contains(&symbol)
            );
            let references = match facts.refs.span(context.facts) {
                Some(references) => references,
                None if at_data_definition => return Some(None),
                None => return None,
            };
            let contains_data_fact = references.iter().any(|fact_ref| {
                facts.facts.iter().any(|(handle, fact)| {
                    handle == fact_ref.fact
                        && matches!(fact.origin, psi_facts::FactOrigin::DataDefinition { .. })
                })
            });
            (at_data_definition || contains_data_fact).then(|| {
                Some(RecheckedDataFactContext {
                    point: context.point,
                    facts: references
                        .iter()
                        .map(|fact_ref| rechecked_semantic_fact(facts, fact_ref.fact))
                        .collect::<Option<Vec<_>>>()?,
                })
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let symbol_sets = facts
        .symbol_sets
        .iter()
        .filter_map(|(_, set)| {
            let references = match facts.refs.span(set.facts) {
                Some(references) => references,
                None if data_symbols.contains(&set.symbol) => return Some(None),
                None => return None,
            };
            let contains_data_fact = references.iter().any(|fact_ref| {
                facts.facts.iter().any(|(handle, fact)| {
                    handle == fact_ref.fact
                        && matches!(fact.origin, psi_facts::FactOrigin::DataDefinition { .. })
                })
            });
            (data_symbols.contains(&set.symbol) || contains_data_fact).then(|| {
                Some(RecheckedDataSymbolFactSet {
                    symbol: set.symbol,
                    facts: references
                        .iter()
                        .map(|fact_ref| rechecked_semantic_fact(facts, fact_ref.fact))
                        .collect::<Option<Vec<_>>>()?,
                })
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(RecheckedDataDefinitionEvidence {
        definitions,
        semantic_facts,
        refs,
        contexts,
        symbol_sets,
    })
}

fn fact_plan_arena_links_are_well_formed(facts: &psi_facts::FactPlan) -> bool {
    facts
        .places
        .iter()
        .all(|(_, place)| facts.place_segments.span(place.segments).is_some())
        && facts.facts.iter().all(|(_, fact)| match fact.place {
            psi_facts::FactPlace::Place(place) => facts.places.is_valid(place),
            psi_facts::FactPlace::Unknown
            | psi_facts::FactPlace::Symbol(_)
            | psi_facts::FactPlace::Expression(_)
            | psi_facts::FactPlace::TypeReference(_) => true,
        })
        && facts
            .refs
            .iter()
            .all(|(_, fact_ref)| facts.facts.is_valid(fact_ref.fact))
        && facts
            .contexts
            .iter()
            .all(|(_, context)| facts.refs.span(context.facts).is_some())
        && facts
            .symbol_sets
            .iter()
            .all(|(_, set)| facts.refs.span(set.facts).is_some())
}

fn rechecked_semantic_fact(
    facts: &psi_facts::FactPlan,
    fact_handle: psi_facts::FactHandle,
) -> Option<RecheckedSemanticFact> {
    let fact = facts
        .facts
        .iter()
        .find_map(|(handle, fact)| (handle == fact_handle).then_some(fact))?;
    rechecked_semantic_fact_value(facts, fact)
}

fn rechecked_semantic_fact_value(
    facts: &psi_facts::FactPlan,
    fact: &psi_facts::Fact,
) -> Option<RecheckedSemanticFact> {
    Some(RecheckedSemanticFact {
        place: rechecked_semantic_fact_place(facts, fact.place)?,
        point: fact.point,
        origin: fact.origin,
        evidence: fact.evidence,
        payload: fact.payload,
    })
}

fn rechecked_semantic_fact_place(
    facts: &psi_facts::FactPlan,
    place: psi_facts::FactPlace,
) -> Option<RecheckedSemanticFactPlace> {
    Some(match place {
        psi_facts::FactPlace::Unknown => RecheckedSemanticFactPlace::Unknown,
        psi_facts::FactPlace::Place(place) => {
            RecheckedSemanticFactPlace::Place(rechecked_fact_place(facts, place)?)
        }
        psi_facts::FactPlace::Symbol(symbol) => RecheckedSemanticFactPlace::Symbol(symbol),
        psi_facts::FactPlace::Expression(expression) => {
            RecheckedSemanticFactPlace::Expression(expression)
        }
        psi_facts::FactPlace::TypeReference(type_reference) => {
            RecheckedSemanticFactPlace::TypeReference(type_reference)
        }
    })
}

fn rechecked_fact_place(
    facts: &psi_facts::FactPlan,
    place_handle: psi_facts::PlaceHandle,
) -> Option<RecheckedFactPlace> {
    let place = facts
        .places
        .iter()
        .find_map(|(handle, place)| (handle == place_handle).then_some(place))?;
    Some(RecheckedFactPlace {
        root: place.root,
        segments: facts.place_segments.span(place.segments)?.to_vec(),
    })
}

fn require_exact_checked_data_fact(
    compilation: &CheckedCompilation,
    data_symbol: SymbolHandle,
    fact_handle: psi_arena::Handle<psi_typed_trees::domain::ProofFact>,
    identity: &PackageReviewNominalIdentity,
) -> Result<(), Vec<Diagnostic>> {
    let point = psi_facts::ProgramPoint::Definition {
        symbol: data_symbol,
    };
    let matching_rows = compilation
        .facts
        .semantic
        .facts
        .iter()
        .filter_map(|(handle, fact)| {
            (fact.point == point
                && fact.origin == psi_facts::FactOrigin::DataDefinition { data_symbol }
                && fact.evidence == psi_facts::QualificationEvidence::default()
                && semantic_fact_matches_definition_fact(compilation, fact, fact_handle))
            .then_some(handle)
        })
        .collect::<Vec<_>>();
    if matching_rows.len() != 1 {
        return Err(vec![Diagnostic::error(format!(
            "public data `{}` invariant fact has {} exact checked definition rows; expected one",
            identity.path,
            matching_rows.len()
        ))]);
    }
    let retained_records = compilation
        .facts
        .semantic
        .data_definition_facts
        .iter()
        .filter(|(_, record)| record.data_symbol == data_symbol && record.fact == fact_handle)
        .map(|(_, record)| record)
        .collect::<Vec<_>>();
    let matching_records = retained_records
        .iter()
        .filter(|record| record.semantic_fact == matching_rows[0])
        .count();
    if retained_records.len() != 1 || matching_records != 1 {
        return Err(vec![Diagnostic::error(format!(
            "public data `{}` invariant fact has {matching_records} exact checked ownership records among {} retained records; expected exactly one retained record",
            identity.path,
            retained_records.len(),
        ))]);
    }
    Ok(())
}

fn project_public_data(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedReviewRow<PackageReviewDataShape>>, Vec<Diagnostic>> {
    require_rederived_data_definition_facts(compilation)?;
    let quotient_formations = psi_validation::validate_quotient_formations(compilation)?;
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
        let parameters = compilation.data_type_parameters(definition);
        let (binders, type_parameters) = project_type_parameters(
            compilation,
            parameters,
            "data",
            &identity.path,
            &definition.lifetime_parameters,
        )?;
        let kind = if definition.quotient.is_some() {
            let matching_formations = quotient_formations
                .iter()
                .filter(|formation| formation.data_symbol == definition.symbol)
                .collect::<Vec<_>>();
            let [formation] = matching_formations.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "public quotient data `{}` has {} independently rederived formation rows; expected one",
                    identity.path,
                    matching_formations.len()
                ))]);
            };
            let matching_relations = compilation
                .propositions()
                .iter()
                .filter(|relation| relation.symbol == formation.relation_symbol)
                .collect::<Vec<_>>();
            let [relation] = matching_relations.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "public quotient data `{}` has {} exact relation declarations; expected one",
                    identity.path,
                    matching_relations.len()
                ))]);
            };
            if !relation.is_public {
                return Err(vec![Diagnostic::error(format!(
                    "public quotient data `{}` exposes non-public relation `{}`",
                    identity.path, relation.name
                ))]);
            }
            PackageReviewDataKind::Quotient {
                carrier: review_signature_type_identity_with_binders(
                    compilation,
                    formation.carrier,
                    &binders,
                    &definition.lifetime_parameters,
                )?,
                relation: nominal_identity(compilation, formation.relation_symbol)?,
            }
        } else {
            PackageReviewDataKind::Ordinary
        };
        let invariants =
            project_data_invariant_facts(compilation, definition, &identity, &binders)?;

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
                kind,
                supply: definition.supply_mode,
                lifetime_parameter_count: definition.lifetime_parameters.len(),
                type_parameters,
                properties: definition.properties,
                zero_gated: definition.zero_gated,
                invariants,
                retired_identities,
                members,
            },
            declaration: definition.symbol,
            nested_source_locations: {
                let mut locations = Vec::new();
                collect_type_parameter_source_locations(compilation, parameters, &mut locations)?;
                locations
            },
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
    lifetime_binders: &[psi_typed_trees::name::Identifier],
) -> Result<(Vec<(SymbolHandle, String)>, Vec<PackageReviewTypeParameter>), Vec<Diagnostic>> {
    project_type_parameters_after(
        compilation,
        parameters,
        declaration_kind,
        declaration_path,
        &[],
        0,
        lifetime_binders,
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
    lifetime_binders: &[psi_typed_trees::name::Identifier],
    depth: usize,
) -> Result<(Vec<(SymbolHandle, String)>, Vec<PackageReviewTypeParameter>), Vec<Diagnostic>> {
    if depth >= 64 {
        return Err(vec![Diagnostic::error(format!(
            "public {declaration_kind} `{declaration_path}` static-machine contract exceeds the package-review depth limit",
        ))]);
    }
    let mut binders = preceding_binders.to_vec();
    binders.extend(parameters.iter().enumerate().map(|(ordinal, parameter)| {
        (
            parameter.symbol,
            format!("type-parameter:{}", ordinal_offset + ordinal),
        )
    }));
    let mut projected = Vec::with_capacity(parameters.len());
    for parameter in parameters {
        let kind = match &parameter.kind {
            psi_typed_trees::data::TypeParameterKind::Type => PackageReviewTypeParameterKind::Type,
            psi_typed_trees::data::TypeParameterKind::Const { type_reference } => {
                PackageReviewTypeParameterKind::Const(review_signature_type_identity_with_binders(
                    compilation,
                    *type_reference,
                    &binders,
                    lifetime_binders,
                )?)
            }
            psi_typed_trees::data::TypeParameterKind::Machine { contract } => {
                PackageReviewTypeParameterKind::Machine(project_machine_parameter_contract(
                    compilation,
                    parameter.symbol,
                    contract,
                    declaration_kind,
                    declaration_path,
                    &binders,
                    ordinal_offset + parameters.len(),
                    lifetime_binders,
                    depth + 1,
                )?)
            }
            psi_typed_trees::data::TypeParameterKind::Proposition { contract } => {
                let mut projected_parameters = Vec::new();
                for value_parameter in compilation
                    .typed
                    .state_parameters
                    .span_or_empty(contract.parameters)
                {
                    if value_parameter.is_const
                        || value_parameter.is_mutable
                        || value_parameter.is_self
                    {
                        return Err(vec![Diagnostic::error(format!(
                            "public {declaration_kind} `{declaration_path}` proposition parameter uses a non-default value-parameter mode not yet certified by package review",
                        ))]);
                    }
                    projected_parameters.push(PackageReviewPropositionParameterValue {
                        type_identity: review_signature_type_identity_with_binders(
                            compilation,
                            value_parameter.type_reference,
                            &binders,
                            lifetime_binders,
                        )?,
                    });
                }
                PackageReviewTypeParameterKind::Proposition(
                    PackageReviewPropositionParameterSignature {
                        parameters: projected_parameters,
                    },
                )
            }
        };
        projected.push(PackageReviewTypeParameter {
            kind,
            bounds: parameter.bounds,
        });
    }
    Ok((binders, projected))
}

#[allow(clippy::too_many_arguments)]
fn project_machine_parameter_contract(
    compilation: &CheckedCompilation,
    parameter_symbol: SymbolHandle,
    contract: &psi_typed_trees::data::MachineParameterContract,
    declaration_kind: &str,
    declaration_path: &str,
    outer_binders: &[(SymbolHandle, String)],
    nested_ordinal_offset: usize,
    outer_lifetime_binders: &[psi_typed_trees::name::Identifier],
    depth: usize,
) -> Result<PackageReviewMachineParameterContract, Vec<Diagnostic>> {
    match contract {
        psi_typed_trees::data::MachineParameterContract::RequirementIdentity => {
            Ok(PackageReviewMachineParameterContract::RequirementIdentity)
        }
        psi_typed_trees::data::MachineParameterContract::Structural(signature) => {
            if signature.spelling.is_some() || signature.is_default {
                return Err(vec![Diagnostic::error(format!(
                    "public {declaration_kind} `{declaration_path}` has a structural static-machine contract with trait-only requirement metadata",
                ))]);
            }
            let mut lifetime_binders = outer_lifetime_binders.to_vec();
            lifetime_binders.extend(signature.lifetime_parameters.iter().cloned());
            let (binders, type_parameters) = project_type_parameters_after(
                compilation,
                compilation.state_signature_type_parameters(signature),
                declaration_kind,
                declaration_path,
                outer_binders,
                nested_ordinal_offset,
                &lifetime_binders,
                depth,
            )?;
            let parameters = compilation.state_signature_parameters(signature);
            let context = ContractProjectionContext {
                subject_kind: "public static-machine parameter",
                subject_name: declaration_path,
                owner: psi_checked_trees::ContractProofFactOwner::StateSignature {
                    owner_symbol: parameter_symbol,
                    state_symbol: signature.symbol,
                },
                point: psi_facts::ProgramPoint::State {
                    machine_symbol: parameter_symbol,
                    state_symbol: signature.symbol,
                },
                parameters,
                domain_symbol: None,
                data_symbol: None,
                lifetime_binders: &lifetime_binders,
            };
            let contracts = project_contracts(
                compilation,
                compilation.state_signature_contracts(signature),
                &context,
                &binders,
            )?;
            let published_crash = project_signature_crash_routes(
                compilation,
                parameter_symbol,
                signature.symbol,
                "public static-machine parameter",
                declaration_path,
            )?;
            Ok(PackageReviewMachineParameterContract::Structural(
                PackageReviewMachineParameterSignature {
                    lifetime_parameter_count: signature.lifetime_parameters.len(),
                    type_parameters,
                    parameters: parameters
                        .iter()
                        .map(|parameter| {
                            Ok(PackageReviewMachineParameterValue {
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
                        signature.return_type,
                        &binders,
                        &lifetime_binders,
                    )?,
                    contracts,
                    published_crash,
                    service_reach: project_service_row(compilation, signature.service_reach_row)?,
                    service_reach_is_installation_bound: signature
                        .service_reach_is_installation_bound,
                    synchronous_invocations: project_synchronous_invocations(
                        compilation,
                        &psi_effects::declared_signature_invocations(compilation, signature),
                    )?,
                    suspends: signature.suspends,
                    blocks: signature.blocks,
                    termination: project_machine_parameter_termination(
                        compilation,
                        signature,
                        declaration_path,
                    )?,
                },
            ))
        }
        psi_typed_trees::data::MachineParameterContract::Nominal {
            trait_definition,
            requirement,
        } => {
            let matching_traits = compilation
                .traits()
                .iter()
                .filter(|candidate| candidate.symbol == *trait_definition)
                .collect::<Vec<_>>();
            let [trait_definition] = matching_traits.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "public {declaration_kind} `{declaration_path}` static-machine contract resolves its nominal trait to {} declarations; expected exactly one",
                    matching_traits.len(),
                ))]);
            };
            let matching_requirements = compilation
                .trait_machine_signatures(trait_definition)
                .iter()
                .filter(|candidate| candidate.symbol == *requirement)
                .collect::<Vec<_>>();
            let [requirement] = matching_requirements.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "public {declaration_kind} `{declaration_path}` static-machine contract resolves its nominal requirement to {} declarations in trait `{}`; expected exactly one",
                    matching_requirements.len(),
                    trait_definition.name,
                ))]);
            };
            if !trait_definition.is_public {
                return Err(vec![Diagnostic::error(format!(
                    "public {declaration_kind} `{declaration_path}` exposes non-public trait `{}` through a static-machine contract",
                    trait_definition.name,
                ))]);
            }
            let trait_identity = nominal_identity(compilation, trait_definition.symbol)?;
            let requirement_identity =
                trait_requirement_identity(compilation, trait_definition, requirement)?;
            if trait_identity.owner != requirement_identity.owner {
                return Err(vec![Diagnostic::error(format!(
                    "public {declaration_kind} `{declaration_path}` static-machine contract has mismatched trait and requirement ownership",
                ))]);
            }
            Ok(PackageReviewMachineParameterContract::Nominal {
                trait_identity,
                requirement_identity,
            })
        }
    }
}

fn project_signature_crash_routes(
    compilation: &CheckedCompilation,
    target_machine: SymbolHandle,
    target_state: SymbolHandle,
    subject_kind: &str,
    subject_name: &str,
) -> Result<Vec<PackageReviewCrashRoute>, Vec<Diagnostic>> {
    let matching = compilation
        .facts
        .contract_plans
        .crash_capsules
        .iter()
        .filter(|capsule| {
            capsule.target_machine() == target_machine && capsule.target_state() == target_state
        })
        .collect::<Vec<_>>();
    let [capsule] = matching.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "{subject_kind} `{subject_name}` has {} exact checked crash capsules; expected one",
            matching.len(),
        ))]);
    };
    Ok(project_crash_routes(capsule.published_buckets()))
}

struct ProjectedSelectedConformanceApplication {
    declaration: PackageReviewNominalIdentity,
    lifetime_arguments: Vec<u32>,
    arguments: Vec<PackageReviewContractStaticArgument>,
    subject: PackageReviewContractStaticArgument,
    trait_symbol: SymbolHandle,
    trait_arguments: Vec<PackageReviewTypeIdentity>,
}

fn selected_conformance_application_type_reference(
    compilation: &mut CheckedCompilation,
    argument: &psi_typed_trees::expression::StaticMachineArgument,
    parameter_kind: ContractCallStaticParameterKind,
    subject_kind: &str,
    subject_name: &str,
    depth: usize,
) -> Result<psi_typed_trees::types::TypeReferenceHandle, Vec<Diagnostic>> {
    use psi_typed_trees::types::TypeReferenceNode;

    let rejected = |reason: &str| {
        vec![Diagnostic::error(format!(
            "reviewed {subject_kind} `{subject_name}` selected conformance has {reason}",
        ))]
    };
    if depth >= 64 {
        return Err(rejected(
            "an application deeper than the portable review limit",
        ));
    }
    if argument.evidence_projection.is_some()
        || parameter_kind == ContractCallStaticParameterKind::Proposition
    {
        return Err(rejected(
            "a proposition or evidence-projection argument not represented by package review",
        ));
    }
    if let Some(literal) = argument.const_literal.as_ref() {
        if parameter_kind != ContractCallStaticParameterKind::Const {
            return Err(rejected("a literal in a non-const telescope slot"));
        }
        return Ok(compilation
            .typed
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: psi_typed_trees::name::Identifier::generated(literal.text()),
            }));
    }
    if let Some(application) = argument.application.as_ref() {
        if parameter_kind != ContractCallStaticParameterKind::Type
            || !argument.symbol.is_valid()
            || compilation.typed.symbols.get(argument.symbol).kind != psi_symbols::SymbolKind::Data
        {
            return Err(rejected(
                "a nested non-data application in its declaration telescope",
            ));
        }
        let definition = compilation
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == argument.symbol)
            .cloned()
            .ok_or_else(|| rejected("a nested data application without one exact declaration"))?;
        if definition.lifetime_parameters.len() != application.lifetime_arguments.len() {
            return Err(rejected(
                "a nested data application with the wrong lifetime arity",
            ));
        }
        let parameters = compilation.data_type_parameters(&definition).to_vec();
        if parameters.len() != application.arguments.len() {
            return Err(rejected(
                "a nested data application with the wrong static arity",
            ));
        }
        let mut children = Vec::with_capacity(parameters.len());
        for (child, parameter) in application.arguments.iter().zip(&parameters) {
            children.push(selected_conformance_application_type_reference(
                compilation,
                child,
                contract_call_static_parameter_kind(parameter),
                subject_kind,
                subject_name,
                depth + 1,
            )?);
        }
        let arguments = compilation
            .typed
            .type_reference_table
            .insert_type_reference_handles(children);
        return Ok(compilation
            .typed
            .type_reference_table
            .insert(TypeReferenceNode::Generic {
                base_symbol: definition.symbol,
                base_name: definition.name,
                lifetime_arguments: application.lifetime_arguments.to_vec(),
                arguments,
            }));
    }
    if !argument.symbol.is_valid() {
        return Err(rejected("an unresolved declaration argument"));
    }
    let name = argument.path.last().cloned().unwrap_or_else(|| {
        psi_typed_trees::name::Identifier::generated(
            compilation.typed.symbols.name(argument.symbol),
        )
    });
    Ok(compilation
        .typed
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: argument.symbol,
            name,
        }))
}

fn project_selected_conformance_application(
    compilation: &CheckedCompilation,
    selected: &psi_typed_trees::expression::StaticMachineArgument,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[psi_typed_trees::name::Identifier],
    declaration_kind: &str,
    declaration_path: &str,
) -> Result<ProjectedSelectedConformanceApplication, Vec<Diagnostic>> {
    use psi_typed_trees::trait_definition::ConformanceSubject;

    let closed = psi_typed_trees_to_checked_trees::close_conformance_application(
        &compilation.typed,
        selected,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    let declarations = compilation
        .conformances()
        .iter()
        .filter(|declaration| declaration.symbol == selected.symbol)
        .collect::<Vec<_>>();
    let [declaration] = declarations.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "{declaration_kind} `{declaration_path}` resolves its selected conformance application to {} declarations; expected exactly one",
            declarations.len()
        ))]);
    };
    if !declaration.is_public {
        return Err(vec![Diagnostic::error(format!(
            "{declaration_kind} `{declaration_path}` exposes non-public selected conformance `{}`",
            declaration
                .alias
                .as_ref()
                .map_or("<unnamed>", |name| name.as_str())
        ))]);
    }
    if closed.declaration != declaration.symbol {
        return Err(vec![Diagnostic::error(format!(
            "{declaration_kind} `{declaration_path}` selected conformance closure changed declaration identity"
        ))]);
    }
    let parameters = compilation.conformance_type_parameters(declaration);
    let supplied = selected
        .application
        .as_ref()
        .map_or(&[][..], |application| application.arguments.as_ref());
    if parameters.len() != supplied.len() {
        return Err(vec![Diagnostic::error(format!(
            "{declaration_kind} `{declaration_path}` selected conformance application has inconsistent checked arity"
        ))]);
    }
    let arguments = supplied
        .iter()
        .zip(parameters)
        .map(|(argument, parameter)| {
            project_static_argument(
                compilation,
                declaration_kind,
                declaration_path,
                binders,
                lifetime_binders,
                argument,
                contract_call_static_parameter_kind(parameter),
                0,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let lifetime_arguments = selected
        .application
        .as_ref()
        .map_or(&[][..], |application| {
            application.lifetime_arguments.as_ref()
        })
        .iter()
        .map(|lifetime| {
            lifetime_binder_ordinal(
                lifetime,
                lifetime_binders,
                "selected conformance application",
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let subject = match &declaration.subject {
        ConformanceSubject::Subjectless => {
            return Err(vec![Diagnostic::error(format!(
                "{declaration_kind} `{declaration_path}` selects a subjectless conformance for a type-parameter bound"
            ))]);
        }
        ConformanceSubject::Carrier(_) => {
            if let Some(position) = parameters
                .iter()
                .position(|parameter| parameter.symbol == declaration.carrier_symbol)
            {
                let subject = arguments[position].clone();
                if !matches!(
                    subject,
                    PackageReviewContractStaticArgument::Type(_)
                        | PackageReviewContractStaticArgument::GenericTypeBinder(_)
                        | PackageReviewContractStaticArgument::GenericType { .. }
                ) {
                    return Err(vec![Diagnostic::error(format!(
                        "{declaration_kind} `{declaration_path}` selected conformance instantiates its subject from a non-type argument"
                    ))]);
                }
                subject
            } else {
                let mut projected = compilation.clone();
                let carrier = projected
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.symbol == declaration.carrier_symbol)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(format!(
                            "{declaration_kind} `{declaration_path}` selected conformance has no exact nominal subject"
                        ))]
                    })?;
                if !carrier.is_public {
                    return Err(vec![Diagnostic::error(format!(
                        "{declaration_kind} `{declaration_path}` exposes non-public selected-conformance subject `{}`",
                        carrier.name
                    ))]);
                }
                let carrier_name = carrier.name.clone();
                let carrier = projected.typed.type_reference_table.insert(
                    psi_typed_trees::types::TypeReferenceNode::Named {
                        symbol: declaration.carrier_symbol,
                        name: carrier_name,
                    },
                );
                PackageReviewContractStaticArgument::Type(
                    review_signature_type_identity_with_binders(
                        &projected,
                        carrier,
                        binders,
                        lifetime_binders,
                    )?,
                )
            }
        }
    };

    let mut instantiated = compilation.clone();
    let mut substitutions = Vec::with_capacity(parameters.len());
    for (parameter, argument) in parameters.iter().zip(supplied) {
        substitutions.push((
            parameter.symbol,
            selected_conformance_application_type_reference(
                &mut instantiated,
                argument,
                contract_call_static_parameter_kind(parameter),
                declaration_kind,
                declaration_path,
                0,
            )?,
        ));
    }
    let selected_lifetimes = selected
        .application
        .as_ref()
        .map_or(&[][..], |application| {
            application.lifetime_arguments.as_ref()
        });
    let lifetime_substitutions = declaration
        .lifetime_parameters
        .iter()
        .cloned()
        .zip(selected_lifetimes.iter().cloned())
        .collect::<Vec<_>>();
    let trait_arguments = compilation
        .type_reference_table
        .type_reference_handles(declaration.arguments)
        .iter()
        .map(|argument| {
            review_signature_type_identity_with_binders_and_substitutions_and_lifetimes(
                &instantiated,
                *argument,
                binders,
                lifetime_binders,
                &substitutions,
                &lifetime_substitutions,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    if closed.trait_definition != declaration.trait_symbol
        || closed.trait_arguments.len() != trait_arguments.len()
    {
        return Err(vec![Diagnostic::error(format!(
            "{declaration_kind} `{declaration_path}` selected conformance closure disagrees with its exact instantiated trait application"
        ))]);
    }
    Ok(ProjectedSelectedConformanceApplication {
        declaration: nominal_identity(compilation, declaration.symbol)?,
        lifetime_arguments,
        arguments,
        subject,
        trait_symbol: declaration.trait_symbol,
        trait_arguments,
    })
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
        let (
            selected_conformance,
            selected_lifetime_arguments,
            selected_arguments,
            selected_subject,
            trait_symbol,
            trait_arguments,
        ) = match bound.selected_conformance.as_ref() {
            None => (
                None,
                Vec::new(),
                Vec::new(),
                None,
                bound.carrier,
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
            ),
            Some(selected) => {
                let selected = project_selected_conformance_application(
                    compilation,
                    selected,
                    binders,
                    lifetime_binders,
                    declaration_kind,
                    declaration_path,
                )?;
                (
                    Some(selected.declaration),
                    selected.lifetime_arguments,
                    selected.arguments,
                    Some(selected.subject),
                    selected.trait_symbol,
                    selected.trait_arguments,
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
            selected_lifetime_arguments,
            selected_arguments,
            selected_subject,
            trait_identity: nominal_identity(compilation, trait_definition.symbol)?,
            arguments: trait_arguments,
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
) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
    validate_package_type_identity_input(&compilation.typed, type_reference, binders)?;
    let identity = compilation
        .package_qualified_type_identity_with_binders_and_toolchain_sources(
            type_reference,
            binders,
            compilation.exact_toolchain_sources(),
        )
        .ok_or_else(missing_exact_toolchain_type_owner)?;
    Ok(PackageReviewTypeIdentity {
        canonical: identity.into_string(),
    })
}

fn review_type_identity_with_binders_and_substitutions(
    compilation: &CheckedCompilation,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    substitutions: &[(SymbolHandle, psi_typed_trees::types::TypeReferenceHandle)],
) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
    validate_package_type_identity_input(&compilation.typed, type_reference, binders)?;
    let identity = compilation
        .package_qualified_type_identity_with_binders_substitutions_and_toolchain_sources(
            type_reference,
            binders,
            substitutions,
            compilation.exact_toolchain_sources(),
        )
        .ok_or_else(missing_exact_toolchain_type_owner)?;
    Ok(PackageReviewTypeIdentity {
        canonical: identity.into_string(),
    })
}

fn validate_package_type_identity_input(
    program: &psi_typed_trees::TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
) -> Result<(), Vec<Diagnostic>> {
    validate_package_type_identity_input_inner(program, type_reference, binders, false)
}

fn validate_package_type_identity_input_inner(
    program: &psi_typed_trees::TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    allow_const_value: bool,
) -> Result<(), Vec<Diagnostic>> {
    use psi_typed_trees::types::{FixedArrayLength, TypeConstraintNode, TypeReferenceNode};

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            validate_package_type_identity_input_inner(program, *referee, binders, false)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            validate_package_type_identity_input_inner(program, *base_type, binders, false)?;
            for constraint in program.type_reference_table.constraints(*constraints) {
                match constraint {
                    TypeConstraintNode::Range { minimum, maximum } => {
                        validate_package_index_expression(program, *minimum, binders)?;
                        validate_package_index_expression(program, *maximum, binders)?;
                    }
                    TypeConstraintNode::Domain(domain) => {
                        use psi_typed_trees::types::DomainConstraintSubject;

                        match domain.subject {
                            DomainConstraintSubject::Declared => {
                                if domain.name.as_str() == "OmegaLayout"
                                    || psi_typed_trees::wire::is_layout_domain_name(
                                        domain.name.as_str(),
                                    )
                                {
                                    return Err(vec![Diagnostic::error(
                                        "package review rejects an unclassified or legacy flattened OmegaLayout constraint",
                                    )]);
                                }
                                if !domain.symbol.is_valid() {
                                    return Err(vec![Diagnostic::error(
                                        "package review rejects a declared domain without an exact symbol",
                                    )]);
                                }
                            }
                            DomainConstraintSubject::Carry(_)
                            | DomainConstraintSubject::Value(_) => {
                                if domain.symbol.is_valid() || !domain.arguments.is_empty() {
                                    return Err(vec![Diagnostic::error(
                                        "package review rejects a malformed compiler-owned scalar domain constraint",
                                    )]);
                                }
                            }
                            DomainConstraintSubject::OmegaLayout { .. } => {
                                if domain.symbol.is_valid() || domain.arguments.len() != 1 {
                                    return Err(vec![Diagnostic::error(
                                        "package review rejects a malformed compiler-owned OmegaLayout constraint",
                                    )]);
                                }
                            }
                        }
                        let declared_parameters = (domain.subject
                            == psi_typed_trees::types::DomainConstraintSubject::Declared)
                            .then(|| {
                                program
                                    .domain_definitions()
                                    .iter()
                                    .find(|definition| definition.symbol == domain.symbol)
                            })
                            .flatten()
                            .map(|definition| program.domain_type_parameters(definition));
                        for (index, argument) in domain.arguments.iter().enumerate() {
                            let is_const = declared_parameters
                                .and_then(|parameters| parameters.get(index + 1))
                                .is_some_and(|parameter| {
                                    matches!(
                                        parameter.kind,
                                        psi_typed_trees::data::TypeParameterKind::Const { .. }
                                    )
                                });
                            validate_package_type_identity_input_inner(
                                program, *argument, binders, is_const,
                            )?;
                        }
                    }
                    TypeConstraintNode::Named(_) | TypeConstraintNode::ArithmeticDomain(_) => {}
                }
            }
            Ok(())
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            validate_package_type_identity_input_inner(program, *element_type, binders, false)?;
            match length {
                FixedArrayLength::Literal(_) => Ok(()),
                FixedArrayLength::ConstParameter { symbol, name } => {
                    validate_package_const_binder(program, *symbol, name.as_str(), binders)
                }
                FixedArrayLength::ConstCall { .. } => Err(vec![Diagnostic::error(
                    "package review rejects an unevaluated const call in structural type identity",
                )]),
            }
        }
        TypeReferenceNode::Slice { element_type } => {
            validate_package_type_identity_input_inner(program, *element_type, binders, false)
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            let parameters = program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == *base_symbol)
                .map(|definition| program.data_type_parameters(definition));
            for (index, argument) in program
                .type_reference_table
                .type_reference_handles(*arguments)
                .iter()
                .enumerate()
            {
                let is_const = parameters
                    .and_then(|parameters| parameters.get(index))
                    .is_some_and(|parameter| {
                        matches!(
                            parameter.kind,
                            psi_typed_trees::data::TypeParameterKind::Const { .. }
                        )
                    });
                validate_package_type_identity_input_inner(program, *argument, binders, is_const)?;
            }
            Ok(())
        }
        TypeReferenceNode::ConstExpression(expression) => {
            if !allow_const_value {
                return Err(vec![Diagnostic::error(
                    "package review rejects a const expression outside one exact declared const-parameter slot",
                )]);
            }
            validate_package_index_expression(program, *expression, binders)
        }
        TypeReferenceNode::Named { symbol, name } => validate_package_named_type_leaf(
            program,
            *symbol,
            name.as_str(),
            binders,
            allow_const_value,
        ),
        TypeReferenceNode::DynamicTrait { .. } | TypeReferenceNode::Unit => Ok(()),
    }
}

fn validate_package_named_type_leaf(
    program: &psi_typed_trees::TypedTrees,
    symbol: SymbolHandle,
    spelling: &str,
    binders: &[(SymbolHandle, String)],
    allow_const_value: bool,
) -> Result<(), Vec<Diagnostic>> {
    if symbol.is_valid() {
        if program.symbols.get(symbol).kind == psi_symbols::SymbolKind::Const {
            return Err(vec![Diagnostic::error(
                "package review rejects a residual const declaration in structural type identity",
            )]);
        }
        return Ok(());
    }
    if allow_const_value
        && (psi_language_semantics::const_value::CanonicalConstValue::from_atom(spelling).is_some()
            || spelling.parse::<i128>().is_ok())
    {
        return Ok(());
    }
    let mut matches = binders.iter().filter(|(candidate, _)| {
        candidate.is_valid() && program.symbols.name(*candidate) == spelling
    });
    if matches.next().is_some() && matches.next().is_none() {
        return Ok(());
    }
    Err(vec![Diagnostic::error(
        "package review rejects a source-spelled type or const leaf without exact semantic identity",
    )])
}

fn validate_package_const_binder(
    program: &psi_typed_trees::TypedTrees,
    symbol: SymbolHandle,
    spelling: &str,
    binders: &[(SymbolHandle, String)],
) -> Result<(), Vec<Diagnostic>> {
    if symbol.is_valid() && binders.iter().any(|(candidate, _)| *candidate == symbol) {
        return Ok(());
    }
    let mut matches = binders.iter().filter(|(candidate, _)| {
        !symbol.is_valid() && candidate.is_valid() && program.symbols.name(*candidate) == spelling
    });
    if matches.next().is_some() && matches.next().is_none() {
        return Ok(());
    }
    Err(vec![Diagnostic::error(
        "package review rejects a const binder without one exact telescope identity",
    )])
}

fn validate_package_index_expression(
    program: &psi_typed_trees::TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
    binders: &[(SymbolHandle, String)],
) -> Result<(), Vec<Diagnostic>> {
    use psi_typed_trees::expression::ExpressionNode;

    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            let spelling = members
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            if !path.symbol.is_valid()
                && (psi_language_semantics::const_value::CanonicalConstValue::from_atom(&spelling)
                    .is_some()
                    || spelling.parse::<i128>().is_ok())
            {
                return Ok(());
            }
            if members.len() == 1 {
                validate_package_const_binder(program, path.symbol, &spelling, binders)
            } else {
                Err(vec![Diagnostic::error(
                    "package review rejects an index name without one exact const-binder or compiler-const identity",
                )])
            }
        }
        ExpressionNode::Integer(_) => Ok(()),
        ExpressionNode::Unary(unary) => {
            validate_package_index_expression(program, unary.operand, binders)
        }
        ExpressionNode::Binary(binary) => {
            let mut selections = program
                .open_index_normalizations
                .iter()
                .flat_map(|normalization| &normalization.operations)
                .filter(|selection| selection.expression == expression);
            let Some(selection) = selections.next() else {
                return Err(vec![Diagnostic::error(
                    "package review rejects an open index operation without exact checked selection",
                )]);
            };
            if selections.next().is_some() {
                return Err(vec![Diagnostic::error(
                    "package review rejects an open index operation with duplicate checked selections",
                )]);
            }
            if !selection.operator.is_valid()
                || !selection.provider.is_valid()
                || !selection.algebra_trait.is_valid()
            {
                return Err(vec![Diagnostic::error(
                    "package review rejects an open index operation with incomplete semantic authority",
                )]);
            }
            validate_package_index_expression(program, binary.left, binders)?;
            validate_package_index_expression(program, binary.right, binders)
        }
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Atomic(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Indexed(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Borrow(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::String(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::ZeroValue(_) => Err(vec![Diagnostic::error(
            "package review rejects an unsupported structural index expression",
        )]),
    }
}

fn missing_exact_toolchain_type_owner() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "package review structural type identity has unresolved nominal ownership or is missing exact source-backed toolchain ownership",
    )]
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
    review_signature_type_identity_with_binders_and_substitutions(
        compilation,
        type_reference,
        binders,
        lifetime_binders,
        &[],
    )
}

fn review_signature_type_identity_with_binders_and_substitutions(
    compilation: &CheckedCompilation,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[psi_typed_trees::name::Identifier],
    substitutions: &[(SymbolHandle, psi_typed_trees::types::TypeReferenceHandle)],
) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
    review_signature_type_identity_with_binders_and_substitutions_and_lifetimes(
        compilation,
        type_reference,
        binders,
        lifetime_binders,
        substitutions,
        &[],
    )
}

fn review_signature_type_identity_with_binders_and_substitutions_and_lifetimes(
    compilation: &CheckedCompilation,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[psi_typed_trees::name::Identifier],
    substitutions: &[(SymbolHandle, psi_typed_trees::types::TypeReferenceHandle)],
    lifetime_substitutions: &[(
        psi_typed_trees::name::Identifier,
        psi_typed_trees::name::Identifier,
    )],
) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
    validate_package_type_identity_input(&compilation.typed, type_reference, binders)?;
    let runtime = compilation
        .package_qualified_type_identity_with_binders_substitutions_and_toolchain_sources(
            type_reference,
            binders,
            substitutions,
            compilation.exact_toolchain_sources(),
        )
        .ok_or_else(missing_exact_toolchain_type_owner)?
        .into_string();
    let lifetime = review_lifetime_topology_with_substitutions(
        compilation,
        type_reference,
        lifetime_binders,
        substitutions,
        lifetime_substitutions,
        &mut Vec::new(),
    )?;
    Ok(PackageReviewTypeIdentity {
        canonical: framed_identity("signature-type", &[runtime, lifetime]),
    })
}

fn review_domain_lifetime_label(
    compilation: &CheckedCompilation,
    domain: &psi_typed_trees::types::DomainConstraint,
) -> Result<String, Vec<Diagnostic>> {
    use psi_typed_trees::types::{DomainConstraintSubject, OmegaLayoutGrammar};

    match domain.subject {
        DomainConstraintSubject::Declared => {
            let identity = nominal_identity(compilation, domain.symbol)?;
            let owner = match identity.owner {
                PackageReviewNominalOwner::Package(package) => {
                    canonical_digest_label("package", package.digest())
                }
                PackageReviewNominalOwner::ToolchainSource(source) => {
                    canonical_digest_label("toolchain-source", source.digest())
                }
                PackageReviewNominalOwner::Unresolved => {
                    return Err(vec![Diagnostic::error(
                        "package review rejects a declared domain without exact nominal ownership",
                    )]);
                }
            };
            Ok(framed_identity("declared-domain", &[owner, identity.path]))
        }
        DomainConstraintSubject::Carry(permission) => Ok(framed_identity(
            "compiler-domain",
            &[
                "carry".to_owned(),
                match permission {
                    psi_language_semantics::CarryPermission::AcrossSuspend => "across-suspend",
                    psi_language_semantics::CarryPermission::AnyCpu => "any-cpu",
                    psi_language_semantics::CarryPermission::AnyThread => "any-thread",
                    psi_language_semantics::CarryPermission::MovableAddress => "movable-address",
                }
                .to_owned(),
            ],
        )),
        DomainConstraintSubject::Value(value_domain) => Ok(framed_identity(
            "compiler-domain",
            &[
                "value".to_owned(),
                match value_domain {
                    psi_language_semantics::value_domain::ValueDomain::Finite => "finite",
                }
                .to_owned(),
            ],
        )),
        DomainConstraintSubject::OmegaLayout { grammar } => Ok(framed_identity(
            "compiler-domain",
            &[
                "omega-layout".to_owned(),
                match grammar {
                    OmegaLayoutGrammar::Derived => "derived",
                }
                .to_owned(),
            ],
        )),
    }
}

fn canonical_digest_label(kind: &str, digest: [u8; 32]) -> String {
    use std::fmt::Write as _;

    let mut label = String::with_capacity(kind.len() + 1 + digest.len() * 2);
    label.push_str(kind);
    label.push(':');
    for byte in digest {
        let _ = write!(label, "{byte:02x}");
    }
    label
}

fn review_lifetime_topology_with_substitutions(
    compilation: &CheckedCompilation,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    lifetime_binders: &[psi_typed_trees::name::Identifier],
    substitutions: &[(SymbolHandle, psi_typed_trees::types::TypeReferenceHandle)],
    lifetime_substitutions: &[(
        psi_typed_trees::name::Identifier,
        psi_typed_trees::name::Identifier,
    )],
    active_substitutions: &mut Vec<SymbolHandle>,
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
                    substituted_lifetime_binder_ordinal(
                        lifetime,
                        lifetime_binders,
                        lifetime_substitutions,
                        "public type",
                    )?
                ),
                None => "elided".to_owned(),
            };
            framed_identity(
                "reference",
                &[
                    lifetime,
                    review_lifetime_topology_with_substitutions(
                        compilation,
                        *referee,
                        lifetime_binders,
                        substitutions,
                        lifetime_substitutions,
                        active_substitutions,
                    )?,
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
                    TypeConstraintNode::Domain(domain) if !domain.arguments.is_empty() => {
                        Some((|| {
                            let label = review_domain_lifetime_label(compilation, domain)?;
                            let arguments = domain
                                .arguments
                                .iter()
                                .map(|argument| {
                                    review_lifetime_topology_with_substitutions(
                                        compilation,
                                        *argument,
                                        lifetime_binders,
                                        substitutions,
                                        lifetime_substitutions,
                                        active_substitutions,
                                    )
                                })
                                .collect::<Result<Vec<_>, _>>()?;
                            Ok::<String, Vec<Diagnostic>>(framed_identity(&label, &arguments))
                        })())
                    }
                    _ => None,
                })
                .collect::<Result<Vec<_>, _>>()?;
            constraint_topologies.sort();
            constraint_topologies.dedup();
            let mut children = vec![review_lifetime_topology_with_substitutions(
                compilation,
                *base_type,
                lifetime_binders,
                substitutions,
                lifetime_substitutions,
                active_substitutions,
            )?];
            children.extend(constraint_topologies);
            framed_identity("constrained", &children)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => framed_identity(
            "array",
            &[review_lifetime_topology_with_substitutions(
                compilation,
                *element_type,
                lifetime_binders,
                substitutions,
                lifetime_substitutions,
                active_substitutions,
            )?],
        ),
        TypeReferenceNode::Slice { element_type } => framed_identity(
            "slice",
            &[review_lifetime_topology_with_substitutions(
                compilation,
                *element_type,
                lifetime_binders,
                substitutions,
                lifetime_substitutions,
                active_substitutions,
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
                    substituted_lifetime_binder_ordinal(
                        lifetime,
                        lifetime_binders,
                        lifetime_substitutions,
                        "public type",
                    )
                    .map(|ordinal| format!("binder:{ordinal}"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            children.extend(
                compilation
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .iter()
                    .map(|argument| {
                        review_lifetime_topology_with_substitutions(
                            compilation,
                            *argument,
                            lifetime_binders,
                            substitutions,
                            lifetime_substitutions,
                            active_substitutions,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            );
            framed_identity("generic", &children)
        }
        TypeReferenceNode::Named { symbol, .. } => {
            let Some((_, replacement)) = substitutions
                .iter()
                .rev()
                .find(|(parameter, _)| parameter == symbol)
            else {
                return Ok("named".to_owned());
            };
            if active_substitutions.contains(symbol) {
                return Err(vec![Diagnostic::error(
                    "package review rejects a cyclic inherited type substitution",
                )]);
            }
            active_substitutions.push(*symbol);
            let topology = review_lifetime_topology_with_substitutions(
                compilation,
                *replacement,
                lifetime_binders,
                substitutions,
                lifetime_substitutions,
                active_substitutions,
            );
            active_substitutions.pop();
            topology?
        }
        TypeReferenceNode::DynamicTrait { .. } => "dynamic-trait".to_owned(),
        TypeReferenceNode::ConstExpression(_) => "const-expression".to_owned(),
        TypeReferenceNode::Unit => "unit".to_owned(),
    };
    Ok(topology)
}

fn substituted_lifetime_binder_ordinal(
    lifetime: &psi_typed_trees::name::Identifier,
    lifetime_binders: &[psi_typed_trees::name::Identifier],
    substitutions: &[(
        psi_typed_trees::name::Identifier,
        psi_typed_trees::name::Identifier,
    )],
    context: &str,
) -> Result<u32, Vec<Diagnostic>> {
    let lifetime = substitutions
        .iter()
        .rev()
        .find_map(|(parameter, argument)| (parameter == lifetime).then_some(argument))
        .unwrap_or(lifetime);
    lifetime_binder_ordinal(lifetime, lifetime_binders, context)
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
        PackageReviewNominalOwner::ToolchainSource(_) => Ok(false),
        PackageReviewNominalOwner::Unresolved => Err(vec![Diagnostic::error(format!(
            "reviewed public declaration `{}` has no managed package owner",
            identity.path
        ))]),
    }
}

fn project_callable(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
    role: PackageReviewCallableRole,
    identity: PackageReviewNominalIdentity,
) -> Result<
    (
        CheckedPackageCallableReview,
        Vec<ProjectedReviewRow<PackageReviewExternalExecutableSupply>>,
    ),
    Vec<Diagnostic>,
> {
    let subject = identity.path.as_str();
    let Some(entry) = compilation.machine_states(machine).first() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{subject}` has no canonical entry signature"
        ))]);
    };
    let machine_type_parameters = compilation.machine_type_parameters(machine);
    let (binders, type_parameters) = project_type_parameters(
        compilation,
        machine_type_parameters,
        "callable",
        subject,
        &machine.lifetime_parameters,
    )?;
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
    let (conformances, operator_realizations, external_executable_supply) =
        project_callable_conformances(compilation, machine, &identity, &binders, true)?;
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
    let canonical_published =
        canonical_checked_invocation_targets(compilation, &checked_invocation.published_targets)?;
    let canonical_checked_inferred = canonical_checked_invocation_targets(
        compilation,
        &checked_invocation.checked_inferred_targets,
    )?;
    if checked_invocation.plan.published != canonical_published
        || checked_invocation.plan.checked_inferred != canonical_checked_inferred
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{subject}` has contradictory exact and rendered synchronous-invocation facts"
        ))]);
    }
    let suspension = compilation
        .facts
        .suspensions
        .for_machine(machine.symbol)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "reviewed callable `{subject}` has no exact suspension fact"
            ))]
        })?;
    let blocking = compilation
        .facts
        .blocking
        .for_machine(machine.symbol)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "reviewed callable `{subject}` has no exact blocking fact"
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
    if role != PackageReviewCallableRole::Build
        && matches!(
            suspension.interface,
            psi_language_semantics::SuspensionInterface::InternalInferred
        )
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{subject}` has no published suspension ceiling"
        ))]);
    }
    if role != PackageReviewCallableRole::Build
        && matches!(
            blocking.interface,
            psi_language_semantics::BlockingInterface::InternalInferred
        )
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{subject}` has no published blocking ceiling"
        ))]);
    }
    if suspension.checked_may_suspend != realized.checked_may_suspend
        || blocking.checked_may_block != realized.checked_may_block
        || checked_invocation.plan.checked_inferred != realized.effective_synchronous_invocations
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{subject}` operational facts do not equal its exact realized contract envelope"
        ))]);
    }
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
            project_synchronous_invocations(compilation, &checked_invocation.published_targets)?,
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
        project_synchronous_invocations(compilation, &checked_invocation.checked_inferred_targets)?;
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

    let supply = match machine.supply_mode {
        MachineSupplyMode::CheckedBody => PackageReviewCallableSupply::CheckedBody,
        MachineSupplyMode::Requirement => PackageReviewCallableSupply::Requirement,
        MachineSupplyMode::Boundary => PackageReviewCallableSupply::Boundary,
        MachineSupplyMode::Accepted => PackageReviewCallableSupply::Accepted,
        MachineSupplyMode::ExternalRealization { .. } => {
            PackageReviewCallableSupply::ExternalRealization
        }
    };

    Ok((
        CheckedPackageCallableReview {
            role,
            identity,
            supply,
            lifetime_parameter_count: machine.lifetime_parameters.len(),
            type_parameters,
            conformance_bounds,
            parameters,
            return_type,
            conformances,
            operator_realizations,
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
        },
        external_executable_supply,
    ))
}

fn project_private_external_executable_supply(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
    identity: &PackageReviewNominalIdentity,
) -> Result<Vec<ProjectedReviewRow<PackageReviewExternalExecutableSupply>>, Vec<Diagnostic>> {
    let machine_type_parameters = compilation.machine_type_parameters(machine);
    let (binders, _) = project_type_parameters(
        compilation,
        machine_type_parameters,
        "external executable supply",
        identity.path.as_str(),
        &machine.lifetime_parameters,
    )?;
    let (_, _, supply) =
        project_callable_conformances(compilation, machine, identity, &binders, false)?;
    Ok(supply)
}

struct ContractProjectionContext<'a> {
    subject_kind: &'static str,
    subject_name: &'a str,
    owner: psi_checked_trees::ContractProofFactOwner,
    point: psi_facts::ProgramPoint,
    parameters: &'a [psi_typed_trees::signature::StateParameter],
    domain_symbol: Option<SymbolHandle>,
    data_symbol: Option<SymbolHandle>,
    lifetime_binders: &'a [psi_typed_trees::name::Identifier],
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
        data_symbol: None,
        lifetime_binders: &machine.lifetime_parameters,
    };
    project_contracts(
        compilation,
        compilation.machine_contracts(machine),
        &context,
        binders,
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
    )
}

fn project_contracts(
    compilation: &CheckedCompilation,
    contracts: &[psi_typed_trees::signature::SignatureContract],
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
) -> Result<Vec<PackageReviewCallableContract>, Vec<Diagnostic>> {
    use psi_typed_trees::{domain::ProofFact, signature::SignatureContractKind};

    let reviewed_package = compilation.package_identity().ok_or_else(|| {
        vec![Diagnostic::error(
            "contract review requires package-aware checked compilation",
        )]
    })?;
    let mut projected = Vec::new();
    for contract in contracts {
        let (kind, guarded_symbols, result_case) = match contract.kind {
            SignatureContractKind::Requires => (PackageReviewContractKind::Requires, None, None),
            SignatureContractKind::Ensures => (PackageReviewContractKind::Ensures, None, None),
            SignatureContractKind::EnsuresForResultCase {
                result_data,
                result_case,
            } => (
                PackageReviewContractKind::Ensures,
                Some((result_data, result_case)),
                Some(PackageReviewResultCaseIdentity {
                    result_data: nominal_identity(compilation, result_data)?,
                    result_case: nominal_identity(compilation, result_case)?,
                }),
            ),
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
            let evidence_lane_position = if let Some((result_data, result_case)) = guarded_symbols {
                let checked = checked_outcome_specific_guarantee(
                    compilation,
                    context,
                    fact_handle,
                    result_data,
                    result_case,
                    contract.binding.as_ref(),
                )?;
                validate_checked_contract_evidence_components(
                    compilation,
                    context,
                    contract.binding.as_ref(),
                    psi_checked_trees::ContractProofFactOwner::Machine {
                        machine_symbol: checked.machine_symbol,
                    },
                    psi_checked_trees::ContractProofFactKind::Ensures,
                    checked.evidence_term,
                    &fact,
                )?
            } else {
                let checked = checked_contract_fact(compilation, context, fact_handle, kind)?;
                validate_checked_contract_evidence(
                    compilation,
                    context,
                    contract.binding.as_ref(),
                    checked,
                    &fact,
                )?
            };
            projected.push(PackageReviewCallableContract {
                kind,
                result_case: result_case.clone(),
                binding: match kind {
                    PackageReviewContractKind::Ensures => contract
                        .binding
                        .as_ref()
                        .map(|binding| binding.as_str().to_owned()),
                    PackageReviewContractKind::Requires => None,
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

fn project_contract_clause_source_locations(
    contracts: &[psi_typed_trees::signature::SignatureContract],
) -> Vec<ProjectedNestedSourceLocation> {
    contracts
        .iter()
        .filter_map(|contract| {
            contract
                .keyword_source_span
                .map(|source_span| ProjectedNestedSourceLocation {
                    source_span,
                    role: PackageReviewSourceLocationRole::ContractClause,
                })
        })
        .collect()
}

fn project_machine_service_reach_source_locations(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
) -> Result<Vec<ProjectedNestedSourceLocation>, Vec<Diagnostic>> {
    let authored = exact_authored_service_reach_row(
        compilation,
        machine.symbol,
        machine.name.as_str(),
        machine.service_reach_is_installation_bound,
    )?;
    let parameters = compilation
        .machine_states(machine)
        .first()
        .map(|state| compilation.state_parameters(state))
        .unwrap_or_default();
    let declared = derive_declared_service_reach(
        compilation,
        authored,
        &psi_effects::declared_machine_invocations(compilation, machine),
        parameters,
        machine.name.as_str(),
    )?;
    if compilation
        .service_reach_rows
        .services(machine.service_reach_row)
        != declared
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` authored reaches/invokes targets do not equal its exact normalized service-reach row",
            machine.name,
        ))]);
    }

    let checked = exactly_one(
        compilation
            .facts
            .service_reaches
            .machines()
            .iter()
            .filter(|fact| fact.machine == machine.symbol),
        machine.name.as_str(),
        "service-reach",
    )?;
    let should_publish = machine.supply_mode
        != psi_language_semantics::MachineSupplyMode::CheckedBody
        || machine.is_public
        || authored.is_some()
        || !declared.is_empty();
    let expected_interface = if should_publish {
        psi_language_semantics::ServiceReachInterface::PublishedCeiling(machine.service_reach_row)
    } else {
        psi_language_semantics::ServiceReachInterface::InternalInferred
    };
    if checked.interface != expected_interface
        || checked.published_ceiling != machine.service_reach_row
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` authored service-reach custody does not equal its exact checked service-reach fact",
            machine.name,
        ))]);
    }

    Ok(authored_service_reach_locations(authored))
}

fn project_signature_service_reach_source_locations(
    compilation: &CheckedCompilation,
    owner: SymbolHandle,
    signature: &psi_typed_trees::signature::StateSignature,
) -> Result<Vec<ProjectedNestedSourceLocation>, Vec<Diagnostic>> {
    let authored = exact_authored_service_reach_row(
        compilation,
        signature.symbol,
        signature.name.as_str(),
        signature.service_reach_is_installation_bound,
    )?;
    let declared = derive_declared_service_reach(
        compilation,
        authored,
        &psi_effects::declared_signature_invocations(compilation, signature),
        compilation.state_signature_parameters(signature),
        signature.name.as_str(),
    )?;
    if compilation
        .service_reach_rows
        .services(signature.service_reach_row)
        != declared
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed signature `{}` authored reaches/invokes targets do not equal its exact normalized service-reach row",
            signature.name,
        ))]);
    }

    let checked = exactly_one(
        compilation
            .facts
            .contract_plans
            .crash_capsules
            .iter()
            .filter(|capsule| {
                capsule.target_machine() == owner && capsule.target_state() == signature.symbol
            }),
        signature.name.as_str(),
        "signature contract capsule",
    )?;
    let mut checked_published = checked.published_service_reach().to_vec();
    checked_published.sort();
    checked_published.dedup();
    let mut declared_names = declared
        .iter()
        .map(|service| {
            compilation
                .service_reaches
                .definition(*service)
                .map(|definition| definition.name.clone())
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "reviewed signature `{}` has a normalized service outside its exact declaration table",
                        signature.name,
                    ))]
                })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    declared_names.sort();
    declared_names.dedup();
    if checked_published != declared_names {
        return Err(vec![Diagnostic::error(format!(
            "reviewed signature `{}` authored service-reach custody does not equal its exact checked contract capsule",
            signature.name,
        ))]);
    }

    Ok(authored_service_reach_locations(authored))
}

fn exact_authored_service_reach_row<'a>(
    compilation: &'a CheckedCompilation,
    owner: SymbolHandle,
    owner_name: &str,
    installation_bound: bool,
) -> Result<Option<&'a psi_typed_trees::signature::AuthoredServiceReachRow>, Vec<Diagnostic>> {
    let matching = compilation
        .authored_service_reach_rows_for(owner)
        .collect::<Vec<_>>();
    let authored = match matching.as_slice() {
        [] => None,
        [row] => Some(*row),
        _ => {
            return Err(vec![Diagnostic::error(format!(
                "reviewed callable `{owner_name}` has {} authored service-reach custody rows; expected at most one",
                matching.len(),
            ))]);
        }
    };
    if installation_bound != authored.is_some_and(|row| row.installation_bound) {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{owner_name}` has contradictory installation-bound service-reach custody",
        ))]);
    }
    if authored.is_some_and(|row| row.keyword_source_spans.is_empty()) {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{owner_name}` has authored service-reach custody without a `reaches` keyword occurrence",
        ))]);
    }
    if let Some(authored) = authored {
        for keyword_source_span in &authored.keyword_source_spans {
            let _ = canonical_source_span_location(
                compilation,
                *keyword_source_span,
                PackageReviewSourceLocationRole::ServiceReach,
            )?;
        }
    }
    Ok(authored)
}

fn derive_declared_service_reach(
    compilation: &CheckedCompilation,
    authored: Option<&psi_typed_trees::signature::AuthoredServiceReachRow>,
    invocations: &[psi_effects::InvocationTarget],
    parameters: &[psi_typed_trees::signature::StateParameter],
    owner_name: &str,
) -> Result<Vec<psi_language_semantics::ServiceReachId>, Vec<Diagnostic>> {
    let mut direct = authored
        .into_iter()
        .flat_map(|row| &row.targets)
        .map(|target| {
            compilation
                .service_reaches
                .id_for_symbol(target.service)
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "reviewed callable `{owner_name}` retains an authored service-reach target that is stale or not a boundary trait",
                    ))]
                })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    let non_self_parameters = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    for invocation in invocations {
        let symbol = match invocation {
            psi_effects::InvocationTarget::Parameter(ordinal) => non_self_parameters
                .get(*ordinal as usize)
                .map(|parameter| {
                    compilation
                        .type_reference_table
                        .type_reference(parameter.type_reference)
                        .type_symbol(&compilation.type_reference_table)
                })
                .unwrap_or_else(SymbolHandle::invalid),
            psi_effects::InvocationTarget::Service(symbol) => *symbol,
        };
        let service = compilation
            .service_reaches
            .id_for_symbol(symbol)
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "reviewed callable `{owner_name}` has an invocation target without an exact boundary-service identity",
                ))]
            })?;
        direct.push(service);
    }

    let mut closure = Vec::new();
    for service in direct {
        compilation
            .service_reaches
            .extend_closure(service, &mut closure);
    }
    closure.sort_by_key(|service| service.0);
    closure.dedup();
    Ok(closure)
}

fn authored_service_reach_locations(
    authored: Option<&psi_typed_trees::signature::AuthoredServiceReachRow>,
) -> Vec<ProjectedNestedSourceLocation> {
    let Some(authored) = authored else {
        return Vec::new();
    };
    if authored.targets.is_empty() {
        authored
            .keyword_source_spans
            .iter()
            .copied()
            .map(|source_span| ProjectedNestedSourceLocation {
                source_span,
                role: PackageReviewSourceLocationRole::ServiceReach,
            })
            .collect()
    } else {
        authored
            .targets
            .iter()
            .map(|target| ProjectedNestedSourceLocation {
                source_span: target.source_span,
                role: PackageReviewSourceLocationRole::ServiceReach,
            })
            .collect()
    }
}

fn project_machine_operational_source_locations(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
) -> Result<Vec<ProjectedNestedSourceLocation>, Vec<Diagnostic>> {
    let mut locations = project_operational_keyword_locations(
        compilation,
        machine.name.as_str(),
        "suspends",
        machine.suspends,
        &machine.suspends_keyword_source_spans,
        PackageReviewSourceLocationRole::Suspension,
    )?;
    locations.extend(project_operational_keyword_locations(
        compilation,
        machine.name.as_str(),
        "blocks",
        machine.blocks,
        &machine.blocks_keyword_source_spans,
        PackageReviewSourceLocationRole::Blocking,
    )?);

    let suspension = compilation
        .facts
        .suspensions
        .for_machine(machine.symbol)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "reviewed callable `{}` has no exact suspension fact",
                machine.name
            ))]
        })?;
    let blocking = compilation
        .facts
        .blocking
        .for_machine(machine.symbol)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "reviewed callable `{}` has no exact blocking fact",
                machine.name
            ))]
        })?;
    let publishes = machine.is_public
        || machine.supply_mode != psi_language_semantics::MachineSupplyMode::CheckedBody;
    let expected_suspension = if publishes || machine.suspends {
        psi_language_semantics::SuspensionInterface::PublishedMaySuspend(machine.suspends)
    } else {
        psi_language_semantics::SuspensionInterface::InternalInferred
    };
    let expected_blocking = if publishes || machine.blocks {
        psi_language_semantics::BlockingInterface::PublishedMayBlock(machine.blocks)
    } else {
        psi_language_semantics::BlockingInterface::InternalInferred
    };
    if suspension.interface != expected_suspension || blocking.interface != expected_blocking {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` authored operational custody does not equal its exact checked interfaces",
            machine.name
        ))]);
    }
    Ok(locations)
}

fn project_signature_operational_source_locations(
    compilation: &CheckedCompilation,
    owner: SymbolHandle,
    signature: &psi_typed_trees::signature::StateSignature,
) -> Result<Vec<ProjectedNestedSourceLocation>, Vec<Diagnostic>> {
    let mut locations = project_operational_keyword_locations(
        compilation,
        signature.name.as_str(),
        "suspends",
        signature.suspends,
        &signature.suspends_keyword_source_spans,
        PackageReviewSourceLocationRole::Suspension,
    )?;
    locations.extend(project_operational_keyword_locations(
        compilation,
        signature.name.as_str(),
        "blocks",
        signature.blocks,
        &signature.blocks_keyword_source_spans,
        PackageReviewSourceLocationRole::Blocking,
    )?);
    let checked = exactly_one(
        compilation
            .facts
            .contract_plans
            .crash_capsules
            .iter()
            .filter(|capsule| {
                capsule.target_machine() == owner && capsule.target_state() == signature.symbol
            }),
        signature.name.as_str(),
        "signature contract capsule",
    )?;
    if checked.published_may_suspend() != signature.suspends
        || checked.published_may_block() != signature.blocks
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed signature `{}` authored operational custody does not equal its exact checked contract capsule",
            signature.name
        ))]);
    }
    Ok(locations)
}

fn project_operational_keyword_locations(
    compilation: &CheckedCompilation,
    owner_name: &str,
    clause: &str,
    authored: bool,
    source_spans: &[psi_source::SourceSpan],
    role: PackageReviewSourceLocationRole,
) -> Result<Vec<ProjectedNestedSourceLocation>, Vec<Diagnostic>> {
    if authored != !source_spans.is_empty() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{owner_name}` has contradictory authored `{clause}` source custody"
        ))]);
    }
    source_spans
        .iter()
        .copied()
        .map(|source_span| {
            canonical_source_span_location(compilation, source_span, role)?;
            Ok(ProjectedNestedSourceLocation { source_span, role })
        })
        .collect()
}

fn project_machine_invocation_source_locations(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
) -> Result<Vec<ProjectedNestedSourceLocation>, Vec<Diagnostic>> {
    let declarations = compilation.machine_invokes(machine);
    let declared = psi_effects::declared_machine_invocations(compilation, machine);
    if declared.len() != declarations.len() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` has an unresolved, duplicate, or semantically aliased authored invokes target",
            machine.name,
        ))]);
    }
    let checked = exactly_one(
        compilation
            .facts
            .synchronous_invocations
            .machines
            .iter()
            .filter(|fact| fact.machine == machine.symbol),
        machine.name.as_str(),
        "synchronous-invocation",
    )?;
    if checked.published_targets != declared {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` authored invokes targets do not equal its exact checked published ceiling",
            machine.name,
        ))]);
    }
    let checked_published = canonical_checked_invocation_targets(compilation, &declared)?;
    let checked_inferred =
        canonical_checked_invocation_targets(compilation, &checked.checked_inferred_targets)?;
    if checked.plan.published != checked_published
        || checked.plan.checked_inferred != checked_inferred
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` has contradictory exact and rendered synchronous-invocation facts",
            machine.name,
        ))]);
    }

    Ok(declarations
        .iter()
        .map(|declaration| ProjectedNestedSourceLocation {
            source_span: declaration.source_span,
            role: PackageReviewSourceLocationRole::SynchronousInvocation,
        })
        .collect())
}

fn project_signature_invocation_source_locations(
    compilation: &CheckedCompilation,
    signature: &psi_typed_trees::signature::StateSignature,
) -> Result<Vec<ProjectedNestedSourceLocation>, Vec<Diagnostic>> {
    let declarations = compilation.state_signature_invokes(signature);
    let targets = psi_effects::declared_signature_invocations(compilation, signature);
    if targets.len() != declarations.len() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed signature `{}` has an unresolved, duplicate, or semantically aliased authored invokes target",
            signature.name,
        ))]);
    }

    Ok(declarations
        .iter()
        .map(|declaration| ProjectedNestedSourceLocation {
            source_span: declaration.source_span,
            role: PackageReviewSourceLocationRole::SynchronousInvocation,
        })
        .collect())
}

fn canonical_checked_invocation_targets(
    compilation: &CheckedCompilation,
    targets: &[psi_effects::InvocationTarget],
) -> Result<Vec<String>, Vec<Diagnostic>> {
    let mut canonical = targets
        .iter()
        .map(|target| match target {
            psi_effects::InvocationTarget::Parameter(index) => Ok(format!("parameter:{index}")),
            psi_effects::InvocationTarget::Service(symbol) => {
                let matching = compilation
                    .traits()
                    .iter()
                    .filter(|definition| definition.symbol == *symbol)
                    .collect::<Vec<_>>();
                let [definition] = matching.as_slice() else {
                    return Err(vec![Diagnostic::error(format!(
                        "reviewed synchronous invocation resolves service symbol {} to {} declarations; expected exactly one",
                        symbol.arena_index(),
                        matching.len(),
                    ))]);
                };
                if !definition.is_boundary {
                    return Err(vec![Diagnostic::error(format!(
                        "reviewed synchronous invocation resolves `{}` to a non-boundary trait",
                        definition.name,
                    ))]);
                }
                Ok(format!("service:{}", definition.name))
            }
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    canonical.sort();
    canonical.dedup();
    Ok(canonical)
}

fn collect_type_parameter_source_locations(
    compilation: &CheckedCompilation,
    parameters: &[psi_typed_trees::data::TypeParameter],
    locations: &mut Vec<ProjectedNestedSourceLocation>,
) -> Result<(), Vec<Diagnostic>> {
    for parameter in parameters {
        let psi_typed_trees::data::TypeParameterKind::Machine {
            contract: psi_typed_trees::data::MachineParameterContract::Structural(signature),
        } = &parameter.kind
        else {
            continue;
        };
        locations.extend(project_contract_clause_source_locations(
            compilation.state_signature_contracts(signature),
        ));
        locations.extend(project_signature_invocation_source_locations(
            compilation,
            signature,
        )?);
        locations.extend(project_signature_service_reach_source_locations(
            compilation,
            parameter.symbol,
            signature,
        )?);
        locations.extend(project_signature_operational_source_locations(
            compilation,
            parameter.symbol,
            signature,
        )?);
        collect_type_parameter_source_locations(
            compilation,
            compilation.state_signature_type_parameters(signature),
            locations,
        )?;
    }
    Ok(())
}

fn checked_outcome_specific_guarantee<'a>(
    compilation: &'a CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    fact: psi_arena::Handle<psi_typed_trees::domain::ProofFact>,
    result_data: SymbolHandle,
    result_case: SymbolHandle,
    binding: Option<&psi_typed_trees::name::Identifier>,
) -> Result<&'a psi_checked_trees::OutcomeSpecificGuaranteeFact, Vec<Diagnostic>> {
    let psi_checked_trees::ContractProofFactOwner::Machine { machine_symbol } = context.owner
    else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` publishes an outcome-specific guarantee without a checked machine owner",
            context.subject_kind, context.subject_name
        ))]);
    };
    let public_selector = binding.map(|binding| binding.as_str());
    let matching = compilation
        .facts
        .proof
        .outcome_specific_guarantees
        .iter()
        .filter_map(|(_, checked)| {
            (checked.machine_symbol == machine_symbol
                && checked.fact == fact
                && checked.result_data == result_data
                && checked.result_case == result_case
                && checked.public_selector.as_deref() == public_selector)
                .then_some(checked)
        })
        .collect::<Vec<_>>();
    let [checked] = matching.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` outcome-specific guarantee has {} exact checked carrier rows; expected one",
            context.subject_kind,
            context.subject_name,
            matching.len()
        ))]);
    };
    Ok(*checked)
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
    validate_checked_contract_evidence_components(
        compilation,
        context,
        binding,
        checked.owner,
        checked.kind,
        checked.evidence_term,
        projected,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_checked_contract_evidence_components(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    binding: Option<&psi_typed_trees::name::Identifier>,
    checked_owner: psi_checked_trees::ContractProofFactOwner,
    checked_kind: psi_checked_trees::ContractProofFactKind,
    checked_evidence_term: Option<psi_arena::Handle<psi_checked_trees::CheckedEvidenceTerm>>,
    projected: &PackageReviewContractFact,
) -> Result<Option<u32>, Vec<Diagnostic>> {
    let Some(binding) = binding else {
        if checked_evidence_term.is_some() {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` has an unnamed contract with a checked evidence term",
                context.subject_kind, context.subject_name
            ))]);
        }
        return Ok(None);
    };
    let Some(term_handle) = checked_evidence_term else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` named contract `{}` has no checked evidence term",
            context.subject_kind, context.subject_name, binding
        ))]);
    };
    let term = compilation.facts.proof.evidence_terms.get(term_handle);
    if term.name != binding.as_str() || term.owner != checked_owner || term.kind != checked_kind {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` named contract `{}` does not match its checked evidence term",
            context.subject_kind, context.subject_name, binding
        ))]);
    }
    if matches!(
        projected,
        PackageReviewContractFact::PropositionParameter(_)
    ) {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` named contract `{}` uses a generic proposition endpoint without an exact checked witness interface",
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
            let owner = compilation
                .traits()
                .iter()
                .find(|candidate| candidate.symbol == requirement.declaring_trait)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "checked witness requirement has no exact declaring trait",
                    )]
                })?;
            let signature = compilation
                .trait_machine_signatures(owner)
                .iter()
                .find(|candidate| candidate.symbol == requirement.requirement)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "checked witness requirement has no exact overload declaration",
                    )]
                })?;
            Ok((
                nominal_identity(compilation, requirement.declaring_trait)?,
                trait_requirement_identity(compilation, owner, signature)?,
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
    if application.proposition.is_valid()
        && compilation.typed.symbols.get(application.proposition).kind
            == psi_symbols::SymbolKind::PropositionParameter
    {
        if !application.binder_arguments.is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` generic proposition endpoint has unexpected static arguments",
                context.subject_kind, context.subject_name
            ))]);
        }
        let matching_parameters = compilation
            .typed
            .data_type_parameters
            .iter()
            .map(|(_, parameter)| parameter)
            .filter(|parameter| parameter.symbol == application.proposition)
            .collect::<Vec<_>>();
        let [parameter] = matching_parameters.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` generic proposition endpoint rejoins {} declaration parameters; expected exactly one",
                context.subject_kind,
                context.subject_name,
                matching_parameters.len()
            ))]);
        };
        let psi_typed_trees::data::TypeParameterKind::Proposition { contract } = &parameter.kind
        else {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` generic proposition endpoint does not rejoin a proposition-family signature",
                context.subject_kind, context.subject_name
            ))]);
        };
        let argument_handles = compilation
            .expression_table
            .expression_handles(application.arguments);
        let parameter_count = compilation
            .typed
            .state_parameters
            .span_or_empty(contract.parameters)
            .len();
        if argument_handles.len() != parameter_count {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` generic proposition endpoint has inconsistent checked arity",
                context.subject_kind, context.subject_name
            ))]);
        }
        let mut static_ordinal = 0usize;
        let mut matching_ordinals = Vec::new();
        for (symbol, _) in callable_binders {
            if matches!(
                compilation.typed.symbols.get(*symbol).kind,
                psi_symbols::SymbolKind::TypeParameter
                    | psi_symbols::SymbolKind::MachineParameter
                    | psi_symbols::SymbolKind::PropositionParameter
            ) {
                if *symbol == application.proposition {
                    matching_ordinals.push(static_ordinal);
                }
                static_ordinal += 1;
            }
        }
        let [binder_ordinal] = matching_ordinals.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` generic proposition endpoint rejoins {} callable static binders; expected exactly one",
                context.subject_kind,
                context.subject_name,
                matching_ordinals.len()
            ))]);
        };
        let arguments = argument_handles
            .iter()
            .map(|argument| {
                project_contract_expression_with_substitutions(
                    compilation,
                    context,
                    callable_binders,
                    *argument,
                    value_substitutions,
                    &[],
                    checked_fact,
                    0,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(PackageReviewContractFact::PropositionParameter(
            PackageReviewPropositionParameterApplication {
                binder_ordinal: portable_parameter_position(*binder_ordinal)?,
                arguments,
            },
        ));
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
                &[],
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
                    let projection_substitutions = declaration_parameters
                        .iter()
                        .zip(
                            compilation
                                .expression_table
                                .expression_handles(application.arguments),
                        )
                        .map(|(parameter, argument)| (parameter.symbol, *argument))
                        .collect::<Vec<_>>();
                    project_contract_expression_with_substitutions(
                        compilation,
                        context,
                        callable_binders,
                        *expression,
                        &nested_values,
                        &projection_substitutions,
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
        match argument.kind {
            psi_typed_trees::proposition::PropositionBinderArgumentKind::Type => {
                let identity = compilation
                    .package_qualified_nominal_type_identity_with_toolchain_sources(
                        argument.symbol,
                        compilation.exact_toolchain_sources(),
                    )
                    .ok_or_else(missing_exact_toolchain_type_owner)?;
                PackageReviewPropositionBinderValue::Type(PackageReviewTypeIdentity {
                    canonical: identity.into_string(),
                })
            }
            psi_typed_trees::proposition::PropositionBinderArgumentKind::Machine => {
                PackageReviewPropositionBinderValue::Machine(nominal_identity(
                    compilation,
                    argument.symbol,
                )?)
            }
            psi_typed_trees::proposition::PropositionBinderArgumentKind::Const => {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` proposition contains a non-literal const binder argument without an exact caller binder",
                    context.subject_kind, context.subject_name
                ))]);
            }
        }
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
    let requirement = trait_requirement_identity_from_symbols(
        compilation,
        checked_requirement.declaring_trait,
        checked_requirement.requirement,
        "checked evidence projection",
    )?;
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
        PackageReviewPropositionBinderValue::Machine(identity) => {
            Some(PackageReviewContractExpression::Nominal(identity.clone()))
        }
        PackageReviewPropositionBinderValue::Type(_) => None,
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
    use psi_typed_trees::proposition::PropositionBody;

    let (binders, parameter_types) = project_proposition_signature(compilation, declaration)?;
    let binder_symbols = compilation
        .proposition_binders(declaration)
        .iter()
        .enumerate()
        .map(|(position, binder)| (binder.symbol, format!("proposition-binder:{position}")))
        .collect::<Vec<_>>();
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

fn project_proposition_signature(
    compilation: &CheckedCompilation,
    declaration: &psi_typed_trees::proposition::PropositionDefinition,
) -> Result<
    (
        Vec<PackageReviewPropositionBinder>,
        Vec<PackageReviewTypeIdentity>,
    ),
    Vec<Diagnostic>,
> {
    use psi_typed_trees::proposition::PropositionBinderKind;

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
                            )?,
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
        .collect::<Result<Vec<_>, _>>()?;
    Ok((binders, parameter_types))
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
        .collect::<Result<Vec<_>, _>>()?;
    let mut requirements = Vec::new();
    collect_evidence_requirements(
        compilation,
        trait_symbol,
        &arguments,
        proposition_binders,
        None,
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
    lifetime_binders: Option<&[psi_typed_trees::name::Identifier]>,
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
            &parameter.kind,
            psi_typed_trees::data::TypeParameterKind::Type
                | psi_typed_trees::data::TypeParameterKind::Const { .. }
                | psi_typed_trees::data::TypeParameterKind::Machine {
                    contract: psi_typed_trees::data::MachineParameterContract::RequirementIdentity
                }
        )
    }) {
        return Err(vec![Diagnostic::error(format!(
            "reviewed evidence trait `{}` uses a structural/nominal machine or proposition parameter not represented by package review",
            definition.name
        ))]);
    }
    let argument_identities = trait_arguments
        .iter()
        .map(|argument| match lifetime_binders {
            Some(lifetime_binders) => {
                review_signature_type_identity_with_binders_and_substitutions(
                    compilation,
                    *argument,
                    proposition_binders,
                    lifetime_binders,
                    inherited_substitutions,
                )
            }
            None => review_type_identity_with_binders_and_substitutions(
                compilation,
                *argument,
                proposition_binders,
                inherited_substitutions,
            ),
        })
        .collect::<Result<Vec<_>, _>>()?;
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
            requirement: trait_requirement_identity(compilation, definition, requirement)?,
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
            lifetime_binders,
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
        &[],
        checked_fact,
        depth,
    )
}

fn exact_fact_call_projection<'compilation>(
    compilation: &'compilation CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    projection_expression: psi_typed_trees::expression::ExpressionHandle,
    call_expression: psi_typed_trees::expression::ExpressionHandle,
    member: &psi_typed_trees::expression::TableMemberExpression,
) -> Result<&'compilation psi_checked_trees::CheckedFactCallProjection, Vec<Diagnostic>> {
    use psi_typed_trees::expression::ExpressionNode;

    let ExpressionNode::Call(call) = compilation.expression_table.expression(call_expression)
    else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` fact-call projection does not rejoin a call expression",
            context.subject_kind, context.subject_name
        ))]);
    };
    let matching = compilation
        .facts
        .fact_call_projections
        .iter()
        .filter(|projection| {
            projection.projection_expression == projection_expression
                && projection.call_expression == call_expression
                && projection.target_state == call.target_symbol
                && projection.machine_arguments == call.machine_arguments
        })
        .collect::<Vec<_>>();
    let [projection] = matching.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` fact-call projection rejoins {} exact eligibility certificates; expected one",
            context.subject_kind,
            context.subject_name,
            matching.len()
        ))]);
    };
    let target = compilation
        .machines()
        .iter()
        .find(|machine| machine.symbol == projection.target_machine)
        .and_then(|machine| {
            compilation
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == projection.target_state)
        });
    if target.is_none_or(|state| state.return_type != projection.result_type) {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` fact-call projection certificate no longer rejoins its exact result type",
            context.subject_kind, context.subject_name
        ))]);
    }
    if member.case_variant.is_some() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` fact-call projection is not a direct record field",
            context.subject_kind, context.subject_name
        ))]);
    }
    let data_symbol = match compilation
        .type_reference_table
        .type_reference(projection.result_type)
    {
        psi_typed_trees::types::TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
        psi_typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. } => {
            Some(*base_symbol)
        }
        _ => None,
    };
    let field_rejoins = data_symbol
        .and_then(|symbol| {
            compilation
                .data_definitions()
                .iter()
                .find(|data| data.symbol == symbol)
        })
        .and_then(|data| {
            compilation.data_members(data).iter().find_map(|candidate| {
                let psi_typed_trees::data::DataMember::Field(field) = candidate else {
                    return None;
                };
                (field.name.as_str() == member.member.as_str()).then_some(field.symbol)
            })
        })
        .is_some_and(|field| field == projection.field);
    if !field_rejoins {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` fact-call projection certificate no longer rejoins its exact field",
            context.subject_kind, context.subject_name
        ))]);
    }
    Ok(projection)
}

fn contract_parameter_field_symbol(
    compilation: &CheckedCompilation,
    parameter: &psi_typed_trees::signature::StateParameter,
    field_name: &str,
) -> Option<SymbolHandle> {
    use psi_typed_trees::types::TypeReferenceNode;

    let mut type_reference = parameter.type_reference;
    let data_symbol = loop {
        match compilation
            .type_reference_table
            .type_reference(type_reference)
        {
            TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Named { symbol, .. } => break *symbol,
            TypeReferenceNode::Generic { base_symbol, .. } => break *base_symbol,
            _ => return None,
        }
    };
    compilation
        .data_definitions()
        .iter()
        .find(|data| data.symbol == data_symbol)
        .and_then(|data| {
            compilation.data_members(data).iter().find_map(|member| {
                let psi_typed_trees::data::DataMember::Field(field) = member else {
                    return None;
                };
                (field.name.as_str() == field_name).then_some(field.symbol)
            })
        })
}

fn project_contract_expression_with_substitutions(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
    expression: psi_typed_trees::expression::ExpressionHandle,
    substitutions: &[(SymbolHandle, PackageReviewContractExpression)],
    projection_substitutions: &[(SymbolHandle, psi_typed_trees::expression::ExpressionHandle)],
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
            projection_substitutions,
            checked_fact,
            depth + 1,
        )
    };
    match compilation.expression_table.expression(expression) {
        ExpressionNode::Boolean(value) => Ok(PackageReviewContractExpression::Boolean(*value)),
        ExpressionNode::Integer(value) => Ok(PackageReviewContractExpression::Integer(
            value.text().to_owned(),
        )),
        ExpressionNode::ArrayLiteral(values) => Ok(PackageReviewContractExpression::Array(
            compilation
                .expression_table
                .expression_handles(*values)
                .iter()
                .map(|value| child(*value))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        ExpressionNode::StructLiteral(literal) => {
            project_contract_constructor_expression(compilation, context, literal, &child)
        }
        ExpressionNode::Indexed(indexed) => Ok(PackageReviewContractExpression::Indexed {
            meaning: exact_checked_contract_operator_meaning(compilation, context, expression)?,
            collection: Box::new(child(indexed.collection)?),
            index: Box::new(child(indexed.index)?),
        }),
        ExpressionNode::Range(range) => Ok(PackageReviewContractExpression::Range {
            start: range
                .start
                .is_valid()
                .then(|| child(range.start))
                .transpose()?
                .map(Box::new),
            end: range
                .end
                .is_valid()
                .then(|| child(range.end))
                .transpose()?
                .map(Box::new),
            end_inclusive: range.end_inclusive,
        }),
        ExpressionNode::String(value) => Ok(PackageReviewContractExpression::ByteSequence(
            value.to_vec(),
        )),
        ExpressionNode::ZeroValue(type_reference) => {
            Ok(PackageReviewContractExpression::ZeroValue(
                review_signature_type_identity_with_binders(
                    compilation,
                    *type_reference,
                    binders,
                    context.lifetime_binders,
                )?,
            ))
        }
        ExpressionNode::Binary(binary) => Ok(PackageReviewContractExpression::Binary {
            meaning: exact_checked_contract_operator_meaning(compilation, context, expression)?,
            operator: project_contract_binary_operator(binary.operator),
            left: Box::new(child(binary.left)?),
            right: Box::new(child(binary.right)?),
        }),
        ExpressionNode::Unary(unary) => Ok(PackageReviewContractExpression::Unary {
            operator: project_contract_unary_operator(unary.operator),
            operand: Box::new(child(unary.operand)?),
        }),
        ExpressionNode::Call(call) => {
            let target =
                exact_checked_contract_call_target(compilation, context, expression, call)?;
            let static_parameter_kinds = match &target {
                PackageReviewContractCallTarget::Nominal(_) => {
                    contract_call_static_parameter_kinds(
                        compilation,
                        context,
                        call.target_symbol,
                        call.machine_arguments.len(),
                    )?
                }
                PackageReviewContractCallTarget::ByteSequencePredicate(_) => {
                    if !call.machine_arguments.is_empty() {
                        return Err(vec![Diagnostic::error(format!(
                            "reviewed {} `{}` supplies static arguments to a compiler-owned byte-sequence predicate",
                            context.subject_kind, context.subject_name
                        ))]);
                    }
                    Vec::new()
                }
            };
            if call.quotient_operation.is_some() {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` uses a quotient contract call not yet represented by package review",
                    context.subject_kind, context.subject_name
                ))]);
            }
            if !call.evidence_arguments.is_empty() {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` uses a contract call with evidence arguments not yet represented by package review",
                    context.subject_kind, context.subject_name
                ))]);
            }
            // Call-site suspend/block acknowledgement is diagnostic audit
            // metadata, explicitly outside contract identity. Fact-position
            // calls have already been checked as total and pure.
            Ok(PackageReviewContractExpression::Call {
                receiver: call
                    .receiver
                    .is_valid()
                    .then(|| child(call.receiver))
                    .transpose()?
                    .map(Box::new),
                target,
                static_arguments: call
                    .machine_arguments
                    .iter()
                    .zip(static_parameter_kinds)
                    .map(|(argument, parameter_kind)| {
                        project_contract_static_argument(
                            compilation,
                            context,
                            binders,
                            argument,
                            parameter_kind,
                            0,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                arguments: compilation
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .map(|argument| child(*argument))
                    .collect::<Result<Vec<_>, _>>()?,
            })
        }
        ExpressionNode::Name(path) => project_contract_name_expression(
            compilation,
            context,
            binders,
            expression,
            path,
            substitutions,
            checked_fact,
        ),
        ExpressionNode::Member(member)
            if matches!(
                compilation.expression_table.expression(member.receiver),
                ExpressionNode::Name(path)
                    if projection_substitutions
                        .iter()
                        .any(|(symbol, actual)| {
                            *symbol == path.symbol
                                && matches!(
                                    compilation.expression_table.expression(*actual),
                                    ExpressionNode::Call(_)
                                )
                        })
            ) =>
        {
            let ExpressionNode::Name(path) =
                compilation.expression_table.expression(member.receiver)
            else {
                unreachable!()
            };
            let actual = projection_substitutions
                .iter()
                .find(|(symbol, _)| *symbol == path.symbol)
                .map(|(_, actual)| *actual)
                .expect("guarded projection substitution");
            let projection =
                exact_fact_call_projection(compilation, context, expression, actual, member)?;
            project_contract_member_expression(
                compilation,
                context,
                child(actual)?,
                projection.field,
                None,
            )
        }
        ExpressionNode::Member(member)
            if matches!(
                compilation.expression_table.expression(member.receiver),
                ExpressionNode::Name(path)
                    if substitutions.iter().any(|(symbol, _)| *symbol == path.symbol)
            ) =>
        {
            project_contract_member_expression(
                compilation,
                context,
                child(member.receiver)?,
                member.member_symbol,
                None,
            )
        }
        ExpressionNode::Member(member)
            if checked_fact.is_none()
                && matches!(
                    compilation.expression_table.expression(member.receiver),
                    ExpressionNode::Name(path)
                        if context.parameters.iter().any(|parameter| {
                            parameter.symbol == path.symbol && member.case_variant.is_none()
                        })
                ) =>
        {
            let ExpressionNode::Name(path) =
                compilation.expression_table.expression(member.receiver)
            else {
                unreachable!()
            };
            let parameter = context
                .parameters
                .iter()
                .find(|parameter| parameter.symbol == path.symbol)
                .expect("guarded proposition parameter member");
            let field = contract_parameter_field_symbol(
                compilation,
                parameter,
                member.member.as_str(),
            )
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "reviewed {} `{}` proposition parameter member does not resolve through its declared carrier",
                    context.subject_kind, context.subject_name
                ))]
            })?;
            project_contract_member_expression(
                compilation,
                context,
                child(member.receiver)?,
                field,
                None,
            )
        }
        ExpressionNode::Member(member)
            if matches!(
                compilation.expression_table.expression(member.receiver),
                ExpressionNode::Call(_)
            ) =>
        {
            let projection = exact_fact_call_projection(
                compilation,
                context,
                expression,
                member.receiver,
                member,
            )?;
            project_contract_member_expression(
                compilation,
                context,
                child(member.receiver)?,
                projection.field,
                None,
            )
        }
        ExpressionNode::Member(_) => {
            let Some(checked_fact) = checked_fact else {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` uses a proposition-argument member expression without an exact checked place join",
                    context.subject_kind, context.subject_name
                ))]);
            };
            let Some((root_expression, mut source_members)) =
                contract_member_path_source(compilation, expression)
            else {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` uses a computed member expression not yet represented by package review",
                    context.subject_kind, context.subject_name
                ))]);
            };
            let data_subject_root = context.data_symbol.is_some_and(|data_symbol| {
                is_data_subject_field_expression(compilation, data_symbol, root_expression)
            });
            let root = contract_member_path_root(compilation, context, root_expression)
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "reviewed {} `{}` contract member path has no exact semantic root",
                        context.subject_kind, context.subject_name
                    ))]
                })?;
            let receiver = if data_subject_root {
                let psi_typed_trees::expression::ExpressionNode::Name(path) =
                    compilation.expression_table.expression(root_expression)
                else {
                    unreachable!("guarded data-subject name root")
                };
                let [field_name] = compilation.expression_table.name_path_members(path.members)
                else {
                    unreachable!("guarded single data-subject field")
                };
                source_members.insert(0, field_name.clone());
                PackageReviewContractExpression::DomainSubject
            } else {
                child(root_expression)?
            };
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
                target: review_type_identity_with_binders(compilation, cast.target_type, binders)?,
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
                    .collect::<Result<Vec<_>, _>>()?,
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

fn project_contract_constructor_expression(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    literal: &psi_typed_trees::expression::TableStructLiteral,
    child: &impl Fn(
        psi_typed_trees::expression::ExpressionHandle,
    ) -> Result<PackageReviewContractExpression, Vec<Diagnostic>>,
) -> Result<PackageReviewContractExpression, Vec<Diagnostic>> {
    use psi_typed_trees::data::DataMember;

    let matching_data = compilation
        .data_definitions()
        .iter()
        .filter(|data| data.symbol == literal.type_symbol)
        .collect::<Vec<_>>();
    let [data] = matching_data.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` constructor resolves its data symbol to {} declarations; expected one",
            context.subject_kind,
            context.subject_name,
            matching_data.len()
        ))]);
    };
    let data_identity = nominal_identity(compilation, data.symbol)?;
    let reviewed_package = compilation.package_identity().ok_or_else(|| {
        vec![Diagnostic::error(
            "package review requires package-aware checked compilation",
        )]
    })?;
    if reviewed_package_owns(&data_identity, reviewed_package)? && !data.is_public {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` exposes non-public data `{}` through a constructor",
            context.subject_kind, context.subject_name, data.name
        ))]);
    }

    let selected_variant = match literal.case_symbol {
        Some(case_symbol) => {
            let matching = compilation
                .data_members(data)
                .iter()
                .filter_map(|member| match member {
                    DataMember::Variant(variant) if variant.symbol == case_symbol => Some(variant),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let [variant] = matching.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` constructor case resolves to {} variants in `{}`; expected one",
                    context.subject_kind,
                    context.subject_name,
                    matching.len(),
                    data.name
                ))]);
            };
            Some(*variant)
        }
        None => None,
    };
    if literal.case_name.is_some() != selected_variant.is_some() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` constructor has inconsistent checked case identity",
            context.subject_kind, context.subject_name
        ))]);
    }

    let mut allowed_fields = compilation
        .data_members(data)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) => Some(field.symbol),
            DataMember::Variant(_) => None,
        })
        .collect::<Vec<_>>();
    if let Some(variant) = selected_variant {
        allowed_fields.extend(
            compilation
                .data_payload_fields(variant)
                .iter()
                .map(|field| field.symbol),
        );
    }

    let mut fields = compilation
        .expression_table
        .struct_fields(literal.fields)
        .iter()
        .map(|field| {
            if !field.field_symbol.is_valid()
                || !allowed_fields.contains(&field.field_symbol)
            {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed {} `{}` constructor field `{}` does not rejoin its selected data/case",
                    context.subject_kind, context.subject_name, field.name
                ))]);
            }
            Ok(PackageReviewConstructorField {
                field: nominal_identity(compilation, field.field_symbol)?,
                value: child(field.value)?,
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    fields.sort();
    if fields.windows(2).any(|pair| pair[0].field == pair[1].field) {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` constructor repeats one exact field",
            context.subject_kind, context.subject_name
        ))]);
    }

    Ok(PackageReviewContractExpression::Constructor {
        data: data_identity,
        case: selected_variant
            .map(|variant| nominal_identity(compilation, variant.symbol))
            .transpose()?,
        fields,
    })
}

fn exact_checked_contract_operator_meaning(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Result<PackageReviewContractOperatorMeaning, Vec<Diagnostic>> {
    use psi_language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionIntrinsic,
        AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget,
    };

    let selections = compilation
        .expression_table
        .authored_selection_occurrences(expression)
        .filter_map(|occurrence| {
            compilation
                .authored_declaration_selections()
                .get(occurrence)
        })
        .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::Operator)
        .collect::<Vec<_>>();
    let [selection] = selections.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract operator has {} exact checked selection rows; expected one",
            context.subject_kind,
            context.subject_name,
            selections.len()
        ))]);
    };
    if selection.exposure() != AuthoredDeclarationSelectionExposure::PublicInterface {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract operator is not retained as a public-interface selection",
            context.subject_kind, context.subject_name
        ))]);
    }
    match selection.target() {
        AuthoredDeclarationSelectionTarget::Intrinsic(
            AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
        ) => Ok(PackageReviewContractOperatorMeaning::Builtin),
        AuthoredDeclarationSelectionTarget::Resolved(target) => {
            let symbol = target.selected_symbol();
            let declaration = psi_typed_trees::operator::declaration_by_symbol(compilation, symbol)
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "reviewed {} `{}` contract selected an operator without one retained declaration",
                        context.subject_kind, context.subject_name
                    ))]
                })?;
            Ok(PackageReviewContractOperatorMeaning::Declared(
                project_operator_coordinate(compilation, declaration)?,
            ))
        }
        AuthoredDeclarationSelectionTarget::Intrinsic(_) => Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract operator selected a non-operator intrinsic",
            context.subject_kind, context.subject_name
        ))]),
        AuthoredDeclarationSelectionTarget::LateBound(_) => Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract operator remains late-bound after checked lowering",
            context.subject_kind, context.subject_name
        ))]),
    }
}

fn exact_checked_contract_call_target(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    expression: psi_typed_trees::expression::ExpressionHandle,
    call: &psi_typed_trees::expression::TableCallExpression,
) -> Result<PackageReviewContractCallTarget, Vec<Diagnostic>> {
    use psi_language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
        AuthoredDeclarationSelectionTarget,
    };

    let selections = compilation
        .expression_table
        .authored_selection_occurrences(expression)
        .filter_map(|occurrence| {
            compilation
                .authored_declaration_selections()
                .get(occurrence)
        })
        .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::Call)
        .collect::<Vec<_>>();
    let [selection] = selections.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract call has {} exact checked call-selection rows; expected one",
            context.subject_kind,
            context.subject_name,
            selections.len()
        ))]);
    };
    if selection.exposure() != AuthoredDeclarationSelectionExposure::PublicInterface {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract call is not retained as a public-interface selection",
            context.subject_kind, context.subject_name
        ))]);
    }
    match selection.target() {
        AuthoredDeclarationSelectionTarget::Resolved(target)
            if target.selected_symbol() == call.target_symbol =>
        {
            Ok(PackageReviewContractCallTarget::Nominal(nominal_identity(
                compilation,
                call.target_symbol,
            )?))
        }
        AuthoredDeclarationSelectionTarget::Resolved(_) => Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract call target disagrees with its exact checked call-selection row",
            context.subject_kind, context.subject_name
        ))]),
        AuthoredDeclarationSelectionTarget::Intrinsic(
            psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic::ByteSequencePredicate(
                predicate,
            ),
        ) if !call.target_symbol.is_valid()
            && !call.receiver.is_valid()
            && psi_language_semantics::byte_predicates::ByteSequencePredicate::from_name(
                call.target.as_str(),
            ) == Some(predicate) => Ok(PackageReviewContractCallTarget::ByteSequencePredicate(
                match predicate {
                    psi_language_semantics::byte_predicates::ByteSequencePredicate::ValidUtf8 => {
                        PackageReviewByteSequencePredicate::ValidUtf8
                    }
                    psi_language_semantics::byte_predicates::ByteSequencePredicate::NoNul => {
                        PackageReviewByteSequencePredicate::NoNul
                    }
                    psi_language_semantics::byte_predicates::ByteSequencePredicate::AsciiOnly => {
                        PackageReviewByteSequencePredicate::AsciiOnly
                    }
                    psi_language_semantics::byte_predicates::ByteSequencePredicate::NonEmpty => {
                        PackageReviewByteSequencePredicate::NonEmpty
                    }
                },
            )),
        AuthoredDeclarationSelectionTarget::Intrinsic(_) => Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract-call intrinsic identity disagrees with its exact checked call-selection row or is not yet represented by package review",
            context.subject_kind, context.subject_name
        ))]),
        AuthoredDeclarationSelectionTarget::LateBound(_) => Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` retains an unresolved contract call selection",
            context.subject_kind, context.subject_name
        ))]),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContractCallStaticParameterKind {
    Type,
    Const,
    Machine,
    Proposition,
}

fn contract_call_static_parameter_kind(
    parameter: &psi_typed_trees::data::TypeParameter,
) -> ContractCallStaticParameterKind {
    match parameter.kind {
        psi_typed_trees::data::TypeParameterKind::Type => ContractCallStaticParameterKind::Type,
        psi_typed_trees::data::TypeParameterKind::Const { .. } => {
            ContractCallStaticParameterKind::Const
        }
        psi_typed_trees::data::TypeParameterKind::Machine { .. } => {
            ContractCallStaticParameterKind::Machine
        }
        psi_typed_trees::data::TypeParameterKind::Proposition { .. } => {
            ContractCallStaticParameterKind::Proposition
        }
    }
}

fn contract_call_static_parameter_kinds(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    target: SymbolHandle,
    supplied_count: usize,
) -> Result<Vec<ContractCallStaticParameterKind>, Vec<Diagnostic>> {
    let project = |parameters: &[psi_typed_trees::data::TypeParameter]| {
        parameters
            .iter()
            .map(contract_call_static_parameter_kind)
            .collect::<Vec<_>>()
    };
    let mut candidates = compilation
        .machines()
        .iter()
        .filter(|machine| {
            compilation
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == target)
        })
        .map(|machine| project(compilation.machine_type_parameters(machine)))
        .collect::<Vec<_>>();
    if let Some((_, signature)) = compilation.machine_parameter_signature(target) {
        candidates.push(project(
            compilation.state_signature_type_parameters(signature),
        ));
    }
    candidates.extend(compilation.traits().iter().flat_map(|definition| {
        compilation
            .trait_machine_signatures(definition)
            .iter()
            .filter(|signature| signature.symbol == target)
            .map(|signature| project(compilation.state_signature_type_parameters(signature)))
    }));
    let [parameter_kinds] = candidates.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract call target rejoins {} static telescopes; expected exactly one",
            context.subject_kind,
            context.subject_name,
            candidates.len()
        ))]);
    };
    if parameter_kinds.len() != supplied_count {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract call supplies {supplied_count} static arguments for a checked telescope of {} parameters",
            context.subject_kind,
            context.subject_name,
            parameter_kinds.len()
        ))]);
    }
    Ok(parameter_kinds.clone())
}

fn project_contract_static_argument(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
    argument: &psi_typed_trees::expression::StaticMachineArgument,
    parameter_kind: ContractCallStaticParameterKind,
    depth: usize,
) -> Result<PackageReviewContractStaticArgument, Vec<Diagnostic>> {
    project_static_argument(
        compilation,
        context.subject_kind,
        context.subject_name,
        binders,
        context.lifetime_binders,
        argument,
        parameter_kind,
        depth,
    )
}

fn project_static_argument(
    compilation: &CheckedCompilation,
    subject_kind: &str,
    subject_name: &str,
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[psi_typed_trees::name::Identifier],
    argument: &psi_typed_trees::expression::StaticMachineArgument,
    parameter_kind: ContractCallStaticParameterKind,
    depth: usize,
) -> Result<PackageReviewContractStaticArgument, Vec<Diagnostic>> {
    let rejected = |reason: &str| {
        vec![Diagnostic::error(format!(
            "reviewed {subject_kind} `{subject_name}` uses a static argument {reason}",
        ))]
    };
    if depth >= 64 {
        return Err(rejected(
            "whose nested application exceeds the package-review depth limit",
        ));
    }
    if argument.evidence_projection.is_some() {
        return Err(rejected(
            "from an evidence projection not yet represented by package review",
        ));
    }
    if parameter_kind == ContractCallStaticParameterKind::Proposition {
        return Err(rejected(
            "for a proposition parameter not yet represented by package review",
        ));
    }
    if let Some(application) = argument.application.as_ref() {
        if parameter_kind != ContractCallStaticParameterKind::Type
            || !argument.symbol.is_valid()
            || compilation.typed.symbols.get(argument.symbol).kind != psi_symbols::SymbolKind::Data
        {
            return Err(rejected(
                "with a non-data nested static application not yet represented by package review",
            ));
        }
        let definitions = compilation
            .data_definitions()
            .iter()
            .filter(|definition| definition.symbol == argument.symbol)
            .collect::<Vec<_>>();
        let [definition] = definitions.as_slice() else {
            return Err(rejected(
                "whose generic data base does not rejoin exactly one checked declaration",
            ));
        };
        if definition.lifetime_parameters.len() != application.lifetime_arguments.len() {
            return Err(rejected(
                "whose lifetime argument count differs from its checked data declaration",
            ));
        }
        let parameters = compilation.data_type_parameters(definition);
        if parameters.len() != application.arguments.len() {
            return Err(rejected(
                "whose generic data argument count differs from its checked telescope",
            ));
        }
        let base = compilation
            .package_qualified_nominal_type_identity_with_toolchain_sources(
                argument.symbol,
                compilation.exact_toolchain_sources(),
            )
            .ok_or_else(missing_exact_toolchain_type_owner)?;
        let arguments = application
            .arguments
            .iter()
            .zip(parameters)
            .map(|(argument, parameter)| {
                project_static_argument(
                    compilation,
                    subject_kind,
                    subject_name,
                    binders,
                    lifetime_binders,
                    argument,
                    contract_call_static_parameter_kind(parameter),
                    depth + 1,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let lifetime_arguments = application
            .lifetime_arguments
            .iter()
            .map(|lifetime| {
                lifetime_binder_ordinal(lifetime, lifetime_binders, "contract-call nested type")
            })
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(PackageReviewContractStaticArgument::GenericType {
            base: PackageReviewTypeIdentity {
                canonical: base.into_string(),
            },
            lifetime_arguments,
            arguments,
        });
    }
    if let Some(literal) = argument.const_literal.as_ref() {
        if parameter_kind != ContractCallStaticParameterKind::Const {
            return Err(rejected(
                "whose category differs from its checked telescope slot",
            ));
        }
        return Ok(PackageReviewContractStaticArgument::ConstInteger(
            literal.text().to_owned(),
        ));
    }
    if let Some(position) = binders
        .iter()
        .position(|(symbol, _)| *symbol == argument.symbol)
    {
        let position = portable_parameter_position(position)?;
        return match compilation.typed.symbols.get(argument.symbol).kind {
            psi_symbols::SymbolKind::MachineParameter
                if parameter_kind == ContractCallStaticParameterKind::Machine =>
            {
                Ok(PackageReviewContractStaticArgument::GenericMachineBinder(
                    position,
                ))
            }
            psi_symbols::SymbolKind::TypeParameter => {
                let matching = compilation
                    .typed
                    .data_type_parameters
                    .iter()
                    .map(|(_, parameter)| parameter)
                    .filter(|parameter| parameter.symbol == argument.symbol)
                    .collect::<Vec<_>>();
                let [parameter] = matching.as_slice() else {
                    return Err(rejected(
                        "that does not rejoin exactly one checked caller parameter",
                    ));
                };
                match (&parameter.kind, parameter_kind) {
                    (
                        psi_typed_trees::data::TypeParameterKind::Type,
                        ContractCallStaticParameterKind::Type,
                    ) => Ok(PackageReviewContractStaticArgument::GenericTypeBinder(
                        position,
                    )),
                    (
                        psi_typed_trees::data::TypeParameterKind::Const { .. },
                        ContractCallStaticParameterKind::Const,
                    ) => Ok(PackageReviewContractStaticArgument::GenericConstBinder(
                        position,
                    )),
                    _ => Err(rejected(
                        "whose category differs from its checked caller and callee telescope slots",
                    )),
                }
            }
            _ => Err(rejected(
                "whose category differs from its checked caller and callee telescope slots",
            )),
        };
    }
    if parameter_kind == ContractCallStaticParameterKind::Type {
        if !argument.symbol.is_valid()
            || !matches!(
                compilation.typed.symbols.get(argument.symbol).kind,
                psi_symbols::SymbolKind::BuiltinType | psi_symbols::SymbolKind::Data
            )
        {
            return Err(rejected(
                "whose category differs from its checked type slot",
            ));
        }
        let identity = compilation
            .package_qualified_nominal_type_identity_with_toolchain_sources(
                argument.symbol,
                compilation.exact_toolchain_sources(),
            )
            .ok_or_else(missing_exact_toolchain_type_owner)?;
        return Ok(PackageReviewContractStaticArgument::Type(
            PackageReviewTypeIdentity {
                canonical: identity.into_string(),
            },
        ));
    }
    if parameter_kind == ContractCallStaticParameterKind::Const {
        return Err(rejected(
            "from a forwarded or symbolic const not yet represented by package review",
        ));
    }
    if !argument.symbol.is_valid()
        || compilation.typed.symbols.get(argument.symbol).kind != psi_symbols::SymbolKind::State
    {
        return Err(rejected(
            "whose category differs from its checked machine slot",
        ));
    }
    let matching_states = compilation
        .machines()
        .iter()
        .filter_map(|machine| compilation.machine_states(machine).first())
        .filter(|entry| entry.symbol == argument.symbol)
        .count();
    if matching_states != 1 {
        return Err(rejected(
            "that does not rejoin exactly one checked concrete machine entry",
        ));
    }
    Ok(PackageReviewContractStaticArgument::ConcreteMachine(
        nominal_identity(compilation, argument.symbol)?,
    ))
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
    let data_binder_position = context.data_symbol.and_then(|data_symbol| {
        data_subject_binder_position(compilation, data_symbol, expression, binders)
    });
    if data_binder_position.is_none()
        && context.data_symbol.is_some_and(|data_symbol| {
            is_data_subject_field_expression(compilation, data_symbol, expression)
        })
    {
        let Some(checked_fact) = checked_fact else {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` uses a data-invariant field without an exact checked place join",
                context.subject_kind, context.subject_name
            ))]);
        };
        return checked_contract_member_path(
            compilation,
            context,
            checked_fact,
            expression,
            psi_facts::PlaceRoot::Symbol(context.data_symbol.expect("guarded data subject")),
            members,
        )?
        .into_iter()
        .try_fold(
            PackageReviewContractExpression::DomainSubject,
            |receiver, (case_variant, member_symbol)| {
                project_contract_member_expression(
                    compilation,
                    context,
                    receiver,
                    member_symbol,
                    case_variant,
                )
            },
        );
    }
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
        if root_symbol.is_valid() {
            parameter.symbol == root_symbol
        } else {
            root_name.is_some_and(|name| name == &parameter.name)
        }
    });
    let is_domain_subject =
        context.domain_symbol.is_some() && root_name.is_some_and(|name| name.as_str() == "self");
    let binder_position = binders
        .iter()
        .position(|(symbol, _)| *symbol == root_symbol)
        .or(data_binder_position);
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
        if root_symbol.is_valid()
            && root_name.is_some_and(|name| {
                context
                    .parameters
                    .iter()
                    .any(|parameter| name == &parameter.name)
            })
        {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` contract parameter spelling does not match its exact resolved symbol",
                context.subject_kind, context.subject_name
            ))]);
        }
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
    if context.data_symbol.is_some_and(|data_symbol| {
        is_data_subject_field_expression(compilation, data_symbol, expression)
    }) {
        return context.data_symbol.map(psi_facts::PlaceRoot::Symbol);
    }
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

fn is_data_subject_field_expression(
    compilation: &CheckedCompilation,
    data_symbol: SymbolHandle,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> bool {
    let psi_typed_trees::expression::ExpressionNode::Name(path) =
        compilation.expression_table.expression(expression)
    else {
        return false;
    };
    let [name] = compilation.expression_table.name_path_members(path.members) else {
        return false;
    };
    let [member_symbol] = compilation
        .expression_table
        .name_path_member_symbols(path.member_symbols)
    else {
        return false;
    };
    if !path.head_symbol.is_valid()
        || path.symbol != path.head_symbol
        || *member_symbol != path.head_symbol
    {
        return false;
    }
    let selected = path.head_symbol;
    let Some(data) = compilation
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == data_symbol)
    else {
        return false;
    };
    compilation.data_members(data).iter().any(|member| {
        let psi_typed_trees::data::DataMember::Field(field) = member else {
            return false;
        };
        field.symbol == selected && field.name == *name
    })
}

fn data_subject_binder_position(
    compilation: &CheckedCompilation,
    data_symbol: SymbolHandle,
    expression: psi_typed_trees::expression::ExpressionHandle,
    binders: &[(SymbolHandle, String)],
) -> Option<usize> {
    let psi_typed_trees::expression::ExpressionNode::Name(path) =
        compilation.expression_table.expression(expression)
    else {
        return None;
    };
    let [name] = compilation.expression_table.name_path_members(path.members) else {
        return None;
    };
    let [member_symbol] = compilation
        .expression_table
        .name_path_member_symbols(path.member_symbols)
    else {
        return None;
    };
    if !path.head_symbol.is_valid()
        || path.symbol != path.head_symbol
        || *member_symbol != path.head_symbol
    {
        return None;
    }
    let selected = path.head_symbol;
    let data = compilation
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == data_symbol)?;
    let parameter = compilation
        .data_type_parameters(data)
        .iter()
        .find(|parameter| parameter.symbol == selected && parameter.name == *name)?;
    binders
        .iter()
        .position(|(symbol, _)| *symbol == parameter.symbol)
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

    if let Some(data_symbol) = context.data_symbol {
        let mut candidates = Vec::new();
        for (_, record) in compilation
            .facts
            .semantic
            .data_definition_facts
            .iter()
            .filter(|(_, record)| record.data_symbol == data_symbol && record.fact == checked_fact)
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
    callable_identity: &PackageReviewNominalIdentity,
    binders: &[(SymbolHandle, String)],
    require_public_trait: bool,
) -> Result<
    (
        Vec<PackageReviewCallableConformance>,
        Vec<PackageReviewOperatorCoordinate>,
        Vec<ProjectedReviewRow<PackageReviewExternalExecutableSupply>>,
    ),
    Vec<Diagnostic>,
> {
    let expected_external = match machine.supply_mode {
        MachineSupplyMode::ExternalRealization { binding, mechanism } => {
            if machine.body_is_present {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed external callable `{}` retains an implementation body",
                    machine.name
                ))]);
            }
            let conformances = compilation.machine_trait_conformances(machine);
            if conformances.len() != 1 {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed external callable `{}` has {} conformance applications; expected exactly one",
                    machine.name,
                    conformances.len()
                ))]);
            }
            let Some(identity) = compilation.external_bindings.identity(binding) else {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed external callable `{}` has no exact binding-table identity",
                    machine.name
                ))]);
            };
            if identity.mechanism() != mechanism {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed external callable `{}` has a supply mechanism inconsistent with its exact binding identity",
                    machine.name
                ))]);
            }
            validate_external_binding_payload(compilation, machine, identity)?;
            Some((binding, project_external_binding(identity)))
        }
        MachineSupplyMode::CheckedBody
        | MachineSupplyMode::Requirement
        | MachineSupplyMode::Boundary
        | MachineSupplyMode::Accepted => None,
    };
    let mut projected = Vec::new();
    let mut operator_realizations = Vec::new();
    let mut external_executable_supply = Vec::new();
    for conformance in compilation.machine_trait_conformances(machine) {
        match (
            conformance.external_binding,
            conformance.external_binding_source_span,
        ) {
            (None, None) | (Some(_), Some(_)) => {}
            (None, Some(_)) => {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` retains authored `via` custody without an external binding",
                    machine.name
                ))]);
            }
            (Some(_), None) => {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed external callable `{}` has no exact authored `via` custody",
                    machine.name
                ))]);
            }
        }
        match (expected_external.as_ref(), conformance.external_binding) {
            (None, None) => {}
            (None, Some(_)) => {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` retains an external conformance binding without external supply",
                    machine.name
                ))]);
            }
            (Some(_), None) => {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed external callable `{}` has a conformance without its exact external binding",
                    machine.name
                ))]);
            }
            (Some((expected, _)), Some(actual)) if *expected != actual => {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed external callable `{}` has a conformance binding inconsistent with its supply mode",
                    machine.name
                ))]);
            }
            (Some(_), Some(_)) => {}
        }
        let trait_definition = compilation
            .traits()
            .iter()
            .find(|definition| definition.symbol == conformance.symbol);
        let Some(trait_definition) = trait_definition else {
            let Some(requirement_name) = conformance.requirement.as_ref() else {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` has an unresolved realization without an exact requirement",
                    machine.name
                ))]);
            };
            if !compilation
                .type_reference_table
                .type_reference_handles(conformance.arguments)
                .is_empty()
            {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` supplies type arguments to operator realization `{}::{}`",
                    machine.name, conformance.name, requirement_name
                ))]);
            }
            let external_operator =
                expected_external.is_some() || conformance.external_binding.is_some();
            let operator = if external_operator {
                psi_typed_trees::operator::resolve_satisfied_boundary_operator(
                    &compilation.typed,
                    machine,
                    conformance.name.as_str(),
                    requirement_name.as_str(),
                )
            } else {
                psi_typed_trees::operator::resolve_satisfied_checked_operator(
                    &compilation.typed,
                    machine,
                    conformance.name.as_str(),
                    requirement_name.as_str(),
                )
            };
            let Some(operator) = operator else {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` realization `{}::{}` resolves to neither one exact trait requirement nor one exact {}operator",
                    machine.name,
                    conformance.name,
                    requirement_name,
                    if external_operator {
                        "boundary "
                    } else {
                        "checked "
                    }
                ))]);
            };
            if !operator.is_public && (external_operator || require_public_trait) {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` realizes non-public operator `{}::{}` whose complete contract is absent from package review",
                    machine.name, conformance.name, requirement_name
                ))]);
            }
            if external_operator {
                if operator.spelling.is_some() {
                    return Err(vec![Diagnostic::error(format!(
                        "reviewed callable `{}` realizes fixed-token boundary operator `{}::{}` before external token dispatch is represented",
                        machine.name, conformance.name, requirement_name
                    ))]);
                }
                if !operator.lifetime_parameters.is_empty()
                    || !compilation.operator_type_parameters(operator).is_empty()
                    || !machine.lifetime_parameters.is_empty()
                    || !machine.type_parameters.is_empty()
                {
                    return Err(vec![Diagnostic::error(format!(
                        "reviewed callable `{}` realizes generic or lifetime-parameterized boundary operator `{}::{}` through external supply not yet represented by package review",
                        machine.name, conformance.name, requirement_name
                    ))]);
                }
                if conformance.alias.is_some() {
                    return Err(vec![Diagnostic::error(format!(
                        "reviewed callable `{}` realizes boundary operator `{}::{}` through an alias not yet represented by package review",
                        machine.name, conformance.name, requirement_name
                    ))]);
                }
                let Some((_, binding)) = expected_external.as_ref() else {
                    return Err(vec![Diagnostic::error(format!(
                        "reviewed callable `{}` realizes boundary operator `{}::{}` through an external binding without exact external supply",
                        machine.name, conformance.name, requirement_name
                    ))]);
                };
                validate_selected_boundary_operator_external_supply(
                    compilation,
                    machine,
                    operator,
                    binding,
                )?;
                let coordinate = project_operator_coordinate(compilation, operator)?;
                if require_public_trait {
                    operator_realizations.push(coordinate.clone());
                }
                external_executable_supply.push(project_external_executable_supply_with_source(
                    machine,
                    conformance,
                    PackageReviewExternalExecutableSupply {
                        callable: callable_identity.clone(),
                        requirement: PackageReviewExternalRequirement::Operator(coordinate),
                        binding: binding.clone(),
                    },
                )?);
                continue;
            }
            if !matches!(machine.supply_mode, MachineSupplyMode::CheckedBody)
                || !machine.body_is_present
            {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` realizes operator `{}::{}` without one checked implementation body",
                    machine.name, conformance.name, requirement_name
                ))]);
            }
            if operator.is_boundary && operator.spelling.is_some() {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` realizes fixed-token boundary operator `{}::{}` before checked-adapter token dispatch is represented",
                    machine.name, conformance.name, requirement_name
                ))]);
            }
            if operator.is_boundary {
                validate_selected_boundary_operator_checked_adapter(
                    compilation,
                    machine,
                    operator,
                )?;
            }
            if !operator.lifetime_parameters.is_empty()
                || !compilation.operator_type_parameters(operator).is_empty()
            {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` realizes generic or lifetime-parameterized operator `{}::{}` not yet represented by package review",
                    machine.name, conformance.name, requirement_name
                ))]);
            }
            if conformance.alias.is_some() {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` realizes operator `{}::{}` through an alias not yet represented by package review",
                    machine.name, conformance.name, requirement_name
                ))]);
            }
            if compilation.operator_contracts(operator).iter().any(|contract| {
                matches!(
                    contract.kind,
                    psi_typed_trees::signature::SignatureContractKind::EnsuresForResultCase {
                        ..
                    } | psi_typed_trees::signature::SignatureContractKind::Crashes { .. }
                )
            }) {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` realizes operator `{}::{}` with outcome-specific or crash contracts outside checked operator refinement",
                    machine.name, conformance.name, requirement_name
                ))]);
            }
            let Some(provider_envelope) = compilation
                .facts
                .contract_plans
                .realized_envelope(machine.symbol)
            else {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed checked operator provider `{}` has no retained realized contract envelope",
                    machine.name
                ))]);
            };
            if !provider_envelope.checked_crash.published().is_empty()
                || !provider_envelope.checked_crash.checked_sites().is_empty()
                || !provider_envelope.checked_crash.checked_calls().is_empty()
            {
                return Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{}` realizes operator `{}::{}` with nonempty checked crash behavior outside checked operator refinement",
                    machine.name, conformance.name, requirement_name
                ))]);
            }
            psi_validation::validate_checked_operator_realization_contract(
                &compilation.typed,
                machine,
                operator,
            )?;
            operator_realizations.push(project_operator_coordinate(compilation, operator)?);
            continue;
        };
        if require_public_trait && !trait_definition.is_public {
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
        let row = PackageReviewCallableConformance {
            trait_identity: nominal_identity(compilation, trait_definition.symbol)?,
            requirement_identity: trait_requirement_identity(
                compilation,
                trait_definition,
                requirement,
            )?,
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
        };
        if let Some((_, binding)) = expected_external.as_ref() {
            external_executable_supply.push(project_external_executable_supply_with_source(
                machine,
                conformance,
                PackageReviewExternalExecutableSupply {
                    callable: callable_identity.clone(),
                    requirement: PackageReviewExternalRequirement::Trait(row.clone()),
                    binding: binding.clone(),
                },
            )?);
        }
        projected.push(row);
    }
    if expected_external.is_some() && external_executable_supply.is_empty() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed external callable `{}` has no exact conformance application",
            machine.name
        ))]);
    }
    projected.sort();
    if projected.windows(2).any(|rows| rows[0] == rows[1]) {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` contains a duplicate exact trait realization",
            machine.name
        ))]);
    }
    operator_realizations.sort();
    if operator_realizations
        .windows(2)
        .any(|rows| rows[0] == rows[1])
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` contains a duplicate exact operator realization",
            machine.name
        ))]);
    }
    external_executable_supply.sort_by(|left, right| left.row.cmp(&right.row));
    if external_executable_supply
        .windows(2)
        .any(|rows| rows[0].row == rows[1].row)
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed external callable `{}` contains duplicate executable-supply identity",
            machine.name
        ))]);
    }
    Ok((projected, operator_realizations, external_executable_supply))
}

fn project_external_executable_supply_with_source(
    machine: &psi_typed_trees::machine::Machine,
    conformance: &psi_typed_trees::machine::TraitConformance,
    row: PackageReviewExternalExecutableSupply,
) -> Result<ProjectedReviewRow<PackageReviewExternalExecutableSupply>, Vec<Diagnostic>> {
    let Some(source_span) = conformance.external_binding_source_span else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed external callable `{}` has no exact authored `via` custody",
            machine.name
        ))]);
    };
    Ok(ProjectedReviewRow {
        row,
        declaration: machine.symbol,
        nested_source_locations: vec![ProjectedNestedSourceLocation {
            source_span,
            role: PackageReviewSourceLocationRole::ExternalBinding,
        }],
    })
}

fn validate_selected_boundary_operator_checked_adapter(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
    operator: &psi_typed_trees::operator::OperatorDefinition,
) -> Result<(), Vec<Diagnostic>> {
    let plans = compilation.selected_provider_plans().plans();
    let provenance = compilation.selected_provider_provenance();
    if plans.len() != provenance.len() {
        return Err(vec![Diagnostic::error(
            "selected boundary-operator provider plans are not aligned with retained declaration provenance",
        )]);
    }
    let slot = psi_typed_trees::operator::boundary_operator_requirement_identity(
        &compilation.typed,
        operator,
    );
    let matches = plans
        .iter()
        .zip(provenance)
        .filter(|(plan, retained)| {
            plan.schema.trait_name == slot
                && retained.provider.schema
                    == super::provider_plans::ProviderSchemaDeclaration::BoundaryOperator(
                        operator.symbol,
                    )
        })
        .collect::<Vec<_>>();
    let [(plan, retained)] = matches.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed checked adapter `{}` realizes boundary operator `{slot}`, but package review found {} exact selected provider plans for that operator",
            machine.name,
            matches.len(),
        ))]);
    };
    let [method] = plan.schema.methods.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` must contain exactly one schema method",
            plan.name,
        ))]);
    };
    let [row] = plan.rows.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` must contain exactly one realization row",
            plan.name,
        ))]);
    };
    let [requirement_symbol] = retained.provider.row_requirements.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` must retain exactly one requirement declaration",
            plan.name,
        ))]);
    };
    let [realization_symbol] = retained.provider.row_realizations.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` must retain exactly one realization declaration",
            plan.name,
        ))]);
    };
    let expected_machine_identity = compilation
        .normalized_machine_overload_identity(machine)
        .map(|identity| identity.identity())
        .unwrap_or_default();
    let expected_package = compilation
        .typed
        .symbols
        .symbol_package_identity(machine.symbol);
    if retained.plan != **plan
        || *requirement_symbol != operator.symbol
        || *realization_symbol != machine.symbol
        || method.requirement_owner != slot
        || method.requirement_identity != slot
        || row.requirement_identity != slot
        || !matches!(
            &row.binding,
            omega_effects::provider_plan::ProviderBinding::CheckedAdapter {
                machine_identity,
                machine_package_identity,
            } if machine_identity == &expected_machine_identity
                && *machine_package_identity == expected_package
        )
    {
        return Err(vec![Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` does not join exact operator `{slot}` to checked adapter `{}`",
            plan.name, machine.name,
        ))]);
    }
    Ok(())
}

fn validate_selected_boundary_operator_external_supply(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
    operator: &psi_typed_trees::operator::OperatorDefinition,
    binding: &PackageReviewExternalBinding,
) -> Result<(), Vec<Diagnostic>> {
    let plans = compilation.selected_provider_plans().plans();
    let provenance = compilation.selected_provider_provenance();
    if plans.len() != provenance.len() {
        return Err(vec![Diagnostic::error(
            "selected boundary-operator provider plans are not aligned with retained declaration provenance",
        )]);
    }
    let slot = psi_typed_trees::operator::boundary_operator_requirement_identity(
        &compilation.typed,
        operator,
    );
    let matches = plans
        .iter()
        .zip(provenance)
        .filter(|(plan, retained)| {
            plan.schema.trait_name == slot
                && retained.provider.schema
                    == super::provider_plans::ProviderSchemaDeclaration::BoundaryOperator(
                        operator.symbol,
                    )
                && retained.provider.row_realizations.contains(&machine.symbol)
        })
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return Ok(());
    }
    let [(plan, retained)] = matches.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed external leaf `{}` realizes boundary operator `{slot}`, but package review found {} selected provider plans for that exact candidate",
            machine.name,
            matches.len(),
        ))]);
    };
    let [method] = plan.schema.methods.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` must contain exactly one schema method",
            plan.name,
        ))]);
    };
    let [row] = plan.rows.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` must contain exactly one realization row",
            plan.name,
        ))]);
    };
    let [requirement_symbol] = retained.provider.row_requirements.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` must retain exactly one requirement declaration",
            plan.name,
        ))]);
    };
    let [realization_symbol] = retained.provider.row_realizations.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` must retain exactly one realization declaration",
            plan.name,
        ))]);
    };
    let expected_machine_identity = compilation
        .normalized_machine_overload_identity(machine)
        .map(|identity| identity.identity())
        .unwrap_or_default();
    let expected_package = compilation
        .typed
        .symbols
        .symbol_package_identity(machine.symbol);
    let expected_table = machine
        .attached_data
        .as_ref()
        .map(|name| name.as_str())
        .unwrap_or_default();
    let binding_matches = match (binding, &row.binding) {
        (
            PackageReviewExternalBinding::Import { library, symbol },
            omega_effects::provider_plan::ProviderBinding::StringBackedImportBootstrap {
                library: selected_library,
                symbol: selected_symbol,
            },
        ) => library == selected_library && symbol == selected_symbol,
        (
            PackageReviewExternalBinding::Syscall { number },
            omega_effects::provider_plan::ProviderBinding::Syscall {
                number: selected_number,
            },
        ) => number == selected_number,
        (
            PackageReviewExternalBinding::CompilerIntrinsic,
            omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic {
                machine: selected_machine,
            },
        ) => selected_machine == &expected_machine_identity,
        (
            PackageReviewExternalBinding::VtableSlot { index },
            omega_effects::provider_plan::ProviderBinding::VtableSlot {
                index: selected_index,
            },
        ) => index == selected_index,
        (
            PackageReviewExternalBinding::VtableField { field },
            omega_effects::provider_plan::ProviderBinding::VtableField {
                table,
                field: selected_field,
            },
        ) => table == expected_table && field == selected_field,
        (
            PackageReviewExternalBinding::TableFunction { field },
            omega_effects::provider_plan::ProviderBinding::TableFunction {
                table,
                field: selected_field,
            },
        ) => table == expected_table && field == selected_field,
        _ => false,
    };
    if retained.plan != **plan
        || *requirement_symbol != operator.symbol
        || *realization_symbol != machine.symbol
        || plan.origin_package_identity != expected_package
        || method.requirement_owner != slot
        || method.requirement_identity != slot
        || row.requirement_identity != slot
        || !binding_matches
    {
        return Err(vec![Diagnostic::error(format!(
            "selected boundary-operator ProviderPlan `{}` does not join exact operator `{slot}` to external leaf `{}` and its binding",
            plan.name, machine.name,
        ))]);
    }
    Ok(())
}

fn validate_external_binding_payload(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
    identity: &psi_language_semantics::ExternalBindingIdentity,
) -> Result<(), Vec<Diagnostic>> {
    use psi_language_semantics::ExternalBindingIdentity;

    let invalid = match identity {
        ExternalBindingIdentity::Import { library, symbol } if library.is_empty() => {
            Some("has no exact import-library identity")
        }
        ExternalBindingIdentity::Import { symbol, .. } if symbol.is_empty() => {
            Some("has no exact import-symbol identity")
        }
        ExternalBindingIdentity::Syscall { number } if u32::try_from(*number).is_err() => {
            Some("has a syscall number outside 0..=u32::MAX")
        }
        ExternalBindingIdentity::VtableSlot { index } if *index < 0 => {
            Some("has a negative vtable-slot index")
        }
        ExternalBindingIdentity::VtableField { field }
        | ExternalBindingIdentity::TableFunction { field }
            if field.is_empty() =>
        {
            Some("has no exact table-field identity")
        }
        ExternalBindingIdentity::VtableField { .. }
        | ExternalBindingIdentity::TableFunction { .. }
            if !machine.attached_data_symbol.is_valid()
                || machine.attached_data.is_none()
                || !compilation
                    .data_definitions()
                    .iter()
                    .any(|definition| definition.symbol == machine.attached_data_symbol) =>
        {
            Some("has table-field supply without one exact attached provider data declaration")
        }
        ExternalBindingIdentity::Import { .. }
        | ExternalBindingIdentity::Syscall { .. }
        | ExternalBindingIdentity::CompilerIntrinsic
        | ExternalBindingIdentity::VtableSlot { .. }
        | ExternalBindingIdentity::VtableField { .. }
        | ExternalBindingIdentity::TableFunction { .. } => None,
    };
    match invalid {
        Some(reason) => Err(vec![Diagnostic::error(format!(
            "reviewed external callable `{}` {reason}",
            machine.name
        ))]),
        None => Ok(()),
    }
}

fn project_external_binding(
    identity: &psi_language_semantics::ExternalBindingIdentity,
) -> PackageReviewExternalBinding {
    match identity {
        psi_language_semantics::ExternalBindingIdentity::Import { library, symbol } => {
            PackageReviewExternalBinding::Import {
                library: library.clone(),
                symbol: symbol.clone(),
            }
        }
        psi_language_semantics::ExternalBindingIdentity::Syscall { number } => {
            PackageReviewExternalBinding::Syscall { number: *number }
        }
        psi_language_semantics::ExternalBindingIdentity::CompilerIntrinsic => {
            PackageReviewExternalBinding::CompilerIntrinsic
        }
        psi_language_semantics::ExternalBindingIdentity::VtableSlot { index } => {
            PackageReviewExternalBinding::VtableSlot { index: *index }
        }
        psi_language_semantics::ExternalBindingIdentity::VtableField { field } => {
            PackageReviewExternalBinding::VtableField {
                field: field.clone(),
            }
        }
        psi_language_semantics::ExternalBindingIdentity::TableFunction { field } => {
            PackageReviewExternalBinding::TableFunction {
                field: field.clone(),
            }
        }
    }
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
        ([(owner, requirement)], []) => trait_requirement_identity(compilation, owner, requirement),
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

fn project_machine_parameter_termination(
    compilation: &CheckedCompilation,
    signature: &psi_typed_trees::signature::StateSignature,
    declaration_path: &str,
) -> Result<PackageReviewTermination, Vec<Diagnostic>> {
    let parameters = compilation.state_signature_parameters(signature);
    if let psi_language_semantics::TerminationGuarantee::Terminates { premises } =
        &signature.termination_guarantee
    {
        for premise in premises {
            let profile = compilation
                .domain_definitions()
                .iter()
                .find(|domain| domain.semantic_id == premise.profile)
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "public static-machine parameter on `{declaration_path}` has an unknown termination profile",
                    ))]
                })?;
            if !profile.is_public {
                return Err(vec![Diagnostic::error(format!(
                    "public static-machine parameter on `{declaration_path}` exposes non-public progress profile `{}`",
                    profile.name,
                ))]);
            }
        }
    }
    project_termination_with_subject(compilation, &signature.termination_guarantee, |root| {
        let parameter = parameters
            .iter()
            .find(|parameter| parameter.symbol == root)
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "public static-machine parameter on `{declaration_path}` has a termination premise outside its parameter telescope",
                ))]
            })?;
        if parameter.is_self {
            return Ok(PackageReviewProgressSubject::Receiver);
        }
        let position = parameters
            .iter()
            .filter(|candidate| !candidate.is_self)
            .position(|candidate| candidate.symbol == root)
            .expect("matched non-self machine-parameter contract parameter must have an ordinal");
        Ok(PackageReviewProgressSubject::Parameter(
            portable_parameter_position(position)?,
        ))
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

fn trait_requirement_identity(
    compilation: &CheckedCompilation,
    owner: &psi_typed_trees::trait_definition::TraitDefinition,
    requirement: &psi_typed_trees::signature::StateSignature,
) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
    let owner_identity = nominal_identity(compilation, owner.symbol)?;
    let requirement_owner = nominal_owner(compilation, requirement.symbol)?;
    if owner_identity.owner != requirement_owner {
        return Err(vec![Diagnostic::error(format!(
            "package review trait `{}` and requirement `{}` have mismatched exact ownership",
            owner.name, requirement.name
        ))]);
    }
    Ok(PackageReviewNominalIdentity {
        owner: requirement_owner,
        path: compilation
            .normalized_trait_requirement_overload_identity(owner, requirement)
            .identity(),
    })
}

fn trait_requirement_identity_from_symbols(
    compilation: &CheckedCompilation,
    trait_symbol: SymbolHandle,
    requirement_symbol: SymbolHandle,
    context: &str,
) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
    let owners = compilation
        .traits()
        .iter()
        .filter(|candidate| candidate.symbol == trait_symbol)
        .collect::<Vec<_>>();
    let [owner] = owners.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "{context} resolves its declaring trait to {} declarations; expected exactly one",
            owners.len()
        ))]);
    };
    let requirements = compilation
        .trait_machine_signatures(owner)
        .iter()
        .filter(|candidate| candidate.symbol == requirement_symbol)
        .collect::<Vec<_>>();
    let [requirement] = requirements.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "{context} resolves its requirement to {} overload declarations under its exact trait; expected exactly one",
            requirements.len()
        ))]);
    };
    trait_requirement_identity(compilation, owner, requirement)
}

fn provider_requirement_identity(
    compilation: &CheckedCompilation,
    schema: super::provider_plans::ProviderSchemaDeclaration,
    requirement_symbol: SymbolHandle,
) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
    match schema {
        super::provider_plans::ProviderSchemaDeclaration::BoundaryTrait(trait_symbol) => {
            trait_requirement_identity_from_symbols(
                compilation,
                trait_symbol,
                requirement_symbol,
                "selected provider row",
            )
        }
        super::provider_plans::ProviderSchemaDeclaration::BoundaryOperator(_) => {
            let operators = compilation.operators().iter().chain(
                compilation
                    .domain_definitions()
                    .iter()
                    .flat_map(|domain| compilation.domain_operators(domain)),
            );
            let matches = operators
                .filter(|candidate| candidate.symbol == requirement_symbol && candidate.is_boundary)
                .collect::<Vec<_>>();
            let [operator] = matches.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "selected provider row resolves its boundary operator requirement to {} declarations; expected exactly one",
                    matches.len()
                ))]);
            };
            let nominal = nominal_identity(compilation, requirement_symbol)?;
            Ok(PackageReviewNominalIdentity {
                owner: nominal.owner,
                path: psi_typed_trees::operator::boundary_operator_requirement_identity(
                    &compilation.typed,
                    operator,
                ),
            })
        }
    }
}

fn nominal_owner(
    compilation: &CheckedCompilation,
    symbol: SymbolHandle,
) -> Result<PackageReviewNominalOwner, Vec<Diagnostic>> {
    nominal_owner_from_symbols(&compilation.typed.symbols, symbol)
}

fn nominal_owner_from_symbols(
    symbols: &psi_symbols::SymbolTable,
    symbol: SymbolHandle,
) -> Result<PackageReviewNominalOwner, Vec<Diagnostic>> {
    if let Some(package) = symbols.symbol_package_identity(symbol) {
        return Ok(PackageReviewNominalOwner::Package(package));
    }
    let Some(source_file) = symbols
        .symbol_provenance_source_span(symbol)
        .and_then(|span| symbols.source_file(span))
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
    Ok(PackageReviewToolchainSourceIdentity {
        digest: super::package_source_consumption::toolchain_source_identity_digest(source_file)?,
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
    use super::{
        PackageReviewNominalIdentity, PackageReviewNominalOwner,
        PackageReviewToolchainSourceIdentity, nominal_owner_from_symbols,
        toolchain_source_identity, validate_package_type_identity_input,
        validate_package_type_identity_input_inner, validate_selected_provider_declaration_owner,
    };
    use psi_core::PackageKeyIdentity;
    use psi_source::{SourceFile, SourceId, SourceMap, SourceOrigin, SourceSpan, Span};
    use psi_symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTable, SymbolTableBuilder};
    use std::path::PathBuf;
    use std::sync::Arc;

    #[test]
    fn selected_provider_declaration_ownership_is_exact_and_fail_closed() {
        let package = PackageKeyIdentity::from_digest([1; 32]).expect("package identity");
        let other_package =
            PackageKeyIdentity::from_digest([2; 32]).expect("other package identity");
        let package_declaration = PackageReviewNominalIdentity {
            owner: PackageReviewNominalOwner::Package(package),
            path: "service::requirement".to_owned(),
        };
        let toolchain_declaration = PackageReviewNominalIdentity {
            owner: PackageReviewNominalOwner::ToolchainSource(
                PackageReviewToolchainSourceIdentity { digest: [3; 32] },
            ),
            path: "service::requirement".to_owned(),
        };
        let unresolved_declaration = PackageReviewNominalIdentity {
            owner: PackageReviewNominalOwner::Unresolved,
            path: "service::requirement".to_owned(),
        };

        validate_selected_provider_declaration_owner(
            &package_declaration,
            Some(package),
            "plan",
            "row requirement",
        )
        .expect("an exact package owner must pass");
        validate_selected_provider_declaration_owner(
            &toolchain_declaration,
            None,
            "plan",
            "row requirement",
        )
        .expect("an exact authored toolchain source must pass");

        for (declaration, expected_package) in [
            (&package_declaration, Some(other_package)),
            (&package_declaration, None),
            (&toolchain_declaration, Some(package)),
            (&unresolved_declaration, Some(package)),
            (&unresolved_declaration, None),
        ] {
            let error = validate_selected_provider_declaration_owner(
                declaration,
                expected_package,
                "plan",
                "row requirement",
            )
            .expect_err("mismatched or unresolved ownership must reject");
            assert!(
                error[0]
                    .message
                    .contains("exact package/toolchain ownership")
            );
        }
    }

    #[test]
    fn package_type_identity_rejects_textual_and_unselected_fallbacks() {
        use psi_typed_trees::expression::{BinaryOperator, ExpressionNode, TableBinaryExpression};
        use psi_typed_trees::name::Identifier;
        use psi_typed_trees::types::{
            DomainConstraint, DomainConstraintSubject, FixedArrayLength, TypeConstraintNode,
            TypeReferenceNode,
        };

        let mut program = psi_typed_trees::TypedTrees::default();
        let element_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let residual = program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::ConstCall {
                    name: Identifier::generated("length"),
                    source_span: SourceSpan::default(),
                },
            });
        let error = validate_package_type_identity_input(&program, residual, &[])
            .expect_err("residual const call must reject package evidence");
        assert!(error[0].message.contains("unevaluated const call"));

        let textual = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated("source_spelling"),
            });
        let error = validate_package_type_identity_input(&program, textual, &[])
            .expect_err("unresolved source spelling must reject package evidence");
        assert!(error[0].message.contains("without exact semantic identity"));

        let misplaced_const = psi_language_semantics::const_value::CanonicalConstValue::new(
            "u32",
            "integer3:u321:7",
            "7",
        );
        let misplaced_const = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated(misplaced_const.atom()),
            });
        let error = validate_package_type_identity_input(&program, misplaced_const, &[])
            .expect_err("canonical const outside a declared const slot must reject");
        assert!(error[0].message.contains("without exact semantic identity"));

        let unresolved_binder =
            program
                .type_reference_table
                .insert(TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::ConstParameter {
                        symbol: SymbolHandle::invalid(),
                        name: Identifier::generated("N"),
                    },
                });
        let error = validate_package_type_identity_input(&program, unresolved_binder, &[])
            .expect_err("unreconciled const binder must reject package evidence");
        assert!(error[0].message.contains("exact telescope identity"));

        let left = program.expression_table.insert(ExpressionNode::Integer(
            psi_numerics::literals::IntegerLiteral::zero(),
        ));
        let right = program.expression_table.insert(ExpressionNode::Integer(
            psi_numerics::literals::IntegerLiteral::zero(),
        ));
        let binary =
            program
                .expression_table
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator: BinaryOperator::Add,
                    right,
                }));
        let open_index = program
            .type_reference_table
            .insert(TypeReferenceNode::ConstExpression(binary));
        let error = validate_package_type_identity_input_inner(&program, open_index, &[], true)
            .expect_err("unselected open index operation must reject package evidence");
        assert!(error[0].message.contains("without exact checked selection"));

        let unsupported = program
            .expression_table
            .insert(ExpressionNode::Boolean(true));
        let unsupported = program
            .type_reference_table
            .insert(TypeReferenceNode::ConstExpression(unsupported));
        let error = validate_package_type_identity_input_inner(&program, unsupported, &[], true)
            .expect_err("unsupported index shape must reject package evidence");
        assert!(error[0].message.contains("unsupported structural index"));

        let legacy_layout =
            program
                .type_reference_table
                .insert_constraints([TypeConstraintNode::Domain(DomainConstraint {
                    name: Identifier::generated("OmegaLayout<Save>"),
                    ..DomainConstraint::default()
                })]);
        let legacy_layout = program
            .type_reference_table
            .insert(TypeReferenceNode::Constrained {
                base_type: element_type,
                constraints: legacy_layout,
            });
        let error = validate_package_type_identity_input(&program, legacy_layout, &[])
            .expect_err("flattened layout spelling must reject package evidence");
        assert!(error[0].message.contains("legacy flattened OmegaLayout"));

        let malformed_carry =
            program
                .type_reference_table
                .insert_constraints([TypeConstraintNode::Domain(DomainConstraint {
                    name: Identifier::generated("diagnostic-only"),
                    arguments: vec![element_type],
                    subject: DomainConstraintSubject::Carry(
                        psi_language_semantics::CarryPermission::AnyCpu,
                    ),
                    ..DomainConstraint::default()
                })]);
        let malformed_carry = program
            .type_reference_table
            .insert(TypeReferenceNode::Constrained {
                base_type: element_type,
                constraints: malformed_carry,
            });
        let error = validate_package_type_identity_input(&program, malformed_carry, &[])
            .expect_err("malformed closed domain must reject package evidence");
        assert!(error[0].message.contains("malformed compiler-owned scalar"));
    }

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

    fn generated_symbol_owner(
        origin: SourceOrigin,
        package_identity: Option<PackageKeyIdentity>,
    ) -> PackageReviewNominalOwner {
        let mut sources = SourceMap::default();
        let source_id = sources
            .add_with_metadata(
                PathBuf::from("toolchain/std/origin.omg"),
                String::from("origin"),
                PathBuf::from("toolchain/std"),
                package_identity,
                origin,
            )
            .source_id;
        let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let authored = SymbolTableBuilder::child_handles(builder.insert_children(
            root,
            [(
                SymbolKind::Machine,
                SymbolNameRef::Source(SourceSpan::new(source_id, Span::new(0, 6))),
            )],
        ))
        .next()
        .expect("authored derivation origin");
        let mut symbols = builder.finish();
        let generated =
            symbols.insert_generated_root_from(authored, SymbolKind::Machine, "generated_origin");
        nominal_owner_from_symbols(&symbols, generated).expect("generated nominal owner")
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

    #[test]
    fn generated_nominals_follow_exact_derivation_ownership() {
        assert!(matches!(
            generated_symbol_owner(SourceOrigin::Toolchain, None),
            PackageReviewNominalOwner::ToolchainSource(_)
        ));

        let package_identity =
            PackageKeyIdentity::from_digest([41; 32]).expect("nonzero package identity");
        assert_eq!(
            generated_symbol_owner(SourceOrigin::User, Some(package_identity)),
            PackageReviewNominalOwner::Package(package_identity)
        );

        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let source_free = SymbolTableBuilder::child_handles(builder.insert_children(
            root,
            [(SymbolKind::Machine, SymbolNameRef::Static("source_free"))],
        ))
        .next()
        .expect("source-free symbol");
        let symbols: SymbolTable = builder.finish();
        assert_eq!(
            nominal_owner_from_symbols(&symbols, source_free).expect("source-free nominal owner"),
            PackageReviewNominalOwner::Unresolved
        );
    }
}
