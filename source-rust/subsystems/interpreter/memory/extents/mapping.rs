/*
We govern address-space translation grants (`MappingGrant`), pending map and
unmap transitions (`PendingMap`, `PendingUnmap`), peer-write revocation
receipts (`PeerWriteRevocationReceipt`), and authority conservation across
mapped and unmapped extents.
*/

use crate::extents::extent::Extent;
use crate::extents::extent::diagnostic::ExtentDiagnostic;

use crate::extents::loans::ExtentLoan;
use crate::extents::roots::root_grants::ValidatedExtentGeometry;
use crate::extents::roots::root_origins::ExtentRootOrigin;
use arena::NormalizedIdentitySet;
use extent_contracts::LoanPolarity;
use extent_contracts::MappingGrantId;
use extent_contracts::MappingId;
use extent_contracts::{
    AddressSpaceId, ExtentLineageId, ExtentProvenanceId, ExtentRights, MappingEraId,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingSourceMode {
    Owned,
    BorrowedShared,
    BorrowedExclusive,
}

use extent_contracts::TranslationActivationFactId;
pub type TranslationInstallObligations = NormalizedIdentitySet<TranslationActivationFactId>;
use extent_contracts::TranslationCompletionFactId;
pub type TranslationReleaseObligations = NormalizedIdentitySet<TranslationCompletionFactId>;
use extent_contracts::PeerWriteRevocationFactId;
pub type PeerWriteRevocationObligations = NormalizedIdentitySet<PeerWriteRevocationFactId>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappingGrant {
    identity: MappingGrantId,
    source_mode: MappingSourceMode,
    source_space: AddressSpaceId,
    destination_space: AddressSpaceId,
    required_source_rights: ExtentRights,
    required_destination_rights: ExtentRights,
    mapped_rights: ExtentRights,
    mapped_provenance: ExtentProvenanceId,
    mapped_era: MappingEraId,
    map_obligations: TranslationInstallObligations,
    unmap_obligations: TranslationReleaseObligations,
    peer_revocation_obligations: PeerWriteRevocationObligations,
}

impl MappingGrant {
    #[allow(clippy::too_many_arguments)]
    pub fn from_admitted_provider(
        identity: MappingGrantId,
        source_mode: MappingSourceMode,
        source_space: AddressSpaceId,
        destination_space: AddressSpaceId,
        required_source_rights: ExtentRights,
        required_destination_rights: ExtentRights,
        mapped_rights: ExtentRights,
        mapped_provenance: ExtentProvenanceId,
        mapped_era: MappingEraId,
        map_obligations: TranslationInstallObligations,
        unmap_obligations: TranslationReleaseObligations,
        peer_revocation_obligations: PeerWriteRevocationObligations,
    ) -> Self {
        Self {
            identity,
            source_mode,
            source_space,
            destination_space,
            required_source_rights,
            required_destination_rights,
            mapped_rights,
            mapped_provenance,
            mapped_era,
            map_obligations,
            unmap_obligations,
            peer_revocation_obligations,
        }
    }

    get_fields!(copy &self; identity: MappingGrantId);
    get_fields!(ref &self;
        install_obligations(map_obligations): TranslationInstallObligations,
        release_obligations(unmap_obligations): TranslationReleaseObligations,
        peer_revocation_obligations: PeerWriteRevocationObligations,
    );
}

#[derive(Debug)]
enum MappingSource<'source> {
    Owned(Extent),
    Borrowed(ExtentLoan<'source>),
}

macro_rules! delegate_mapping_source {
    ($($name:ident: $ret:ty),* $(,)?) => {
        $(fn $name(&self) -> $ret {
            match self {
                Self::Owned(extent) => extent.$name(),
                Self::Borrowed(loan) => loan.$name(),
            }
        })*
    };
}

impl MappingSource<'_> {
    delegate_mapping_source!(
        base: u64,
        length: u64,
        address_space: AddressSpaceId,
        rights: &ExtentRights,
        provenance: ExtentProvenanceId,
        era: MappingEraId,
        origin: ExtentRootOrigin,
        lineage_root: ExtentLineageId,
    );

    fn mode(&self) -> MappingSourceMode {
        match self {
            Self::Owned(_) => MappingSourceMode::Owned,
            Self::Borrowed(loan) => match loan.polarity() {
                LoanPolarity::Shared => MappingSourceMode::BorrowedShared,
                LoanPolarity::Exclusive => MappingSourceMode::BorrowedExclusive,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DestinationRestoration {
    rights: ExtentRights,
    provenance: ExtentProvenanceId,
    era: MappingEraId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MappingEvidence {
    identity: MappingId,
    grant: MappingGrant,
    source_mode: MappingSourceMode,
    source_base: u64,
    source_length: u64,
    source_space: AddressSpaceId,
    source_rights: ExtentRights,
    source_provenance: ExtentProvenanceId,
    source_era: MappingEraId,
    source_origin: ExtentRootOrigin,
    source_lineage: ExtentLineageId,
    mapped_base: u64,
    mapped_length: u64,
    mapped_space: AddressSpaceId,
    mapped_rights: ExtentRights,
    mapped_provenance: ExtentProvenanceId,
    mapped_era: MappingEraId,
    mapped_origin: ExtentRootOrigin,
    mapped_lineage: ExtentLineageId,
    destination: DestinationRestoration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappingReceiptContext(MappingEvidence);

impl MappingReceiptContext {
    pub const fn install_obligations(&self) -> &TranslationInstallObligations {
        &self.0.grant.map_obligations
    }

    pub const fn release_obligations(&self) -> &TranslationReleaseObligations {
        &self.0.grant.unmap_obligations
    }

    pub const fn peer_revocation_obligations(&self) -> &PeerWriteRevocationObligations {
        &self.0.grant.peer_revocation_obligations
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct MappingTransitionReceipt<FactId> {
    mapping: MappingEvidence,
    established: bool,
    established_facts: NormalizedIdentitySet<FactId>,
}

impl<FactId: Copy + Ord> MappingTransitionReceipt<FactId> {
    pub fn from_admitted_provider(
        context: &MappingReceiptContext,
        established: bool,
        established_facts: impl IntoIterator<Item = FactId>,
    ) -> Self {
        Self {
            mapping: context.0.clone(),
            established,
            established_facts: NormalizedIdentitySet::from_normalized_identities(established_facts),
        }
    }

    fn verify(
        &self,
        mapping: &MappingEvidence,
        obligations: &NormalizedIdentitySet<FactId>,
        wording: &TransitionWording,
    ) -> Result<(), ExtentDiagnostic> {
        let refusal = if &self.mapping != mapping {
            format!(
                "{} receipt does not bind the exact {} mapping",
                wording.transition, wording.state
            )
        } else if !self.established {
            format!(
                "{} receipt does not {}",
                wording.transition, wording.establishes
            )
        } else if !self.established_facts.contains(obligations) {
            format!(
                "{} receipt lacks required {} facts",
                wording.transition, wording.facts
            )
        } else {
            return Ok(());
        };
        Err(ExtentDiagnostic(refusal))
    }
}

struct TransitionWording {
    transition: &'static str,
    state: &'static str,
    establishes: &'static str,
    facts: &'static str,
}

const ACTIVATION_WORDING: TransitionWording = TransitionWording {
    transition: "translation-activation",
    state: "pending",
    establishes: "establish installed translations",
    facts: "installation",
};

const REVOCATION_WORDING: TransitionWording = TransitionWording {
    transition: "peer-write-revocation",
    state: "active",
    establishes: "establish the revoked write permission",
    facts: "invalidation",
};

const RELEASE_WORDING: TransitionWording = TransitionWording {
    transition: "translation-release",
    state: "pending",
    establishes: "release stale translations",
    facts: "completion",
};

pub type TranslationActivationReceipt = MappingTransitionReceipt<TranslationActivationFactId>;
pub type PeerWriteRevocationReceipt = MappingTransitionReceipt<PeerWriteRevocationFactId>;
pub type TranslationReleaseReceipt = MappingTransitionReceipt<TranslationCompletionFactId>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappedRangeReceiptContext {
    mapping: MappingEvidence,
    range: ValidatedExtentGeometry,
}

#[derive(Debug)]
pub struct MappedExtent<'source> {
    evidence: MappingEvidence,
    mapped: Extent,
    source: MappingSource<'source>,
    peer_write_revoked: bool,
}

impl PartialEq for MappedExtent<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.evidence == other.evidence
    }
}

impl Eq for MappedExtent<'_> {}

#[derive(Debug)]
pub struct PendingMap<'source> {
    mapping: MappedExtent<'source>,
}

impl<'source> PendingMap<'source> {
    delegate_methods!(const pub mapping { mapping => identity: MappingId, grant: MappingGrantId });
    delegate_methods!(const pub mapping.evidence.grant { install_obligations: &TranslationInstallObligations });
    delegate_methods!(pub mapping { receipt_context: MappingReceiptContext });
    delegate_methods!(pub mapping.source {
        source_base => base: u64,
        source_length => length: u64,
        source_rights => rights: &ExtentRights,
        source_provenance => provenance: ExtentProvenanceId,
        source_era => era: MappingEraId,
        source_origin => origin: ExtentRootOrigin,
        source_mode => mode: MappingSourceMode,
    });
    delegate_methods!(pub mapping.mapped {
        destination_base => base: u64,
        mapped_rights => rights: &ExtentRights,
        mapped_origin => origin: ExtentRootOrigin,
        mapped_provenance => provenance: ExtentProvenanceId,
        mapped_era => era: MappingEraId,
    });

    pub fn complete(
        self,
        receipt: TranslationActivationReceipt,
    ) -> Result<MappedExtent<'source>, Box<MapActivationError<'source>>> {
        if let Err(diagnostic) = receipt.verify(
            &self.mapping.evidence,
            self.install_obligations(),
            &ACTIVATION_WORDING,
        ) {
            return Err(Box::new(MapActivationError {
                pending: self,
                receipt,
                diagnostic,
            }));
        }

        Ok(self.mapping)
    }
}

conserved_error!(
    #[derive(Debug)]
    pub struct MapActivationError<'source> {
        pending: PendingMap<'source>,
        receipt: TranslationActivationReceipt,
    } => into_parts
);

impl<'source> MappedExtent<'source> {
    pub const fn identity(&self) -> MappingId {
        self.evidence.identity
    }

    delegate_methods!(const pub evidence.grant {
        grant => identity: MappingGrantId,
        release_obligations: &TranslationReleaseObligations,
    });
    delegate_methods!(const pub mapped {
        base: u64,
        length: u64,
        rights: &ExtentRights,
        address_space: AddressSpaceId,
        provenance: ExtentProvenanceId,
        era: MappingEraId,
        origin: ExtentRootOrigin,
        lineage_root: ExtentLineageId,
    });

    pub fn receipt_context(&self) -> MappingReceiptContext {
        MappingReceiptContext(self.evidence.clone())
    }

    pub fn range_receipt_context(
        &self,
        offset: u64,
        length: u64,
    ) -> Result<MappedRangeReceiptContext, ExtentDiagnostic> {
        if length == 0 {
            return Err(ExtentDiagnostic(
                "mapped receipt range cannot be empty".into(),
            ));
        }
        let end = offset
            .checked_add(length)
            .ok_or_else(|| ExtentDiagnostic("mapped receipt range overflows".into()))?;
        if end > self.mapped.length() {
            return Err(ExtentDiagnostic(format!(
                "mapped receipt range {offset}..{end} exceeds {}-byte mapping",
                self.mapped.length()
            )));
        }
        let base = self
            .mapped
            .base()
            .checked_add(offset)
            .ok_or_else(|| ExtentDiagnostic("mapped receipt base overflows".into()))?;
        let range = ValidatedExtentGeometry::check(base, length)?;
        Ok(MappedRangeReceiptContext {
            mapping: self.evidence.clone(),
            range,
        })
    }

    pub fn loan(&self, offset: u64, length: u64) -> Result<ExtentLoan<'_>, ExtentDiagnostic> {
        self.mapped.loan(offset, length)
    }

    pub fn loan_mut(
        &mut self,
        offset: u64,
        length: u64,
    ) -> Result<ExtentLoan<'_>, ExtentDiagnostic> {
        if matches!(self.source.mode(), MappingSourceMode::BorrowedShared) {
            return Err(ExtentDiagnostic(
                "a mapping with shared source custody cannot expose mutable access".into(),
            ));
        }
        self.mapped.loan_mut(offset, length)
    }

    pub fn stable_loan(
        &self,
        offset: u64,
        length: u64,
    ) -> Result<ExtentLoan<'_>, ExtentDiagnostic> {
        if matches!(self.source.mode(), MappingSourceMode::BorrowedShared)
            && !self.peer_write_revoked
        {
            return Err(ExtentDiagnostic(
                "a shared-custody mapping cannot expose a stable view until a peer-write-revocation receipt completes".into(),
            ));
        }
        self.mapped.loan(offset, length)
    }

    pub fn begin_peer_write_revocation(
        self,
    ) -> Result<PendingPeerWriteRevocation<'source>, Box<PeerWriteRevocationStartError<'source>>>
    {
        if !matches!(self.source.mode(), MappingSourceMode::BorrowedShared) {
            return Err(Box::new(PeerWriteRevocationStartError {
                mapping: self,
                diagnostic: ExtentDiagnostic(
                    "peer-write revocation applies only to shared source custody".into(),
                ),
            }));
        }
        Ok(PendingPeerWriteRevocation { mapping: self })
    }

    pub fn begin_unmap(self) -> PendingUnmap<'source> {
        PendingUnmap { mapping: self }
    }
}

