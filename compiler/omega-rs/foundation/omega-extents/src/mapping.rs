use std::collections::BTreeSet;

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MappingGrantId(u64);

impl MappingGrantId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, ExtentDiagnostic> {
        nonzero_identity(identity, "mapping-grant")?;
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MappingId(u64);

impl MappingId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, ExtentDiagnostic> {
        nonzero_identity(identity, "mapping")?;
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingSourceMode {
    Owned,
    BorrowedShared,
    BorrowedExclusive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TranslationActivationFactId(u64);

impl TranslationActivationFactId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, ExtentDiagnostic> {
        nonzero_identity(identity, "translation-activation-fact")?;
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TranslationCompletionFactId(u64);

impl TranslationCompletionFactId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, ExtentDiagnostic> {
        nonzero_identity(identity, "translation-completion-fact")?;
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

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
        }
    }

    pub const fn identity(&self) -> MappingGrantId {
        self.identity
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

#[derive(Debug)]
struct DestinationRestoration {
    rights: ExtentRights,
    provenance: ExtentProvenanceId,
    era: MappingEraId,
}

/// One active mapping. It owns the destination virtual-range authority and
/// either owns or borrow-carries its source as declared by the grant.
#[derive(Debug)]
pub struct MappedExtent<'source> {
    identity: MappingId,
    grant: MappingGrantId,
    mapped: Extent,
    source: MappingSource<'source>,
    destination: DestinationRestoration,
    unmap_obligations: TranslationReleaseObligations,
}

/// Linear pending state after structural mapping validation but before any
/// translated access is exposed.
///
/// The provider may now edit page tables, issue required ordering operations,
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

    pub(crate) const fn mapped_extent(&self) -> &Extent {
        &self.mapping.mapped
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
        let mismatch = if receipt.mapping != self.mapping.identity {
            Some("translation-activation receipt names a different mapping")
        } else if receipt.grant != self.mapping.grant {
            Some("translation-activation receipt names a different mapping grant")
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
    mapping: MappingId,
    grant: MappingGrantId,
    translations_installed: bool,
    established_facts: BTreeSet<TranslationActivationFactId>,
}

impl TranslationActivationReceipt {
    pub fn from_admitted_provider(
        mapping: MappingId,
        grant: MappingGrantId,
        translations_installed: bool,
        established_facts: impl IntoIterator<Item = TranslationActivationFactId>,
    ) -> Self {
        Self {
            mapping,
            grant,
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

    pub fn begin_unmap(self) -> PendingUnmap<'source> {
        PendingUnmap { mapping: self }
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
    Ok(PendingMap {
        mapping: MappedExtent {
            identity,
            grant: grant.identity,
            mapped,
            source,
            destination: destination_restoration,
            unmap_obligations: grant.unmap_obligations.clone(),
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

    pub(crate) fn validate_release_receipt(
        &self,
        receipt: &TranslationReleaseReceipt,
    ) -> Result<(), ExtentDiagnostic> {
        let mismatch = if receipt.mapping != self.mapping.identity {
            Some("translation-release receipt names a different mapping")
        } else if receipt.grant != self.mapping.grant {
            Some("translation-release receipt names a different mapping grant")
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
    mapping: MappingId,
    grant: MappingGrantId,
    translations_released: bool,
    established_facts: BTreeSet<TranslationCompletionFactId>,
}

impl TranslationReleaseReceipt {
    pub fn from_admitted_provider(
        mapping: MappingId,
        grant: MappingGrantId,
        translations_released: bool,
        established_facts: impl IntoIterator<Item = TranslationCompletionFactId>,
    ) -> Self {
        Self {
            mapping,
            grant,
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
mod tests {
    use super::*;

    fn id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExtentDiagnostic>) -> T {
        constructor(identity).expect("normalized identity")
    }

    fn rights(identities: &[u64]) -> ExtentRights {
        ExtentRights::from_normalized_identities(
            identities
                .iter()
                .copied()
                .map(|identity| id(identity, ExtentRightId::from_normalized_identity)),
        )
    }

    fn extent(
        lineage: u64,
        base: u64,
        length: u64,
        space: u64,
        provenance: u64,
        extent_rights: &[u64],
    ) -> Extent {
        ExtentRootGrant::from_admitted_provider(
            id(lineage, ExtentLineageId::from_normalized_identity),
            id(space, AddressSpaceId::from_normalized_identity),
            rights(extent_rights),
            id(provenance, ExtentProvenanceId::from_normalized_identity),
            id(30, MappingEraId::from_normalized_identity),
        )
        .mint(base, length)
        .expect("root extent")
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
            TranslationInstallObligations::from_normalized_facts([activation_fact(600)]),
            TranslationReleaseObligations::from_normalized_facts([translation_fact(700)]),
        )
    }

    fn source() -> Extent {
        extent(1, 0x1000, 0x1000, 10, 20, &[100])
    }

    fn destination() -> Extent {
        extent(2, 0xffff_8000_0000_0000, 0x1000, 11, 22, &[200])
    }

    fn release(mapping: MappingId) -> TranslationReleaseReceipt {
        TranslationReleaseReceipt::from_admitted_provider(
            mapping,
            id(40, MappingGrantId::from_normalized_identity),
            true,
            [translation_fact(700)],
        )
    }

    fn activate(mapping: MappingId) -> TranslationActivationReceipt {
        TranslationActivationReceipt::from_admitted_provider(
            mapping,
            id(40, MappingGrantId::from_normalized_identity),
            true,
            [activation_fact(600)],
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
        let error = pending
            .complete(TranslationActivationReceipt::from_admitted_provider(
                mapping_id(999),
                id(40, MappingGrantId::from_normalized_identity),
                true,
                [activation_fact(600)],
            ))
            .expect_err("receipt for another mapping");
        assert!(error.diagnostic().0.contains("different mapping"));
        let (pending, _) = (*error).into_parts();
        let error = pending
            .complete(TranslationActivationReceipt::from_admitted_provider(
                mapping_identity,
                id(40, MappingGrantId::from_normalized_identity),
                false,
                [activation_fact(600)],
            ))
            .expect_err("translations not installed");
        assert!(error.diagnostic().0.contains("does not establish"));
        let (pending, _) = (*error).into_parts();
        let error = pending
            .complete(TranslationActivationReceipt::from_admitted_provider(
                mapping_identity,
                id(40, MappingGrantId::from_normalized_identity),
                true,
                [],
            ))
            .expect_err("installation fact missing");
        assert!(error.diagnostic().0.contains("lacks"));
        let (pending, _) = (*error).into_parts();
        let mapping = pending
            .complete(activate(mapping_identity))
            .expect("translations installed");
        assert_eq!(mapping.rights(), &rights(&[300]));

        let pending = mapping.begin_unmap();
        let error = pending
            .complete(TranslationReleaseReceipt::from_admitted_provider(
                mapping_identity,
                id(40, MappingGrantId::from_normalized_identity),
                true,
                [],
            ))
            .expect_err("shootdown fact missing");
        assert!(error.diagnostic().0.contains("lacks"));
        let (pending, _) = (*error).into_parts();
        let (destination, source) = pending
            .complete(release(mapping_identity))
            .expect("translations released")
            .into_parts();
        assert_eq!(destination.rights(), &rights(&[200]));
        assert_eq!(
            source.expect("owned source returned").rights(),
            &rights(&[100])
        );
    }

    #[test]
    fn borrowed_mapping_retains_source_loan_and_shared_polarity() {
        let source = source();
        let source_loan = source.loan(0, 0x1000).expect("shared source");
        let mut mapping = map_borrowed(
            source_loan,
            destination(),
            mapping_id(51),
            &mapping_grant(MappingSourceMode::BorrowedShared),
        )
        .expect("borrowed map candidate")
        .complete(activate(mapping_id(51)))
        .expect("translations installed");
        assert!(mapping.loan(0, 16).is_ok());
        assert!(mapping.loan_mut(0, 16).is_err());
        let (destination, owned_source) = mapping
            .begin_unmap()
            .complete(release(mapping_id(51)))
            .expect("translations released")
            .into_parts();
        assert!(owned_source.is_none());
        assert_eq!(destination.base(), 0xffff_8000_0000_0000);
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
}
