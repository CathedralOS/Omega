//! One-shot grants that mint a root `Extent`, with the geometry check that
//! precedes consumption and the existing-content evidence a provider root may
//! carry.

use crate::extent::diagnostic::{ExtentDiagnostic, validate_range};
use crate::extent::{Extent, ExtentSharingMode, Lineage};
use crate::identities::{
    AddressSpaceId, ExtentContentCustodyReceiptId, ExtentContentInterpretation,
    ExtentContentValidityReceiptId, ExtentLineageId, ExtentProvenanceId, ExtentRights,
    MappingEraId, ResidentClaimId,
};
use crate::roots::root_origins::{
    ExtentProgramLocalOrigin, ExtentProviderIssuance, ExtentRootOrigin,
};

/// One-shot authority to mint exactly one root extent.
///
/// Compiler code constructs this from one of the two exact root origins.
/// Omega source never receives a constructor for either this grant or `Extent`
/// itself. Program-local callers must additionally retain the exact installed
/// occurrence account; this grant carries only its passive origin descriptor.
#[derive(Debug, PartialEq, Eq)]
pub struct ExtentRootGrant {
    origin: ExtentRootOrigin,
    lineage: ExtentLineageId,
    address_space: AddressSpaceId,
    rights: ExtentRights,
    provenance: ExtentProvenanceId,
    era: MappingEraId,
    sharing: ExtentSharingMode,
}

/// One-shot provider evidence that an exact freshly introduced Extent already
/// contains valid content under one normalized interpretation and that
/// custody of that content transfers with the Extent.
///
/// This grant is deliberately non-`Clone` and can only be emitted while
/// consuming a provider-issued [`ExtentRootGrant`]. Program-local
/// capacity cannot acquire provider content validity through this route.
#[derive(Debug, PartialEq, Eq)]
pub struct ProviderExistingContentGrant {
    origin: ExtentRootOrigin,
    lineage_root: ExtentLineageId,
    base: u64,
    length: u64,
    address_space: AddressSpaceId,
    provenance: ExtentProvenanceId,
    era: MappingEraId,
    interpretation: ExtentContentInterpretation,
    resident_claim: ResidentClaimId,
    validity_receipt: ExtentContentValidityReceiptId,
    custody_receipt: ExtentContentCustodyReceiptId,
}

impl ProviderExistingContentGrant {
    pub const fn origin(&self) -> ExtentRootOrigin {
        self.origin
    }

    pub const fn lineage_root(&self) -> ExtentLineageId {
        self.lineage_root
    }

    pub const fn base(&self) -> u64 {
        self.base
    }

    pub const fn length(&self) -> u64 {
        self.length
    }

    pub const fn address_space(&self) -> AddressSpaceId {
        self.address_space
    }

    pub const fn provenance(&self) -> ExtentProvenanceId {
        self.provenance
    }

    pub const fn era(&self) -> MappingEraId {
        self.era
    }

    pub const fn interpretation(&self) -> ExtentContentInterpretation {
        self.interpretation
    }

    pub const fn resident_claim(&self) -> ResidentClaimId {
        self.resident_claim
    }

    pub const fn validity_receipt(&self) -> ExtentContentValidityReceiptId {
        self.validity_receipt
    }

    pub const fn custody_receipt(&self) -> ExtentContentCustodyReceiptId {
        self.custody_receipt
    }
}

/// Runtime geometry whose source-language `no_wrap(base, length)` predicate
/// has already been checked in proof-level (non-wrapping) arithmetic.
///
/// Keeping this as a separate value lets an entry installer validate every
/// inbound range before it imports any complete `Extent::Granted` fact.  It
/// carries geometry only; authority still comes exclusively from consuming an
/// admitted [`ExtentRootGrant`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatedExtentGeometry {
    base: u64,
    length: u64,
}

impl ValidatedExtentGeometry {
    pub fn check(base: u64, length: u64) -> Result<Self, ExtentDiagnostic> {
        validate_range(base, length)?;
        Ok(Self { base, length })
    }

    pub const fn base(self) -> u64 {
        self.base
    }

    pub const fn length(self) -> u64 {
        self.length
    }
}

impl ExtentRootGrant {
    pub const fn from_admitted_provider(
        provider_issuance: ExtentProviderIssuance,
        lineage: ExtentLineageId,
        address_space: AddressSpaceId,
        rights: ExtentRights,
        provenance: ExtentProvenanceId,
        era: MappingEraId,
    ) -> Self {
        Self {
            origin: ExtentRootOrigin::ProviderIssued(provider_issuance),
            lineage,
            address_space,
            rights,
            provenance,
            era,
            sharing: ExtentSharingMode::Exclusive,
        }
    }