#[derive(Debug)]
pub struct PendingPeerWriteRevocation<'source> {
    mapping: MappedExtent<'source>,
}

impl<'source> PendingPeerWriteRevocation<'source> {
    delegate_methods!(const pub mapping { mapping => identity: MappingId, grant: MappingGrantId });
    delegate_methods!(pub mapping { receipt_context: MappingReceiptContext });
    delegate_methods!(const pub mapping.evidence.grant {
        revocation_obligations => peer_revocation_obligations: &PeerWriteRevocationObligations,
    });

    pub fn complete(
        self,
        receipt: PeerWriteRevocationReceipt,
    ) -> Result<MappedExtent<'source>, Box<PeerWriteRevocationError<'source>>> {
        if let Err(diagnostic) = receipt.verify(
            &self.mapping.evidence,
            self.revocation_obligations(),
            &REVOCATION_WORDING,
        ) {
            return Err(Box::new(PeerWriteRevocationError {
                pending: self,
                receipt,
                diagnostic,
            }));
        }
        let mut mapping = self.mapping;
        mapping.peer_write_revoked = true;
        Ok(mapping)
    }
}

conserved_error!(
    #[derive(Debug)]
    pub struct PeerWriteRevocationStartError<'source> {
        mapping: MappedExtent<'source>,
    } => into_mapping
);

