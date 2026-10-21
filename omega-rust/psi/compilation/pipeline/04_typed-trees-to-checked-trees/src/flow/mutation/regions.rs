//! A mutation footprint is not always storage inside its loan anchor.
//!
//! Resident views lend separately owned content, not the bytes of the Extent
//! descriptor. Keep that boundary through summary substitution and completion:
//! a content store invalidates observations but does not replace the owner.
//! These are may-write coordinates, never evidence establishing a resident.
//! Formation provenance comes from the captured, checked view operation; its
//! type and numeric address cannot manufacture a content region.

use validation::CanonicalPlace;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StorageRegion {
    Declared(CanonicalPlace),
    /// The carrier may hold externally writable capability storage whose
    /// backing cannot be enumerated. Its anchor is not that backing. This is
    /// a complete conservative effect (invalidate unknown observations), not
    /// authority to replace any additional declared ownership home.
    UnresolvedContent {
        carrier: CanonicalPlace,
    },
    ResidentContent {
        owner: CanonicalPlace,
        formation: typed_trees::expression::ExpressionHandle,
        granted: language_semantics::SemanticDomainId,
        resident: language_semantics::SemanticDomainId,
    },
}

impl StorageRegion {
    /// The declaration through which this effect is reached. For content this
    /// is a loan anchor, not an alternative spelling of the written bytes.
    pub(crate) fn anchor(&self) -> &CanonicalPlace {
        match self {
            Self::Declared(place) => place,
            Self::ResidentContent { owner, .. } => owner,
            Self::UnresolvedContent { carrier } => carrier,
        }
    }

    pub(crate) fn into_declared(self) -> Option<CanonicalPlace> {
        match self {
            Self::Declared(place) => Some(place),
            Self::ResidentContent { .. } | Self::UnresolvedContent { .. } => None,
        }
    }

    /// A consumer confined to declared value coordinates must retain unknown
    /// effects when another address space is present, never silently omit them.
    pub(crate) fn declared_places(
        regions: impl IntoIterator<Item = Self>,
    ) -> Option<Vec<CanonicalPlace>> {
        regions.into_iter().map(Self::into_declared).collect()
    }

    pub(crate) fn same_storage_space(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::Declared(_), Self::Declared(_))
                | (Self::ResidentContent { .. }, Self::ResidentContent { .. })
        )
    }

    pub(crate) fn contains(&self, other: &Self) -> bool {
        self.same_storage_space(other)
            && self.anchor().root == other.anchor().root
            && other.anchor().segments.starts_with(&self.anchor().segments)
    }

    /// Ordinary normal-return writes through a borrowed resident preserve its
    /// captured owner's qualifications. This only retains an existing fact at
    /// that exact owner: it neither issues a qualification nor restores content
    /// observations. Borrowed-content checking must still restore every hole.
    pub(crate) fn preserves_owner_qualification(
        &self,
        program: &dyn validation::CheckingInput,
        semantic: &facts::FactPlan,
        fact: &facts::Fact,
        place: facts::PlaceHandle,
    ) -> bool {
        let Self::ResidentContent {
            owner,
            granted,
            resident,
            ..
        } = self
        else {
            return false;
        };
        let domain = match fact.payload {
            facts::FactPayload::DomainMembership {
                semantic_domain, ..
            }
            | facts::FactPayload::ContractDomainMembership {
                semantic_domain, ..
            } => semantic_domain,
            _ => return false,
        };
        if domain != *granted && domain != *resident {
            return false;
        }
        crate::flow::canonical_place_from_semantic_place(
            program,
            semantic,
            semantic.places.get(place),
        )
        .is_some_and(|place| place == *owner)
    }
}