    /// Admit that this root's backing may carry writable peer aliases the
    /// checker cannot see. The minted extent and every descendant refuse
    /// exclusive borrows: a writable peer cannot be wished into an exclusive
    /// borrow, and shared reads remain the consumer's copy-and-validate
    /// responsibility.
    pub const fn admitting_peer_shared_backing(mut self) -> Self {
        self.sharing = ExtentSharingMode::PeerShared;
        self
    }

    #[doc(hidden)]
    pub const fn from_established_program_local(
        origin: ExtentProgramLocalOrigin,
        lineage: ExtentLineageId,
        address_space: AddressSpaceId,
        rights: ExtentRights,
        provenance: ExtentProvenanceId,
        era: MappingEraId,
    ) -> Self {
        Self {
            origin: ExtentRootOrigin::ProgramLocal(origin),
            lineage,
            address_space,
            rights,
            provenance,
            era,
            sharing: ExtentSharingMode::Exclusive,
        }
    }

    pub const fn origin(&self) -> ExtentRootOrigin {
        self.origin
    }

    /// Report-only identity of the root lineage this one-shot grant will mint.
    /// Observing it does not duplicate or consume the grant.
    pub const fn lineage_root(&self) -> ExtentLineageId {
        self.lineage
    }

    pub fn mint(self, base: u64, length: u64) -> Result<Extent, MintError> {
        let geometry = match ValidatedExtentGeometry::check(base, length) {
            Ok(geometry) => geometry,
            Err(diagnostic) => {
                return Err(MintError {
                    grant: self,
                    diagnostic,
                });
            }
        };
        Ok(self.mint_validated(geometry))
    }

    /// Consume one provider-issued root grant into both its exact Extent and
    /// one-shot existing-content authority.
    ///
    /// The interpretation is provider-admitted input here; its consumer must
    /// compare both its report fingerprint and strong commitment with the
    /// actual normalized placement selected for the Extent. Failure returns
    /// the complete root grant for retry.
    pub fn mint_provider_existing_content(
        self,
        base: u64,
        length: u64,
        interpretation: ExtentContentInterpretation,
        resident_claim: ResidentClaimId,
        validity_receipt: ExtentContentValidityReceiptId,
        custody_receipt: ExtentContentCustodyReceiptId,
    ) -> Result<(Extent, ProviderExistingContentGrant), ExistingContentMintError> {
        let geometry = match ValidatedExtentGeometry::check(base, length) {
            Ok(geometry) => geometry,
            Err(diagnostic) => {
                return Err(ExistingContentMintError {
                    grant: self,
                    diagnostic,
                });
            }
        };
        if !matches!(self.origin, ExtentRootOrigin::ProviderIssued(_)) {
            return Err(ExistingContentMintError {
                grant: self,
                diagnostic: ExtentDiagnostic(
                    "program-local roots cannot issue provider existing-content evidence".into(),
                ),
            });
        }
        let content = ProviderExistingContentGrant {
            origin: self.origin,
            lineage_root: self.lineage,
            base: geometry.base,
            length: geometry.length,
            address_space: self.address_space,
            provenance: self.provenance,
            era: self.era,
            interpretation,
            resident_claim,
            validity_receipt,
            custody_receipt,
        };
        Ok((self.mint_validated(geometry), content))
    }

    /// Consume admitted authority for geometry whose `no_wrap` obligation was
    /// checked before this operation began.
    pub fn mint_validated(self, geometry: ValidatedExtentGeometry) -> Extent {
        let Self {
            origin,
            lineage,
            address_space,
            rights,
            provenance,
            era,
            sharing,
        } = self;
        Extent {
            base: geometry.base,
            length: geometry.length,
            address_space,
            rights,
            provenance,
            era,
            origin,
            lineage: Lineage {
                root: lineage,
                path: Vec::new(),
            },
            sharing,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct MintError {
    grant: ExtentRootGrant,
    diagnostic: ExtentDiagnostic,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ExistingContentMintError {
    grant: ExtentRootGrant,
    diagnostic: ExtentDiagnostic,
}

impl ExistingContentMintError {
    pub const fn diagnostic(&self) -> &ExtentDiagnostic {
        &self.diagnostic
    }

    pub fn into_grant(self) -> ExtentRootGrant {
        self.grant
    }
}

impl MintError {
    pub const fn diagnostic(&self) -> &ExtentDiagnostic {
        &self.diagnostic
    }

    pub fn into_grant(self) -> ExtentRootGrant {
        self.grant
    }
}
