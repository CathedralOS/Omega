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
    pub(crate) relevance: psi_language_core::BindingRelevance,
    pub(crate) type_identity: PackageReviewTypeIdentity,
}

impl PackageReviewDataField {
    pub const fn identity(&self) -> Option<u64> {
        self.identity
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn relevance(&self) -> psi_language_core::BindingRelevance {
        self.relevance
    }

    pub const fn type_identity(&self) -> &PackageReviewTypeIdentity {
        &self.type_identity
    }
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackageReviewDataProperties {
    pub(crate) multiplicity: psi_language_semantics::Multiplicity,
    pub(crate) carry: Option<psi_language_semantics::CarryPolicy>,
}

impl PackageReviewDataProperties {
    pub const fn multiplicity(&self) -> psi_language_semantics::Multiplicity {
        self.multiplicity
    }

    pub const fn carry(&self) -> Option<psi_language_semantics::CarryPolicy> {
        self.carry
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewDataShape {
    pub(crate) identity: PackageReviewNominalIdentity,
    pub(crate) kind: PackageReviewDataKind,
    pub(crate) supply: psi_language_semantics::DataSupplyMode,
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) type_parameters: Vec<PackageReviewTypeParameter>,
    pub(crate) properties: PackageReviewDataProperties,
    pub(crate) zero_gated: bool,
    pub(crate) invariants: Vec<PackageReviewContractFact>,
    pub(crate) retired_identities: Vec<u64>,
    pub(crate) members: Vec<PackageReviewDataMember>,
}

impl PackageReviewDataShape {
    pub const fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }

    pub const fn kind(&self) -> &PackageReviewDataKind {
        &self.kind
    }

    pub const fn supply(&self) -> psi_language_semantics::DataSupplyMode {
        self.supply
    }

    pub const fn lifetime_parameter_count(&self) -> usize {
        self.lifetime_parameter_count
    }

    pub fn type_parameters(&self) -> &[PackageReviewTypeParameter] {
        &self.type_parameters
    }

    pub const fn properties(&self) -> PackageReviewDataProperties {
        self.properties
    }

    pub const fn zero_gated(&self) -> bool {
        self.zero_gated
    }

    pub fn invariants(&self) -> &[PackageReviewContractFact] {
        &self.invariants
    }

    pub fn retired_identities(&self) -> &[u64] {
        &self.retired_identities
    }

    pub fn members(&self) -> &[PackageReviewDataMember] {
        &self.members
    }
}
