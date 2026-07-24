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
        let mapping_ids = self.mappings.keys().copied().collect::<BTreeSet<_>>();
        let mismatch = if receipt.table != self.identity {
            Some("page-table construction receipt names a different table")
        } else if receipt.grant != self.grant.identity {
            Some("page-table construction receipt names a different grant")
        } else if receipt.plan != plan {
            Some("page-table construction receipt names a different normalized plan")
        } else if receipt.mappings != mapping_ids {
            Some("page-table construction receipt does not cover the exact mapping set")
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
    content: PageTableContentId,
    evidence: PageTableConstructionEvidence,
    mappings: BTreeSet<MappingId>,
    complete: bool,
}

impl PageTableConstructionReceipt {
    #[allow(clippy::too_many_arguments)]
    pub fn from_admitted_provider(
        identity: PageTableConstructionReceiptId,
        table: PageTableId,
        grant: PageTableGrantId,
        plan: PageTablePlanId,
        content: PageTableContentId,
        evidence: PageTableConstructionEvidence,
        mappings: impl IntoIterator<Item = MappingId>,
        complete: bool,
    ) -> Self {
        Self {
            identity,
            table,
            grant,
            plan,
            content,
            evidence,
            mappings: mappings.into_iter().collect(),
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

        Ok(InstalledPageTable {
            identity: self.identity,
            grant: self.grant.identity,
            plan: self.plan,
            content: self.content,
            installation_receipt: receipt.identity,
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

fn ranges_overlap(left_base: u64, left_length: u64, right_base: u64, right_length: u64) -> bool {
    left_base < right_base + right_length && right_base < left_base + left_length
}

fn normalized_plan_identity(draft: &PageTableDraft<'_>) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    fn mix(hash: &mut u64, value: u64) {
        *hash ^= value;
        *hash = hash.wrapping_mul(PRIME);
    }

    let mut hash = OFFSET_BASIS;
    mix(&mut hash, draft.identity.normalized_identity());
    mix(&mut hash, draft.grant.identity.normalized_identity());
    mix(
        &mut hash,
        draft.storage.address_space().normalized_identity(),
    );
    mix(&mut hash, draft.storage.provenance().normalized_identity());
    mix(
        &mut hash,
        draft.storage.lineage_root().normalized_identity(),
    );
    mix(&mut hash, draft.storage.base());
    mix(&mut hash, draft.storage.length());
    mix(
        &mut hash,
        draft.storage.rights().identities().count() as u64,
    );
    for right in draft.storage.rights().identities() {
        mix(&mut hash, right.normalized_identity());
    }
    mix(&mut hash, draft.mappings.len() as u64);
    for (identity, pending) in &draft.mappings {
        let mapped = pending.mapped_extent();
        mix(&mut hash, identity.normalized_identity());
        mix(&mut hash, pending.grant().normalized_identity());
        mix(&mut hash, mapped.address_space().normalized_identity());
        mix(&mut hash, mapped.provenance().normalized_identity());
        mix(&mut hash, mapped.era().normalized_identity());
        mix(&mut hash, mapped.lineage_root().normalized_identity());
        mix(&mut hash, mapped.base());
        mix(&mut hash, mapped.length());
        mix(&mut hash, mapped.rights().identities().count() as u64);
        for right in mapped.rights().identities() {
            mix(&mut hash, right.normalized_identity());
        }
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
            TranslationReleaseObligations::default(),
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

    fn construction_receipt(
        draft: &PageTableDraft<'_>,
        complete: bool,
    ) -> PageTableConstructionReceipt {
        PageTableConstructionReceipt::from_admitted_provider(
            id(60, PageTableConstructionReceiptId::from_normalized_identity),
            draft.identity(),
            draft.grant(),
            draft.plan_identity(),
            id(61, PageTableContentId::from_normalized_identity),
            PageTableConstructionEvidence::Generated,
            draft.mappings.keys().copied(),
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
            draft.identity(),
            draft.grant(),
            draft.plan_identity(),
            id(61, PageTableContentId::from_normalized_identity),
            PageTableConstructionEvidence::ImportedScan,
            [id(51, MappingId::from_normalized_identity)],
            true,
        );
        let installable = draft.finish(receipt).expect("scanned installable table");
        assert_eq!(
            installable.evidence(),
            PageTableConstructionEvidence::ImportedScan
        );
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
}
