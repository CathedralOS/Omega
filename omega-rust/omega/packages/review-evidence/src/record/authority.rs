pub use super::authority_expressions::{
    PackageReviewBooleanExpression, PackageReviewIeeeFloatComparisonKind,
    PackageReviewIntegerBinaryKind, PackageReviewIntegerComparisonKind,
    PackageReviewIntegerLiteral, PackageReviewIntegerLiteralLanding, PackageReviewIntegerRange,
    PackageReviewPrimitiveType, PackageReviewRuntimeIndex, PackageReviewScalarExpression,
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

impl PackageReviewCrashPredicate {}

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

impl PackageReviewCrashRoute {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewProgressSubject {
    Declaration(PackageReviewNominalIdentity),
    Parameter(u32),
}

impl PackageReviewProgressSubject {
    pub const fn declaration(&self) -> Option<&PackageReviewNominalIdentity> {
        match self {
            Self::Declaration(identity) => Some(identity),
            Self::Parameter(_) => None,
        }
    }

    pub const fn parameter(&self) -> Option<u32> {
        match self {
            Self::Parameter(position) => Some(*position),
            Self::Declaration(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewProgressPremise {
    pub(crate) profile: PackageReviewNominalIdentity,
    pub(crate) subject: PackageReviewProgressSubject,
    pub(crate) projections: Vec<PackageReviewNominalIdentity>,
}

impl PackageReviewProgressPremise {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewTermination {
    NoGuarantee,
    Terminates {
        premises: Vec<PackageReviewProgressPremise>,
    },
}

impl PackageReviewTermination {}
