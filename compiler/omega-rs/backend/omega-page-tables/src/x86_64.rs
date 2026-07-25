use std::collections::{BTreeMap, BTreeSet};

use omega_extents::{
    AddressSpaceId, ExtentRightId, ExtentRights, PageTableConstructionEvidence,
    PageTableConstructionReceipt, PageTableConstructionReceiptId, PageTableDraft, PageTablePlanId,
};

pub const X86_64_PAGE_BYTES: u64 = 4096;
pub const X86_64_ENTRIES_PER_TABLE: usize = 512;

const PRESENT: u64 = 1 << 0;
const WRITABLE: u64 = 1 << 1;
const USER: u64 = 1 << 2;
const WRITE_THROUGH: u64 = 1 << 3;
const CACHE_DISABLE: u64 = 1 << 4;
const GLOBAL: u64 = 1 << 8;
const NO_EXECUTE: u64 = 1 << 63;

/// Provider-owned mapping between semantic Extent rights and x86-64 PTE bits.
///
/// Rights are grant-established facts. Naming an identity here does not mint
/// it; the materializer only observes rights already retained by each pending
/// mapping. Extra orthogonal facts must be admitted explicitly as `unencoded`
/// rather than being silently discarded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86_64PageRights {
    readable: ExtentRightId,
    writable: ExtentRightId,
    executable: ExtentRightId,
    user: Option<ExtentRightId>,
    global: Option<ExtentRightId>,
    write_through: Option<ExtentRightId>,
    cache_disable: Option<ExtentRightId>,
    unencoded: ExtentRights,
}

impl X86_64PageRights {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        readable: ExtentRightId,
        writable: ExtentRightId,
        executable: ExtentRightId,
        user: Option<ExtentRightId>,
        global: Option<ExtentRightId>,
        write_through: Option<ExtentRightId>,
        cache_disable: Option<ExtentRightId>,
        unencoded: ExtentRights,
    ) -> Result<Self, PageTableMaterializationDiagnostic> {
        let encoded = [
            Some(readable),
            Some(writable),
            Some(executable),
            user,
            global,
            write_through,
            cache_disable,
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        let distinct = encoded.iter().copied().collect::<BTreeSet<_>>();
        if distinct.len() != encoded.len() {
            return Err(PageTableMaterializationDiagnostic(
                "x86-64 page-right roles must use distinct normalized identities".into(),
            ));
        }
        if unencoded
            .identities()
            .any(|identity| distinct.contains(&identity))
        {
            return Err(PageTableMaterializationDiagnostic(
                "an x86-64 page right cannot be both encoded and unencoded".into(),
            ));
        }
        Ok(Self {
            readable,
            writable,
            executable,
            user,
            global,
            write_through,
            cache_disable,
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
            self.write_through,
            self.cache_disable,
        ]
        .into_iter()
        .flatten()
        .chain(self.unencoded.identities())
        .collect()
    }

    fn leaf_flags(
        &self,
        rights: &ExtentRights,
        nx_supported: bool,
    ) -> Result<u64, PageTableMaterializationDiagnostic> {
        if !rights.contains(&ExtentRights::from_normalized_identities([self.readable])) {
            return Err(PageTableMaterializationDiagnostic(
                "x86-64 present mappings require the admitted readable right".into(),
            ));
        }
        let known = self.known_rights();
        if let Some(unknown) = rights
            .identities()
            .find(|identity| !known.contains(identity))
        {
            return Err(PageTableMaterializationDiagnostic(format!(
                "mapped right {} has no admitted x86-64 PTE meaning",
                unknown.normalized_identity()
            )));
        }

        let has = |identity: ExtentRightId| {
            rights.contains(&ExtentRights::from_normalized_identities([identity]))
        };
        let optional = |identity: Option<ExtentRightId>| identity.is_some_and(has);
        let mut flags = PRESENT;
        if has(self.writable) {
            flags |= WRITABLE;
        }
        if optional(self.user) {
            flags |= USER;
        }
        if optional(self.global) {
            flags |= GLOBAL;
        }
        if optional(self.write_through) {
            flags |= WRITE_THROUGH;
        }
        if optional(self.cache_disable) {
            flags |= CACHE_DISABLE;
        }
        if !has(self.executable) {
            if !nx_supported {
                return Err(PageTableMaterializationDiagnostic(
                    "target cannot enforce a non-executable mapping without NX support".into(),
                ));
            }
            flags |= NO_EXECUTE;
        }
        Ok(flags)
    }
}

/// Closed provider policy for the first x86-64 four-level, 4 KiB-page writer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86_64PageTablePolicy {
    physical_space: AddressSpaceId,
    virtual_space: AddressSpaceId,
    physical_address_bits: u8,
    nx_supported: bool,
    max_table_pages: usize,
    max_leaf_entries: usize,
    rights: X86_64PageRights,
}

