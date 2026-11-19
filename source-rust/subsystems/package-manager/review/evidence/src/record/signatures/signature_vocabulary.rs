// Canonical type identities, proposition/signature parameters and conformance bounds.

use crate::record::PackageReviewNominalIdentity;

use crate::record::accessors::field_projection_accessors;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewTypeIdentity {
    pub(crate) canonical: String,
}

impl PackageReviewTypeIdentity {
    pub fn canonical(&self) -> &str {
        &self.canonical
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewPropositionParameterSignature {
    pub(crate) parameters: Vec<PackageReviewPropositionParameterValue>,
}

impl PackageReviewPropositionParameterSignature {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PackageReviewPropositionParameterValue {
    pub(crate) type_identity: PackageReviewTypeIdentity,
    pub(crate) is_const: bool,
    pub(crate) is_mutable: bool,
}

impl PackageReviewPropositionParameterValue {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewSignatureParameter {
    pub(crate) name: String,
    pub(crate) type_identity: PackageReviewTypeIdentity,
    pub(crate) is_const: bool,
    pub(crate) is_mutable: bool,
}

impl PackageReviewSignatureParameter {
    crate::record::accessors::signature_parameter_accessors!();
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewConformanceBound {
    pub(crate) binder_ordinal: u32,
    pub(crate) trait_identity: PackageReviewNominalIdentity,
    pub(crate) trait_lifetime_arguments: Vec<u32>,
    pub(crate) arguments: Vec<PackageReviewTypeIdentity>,
}

impl PackageReviewConformanceBound {
    field_projection_accessors!(copied binder_ordinal: u32);
    field_projection_accessors!(borrowed trait_identity: &PackageReviewNominalIdentity);
    field_projection_accessors!(sliced trait_lifetime_arguments: &[u32]);
    field_projection_accessors!(sliced arguments: &[PackageReviewTypeIdentity]);
}
