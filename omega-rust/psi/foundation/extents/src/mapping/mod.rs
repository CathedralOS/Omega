use crate::extent::Extent;
use crate::extent::diagnostic::ExtentDiagnostic;
use crate::identities::{
    AddressSpaceId, ExtentLineageId, ExtentProvenanceId, ExtentRights, MappingEraId,
    normalized_extent_identity,
};
use crate::loans::{ExtentLoan, LoanPolarity};
use crate::roots::root_grants::ValidatedExtentGeometry;
use crate::roots::root_origins::ExtentRootOrigin;
use std::collections::BTreeSet;

normalized_extent_identity!(MappingGrantId, "mapping-grant");

normalized_extent_identity!(MappingId, "mapping");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingSourceMode {
    Owned,
    BorrowedShared,
    BorrowedExclusive,
}

normalized_extent_identity!(
    TranslationActivationFactId,
    "translation-activation-fact"
);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TranslationInstallObligations(BTreeSet<TranslationActivationFactId>);

impl TranslationInstallObligations {
    pub fn from_normalized_facts(
        facts: impl IntoIterator<Item = TranslationActivationFactId>,
    ) -> Self {
        Self(facts.into_iter().collect())
    }

    pub fn facts(&self) -> impl Iterator<Item = TranslationActivationFactId> + '_ {
        self.0.iter().copied()
    }
}

normalized_extent_identity!(
    TranslationCompletionFactId,
    "translation-completion-fact"
);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TranslationReleaseObligations(BTreeSet<TranslationCompletionFactId>);

impl TranslationReleaseObligations {
    pub fn from_normalized_facts(
        facts: impl IntoIterator<Item = TranslationCompletionFactId>,
    ) -> Self {
        Self(facts.into_iter().collect())
    }

    pub fn facts(&self) -> impl Iterator<Item = TranslationCompletionFactId> + '_ {
        self.0.iter().copied()
    }
}

normalized_extent_identity!(
    PeerWriteRevocationFactId,
    "peer-write-revocation-fact"
);

/// The provider-established facts a shared-custody mapping's revocation
/// receipt must show before its payload may be read zero-copy: the hostile
/// peer's write permission was revoked or remapped away and the resulting
/// cross-core invalidation completed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PeerWriteRevocationObligations(BTreeSet<PeerWriteRevocationFactId>);

impl PeerWriteRevocationObligations {
    pub fn from_normalized_facts(
        facts: impl IntoIterator<Item = PeerWriteRevocationFactId>,
    ) -> Self {
        Self(facts.into_iter().collect())
    }

    pub fn facts(&self) -> impl Iterator<Item = PeerWriteRevocationFactId> + '_ {
        self.0.iter().copied()
    }
}

/// Reusable provider-admitted mapping policy.
///
/// Source and destination rights are requirements on existing authority. The
/// mapped rights/provenance/era are provider-established output facts, not
/// facts inferred from address bits.
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

    /// The translation-activation facts this admitted grant obliges an
    /// install receipt to establish. Providers read the bound policy here;
    /// the set carries no authority of its own.
    pub const fn install_obligations(&self) -> &TranslationInstallObligations {
        &self.map_obligations
    }

    /// The translation-completion facts this admitted grant obliges a
    /// release receipt to establish before either side's authority becomes
    /// reusable.
    pub const fn release_obligations(&self) -> &TranslationReleaseObligations {
        &self.unmap_obligations
    }

    /// The revocation facts this admitted grant obliges a peer-write
    /// revocation receipt to establish before a shared-custody mapping may
    /// expose a stable view. Only `BorrowedShared` source custody has a
    /// hostile peer to revoke; the admitted set is the policy the transition
    /// enforces, not a caller-chosen demand.
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

/// Exact inert facts behind one pending or active translation.
///
/// This is deliberately not source-visible mapping authority. Provider
/// receipts retain it so compact mapping/grant IDs cannot authorize another
/// range, authority lineage, custody mode, or grant after a collision.
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

/// Reusable opaque provider context for minting activation or release
/// receipts for one exact mapping. It contains no authority and exposes no
/// address fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappingReceiptContext(MappingEvidence);

impl MappingReceiptContext {
    /// The install obligations the bound grant demands of an activation
    /// receipt for this mapping. Visible here so a provider holding only the
    /// exported context can still discover the facts it must establish.
    pub const fn install_obligations(&self) -> &TranslationInstallObligations {
        &self.0.grant.map_obligations
    }

    /// The release obligations the bound grant demands of a release receipt
    /// for this mapping.
    pub const fn release_obligations(&self) -> &TranslationReleaseObligations {
        &self.0.grant.unmap_obligations
    }

