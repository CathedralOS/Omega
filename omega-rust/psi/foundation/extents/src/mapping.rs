/*
We govern address-space translation grants (`MappingGrant`), pending map and
unmap transitions (`PendingMap`, `PendingUnmap`), peer-write revocation
receipts (`PeerWriteRevocationReceipt`), and authority conservation across
mapped and unmapped extents.
*/

use crate::extent::Extent;
use crate::extent::diagnostic::{ExtentDiagnostic, nonzero_identity};
use crate::identities::normalized_extent_identity;
use crate::identities::{
    AddressSpaceId, ExtentLineageId, ExtentProvenanceId, ExtentRights, MappingEraId,
    NormalizedIdentitySet,
};
use crate::loans::{ExtentLoan, LoanPolarity};
use crate::roots::root_grants::ValidatedExtentGeometry;
use crate::roots::root_origins::ExtentRootOrigin;
normalized_extent_identity! { MappingGrantId, "mapping-grant" }
normalized_extent_identity! { MappingId, "mapping" }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingSourceMode {
    Owned,
    BorrowedShared,
    BorrowedExclusive,
}

normalized_extent_identity! { TranslationActivationFactId, "translation-activation-fact" }
pub type TranslationInstallObligations = NormalizedIdentitySet<TranslationActivationFactId>;
normalized_extent_identity! { TranslationCompletionFactId, "translation-completion-fact" }
pub type TranslationReleaseObligations = NormalizedIdentitySet<TranslationCompletionFactId>;
normalized_extent_identity! { PeerWriteRevocationFactId, "peer-write-revocation-fact" }
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

    pub const fn identity(&self) -> MappingGrantId {
        self.identity
    }

    pub const fn install_obligations(&self) -> &TranslationInstallObligations {
        &self.map_obligations
    }

    pub const fn release_obligations(&self) -> &TranslationReleaseObligations {
        &self.unmap_obligations
    }

    pub const fn peer_revocation_obligations(&self) -> &PeerWriteRevocationObligations {
        &self.peer_revocation_obligations
    }
}

#[derive(Debug)]
enum MappingSource<'source> {
    Owned(Extent),
    Borrowed(ExtentLoan<'source>),
}

impl MappingSource<'_> {
    fn base(&self) -> u64 {
        match self {
            Self::Owned(extent) => extent.base(),
            Self::Borrowed(loan) => loan.base(),
        }
    }

    fn length(&self) -> u64 {
        match self {
            Self::Owned(extent) => extent.length(),
            Self::Borrowed(loan) => loan.length(),
        }
    }

    fn address_space(&self) -> AddressSpaceId {
        match self {
            Self::Owned(extent) => extent.address_space(),
            Self::Borrowed(loan) => loan.address_space(),
        }
    }

    fn rights(&self) -> &ExtentRights {
        match self {
            Self::Owned(extent) => extent.rights(),
            Self::Borrowed(loan) => loan.rights(),
        }
    }

    fn provenance(&self) -> ExtentProvenanceId {
        match self {
            Self::Owned(extent) => extent.provenance(),
            Self::Borrowed(loan) => loan.provenance(),
        }
    }

    fn era(&self) -> MappingEraId {
        match self {
            Self::Owned(extent) => extent.era(),
            Self::Borrowed(loan) => loan.era(),
        }
    }

    fn origin(&self) -> ExtentRootOrigin {
        match self {
            Self::Owned(extent) => extent.origin(),
            Self::Borrowed(loan) => loan.origin(),
        }
    }

    fn lineage_root(&self) -> ExtentLineageId {
        match self {
            Self::Owned(extent) => extent.lineage_root(),
            Self::Borrowed(loan) => loan.lineage_root(),
        }
    }

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
    pub const fn mapping(&self) -> MappingId {
        self.mapping.identity()
    }

    pub const fn grant(&self) -> MappingGrantId {
        self.mapping.grant()
    }

    pub fn receipt_context(&self) -> MappingReceiptContext {
        MappingReceiptContext(self.mapping.evidence.clone())
    }

    pub const fn install_obligations(&self) -> &TranslationInstallObligations {
        self.mapping.evidence.grant.install_obligations()
    }

    pub fn source_base(&self) -> u64 {
        self.mapping.source.base()
    }

    pub fn source_length(&self) -> u64 {
        self.mapping.source.length()
    }

    pub fn source_rights(&self) -> &ExtentRights {
        self.mapping.source.rights()
    }

    pub fn source_provenance(&self) -> ExtentProvenanceId {
        self.mapping.source.provenance()
    }

    pub fn source_era(&self) -> MappingEraId {
        self.mapping.source.era()
    }

    pub fn source_origin(&self) -> ExtentRootOrigin {
        self.mapping.source.origin()
    }

    pub fn source_mode(&self) -> MappingSourceMode {
        self.mapping.source.mode()
    }

    pub fn destination_base(&self) -> u64 {
        self.mapping.mapped.base()
    }

    pub fn mapped_rights(&self) -> &ExtentRights {
        self.mapping.mapped.rights()
    }

    pub fn mapped_origin(&self) -> ExtentRootOrigin {
        self.mapping.mapped.origin()
    }

    pub fn mapped_provenance(&self) -> ExtentProvenanceId {
        self.mapping.mapped.provenance()
    }

    pub fn mapped_era(&self) -> MappingEraId {
        self.mapping.mapped.era()
    }

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