conserved_error!(
    #[derive(Debug)]
    pub struct PeerWriteRevocationError<'source> {
        pending: PendingPeerWriteRevocation<'source>,
        receipt: PeerWriteRevocationReceipt,
    } => into_parts
);

pub fn map_owned(
    source: Extent,
    destination: Extent,
    identity: MappingId,
    grant: &MappingGrant,
) -> Result<PendingMap<'static>, Box<OwnedMappingError>> {
    if grant.source_mode != MappingSourceMode::Owned {
        return Err(Box::new(OwnedMappingError {
            source,
            destination,
            diagnostic: ExtentDiagnostic(
                "mapping grant does not admit owned source custody".into(),
            ),
        }));
    }
    map_with_source(MappingSource::Owned(source), destination, identity, grant).map_err(|error| {
        let MappingStartError {
            source,
            destination,
            diagnostic,
        } = *error;
        let MappingSource::Owned(source) = source else {
            unreachable!("owned mapping preserves source mode")
        };
        Box::new(OwnedMappingError {
            source,
            destination,
            diagnostic,
        })
    })
}

pub fn map_borrowed<'source>(
    source: ExtentLoan<'source>,
    destination: Extent,
    identity: MappingId,
    grant: &MappingGrant,
) -> Result<PendingMap<'source>, Box<BorrowedMappingError<'source>>> {
    map_with_source(
        MappingSource::Borrowed(source),
        destination,
        identity,
        grant,
    )
    .map_err(|error| {
        let MappingStartError {
            source,
            destination,
            diagnostic,
        } = *error;
        let MappingSource::Borrowed(source) = source else {
            unreachable!("borrowed mapping preserves source mode")
        };
        Box::new(BorrowedMappingError {
            source,
            destination,
            diagnostic,
        })
    })
}

