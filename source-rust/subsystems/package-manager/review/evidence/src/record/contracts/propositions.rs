// Public proposition declarations, binders, evidence interfaces, and
// applications (`PackageReviewPropositionShape` and
// `PackageReviewPropositionApplication`).

use crate::record::PackageReviewContractExpression;
use crate::record::PackageReviewContractFact;
use crate::record::PackageReviewContractKind;
use crate::record::PackageReviewDataProperties;
use crate::record::PackageReviewNominalIdentity;
use crate::record::PackageReviewTypeIdentity;
use crate::record::accessors::field_projection_accessors;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(
    not(any(test, feature = "full-policy-recovery")),
    expect(
        dead_code,
        reason = "only full policy recovery constructs these wire variants"
    )
)]
pub(crate) enum PackageReviewPropositionBinderKind {
    Type,
    Const(PackageReviewTypeIdentity),
    Machine,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewPropositionBinder {
    pub(crate) kind: PackageReviewPropositionBinderKind,
    pub(crate) bounds: PackageReviewDataProperties,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(
    not(any(test, feature = "full-policy-recovery")),
    expect(
        dead_code,
        reason = "only full policy recovery constructs these wire variants"
    )
)]
pub(crate) enum PackageReviewPropositionBinderValue {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PackageReviewPropositionBinderArgumentKind {
    Type,
    Const,
    Machine,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PackageReviewPropositionBinderArgument {
    pub(crate) kind: PackageReviewPropositionBinderArgumentKind,
    pub(crate) value: PackageReviewPropositionBinderValue,
}

impl PackageReviewPropositionBinderArgument {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewEvidenceRequirement {
    pub declaring_trait: PackageReviewNominalIdentity,
    pub declaring_trait_lifetime_arguments: Vec<u32>,
    pub declaring_trait_arguments: Vec<PackageReviewTypeIdentity>,
    pub requirement: PackageReviewNominalIdentity,
}

impl PackageReviewEvidenceRequirement {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewEvidenceInterface {
    pub trait_identity: PackageReviewNominalIdentity,
    pub lifetime_arguments: Vec<u32>,
    pub arguments: Vec<PackageReviewTypeIdentity>,
    pub requirements: Vec<PackageReviewEvidenceRequirement>,
}

impl PackageReviewEvidenceInterface {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(
    not(any(test, feature = "full-policy-recovery")),
    expect(
        dead_code,
        reason = "only full policy recovery constructs these wire variants"
    )
)]
pub(crate) enum PackageReviewPropositionEvidence {
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
    field_projection_accessors!(borrowed identity: &PackageReviewNominalIdentity);
    field_projection_accessors!(sliced binders: &[PackageReviewPropositionBinder]);
    field_projection_accessors!(sliced parameter_types: &[PackageReviewTypeIdentity]);
    field_projection_accessors!(borrowed body: &PackageReviewPublicPropositionBody);
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

impl PackageReviewPropositionApplication {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewPropositionParameterApplication {
    pub(crate) binder_ordinal: u32,
    pub(crate) arguments: Vec<PackageReviewContractExpression>,
}

impl PackageReviewPropositionParameterApplication {}
