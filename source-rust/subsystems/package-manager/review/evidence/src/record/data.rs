// Stable public-data evidence.
//
// `PackageReviewDataKind::Quotient` records the carrier family plus relation
// declaration; the proof implementation that licensed formation is not API
// identity. The properties record retains the declaration's multiplicity.

use crate::record::{PackageReviewNominalIdentity, PackageReviewTypeIdentity};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewDataField {
    pub identity: Option<u64>,
    pub name: String,
    pub relevance: language_core::BindingRelevance,
    pub type_identity: PackageReviewTypeIdentity,
}

impl PackageReviewDataField {}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageReviewDataKind {
    Ordinary,
    Quotient {
        carrier: PackageReviewTypeIdentity,
        relation: PackageReviewNominalIdentity,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageReviewDataProperties {
    pub multiplicity: language_core::Multiplicity,
}

impl PackageReviewDataProperties {}