impl X86_64PageTablePolicy {
    pub fn new(
        physical_space: AddressSpaceId,
        virtual_space: AddressSpaceId,
        physical_address_bits: u8,
        nx_supported: bool,
        max_table_pages: usize,
        max_leaf_entries: usize,
        rights: X86_64PageRights,
    ) -> Result<Self, PageTableMaterializationDiagnostic> {
        if physical_space == virtual_space {
            return Err(PageTableMaterializationDiagnostic(
                "x86-64 physical and virtual address spaces must be distinct".into(),
            ));
        }
        if !(32..=52).contains(&physical_address_bits) {
            return Err(PageTableMaterializationDiagnostic(
                "x86-64 physical-address width must be between 32 and 52 bits".into(),
            ));
        }
        if max_table_pages == 0 || max_leaf_entries == 0 {
            return Err(PageTableMaterializationDiagnostic(
                "x86-64 page-table bounds must be nonzero".into(),
            ));
        }
        Ok(Self {
            physical_space,
            virtual_space,
            physical_address_bits,
            nx_supported,
            max_table_pages,
            max_leaf_entries,
            rights,
        })
    }
}

/// Inert exact bytes for one normalized x86-64 page-table draft.
///
/// This value grants neither page-table-control authority nor active mappings.
/// The admitted provider may use the bytes to mint the separately checked
/// construction receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializedX86_64PageTable {
    plan: PageTablePlanId,
    root_physical_address: u64,
    table_page_count: usize,
    leaf_entry_count: usize,
    content_fingerprint: u64,
    bytes: Vec<u8>,
}

impl MaterializedX86_64PageTable {
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