    /// The revocation obligations the bound grant demands of a peer-write
    /// revocation receipt before this mapping exposes a stable view.
    pub const fn peer_revocation_obligations(&self) -> &PeerWriteRevocationObligations {
        &self.0.grant.peer_revocation_obligations
    }
}

/// Opaque provider context binding one exact mapped subrange to the complete
/// structural evidence for its active mapping.
///
/// This is an inert receipt input, not mapping or access authority. Its range
/// geometry and mapping evidence remain private so cloning the context cannot
/// expose or amplify the non-clonable [`MappedExtent`] authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappedRangeReceiptContext {
    mapping: MappingEvidence,
    range: ValidatedExtentGeometry,
}

/// One active mapping. It owns the destination virtual-range authority and
/// either owns or borrow-carries its source as declared by the grant.
#[derive(Debug)]
pub struct MappedExtent<'source> {
    identity: MappingId,
    grant: MappingGrantId,
    evidence: MappingEvidence,
    mapped: Extent,
    source: MappingSource<'source>,
    destination: DestinationRestoration,
    unmap_obligations: TranslationReleaseObligations,
    /// Whether a peer-write-revocation receipt completed for a shared-custody
    /// source. Irrelevant for owned/exclusive sources; gates `stable_loan`.
    peer_write_revoked: bool,
}

impl PartialEq for MappedExtent<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.evidence == other.evidence
    }
}

impl Eq for MappedExtent<'_> {}

/// Linear pending state after structural mapping validation but before any
/// translated access is exposed.
///
/// The provider may now establish target translations and ordering operations,
/// and establish target-specific activation facts. Only an exact receipt can
/// turn this state into `MappedExtent`; a structurally valid mapping candidate
/// is never itself evidence that hardware translations exist.
#[derive(Debug)]
pub struct PendingMap<'source> {
    mapping: MappedExtent<'source>,
    map_obligations: TranslationInstallObligations,
}

impl<'source> PendingMap<'source> {
    pub const fn mapping(&self) -> MappingId {
        self.mapping.identity
    }

    pub const fn grant(&self) -> MappingGrantId {
        self.mapping.grant
    }

    pub fn receipt_context(&self) -> MappingReceiptContext {
        MappingReceiptContext(self.mapping.evidence.clone())
    }

    /// The exact install obligations this pending mapping's activation
    /// receipt must establish. A provider holding only the pending carrier
    /// reads the admitted fact set here; the obligations carry no authority.
    pub const fn install_obligations(&self) -> &TranslationInstallObligations {
        &self.map_obligations
    }

    /// Provider-side source address data for deriving target translation
    /// entries. The returned number grants no access or mapping authority.
    pub fn source_base(&self) -> u64 {
        self.mapping.source.base()
    }

    pub fn source_length(&self) -> u64 {
        self.mapping.source.length()
    }

