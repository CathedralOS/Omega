//! Provider-neutral page-table construction and activation.
//!
//! Page-table bytes remain target policy. This module owns the authority
//! lifecycle around them: a provider-admitted grant accepts one table-storage
//! extent, construction accumulates already-validated pending mappings,
//! generated or scanned bytes must receive an exact construction receipt, and
//! only a later installation receipt may activate those mappings.

use std::collections::{BTreeMap, BTreeSet};

use super::*;

macro_rules! normalized_id {
    ($name:ident, $label:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            pub fn from_normalized_identity(identity: u64) -> Result<Self, ExtentDiagnostic> {
                nonzero_identity(identity, $label)?;
                Ok(Self(identity))
            }

            pub const fn normalized_identity(self) -> u64 {
                self.0
            }
        }
    };
}

normalized_id!(PageTableId, "page-table");
normalized_id!(PageTableGrantId, "page-table-grant");
normalized_id!(PageTablePlanId, "page-table-plan");
normalized_id!(PageTableContentId, "page-table-content");
normalized_id!(
    PageTableConstructionReceiptId,
    "page-table-construction-receipt"
);
normalized_id!(
    PageTableInstallationReceiptId,
    "page-table-installation-receipt"
);
normalized_id!(PageTableRemovalReceiptId, "page-table-removal-receipt");
normalized_id!(PageTableRetirementFactId, "page-table-retirement-fact");

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PageTableRetirementObligations(BTreeSet<PageTableRetirementFactId>);

impl PageTableRetirementObligations {
    pub fn from_normalized_facts(
        facts: impl IntoIterator<Item = PageTableRetirementFactId>,
    ) -> Self {
        Self(facts.into_iter().collect())
    }

    pub fn facts(&self) -> impl Iterator<Item = PageTableRetirementFactId> + '_ {
        self.0.iter().copied()
    }
}

/// Reusable provider-admitted policy for one page-table construction family.
///
/// These are requirements on authority supplied by the caller. Merely naming
/// an address space, provenance, or right never establishes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageTableGrant {
    identity: PageTableGrantId,
    storage_space: AddressSpaceId,
    storage_provenance: ExtentProvenanceId,
    required_storage_rights: ExtentRights,
    mapped_space: AddressSpaceId,
    minimum_storage_bytes: u64,
    storage_alignment: u64,
    retirement_obligations: PageTableRetirementObligations,
}

impl PageTableGrant {
    #[allow(clippy::too_many_arguments)]
    pub fn from_admitted_provider(
        identity: PageTableGrantId,
        storage_space: AddressSpaceId,
        storage_provenance: ExtentProvenanceId,
        required_storage_rights: ExtentRights,
        mapped_space: AddressSpaceId,
        minimum_storage_bytes: u64,
        storage_alignment: u64,
        retirement_obligations: PageTableRetirementObligations,
    ) -> Result<Self, ExtentDiagnostic> {
        if minimum_storage_bytes == 0 {
            return Err(ExtentDiagnostic(
                "page-table storage requirement must be nonempty".into(),
            ));
        }
        if !storage_alignment.is_power_of_two() {
            return Err(ExtentDiagnostic(
                "page-table storage alignment must be a nonzero power of two".into(),
            ));
        }
        Ok(Self {
            identity,
            storage_space,
            storage_provenance,
            required_storage_rights,
            mapped_space,
            minimum_storage_bytes,
            storage_alignment,
            retirement_obligations,
        })
    }

    pub const fn identity(&self) -> PageTableGrantId {
        self.identity
    }
}

/// Construction state that owns table storage and every pending translation.
///
/// No mapping in this state exposes access. Failed insertion returns the exact
/// pending mapping to its caller, and failed finish returns the complete draft.
#[derive(Debug)]
pub struct PageTableDraft<'source> {
    identity: PageTableId,
    grant: PageTableGrant,
    storage: Extent,
    mappings: BTreeMap<MappingId, PendingMap<'source>>,
}

pub fn begin_page_table<'source>(
    identity: PageTableId,
    grant: &PageTableGrant,
    storage: Extent,
) -> Result<PageTableDraft<'source>, Box<PageTableBeginError>> {
    let mismatch = if storage.address_space() != grant.storage_space {
        Some("page-table storage is in the wrong address space")
    } else if storage.provenance() != grant.storage_provenance {
        Some("page-table storage has the wrong provenance")
    } else if !storage.rights().contains(&grant.required_storage_rights) {
        Some("page-table storage lacks required rights")
    } else if storage.length() < grant.minimum_storage_bytes {
        Some("page-table storage is smaller than the admitted table requirement")
    } else if !storage.base().is_multiple_of(grant.storage_alignment) {
        Some("page-table storage does not satisfy the admitted alignment")
    } else {
        None
    };

    if let Some(message) = mismatch {
        return Err(Box::new(PageTableBeginError {
            storage,
            diagnostic: ExtentDiagnostic(message.into()),
        }));
    }

    Ok(PageTableDraft {
        identity,
        grant: grant.clone(),
        storage,
        mappings: BTreeMap::new(),
    })
}

impl<'source> PageTableDraft<'source> {
    pub const fn identity(&self) -> PageTableId {
        self.identity
    }

    pub const fn grant(&self) -> PageTableGrantId {
        self.grant.identity
    }

    /// Inert provider-side projections of the exact destination storage.
    /// These values let a target writer derive byte placement without
    /// borrowing, consuming, splitting, or completing the owned Extent.
    pub const fn storage_base(&self) -> u64 {
        self.storage.base()
    }