fn map_with_source<'source>(
    source: MappingSource<'source>,
    destination: Extent,
    identity: MappingId,
    grant: &MappingGrant,
) -> Result<PendingMap<'source>, Box<MappingStartError<'source>>> {
    let mismatch = first_mismatch!(
        source.mode() != grant.source_mode
            => "source custody does not match mapping grant",
        source.address_space() != grant.source_space
            => "source address space does not match mapping grant",
        destination.address_space() != grant.destination_space
            => "destination address space does not match mapping grant",
        !source.rights().contains(&grant.required_source_rights)
            => "source lacks rights required by mapping grant",
        !destination.rights().contains(&grant.required_destination_rights)
            => "destination lacks rights required by mapping grant",
        source.length() != destination.length()
            => "source and destination mapping ranges must have equal length",
    );

    if let Some(message) = mismatch {
        return Err(Box::new(MappingStartError {
            source,
            destination,
            diagnostic: ExtentDiagnostic(message.into()),
        }));
    }

    let destination_restoration = DestinationRestoration {
        rights: destination.rights,
        provenance: destination.provenance,
        era: destination.era,
    };
    let mapped = Extent {
        rights: grant.mapped_rights.clone(),
        provenance: grant.mapped_provenance,
        era: grant.mapped_era,
        ..destination
    };
    let evidence = MappingEvidence {
        identity,
        grant: grant.clone(),
        source_mode: source.mode(),
        source_base: source.base(),
        source_length: source.length(),
        source_space: source.address_space(),
        source_rights: source.rights().clone(),
        source_provenance: source.provenance(),
        source_era: source.era(),
        source_origin: source.origin(),
        source_lineage: source.lineage_root(),
        mapped_base: mapped.base(),
        mapped_length: mapped.length(),
        mapped_space: mapped.address_space(),
        mapped_rights: mapped.rights().clone(),
        mapped_provenance: mapped.provenance(),
        mapped_era: mapped.era(),
        mapped_origin: mapped.origin(),
        mapped_lineage: mapped.lineage_root(),
        destination: destination_restoration,
    };
    Ok(PendingMap {
        mapping: MappedExtent {
            evidence,
            mapped,
            source,
            peer_write_revoked: false,
        },
    })
}

#[derive(Debug)]
struct MappingStartError<'source> {
    source: MappingSource<'source>,
    destination: Extent,
    diagnostic: ExtentDiagnostic,
}

conserved_error!(
    #[derive(Debug)]
    pub struct OwnedMappingError { source: Extent, destination: Extent } => into_extents
);

conserved_error!(
    #[derive(Debug)]
    pub struct BorrowedMappingError<'source> {
        source: ExtentLoan<'source>,
        destination: Extent,
    } => into_parts
);

#[derive(Debug)]
pub struct PendingUnmap<'source> {
    mapping: MappedExtent<'source>,
}

impl<'source> PendingUnmap<'source> {
    delegate_methods!(const pub mapping {
        mapping => identity: MappingId,
        release_obligations: &TranslationReleaseObligations,
    });
    delegate_methods!(pub mapping { receipt_context: MappingReceiptContext });

