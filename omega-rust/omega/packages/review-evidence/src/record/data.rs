// Stable public-data evidence.
//
// `PackageReviewDataKind::Quotient` records the carrier family plus relation
// declaration; the proof implementation that licensed formation is not API
// identity. `PackageReviewDataProperties` carries declaration multiplicity and
// movement policy without retaining a typed-tree declaration node.

use crate::record::{PackageReviewNominalIdentity, PackageReviewTypeIdentity};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewDataField {
    pub(crate) identity: Option<u64>,
    pub(crate) name: String,
    pub(crate) relevance: language_core::BindingRelevance,
    pub(crate) type_identity: PackageReviewTypeIdentity,
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
    pub(crate) multiplicity: language_semantics::Multiplicity,
    pub(crate) carry: Option<language_semantics::CarryPolicy>,
}

impl PackageReviewDataProperties {}