    pub fn source_address_space(&self) -> AddressSpaceId {
        self.mapping.source.address_space()
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

    pub fn source_lineage_root(&self) -> ExtentLineageId {
        self.mapping.source.lineage_root()
    }

    pub fn source_mode(&self) -> MappingSourceMode {
        self.mapping.source.mode()
    }

    pub fn destination_base(&self) -> u64 {
        self.mapping.mapped.base()
    }

    pub fn destination_length(&self) -> u64 {
        self.mapping.mapped.length()
    }

    pub fn destination_address_space(&self) -> AddressSpaceId {
        self.mapping.mapped.address_space()
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

    pub fn destination_lineage_root(&self) -> ExtentLineageId {
        self.mapping.mapped.lineage_root()
    }

    /// Authority facts that an unmap provider must restore. These are exposed
    /// only as inert normalized data; the pending map retains the authority.
    pub fn destination_restoration_rights(&self) -> &ExtentRights {
        &self.mapping.destination.rights
    }

    pub fn destination_restoration_provenance(&self) -> ExtentProvenanceId {
        self.mapping.destination.provenance
    }

    pub fn destination_restoration_era(&self) -> MappingEraId {
        self.mapping.destination.era
    }

    pub(crate) fn validate_activation_receipt(
        &self,
        receipt: &TranslationActivationReceipt,
    ) -> Result<(), ExtentDiagnostic> {
        let mismatch = if receipt.mapping != self.mapping.evidence {
            Some("translation-activation receipt does not bind the exact pending mapping")
        } else if !receipt.translations_installed {
            Some("translation-activation receipt does not establish installed translations")
        } else if !self.map_obligations.0.is_subset(&receipt.established_facts) {
            Some("translation-activation receipt lacks required installation facts")
        } else {
            None
        };

        match mismatch {
            Some(message) => Err(ExtentDiagnostic(message.into())),
            None => Ok(()),
        }
    }

    pub fn complete(
        self,
        receipt: TranslationActivationReceipt,
    ) -> Result<MappedExtent<'source>, Box<MapActivationError<'source>>> {
        if let Err(diagnostic) = self.validate_activation_receipt(&receipt) {
            return Err(Box::new(MapActivationError {
                pending: self,
                receipt,
                diagnostic,
            }));
        }

        Ok(self.mapping)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct TranslationActivationReceipt {
    mapping: MappingEvidence,
    translations_installed: bool,
    established_facts: BTreeSet<TranslationActivationFactId>,
}

impl TranslationActivationReceipt {
    pub fn from_admitted_provider(
        context: &MappingReceiptContext,
        translations_installed: bool,
        established_facts: impl IntoIterator<Item = TranslationActivationFactId>,
    ) -> Self {
        Self {
            mapping: context.0.clone(),
            translations_installed,
            established_facts: established_facts.into_iter().collect(),
        }
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
        self.identity
    }

    pub const fn grant(&self) -> MappingGrantId {
        self.grant
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

    /// Export inert evidence for provider receipts that must bind this exact
    /// activated translation. The context carries no mapping authority and
    /// exposes no address fields; the non-clonable `MappedExtent` remains the
    /// sole live authority over the translated range.
    pub fn receipt_context(&self) -> MappingReceiptContext {
        MappingReceiptContext(self.evidence.clone())
    }

    /// Export inert evidence for a receipt concerning one exact, nonempty
    /// subrange of this mapping.
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

    /// Zero-copy validation access to the mapped payload. Under
    /// `BorrowedShared` source custody the peer holding the other end may be
    /// hostile: a shared borrow observes its writes with no ordering
    /// guarantees, so a stable view exists only after a peer-write-revocation
    /// receipt shows the peer's write permission was revoked or remapped away
    /// and the cross-core invalidation completed. Consumers that must read
    /// without that receipt have to copy through [`Self::loan`] and validate
    /// the copy instead.
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

    /// The exact release obligations this active mapping's eventual release
    /// receipt must establish. Reading them while the mapping is installed
    /// lets a provider assemble teardown evidence before consuming it.
    pub const fn release_obligations(&self) -> &TranslationReleaseObligations {
        &self.unmap_obligations
    }

    /// Consume this mapping into a pending peer-write revocation carrying
    /// the admitted grant's revocation obligations. Only shared source
    /// custody has a hostile writable peer to revoke; owned and exclusive
    /// sources have no second writer and refuse the transition.
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
        let obligations = self.evidence.grant.peer_revocation_obligations.clone();
        Ok(PendingPeerWriteRevocation {
            mapping: self,
            obligations,
        })
    }

    pub fn begin_unmap(self) -> PendingUnmap<'source> {
        PendingUnmap { mapping: self }
    }
}

/// Linear pending state between requesting a shared peer's write revocation
/// and treating the mapping's payload as a stable view. The mapping's access
/// authority stays live but its shared reads remain unvalidated until an
/// exact revocation receipt completes the transition.
#[derive(Debug)]
pub struct PendingPeerWriteRevocation<'source> {
    mapping: MappedExtent<'source>,
    obligations: PeerWriteRevocationObligations,
}

impl<'source> PendingPeerWriteRevocation<'source> {
    pub const fn mapping(&self) -> MappingId {
        self.mapping.identity
    }

    pub const fn grant(&self) -> MappingGrantId {
        self.mapping.grant
    }

    /// The inert evidence a revocation receipt must bind: this exact active
    /// mapping, not merely its compact grant or mapping identity.
    pub fn receipt_context(&self) -> MappingReceiptContext {
        self.mapping.receipt_context()
    }

    /// The exact revocation facts the completing receipt must establish.
    /// They carry no authority; a provider holding only this pending carrier
    /// reads the demanded fact set here.
    pub const fn revocation_obligations(&self) -> &PeerWriteRevocationObligations {
        &self.obligations
    }

    pub fn complete(
        self,
        receipt: PeerWriteRevocationReceipt,
    ) -> Result<MappedExtent<'source>, Box<PeerWriteRevocationError<'source>>> {
        let diagnostic = if receipt.mapping != self.mapping.evidence {
            Some("peer-write-revocation receipt does not bind the exact active mapping")
        } else if !receipt.peer_write_revoked {
            Some("peer-write-revocation receipt does not establish the revoked write permission")
        } else if !self.obligations.0.is_subset(&receipt.established_facts) {
            Some("peer-write-revocation receipt lacks required invalidation facts")
        } else {
            None
        };
        if let Some(message) = diagnostic {
            return Err(Box::new(PeerWriteRevocationError {
                pending: self,
                receipt,
                diagnostic: ExtentDiagnostic(message.into()),
            }));
        }
        let mut mapping = self.mapping;
        mapping.peer_write_revoked = true;
        Ok(mapping)
    }
}

/// Provider receipt asserting that for one exact active shared mapping the
/// hostile peer's write permission was revoked or remapped away and the
/// required cross-core invalidation completed. Like every provider receipt
/// here it binds complete mapping evidence; a receipt naming only a compact
/// identity cannot claim the transition.
#[derive(Debug, PartialEq, Eq)]
pub struct PeerWriteRevocationReceipt {
    mapping: MappingEvidence,
    peer_write_revoked: bool,
    established_facts: BTreeSet<PeerWriteRevocationFactId>,
}

impl PeerWriteRevocationReceipt {
    pub fn from_admitted_provider(
        context: &MappingReceiptContext,
        peer_write_revoked: bool,
        established_facts: impl IntoIterator<Item = PeerWriteRevocationFactId>,
    ) -> Self {
        Self {
            mapping: context.0.clone(),
            peer_write_revoked,
            established_facts: established_facts.into_iter().collect(),
        }
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
        destination: destination_restoration.clone(),
    };
    Ok(PendingMap {
        mapping: MappedExtent {
            identity,
            grant: grant.identity,
            evidence,
            mapped,
            source,
            destination: destination_restoration,
            unmap_obligations: grant.unmap_obligations.clone(),
            peer_write_revoked: false,
        },
        map_obligations: grant.map_obligations.clone(),
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

/// Linear pending state between invalidating a mapping and reclaiming either
/// side. Even synchronous providers discharge this internally before returning
/// reusable extents to their caller.
#[derive(Debug)]
pub struct PendingUnmap<'source> {
    mapping: MappedExtent<'source>,
}

impl<'source> PendingUnmap<'source> {
    pub const fn mapping(&self) -> MappingId {
        self.mapping.identity
    }

    pub fn receipt_context(&self) -> MappingReceiptContext {
        MappingReceiptContext(self.mapping.evidence.clone())
    }

    /// The exact release obligations this teardown's release receipt must
    /// establish before either side's authority becomes reusable.
    pub const fn release_obligations(&self) -> &TranslationReleaseObligations {
        self.mapping.release_obligations()
    }

    pub(crate) fn validate_release_receipt(
        &self,
        receipt: &TranslationReleaseReceipt,
    ) -> Result<(), ExtentDiagnostic> {
        let mismatch = if receipt.mapping != self.mapping.evidence {
            Some("translation-release receipt does not bind the exact pending mapping")
        } else if !receipt.translations_released {
            Some("translation-release receipt does not release stale translations")
        } else if !self
            .mapping
            .unmap_obligations
            .0
            .is_subset(&receipt.established_facts)
        {
            Some("translation-release receipt lacks required completion facts")
        } else {
            None
        };

        match mismatch {
            Some(message) => Err(ExtentDiagnostic(message.into())),
            None => Ok(()),
        }
    }

    pub fn complete(
        self,
        receipt: TranslationReleaseReceipt,
    ) -> Result<UnmappedExtents, Box<UnmapCompletionError<'source>>> {
        if let Err(diagnostic) = self.validate_release_receipt(&receipt) {
            return Err(Box::new(UnmapCompletionError {
                pending: self,
                receipt,
                diagnostic,
            }));
        }

        let MappedExtent {
            mapped,
            source,
            destination,
            ..
        } = self.mapping;
        let destination = Extent {
            rights: destination.rights,
            provenance: destination.provenance,
            era: destination.era,
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

#[derive(Debug, PartialEq, Eq)]
pub struct TranslationReleaseReceipt {
    mapping: MappingEvidence,
    translations_released: bool,
    established_facts: BTreeSet<TranslationCompletionFactId>,
}

impl TranslationReleaseReceipt {
    pub fn from_admitted_provider(
        context: &MappingReceiptContext,
        translations_released: bool,
        established_facts: impl IntoIterator<Item = TranslationCompletionFactId>,
    ) -> Self {
        Self {
            mapping: context.0.clone(),
            translations_released,
            established_facts: established_facts.into_iter().collect(),
        }
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
mod tests;