    /// Converts these exact generated bytes into the ordinary page-table
    /// construction receipt for the same draft. This consumes the inert image;
    /// installation remains a separate provider-controlled transition.
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
                "materialized x86-64 bytes do not bind the supplied page-table draft".into(),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageTableMaterializationDiagnostic(pub String);

impl std::fmt::Display for PageTableMaterializationDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PageTableMaterializationDiagnostic {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LeafPage {
    virtual_address: u64,
    physical_address: u64,
    flags: u64,
}

/// Materializes a complete four-level x86-64 table using only 4 KiB leaves.
///
/// Table pages are allocated deterministically from the beginning of the
/// draft's exact physical storage extent. The returned byte vector covers that
/// entire extent and leaves all unused bytes zero. Huge pages, five-level
/// paging, PAT selection, and runtime installation are deliberately separate
/// provider work.
pub fn materialize_x86_64_4k_page_table(
    draft: &PageTableDraft<'_>,
    policy: &X86_64PageTablePolicy,
) -> Result<MaterializedX86_64PageTable, PageTableMaterializationDiagnostic> {
    validate_storage(draft, policy)?;
    let leaves = collect_leaves(draft, policy)?;

    let storage_page_count =
        usize::try_from(draft.storage_length() / X86_64_PAGE_BYTES).map_err(|_| {
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
    let mut tables = vec![[0u64; X86_64_ENTRIES_PER_TABLE]];
    let mut children = BTreeMap::<(usize, usize), usize>::new();

    for leaf in &leaves {
        let writable_user = leaf.flags & (WRITABLE | USER);
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
                writable_user,
                draft.storage_base(),
                storage_page_count,
                policy.max_table_pages,
                address_mask,
            )?;
        }
        let leaf_index = indices[3];
        if tables[table][leaf_index] != 0 {
            return Err(PageTableMaterializationDiagnostic(format!(
                "two normalized mappings target virtual page 0x{:x}",
                leaf.virtual_address
            )));
        }
        tables[table][leaf_index] = (leaf.physical_address & address_mask) | leaf.flags;
    }

    let mut bytes = vec![0u8; storage_length];
    for (table_index, table) in tables.iter().enumerate() {
        let table_offset = table_index
            .checked_mul(X86_64_PAGE_BYTES as usize)
            .ok_or_else(|| {
                PageTableMaterializationDiagnostic("page-table byte offset overflows".into())
            })?;
        for (entry_index, entry) in table.iter().enumerate() {
            let offset = table_offset + entry_index * size_of::<u64>();
            bytes[offset..offset + size_of::<u64>()].copy_from_slice(&entry.to_le_bytes());
        }
    }

    Ok(MaterializedX86_64PageTable {
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
    policy: &X86_64PageTablePolicy,
) -> Result<(), PageTableMaterializationDiagnostic> {
    if draft.storage_address_space() != policy.physical_space {
        return Err(PageTableMaterializationDiagnostic(
            "x86-64 page-table storage must be in the admitted physical address space".into(),
        ));
    }
    if draft.storage_length() < X86_64_PAGE_BYTES
        || !draft.storage_length().is_multiple_of(X86_64_PAGE_BYTES)
        || !draft.storage_base().is_multiple_of(X86_64_PAGE_BYTES)
    {
        return Err(PageTableMaterializationDiagnostic(
            "x86-64 page-table storage must be a nonempty whole number of aligned 4 KiB pages"
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
    policy: &X86_64PageTablePolicy,
) -> Result<Vec<LeafPage>, PageTableMaterializationDiagnostic> {
    let mut leaves = Vec::new();
    for mapping in draft.mappings() {
        if mapping.source_address_space() != policy.physical_space
            || mapping.destination_address_space() != policy.virtual_space
        {
            return Err(PageTableMaterializationDiagnostic(
                "pending mapping uses an address space outside the x86-64 page-table policy".into(),
            ));
        }
        if mapping.source_length() != mapping.destination_length()
            || mapping.source_length() == 0
            || !mapping.source_length().is_multiple_of(X86_64_PAGE_BYTES)
            || !mapping.source_base().is_multiple_of(X86_64_PAGE_BYTES)
            || !mapping.destination_base().is_multiple_of(X86_64_PAGE_BYTES)
        {
            return Err(PageTableMaterializationDiagnostic(
                "x86-64 mappings require equal, nonempty, 4 KiB-aligned whole-page ranges".into(),
            ));
        }
        let last_virtual = mapping
            .destination_base()
            .checked_add(mapping.destination_length() - 1)
            .ok_or_else(|| {
                PageTableMaterializationDiagnostic("mapped virtual range overflows".into())
            })?;
        if !is_canonical_48(mapping.destination_base()) || !is_canonical_48(last_virtual) {
            return Err(PageTableMaterializationDiagnostic(
                "x86-64 mapping contains a noncanonical 48-bit virtual address".into(),
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
                "x86-64 mapping lies outside the admitted physical-address width".into(),
            ));
        }
        let flags = policy
            .rights
            .leaf_flags(mapping.mapped_rights(), policy.nx_supported)?;
        let page_count =
            usize::try_from(mapping.source_length() / X86_64_PAGE_BYTES).map_err(|_| {
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
            let offset = (page as u64) * X86_64_PAGE_BYTES;
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
    tables: &mut Vec<[u64; X86_64_ENTRIES_PER_TABLE]>,
    children: &mut BTreeMap<(usize, usize), usize>,
    parent: usize,
    slot: usize,
    propagated_flags: u64,
    storage_base: u64,
    storage_page_count: usize,
    max_table_pages: usize,
    address_mask: u64,
) -> Result<usize, PageTableMaterializationDiagnostic> {
    if let Some(child) = children.get(&(parent, slot)).copied() {
        tables[parent][slot] |= propagated_flags;
        return Ok(child);
    }
    let child = tables.len();
    if child >= storage_page_count || child >= max_table_pages {
        return Err(PageTableMaterializationDiagnostic(
            "x86-64 page-table storage cannot hold the required hierarchy".into(),
        ));
    }
    let child_address = storage_base
        .checked_add((child as u64) * X86_64_PAGE_BYTES)
        .ok_or_else(|| {
            PageTableMaterializationDiagnostic("child page-table address overflows".into())
        })?;
    if child_address & !address_mask != 0 {
        return Err(PageTableMaterializationDiagnostic(
            "child page-table address exceeds admitted physical width".into(),
        ));
    }
    tables.push([0; X86_64_ENTRIES_PER_TABLE]);
    children.insert((parent, slot), child);
    tables[parent][slot] = child_address | PRESENT | propagated_flags;
    Ok(child)
}

const fn physical_address_mask(bits: u8) -> u64 {
    ((1u64 << bits) - 1) & !0xfff
}

const fn is_canonical_48(address: u64) -> bool {
    let high = address >> 48;
    if address & (1 << 47) == 0 {
        high == 0
    } else {
        high == 0xffff
    }
}

fn fingerprint_bytes(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in b"omega.x86-64-page-table.v1".iter().chain(bytes) {
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
            }
        }

        fn policy(&self, nx_supported: bool) -> X86_64PageTablePolicy {
            let right_policy = X86_64PageRights::new(
                self.readable,
                self.writable,
                self.executable,
                Some(self.user),
                None,
                None,
                None,
                ExtentRights::default(),
            )
            .expect("rights policy");
            X86_64PageTablePolicy::new(
                self.physical,
                self.virtual_,
                48,
                nx_supported,
                16,
                1024,
                right_policy,
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
            .mint(0x1000_0000, storage_pages * X86_64_PAGE_BYTES)
            .expect("storage");
            let table_grant = PageTableGrant::from_admitted_provider(
                id(21, PageTableGrantId::from_normalized_identity),
                self.physical,
                self.storage_provenance,
                rights(&[11]),
                self.virtual_,
                X86_64_PAGE_BYTES,
                X86_64_PAGE_BYTES,
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
            .mint(0x20_0000, X86_64_PAGE_BYTES)
            .expect("source");
            let destination = ExtentRootGrant::from_admitted_provider(
                id(25, ExtentLineageId::from_normalized_identity),
                self.virtual_,
                rights(&[11]),
                id(26, ExtentProvenanceId::from_normalized_identity),
                id(3, MappingEraId::from_normalized_identity),
            )
            .mint(destination_base, X86_64_PAGE_BYTES)
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
        let offset = table * X86_64_PAGE_BYTES as usize + slot * size_of::<u64>();
        u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("entry bytes"))
    }

    #[test]
    fn materializes_upper_half_mapping_with_exact_hierarchy_and_nx() {
        let fixture = Fixture::new();
        let virtual_address = 0xffff_8000_0000_1000;
        let draft = fixture.draft(
            virtual_address,
            ExtentRights::from_normalized_identities([
                fixture.readable,
                fixture.writable,
                fixture.user,
            ]),
            4,
        );
        let image =
            materialize_x86_64_4k_page_table(&draft, &fixture.policy(true)).expect("materialize");

        assert_eq!(image.plan(), draft.plan_identity());
        assert_eq!(image.root_physical_address(), 0x1000_0000);
        assert_eq!(image.table_page_count(), 4);
        assert_eq!(image.leaf_entry_count(), 1);
        assert_eq!(
            entry(image.bytes(), 0, 256),
            0x1000_1000 | PRESENT | WRITABLE | USER
        );
        assert_eq!(
            entry(image.bytes(), 1, 0),
            0x1000_2000 | PRESENT | WRITABLE | USER
        );
        assert_eq!(
            entry(image.bytes(), 2, 0),
            0x1000_3000 | PRESENT | WRITABLE | USER
        );
        assert_eq!(
            entry(image.bytes(), 3, 1),
            0x20_0000 | PRESENT | WRITABLE | USER | NO_EXECUTE
        );
        assert_ne!(image.content_fingerprint(), 0);
    }

    #[test]
    fn executable_leaf_omits_nx() {
        let fixture = Fixture::new();
        let draft = fixture.draft(
            0x4000,
            ExtentRights::from_normalized_identities([fixture.readable, fixture.executable]),
            4,
        );
        let image =
            materialize_x86_64_4k_page_table(&draft, &fixture.policy(true)).expect("materialize");
        assert_eq!(entry(image.bytes(), 3, 4), 0x20_0000 | PRESENT);
    }

    #[test]
    fn generated_image_enters_the_existing_installable_lifecycle() {
        let fixture = Fixture::new();
        let draft = fixture.draft(
            0x4000,
            ExtentRights::from_normalized_identities([fixture.readable]),
            4,
        );
        let image =
            materialize_x86_64_4k_page_table(&draft, &fixture.policy(true)).expect("materialize");
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
        assert_eq!(installable.bytes().len(), 4 * X86_64_PAGE_BYTES as usize);
    }

    #[test]
    fn non_executable_mapping_rejects_without_nx_support() {
        let fixture = Fixture::new();
        let draft = fixture.draft(
            0x4000,
            ExtentRights::from_normalized_identities([fixture.readable]),
            4,
        );
        let error = materialize_x86_64_4k_page_table(&draft, &fixture.policy(false))
            .expect_err("NX must be enforceable");
        assert!(error.0.contains("without NX"));
    }

    #[test]
    fn hierarchy_capacity_and_unknown_rights_fail_closed() {
        let fixture = Fixture::new();
        let too_small = fixture.draft(
            0x4000,
            ExtentRights::from_normalized_identities([fixture.readable]),
            3,
        );
        let error = materialize_x86_64_4k_page_table(&too_small, &fixture.policy(true))
            .expect_err("four-level hierarchy needs four pages");
        assert!(error.0.contains("cannot hold"));

        let unknown = right(99);
        let draft = fixture.draft(
            0x4000,
            ExtentRights::from_normalized_identities([fixture.readable, unknown]),
            4,
        );
        let error = materialize_x86_64_4k_page_table(&draft, &fixture.policy(true))
            .expect_err("unknown PTE meaning");
        assert!(error.0.contains("has no admitted"));
    }

    #[test]
    fn noncanonical_destination_rejects() {
        let fixture = Fixture::new();
        let draft = fixture.draft(
            0x0000_8000_0000_0000,
            ExtentRights::from_normalized_identities([fixture.readable]),
            4,
        );
        let error = materialize_x86_64_4k_page_table(&draft, &fixture.policy(true))
            .expect_err("canonical virtual address");
        assert!(error.0.contains("noncanonical"));
    }
}
