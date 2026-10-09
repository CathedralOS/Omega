//! Stable public-data evidence.

use super::{
    contracts::PackageReviewContractFact,
    identity::PackageReviewNominalIdentity,
    signatures::{PackageReviewTypeIdentity, PackageReviewTypeParameter},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewDataField {
    pub(crate) identity: Option<u64>,
    pub(crate) name: String,
    pub(crate) relevance: language_core::BindingRelevance,
    pub(crate) type_identity: PackageReviewTypeIdentity,
}

impl PackageReviewDataField {



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

/// Closed package-evidence carrier for declaration multiplicity and movement
/// policy. The language semantics remain explicit without retaining a typed-
/// tree declaration node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewDataProperties {
    pub(crate) multiplicity: language_semantics::Multiplicity,
    pub(crate) carry: Option<language_semantics::CarryPolicy>,
}

impl PackageReviewDataProperties {

}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageReviewDataShape {
    pub(crate) identity: PackageReviewNominalIdentity,
    pub(crate) kind: PackageReviewDataKind,
    pub(crate) supply: language_semantics::DataSupplyMode,
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) type_parameters: Vec<PackageReviewTypeParameter>,
    pub(crate) properties: PackageReviewDataProperties,
    pub(crate) zero_gated: bool,
    pub(crate) invariants: Vec<PackageReviewContractFact>,
    pub(crate) retired_identities: Vec<u64>,
    pub(crate) members: Vec<PackageReviewDataMember>,
}
