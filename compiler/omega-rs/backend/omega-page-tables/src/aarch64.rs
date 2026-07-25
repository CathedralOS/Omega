use std::collections::{BTreeMap, BTreeSet};

use omega_extents::{
    AddressSpaceId, ExtentRightId, ExtentRights, PageTableConstructionEvidence,
    PageTableConstructionReceipt, PageTableConstructionReceiptId, PageTableDraft, PageTablePlanId,
};
use omega_layout_plans::{
    ByteOrder, LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport, ScalarFieldSchema,
    ScalarFieldValue, decode_scalar_layout, materialize_scalar_layout_into,
    normalized_layout_plan_fingerprint,
};

use crate::PageTableMaterializationDiagnostic;

pub const AARCH64_PAGE_BYTES: u64 = 4096;
pub const AARCH64_ENTRIES_PER_TABLE: usize = 512;

const VALID_PAGE_OR_TABLE: u64 = 0b11;
const ACCESS_PERMISSION_USER: u64 = 1 << 6;
const ACCESS_PERMISSION_READ_ONLY: u64 = 1 << 7;
const ACCESS_FLAG: u64 = 1 << 10;
const NOT_GLOBAL: u64 = 1 << 11;
const PRIVILEGED_EXECUTE_NEVER: u64 = 1 << 53;
const UNPRIVILEGED_EXECUTE_NEVER: u64 = 1 << 54;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64TranslationBase {
    Lower,
    Upper,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64Shareability {
    NonShareable,
    OuterShareable,
    InnerShareable,
}

impl Aarch64Shareability {
    const fn descriptor_bits(self) -> u64 {
        match self {
            Self::NonShareable => 0,
            Self::OuterShareable => 0b10 << 8,
            Self::InnerShareable => 0b11 << 8,
        }
    }
}

/// One provider-admitted semantic memory class and its MAIR selector.
///
/// The right is established on the mapping by its grant. The materializer only
/// translates that existing fact into `AttrIndx` and shareability bits; it
/// cannot establish a cacheability or device-memory claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aarch64MemoryClass {
    right: ExtentRightId,
    attribute_index: u8,
    shareability: Aarch64Shareability,
}

impl Aarch64MemoryClass {
    pub fn new(
        right: ExtentRightId,
        attribute_index: u8,
        shareability: Aarch64Shareability,
    ) -> Result<Self, PageTableMaterializationDiagnostic> {
        if attribute_index > 7 {
            return Err(PageTableMaterializationDiagnostic(
                "AArch64 MAIR attribute index must fit three bits".into(),
            ));
        }
        Ok(Self {
            right,
            attribute_index,
            shareability,
        })
    }

    pub const fn right(self) -> ExtentRightId {
        self.right
    }

    pub const fn attribute_index(self) -> u8 {
        self.attribute_index
    }

    pub const fn shareability(self) -> Aarch64Shareability {
        self.shareability
    }
}

/// Provider-owned mapping between semantic Extent rights and AArch64 stage-1
/// page-descriptor facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aarch64PageRights {
    readable: ExtentRightId,
    writable: ExtentRightId,
    executable: ExtentRightId,
    user: Option<ExtentRightId>,
    global: Option<ExtentRightId>,
    memory_classes: Vec<Aarch64MemoryClass>,
    unencoded: ExtentRights,
}

impl Aarch64PageRights {
    pub fn new(
        readable: ExtentRightId,
        writable: ExtentRightId,
        executable: ExtentRightId,
        user: Option<ExtentRightId>,
        global: Option<ExtentRightId>,
        memory_classes: Vec<Aarch64MemoryClass>,
        unencoded: ExtentRights,
    ) -> Result<Self, PageTableMaterializationDiagnostic> {
        if memory_classes.is_empty() {
            return Err(PageTableMaterializationDiagnostic(
                "AArch64 page policy requires at least one admitted memory class".into(),
            ));
        }
        let role_rights = [
            Some(readable),
            Some(writable),
            Some(executable),
            user,
            global,
        ]
        .into_iter()
        .flatten()
        .chain(memory_classes.iter().map(|class| class.right))
        .collect::<Vec<_>>();
        let distinct = role_rights.iter().copied().collect::<BTreeSet<_>>();
        if distinct.len() != role_rights.len() {
            return Err(PageTableMaterializationDiagnostic(
                "AArch64 page-right roles and memory classes must use distinct identities".into(),
            ));
        }
        if unencoded
            .identities()
            .any(|identity| distinct.contains(&identity))
        {
            return Err(PageTableMaterializationDiagnostic(
                "an AArch64 page right cannot be both encoded and unencoded".into(),
            ));
        }
        Ok(Self {
            readable,
            writable,
            executable,
            user,
            global,
            memory_classes,
            unencoded,
        })
    }

    fn known_rights(&self) -> BTreeSet<ExtentRightId> {
        [
            Some(self.readable),
            Some(self.writable),
            Some(self.executable),
            self.user,
            self.global,
        ]
        .into_iter()
        .flatten()
        .chain(self.memory_classes.iter().map(|class| class.right))
        .chain(self.unencoded.identities())
        .collect()
    }

