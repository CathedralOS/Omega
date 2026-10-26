pub(crate) use super::authority_expressions::{
    PackageReviewBooleanExpression, PackageReviewIeeeFloatComparisonKind,
    PackageReviewIntegerBinaryKind, PackageReviewIntegerComparisonKind,
    PackageReviewIntegerLiteral, PackageReviewIntegerLiteralLanding, PackageReviewIntegerRange,
    PackageReviewPrimitiveType, PackageReviewRuntimeIndex, PackageReviewScalarExpression,
    PackageReviewStructuralParameterField, PackageReviewStructuralPredicatePathSegment,
};
use super::{contracts::PackageReviewContractExpression, identity::PackageReviewNominalIdentity};

/// Core critical-service or consuming-root audit policy for a reviewed package.
///
/// Core classes require exact toolchain provenance; custom classification
/// requires an executed root registration for the exact boundary identity.
/// Neither classifies exercised physical authority or grants permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageReviewDangerousAuthorityClass {
    Filesystem,
    MachineControl,
    PortIo,
    InterruptControl,
    InterruptEntry,
    RootMemory,
    Process,
    ConsumerAudit,
}

/// One exact reached/invoked service requiring core or consuming-root audit.
/// This is review evidence, not an authority grant.
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
    crate::record::accessors::record_accessors!(requirement);
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
pub(crate) struct PackageReviewCrashPredicate {
    pub(crate) canonical_bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PackageReviewCrashRouteGuard {
    Truth,
    Predicate(PackageReviewCrashPredicate),
    /// Exact structural guard for an abstract public-operator crash ceiling.
    /// Unlike runtime crash-predicate bytes, this retains selected nominal
    /// package identity for calls, members, and declared overloads.
    Expression(PackageReviewContractExpression),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PackageReviewCrashRoute {
    pub(crate) cause: PackageReviewCrashCause,
    pub(crate) alternative_guards: Vec<PackageReviewCrashRouteGuard>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PackageReviewProgressSubject {
    Declaration(PackageReviewNominalIdentity),
    Parameter(u32),
}

impl PackageReviewProgressSubject {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PackageReviewProgressPremise {
    pub(crate) profile: PackageReviewNominalIdentity,
    pub(crate) subject: PackageReviewProgressSubject,
    pub(crate) projections: Vec<PackageReviewNominalIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PackageReviewTermination {
    NoGuarantee,
    Terminates {
        premises: Vec<PackageReviewProgressPremise>,
    },
}