#[derive(Debug)]
pub struct MapActivationError<'source> {
    pending: PendingMap<'source>,
    receipt: TranslationActivationReceipt,
    diagnostic: ExtentDiagnostic,
}

impl<'source> MapActivationError<'source> {
    pub const fn diagnostic(&self) -> &ExtentDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (PendingMap<'source>, TranslationActivationReceipt) {
        (self.pending, self.receipt)
    }
}

impl<'source> MappedExtent<'source> {
    pub const fn identity(&self) -> MappingId {
        self.evidence.identity
    }

    pub const fn grant(&self) -> MappingGrantId {
        self.evidence.grant.identity()
    }

    pub const fn base(&self) -> u64 {
        self.mapped.base()
    }

    pub const fn length(&self) -> u64 {
        self.mapped.length()
    }

    pub const fn rights(&self) -> &ExtentRights {
        self.mapped.rights()
    }

    pub const fn address_space(&self) -> AddressSpaceId {
        self.mapped.address_space()
    }

    pub const fn provenance(&self) -> ExtentProvenanceId {
        self.mapped.provenance()
    }

    pub const fn era(&self) -> MappingEraId {
        self.mapped.era()
    }

    pub const fn origin(&self) -> ExtentRootOrigin {
        self.mapped.origin()
    }

    pub const fn lineage_root(&self) -> ExtentLineageId {
        self.mapped.lineage_root()
    }

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

    pub const fn release_obligations(&self) -> &TranslationReleaseObligations {
        self.evidence.grant.release_obligations()
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
    pub const fn mapping(&self) -> MappingId {
        self.mapping.identity()
    }

    pub const fn grant(&self) -> MappingGrantId {
        self.mapping.grant()
    }

    pub fn receipt_context(&self) -> MappingReceiptContext {
        self.mapping.receipt_context()
    }

    pub const fn revocation_obligations(&self) -> &PeerWriteRevocationObligations {
        self.mapping.evidence.grant.peer_revocation_obligations()
    }

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

#[derive(Debug)]
pub struct PeerWriteRevocationStartError<'source> {
    mapping: MappedExtent<'source>,
    diagnostic: ExtentDiagnostic,
}

impl<'source> PeerWriteRevocationStartError<'source> {
    pub const fn diagnostic(&self) -> &ExtentDiagnostic {
        &self.diagnostic
    }

    pub fn into_mapping(self) -> MappedExtent<'source> {
        self.mapping
    }
}

#[derive(Debug)]
pub struct PeerWriteRevocationError<'source> {
    pending: PendingPeerWriteRevocation<'source>,
    receipt: PeerWriteRevocationReceipt,
    diagnostic: ExtentDiagnostic,
}

impl<'source> PeerWriteRevocationError<'source> {
    pub const fn diagnostic(&self) -> &ExtentDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        PendingPeerWriteRevocation<'source>,
        PeerWriteRevocationReceipt,
    ) {
        (self.pending, self.receipt)
    }
}

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
    let source = MappingSource::Owned(source);
    match map_with_source(source, destination, identity, grant) {
        Ok(mapping) => Ok(mapping),
        Err(error) => {
            let MappingStartError {
                source,
                destination,
                diagnostic,
            } = *error;
            let MappingSource::Owned(source) = source else {
                unreachable!("owned mapping preserves source mode")
            };
            Err(Box::new(OwnedMappingError {
                source,
                destination,
                diagnostic,
            }))
        }
    }
}