    fn leaf_flags(&self, rights: &ExtentRights) -> Result<u64, PageTableMaterializationDiagnostic> {
        if !contains(rights, self.readable) {
            return Err(PageTableMaterializationDiagnostic(
                "AArch64 valid mappings require the admitted readable right".into(),
            ));
        }
        let known = self.known_rights();
        if let Some(unknown) = rights
            .identities()
            .find(|identity| !known.contains(identity))
        {
            return Err(PageTableMaterializationDiagnostic(format!(
                "mapped right {} has no admitted AArch64 descriptor meaning",
                unknown.normalized_identity()
            )));
        }
        let classes = self
            .memory_classes
            .iter()
            .copied()
            .filter(|class| contains(rights, class.right))
            .collect::<Vec<_>>();
        let [memory_class] = classes.as_slice() else {
            return Err(PageTableMaterializationDiagnostic(format!(
                "AArch64 mapping requires exactly one admitted memory class, found {}",
                classes.len()
            )));
        };

        let user = self.user.is_some_and(|right| contains(rights, right));
        let writable = contains(rights, self.writable);
        let executable = contains(rights, self.executable);
        let global = self.global.is_some_and(|right| contains(rights, right));

        let mut flags = VALID_PAGE_OR_TABLE
            | ACCESS_FLAG
            | (u64::from(memory_class.attribute_index) << 2)
            | memory_class.shareability.descriptor_bits();
        if user {
            flags |= ACCESS_PERMISSION_USER;
        }
        if !writable {
            flags |= ACCESS_PERMISSION_READ_ONLY;
        }
        if !global {
            flags |= NOT_GLOBAL;
        }
        match (executable, user) {
            (true, true) => flags |= PRIVILEGED_EXECUTE_NEVER,
            (true, false) => flags |= UNPRIVILEGED_EXECUTE_NEVER,
            (false, _) => {
                flags |= PRIVILEGED_EXECUTE_NEVER | UNPRIVILEGED_EXECUTE_NEVER;
            }
        }
        Ok(flags)
    }
}

/// Closed provider policy for a four-level, 4 KiB-granule AArch64 stage-1
/// translation table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aarch64PageTablePolicy {
    physical_space: AddressSpaceId,
    virtual_space: AddressSpaceId,
    translation_base: Aarch64TranslationBase,
    physical_address_bits: u8,
    max_table_pages: usize,
    max_leaf_entries: usize,
    rights: Aarch64PageRights,
    descriptor_layout: LayoutPlanReport,
}