    pub fn complete(
        self,
        receipt: TranslationReleaseReceipt,
    ) -> Result<UnmappedExtents, Box<UnmapCompletionError<'source>>> {
        if let Err(diagnostic) = receipt.verify(
            &self.mapping.evidence,
            self.release_obligations(),
            &RELEASE_WORDING,
        ) {
            return Err(Box::new(UnmapCompletionError {
                pending: self,
                receipt,
                diagnostic,
            }));
        }

        let MappedExtent {
            evidence,
            mapped,
            source,
            ..
        } = self.mapping;
        let destination = Extent {
            rights: evidence.destination.rights,
            provenance: evidence.destination.provenance,
            era: evidence.destination.era,
            ..mapped
        };
        let owned_source = match source {
            MappingSource::Owned(source) => Some(source),
            MappingSource::Borrowed(_loan) => None,
        };
        Ok(UnmappedExtents {
            destination,
            owned_source,
        })
    }
}

conserved_error!(
    #[derive(Debug)]
    pub struct UnmapCompletionError<'source> {
        pending: PendingUnmap<'source>,
        receipt: TranslationReleaseReceipt,
    } => into_parts
);

#[derive(Debug)]
pub struct UnmappedExtents {
    destination: Extent,
    owned_source: Option<Extent>,
}

impl UnmappedExtents {
    pub fn into_parts(self) -> (Extent, Option<Extent>) {
        (self.destination, self.owned_source)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AddressSpaceId, Extent, ExtentLineageId, ExtentProvenanceId, ExtentRootOrigin,
        MappingEraId, MappingGrant, MappingGrantId, MappingId, MappingSourceMode,
        PeerWriteRevocationFactId, PeerWriteRevocationObligations, PeerWriteRevocationReceipt,
        PendingMap, PendingUnmap, TranslationActivationFactId, TranslationActivationReceipt,
        TranslationCompletionFactId, TranslationInstallObligations, TranslationReleaseObligations,
        TranslationReleaseReceipt, map_borrowed, map_owned,
    };
    use crate::extents::ExtentRootGrant;
    use crate::extents::tests::{
        id, program_local_origin, provider_issuance, provider_issuance_for_invocation, rights,
    };

    fn extent(
        lineage: u64,
        base: u64,
        length: u64,
        space: u64,
        provenance: u64,
        extent_rights: &[u64],
    ) -> Extent {
        ExtentRootGrant::from_admitted_provider(
            provider_issuance(lineage),
            id(lineage, ExtentLineageId::from_normalized_identity),
            id(space, AddressSpaceId::from_normalized_identity),
            rights(extent_rights),
            id(provenance, ExtentProvenanceId::from_normalized_identity),
            id(30, MappingEraId::from_normalized_identity),
        )
        .mint(base, length)
        .expect("root extent")
    }

    fn local_extent(
        provision: u64,
        lineage: u64,
        base: u64,
        space: u64,
        provenance: u64,
        extent_rights: &[u64],
    ) -> Extent {
        ExtentRootGrant::from_established_program_local(
            program_local_origin(provision),
            id(lineage, ExtentLineageId::from_normalized_identity),
            id(space, AddressSpaceId::from_normalized_identity),
            rights(extent_rights),
            id(provenance, ExtentProvenanceId::from_normalized_identity),
            id(30, MappingEraId::from_normalized_identity),
        )
        .mint(base, 0x1000)
        .expect("local root extent")
    }

    fn translation_fact(identity: u64) -> TranslationCompletionFactId {
        id(
            identity,
            TranslationCompletionFactId::from_normalized_identity,
        )
    }

    fn activation_fact(identity: u64) -> TranslationActivationFactId {
        id(
            identity,
            TranslationActivationFactId::from_normalized_identity,
        )
    }

    fn mapping_id(identity: u64) -> MappingId {
        id(identity, MappingId::from_normalized_identity)
    }

    fn mapping_grant(mode: MappingSourceMode) -> MappingGrant {
        mapping_grant_with_revocation(mode, revocation_obligations())
    }

    fn mapping_grant_with_revocation(
        mode: MappingSourceMode,
        peer_revocation_obligations: PeerWriteRevocationObligations,
    ) -> MappingGrant {
        MappingGrant::from_admitted_provider(
            id(40, MappingGrantId::from_normalized_identity),
            mode,
            id(10, AddressSpaceId::from_normalized_identity),
            id(11, AddressSpaceId::from_normalized_identity),
            rights(&[100]),
            rights(&[200]),
            rights(&[300]),
            id(21, ExtentProvenanceId::from_normalized_identity),
            id(31, MappingEraId::from_normalized_identity),
            TranslationInstallObligations::from_normalized_identities([activation_fact(600)]),
            TranslationReleaseObligations::from_normalized_identities([translation_fact(700)]),
            peer_revocation_obligations,
        )
    }

    fn source() -> Extent {
        extent(1, 0x1000, 0x1000, 10, 20, &[100])
    }

    fn destination() -> Extent {
        extent(2, 0xffff_8000_0000_0000, 0x1000, 11, 22, &[200])
    }

