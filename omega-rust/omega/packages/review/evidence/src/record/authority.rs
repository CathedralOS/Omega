pub use super::authority_expressions::{
    PackageReviewBooleanExpression, PackageReviewIeeeFloatComparisonKind,
    PackageReviewIntegerBinaryKind, PackageReviewIntegerComparisonKind,
    PackageReviewIntegerLiteral, PackageReviewIntegerLiteralLanding, PackageReviewIntegerRange,
    PackageReviewPrimitiveType, PackageReviewScalarExpression,
    PackageReviewStructuralParameterField, PackageReviewStructuralPredicatePathSegment,
};
use super::{contracts::PackageReviewContractExpression, identity::PackageReviewNominalIdentity};

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
    pub(crate) class: PackageReviewDangerousAuthorityClass,
    pub(crate) service: PackageReviewNominalIdentity,
}

/// A dangerous service present in a checked callable's published ceiling but
/// absent from that callable's checked transitive body reach.
///
/// This is review guidance, not a claim that the declaration is malicious or
/// that bodyless supply failed to realize anything.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewDangerousAuthoritySlack {
    pub(crate) class: PackageReviewDangerousAuthorityClass,
    pub(crate) callable: PackageReviewNominalIdentity,
    pub(crate) service: PackageReviewNominalIdentity,
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
    pub(crate) capability: PackageReviewNominalIdentity,
    pub(crate) kind: psi_effects::CapabilityFlowKind,
    pub(crate) state: PackageReviewNominalIdentity,
    pub(crate) statement_index: usize,
    pub(crate) call_ordinal: usize,
    pub(crate) via_state: Option<PackageReviewNominalIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewInstallationReach {
    pub(crate) requirement: PackageReviewNominalIdentity,
    pub(crate) upper_bound: Vec<PackageReviewNominalIdentity>,
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
    pub(crate) state: PackageReviewNominalIdentity,
    pub(crate) completeness: PackageReviewWriteFrameCompleteness,
    pub(crate) paths: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewWriteFrameCompleteness {
    Complete,
    Opaque,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewCrashCause {
    Trap,
    Abort,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewCrashInterface {
    InternalInferred,
    PublishedCeiling,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewCrashPredicate {
    pub(crate) canonical_bytes: Vec<u8>,
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
    pub(crate) cause: PackageReviewCrashCause,
    pub(crate) alternative_guards: Vec<PackageReviewCrashRouteGuard>,
}

impl PackageReviewCrashRoute {
    pub const fn cause(&self) -> PackageReviewCrashCause {
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
    pub(crate) machine: PackageReviewNominalIdentity,
    pub(crate) state: PackageReviewNominalIdentity,
    pub(crate) source: PackageReviewPermissionSource,
    pub(crate) ordinal: u32,
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
    pub(crate) state: PackageReviewNominalIdentity,
    pub(crate) statement_ordinal: u32,
    pub(crate) cause: PackageReviewCrashCause,
    pub(crate) path_guard_conjuncts: Vec<PackageReviewCrashPredicate>,
    pub(crate) path_guard_consequences: Vec<PackageReviewCrashPredicate>,
    pub(crate) guard_covering_buckets: Vec<u32>,
    pub(crate) frontier_lower_bound: Vec<PackageReviewPermissionClaim>,
}

impl PackageReviewCrashSite {
    pub fn state(&self) -> &PackageReviewNominalIdentity {
        &self.state
    }

    pub const fn statement_ordinal(&self) -> u32 {
        self.statement_ordinal
    }

    pub const fn cause(&self) -> PackageReviewCrashCause {
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
    pub(crate) state: PackageReviewNominalIdentity,
    pub(crate) statement_ordinal: u32,
    pub(crate) call_ordinal: u32,
    pub(crate) target_machine: PackageReviewNominalIdentity,
    pub(crate) target_state: PackageReviewNominalIdentity,
    pub(crate) path_guard_conjuncts: Vec<PackageReviewCrashPredicate>,
    pub(crate) path_guard_consequences: Vec<PackageReviewCrashPredicate>,
    pub(crate) surviving_buckets: Vec<PackageReviewCrashRoute>,
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
    pub(crate) interface: PackageReviewCrashInterface,
    pub(crate) published: Vec<PackageReviewCrashRoute>,
    pub(crate) structural_runtime_requirements: Option<Vec<PackageReviewBooleanExpression>>,
    pub(crate) checked_sites: Vec<PackageReviewCrashSite>,
    pub(crate) checked_calls: Vec<PackageReviewCrashCall>,
}

impl PackageReviewCrash {
    pub const fn interface(&self) -> PackageReviewCrashInterface {
        self.interface
    }

    pub fn published(&self) -> &[PackageReviewCrashRoute] {
        &self.published
    }

    pub fn structural_runtime_requirements(&self) -> Option<&[PackageReviewBooleanExpression]> {
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
    pub(crate) profile: PackageReviewNominalIdentity,
    pub(crate) subject: PackageReviewProgressSubject,
    pub(crate) projections: Vec<PackageReviewNominalIdentity>,
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
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

    pub const fn completeness(&self) -> PackageReviewWriteFrameCompleteness {
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