pub fn map_borrowed<'source>(
    source: ExtentLoan<'source>,
    destination: Extent,
    identity: MappingId,
    grant: &MappingGrant,
) -> Result<PendingMap<'source>, Box<BorrowedMappingError<'source>>> {
    let source = MappingSource::Borrowed(source);
    match map_with_source(source, destination, identity, grant) {
        Ok(mapping) => Ok(mapping),
        Err(error) => {
            let MappingStartError {
                source,
                destination,
                diagnostic,
            } = *error;
            let MappingSource::Borrowed(source) = source else {
                unreachable!("borrowed mapping preserves source mode")
            };
            Err(Box::new(BorrowedMappingError {
                source,
                destination,
                diagnostic,
            }))
        }
    }
}

fn map_with_source<'source>(
    source: MappingSource<'source>,
    destination: Extent,
    identity: MappingId,
    grant: &MappingGrant,
) -> Result<PendingMap<'source>, Box<MappingStartError<'source>>> {
    let mismatch = if source.mode() != grant.source_mode {
        Some("source custody does not match mapping grant")
    } else if source.address_space() != grant.source_space {
        Some("source address space does not match mapping grant")
    } else if destination.address_space() != grant.destination_space {
        Some("destination address space does not match mapping grant")
    } else if !source.rights().contains(&grant.required_source_rights) {
        Some("source lacks rights required by mapping grant")
    } else if !destination
        .rights()
        .contains(&grant.required_destination_rights)
    {
        Some("destination lacks rights required by mapping grant")
    } else if source.length() != destination.length() {
        Some("source and destination mapping ranges must have equal length")
    } else {
        None
    };

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

#[derive(Debug)]
pub struct OwnedMappingError {
    source: Extent,
    destination: Extent,
    diagnostic: ExtentDiagnostic,
}

impl OwnedMappingError {
    pub const fn diagnostic(&self) -> &ExtentDiagnostic {
        &self.diagnostic
    }

    pub fn into_extents(self) -> (Extent, Extent) {
        (self.source, self.destination)
    }
}

#[derive(Debug)]
pub struct BorrowedMappingError<'source> {
    source: ExtentLoan<'source>,
    destination: Extent,
    diagnostic: ExtentDiagnostic,
}

impl<'source> BorrowedMappingError<'source> {
    pub const fn diagnostic(&self) -> &ExtentDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ExtentLoan<'source>, Extent) {
        (self.source, self.destination)
    }
}

#[derive(Debug)]
pub struct PendingUnmap<'source> {
    mapping: MappedExtent<'source>,
}

impl<'source> PendingUnmap<'source> {
    pub const fn mapping(&self) -> MappingId {
        self.mapping.identity()
    }

    pub fn receipt_context(&self) -> MappingReceiptContext {
        MappingReceiptContext(self.mapping.evidence.clone())
    }

    pub const fn release_obligations(&self) -> &TranslationReleaseObligations {
        self.mapping.release_obligations()
    }

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

#[derive(Debug)]
pub struct UnmapCompletionError<'source> {
    pending: PendingUnmap<'source>,
    receipt: TranslationReleaseReceipt,
    diagnostic: ExtentDiagnostic,
}

