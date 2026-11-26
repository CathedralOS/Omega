// Dangerous-authority audit classes, slack records, installation reach, crash
// routes, and termination premises for reviewed packages.
//
// Core dangerous-authority classes require exact toolchain provenance, while
// `ConsumerAudit` requires an executed root registration for the exact boundary
// identity. Neither classifies exercised physical authority or grants
// permission. `PackageReviewDangerousAuthoritySlack` records a dangerous
// service present in a callable's published ceiling but absent from its checked
// transitive body reach.

pub(crate) use super::authority_expressions::{
    PackageReviewBooleanExpression, PackageReviewIeeeFloatComparisonKind,
    PackageReviewIntegerBinaryKind, PackageReviewIntegerComparisonKind,
    PackageReviewIntegerLiteral, PackageReviewIntegerLiteralLanding, PackageReviewIntegerRange,
    PackageReviewPrimitiveType, PackageReviewRuntimeIndex, PackageReviewScalarExpression,
    PackageReviewStructuralParameterField, PackageReviewStructuralPredicatePathSegment,
};
use super::identity::PackageReviewNominalIdentity;
use crate::record::PackageReviewServiceIdentity;
use crate::record::accessors::field_projection_accessors;

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
    Network,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewDangerousAuthority {
    pub(crate) class: PackageReviewDangerousAuthorityClass,
    pub(crate) service: PackageReviewServiceIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewDangerousAuthoritySlack {
    pub(crate) class: PackageReviewDangerousAuthorityClass,
    pub(crate) callable: PackageReviewNominalIdentity,
    pub(crate) service: PackageReviewServiceIdentity,
}

impl PackageReviewDangerousAuthoritySlack {
    field_projection_accessors!(copied class: PackageReviewDangerousAuthorityClass);
    field_projection_accessors!(borrowed callable: &PackageReviewNominalIdentity);
    field_projection_accessors!(borrowed service: &PackageReviewServiceIdentity);
}

impl PackageReviewDangerousAuthority {
    field_projection_accessors!(copied class: PackageReviewDangerousAuthorityClass);
    field_projection_accessors!(borrowed service: &PackageReviewServiceIdentity);
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewInstallationReach {
    pub requirement: PackageReviewNominalIdentity,
    pub upper_bound: Vec<PackageReviewServiceIdentity>,
}

impl PackageReviewInstallationReach {
    field_projection_accessors!(borrowed requirement: &PackageReviewNominalIdentity);
    field_projection_accessors!(sliced upper_bound: &[PackageReviewServiceIdentity]);
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
pub(crate) enum PackageReviewProgressSubject {
    Declaration(PackageReviewNominalIdentity),
    Parameter(u32),
}

impl PackageReviewProgressSubject {}