    fn release(pending: &PendingUnmap<'_>) -> TranslationReleaseReceipt {
        TranslationReleaseReceipt::from_admitted_provider(
            &pending.receipt_context(),
            true,
            pending.release_obligations().identities(),
        )
    }

    fn activate(pending: &PendingMap<'_>) -> TranslationActivationReceipt {
        TranslationActivationReceipt::from_admitted_provider(
            &pending.receipt_context(),
            true,
            pending.install_obligations().identities(),
        )
    }

    fn revocation_facts(ids: &[u64]) -> Vec<PeerWriteRevocationFactId> {
        ids.iter()
            .map(|&n| id(n, PeerWriteRevocationFactId::from_normalized_identity))
            .collect()
    }

    fn revocation_receipt(
        context: &super::MappingReceiptContext,
        established: bool,
        facts: &[u64],
    ) -> PeerWriteRevocationReceipt {
        PeerWriteRevocationReceipt::from_admitted_provider(
            context,
            established,
            revocation_facts(facts),
        )
    }

    fn revocation_obligations() -> PeerWriteRevocationObligations {
        PeerWriteRevocationObligations::from_normalized_identities(revocation_facts(&[61, 62]))
    }

    fn activate_owned(
        src: Extent,
        dst: Extent,
        id_num: u64,
        grant: &MappingGrant,
    ) -> super::MappedExtent<'static> {
        let pending = map_owned(src, dst, mapping_id(id_num), grant).expect("pending mapping");
        let receipt = activate(&pending);
        pending.complete(receipt).expect("active mapping")
    }

    fn finish_unmap(mapping: super::MappedExtent<'_>) -> (Extent, Option<Extent>) {
        let pending = mapping.begin_unmap();
        let receipt = release(&pending);
        pending
            .complete(receipt)
            .expect("translations released")
            .into_parts()
    }

    fn activated_shared_mapping_with(
        source_extent: &'static Extent,
        mapping_identity: u64,
        grant: &MappingGrant,
    ) -> super::MappedExtent<'static> {
        let source_loan = source_extent.loan(0, 0x1000).expect("shared source");
        let pending = map_borrowed(
            source_loan,
            destination(),
            mapping_id(mapping_identity),
            grant,
        )
        .expect("borrowed map candidate");
        let receipt = activate(&pending);
        pending.complete(receipt).expect("translations installed")
    }

    fn activated_shared_mapping() -> super::MappedExtent<'static> {
        activated_shared_mapping_with(
            Box::leak(Box::new(source())),
            60,
            &mapping_grant(MappingSourceMode::BorrowedShared),
        )
    }

    #[test]
    fn owned_mapping_round_trips_both_authorities_after_translation_release() {
        let mut pending = map_owned(
            source(),
            destination(),
            mapping_id(50),
            &mapping_grant(MappingSourceMode::Owned),
        )
        .expect("owned map candidate");
        assert_eq!(
            pending.source_origin(),
            ExtentRootOrigin::ProviderIssued(provider_issuance(1))
        );
        assert_eq!(
            pending.mapped_origin(),
            ExtentRootOrigin::ProviderIssued(provider_issuance(2))
        );

        let rejected_activations: [(fn(&PendingMap<'_>) -> TranslationActivationReceipt, &str); 4] = [
            (
                |p| {
                    let mut r = TranslationActivationReceipt::from_admitted_provider(
                        &p.receipt_context(),
                        true,
                        [activation_fact(600)],
                    );
                    r.mapping.identity = mapping_id(999);
                    r
                },
                "exact pending mapping",
            ),
            (
                |p| {
                    let mut r = TranslationActivationReceipt::from_admitted_provider(
                        &p.receipt_context(),
                        true,
                        [activation_fact(600)],
                    );
                    r.mapping.source_origin =
                        ExtentRootOrigin::ProviderIssued(provider_issuance_for_invocation(1, 99));
                    r
                },
                "exact pending mapping",
            ),
            (
                |p| {
                    TranslationActivationReceipt::from_admitted_provider(
                        &p.receipt_context(),
                        false,
                        [activation_fact(600)],
                    )
                },
                "does not establish",
            ),
            (
                |p| {
                    TranslationActivationReceipt::from_admitted_provider(
                        &p.receipt_context(),
                        true,
                        [],
                    )
                },
                "lacks",
            ),
        ];
        for (make_receipt, expected) in rejected_activations {
            let receipt = make_receipt(&pending);
            let error = pending.complete(receipt).expect_err(expected);
            assert!(error.diagnostic().0.contains(expected));
            pending = (*error).into_parts().0;
        }

        let receipt = activate(&pending);
        let mapping = pending.complete(receipt).expect("translations installed");
        assert_eq!(mapping.rights(), &rights(&[300]));
        assert_eq!(
            mapping.origin(),
            ExtentRootOrigin::ProviderIssued(provider_issuance(2))
        );

        let pending = mapping.begin_unmap();
        let incomplete =
            TranslationReleaseReceipt::from_admitted_provider(&pending.receipt_context(), true, []);
        let error = pending
            .complete(incomplete)
            .expect_err("shootdown fact missing");
        assert!(error.diagnostic().0.contains("lacks"));
        let (pending, _) = (*error).into_parts();
        let receipt = release(&pending);
        let (destination, source) = pending
            .complete(receipt)
            .expect("translations released")
            .into_parts();
        assert_eq!(destination.rights(), &rights(&[200]));
        assert_eq!(
            destination.origin(),
            ExtentRootOrigin::ProviderIssued(provider_issuance(2))
        );
        let source = source.expect("owned source returned");
        assert_eq!(source.rights(), &rights(&[100]));
        assert_eq!(
            source.origin(),
            ExtentRootOrigin::ProviderIssued(provider_issuance(1))
        );
    }

    #[test]
    fn owned_mapping_round_trips_program_local_origins() {
        let pending = map_owned(
            local_extent(1, 11, 0x1000, 10, 20, &[100]),
            local_extent(2, 12, 0xffff_8000_0000_0000, 11, 22, &[200]),
            mapping_id(52),
            &mapping_grant(MappingSourceMode::Owned),
        )
        .expect("local owned map candidate");
        assert_eq!(
            pending.source_origin(),
            ExtentRootOrigin::ProgramLocal(program_local_origin(1))
        );
        assert_eq!(
            pending.mapped_origin(),
            ExtentRootOrigin::ProgramLocal(program_local_origin(2))
        );

        let receipt = activate(&pending);
        let mapping = pending.complete(receipt).expect("translations installed");
        let (destination, source) = finish_unmap(mapping);
        assert_eq!(
            destination.program_local_origin(),
            Some(program_local_origin(2))
        );
        assert_eq!(
            source
                .expect("owned local source returned")
                .program_local_origin(),
            Some(program_local_origin(1))
        );
    }

    #[test]
    fn provider_reads_obligations_from_grant_and_mapping_carriers() {
        let install =
            TranslationInstallObligations::from_normalized_identities([activation_fact(600)]);
        let release_set =
            TranslationReleaseObligations::from_normalized_identities([translation_fact(700)]);
        let grant = mapping_grant(MappingSourceMode::BorrowedShared);
        assert_eq!(grant.install_obligations(), &install);
        assert_eq!(grant.release_obligations(), &release_set);

        let source = source();
        let source_loan = source.loan(0, 0x1000).expect("shared source loan");
        let pending = map_borrowed(source_loan, destination(), mapping_id(56), &grant)
            .expect("borrowed map candidate");
        assert_eq!(pending.install_obligations(), &install);
        let context = pending.receipt_context();
        assert_eq!(context.install_obligations(), &install);
        assert_eq!(context.release_obligations(), &release_set);

        let receipt = activate(&pending);
        let mapping = pending.complete(receipt).expect("translations installed");
        assert_eq!(mapping.release_obligations(), &release_set);

        let pending = mapping.begin_unmap();
        assert_eq!(pending.release_obligations(), &release_set);
        let receipt = release(&pending);
        let (destination, owned_source) = pending
            .complete(receipt)
            .expect("translations released")
            .into_parts();
        assert!(owned_source.is_none());
        assert_eq!(destination.base(), 0xffff_8000_0000_0000);
    }

    #[test]
    fn borrowed_mapping_retains_source_loan_and_shared_polarity() {
        let source = source();
        let source_loan = source.loan(0, 0x1000).expect("shared source");
        let pending = map_borrowed(
            source_loan,
            destination(),
            mapping_id(51),
            &mapping_grant(MappingSourceMode::BorrowedShared),
        )
        .expect("borrowed map candidate");
        let receipt = activate(&pending);
        let mut mapping = pending.complete(receipt).expect("translations installed");
        assert!(mapping.loan(0, 16).is_ok());
        assert!(mapping.loan_mut(0, 16).is_err());
        let (destination, owned_source) = finish_unmap(mapping);
        assert!(owned_source.is_none());
        assert_eq!(destination.base(), 0xffff_8000_0000_0000);
    }

    #[test]
    fn mapped_range_receipt_context_binds_complete_mapping_evidence_and_exact_range() {
        let grant = mapping_grant(MappingSourceMode::Owned);
        let first = activate_owned(source(), destination(), 54, &grant);
        let drifted = activate_owned(
            extent(9, 0x1000, 0x1000, 10, 20, &[100]),
            extent(10, 0xffff_8000_0000_0000, 0x1000, 11, 22, &[200]),
            54,
            &grant,
        );

        let exact = first
            .range_receipt_context(0x100, 0x80)
            .expect("exact mapped range");
        assert_eq!(exact, exact.clone());
        assert_ne!(
            exact,
            drifted
                .range_receipt_context(0x100, 0x80)
                .expect("same range in structurally drifted mapping")
        );
        assert_ne!(
            exact,
            first
                .range_receipt_context(0x180, 0x80)
                .expect("different exact mapped range")
        );
    }

    #[test]
    fn mapped_range_receipt_context_rejects_invalid_ranges() {
        let mapping = activate_owned(
            source(),
            destination(),
            55,
            &mapping_grant(MappingSourceMode::Owned),
        );
        for (offset, length) in [(0, 0), (u64::MAX, 2), (0xff0, 0x20)] {
            assert!(mapping.range_receipt_context(offset, length).is_err());
        }
    }

    #[test]
    fn failed_mapping_returns_source_and_destination_authority() {
        let short_destination = extent(2, 0x8000, 0x800, 11, 22, &[200]);
        let error = map_owned(
            source(),
            short_destination,
            mapping_id(52),
            &mapping_grant(MappingSourceMode::Owned),
        )
        .expect_err("mapping lengths differ");
        assert!(error.diagnostic().0.contains("equal length"));
        let (source, destination) = (*error).into_extents();
        assert_eq!((source.length(), destination.length()), (0x1000, 0x800));
    }

    #[test]
    fn peer_write_revocation_demands_the_admitted_grants_fact_set() {
        let grant = mapping_grant_with_revocation(
            MappingSourceMode::BorrowedShared,
            PeerWriteRevocationObligations::from_normalized_identities(revocation_facts(&[
                61, 62, 63,
            ])),
        );
        let mapping = activated_shared_mapping_with(Box::leak(Box::new(source())), 60, &grant);
        let pending = mapping
            .begin_peer_write_revocation()
            .expect("shared custody may revoke the peer write");
        assert_eq!(
            pending.revocation_obligations().identities().count(),
            3,
            "the demanded set rides the admitted grant, not the caller"
        );

        let short_receipt = revocation_receipt(&pending.receipt_context(), true, &[61, 62]);
        let error = pending
            .complete(short_receipt)
            .expect_err("the caller-side set cannot substitute for the grant's demand");
        assert!(error.diagnostic().0.contains("required invalidation facts"));
        let (pending, _receipt) = error.into_parts();

        let receipt = PeerWriteRevocationReceipt::from_admitted_provider(
            &pending.receipt_context(),
            true,
            grant.peer_revocation_obligations().identities(),
        );
        pending
            .complete(receipt)
            .expect("the grant's demand completes");
    }

    #[test]
    fn shared_mapping_stable_loan_requires_completed_peer_write_revocation() {
        let mapping = activated_shared_mapping();
        assert!(mapping.loan(0, 16).is_ok());
        let error = mapping
            .stable_loan(0, 16)
            .expect_err("hostile writable peer still live");
        assert!(error.0.contains("peer-write-revocation receipt"));

        let mut pending = mapping
            .begin_peer_write_revocation()
            .expect("shared custody may revoke the peer write");
        assert_eq!(
            pending.revocation_obligations().identities().count(),
            2,
            "pending carrier exposes the grant-demanded fact set"
        );

        for (established, facts, expected) in [
            (true, &[61u64][..], "required invalidation facts"),
            (false, &[61, 62][..], "revoked write permission"),
        ] {
            let receipt = revocation_receipt(&pending.receipt_context(), established, facts);
            let error = pending.complete(receipt).expect_err(expected);
            assert!(error.diagnostic().0.contains(expected));
            pending = error.into_parts().0;
        }

        let receipt = revocation_receipt(&pending.receipt_context(), true, &[61, 62]);
        let mapping = pending.complete(receipt).expect("revocation completes");
        assert!(mapping.stable_loan(0, 16).is_ok());
        let _ = finish_unmap(mapping);
    }

    #[test]
    fn peer_write_revocation_receipt_binds_the_exact_mapping() {
        let mapping = activated_shared_mapping();
        let pending = mapping
            .begin_peer_write_revocation()
            .expect("shared custody may revoke the peer write");

        let foreign_source = Box::leak(Box::new(extent(9, 0x1000, 0x1000, 10, 20, &[100])));
        let other = activated_shared_mapping_with(
            foreign_source,
            60,
            &mapping_grant(MappingSourceMode::BorrowedShared),
        );
        let foreign_receipt = revocation_receipt(&other.receipt_context(), true, &[61, 62]);
        let error = pending
            .complete(foreign_receipt)
            .expect_err("a receipt for a different mapping cannot claim this revocation");
        assert!(error.diagnostic().0.contains("exact active mapping"));
        let (pending, _receipt) = error.into_parts();
        drop(other.begin_unmap());

        let receipt = revocation_receipt(&pending.receipt_context(), true, &[61, 62]);
        pending.complete(receipt).expect("own receipt completes");
    }

    #[test]
    fn peer_write_revocation_refuses_non_shared_custody() {
        let mapping = activate_owned(
            source(),
            destination(),
            63,
            &mapping_grant(MappingSourceMode::Owned),
        );
        let error = mapping
            .begin_peer_write_revocation()
            .expect_err("owned sources have no hostile writable peer");
        assert!(error.diagnostic().0.contains("shared source custody"));
        let mapping = error.into_mapping();
        assert!(mapping.stable_loan(0, 16).is_ok());
    }
}