    pub const fn storage_length(&self) -> u64 {
        self.storage.length()
    }

    pub const fn storage_address_space(&self) -> AddressSpaceId {
        self.storage.address_space()
    }

    pub const fn storage_rights(&self) -> &ExtentRights {
        self.storage.rights()
    }

    pub const fn storage_provenance(&self) -> ExtentProvenanceId {
        self.storage.provenance()
    }

    pub const fn storage_era(&self) -> MappingEraId {
        self.storage.era()
    }

    pub const fn storage_lineage_root(&self) -> ExtentLineageId {
        self.storage.lineage_root()
    }

    /// Canonical provider-side view of pending translations. This exposes only
    /// inert address/fact data through borrowed pending mappings; it cannot
    /// complete a mapping, mint access, or release any owned authority.
    pub fn mappings(&self) -> impl Iterator<Item = &PendingMap<'source>> {
        self.mappings.values()
    }

    pub fn add_mapping(
        &mut self,
        mapping: PendingMap<'source>,
    ) -> Result<(), Box<PageTableMappingError<'source>>> {
        let extent = mapping.mapped_extent();
        let overlaps = self.mappings.values().any(|existing| {
            ranges_overlap(
                extent.base(),
                extent.length(),
                existing.mapped_extent().base(),
                existing.mapped_extent().length(),
            )
        });
        let mismatch = if extent.address_space() != self.grant.mapped_space {
            Some("pending mapping targets the wrong address space")
        } else if self.mappings.contains_key(&mapping.mapping()) {
            Some("page table contains a duplicate mapping identity")
        } else if overlaps {
            Some("page table contains overlapping mapped ranges")
        } else {
            None
        };

        if let Some(message) = mismatch {
            return Err(Box::new(PageTableMappingError {
                mapping,
                diagnostic: ExtentDiagnostic(message.into()),
            }));
        }

        self.mappings.insert(mapping.mapping(), mapping);
        Ok(())
    }

    pub fn plan_identity(&self) -> PageTablePlanId {
        PageTablePlanId(normalized_plan_identity(self))
    }

    pub fn finish(
        self,
        receipt: PageTableConstructionReceipt,
    ) -> Result<InstallablePageTable<'source>, Box<PageTableFinishError<'source>>> {
        let plan = self.plan_identity();
        let plan_evidence = normalized_plan_evidence(&self);
        let mismatch = if receipt.table != self.identity {
            Some("page-table construction receipt names a different table")
        } else if receipt.grant != self.grant.identity {
            Some("page-table construction receipt names a different grant")
        } else if receipt.plan != plan {
            Some("page-table construction receipt names a different normalized plan")
        } else if receipt.plan_evidence != plan_evidence {
            Some("page-table construction receipt does not bind the exact canonical plan")
        } else if !receipt.complete {
            Some("page-table construction receipt does not establish complete table bytes")
        } else {
            None
        };

        if let Some(message) = mismatch {
            return Err(Box::new(PageTableFinishError {
                draft: self,
                receipt,
                diagnostic: ExtentDiagnostic(message.into()),
            }));
        }

        Ok(InstallablePageTable {
            identity: self.identity,
            grant: self.grant,
            plan,
            content: receipt.content,
            evidence: receipt.evidence,
            construction_receipt: receipt.identity,
            storage: self.storage,
            mappings: self.mappings,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageTableConstructionEvidence {
    Generated,
    ImportedScan,
}

/// Accepted evidence that the target-specific table bytes represent one exact
/// normalized mapping set.
#[derive(Debug, PartialEq, Eq)]
pub struct PageTableConstructionReceipt {
    identity: PageTableConstructionReceiptId,
    table: PageTableId,
    grant: PageTableGrantId,
    plan: PageTablePlanId,
    plan_evidence: Vec<u64>,
    content: PageTableContentId,
    evidence: PageTableConstructionEvidence,
    complete: bool,
}

impl PageTableConstructionReceipt {
    pub fn from_admitted_provider(
        identity: PageTableConstructionReceiptId,
        draft: &PageTableDraft<'_>,
        content: PageTableContentId,
        evidence: PageTableConstructionEvidence,
        complete: bool,
    ) -> Self {
        Self {
            identity,
            table: draft.identity,
            grant: draft.grant.identity,
            plan: draft.plan_identity(),
            plan_evidence: normalized_plan_evidence(draft),
            content,
            evidence,
            complete,
        }
    }
}

/// Established page-table bytes that are structurally ready for installation.
///
/// This is the normalized `Installable` state. It still exposes no mapped
/// access and grants no page-table control authority.
#[derive(Debug)]
pub struct InstallablePageTable<'source> {
    identity: PageTableId,
    grant: PageTableGrant,
    plan: PageTablePlanId,
    content: PageTableContentId,
    evidence: PageTableConstructionEvidence,
    construction_receipt: PageTableConstructionReceiptId,
    storage: Extent,
    mappings: BTreeMap<MappingId, PendingMap<'source>>,
}

impl<'source> InstallablePageTable<'source> {
    pub const fn identity(&self) -> PageTableId {
        self.identity
    }

    pub const fn plan(&self) -> PageTablePlanId {
        self.plan
    }

    pub const fn content(&self) -> PageTableContentId {
        self.content
    }

    pub const fn evidence(&self) -> PageTableConstructionEvidence {
        self.evidence
    }

    pub fn install(
        self,
        mut receipt: PageTableInstallationReceipt,
    ) -> Result<InstalledPageTable<'source>, Box<PageTableInstallError<'source>>> {
        let expected = self.mappings.keys().copied().collect::<BTreeSet<_>>();
        let actual = receipt.activations.keys().copied().collect::<BTreeSet<_>>();
        let mismatch = if receipt.table != self.identity {
            Some("page-table installation receipt names a different table".into())
        } else if receipt.grant != self.grant.identity {
            Some("page-table installation receipt names a different grant".into())
        } else if receipt.plan != self.plan {
            Some("page-table installation receipt names a different normalized plan".into())
        } else if receipt.content != self.content {
            Some("page-table installation receipt names different table content".into())
        } else if receipt.construction_receipt != self.construction_receipt {
            Some("page-table installation receipt names different construction evidence".into())
        } else if !receipt.active {
            Some("page-table installation receipt does not establish active translations".into())
        } else if expected != actual {
            Some("page-table installation receipt does not cover the exact mapping set".into())
        } else {
            self.mappings.iter().find_map(|(identity, pending)| {
                pending
                    .validate_activation_receipt(
                        receipt
                            .activations
                            .get(identity)
                            .expect("exact key sets were validated"),
                    )
                    .err()
                    .map(|diagnostic| diagnostic.0)
            })
        };

        if let Some(message) = mismatch {
            return Err(Box::new(PageTableInstallError {
                table: self,
                receipt,
                diagnostic: ExtentDiagnostic(message),
            }));
        }

        let mut mappings = BTreeMap::new();
        for (identity, pending) in self.mappings {
            let activation = receipt
                .activations
                .remove(&identity)
                .expect("exact key sets were validated");
            let mapping = pending
                .complete(activation)
                .expect("activation receipts were prevalidated");
            mappings.insert(identity, mapping);
        }

        let grant = self.grant.identity;
        let retirement_obligations = self.grant.retirement_obligations;
        Ok(InstalledPageTable {
            identity: self.identity,
            grant,
            plan: self.plan,
            content: self.content,
            installation_receipt: receipt.identity,
            retirement_obligations,
            storage: self.storage,
            mappings,
        })
    }
}

#[derive(Debug)]
pub struct PageTableInstallationReceipt {
    identity: PageTableInstallationReceiptId,
    table: PageTableId,
    grant: PageTableGrantId,
    plan: PageTablePlanId,
    content: PageTableContentId,
    construction_receipt: PageTableConstructionReceiptId,
    active: bool,
    activations: BTreeMap<MappingId, TranslationActivationReceipt>,
}

impl PageTableInstallationReceipt {
    #[allow(clippy::too_many_arguments)]
    pub fn from_admitted_provider(
        identity: PageTableInstallationReceiptId,
        table: PageTableId,
        grant: PageTableGrantId,
        plan: PageTablePlanId,
        content: PageTableContentId,
        construction_receipt: PageTableConstructionReceiptId,
        active: bool,
        activations: impl IntoIterator<Item = (MappingId, TranslationActivationReceipt)>,
    ) -> Result<Self, ExtentDiagnostic> {
        let mut normalized = BTreeMap::new();
        for (mapping, receipt) in activations {
            if normalized.insert(mapping, receipt).is_some() {
                return Err(ExtentDiagnostic(
                    "page-table installation receipt repeats a mapping".into(),
                ));
            }
        }
        Ok(Self {
            identity,
            table,
            grant,
            plan,
            content,
            construction_receipt,
            active,
            activations: normalized,
        })
    }
}

/// Active table plus the mapped authorities whose translations it established.
#[derive(Debug)]
pub struct InstalledPageTable<'source> {
    identity: PageTableId,
    grant: PageTableGrantId,
    plan: PageTablePlanId,
    content: PageTableContentId,
    installation_receipt: PageTableInstallationReceiptId,
    retirement_obligations: PageTableRetirementObligations,
    storage: Extent,
    mappings: BTreeMap<MappingId, MappedExtent<'source>>,
}

impl<'source> InstalledPageTable<'source> {
    pub const fn identity(&self) -> PageTableId {
        self.identity
    }

    pub const fn grant(&self) -> PageTableGrantId {
        self.grant
    }

    pub const fn plan(&self) -> PageTablePlanId {
        self.plan
    }

    pub const fn content(&self) -> PageTableContentId {
        self.content
    }

    pub const fn installation_receipt(&self) -> PageTableInstallationReceiptId {
        self.installation_receipt
    }

    pub const fn storage(&self) -> &Extent {
        &self.storage
    }

    pub fn mapping(&self, identity: MappingId) -> Option<&MappedExtent<'source>> {
        self.mappings.get(&identity)
    }

    pub fn mapping_mut(&mut self, identity: MappingId) -> Option<&mut MappedExtent<'source>> {
        self.mappings.get_mut(&identity)
    }

    pub fn begin_removal(self) -> PendingPageTableRemoval<'source> {
        PendingPageTableRemoval {
            identity: self.identity,
            grant: self.grant,
            plan: self.plan,
            content: self.content,
            installation_receipt: self.installation_receipt,
            retirement_obligations: self.retirement_obligations,
            storage: self.storage,
            mappings: self
                .mappings
                .into_iter()
                .map(|(identity, mapping)| (identity, mapping.begin_unmap()))
                .collect(),
        }
    }
}