impl<'source> UnmapCompletionError<'source> {
    pub const fn diagnostic(&self) -> &ExtentDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (PendingUnmap<'source>, TranslationReleaseReceipt) {
        (self.pending, self.receipt)
    }
}

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
    use crate::ExtentRootGrant;
    use crate::tests::{
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

    #[test]
    fn owned_mapping_round_trips_both_authorities_after_translation_release() {
        let mapping_identity = mapping_id(50);
        let pending = map_owned(
            source(),
            destination(),
            mapping_identity,
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
        let mut wrong_mapping = TranslationActivationReceipt::from_admitted_provider(
            &pending.receipt_context(),
            true,
            [activation_fact(600)],
        );
        wrong_mapping.mapping.identity = mapping_id(999);
        let error = pending
            .complete(wrong_mapping)
            .expect_err("receipt for another mapping");
        assert!(error.diagnostic().0.contains("exact pending mapping"));
        let (pending, _) = (*error).into_parts();
        let mut wrong_issuance = TranslationActivationReceipt::from_admitted_provider(
            &pending.receipt_context(),
            true,
            [activation_fact(600)],
        );
        wrong_issuance.mapping.source_origin =
            ExtentRootOrigin::ProviderIssued(provider_issuance_for_invocation(1, 99));
        let error = pending
            .complete(wrong_issuance)
            .expect_err("receipt cannot substitute provider issuance evidence");
        assert!(error.diagnostic().0.contains("exact pending mapping"));
        let (pending, _) = (*error).into_parts();
        let inactive = TranslationActivationReceipt::from_admitted_provider(
            &pending.receipt_context(),
            false,
            [activation_fact(600)],
        );
        let error = pending
            .complete(inactive)
            .expect_err("translations not installed");
        assert!(error.diagnostic().0.contains("does not establish"));
        let (pending, _) = (*error).into_parts();
        let incomplete = TranslationActivationReceipt::from_admitted_provider(
            &pending.receipt_context(),
            true,
            [],
        );
        let error = pending
            .complete(incomplete)
            .expect_err("installation fact missing");
        assert!(error.diagnostic().0.contains("lacks"));
        let (pending, _) = (*error).into_parts();
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
        let source = local_extent(1, 11, 0x1000, 10, 20, &[100]);
        let destination = local_extent(2, 12, 0xffff_8000_0000_0000, 11, 22, &[200]);
        let pending = map_owned(
            source,
            destination,
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
        let pending = mapping.begin_unmap();
        let receipt = release(&pending);
        let (destination, source) = pending
            .complete(receipt)
            .expect("translations released")
            .into_parts();
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
        let pending = mapping.begin_unmap();
        let receipt = release(&pending);
        let (destination, owned_source) = pending
            .complete(receipt)
            .expect("translations released")
            .into_parts();
        assert!(owned_source.is_none());
        assert_eq!(destination.base(), 0xffff_8000_0000_0000);
    }

    #[test]
    fn mapped_range_receipt_context_binds_complete_mapping_evidence_and_exact_range() {
        let grant = mapping_grant(MappingSourceMode::Owned);
        let first = map_owned(source(), destination(), mapping_id(54), &grant)
            .expect("first pending mapping");
        let receipt = activate(&first);
        let first = first.complete(receipt).expect("first active mapping");

        let drifted_source = extent(9, 0x1000, 0x1000, 10, 20, &[100]);
        let drifted_destination = extent(10, 0xffff_8000_0000_0000, 0x1000, 11, 22, &[200]);
        let drifted = map_owned(drifted_source, drifted_destination, mapping_id(54), &grant)
            .expect("same compact IDs with different authority lineages");
        let receipt = activate(&drifted);
        let drifted = drifted.complete(receipt).expect("drifted active mapping");

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
        let pending = map_owned(
            source(),
            destination(),
            mapping_id(55),
            &mapping_grant(MappingSourceMode::Owned),
        )
        .expect("pending mapping");
        let receipt = activate(&pending);
        let mapping = pending.complete(receipt).expect("active mapping");

        assert!(mapping.range_receipt_context(0, 0).is_err());
        assert!(mapping.range_receipt_context(u64::MAX, 2).is_err());
        assert!(mapping.range_receipt_context(0xff0, 0x20).is_err());
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

    fn revocation_obligations() -> PeerWriteRevocationObligations {
        PeerWriteRevocationObligations::from_normalized_identities([
            id(61, PeerWriteRevocationFactId::from_normalized_identity),
            id(62, PeerWriteRevocationFactId::from_normalized_identity),
        ])
    }

    fn activated_shared_mapping_with(
        source_extent: &'static Extent,
        mapping_identity: u64,
    ) -> super::MappedExtent<'static> {
        let source_loan = source_extent.loan(0, 0x1000).expect("shared source");
        let pending = map_borrowed(
            source_loan,
            destination(),
            mapping_id(mapping_identity),
            &mapping_grant(MappingSourceMode::BorrowedShared),
        )
        .expect("borrowed map candidate");
        let receipt = activate(&pending);
        pending.complete(receipt).expect("translations installed")
    }

    fn activated_shared_mapping() -> super::MappedExtent<'static> {
        activated_shared_mapping_with(Box::leak(Box::new(source())), 60)
    }

    fn activated_shared_mapping_with_grant(grant: &MappingGrant) -> super::MappedExtent<'static> {
        let source_loan = Box::leak(Box::new(source()))
            .loan(0, 0x1000)
            .expect("shared source");
        let pending = map_borrowed(source_loan, destination(), mapping_id(60), grant)
            .expect("borrowed map candidate");
        let receipt = activate(&pending);
        pending.complete(receipt).expect("translations installed")
    }

    #[test]
    fn peer_write_revocation_demands_the_admitted_grants_fact_set() {
        let grant = mapping_grant_with_revocation(
            MappingSourceMode::BorrowedShared,
            PeerWriteRevocationObligations::from_normalized_identities([
                id(61, PeerWriteRevocationFactId::from_normalized_identity),
                id(62, PeerWriteRevocationFactId::from_normalized_identity),
                id(63, PeerWriteRevocationFactId::from_normalized_identity),
            ]),
        );
        let mapping = activated_shared_mapping_with_grant(&grant);
        let pending = mapping
            .begin_peer_write_revocation()
            .expect("shared custody may revoke the peer write");
        assert_eq!(
            pending.revocation_obligations().identities().count(),
            3,
            "the demanded set rides the admitted grant, not the caller"
        );

        let short_receipt = PeerWriteRevocationReceipt::from_admitted_provider(
            &pending.receipt_context(),
            true,
            [
                id(61, PeerWriteRevocationFactId::from_normalized_identity),
                id(62, PeerWriteRevocationFactId::from_normalized_identity),
            ],
        );
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

        let pending = mapping
            .begin_peer_write_revocation()
            .expect("shared custody may revoke the peer write");
        assert_eq!(
            pending.revocation_obligations().identities().count(),
            2,
            "pending carrier exposes the grant-demanded fact set"
        );

        let short_receipt = PeerWriteRevocationReceipt::from_admitted_provider(
            &pending.receipt_context(),
            true,
            [id(61, PeerWriteRevocationFactId::from_normalized_identity)],
        );
        let error = pending
            .complete(short_receipt)
            .expect_err("missing invalidation fact must reject");
        assert!(error.diagnostic().0.contains("required invalidation facts"));
        let (pending, _receipt) = error.into_parts();

        let unrevoked_receipt = PeerWriteRevocationReceipt::from_admitted_provider(
            &pending.receipt_context(),
            false,
            [
                id(61, PeerWriteRevocationFactId::from_normalized_identity),
                id(62, PeerWriteRevocationFactId::from_normalized_identity),
            ],
        );
        let error = pending
            .complete(unrevoked_receipt)
            .expect_err("asserting facts without revocation must reject");
        assert!(error.diagnostic().0.contains("revoked write permission"));
        let (pending, _receipt) = error.into_parts();

        let receipt = PeerWriteRevocationReceipt::from_admitted_provider(
            &pending.receipt_context(),
            true,
            [
                id(61, PeerWriteRevocationFactId::from_normalized_identity),
                id(62, PeerWriteRevocationFactId::from_normalized_identity),
            ],
        );
        let mapping = pending.complete(receipt).expect("revocation completes");
        assert!(mapping.stable_loan(0, 16).is_ok());
        let pending = mapping.begin_unmap();
        let receipt = release(&pending);
        pending
            .complete(receipt)
            .expect("revoked mapping still unmaps");
    }

    #[test]
    fn peer_write_revocation_receipt_binds_the_exact_mapping() {
        let mapping = activated_shared_mapping();
        let pending = mapping
            .begin_peer_write_revocation()
            .expect("shared custody may revoke the peer write");

        let foreign_source = Box::leak(Box::new(extent(9, 0x1000, 0x1000, 10, 20, &[100])));
        let other = activated_shared_mapping_with(foreign_source, 60);
        let foreign_context = other.receipt_context();
        let foreign_receipt = PeerWriteRevocationReceipt::from_admitted_provider(
            &foreign_context,
            true,
            [
                id(61, PeerWriteRevocationFactId::from_normalized_identity),
                id(62, PeerWriteRevocationFactId::from_normalized_identity),
            ],
        );
        let error = pending
            .complete(foreign_receipt)
            .expect_err("a receipt for a different mapping cannot claim this revocation");
        assert!(error.diagnostic().0.contains("exact active mapping"));
        let (pending, _receipt) = error.into_parts();
        drop(other.begin_unmap());

        let receipt = PeerWriteRevocationReceipt::from_admitted_provider(
            &pending.receipt_context(),
            true,
            [
                id(61, PeerWriteRevocationFactId::from_normalized_identity),
                id(62, PeerWriteRevocationFactId::from_normalized_identity),
            ],
        );
        pending.complete(receipt).expect("own receipt completes");
    }

    #[test]
    fn peer_write_revocation_refuses_non_shared_custody() {
        let pending = map_owned(
            source(),
            destination(),
            mapping_id(63),
            &mapping_grant(MappingSourceMode::Owned),
        )
        .expect("owned map candidate");
        let receipt = activate(&pending);
        let mapping = pending.complete(receipt).expect("translations installed");

        let error = mapping
            .begin_peer_write_revocation()
            .expect_err("owned sources have no hostile writable peer");
        assert!(error.diagnostic().0.contains("shared source custody"));
        let mapping = error.into_mapping();
        assert!(mapping.stable_loan(0, 16).is_ok());
    }
}