impl Aarch64PageTablePolicy {
    pub fn new(
        physical_space: AddressSpaceId,
        virtual_space: AddressSpaceId,
        translation_base: Aarch64TranslationBase,
        physical_address_bits: u8,
        max_table_pages: usize,
        max_leaf_entries: usize,
        rights: Aarch64PageRights,
    ) -> Result<Self, PageTableMaterializationDiagnostic> {
        Self::from_validated_layout(
            physical_space,
            virtual_space,
            translation_base,
            physical_address_bits,
            max_table_pages,
            max_leaf_entries,
            rights,
            canonical_aarch64_4k_descriptor_layout(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_validated_layout(
        physical_space: AddressSpaceId,
        virtual_space: AddressSpaceId,
        translation_base: Aarch64TranslationBase,
        physical_address_bits: u8,
        max_table_pages: usize,
        max_leaf_entries: usize,
        rights: Aarch64PageRights,
        descriptor_layout: LayoutPlanReport,
    ) -> Result<Self, PageTableMaterializationDiagnostic> {
        if physical_space == virtual_space {
            return Err(PageTableMaterializationDiagnostic(
                "AArch64 physical and virtual address spaces must be distinct".into(),
            ));
        }
        if !(32..=48).contains(&physical_address_bits) {
            return Err(PageTableMaterializationDiagnostic(
                "v1 AArch64 page tables support 32 through 48 physical-address bits".into(),
            ));
        }
        if max_table_pages == 0 || max_leaf_entries == 0 {
            return Err(PageTableMaterializationDiagnostic(
                "AArch64 page-table bounds must be nonzero".into(),
            ));
        }
        require_exact_descriptor_layout(&descriptor_layout)?;
        Ok(Self {
            physical_space,
            virtual_space,
            translation_base,
            physical_address_bits,
            max_table_pages,
            max_leaf_entries,
            rights,
            descriptor_layout,
        })
    }

    pub const fn descriptor_layout(&self) -> &LayoutPlanReport {
        &self.descriptor_layout
    }

    /// Report/cache identity only. Exact normalized layout comparison, not this
    /// compact fingerprint, controls admission.
    pub fn descriptor_layout_identity(&self) -> u64 {
        normalized_layout_plan_fingerprint(&self.descriptor_layout)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializedAarch64PageTable {
    plan: PageTablePlanId,
    root_physical_address: u64,
    table_page_count: usize,
    leaf_entry_count: usize,
    content_fingerprint: u64,
    bytes: Vec<u8>,
}

impl MaterializedAarch64PageTable {
    pub const fn plan(&self) -> PageTablePlanId {
        self.plan
    }

    pub const fn root_physical_address(&self) -> u64 {
        self.root_physical_address
    }

    pub const fn table_page_count(&self) -> usize {
        self.table_page_count
    }

    pub const fn leaf_entry_count(&self) -> usize {
        self.leaf_entry_count
    }

    pub const fn content_fingerprint(&self) -> u64 {
        self.content_fingerprint
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub fn into_construction_receipt(
        self,
        identity: PageTableConstructionReceiptId,
        draft: &PageTableDraft<'_>,
    ) -> Result<PageTableConstructionReceipt, PageTableMaterializationDiagnostic> {
        if self.plan != draft.plan_identity()
            || self.root_physical_address != draft.storage_base()
            || self.bytes.len() as u64 != draft.storage_length()
        {
            return Err(PageTableMaterializationDiagnostic(
                "materialized AArch64 bytes do not bind the supplied page-table draft".into(),
            ));
        }
        Ok(PageTableConstructionReceipt::from_admitted_provider(
            identity,
            draft,
            self.bytes,
            PageTableConstructionEvidence::Generated,
            true,
        ))
    }
}

/// Exact canonical imported AArch64 table validated against one normalized
/// draft and target policy.
///
/// V1 intentionally accepts only the deterministic hierarchy produced by
/// [`materialize_aarch64_4k_page_table`]. Alternate table-page allocation,
/// aliases, block descriptors, and hardware-mutated state require explicit
/// policies before they can establish construction evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedImportedAarch64PageTable {
    plan: PageTablePlanId,
    root_physical_address: u64,
    table_page_count: usize,
    leaf_entry_count: usize,
    content_fingerprint: u64,
    bytes: Vec<u8>,
}

impl ValidatedImportedAarch64PageTable {
    pub const fn plan(&self) -> PageTablePlanId {
        self.plan
    }

    pub const fn root_physical_address(&self) -> u64 {
        self.root_physical_address
    }

    pub const fn table_page_count(&self) -> usize {
        self.table_page_count
    }

    pub const fn leaf_entry_count(&self) -> usize {
        self.leaf_entry_count
    }

    pub const fn content_fingerprint(&self) -> u64 {
        self.content_fingerprint
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Mints the ordinary imported-scan construction receipt over these exact
    /// bytes. It still grants no translation or page-table-control authority.
    pub fn into_construction_receipt(
        self,
        identity: PageTableConstructionReceiptId,
        draft: &PageTableDraft<'_>,
    ) -> Result<PageTableConstructionReceipt, PageTableMaterializationDiagnostic> {
        if self.plan != draft.plan_identity()
            || self.root_physical_address != draft.storage_base()
            || self.bytes.len() as u64 != draft.storage_length()
        {
            return Err(PageTableMaterializationDiagnostic(
                "validated imported AArch64 bytes do not bind the supplied page-table draft".into(),
            ));
        }
        Ok(PageTableConstructionReceipt::from_admitted_provider(
            identity,
            draft,
            self.bytes,
            PageTableConstructionEvidence::ImportedScan,
            true,
        ))
    }
}

/// Validates an imported table image through the same normalized plan and
/// scalar-layout vocabulary as generated construction.
///
/// Each descriptor must round-trip through the declared named fields, making
/// reserved or unsupported bits fail closed. The complete image must then equal
/// the deterministic bytes for the exact draft; decoding alone establishes no
/// mapping or construction authority.
pub fn validate_imported_aarch64_4k_page_table(
    draft: &PageTableDraft<'_>,
    policy: &Aarch64PageTablePolicy,
    imported: &[u8],
) -> Result<ValidatedImportedAarch64PageTable, PageTableMaterializationDiagnostic> {
    let expected = materialize_aarch64_4k_page_table(draft, policy)?;
    if imported.len() != expected.bytes.len() {
        return Err(PageTableMaterializationDiagnostic(format!(
            "imported AArch64 table has {} bytes, exact draft storage has {}",
            imported.len(),
            expected.bytes.len()
        )));
    }

    let schema = aarch64_descriptor_field_schema();
    for (entry_index, source) in imported.chunks_exact(size_of::<u64>()).enumerate() {
        let values = decode_scalar_layout(
            &policy.descriptor_layout,
            &schema,
            ByteOrder::LittleEndian,
            source,
        )
        .map_err(|diagnostic| {
            PageTableMaterializationDiagnostic(format!(
                "imported AArch64 descriptor {entry_index} does not decode: {}",
                diagnostic.0
            ))
        })?;
        let mut canonical = [0_u8; size_of::<u64>()];
        materialize_scalar_layout_into(
            &policy.descriptor_layout,
            &values,
            ByteOrder::LittleEndian,
            &mut canonical,
        )
        .map_err(|diagnostic| PageTableMaterializationDiagnostic(diagnostic.0))?;
        if canonical != source {
            return Err(PageTableMaterializationDiagnostic(format!(
                "imported AArch64 descriptor {entry_index} sets reserved or unsupported bits"
            )));
        }
        let expected_entry =
            &expected.bytes[entry_index * size_of::<u64>()..(entry_index + 1) * size_of::<u64>()];
        if source != expected_entry {
            let table = entry_index / AARCH64_ENTRIES_PER_TABLE;
            let slot = entry_index % AARCH64_ENTRIES_PER_TABLE;
            return Err(PageTableMaterializationDiagnostic(format!(
                "imported AArch64 table page {table} slot {slot} differs from the exact normalized mapping plan"
            )));
        }
    }

    Ok(ValidatedImportedAarch64PageTable {
        plan: expected.plan,
        root_physical_address: expected.root_physical_address,
        table_page_count: expected.table_page_count,
        leaf_entry_count: expected.leaf_entry_count,
        content_fingerprint: expected.content_fingerprint,
        bytes: imported.to_vec(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LeafPage {
    virtual_address: u64,
    physical_address: u64,
    flags: u64,
}

/// Materializes one complete four-level AArch64 stage-1 table with 4 KiB
/// leaves. Table pages come only from the beginning of the exact draft storage
/// extent. Block descriptors, 52-bit/LPA2 encodings, contiguous hints, dirty
/// state, and installation into TTBR0/TTBR1 remain separate provider work.
pub fn materialize_aarch64_4k_page_table(
    draft: &PageTableDraft<'_>,
    policy: &Aarch64PageTablePolicy,
) -> Result<MaterializedAarch64PageTable, PageTableMaterializationDiagnostic> {
    validate_storage(draft, policy)?;
    let leaves = collect_leaves(draft, policy)?;

    let storage_page_count =
        usize::try_from(draft.storage_length() / AARCH64_PAGE_BYTES).map_err(|_| {
            PageTableMaterializationDiagnostic(
                "page-table storage page count does not fit the provider host".into(),
            )
        })?;
    if storage_page_count > policy.max_table_pages {
        return Err(PageTableMaterializationDiagnostic(format!(
            "page-table storage has {storage_page_count} pages, exceeding admitted bound {}",
            policy.max_table_pages
        )));
    }
    let storage_length = usize::try_from(draft.storage_length()).map_err(|_| {
        PageTableMaterializationDiagnostic(
            "page-table storage length does not fit the provider host".into(),
        )
    })?;
    let address_mask = physical_address_mask(policy.physical_address_bits);
    let mut tables = vec![[0u64; AARCH64_ENTRIES_PER_TABLE]];
    let mut children = BTreeMap::<(usize, usize), usize>::new();

    for leaf in &leaves {
        let indices = [
            ((leaf.virtual_address >> 39) & 0x1ff) as usize,
            ((leaf.virtual_address >> 30) & 0x1ff) as usize,
            ((leaf.virtual_address >> 21) & 0x1ff) as usize,
            ((leaf.virtual_address >> 12) & 0x1ff) as usize,
        ];
        let mut table = 0usize;
        for index in indices.into_iter().take(3) {
            table = ensure_child_table(
                &mut tables,
                &mut children,
                table,
                index,
                draft.storage_base(),
                storage_page_count,
                policy.max_table_pages,
                address_mask,
                &policy.descriptor_layout,
            )?;
        }
        let leaf_index = indices[3];
        if tables[table][leaf_index] != 0 {
            return Err(PageTableMaterializationDiagnostic(format!(
                "two normalized mappings target virtual page 0x{:x}",
                leaf.virtual_address
            )));
        }
        tables[table][leaf_index] = materialize_page_descriptor(
            &policy.descriptor_layout,
            leaf.physical_address,
            leaf.flags,
        )?;
    }

    let mut bytes = vec![0u8; storage_length];
    for (table_index, table) in tables.iter().enumerate() {
        let table_offset = table_index
            .checked_mul(AARCH64_PAGE_BYTES as usize)
            .ok_or_else(|| {
                PageTableMaterializationDiagnostic("page-table byte offset overflows".into())
            })?;
        for (entry_index, entry) in table.iter().enumerate() {
            let offset = table_offset + entry_index * size_of::<u64>();
            bytes[offset..offset + size_of::<u64>()].copy_from_slice(&entry.to_le_bytes());
        }
    }

    Ok(MaterializedAarch64PageTable {
        plan: draft.plan_identity(),
        root_physical_address: draft.storage_base(),
        table_page_count: tables.len(),
        leaf_entry_count: leaves.len(),
        content_fingerprint: fingerprint_bytes(&bytes),
        bytes,
    })
}

fn validate_storage(
    draft: &PageTableDraft<'_>,
    policy: &Aarch64PageTablePolicy,
) -> Result<(), PageTableMaterializationDiagnostic> {
    if draft.storage_address_space() != policy.physical_space {
        return Err(PageTableMaterializationDiagnostic(
            "AArch64 page-table storage must be in the admitted physical address space".into(),
        ));
    }
    if draft.storage_length() < AARCH64_PAGE_BYTES
        || !draft.storage_length().is_multiple_of(AARCH64_PAGE_BYTES)
        || !draft.storage_base().is_multiple_of(AARCH64_PAGE_BYTES)
    {
        return Err(PageTableMaterializationDiagnostic(
            "AArch64 page-table storage must be a nonempty whole number of aligned 4 KiB pages"
                .into(),
        ));
    }
    let end = draft
        .storage_base()
        .checked_add(draft.storage_length() - 1)
        .ok_or_else(|| {
            PageTableMaterializationDiagnostic("page-table storage range overflows".into())
        })?;
    if end > physical_address_mask(policy.physical_address_bits) {
        return Err(PageTableMaterializationDiagnostic(
            "page-table storage lies outside the admitted physical-address width".into(),
        ));
    }
    Ok(())
}

fn collect_leaves(
    draft: &PageTableDraft<'_>,
    policy: &Aarch64PageTablePolicy,
) -> Result<Vec<LeafPage>, PageTableMaterializationDiagnostic> {
    let mut leaves = Vec::new();
    for mapping in draft.mappings() {
        if mapping.source_address_space() != policy.physical_space
            || mapping.destination_address_space() != policy.virtual_space
        {
            return Err(PageTableMaterializationDiagnostic(
                "pending mapping uses an address space outside the AArch64 page-table policy"
                    .into(),
            ));
        }
        if mapping.source_length() != mapping.destination_length()
            || mapping.source_length() == 0
            || !mapping.source_length().is_multiple_of(AARCH64_PAGE_BYTES)
            || !mapping.source_base().is_multiple_of(AARCH64_PAGE_BYTES)
            || !mapping
                .destination_base()
                .is_multiple_of(AARCH64_PAGE_BYTES)
        {
            return Err(PageTableMaterializationDiagnostic(
                "AArch64 mappings require equal, nonempty, 4 KiB-aligned whole-page ranges".into(),
            ));
        }
        let last_virtual = mapping
            .destination_base()
            .checked_add(mapping.destination_length() - 1)
            .ok_or_else(|| {
                PageTableMaterializationDiagnostic("mapped virtual range overflows".into())
            })?;
        if !address_matches_translation_base(mapping.destination_base(), policy.translation_base)
            || !address_matches_translation_base(last_virtual, policy.translation_base)
        {
            return Err(PageTableMaterializationDiagnostic(
                "AArch64 mapping lies outside the admitted 48-bit TTBR address half".into(),
            ));
        }
        let last_physical = mapping
            .source_base()
            .checked_add(mapping.source_length() - 1)
            .ok_or_else(|| {
                PageTableMaterializationDiagnostic("mapped physical range overflows".into())
            })?;
        if last_physical > physical_address_mask(policy.physical_address_bits) {
            return Err(PageTableMaterializationDiagnostic(
                "AArch64 mapping lies outside the admitted physical-address width".into(),
            ));
        }
        let flags = policy.rights.leaf_flags(mapping.mapped_rights())?;
        let page_count =
            usize::try_from(mapping.source_length() / AARCH64_PAGE_BYTES).map_err(|_| {
                PageTableMaterializationDiagnostic(
                    "mapping page count does not fit the provider host".into(),
                )
            })?;
        if leaves.len().saturating_add(page_count) > policy.max_leaf_entries {
            return Err(PageTableMaterializationDiagnostic(format!(
                "page-table mappings exceed admitted {}-leaf bound",
                policy.max_leaf_entries
            )));
        }
        for page in 0..page_count {
            let offset = (page as u64) * AARCH64_PAGE_BYTES;
            leaves.push(LeafPage {
                virtual_address: mapping.destination_base() + offset,
                physical_address: mapping.source_base() + offset,
                flags,
            });
        }
    }
    leaves.sort_unstable_by_key(|leaf| leaf.virtual_address);
    Ok(leaves)
}

#[allow(clippy::too_many_arguments)]
fn ensure_child_table(
    tables: &mut Vec<[u64; AARCH64_ENTRIES_PER_TABLE]>,
    children: &mut BTreeMap<(usize, usize), usize>,
    parent: usize,
    slot: usize,
    storage_base: u64,
    storage_page_count: usize,
    max_table_pages: usize,
    address_mask: u64,
    layout: &LayoutPlanReport,
) -> Result<usize, PageTableMaterializationDiagnostic> {
    if let Some(child) = children.get(&(parent, slot)).copied() {
        return Ok(child);
    }
    let child = tables.len();
    if child >= storage_page_count || child >= max_table_pages {
        return Err(PageTableMaterializationDiagnostic(
            "AArch64 page-table storage cannot hold the required hierarchy".into(),
        ));
    }
    let child_address = storage_base
        .checked_add((child as u64) * AARCH64_PAGE_BYTES)
        .ok_or_else(|| {
            PageTableMaterializationDiagnostic("child page-table address overflows".into())
        })?;
    if child_address & !address_mask != 0 {
        return Err(PageTableMaterializationDiagnostic(
            "child page-table address exceeds admitted physical width".into(),
        ));
    }
    tables.push([0; AARCH64_ENTRIES_PER_TABLE]);
    children.insert((parent, slot), child);
    tables[parent][slot] = materialize_page_descriptor(layout, child_address, VALID_PAGE_OR_TABLE)?;
    Ok(child)
}

/// Canonical AArch64 stage-1 descriptor geometry for the v1 4 KiB granule.
///
/// An Omega-authored target policy may produce these named fields in another
/// declaration order, but its exact normalized geometry must equal this
/// hardware contract. LPA2 and block descriptors require distinct policies.
pub fn canonical_aarch64_4k_descriptor_layout() -> LayoutPlanReport {
    LayoutPlanReport {
        entries: vec![
            bit_field("valid", 0),
            bit_field("table_or_page", 1),
            bits_field("attribute_index", 2, 3),
            bits_field("access_permission", 6, 2),
            bits_field("shareability", 8, 2),
            bit_field("access_flag", 10),
            bit_field("not_global", 11),
            bits_field("output_address", 12, 36),
            bit_field("privileged_execute_never", 53),
            bit_field("unprivileged_execute_never", 54),
        ],
        offsets: None,
        size: Some(8),
        align: 8,
    }
}

fn bit_field(field: &str, destination_lsb: i64) -> LayoutFieldEntryReport {
    bits_field(field, destination_lsb, 1)
}

fn bits_field(field: &str, destination_lsb: i64, width: i64) -> LayoutFieldEntryReport {
    LayoutFieldEntryReport {
        field: field.into(),
        placement: LayoutPlacementReport::Bits {
            container: 0,
            container_width: 64,
            destination_lsb,
            source_lsb: 0,
            width,
        },
    }
}

fn require_exact_descriptor_layout(
    layout: &LayoutPlanReport,
) -> Result<(), PageTableMaterializationDiagnostic> {
    if normalized_layout(layout) != normalized_layout(&canonical_aarch64_4k_descriptor_layout()) {
        return Err(PageTableMaterializationDiagnostic(
            "layout does not exactly match AArch64 v1 4 KiB page-descriptor geometry".into(),
        ));
    }
    Ok(())
}

fn normalized_layout(
    layout: &LayoutPlanReport,
) -> (Option<i64>, i64, Vec<(String, LayoutPlacementReport)>) {
    let mut entries = layout
        .entries
        .iter()
        .map(|entry| (entry.field.clone(), entry.placement))
        .collect::<Vec<_>>();
    entries.sort_unstable_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| placement_key(left.1).cmp(&placement_key(right.1)))
    });
    (layout.size, layout.align, entries)
}

fn placement_key(placement: LayoutPlacementReport) -> (u8, i64, i64, i64, i64, i64) {
    match placement {
        LayoutPlacementReport::At { offset } => (0, offset, 0, 0, 0, 0),
        LayoutPlacementReport::Bits {
            container,
            container_width,
            destination_lsb,
            source_lsb,
            width,
        } => (
            1,
            container,
            container_width,
            destination_lsb,
            source_lsb,
            width,
        ),
    }
}

fn materialize_page_descriptor(
    layout: &LayoutPlanReport,
    physical_address: u64,
    flags: u64,
) -> Result<u64, PageTableMaterializationDiagnostic> {
    let fields = [
        scalar("valid", 1, u64::from(flags & 1 != 0))?,
        scalar("table_or_page", 1, (flags >> 1) & 1)?,
        scalar("attribute_index", 3, (flags >> 2) & 0b111)?,
        scalar("access_permission", 2, (flags >> 6) & 0b11)?,
        scalar("shareability", 2, (flags >> 8) & 0b11)?,
        scalar("access_flag", 1, (flags >> 10) & 1)?,
        scalar("not_global", 1, (flags >> 11) & 1)?,
        scalar("output_address", 36, physical_address >> 12)?,
        scalar("privileged_execute_never", 1, (flags >> 53) & 1)?,
        scalar("unprivileged_execute_never", 1, (flags >> 54) & 1)?,
    ];
    let mut bytes = [0_u8; size_of::<u64>()];
    materialize_scalar_layout_into(layout, &fields, ByteOrder::LittleEndian, &mut bytes)
        .map_err(|diagnostic| PageTableMaterializationDiagnostic(diagnostic.0))?;
    Ok(u64::from_le_bytes(bytes))
}

fn aarch64_descriptor_field_schema() -> [ScalarFieldSchema; 10] {
    [
        scalar_schema("valid", 1),
        scalar_schema("table_or_page", 1),
        scalar_schema("attribute_index", 3),
        scalar_schema("access_permission", 2),
        scalar_schema("shareability", 2),
        scalar_schema("access_flag", 1),
        scalar_schema("not_global", 1),
        scalar_schema("output_address", 36),
        scalar_schema("privileged_execute_never", 1),
        scalar_schema("unprivileged_execute_never", 1),
    ]
}

fn scalar_schema(field: &str, width_bits: u16) -> ScalarFieldSchema {
    ScalarFieldSchema::new(field, width_bits)
        .expect("canonical AArch64 page-descriptor schema has valid scalar widths")
}

fn scalar(
    field: &str,
    width_bits: u16,
    value: u64,
) -> Result<ScalarFieldValue, PageTableMaterializationDiagnostic> {
    ScalarFieldValue::new(field, width_bits, value)
        .map_err(|diagnostic| PageTableMaterializationDiagnostic(diagnostic.0))
}

fn contains(rights: &ExtentRights, identity: ExtentRightId) -> bool {
    rights.contains(&ExtentRights::from_normalized_identities([identity]))
}

const fn physical_address_mask(bits: u8) -> u64 {
    ((1u64 << bits) - 1) & !0xfff
}

const fn address_matches_translation_base(address: u64, base: Aarch64TranslationBase) -> bool {
    match base {
        Aarch64TranslationBase::Lower => address >> 48 == 0,
        Aarch64TranslationBase::Upper => address >> 48 == 0xffff,
    }
}

fn fingerprint_bytes(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in b"omega.aarch64-page-table.v1".iter().chain(bytes) {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    if hash == 0 { 1 } else { hash }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_extents::{
        ExtentDiagnostic, ExtentLineageId, ExtentProvenanceId, ExtentRootGrant, MappingEraId,
        MappingGrant, MappingGrantId, MappingId, MappingSourceMode, PageTableGrant,
        PageTableGrantId, PageTableId, PageTableRetirementObligations,
        TranslationInstallObligations, TranslationReleaseObligations, begin_page_table, map_owned,
    };

    fn id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExtentDiagnostic>) -> T {
        constructor(identity).expect("normalized identity")
    }

    fn right(identity: u64) -> ExtentRightId {
        id(identity, ExtentRightId::from_normalized_identity)
    }

    fn rights(identities: &[u64]) -> ExtentRights {
        ExtentRights::from_normalized_identities(identities.iter().copied().map(right))
    }

    struct Fixture {
        physical: AddressSpaceId,
        virtual_: AddressSpaceId,
        storage_provenance: ExtentProvenanceId,
        mapped_provenance: ExtentProvenanceId,
        readable: ExtentRightId,
        writable: ExtentRightId,
        executable: ExtentRightId,
        user: ExtentRightId,
        global: ExtentRightId,
        normal: ExtentRightId,
        device: ExtentRightId,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                physical: id(1, AddressSpaceId::from_normalized_identity),
                virtual_: id(2, AddressSpaceId::from_normalized_identity),
                storage_provenance: id(3, ExtentProvenanceId::from_normalized_identity),
                mapped_provenance: id(4, ExtentProvenanceId::from_normalized_identity),
                readable: right(10),
                writable: right(11),
                executable: right(12),
                user: right(13),
                global: right(14),
                normal: right(15),
                device: right(16),
            }
        }

        fn page_rights(&self) -> Aarch64PageRights {
            Aarch64PageRights::new(
                self.readable,
                self.writable,
                self.executable,
                Some(self.user),
                Some(self.global),
                vec![
                    Aarch64MemoryClass::new(self.normal, 3, Aarch64Shareability::InnerShareable)
                        .expect("normal memory"),
                    Aarch64MemoryClass::new(self.device, 0, Aarch64Shareability::OuterShareable)
                        .expect("device memory"),
                ],
                ExtentRights::default(),
            )
            .expect("page rights")
        }

        fn policy(&self, base: Aarch64TranslationBase) -> Aarch64PageTablePolicy {
            Aarch64PageTablePolicy::new(
                self.physical,
                self.virtual_,
                base,
                48,
                16,
                1024,
                self.page_rights(),
            )
            .expect("page-table policy")
        }

        fn draft(
            &self,
            destination_base: u64,
            mapped_rights: ExtentRights,
            storage_pages: u64,
        ) -> PageTableDraft<'static> {
            let storage = ExtentRootGrant::from_admitted_provider(
                id(20, ExtentLineageId::from_normalized_identity),
                self.physical,
                rights(&[11]),
                self.storage_provenance,
                id(1, MappingEraId::from_normalized_identity),
            )
            .mint(0x1000_0000, storage_pages * AARCH64_PAGE_BYTES)
            .expect("storage");
            let table_grant = PageTableGrant::from_admitted_provider(
                id(21, PageTableGrantId::from_normalized_identity),
                self.physical,
                self.storage_provenance,
                rights(&[11]),
                self.virtual_,
                AARCH64_PAGE_BYTES,
                AARCH64_PAGE_BYTES,
                PageTableRetirementObligations::default(),
            )
            .expect("page-table grant");
            let mut draft = begin_page_table(
                id(22, PageTableId::from_normalized_identity),
                &table_grant,
                storage,
            )
            .expect("page-table draft");

            let source = ExtentRootGrant::from_admitted_provider(
                id(23, ExtentLineageId::from_normalized_identity),
                self.physical,
                mapped_rights.clone(),
                id(24, ExtentProvenanceId::from_normalized_identity),
                id(2, MappingEraId::from_normalized_identity),
            )
            .mint(0x20_0000, AARCH64_PAGE_BYTES)
            .expect("source");
            let destination = ExtentRootGrant::from_admitted_provider(
                id(25, ExtentLineageId::from_normalized_identity),
                self.virtual_,
                rights(&[11]),
                id(26, ExtentProvenanceId::from_normalized_identity),
                id(3, MappingEraId::from_normalized_identity),
            )
            .mint(destination_base, AARCH64_PAGE_BYTES)
            .expect("destination");
            let map_grant = MappingGrant::from_admitted_provider(
                id(27, MappingGrantId::from_normalized_identity),
                MappingSourceMode::Owned,
                self.physical,
                self.virtual_,
                mapped_rights.clone(),
                rights(&[11]),
                mapped_rights,
                self.mapped_provenance,
                id(4, MappingEraId::from_normalized_identity),
                TranslationInstallObligations::default(),
                TranslationReleaseObligations::default(),
            );
            let pending = map_owned(
                source,
                destination,
                id(28, MappingId::from_normalized_identity),
                &map_grant,
            )
            .expect("pending mapping");
            draft.add_mapping(pending).expect("mapping in table");
            draft
        }
    }

    fn entry(bytes: &[u8], table: usize, slot: usize) -> u64 {
        let offset = table * AARCH64_PAGE_BYTES as usize + slot * size_of::<u64>();
        u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("entry bytes"))
    }

    #[test]
    fn materializes_lower_user_rw_normal_mapping() {
        let fixture = Fixture::new();
        let draft = fixture.draft(
            0x4000,
            ExtentRights::from_normalized_identities([
                fixture.readable,
                fixture.writable,
                fixture.user,
                fixture.normal,
            ]),
            4,
        );
        let image = materialize_aarch64_4k_page_table(
            &draft,
            &fixture.policy(Aarch64TranslationBase::Lower),
        )
        .expect("materialize");

        assert_eq!(image.plan(), draft.plan_identity());
        assert_eq!(image.root_physical_address(), 0x1000_0000);
        assert_eq!(image.table_page_count(), 4);
        assert_eq!(image.leaf_entry_count(), 1);
        assert_eq!(
            entry(image.bytes(), 0, 0),
            0x1000_1000 | VALID_PAGE_OR_TABLE
        );
        assert_eq!(
            entry(image.bytes(), 1, 0),
            0x1000_2000 | VALID_PAGE_OR_TABLE
        );
        assert_eq!(
            entry(image.bytes(), 2, 0),
            0x1000_3000 | VALID_PAGE_OR_TABLE
        );
        let expected = 0x20_0000
            | VALID_PAGE_OR_TABLE
            | (3 << 2)
            | ACCESS_PERMISSION_USER
            | Aarch64Shareability::InnerShareable.descriptor_bits()
            | ACCESS_FLAG
            | NOT_GLOBAL
            | PRIVILEGED_EXECUTE_NEVER
            | UNPRIVILEGED_EXECUTE_NEVER;
        assert_eq!(entry(image.bytes(), 3, 4), expected);
        assert_ne!(image.content_fingerprint(), 0);
    }

    #[test]
    fn materializes_upper_privileged_read_only_executable_device_mapping() {
        let fixture = Fixture::new();
        let draft = fixture.draft(
            0xffff_8000_0000_1000,
            ExtentRights::from_normalized_identities([
                fixture.readable,
                fixture.executable,
                fixture.global,
                fixture.device,
            ]),
            4,
        );
        let image = materialize_aarch64_4k_page_table(
            &draft,
            &fixture.policy(Aarch64TranslationBase::Upper),
        )
        .expect("materialize");
        let expected = 0x20_0000
            | VALID_PAGE_OR_TABLE
            | ACCESS_PERMISSION_READ_ONLY
            | Aarch64Shareability::OuterShareable.descriptor_bits()
            | ACCESS_FLAG
            | UNPRIVILEGED_EXECUTE_NEVER;
        assert_eq!(entry(image.bytes(), 3, 1), expected);
    }

    #[test]
    fn translation_half_memory_class_and_unknown_rights_fail_closed() {
        let fixture = Fixture::new();
        let lower = fixture.draft(
            0x4000,
            ExtentRights::from_normalized_identities([fixture.readable, fixture.normal]),
            4,
        );
        let error = materialize_aarch64_4k_page_table(
            &lower,
            &fixture.policy(Aarch64TranslationBase::Upper),
        )
        .expect_err("wrong TTBR half");
        assert!(error.0.contains("TTBR"));

        let no_class = fixture.draft(
            0x4000,
            ExtentRights::from_normalized_identities([fixture.readable]),
            4,
        );
        let error = materialize_aarch64_4k_page_table(
            &no_class,
            &fixture.policy(Aarch64TranslationBase::Lower),
        )
        .expect_err("memory class required");
        assert!(error.0.contains("exactly one"));

        let two_classes = fixture.draft(
            0x4000,
            ExtentRights::from_normalized_identities([
                fixture.readable,
                fixture.normal,
                fixture.device,
            ]),
            4,
        );
        let error = materialize_aarch64_4k_page_table(
            &two_classes,
            &fixture.policy(Aarch64TranslationBase::Lower),
        )
        .expect_err("one memory class");
        assert!(error.0.contains("found 2"));

        let unknown = fixture.draft(
            0x4000,
            ExtentRights::from_normalized_identities([fixture.readable, fixture.normal, right(99)]),
            4,
        );
        let error = materialize_aarch64_4k_page_table(
            &unknown,
            &fixture.policy(Aarch64TranslationBase::Lower),
        )
        .expect_err("unknown descriptor right");
        assert!(error.0.contains("has no admitted"));
    }

    #[test]
    fn generated_image_enters_the_existing_installable_lifecycle() {
        let fixture = Fixture::new();
        let draft = fixture.draft(
            0x4000,
            ExtentRights::from_normalized_identities([fixture.readable, fixture.normal]),
            4,
        );
        let image = materialize_aarch64_4k_page_table(
            &draft,
            &fixture.policy(Aarch64TranslationBase::Lower),
        )
        .expect("materialize");
        let receipt = image
            .into_construction_receipt(
                id(29, PageTableConstructionReceiptId::from_normalized_identity),
                &draft,
            )
            .expect("construction receipt");
        let installable = draft.finish(receipt).expect("installable table");
        assert_eq!(
            installable.evidence(),
            PageTableConstructionEvidence::Generated
        );
        assert_eq!(installable.bytes().len(), 4 * AARCH64_PAGE_BYTES as usize);
    }

    #[test]
    fn normalized_descriptor_layout_accepts_reordering_and_rejects_shifted_hardware_bits() {
        let fixture = Fixture::new();
        let mut reordered = canonical_aarch64_4k_descriptor_layout();
        reordered.entries.reverse();
        let policy = Aarch64PageTablePolicy::from_validated_layout(
            fixture.physical,
            fixture.virtual_,
            Aarch64TranslationBase::Lower,
            48,
            16,
            1024,
            fixture.page_rights(),
            reordered,
        )
        .expect("authored order is normalized away");
        assert_ne!(policy.descriptor_layout_identity(), 0);

        let mut shifted = canonical_aarch64_4k_descriptor_layout();
        shifted
            .entries
            .iter_mut()
            .find(|entry| entry.field == "access_flag")
            .expect("access-flag field")
            .placement = LayoutPlacementReport::Bits {
            container: 0,
            container_width: 64,
            destination_lsb: 9,
            source_lsb: 0,
            width: 1,
        };
        let error = Aarch64PageTablePolicy::from_validated_layout(
            fixture.physical,
            fixture.virtual_,
            Aarch64TranslationBase::Lower,
            48,
            16,
            1024,
            fixture.page_rights(),
            shifted,
        )
        .expect_err("shifted hardware bit must reject");
        assert!(error.0.contains("exactly match"));
    }

    #[test]
    fn canonical_imported_image_enters_the_imported_scan_lifecycle() {
        let fixture = Fixture::new();
        let draft = fixture.draft(
            0x4000,
            ExtentRights::from_normalized_identities([fixture.readable, fixture.normal]),
            4,
        );
        let generated = materialize_aarch64_4k_page_table(
            &draft,
            &fixture.policy(Aarch64TranslationBase::Lower),
        )
        .expect("materialize");
        let imported = validate_imported_aarch64_4k_page_table(
            &draft,
            &fixture.policy(Aarch64TranslationBase::Lower),
            generated.bytes(),
        )
        .expect("canonical imported table");
        assert_eq!(imported.plan(), draft.plan_identity());
        assert_eq!(imported.table_page_count(), 4);
        assert_eq!(imported.leaf_entry_count(), 1);

        let receipt = imported
            .into_construction_receipt(
                id(30, PageTableConstructionReceiptId::from_normalized_identity),
                &draft,
            )
            .expect("imported construction receipt");
        let installable = draft.finish(receipt).expect("installable imported table");
        assert_eq!(
            installable.evidence(),
            PageTableConstructionEvidence::ImportedScan
        );
        assert_eq!(installable.bytes(), generated.bytes());
    }

    #[test]
    fn imported_scan_rejects_reserved_bits_and_mapping_drift() {
        let fixture = Fixture::new();
        let draft = fixture.draft(
            0x4000,
            ExtentRights::from_normalized_identities([fixture.readable, fixture.normal]),
            4,
        );
        let policy = fixture.policy(Aarch64TranslationBase::Lower);
        let generated = materialize_aarch64_4k_page_table(&draft, &policy).expect("materialize");

        let mut reserved = generated.bytes().to_vec();
        reserved[0] |= 1 << 5;
        let error = validate_imported_aarch64_4k_page_table(&draft, &policy, &reserved)
            .expect_err("reserved bit must reject");
        assert!(error.0.contains("reserved or unsupported"));

        let mut drifted = generated.bytes().to_vec();
        let leaf_offset = 3 * AARCH64_PAGE_BYTES as usize + 4 * size_of::<u64>();
        drifted[leaf_offset] ^= ACCESS_PERMISSION_READ_ONLY as u8;
        let error = validate_imported_aarch64_4k_page_table(&draft, &policy, &drifted)
            .expect_err("semantic mapping drift must reject");
        assert!(error.0.contains("differs from the exact normalized"));
    }
}
