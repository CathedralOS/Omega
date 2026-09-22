//! Normalized identities and the open rights set every extent record carries.
//!
//! Every identity is a nonzero `u64` admitted through one constructor, so a
//! zero column can never pass for a real provenance, era, lineage, or right.

use std::collections::BTreeSet;

macro_rules! normalized_extent_identity {
    ($(#[$meta:meta])* $name:ident, $label:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            pub fn from_normalized_identity(
                identity: u64,
            ) -> Result<Self, $crate::extent::diagnostic::ExtentDiagnostic> {
                $crate::extent::diagnostic::nonzero_identity(identity, $label)?;
                Ok(Self(identity))
            }

            pub const fn normalized_identity(self) -> u64 {
                self.0
            }
        }
    };
}

pub(crate) use normalized_extent_identity;

normalized_extent_identity!(AddressSpaceId, "address-space");

normalized_extent_identity!(ExtentProvenanceId, "extent-provenance");

normalized_extent_identity!(MappingEraId, "mapping-era");

normalized_extent_identity!(ExtentLineageId, "extent-lineage");

normalized_extent_identity!(ExtentIssuanceId, "extent-issuance");

normalized_extent_identity!(ExtentBackingId, "extent-backing");

normalized_extent_identity!(ExtentProviderId, "extent-provider");

normalized_extent_identity!(ExtentLiveIssuancePremiseId, "extent-live-issuance-premise");

normalized_extent_identity!(ExtentCustodyRootId, "extent-custody-root");

normalized_extent_identity!(ExtentAliasClassId, "extent-alias-class");

normalized_extent_identity!(
    ExtentProviderCorrespondenceId,
    "extent-provider-correspondence"
);

normalized_extent_identity!(ExtentTrustProvenanceId, "extent-trust-provenance");

normalized_extent_identity!(ExtentProviderPlanId, "extent-provider-plan");

normalized_extent_identity!(ExtentProviderInvocationId, "extent-provider-invocation");

normalized_extent_identity!(ExtentEstablishmentRouteId, "extent-establishment-route");

normalized_extent_identity!(ExtentCapacityId, "extent-capacity");

normalized_extent_identity!(ExtentQualificationId, "extent-qualification");

normalized_extent_identity!(
    ExtentContentInterpretationId,
    "extent-content-interpretation"
);

normalized_extent_identity!(
    ExtentContentValidityReceiptId,
    "extent-content-validity-receipt"
);

normalized_extent_identity!(
    ExtentContentCustodyReceiptId,
    "extent-content-custody-receipt"
);

normalized_extent_identity!(ResidentClaimId, "resident-claim");

/// Exact semantic interpretation selected for provider-existing content.
///
/// The compact fingerprint remains useful for compatibility reporting, but
/// consumers must also rejoin the collision-resistant commitment before the
/// provider's content-validity evidence can be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExtentContentInterpretation {
    compatibility_fingerprint: ExtentContentInterpretationId,
    commitment: [u8; 32],
}

impl ExtentContentInterpretation {
    pub const fn from_sha256_commitment(
        compatibility_fingerprint: ExtentContentInterpretationId,
        commitment: [u8; 32],
    ) -> Self {
        Self {
            compatibility_fingerprint,
            commitment,
        }
    }

    pub const fn compatibility_fingerprint(self) -> ExtentContentInterpretationId {
        self.compatibility_fingerprint
    }

    pub const fn commitment(self) -> [u8; 32] {
        self.commitment
    }
}

normalized_extent_identity!(ExtentRightId, "extent-right");

/// An open, normalized set of grant-established rights.
///
/// The compiler does not bless a READ/WRITE/EXECUTE enumeration here. Target
/// and provider packages define right identities; admission controls which
/// sets may enter a root grant.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExtentRights(BTreeSet<ExtentRightId>);

impl ExtentRights {
    pub const fn none() -> Self {
        Self(BTreeSet::new())
    }

    pub fn from_normalized_identities(rights: impl IntoIterator<Item = ExtentRightId>) -> Self {
        Self(rights.into_iter().collect())
    }

    pub fn union(&self, other: &Self) -> Self {
        Self(self.0.union(&other.0).copied().collect())
    }

    pub fn contains(&self, required: &Self) -> bool {
        required.0.is_subset(&self.0)
    }

    pub fn identities(&self) -> impl Iterator<Item = ExtentRightId> + '_ {
        self.0.iter().copied()
    }
}
