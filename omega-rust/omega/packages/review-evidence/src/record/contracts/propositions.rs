use crate::record::PackageReviewContractExpression;
use crate::record::PackageReviewContractFact;
use crate::record::PackageReviewContractKind;
use crate::record::PackageReviewDataProperties;
use crate::record::PackageReviewNominalIdentity;
use crate::record::PackageReviewTypeIdentity;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PackageReviewPropositionBinderKind {
    Type,
    Const(PackageReviewTypeIdentity),
    Machine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewPropositionBinder {
    pub(crate) kind: PackageReviewPropositionBinderKind,
    pub(crate) bounds: PackageReviewDataProperties,
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
    properties: PackageReviewDataProperties,
) -> (u8, Option<(u8, u8, u8, u8)>) {
    let multiplicity = match properties.multiplicity {
        language_semantics::Multiplicity::Unrestricted => 0,
        language_semantics::Multiplicity::Affine => 1,
        language_semantics::Multiplicity::Linear => 2,
    };
    let carry = properties.carry.map(|carry| {
        (
            u8::from(matches!(
                carry.suspension,
                language_semantics::CarrySuspension::Allowed
            )),
            u8::from(matches!(carry.cpu, language_semantics::CarryCpu::Any)),
            u8::from(matches!(
                carry.host_thread,
                language_semantics::CarryHostThread::Any
            )),
            u8::from(matches!(
                carry.address,
                language_semantics::CarryAddress::Movable
            )),
        )
    });
    (multiplicity, carry)
}

impl PackageReviewPropositionBinder {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
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
pub(crate) struct PackageReviewEvidenceRequirement {
    pub(crate) declaring_trait: PackageReviewNominalIdentity,
    pub(crate) declaring_trait_lifetime_arguments: Vec<u32>,
    pub(crate) declaring_trait_arguments: Vec<PackageReviewTypeIdentity>,
    pub(crate) requirement: PackageReviewNominalIdentity,
}

impl PackageReviewEvidenceRequirement {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewEvidenceInterface {
    pub(crate) trait_identity: PackageReviewNominalIdentity,
    pub(crate) lifetime_arguments: Vec<u32>,
    pub(crate) arguments: Vec<PackageReviewTypeIdentity>,
    pub(crate) requirements: Vec<PackageReviewEvidenceRequirement>,
}

impl PackageReviewEvidenceInterface {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
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
    crate::record::accessors::record_accessors!(identity);
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