/// Linear state after the page-table provider begins retiring the table.
///
/// Storage and every mapping remain captive until the provider proves the
/// table inactive, discharges target retirement facts, and supplies each
/// mapping's exact stale-translation release receipt.
#[derive(Debug)]
pub struct PendingPageTableRemoval<'source> {
    identity: PageTableId,
    grant: PageTableGrantId,
    plan: PageTablePlanId,
    content: PageTableContentId,
    installation_receipt: PageTableInstallationReceiptId,
    retirement_obligations: PageTableRetirementObligations,
    storage: Extent,
    mappings: BTreeMap<MappingId, PendingUnmap<'source>>,
}

impl<'source> PendingPageTableRemoval<'source> {
    pub const fn identity(&self) -> PageTableId {
        self.identity
    }

    pub fn complete(
        self,
        mut receipt: PageTableRemovalReceipt,
    ) -> Result<RemovedPageTable, Box<PageTableRemovalError<'source>>> {
        let expected = self.mappings.keys().copied().collect::<BTreeSet<_>>();
        let actual = receipt.releases.keys().copied().collect::<BTreeSet<_>>();
        let mismatch = if receipt.table != self.identity {
            Some("page-table removal receipt names a different table".into())
        } else if receipt.grant != self.grant {
            Some("page-table removal receipt names a different grant".into())
        } else if receipt.plan != self.plan {
            Some("page-table removal receipt names a different normalized plan".into())
        } else if receipt.content != self.content {
            Some("page-table removal receipt names different table content".into())
        } else if receipt.installation_receipt != self.installation_receipt {
            Some("page-table removal receipt names a different installation".into())
        } else if !receipt.inactive {
            Some("page-table removal receipt does not establish an inactive table".into())
        } else if !self
            .retirement_obligations
            .0
            .is_subset(&receipt.established_facts)
        {
            Some("page-table removal receipt lacks required retirement facts".into())
        } else if expected != actual {
            Some("page-table removal receipt does not cover the exact mapping set".into())
        } else {
            self.mappings.iter().find_map(|(identity, pending)| {
                pending
                    .validate_release_receipt(
                        receipt
                            .releases
                            .get(identity)
                            .expect("exact key sets were validated"),
                    )
                    .err()
                    .map(|diagnostic| diagnostic.0)
            })
        };

