use super::*;

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