        if let Some(message) = mismatch {
            return Err(Box::new(PageTableRemovalError {
                pending: self,
                receipt,
                diagnostic: ExtentDiagnostic(message),
            }));
        }

        let mut mappings = BTreeMap::new();
        for (identity, pending) in self.mappings {
            let release = receipt
                .releases
                .remove(&identity)
                .expect("exact key sets were validated");
            let extents = pending
                .complete(release)
                .expect("translation release receipts were prevalidated");
            mappings.insert(identity, extents);
        }
        Ok(RemovedPageTable {
            table: self.identity,
            removal_receipt: receipt.identity,
            storage: self.storage,
            mappings,
        })
    }
}

#[derive(Debug)]
pub struct PageTableRemovalReceipt {
    identity: PageTableRemovalReceiptId,
    table: PageTableId,
    grant: PageTableGrantId,
    plan: PageTablePlanId,
    content: PageTableContentId,
    installation_receipt: PageTableInstallationReceiptId,
    inactive: bool,
    established_facts: BTreeSet<PageTableRetirementFactId>,
    releases: BTreeMap<MappingId, TranslationReleaseReceipt>,
}

impl PageTableRemovalReceipt {
    #[allow(clippy::too_many_arguments)]
    pub fn from_admitted_provider(
        identity: PageTableRemovalReceiptId,
        table: PageTableId,
        grant: PageTableGrantId,
        plan: PageTablePlanId,
        content: PageTableContentId,
        installation_receipt: PageTableInstallationReceiptId,
        inactive: bool,
        established_facts: impl IntoIterator<Item = PageTableRetirementFactId>,
        releases: impl IntoIterator<Item = (MappingId, TranslationReleaseReceipt)>,
    ) -> Result<Self, ExtentDiagnostic> {
        let mut normalized = BTreeMap::new();
        for (mapping, receipt) in releases {
            if normalized.insert(mapping, receipt).is_some() {
                return Err(ExtentDiagnostic(
                    "page-table removal receipt repeats a mapping".into(),
                ));
            }
        }
        Ok(Self {
            identity,
            table,
            grant,
            plan,
            content,
            installation_receipt,
            inactive,
            established_facts: established_facts.into_iter().collect(),
            releases: normalized,
        })
    }
}

#[derive(Debug)]
pub struct RemovedPageTable {
    table: PageTableId,
    removal_receipt: PageTableRemovalReceiptId,
    storage: Extent,
    mappings: BTreeMap<MappingId, UnmappedExtents>,
}

impl RemovedPageTable {
    pub const fn table(&self) -> PageTableId {
        self.table
    }

    pub const fn removal_receipt(&self) -> PageTableRemovalReceiptId {
        self.removal_receipt
    }

    pub fn into_parts(self) -> (Extent, BTreeMap<MappingId, UnmappedExtents>) {
        (self.storage, self.mappings)
    }
}

#[derive(Debug)]
pub struct PageTableBeginError {
    storage: Extent,
    diagnostic: ExtentDiagnostic,
}

impl PageTableBeginError {
    pub const fn diagnostic(&self) -> &ExtentDiagnostic {
        &self.diagnostic
    }

    pub fn into_storage(self) -> Extent {
        self.storage
    }
}

#[derive(Debug)]
pub struct PageTableMappingError<'source> {
    mapping: PendingMap<'source>,
    diagnostic: ExtentDiagnostic,
}

impl<'source> PageTableMappingError<'source> {
    pub const fn diagnostic(&self) -> &ExtentDiagnostic {
        &self.diagnostic
    }

    pub fn into_mapping(self) -> PendingMap<'source> {
        self.mapping
    }
}

#[derive(Debug)]
pub struct PageTableFinishError<'source> {
    draft: PageTableDraft<'source>,
    receipt: PageTableConstructionReceipt,
    diagnostic: ExtentDiagnostic,
}

impl<'source> PageTableFinishError<'source> {
    pub const fn diagnostic(&self) -> &ExtentDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (PageTableDraft<'source>, PageTableConstructionReceipt) {
        (self.draft, self.receipt)
    }
}

#[derive(Debug)]
pub struct PageTableInstallError<'source> {
    table: InstallablePageTable<'source>,
    receipt: PageTableInstallationReceipt,
    diagnostic: ExtentDiagnostic,
}

impl<'source> PageTableInstallError<'source> {
    pub const fn diagnostic(&self) -> &ExtentDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (InstallablePageTable<'source>, PageTableInstallationReceipt) {
        (self.table, self.receipt)
    }
}

#[derive(Debug)]
pub struct PageTableRemovalError<'source> {
    pending: PendingPageTableRemoval<'source>,
    receipt: PageTableRemovalReceipt,
    diagnostic: ExtentDiagnostic,
}

impl<'source> PageTableRemovalError<'source> {
    pub const fn diagnostic(&self) -> &ExtentDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (PendingPageTableRemoval<'source>, PageTableRemovalReceipt) {
        (self.pending, self.receipt)
    }
}

fn ranges_overlap(left_base: u64, left_length: u64, right_base: u64, right_length: u64) -> bool {
    left_base < right_base + right_length && right_base < left_base + left_length
}

fn normalized_plan_evidence(draft: &PageTableDraft<'_>) -> Vec<u64> {
    let mut values = Vec::new();
    values.push(draft.identity.normalized_identity());
    values.push(draft.grant.identity.normalized_identity());
    values.push(draft.storage.address_space().normalized_identity());
    values.push(draft.storage.provenance().normalized_identity());
    values.push(draft.storage.era().normalized_identity());
    values.push(draft.storage.lineage_root().normalized_identity());
    values.push(draft.storage.base());
    values.push(draft.storage.length());
    values.push(draft.storage.rights().identities().count() as u64);
    values.extend(
        draft
            .storage
            .rights()
            .identities()
            .map(ExtentRightId::normalized_identity),
    );
    values.push(draft.mappings.len() as u64);
    for (identity, pending) in &draft.mappings {
        let mapped = pending.mapped_extent();
        values.push(identity.normalized_identity());
        values.push(pending.grant().normalized_identity());
        values.push(match pending.source_mode() {
            MappingSourceMode::Owned => 1,
            MappingSourceMode::BorrowedShared => 2,
            MappingSourceMode::BorrowedExclusive => 3,
        });
        values.push(pending.source_address_space().normalized_identity());
        values.push(pending.source_provenance().normalized_identity());
        values.push(pending.source_era().normalized_identity());
        values.push(pending.source_lineage_root().normalized_identity());
        values.push(pending.source_base());
        values.push(pending.source_length());
        values.push(pending.source_rights().identities().count() as u64);
        values.extend(
            pending
                .source_rights()
                .identities()
                .map(ExtentRightId::normalized_identity),
        );
        values.push(mapped.address_space().normalized_identity());
        values.push(mapped.provenance().normalized_identity());
        values.push(mapped.era().normalized_identity());
        values.push(mapped.lineage_root().normalized_identity());
        values.push(mapped.base());
        values.push(mapped.length());
        values.push(mapped.rights().identities().count() as u64);
        values.extend(
            mapped
                .rights()
                .identities()
                .map(ExtentRightId::normalized_identity),
        );
        values.push(
            pending
                .destination_restoration_provenance()
                .normalized_identity(),
        );
        values.push(pending.destination_restoration_era().normalized_identity());
        values.push(
            pending
                .destination_restoration_rights()
                .identities()
                .count() as u64,
        );
        values.extend(
            pending
                .destination_restoration_rights()
                .identities()
                .map(ExtentRightId::normalized_identity),
        );
    }
    values
}

fn normalized_plan_identity(draft: &PageTableDraft<'_>) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    fn mix(hash: &mut u64, value: u64) {
        *hash ^= value;
        *hash = hash.wrapping_mul(PRIME);
    }

    let mut hash = OFFSET_BASIS;
    for value in normalized_plan_evidence(draft) {
        mix(&mut hash, value);
    }
    if hash == 0 { 1 } else { hash }
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

    fn table_grant() -> PageTableGrant {
        PageTableGrant::from_admitted_provider(
            id(10, PageTableGrantId::from_normalized_identity),
            id(1, AddressSpaceId::from_normalized_identity),
            id(2, ExtentProvenanceId::from_normalized_identity),
            rights(&[3]),
            id(20, AddressSpaceId::from_normalized_identity),
            4096,
            4096,
            PageTableRetirementObligations::from_normalized_facts([id(
                90,
                PageTableRetirementFactId::from_normalized_identity,
            )]),
        )
        .expect("page-table grant")
    }

    fn mapping_grant(mode: MappingSourceMode) -> MappingGrant {
        MappingGrant::from_admitted_provider(
            id(40, MappingGrantId::from_normalized_identity),
            mode,
            id(30, AddressSpaceId::from_normalized_identity),
            id(20, AddressSpaceId::from_normalized_identity),
            rights(&[31]),
            rights(&[21]),
            rights(&[22]),
            id(23, ExtentProvenanceId::from_normalized_identity),
            id(24, MappingEraId::from_normalized_identity),
            TranslationInstallObligations::from_normalized_facts([id(
                25,
                TranslationActivationFactId::from_normalized_identity,
            )]),
            TranslationReleaseObligations::from_normalized_facts([id(
                26,
                TranslationCompletionFactId::from_normalized_identity,
            )]),
        )
    }

    fn pending_mapping(identity: u64, base: u64) -> PendingMap<'static> {
        map_owned(
            extent(100 + identity, 0x1000_0000 + base, 4096, 30, 32, &[31]),
            extent(200 + identity, base, 4096, 20, 33, &[21]),
            id(identity, MappingId::from_normalized_identity),
            &mapping_grant(MappingSourceMode::Owned),
        )
        .expect("pending mapping")
    }

    fn pending_mapping_with_authority(
        identity: u64,
        source_base: u64,
        destination_base: u64,
        destination_rights: &[u64],
    ) -> PendingMap<'static> {
        map_owned(
            extent(100 + identity, source_base, 4096, 30, 32, &[31]),
            extent(
                200 + identity,
                destination_base,
                4096,
                20,
                33,
                destination_rights,
            ),
            id(identity, MappingId::from_normalized_identity),
            &mapping_grant(MappingSourceMode::Owned),
        )
        .expect("pending mapping with exact authority")
    }

    fn activation(identity: u64) -> TranslationActivationReceipt {
        TranslationActivationReceipt::from_admitted_provider(
            id(identity, MappingId::from_normalized_identity),
            id(40, MappingGrantId::from_normalized_identity),
            true,
            [id(
                25,
                TranslationActivationFactId::from_normalized_identity,
            )],
        )
    }

    fn release(identity: u64) -> TranslationReleaseReceipt {
        TranslationReleaseReceipt::from_admitted_provider(
            id(identity, MappingId::from_normalized_identity),
            id(40, MappingGrantId::from_normalized_identity),
            true,
            [id(
                26,
                TranslationCompletionFactId::from_normalized_identity,
            )],
        )
    }

    fn construction_receipt(
        draft: &PageTableDraft<'_>,
        complete: bool,
    ) -> PageTableConstructionReceipt {
        PageTableConstructionReceipt::from_admitted_provider(
            id(60, PageTableConstructionReceiptId::from_normalized_identity),
            draft,
            id(61, PageTableContentId::from_normalized_identity),
            PageTableConstructionEvidence::Generated,
            complete,
        )
    }

    fn installation_receipt(
        table: &InstallablePageTable<'_>,
        active: bool,
        activations: impl IntoIterator<Item = (MappingId, TranslationActivationReceipt)>,
    ) -> PageTableInstallationReceipt {
        PageTableInstallationReceipt::from_admitted_provider(
            id(70, PageTableInstallationReceiptId::from_normalized_identity),
            table.identity(),
            table.grant.identity,
            table.plan(),
            table.content(),
            table.construction_receipt,
            active,
            activations,
        )
        .expect("installation receipt")
    }

    fn draft() -> PageTableDraft<'static> {
        begin_page_table(
            id(50, PageTableId::from_normalized_identity),
            &table_grant(),
            extent(1, 0x4000, 4096, 1, 2, &[3]),
        )
        .expect("page-table draft")
    }

    #[test]
    fn generated_table_requires_exact_construction_and_installation_receipts() {
        let mut draft = draft();
        draft
            .add_mapping(pending_mapping(51, 0x8000))
            .expect("first mapping");
        draft
            .add_mapping(pending_mapping(52, 0x9000))
            .expect("second mapping");

        let incomplete = construction_receipt(&draft, false);
        let error = draft
            .finish(incomplete)
            .expect_err("incomplete bytes cannot establish Installable");
        assert!(error.diagnostic().0.contains("complete table bytes"));
        let (draft, _) = (*error).into_parts();

        let receipt = construction_receipt(&draft, true);
        let installable = draft.finish(receipt).expect("installable table");
        let inactive = installation_receipt(
            &installable,
            false,
            [(id(51, MappingId::from_normalized_identity), activation(51))],
        );
        let error = installable
            .install(inactive)
            .expect_err("inactive receipt cannot expose mappings");
        assert!(error.diagnostic().0.contains("active translations"));
        let (installable, _) = (*error).into_parts();

        let incomplete = installation_receipt(
            &installable,
            true,
            [(id(51, MappingId::from_normalized_identity), activation(51))],
        );
        let error = installable
            .install(incomplete)
            .expect_err("partial activation cannot expose mappings");
        assert!(error.diagnostic().0.contains("exact mapping set"));
        let (installable, _) = (*error).into_parts();

        let receipt = installation_receipt(
            &installable,
            true,
            [
                (id(51, MappingId::from_normalized_identity), activation(51)),
                (id(52, MappingId::from_normalized_identity), activation(52)),
            ],
        );
        let installed = installable.install(receipt).expect("installed page table");
        assert!(
            installed
                .mapping(id(51, MappingId::from_normalized_identity))
                .is_some()
        );
        assert!(
            installed
                .mapping(id(52, MappingId::from_normalized_identity))
                .is_some()
        );
    }

    #[test]
    fn construction_rejects_overlaps_and_returns_the_pending_authority() {
        let mut draft = draft();
        draft
            .add_mapping(pending_mapping(51, 0x8000))
            .expect("first mapping");
        let error = draft
            .add_mapping(pending_mapping(52, 0x8800))
            .expect_err("overlapping mapping");
        assert!(error.diagnostic().0.contains("overlapping"));
        assert_eq!(
            (*error).into_mapping().mapping(),
            id(52, MappingId::from_normalized_identity)
        );
    }

    #[test]
    fn imported_scan_establishes_the_same_installable_state() {
        let mut draft = draft();
        draft
            .add_mapping(pending_mapping(51, 0x8000))
            .expect("mapping");
        let receipt = PageTableConstructionReceipt::from_admitted_provider(
            id(60, PageTableConstructionReceiptId::from_normalized_identity),
            &draft,
            id(61, PageTableContentId::from_normalized_identity),
            PageTableConstructionEvidence::ImportedScan,
            true,
        );
        let installable = draft.finish(receipt).expect("scanned installable table");
        assert_eq!(
            installable.evidence(),
            PageTableConstructionEvidence::ImportedScan
        );
    }

    #[test]
    fn construction_receipt_retains_exact_plan_evidence_beyond_the_compact_identity() {
        let mut draft = draft();
        draft
            .add_mapping(pending_mapping(51, 0x8000))
            .expect("mapping");
        let mut receipt = construction_receipt(&draft, true);
        let compact_plan = receipt.plan;
        receipt.plan_evidence[4] ^= 1;
        assert_eq!(
            receipt.plan, compact_plan,
            "the negative canary deliberately preserves the compact plan identity"
        );

        let error = draft
            .finish(receipt)
            .expect_err("exact canonical-plan substitution must reject");
        assert!(error.diagnostic().0.contains("exact canonical plan"));
    }

    #[test]
    fn normalized_plan_identity_is_order_independent_but_authority_bound() {
        let mut left = draft();
        left.add_mapping(pending_mapping(51, 0x8000))
            .expect("first mapping");
        left.add_mapping(pending_mapping(52, 0x9000))
            .expect("second mapping");

        let mut right = draft();
        right
            .add_mapping(pending_mapping(52, 0x9000))
            .expect("second mapping first");
        right
            .add_mapping(pending_mapping(51, 0x8000))
            .expect("first mapping second");
        assert_eq!(left.plan_identity(), right.plan_identity());

        let mut different_storage = begin_page_table(
            id(50, PageTableId::from_normalized_identity),
            &table_grant(),
            extent(99, 0x4000, 4096, 1, 2, &[3]),
        )
        .expect("different storage lineage");
        different_storage
            .add_mapping(pending_mapping(51, 0x8000))
            .expect("first mapping");
        different_storage
            .add_mapping(pending_mapping(52, 0x9000))
            .expect("second mapping");
        assert_ne!(left.plan_identity(), different_storage.plan_identity());
    }

    #[test]
    fn normalized_plan_identity_binds_source_and_restored_destination_authority() {
        let mut baseline = draft();
        baseline
            .add_mapping(pending_mapping_with_authority(
                51,
                0x1000_8000,
                0x8000,
                &[21],
            ))
            .expect("baseline mapping");

        let mut different_source = draft();
        different_source
            .add_mapping(pending_mapping_with_authority(
                51,
                0x2000_8000,
                0x8000,
                &[21],
            ))
            .expect("different physical source");
        assert_ne!(
            baseline.plan_identity(),
            different_source.plan_identity(),
            "mapping a different physical source must change normalized plan identity"
        );

        let mut different_restoration = draft();
        different_restoration
            .add_mapping(pending_mapping_with_authority(
                51,
                0x1000_8000,
                0x8000,
                &[21, 99],
            ))
            .expect("destination with additional restorable authority");
        assert_ne!(
            baseline.plan_identity(),
            different_restoration.plan_identity(),
            "authority returned by teardown must participate in plan identity"
        );
    }

    #[test]
    fn normalized_plan_identity_binds_table_storage_mapping_era() {
        let storage = ExtentRootGrant::from_admitted_provider(
            id(1, ExtentLineageId::from_normalized_identity),
            id(1, AddressSpaceId::from_normalized_identity),
            rights(&[3]),
            id(2, ExtentProvenanceId::from_normalized_identity),
            id(30, MappingEraId::from_normalized_identity),
        )
        .mint(0x8000, 4096)
        .expect("first table storage");
        let first = begin_page_table(
            id(1, PageTableId::from_normalized_identity),
            &table_grant(),
            storage,
        )
        .expect("first page-table draft");

        let storage = ExtentRootGrant::from_admitted_provider(
            id(1, ExtentLineageId::from_normalized_identity),
            id(1, AddressSpaceId::from_normalized_identity),
            rights(&[3]),
            id(2, ExtentProvenanceId::from_normalized_identity),
            id(31, MappingEraId::from_normalized_identity),
        )
        .mint(0x8000, 4096)
        .expect("replacement table storage");
        let replacement = begin_page_table(
            id(1, PageTableId::from_normalized_identity),
            &table_grant(),
            storage,
        )
        .expect("replacement page-table draft");

        assert_ne!(
            first.plan_identity(),
            replacement.plan_identity(),
            "construction evidence must not replay after table-storage era drift"
        );
    }

    #[test]
    fn provider_projection_exposes_exact_mapping_data_without_releasing_authority() {
        let mut draft = draft();
        draft
            .add_mapping(pending_mapping_with_authority(
                51,
                0x1234_5000,
                0x8000,
                &[21, 99],
            ))
            .expect("mapping");

        assert_eq!(draft.storage_base(), 0x4000);
        assert_eq!(draft.storage_length(), 4096);
        assert_eq!(draft.storage_address_space().normalized_identity(), 1);
        assert_eq!(draft.storage_provenance().normalized_identity(), 2);
        assert_eq!(draft.storage_era().normalized_identity(), 30);
        assert_eq!(draft.storage_lineage_root().normalized_identity(), 1);
        assert_eq!(
            draft
                .storage_rights()
                .identities()
                .map(ExtentRightId::normalized_identity)
                .collect::<Vec<_>>(),
            vec![3]
        );

        let mapping = draft.mappings().next().expect("provider projection");
        assert_eq!(mapping.mapping().normalized_identity(), 51);
        assert_eq!(mapping.source_mode(), MappingSourceMode::Owned);
        assert_eq!(mapping.source_base(), 0x1234_5000);
        assert_eq!(mapping.source_length(), 4096);
        assert_eq!(mapping.destination_base(), 0x8000);
        assert_eq!(mapping.destination_length(), 4096);
        assert_eq!(
            mapping
                .mapped_rights()
                .identities()
                .map(ExtentRightId::normalized_identity)
                .collect::<Vec<_>>(),
            vec![22]
        );
        assert_eq!(
            mapping
                .destination_restoration_rights()
                .identities()
                .map(ExtentRightId::normalized_identity)
                .collect::<Vec<_>>(),
            vec![21, 99]
        );

        let receipt = construction_receipt(&draft, true);
        draft
            .finish(receipt)
            .expect("borrowed projection did not consume pending authority");
    }

    #[test]
    fn table_construction_retains_a_borrowed_mapping_source() {
        let source = extent(400, 0x2000_0000, 4096, 30, 32, &[31]);
        let pending = map_borrowed(
            source.loan(0, 4096).expect("source loan"),
            extent(401, 0xa000, 4096, 20, 33, &[21]),
            id(53, MappingId::from_normalized_identity),
            &mapping_grant(MappingSourceMode::BorrowedShared),
        )
        .expect("borrowed pending mapping");
        let mut draft = begin_page_table(
            id(50, PageTableId::from_normalized_identity),
            &table_grant(),
            extent(1, 0x4000, 4096, 1, 2, &[3]),
        )
        .expect("page-table draft");
        draft
            .add_mapping(pending)
            .expect("borrowed mapping belongs to the draft");
        assert_ne!(draft.plan_identity().normalized_identity(), 0);
    }

    #[test]
    fn installed_table_releases_authority_only_after_exact_retirement() {
        let mut draft = draft();
        draft
            .add_mapping(pending_mapping(51, 0x8000))
            .expect("mapping");
        let construction = construction_receipt(&draft, true);
        let installable = draft.finish(construction).expect("installable");
        let installation = installation_receipt(
            &installable,
            true,
            [(id(51, MappingId::from_normalized_identity), activation(51))],
        );
        let installed = installable.install(installation).expect("installed");
        let pending = installed.begin_removal();

        let receipt = PageTableRemovalReceipt::from_admitted_provider(
            id(80, PageTableRemovalReceiptId::from_normalized_identity),
            pending.identity,
            pending.grant,
            pending.plan,
            pending.content,
            pending.installation_receipt,
            false,
            [id(90, PageTableRetirementFactId::from_normalized_identity)],
            [(id(51, MappingId::from_normalized_identity), release(51))],
        )
        .expect("inactive-negative receipt");
        let error = pending
            .complete(receipt)
            .expect_err("a live table retains every authority");
        assert!(error.diagnostic().0.contains("inactive table"));
        let (pending, _) = (*error).into_parts();

        let receipt = PageTableRemovalReceipt::from_admitted_provider(
            id(80, PageTableRemovalReceiptId::from_normalized_identity),
            pending.identity,
            pending.grant,
            pending.plan,
            pending.content,
            pending.installation_receipt,
            true,
            [],
            [(id(51, MappingId::from_normalized_identity), release(51))],
        )
        .expect("missing-fact receipt");
        let error = pending
            .complete(receipt)
            .expect_err("retirement facts are mandatory");
        assert!(error.diagnostic().0.contains("retirement facts"));
        let (pending, _) = (*error).into_parts();

        let receipt = PageTableRemovalReceipt::from_admitted_provider(
            id(80, PageTableRemovalReceiptId::from_normalized_identity),
            pending.identity,
            pending.grant,
            pending.plan,
            pending.content,
            pending.installation_receipt,
            true,
            [id(90, PageTableRetirementFactId::from_normalized_identity)],
            [],
        )
        .expect("missing-release receipt");
        let error = pending
            .complete(receipt)
            .expect_err("every mapping needs a release receipt");
        assert!(error.diagnostic().0.contains("exact mapping set"));
        let (pending, _) = (*error).into_parts();

        let receipt = PageTableRemovalReceipt::from_admitted_provider(
            id(80, PageTableRemovalReceiptId::from_normalized_identity),
            pending.identity,
            pending.grant,
            pending.plan,
            pending.content,
            pending.installation_receipt,
            true,
            [id(90, PageTableRetirementFactId::from_normalized_identity)],
            [(id(51, MappingId::from_normalized_identity), release(51))],
        )
        .expect("exact removal receipt");
        let removed = pending.complete(receipt).expect("removed table");
        let (storage, mappings) = removed.into_parts();
        assert_eq!(storage.base(), 0x4000);
        let (destination, source) = mappings
            .into_iter()
            .next()
            .expect("released mapping")
            .1
            .into_parts();
        assert_eq!(destination.base(), 0x8000);
        assert!(source.is_some());
    }
}
