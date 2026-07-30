//! Normalized access policy for placed views.
//!
//! `LayoutPlan` owns geometry. `AccessPlan` owns observation and the exact
//! primitive operations permitted over that geometry. Keeping them separate
//! prevents wire layouts from acquiring MMIO vocabulary and prevents an
//! arbitrary-offset volatile escape hatch from bypassing plan validation.

use std::collections::{BTreeMap, BTreeSet};

use omega_core::atomic::{AtomicOrderingPlan, MemoryOrdering};
use omega_extents::{
    AddressSpaceId, ExtentLoan, ExtentProvenanceId, ExtentRights, LoanPolarity, MappingEraId,
};
use omega_layout_plans::{
    LayoutPlacementReport, LayoutPlanReport, normalized_layout_plan_fingerprint,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoundaryServiceReachId(u64);

impl BoundaryServiceReachId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, AccessPlanDiagnostic> {
        if identity == 0 {
            return Err(AccessPlanDiagnostic(
                "boundary-service reach identity cannot be zero".into(),
            ));
        }
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct BoundaryReach {
    services: BTreeSet<BoundaryServiceReachId>,
}

impl BoundaryReach {
    pub fn from_services(services: impl IntoIterator<Item = BoundaryServiceReachId>) -> Self {
        Self {
            services: services.into_iter().collect(),
        }
    }

    pub fn services(&self) -> impl ExactSizeIterator<Item = BoundaryServiceReachId> + '_ {
        self.services.iter().copied()
    }

    pub fn contains(&self, service: BoundaryServiceReachId) -> bool {
        self.services.contains(&service)
    }

    pub fn contains_all(&self, required: &Self) -> bool {
        required.services.is_subset(&self.services)
    }

    pub fn intersection(&self, other: &Self) -> Self {
        Self {
            services: self
                .services
                .intersection(&other.services)
                .copied()
                .collect(),
        }
    }
}

/// How repeated observations of the placed field relate to one another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObservationModel {
    /// Ordinary owned or immutably borrowed storage. The compiler may use its
    /// ordinary load/store rules.
    Stable,
    /// Another agent may change storage. Every authorized read/write is one
    /// exact-width external event; device ordering still requires fences.
    External,
    /// Shared mutation is legal only through the declared atomic operations.
    Atomic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AccessExposure {
    Exported,
    BindingPrivate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExternalRead {
    None,
    Read,
    Take,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AtomicPermissions {
    pub load: bool,
    pub store: bool,
    pub fetch_add: bool,
    pub fetch_sub: bool,
    pub fetch_xor: bool,
    pub fetch_or: bool,
    pub fetch_and: bool,
    pub swap: bool,
    pub compare_exchange: bool,
}

impl AtomicPermissions {
    pub const fn any(self) -> bool {
        self.load
            || self.store
            || self.fetch_add
            || self.fetch_sub
            || self.fetch_xor
            || self.fetch_or
            || self.fetch_and
            || self.swap
            || self.compare_exchange
    }

    pub const fn contains(self, required: Self) -> bool {
        (!required.load || self.load)
            && (!required.store || self.store)
            && (!required.fetch_add || self.fetch_add)
            && (!required.fetch_sub || self.fetch_sub)
            && (!required.fetch_xor || self.fetch_xor)
            && (!required.fetch_or || self.fetch_or)
            && (!required.fetch_and || self.fetch_and)
            && (!required.swap || self.swap)
            && (!required.compare_exchange || self.compare_exchange)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AccessPermissions {
    pub read: bool,
    pub take: bool,
    pub write: bool,
    pub atomic: AtomicPermissions,
}

impl AccessPermissions {
    pub const fn any(self) -> bool {
        self.read || self.take || self.write || self.atomic.any()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FieldAccess {
    Inaccessible,
    Stable {
        transfer_width_bits: u16,
        read: bool,
        write: bool,
        exposure: AccessExposure,
    },
    External {
        transfer_width_bits: u16,
        read: ExternalRead,
        write: bool,
        exposure: AccessExposure,
    },
    Atomic {
        transfer_width_bits: u16,
        operations: AtomicPermissions,
        exposure: AccessExposure,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AccessFieldKey {
    layout_fingerprint: u64,
    slot: u32,
}

impl AccessFieldKey {
    pub const fn slot(self) -> u32 {
        self.slot
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AccessFieldEntry {
    key: AccessFieldKey,
    field: String,
    access: FieldAccess,
}

impl AccessFieldEntry {
    pub const fn key(&self) -> AccessFieldKey {
        self.key
    }

    pub fn field(&self) -> &str {
        &self.field
    }

    pub const fn access(&self) -> &FieldAccess {
        &self.access
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AccessPlan {
    layout_fingerprint: u64,
    entries: Vec<AccessFieldEntry>,
}

impl AccessPlan {
    pub fn inaccessible(layout: &LayoutPlanReport) -> Result<Self, AccessPlanDiagnostic> {
        let layout_fingerprint = normalized_layout_plan_fingerprint(layout);
        let mut canonical_fields = BTreeMap::new();
        let mut presentation_names = BTreeMap::new();
        for entry in &layout.entries {
            if entry.field.is_empty() {
                return Err(AccessPlanDiagnostic(
                    "layout field name cannot be empty".into(),
                ));
            }
            let identity = match entry.member_identity {
                Some(identity) => CanonicalFieldIdentity::Numbered(identity),
                None => CanonicalFieldIdentity::Positional(entry.field.clone()),
            };
            if let Some(prior) = presentation_names.insert(identity.clone(), entry.field.clone())
                && prior != entry.field
            {
                return Err(AccessPlanDiagnostic(format!(
                    "layout field identity names both `{prior}` and `{}`",
                    entry.field
                )));
            }
            canonical_fields.insert(identity, entry.field.clone());
        }
        let entries = canonical_fields
            .into_values()
            .enumerate()
            .map(|(slot, field)| {
                let slot = u32::try_from(slot).map_err(|_| {
                    AccessPlanDiagnostic("layout has more than u32::MAX schema fields".into())
                })?;
                Ok(AccessFieldEntry {
                    key: AccessFieldKey {
                        layout_fingerprint,
                        slot,
                    },
                    field,
                    access: FieldAccess::Inaccessible,
                })
            })
            .collect::<Result<Vec<_>, AccessPlanDiagnostic>>()?;
        Ok(Self {
            layout_fingerprint,
            entries,
        })
    }

    pub const fn layout_fingerprint(&self) -> u64 {
        self.layout_fingerprint
    }

    pub fn entries(&self) -> &[AccessFieldEntry] {
        &self.entries
    }

    pub fn key_at(&self, slot: usize) -> Option<AccessFieldKey> {
        self.entries.get(slot).map(AccessFieldEntry::key)
    }

    pub fn set(
        &mut self,
        key: AccessFieldKey,
        access: FieldAccess,
    ) -> Result<(), AccessPlanDiagnostic> {
        if key.layout_fingerprint != self.layout_fingerprint {
            return Err(AccessPlanDiagnostic(
                "access field key belongs to a different validated layout".into(),
            ));
        }
        let entry = self.entries.get_mut(key.slot as usize).ok_or_else(|| {
            AccessPlanDiagnostic("access field key is outside the schema cardinality".into())
        })?;
        if entry.key != key {
            return Err(AccessPlanDiagnostic(
                "access field key does not identify this schema slot".into(),
            ));
        }
        entry.access = access;
        Ok(())
    }
}

/// Stable operations supplied by one admitted resource region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StableCapability {
    None,
    Read,
    Write,
    ReadWrite,
}

impl StableCapability {
    const fn permits(self, read: bool, write: bool) -> bool {
        (!read || matches!(self, Self::Read | Self::ReadWrite))
            && (!write || matches!(self, Self::Write | Self::ReadWrite))
    }

    const fn any(self) -> bool {
        !matches!(self, Self::None)
    }
}

/// Repeated-observation behavior supplied for external reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExternalReadBehavior {
    None,
    Repeatable,
    Destructive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TransferRule {
    pub width_bits: u16,
    pub alignment_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ExternalCapability {
    None,
    Access {
        read: ExternalReadBehavior,
        write: bool,
        transfers: Vec<TransferRule>,
    },
}

impl ExternalCapability {
    fn any(&self) -> bool {
        !matches!(self, Self::None)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AtomicTransferRule {
    pub transfer: TransferRule,
    pub operations: AtomicPermissions,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AtomicCapability {
    None,
    Access { transfers: Vec<AtomicTransferRule> },
}

impl AtomicCapability {
    fn any(&self) -> bool {
        !matches!(self, Self::None)
    }
}

/// One provider-supplied relative interval. Regions are normalized and
/// disjoint; uncovered bytes intentionally supply no operation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResourceRegion {
    pub offset: u64,
    pub length: u64,
    pub stable: StableCapability,
    pub external: ExternalCapability,
    pub atomic: AtomicCapability,
    pub reach: BoundaryReach,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct ResourceProfile {
    pub regions: Vec<ResourceRegion>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceProfileId(u64);

impl ResourceProfileId {
    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

/// Canonical provider supply over one relative range length.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedResourceProfile {
    identity: ResourceProfileId,
    length: u64,
    regions: Vec<ResourceRegion>,
}

impl ValidatedResourceProfile {
    pub const fn identity(&self) -> ResourceProfileId {
        self.identity
    }

    pub const fn length(&self) -> u64 {
        self.length
    }

    pub fn regions(&self) -> &[ResourceRegion] {
        &self.regions
    }

    /// Intersect with a child interval, rebase retained regions to child zero,
    /// and attenuate every region's reach.
    pub fn restrict(
        &self,
        offset: u64,
        length: u64,
        permitted_reach: &BoundaryReach,
    ) -> Result<Self, AccessPlanDiagnostic> {
        if length == 0 {
            return Err(AccessPlanDiagnostic(
                "resource-profile restriction cannot be empty".into(),
            ));
        }
        let end = offset.checked_add(length).ok_or_else(|| {
            AccessPlanDiagnostic("resource-profile restriction range overflows".into())
        })?;
        if end > self.length {
            return Err(AccessPlanDiagnostic(format!(
                "resource-profile restriction {offset}..{end} exceeds {}-byte parent",
                self.length
            )));
        }
        let mut regions = Vec::new();
        for region in &self.regions {
            let region_end = region.offset + region.length;
            let start = region.offset.max(offset);
            let retained_end = region_end.min(end);
            if start >= retained_end {
                continue;
            }
            regions.push(ResourceRegion {
                offset: start - offset,
                length: retained_end - start,
                stable: region.stable,
                external: region.external.clone(),
                atomic: region.atomic.clone(),
                reach: region.reach.intersection(permitted_reach),
            });
        }
        validate_resource_profile(ResourceProfile { regions }, length)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum CanonicalFieldIdentity {
    Numbered(u64),
    Positional(String),
}

/// Normalizer-owned identity of one validated access policy.
///
/// The plan contains exactly one canonical slot per layout schema field,
/// including inaccessible fields. Its identity includes every operation,
/// observation, exposure, and transfer-width fact that lowering is allowed to
/// consume. Boundary reach belongs to the enclosing placement identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AccessPlanId(u64);

impl AccessPlanId {
    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

/// Sealed geometry and policy for one projected field.
///
/// The offset is intentionally private. Only plan validation can construct a
/// descriptor, so later lowering never accepts an author-supplied byte offset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldAccessDescriptor {
    key: AccessFieldKey,
    field: String,
    container_byte_offset: u64,
    transfer_width_bits: u16,
    observation: ObservationModel,
    permissions: AccessPermissions,
    exposure: AccessExposure,
}

impl FieldAccessDescriptor {
    pub const fn key(&self) -> AccessFieldKey {
        self.key
    }

    pub fn field(&self) -> &str {
        &self.field
    }

    pub const fn container_byte_offset(&self) -> u64 {
        self.container_byte_offset
    }

    pub const fn transfer_width_bits(&self) -> u16 {
        self.transfer_width_bits
    }

    pub const fn observation(&self) -> ObservationModel {
        self.observation
    }

    pub const fn permissions(&self) -> AccessPermissions {
        self.permissions
    }

    pub const fn exposure(&self) -> AccessExposure {
        self.exposure
    }
}

/// The only value accepted by primitive placed-access lowering.
///
/// It combines plan-derived geometry with a borrow-specific operation check.
/// Callers carry compiler-issued field keys and operations but cannot
/// construct this token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizedFieldAccess {
    descriptor: FieldAccessDescriptor,
    current_borrow: BorrowPolarity,
    source_loan: BorrowPolarity,
    operation: AccessOperation,
}

impl AuthorizedFieldAccess {
    pub const fn descriptor(&self) -> &FieldAccessDescriptor {
        &self.descriptor
    }

    pub const fn current_borrow(&self) -> BorrowPolarity {
        self.current_borrow
    }

    pub const fn source_loan(&self) -> BorrowPolarity {
        self.source_loan
    }

    pub const fn operation(&self) -> AccessOperation {
        self.operation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedAccessPlan {
    identity: AccessPlanId,
    layout_fingerprint: u64,
    plan: AccessPlan,
    fields: Vec<FieldAccessDescriptor>,
    layout_size_bytes: u64,
}

impl ValidatedAccessPlan {
    pub const fn identity(&self) -> AccessPlanId {
        self.identity
    }

    pub const fn layout_fingerprint(&self) -> u64 {
        self.layout_fingerprint
    }

    pub const fn plan(&self) -> &AccessPlan {
        &self.plan
    }

    pub fn field(&self, key: AccessFieldKey) -> Option<&AccessFieldEntry> {
        self.plan
            .entries
            .get(key.slot as usize)
            .filter(|entry| entry.key == key)
    }

    pub fn field_descriptor(&self, key: AccessFieldKey) -> Option<&FieldAccessDescriptor> {
        self.fields.iter().find(|entry| entry.key == key)
    }

    pub fn field_descriptors(&self) -> &[FieldAccessDescriptor] {
        &self.fields
    }

    pub const fn layout_size_bytes(&self) -> u64 {
        self.layout_size_bytes
    }

    pub fn authorize(
        &self,
        key: AccessFieldKey,
        current_borrow: BorrowPolarity,
        source_loan: BorrowPolarity,
        operation: AccessOperation,
    ) -> Result<AuthorizedFieldAccess, AccessPlanDiagnostic> {
        let entry = self.field(key).ok_or_else(|| {
            AccessPlanDiagnostic("field key does not belong to the validated access plan".into())
        })?;
        let descriptor = self.field_descriptor(key).ok_or_else(|| {
            AccessPlanDiagnostic(format!("field `{}` is inaccessible", entry.field))
        })?;
        authorize_descriptor(descriptor, current_borrow, source_loan, operation)?;
        Ok(AuthorizedFieldAccess {
            descriptor: descriptor.clone(),
            current_borrow,
            source_loan,
            operation,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementPlan {
    pub layout: LayoutPlanReport,
    pub access: AccessPlan,
    pub reach: BoundaryReach,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlacementPlanId(u64);

impl PlacementPlanId {
    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPlacementPlan {
    identity: PlacementPlanId,
    layout: LayoutPlanReport,
    access: ValidatedAccessPlan,
    reach: BoundaryReach,
}

impl ValidatedPlacementPlan {
    pub const fn identity(&self) -> PlacementPlanId {
        self.identity
    }

    pub const fn layout(&self) -> &LayoutPlanReport {
        &self.layout
    }

    pub const fn access(&self) -> &ValidatedAccessPlan {
        &self.access
    }

    pub const fn reach(&self) -> &BoundaryReach {
        &self.reach
    }
}

/// Normalized power-of-two constraint on the concrete loan base.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BaseCongruence {
    modulus: u64,
    residue: u64,
}

impl BaseCongruence {
    pub const fn modulus(self) -> u64 {
        self.modulus
    }

    pub const fn residue(self) -> u64 {
        self.residue
    }

    pub const fn admits(self, base: u64) -> bool {
        base % self.modulus == self.residue
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectiveSupplyKind {
    Stable,
    External,
    Atomic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveFieldSupply {
    key: AccessFieldKey,
    field: String,
    offset: u64,
    width_bits: u16,
    alignment_bytes: u64,
    kind: EffectiveSupplyKind,
}

impl EffectiveFieldSupply {
    pub const fn key(&self) -> AccessFieldKey {
        self.key
    }

    pub fn field(&self) -> &str {
        &self.field
    }

    pub const fn offset(&self) -> u64 {
        self.offset
    }

    pub const fn width_bits(&self) -> u16 {
        self.width_bits
    }

    pub const fn alignment_bytes(&self) -> u64 {
        self.alignment_bytes
    }

    pub const fn kind(&self) -> EffectiveSupplyKind {
        self.kind
    }
}

/// Sealed result of joining one normalized placement demand with one
/// normalized provider profile before a concrete loan is admitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementResourceCompatibility {
    placement: PlacementPlanId,
    profile: ResourceProfileId,
    base: BaseCongruence,
    fields: Vec<EffectiveFieldSupply>,
}

impl PlacementResourceCompatibility {
    pub const fn placement(&self) -> PlacementPlanId {
        self.placement
    }

    pub const fn profile(&self) -> ResourceProfileId {
        self.profile
    }

    pub const fn base_congruence(&self) -> BaseCongruence {
        self.base
    }

    pub fn fields(&self) -> &[EffectiveFieldSupply] {
        &self.fields
    }

    fn field(&self, key: AccessFieldKey) -> Option<&EffectiveFieldSupply> {
        self.fields.iter().find(|field| field.key == key)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BorrowPolarity {
    Shared,
    Exclusive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessOperation {
    Read,
    /// One destructive external read. It consumes the device event through an
    /// exclusive current borrow and never derives ordinary readability.
    Take,
    Write,
    /// Ordinary stable compound mutation. Legality is derived from stable
    /// read+write permission and an exclusive borrow; it is never an external
    /// primitive permission.
    CompoundMutation,
    /// One atomic operation carrying the exact source-selected ordering plan.
    ///
    /// The exact operation family remains distinct while its ordering converts
    /// to the shared `AtomicOrderingPlan` carried by the compiler pipeline.
    Atomic(AtomicAccessOperation),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AtomicAccessOperation {
    Load(MemoryOrdering),
    Store(MemoryOrdering),
    FetchAdd(MemoryOrdering),
    FetchSub(MemoryOrdering),
    FetchXor(MemoryOrdering),
    FetchOr(MemoryOrdering),
    FetchAnd(MemoryOrdering),
    Swap(MemoryOrdering),
    CompareExchange {
        success: MemoryOrdering,
        failure: MemoryOrdering,
    },
}

impl AtomicAccessOperation {
    pub const fn ordering_plan(self) -> AtomicOrderingPlan {
        match self {
            Self::Load(ordering) => AtomicOrderingPlan::Load(ordering),
            Self::Store(ordering) => AtomicOrderingPlan::Store(ordering),
            Self::FetchAdd(ordering)
            | Self::FetchSub(ordering)
            | Self::FetchXor(ordering)
            | Self::FetchOr(ordering)
            | Self::FetchAnd(ordering) => AtomicOrderingPlan::ReadModifyWrite(ordering),
            Self::Swap(ordering) => AtomicOrderingPlan::Swap(ordering),
            Self::CompareExchange { success, failure } => {
                AtomicOrderingPlan::CompareExchange { success, failure }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessPlanDiagnostic(pub String);

impl std::fmt::Display for AccessPlanDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for AccessPlanDiagnostic {}

pub fn validate_resource_profile(
    mut profile: ResourceProfile,
    length: u64,
) -> Result<ValidatedResourceProfile, AccessPlanDiagnostic> {
    if length == 0 {
        return Err(AccessPlanDiagnostic(
            "resource profile must describe a nonempty range".into(),
        ));
    }
    profile
        .regions
        .sort_by_key(|region| (region.offset, region.length));
    let mut normalized: Vec<ResourceRegion> = Vec::with_capacity(profile.regions.len());
    for mut region in profile.regions {
        if region.length == 0 {
            return Err(AccessPlanDiagnostic(
                "resource-profile region cannot be empty".into(),
            ));
        }
        let end = region.offset.checked_add(region.length).ok_or_else(|| {
            AccessPlanDiagnostic("resource-profile region range overflows".into())
        })?;
        if end > length {
            return Err(AccessPlanDiagnostic(format!(
                "resource-profile region {}..{end} exceeds {length}-byte profile",
                region.offset
            )));
        }
        normalize_external_capability(&mut region.external)?;
        normalize_atomic_capability(&mut region.atomic)?;
        if !region.stable.any() && !region.external.any() && !region.atomic.any() {
            return Err(AccessPlanDiagnostic(format!(
                "resource-profile region {}..{end} supplies no operation",
                region.offset
            )));
        }
        if let Some(previous) = normalized.last_mut() {
            let previous_end = previous.offset + previous.length;
            if region.offset < previous_end {
                return Err(AccessPlanDiagnostic(format!(
                    "resource-profile regions {}..{} and {}..{end} overlap",
                    previous.offset, previous_end, region.offset
                )));
            }
            if region.offset == previous_end
                && previous.stable == region.stable
                && previous.external == region.external
                && previous.atomic == region.atomic
                && previous.reach == region.reach
            {
                previous.length = previous.length.checked_add(region.length).ok_or_else(|| {
                    AccessPlanDiagnostic("merged resource-profile region length overflows".into())
                })?;
                continue;
            }
        }
        normalized.push(region);
    }
    let identity = normalized_resource_profile_identity(length, &normalized);
    Ok(ValidatedResourceProfile {
        identity,
        length,
        regions: normalized,
    })
}

fn normalize_external_capability(
    capability: &mut ExternalCapability,
) -> Result<(), AccessPlanDiagnostic> {
    let ExternalCapability::Access {
        read,
        write,
        transfers,
    } = capability
    else {
        return Ok(());
    };
    if *read == ExternalReadBehavior::None && !*write {
        return Err(AccessPlanDiagnostic(
            "external capability supplies no operation; use None".into(),
        ));
    }
    normalize_transfer_rules(transfers)?;
    if transfers.is_empty() {
        return Err(AccessPlanDiagnostic(
            "external capability must list at least one transfer rule".into(),
        ));
    }
    Ok(())
}

fn normalize_atomic_capability(
    capability: &mut AtomicCapability,
) -> Result<(), AccessPlanDiagnostic> {
    let AtomicCapability::Access { transfers } = capability else {
        return Ok(());
    };
    transfers.sort_by_key(|rule| rule.transfer.width_bits);
    let mut prior_width = None;
    for rule in transfers.iter() {
        validate_transfer_rule(rule.transfer)?;
        if !rule.operations.any() {
            return Err(AccessPlanDiagnostic(format!(
                "atomic {}-bit transfer supplies no operation",
                rule.transfer.width_bits
            )));
        }
        if prior_width.replace(rule.transfer.width_bits) == Some(rule.transfer.width_bits) {
            return Err(AccessPlanDiagnostic(format!(
                "atomic capability repeats {}-bit transfer width",
                rule.transfer.width_bits
            )));
        }
    }
    if transfers.is_empty() {
        return Err(AccessPlanDiagnostic(
            "atomic capability must list at least one transfer rule".into(),
        ));
    }
    Ok(())
}

fn normalize_transfer_rules(transfers: &mut [TransferRule]) -> Result<(), AccessPlanDiagnostic> {
    transfers.sort_by_key(|rule| rule.width_bits);
    let mut prior_width = None;
    for rule in transfers.iter().copied() {
        validate_transfer_rule(rule)?;
        if prior_width.replace(rule.width_bits) == Some(rule.width_bits) {
            return Err(AccessPlanDiagnostic(format!(
                "external capability repeats {}-bit transfer width",
                rule.width_bits
            )));
        }
    }
    Ok(())
}

fn validate_transfer_rule(rule: TransferRule) -> Result<(), AccessPlanDiagnostic> {
    if rule.width_bits == 0 || rule.width_bits > 128 || !rule.width_bits.is_multiple_of(8) {
        return Err(AccessPlanDiagnostic(format!(
            "resource transfer width {} is not a supported whole-byte width in 8..=128",
            rule.width_bits
        )));
    }
    if rule.alignment_bytes == 0 || !rule.alignment_bytes.is_power_of_two() {
        return Err(AccessPlanDiagnostic(format!(
            "resource transfer alignment {} is not a positive power of two",
            rule.alignment_bytes
        )));
    }
    Ok(())
}

pub fn validate_access_plan(
    plan: AccessPlan,
    layout: &LayoutPlanReport,
) -> Result<ValidatedAccessPlan, AccessPlanDiagnostic> {
    let layout_size = layout.size.ok_or_else(|| {
        AccessPlanDiagnostic("placed access requires a fixed-size layout plan".into())
    })?;
    let expected = AccessPlan::inaccessible(layout)?;
    if plan.layout_fingerprint != expected.layout_fingerprint {
        return Err(AccessPlanDiagnostic(
            "access plan belongs to a different validated layout".into(),
        ));
    }
    if plan.entries.len() != expected.entries.len()
        || plan
            .entries
            .iter()
            .zip(&expected.entries)
            .any(|(actual, expected)| actual.key != expected.key || actual.field != expected.field)
    {
        return Err(AccessPlanDiagnostic(
            "access plan does not contain exactly one canonical decision per schema field".into(),
        ));
    }

    let mut descriptors = Vec::with_capacity(plan.entries.len());
    for entry in &plan.entries {
        let Some(policy) = validate_entry_policy(entry)? else {
            continue;
        };
        let container_byte_offset = validate_entry_geometry(
            &entry.field,
            policy.transfer_width_bits,
            layout,
            layout_size,
        )?;
        descriptors.push(FieldAccessDescriptor {
            key: entry.key,
            field: entry.field.clone(),
            container_byte_offset,
            transfer_width_bits: policy.transfer_width_bits,
            observation: policy.observation,
            permissions: policy.permissions,
            exposure: policy.exposure,
        });
    }

    let layout_fingerprint = plan.layout_fingerprint;
    let identity = normalized_access_plan_identity(&plan, layout_fingerprint);
    Ok(ValidatedAccessPlan {
        identity,
        layout_fingerprint,
        plan,
        fields: descriptors,
        layout_size_bytes: layout_size,
    })
}

pub fn validate_placement_plan(
    plan: PlacementPlan,
) -> Result<ValidatedPlacementPlan, AccessPlanDiagnostic> {
    let PlacementPlan {
        layout,
        access,
        reach,
    } = plan;
    let access = validate_access_plan(access, &layout)?;
    let identity = normalized_placement_plan_identity(access.identity(), &reach);
    Ok(ValidatedPlacementPlan {
        identity,
        layout,
        access,
        reach,
    })
}

pub fn validate_placement_resources(
    plan: &ValidatedPlacementPlan,
    profile: &ValidatedResourceProfile,
) -> Result<PlacementResourceCompatibility, AccessPlanDiagnostic> {
    if plan.access.layout_size_bytes > profile.length {
        return Err(AccessPlanDiagnostic(format!(
            "{}-byte placed layout exceeds {}-byte resource profile",
            plan.access.layout_size_bytes, profile.length
        )));
    }
    let mut congruence = CongruenceAccumulator {
        value: BaseCongruence {
            modulus: 1,
            residue: 0,
        },
        source: "unconstrained base".into(),
    };
    require_base_congruence(&mut congruence, "layout base", 0, plan.layout.align)?;

    let mut fields = Vec::with_capacity(plan.access.fields.len());
    for descriptor in &plan.access.fields {
        let width_bytes = u64::from(descriptor.transfer_width_bits / 8);
        let end = descriptor
            .container_byte_offset
            .checked_add(width_bytes)
            .ok_or_else(|| {
                AccessPlanDiagnostic(format!(
                    "field `{}` resource interval overflows",
                    descriptor.field
                ))
            })?;
        let region = profile
            .regions
            .iter()
            .find(|region| {
                descriptor.container_byte_offset >= region.offset
                    && end <= region.offset + region.length
            })
            .ok_or_else(|| {
                AccessPlanDiagnostic(format!(
                    "field `{}` transfer at {}..{end} is not covered by one resource region",
                    descriptor.field, descriptor.container_byte_offset
                ))
            })?;
        if !region.reach.contains_all(&plan.reach) {
            return Err(AccessPlanDiagnostic(format!(
                "resource region covering field `{}` does not supply the placement's complete boundary reach",
                descriptor.field
            )));
        }
        let access = plan
            .access
            .field(descriptor.key)
            .expect("validated descriptor must retain its source access decision")
            .access();
        let (kind, alignment_bytes) = select_effective_supply(&descriptor.field, access, region)?;
        require_base_congruence(
            &mut congruence,
            descriptor.field.as_str(),
            descriptor.container_byte_offset,
            alignment_bytes,
        )?;
        fields.push(EffectiveFieldSupply {
            key: descriptor.key,
            field: descriptor.field.clone(),
            offset: descriptor.container_byte_offset,
            width_bits: descriptor.transfer_width_bits,
            alignment_bytes,
            kind,
        });
    }
    Ok(PlacementResourceCompatibility {
        placement: plan.identity,
        profile: profile.identity,
        base: congruence.value,
        fields,
    })
}

fn select_effective_supply(
    field: &str,
    access: &FieldAccess,
    region: &ResourceRegion,
) -> Result<(EffectiveSupplyKind, u64), AccessPlanDiagnostic> {
    match access {
        FieldAccess::Inaccessible => {
            unreachable!("inaccessible fields do not have validated descriptors")
        }
        FieldAccess::Stable {
            transfer_width_bits,
            read,
            write,
            ..
        } => {
            if !region.stable.permits(*read, *write) {
                return Err(AccessPlanDiagnostic(format!(
                    "field `{field}` requests Stable read={read} write={write}, but its resource region does not supply them"
                )));
            }
            Ok((
                EffectiveSupplyKind::Stable,
                stable_transfer_alignment(*transfer_width_bits),
            ))
        }
        FieldAccess::External {
            transfer_width_bits,
            read,
            write,
            ..
        } => {
            if let ExternalCapability::Access {
                read: supplied_read,
                write: supplied_write,
                transfers,
            } = &region.external
                && external_read_compatible(*read, *supplied_read)
                && (!*write || *supplied_write)
                && let Some(rule) = transfers
                    .iter()
                    .find(|rule| rule.width_bits == *transfer_width_bits)
            {
                return Ok((EffectiveSupplyKind::External, rule.alignment_bytes));
            }
            let stable_read = *read == ExternalRead::Read;
            if *read != ExternalRead::Take && region.stable.permits(stable_read, *write) {
                return Ok((
                    EffectiveSupplyKind::Stable,
                    stable_transfer_alignment(*transfer_width_bits),
                ));
            }
            Err(AccessPlanDiagnostic(format!(
                "field `{field}` requests incompatible External {transfer_width_bits}-bit read={read:?} write={write}"
            )))
        }
        FieldAccess::Atomic {
            transfer_width_bits,
            operations,
            ..
        } => {
            let AtomicCapability::Access { transfers } = &region.atomic else {
                return Err(AccessPlanDiagnostic(format!(
                    "field `{field}` requests Atomic access, but its resource region supplies none"
                )));
            };
            let rule = transfers
                .iter()
                .find(|rule| {
                    rule.transfer.width_bits == *transfer_width_bits
                        && rule.operations.contains(*operations)
                })
                .ok_or_else(|| {
                    AccessPlanDiagnostic(format!(
                        "field `{field}` requests unsupported Atomic {transfer_width_bits}-bit operation families"
                    ))
                })?;
            Ok((EffectiveSupplyKind::Atomic, rule.transfer.alignment_bytes))
        }
    }
}

const fn external_read_compatible(demand: ExternalRead, supply: ExternalReadBehavior) -> bool {
    match demand {
        ExternalRead::None => true,
        ExternalRead::Read => matches!(supply, ExternalReadBehavior::Repeatable),
        ExternalRead::Take => matches!(supply, ExternalReadBehavior::Destructive),
    }
}

const fn stable_transfer_alignment(width_bits: u16) -> u64 {
    let width_bytes = width_bits / 8;
    if width_bytes.is_power_of_two() {
        width_bytes as u64
    } else {
        1
    }
}

struct CongruenceAccumulator {
    value: BaseCongruence,
    source: String,
}

fn require_base_congruence(
    accumulated: &mut CongruenceAccumulator,
    source: &str,
    offset: u64,
    alignment: u64,
) -> Result<(), AccessPlanDiagnostic> {
    if alignment == 0 || !alignment.is_power_of_two() {
        return Err(AccessPlanDiagnostic(format!(
            "field `{source}` requires invalid transfer alignment {alignment}"
        )));
    }
    let required = BaseCongruence {
        modulus: alignment,
        residue: (alignment - offset % alignment) % alignment,
    };
    let shared_modulus = accumulated.value.modulus.min(required.modulus);
    if accumulated.value.residue % shared_modulus != required.residue % shared_modulus {
        return Err(AccessPlanDiagnostic(format!(
            "field `{source}` at offset {offset} with {alignment}-byte transfer alignment conflicts with {} (base mod {} = {}, required base mod {alignment} = {})",
            accumulated.source,
            accumulated.value.modulus,
            accumulated.value.residue,
            required.residue
        )));
    }
    if required.modulus > accumulated.value.modulus {
        accumulated.value = required;
        accumulated.source =
            format!("field `{source}` at offset {offset} with {alignment}-byte transfer alignment");
    }
    Ok(())
}

fn normalized_placement_plan_identity(
    access: AccessPlanId,
    reach: &BoundaryReach,
) -> PlacementPlanId {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(&mut hash, b"omega.placement-plan.v1");
    hash_u64(&mut hash, access.normalized_identity());
    hash_u64(&mut hash, reach.services().len() as u64);
    for service in reach.services() {
        hash_u64(&mut hash, service.normalized_identity());
    }
    PlacementPlanId(if hash == 0 { 1 } else { hash })
}

fn normalized_resource_profile_identity(
    length: u64,
    regions: &[ResourceRegion],
) -> ResourceProfileId {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(&mut hash, b"omega.resource-profile.v1");
    hash_u64(&mut hash, length);
    hash_u64(&mut hash, regions.len() as u64);
    for region in regions {
        hash_u64(&mut hash, region.offset);
        hash_u64(&mut hash, region.length);
        hash_byte(
            &mut hash,
            match region.stable {
                StableCapability::None => 0,
                StableCapability::Read => 1,
                StableCapability::Write => 2,
                StableCapability::ReadWrite => 3,
            },
        );
        match &region.external {
            ExternalCapability::None => hash_byte(&mut hash, 0),
            ExternalCapability::Access {
                read,
                write,
                transfers,
            } => {
                hash_byte(&mut hash, 1);
                hash_byte(
                    &mut hash,
                    match read {
                        ExternalReadBehavior::None => 0,
                        ExternalReadBehavior::Repeatable => 1,
                        ExternalReadBehavior::Destructive => 2,
                    },
                );
                hash_byte(&mut hash, u8::from(*write));
                hash_transfer_rules(&mut hash, transfers);
            }
        }
        match &region.atomic {
            AtomicCapability::None => hash_byte(&mut hash, 0),
            AtomicCapability::Access { transfers } => {
                hash_byte(&mut hash, 1);
                hash_u64(&mut hash, transfers.len() as u64);
                for rule in transfers {
                    hash_transfer_rule(&mut hash, rule.transfer);
                    hash_atomic_permissions(&mut hash, rule.operations);
                }
            }
        }
        hash_u64(&mut hash, region.reach.services().len() as u64);
        for service in region.reach.services() {
            hash_u64(&mut hash, service.normalized_identity());
        }
    }
    ResourceProfileId(if hash == 0 { 1 } else { hash })
}

fn hash_transfer_rules(hash: &mut u64, rules: &[TransferRule]) {
    hash_u64(hash, rules.len() as u64);
    for rule in rules {
        hash_transfer_rule(hash, *rule);
    }
}

fn hash_transfer_rule(hash: &mut u64, rule: TransferRule) {
    hash_u64(hash, u64::from(rule.width_bits));
    hash_u64(hash, rule.alignment_bytes);
}

fn hash_atomic_permissions(hash: &mut u64, permissions: AtomicPermissions) {
    for enabled in [
        permissions.load,
        permissions.store,
        permissions.fetch_add,
        permissions.fetch_sub,
        permissions.fetch_xor,
        permissions.fetch_or,
        permissions.fetch_and,
        permissions.swap,
        permissions.compare_exchange,
    ] {
        hash_byte(hash, u8::from(enabled));
    }
}

fn normalized_access_plan_identity(plan: &AccessPlan, layout_fingerprint: u64) -> AccessPlanId {
    // FNV-1a is used as a compact deterministic artifact identity here, never
    // as authorization or collision-resistant evidence. The versioned prefix
    // makes any future vocabulary change an explicit identity migration.
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(&mut hash, b"omega.access-plan.v4");
    hash_u64(&mut hash, layout_fingerprint);
    hash_u64(&mut hash, plan.entries.len() as u64);
    for entry in &plan.entries {
        hash_u64(&mut hash, u64::from(entry.key.slot));
        match &entry.access {
            FieldAccess::Inaccessible => hash_byte(&mut hash, 0),
            FieldAccess::Stable {
                transfer_width_bits,
                read,
                write,
                exposure,
            } => {
                hash_byte(&mut hash, 1);
                hash_u64(&mut hash, u64::from(*transfer_width_bits));
                hash_byte(&mut hash, u8::from(*read));
                hash_byte(&mut hash, u8::from(*write));
                hash_exposure(&mut hash, *exposure);
            }
            FieldAccess::External {
                transfer_width_bits,
                read,
                write,
                exposure,
            } => {
                hash_byte(&mut hash, 2);
                hash_u64(&mut hash, u64::from(*transfer_width_bits));
                hash_byte(
                    &mut hash,
                    match read {
                        ExternalRead::None => 0,
                        ExternalRead::Read => 1,
                        ExternalRead::Take => 2,
                    },
                );
                hash_byte(&mut hash, u8::from(*write));
                hash_exposure(&mut hash, *exposure);
            }
            FieldAccess::Atomic {
                transfer_width_bits,
                operations,
                exposure,
            } => {
                hash_byte(&mut hash, 3);
                hash_u64(&mut hash, u64::from(*transfer_width_bits));
                hash_atomic_permissions(&mut hash, *operations);
                hash_exposure(&mut hash, *exposure);
            }
        }
    }
    // Zero is reserved as the inert/no-plan identity throughout the semantic
    // spine. A hash hitting it remains deterministic but is remapped out of
    // the reserved value.
    AccessPlanId(if hash == 0 { 1 } else { hash })
}

fn hash_exposure(hash: &mut u64, exposure: AccessExposure) {
    hash_byte(
        hash,
        match exposure {
            AccessExposure::Exported => 0,
            AccessExposure::BindingPrivate => 1,
        },
    );
}

fn hash_u64(hash: &mut u64, value: u64) {
    hash_bytes(hash, &value.to_le_bytes());
}

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        hash_byte(hash, *byte);
    }
}

fn hash_byte(hash: &mut u64, byte: u8) {
    *hash ^= u64::from(byte);
    *hash = hash.wrapping_mul(0x100000001b3);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceProfileReceiptId(u64);

impl ResourceProfileReceiptId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, AccessPlanDiagnostic> {
        if identity == 0 {
            return Err(AccessPlanDiagnostic(
                "resource-profile receipt identity cannot be zero".into(),
            ));
        }
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

/// Provider-only authority to bind one normalized profile to one exact range
/// and provenance tuple.
#[derive(Debug)]
pub struct ResourceProfileGrant {
    receipt: ResourceProfileReceiptId,
    base: u64,
    length: u64,
    address_space: AddressSpaceId,
    provenance: ExtentProvenanceId,
    era: MappingEraId,
    required_rights: ExtentRights,
    permitted_reach: BoundaryReach,
}

impl ResourceProfileGrant {
    #[allow(clippy::too_many_arguments)]
    pub fn from_admitted_provider(
        receipt: ResourceProfileReceiptId,
        base: u64,
        length: u64,
        address_space: AddressSpaceId,
        provenance: ExtentProvenanceId,
        era: MappingEraId,
        required_rights: ExtentRights,
        permitted_reach: BoundaryReach,
    ) -> Result<Self, AccessPlanDiagnostic> {
        if length == 0 {
            return Err(AccessPlanDiagnostic(
                "resource-profile grant cannot bind an empty range".into(),
            ));
        }
        base.checked_add(length)
            .ok_or_else(|| AccessPlanDiagnostic("resource-profile grant range overflows".into()))?;
        Ok(Self {
            receipt,
            base,
            length,
            address_space,
            provenance,
            era,
            required_rights,
            permitted_reach,
        })
    }

    pub fn admit(
        self,
        profile: ResourceProfile,
    ) -> Result<AdmittedResourceProfile, ResourceProfileAdmissionError> {
        let validated = match validate_resource_profile(profile.clone(), self.length) {
            Ok(validated) => validated,
            Err(diagnostic) => {
                return Err(ResourceProfileAdmissionError {
                    grant: Box::new(self),
                    profile,
                    diagnostic,
                });
            }
        };
        if let Some(region) = validated
            .regions
            .iter()
            .find(|region| !self.permitted_reach.contains_all(&region.reach))
        {
            return Err(ResourceProfileAdmissionError {
                grant: Box::new(self),
                profile,
                diagnostic: AccessPlanDiagnostic(format!(
                    "resource region {}..{} claims reach outside the provider grant",
                    region.offset,
                    region.offset + region.length
                )),
            });
        }
        Ok(AdmittedResourceProfile {
            receipt: self.receipt,
            base: self.base,
            length: self.length,
            address_space: self.address_space,
            provenance: self.provenance,
            era: self.era,
            required_rights: self.required_rights,
            permitted_reach: self.permitted_reach,
            profile: validated,
        })
    }
}

#[derive(Debug)]
pub struct ResourceProfileAdmissionError {
    grant: Box<ResourceProfileGrant>,
    profile: ResourceProfile,
    diagnostic: AccessPlanDiagnostic,
}

impl ResourceProfileAdmissionError {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ResourceProfileGrant, ResourceProfile, AccessPlanDiagnostic) {
        (*self.grant, self.profile, self.diagnostic)
    }
}

#[derive(Debug, Clone)]
pub struct AdmittedResourceProfile {
    receipt: ResourceProfileReceiptId,
    base: u64,
    length: u64,
    address_space: AddressSpaceId,
    provenance: ExtentProvenanceId,
    era: MappingEraId,
    required_rights: ExtentRights,
    permitted_reach: BoundaryReach,
    profile: ValidatedResourceProfile,
}

impl AdmittedResourceProfile {
    pub const fn receipt(&self) -> ResourceProfileReceiptId {
        self.receipt
    }

    pub const fn profile(&self) -> &ValidatedResourceProfile {
        &self.profile
    }

    fn restrict_to_loan(
        &self,
        loan: &ExtentLoan<'_>,
    ) -> Result<ValidatedResourceProfile, AccessPlanDiagnostic> {
        if loan.address_space() != self.address_space {
            return Err(AccessPlanDiagnostic(
                "extent address space does not match admitted resource profile".into(),
            ));
        }
        if loan.provenance() != self.provenance {
            return Err(AccessPlanDiagnostic(
                "extent provenance does not match admitted resource profile".into(),
            ));
        }
        if loan.era() != self.era {
            return Err(AccessPlanDiagnostic(
                "extent mapping era does not match admitted resource profile".into(),
            ));
        }
        if !loan.rights().contains(&self.required_rights) {
            return Err(AccessPlanDiagnostic(
                "extent lacks rights bound into the admitted resource profile".into(),
            ));
        }
        let offset = loan.base().checked_sub(self.base).ok_or_else(|| {
            AccessPlanDiagnostic(
                "extent loan begins before the admitted resource-profile range".into(),
            )
        })?;
        let end = offset.checked_add(loan.length()).ok_or_else(|| {
            AccessPlanDiagnostic("extent loan range overflows resource profile".into())
        })?;
        if end > self.length {
            return Err(AccessPlanDiagnostic(format!(
                "extent loan relative range {offset}..{end} exceeds {}-byte admitted resource profile",
                self.length
            )));
        }
        self.profile
            .restrict(offset, loan.length(), &self.permitted_reach)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlacementAdmissionId(u64);

impl PlacementAdmissionId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, AccessPlanDiagnostic> {
        if identity == 0 {
            return Err(AccessPlanDiagnostic(
                "placement-admission identity cannot be zero".into(),
            ));
        }
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

/// One accepted placement that owns the exact extent loan checked by the
/// provider. It cannot be reused to admit another range or another loan.
#[derive(Debug)]
pub struct PlacementAdmission<'extent> {
    identity: PlacementAdmissionId,
    placement_plan: ValidatedPlacementPlan,
    profile_receipt: ResourceProfileReceiptId,
    resources: PlacementResourceCompatibility,
    loan: ExtentLoan<'extent>,
}

impl PlacementAdmission<'_> {
    pub const fn identity(&self) -> PlacementAdmissionId {
        self.identity
    }

    pub const fn profile_receipt(&self) -> ResourceProfileReceiptId {
        self.profile_receipt
    }

    pub const fn resources(&self) -> &PlacementResourceCompatibility {
        &self.resources
    }
}

#[derive(Debug)]
pub struct PlacementRejection<'extent> {
    loan: ExtentLoan<'extent>,
    diagnostic: AccessPlanDiagnostic,
}

impl<'extent> PlacementRejection<'extent> {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ExtentLoan<'extent>, AccessPlanDiagnostic) {
        (self.loan, self.diagnostic)
    }
}

/// A plan-qualified interpretation of one borrowed concrete range.
#[derive(Debug)]
pub struct PlacedView<'extent> {
    loan: ExtentLoan<'extent>,
    plan: ValidatedPlacementPlan,
    profile_receipt: ResourceProfileReceiptId,
    resources: PlacementResourceCompatibility,
    admission: PlacementAdmissionId,
}

impl<'extent> PlacedView<'extent> {
    pub const fn admission(&self) -> PlacementAdmissionId {
        self.admission
    }

    pub const fn base(&self) -> u64 {
        self.loan.base()
    }

    pub const fn length(&self) -> u64 {
        self.loan.length()
    }

    /// Purely project one accepted field through a shared view borrow.
    ///
    /// Projection performs no memory event. The returned accessor remains
    /// tied to this placed view and exposes only named operation methods that
    /// create sealed lowering requests.
    pub fn project<'view>(
        &'view self,
        key: AccessFieldKey,
    ) -> Result<PlacedFieldProjection<'view, 'extent>, AccessPlanDiagnostic> {
        self.project_with(key, BorrowPolarity::Shared)
    }

    /// Purely project one accepted field through an exclusive view borrow.
    pub fn project_mut<'view>(
        &'view mut self,
        key: AccessFieldKey,
    ) -> Result<PlacedFieldProjection<'view, 'extent>, AccessPlanDiagnostic> {
        self.project_with(key, BorrowPolarity::Exclusive)
    }

    fn project_with<'view>(
        &'view self,
        key: AccessFieldKey,
        current_borrow: BorrowPolarity,
    ) -> Result<PlacedFieldProjection<'view, 'extent>, AccessPlanDiagnostic> {
        let source_loan = match self.loan.polarity() {
            LoanPolarity::Shared => BorrowPolarity::Shared,
            LoanPolarity::Exclusive => BorrowPolarity::Exclusive,
        };
        let descriptor = self
            .plan
            .access
            .field_descriptor(key)
            .cloned()
            .ok_or_else(|| {
                AccessPlanDiagnostic(format!(
                    "field key in canonical slot {} does not expose a placed accessor",
                    key.slot()
                ))
            })?;
        let supply = self.resources.field(key).ok_or_else(|| {
            AccessPlanDiagnostic(format!(
                "field `{}` has no sealed resource compatibility",
                descriptor.field()
            ))
        })?;
        let primitive_address = self
            .loan
            .base()
            .checked_add(descriptor.container_byte_offset())
            .ok_or_else(|| {
                AccessPlanDiagnostic(format!(
                    "field `{}` primitive address overflows address width",
                    descriptor.field()
                ))
            })?;
        Ok(PlacedFieldProjection {
            descriptor,
            current_borrow,
            source_loan,
            primitive_address,
            plan: self.plan.identity(),
            profile_receipt: self.profile_receipt,
            supply: supply.clone(),
            reach: self.plan.reach.clone(),
            admission: self.admission,
            _loan: &self.loan,
        })
    }
}

/// Pure field projection from one placed view.
///
/// This carrier is deliberately not `Clone`: re-projecting is the explicit
/// route to another accessor. Named operation methods are the only route from
/// this pure projection to a sealed memory event.
#[derive(Debug)]
pub struct PlacedFieldProjection<'view, 'extent> {
    descriptor: FieldAccessDescriptor,
    current_borrow: BorrowPolarity,
    source_loan: BorrowPolarity,
    primitive_address: u64,
    plan: PlacementPlanId,
    profile_receipt: ResourceProfileReceiptId,
    supply: EffectiveFieldSupply,
    reach: BoundaryReach,
    admission: PlacementAdmissionId,
    _loan: &'view ExtentLoan<'extent>,
}

impl<'view, 'extent> PlacedFieldProjection<'view, 'extent> {
    pub fn field(&self) -> &str {
        self.descriptor.field()
    }

    pub const fn key(&self) -> AccessFieldKey {
        self.descriptor.key()
    }

    pub const fn primitive_address(&self) -> u64 {
        self.primitive_address
    }

    pub const fn observation(&self) -> ObservationModel {
        self.descriptor.observation()
    }

    pub fn read<'access>(
        &'access self,
    ) -> Result<PlacedFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Read)
    }

    pub fn take<'access>(
        &'access mut self,
    ) -> Result<PlacedFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Take)
    }

    pub fn write<'access>(
        &'access mut self,
    ) -> Result<PlacedFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Write)
    }

    pub fn compound_mutation<'access>(
        &'access mut self,
    ) -> Result<PlacedFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::CompoundMutation)
    }

    pub fn atomic_load<'access>(
        &'access self,
        ordering: MemoryOrdering,
    ) -> Result<PlacedFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Atomic(AtomicAccessOperation::Load(
            ordering,
        )))
    }

    pub fn atomic_store<'access>(
        &'access self,
        ordering: MemoryOrdering,
    ) -> Result<PlacedFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Atomic(AtomicAccessOperation::Store(
            ordering,
        )))
    }

    pub fn atomic_fetch_add<'access>(
        &'access self,
        ordering: MemoryOrdering,
    ) -> Result<PlacedFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Atomic(AtomicAccessOperation::FetchAdd(
            ordering,
        )))
    }

    pub fn atomic_fetch_sub<'access>(
        &'access self,
        ordering: MemoryOrdering,
    ) -> Result<PlacedFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Atomic(AtomicAccessOperation::FetchSub(
            ordering,
        )))
    }

    pub fn atomic_fetch_xor<'access>(
        &'access self,
        ordering: MemoryOrdering,
    ) -> Result<PlacedFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Atomic(AtomicAccessOperation::FetchXor(
            ordering,
        )))
    }

    pub fn atomic_fetch_or<'access>(
        &'access self,
        ordering: MemoryOrdering,
    ) -> Result<PlacedFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Atomic(AtomicAccessOperation::FetchOr(
            ordering,
        )))
    }

    pub fn atomic_fetch_and<'access>(
        &'access self,
        ordering: MemoryOrdering,
    ) -> Result<PlacedFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Atomic(AtomicAccessOperation::FetchAnd(
            ordering,
        )))
    }

    pub fn atomic_swap<'access>(
        &'access self,
        ordering: MemoryOrdering,
    ) -> Result<PlacedFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Atomic(AtomicAccessOperation::Swap(
            ordering,
        )))
    }

    pub fn atomic_compare_exchange<'access>(
        &'access self,
        success: MemoryOrdering,
        failure: MemoryOrdering,
    ) -> Result<PlacedFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.authorize(AccessOperation::Atomic(
            AtomicAccessOperation::CompareExchange { success, failure },
        ))
    }

    fn authorize<'access>(
        &'access self,
        operation: AccessOperation,
    ) -> Result<PlacedFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        authorize_descriptor(
            &self.descriptor,
            self.current_borrow,
            self.source_loan,
            operation,
        )?;
        Ok(PlacedFieldAccess {
            access: AuthorizedFieldAccess {
                descriptor: self.descriptor.clone(),
                current_borrow: self.current_borrow,
                source_loan: self.source_loan,
                operation,
            },
            primitive_address: self.primitive_address,
            plan: self.plan,
            profile_receipt: self.profile_receipt,
            supply: self.supply.clone(),
            reach: self.reach.clone(),
            admission: self.admission,
            _loan: self._loan,
        })
    }
}

/// Sealed lowering input carrying both authorized field geometry and the
/// actual extent borrow from which its polarity was derived.
#[derive(Debug)]
pub struct PlacedFieldAccess<'view, 'extent> {
    access: AuthorizedFieldAccess,
    primitive_address: u64,
    plan: PlacementPlanId,
    profile_receipt: ResourceProfileReceiptId,
    supply: EffectiveFieldSupply,
    reach: BoundaryReach,
    admission: PlacementAdmissionId,
    _loan: &'view ExtentLoan<'extent>,
}

impl<'view, 'extent> PlacedFieldAccess<'view, 'extent> {
    pub const fn access(&self) -> &AuthorizedFieldAccess {
        &self.access
    }

    pub const fn primitive_address(&self) -> u64 {
        self.primitive_address
    }

    /// Consume one authorized access event into the only request primitive
    /// lowering accepts.
    ///
    /// The request remains bound to the normalized plan, exact admission and
    /// source loan, address, width, observation model, operation ordering, and
    /// static service reach that produced it. It contains no author-supplied
    /// offset.
    pub fn into_primitive_request(self) -> PrimitiveAccessRequest<'view, 'extent> {
        let AuthorizedFieldAccess {
            descriptor,
            current_borrow,
            source_loan,
            operation,
        } = self.access;
        PrimitiveAccessRequest {
            plan: self.plan,
            profile_receipt: self.profile_receipt,
            effective_supply: self.supply,
            admission: self.admission,
            primitive_address: self.primitive_address,
            field: descriptor.field,
            transfer_width_bits: descriptor.transfer_width_bits,
            observation: descriptor.observation,
            current_borrow,
            source_loan,
            operation,
            reach: self.reach,
            _loan: self._loan,
        }
    }
}

/// Canonical input to target-specific placed-memory lowering.
///
/// Construction is sealed behind `PlacedFieldAccess::into_primitive_request`.
/// Target lowering may choose a stronger instruction where architecture law
/// requires it, but may not weaken the exact event recorded here.
#[derive(Debug)]
pub struct PrimitiveAccessRequest<'view, 'extent> {
    plan: PlacementPlanId,
    profile_receipt: ResourceProfileReceiptId,
    effective_supply: EffectiveFieldSupply,
    admission: PlacementAdmissionId,
    primitive_address: u64,
    field: String,
    transfer_width_bits: u16,
    observation: ObservationModel,
    current_borrow: BorrowPolarity,
    source_loan: BorrowPolarity,
    operation: AccessOperation,
    reach: BoundaryReach,
    _loan: &'view ExtentLoan<'extent>,
}

impl PrimitiveAccessRequest<'_, '_> {
    pub const fn plan(&self) -> PlacementPlanId {
        self.plan
    }

    pub const fn admission(&self) -> PlacementAdmissionId {
        self.admission
    }

    pub const fn profile_receipt(&self) -> ResourceProfileReceiptId {
        self.profile_receipt
    }

    pub const fn effective_supply(&self) -> &EffectiveFieldSupply {
        &self.effective_supply
    }

    pub const fn primitive_address(&self) -> u64 {
        self.primitive_address
    }

    pub fn field(&self) -> &str {
        &self.field
    }

    pub const fn transfer_width_bits(&self) -> u16 {
        self.transfer_width_bits
    }

    pub const fn observation(&self) -> ObservationModel {
        self.observation
    }

    pub const fn current_borrow(&self) -> BorrowPolarity {
        self.current_borrow
    }

    pub const fn source_loan(&self) -> BorrowPolarity {
        self.source_loan
    }

    pub const fn operation(&self) -> AccessOperation {
        self.operation
    }

    pub const fn reach(&self) -> &BoundaryReach {
        &self.reach
    }
}

pub fn admit_placement<'extent>(
    identity: PlacementAdmissionId,
    loan: ExtentLoan<'extent>,
    plan: &ValidatedPlacementPlan,
    profile: &AdmittedResourceProfile,
) -> Result<PlacementAdmission<'extent>, PlacementRejection<'extent>> {
    let validation = validate_placement_admission(&loan, plan, profile);
    match validation {
        Ok(resources) => Ok(PlacementAdmission {
            identity,
            placement_plan: plan.clone(),
            profile_receipt: profile.receipt,
            resources,
            loan,
        }),
        Err(diagnostic) => Err(PlacementRejection { loan, diagnostic }),
    }
}

fn validate_placement_admission(
    loan: &ExtentLoan<'_>,
    plan: &ValidatedPlacementPlan,
    profile: &AdmittedResourceProfile,
) -> Result<PlacementResourceCompatibility, AccessPlanDiagnostic> {
    let restricted = profile.restrict_to_loan(loan)?;
    let compatibility = validate_placement_resources(plan, &restricted)?;
    if !compatibility.base.admits(loan.base()) {
        return Err(AccessPlanDiagnostic(format!(
            "extent loan base {} does not satisfy placement base congruence: base mod {} must equal {}",
            loan.base(),
            compatibility.base.modulus,
            compatibility.base.residue
        )));
    }
    Ok(compatibility)
}

pub fn place<'extent>(admission: PlacementAdmission<'extent>) -> PlacedView<'extent> {
    PlacedView {
        loan: admission.loan,
        plan: admission.placement_plan,
        profile_receipt: admission.profile_receipt,
        resources: admission.resources,
        admission: admission.identity,
    }
}

#[derive(Debug, Clone, Copy)]
struct ValidatedEntryPolicy {
    transfer_width_bits: u16,
    observation: ObservationModel,
    permissions: AccessPermissions,
    exposure: AccessExposure,
}

fn validate_entry_policy(
    entry: &AccessFieldEntry,
) -> Result<Option<ValidatedEntryPolicy>, AccessPlanDiagnostic> {
    let policy = match entry.access {
        FieldAccess::Inaccessible => return Ok(None),
        FieldAccess::Stable {
            transfer_width_bits,
            read,
            write,
            exposure,
        } => {
            if !read && !write {
                return Err(AccessPlanDiagnostic(format!(
                    "stable field `{}` exposes no operation; use Inaccessible",
                    entry.field
                )));
            }
            ValidatedEntryPolicy {
                transfer_width_bits,
                observation: ObservationModel::Stable,
                permissions: AccessPermissions {
                    read,
                    write,
                    ..AccessPermissions::default()
                },
                exposure,
            }
        }
        FieldAccess::External {
            transfer_width_bits,
            read,
            write,
            exposure,
        } => {
            if read == ExternalRead::None && !write {
                return Err(AccessPlanDiagnostic(format!(
                    "external field `{}` exposes no operation; use Inaccessible",
                    entry.field
                )));
            }
            ValidatedEntryPolicy {
                transfer_width_bits,
                observation: ObservationModel::External,
                permissions: AccessPermissions {
                    read: read == ExternalRead::Read,
                    take: read == ExternalRead::Take,
                    write,
                    ..AccessPermissions::default()
                },
                exposure,
            }
        }
        FieldAccess::Atomic {
            transfer_width_bits,
            operations,
            exposure,
        } => {
            if !operations.any() {
                return Err(AccessPlanDiagnostic(format!(
                    "atomic field `{}` exposes no operation; use Inaccessible",
                    entry.field
                )));
            }
            ValidatedEntryPolicy {
                transfer_width_bits,
                observation: ObservationModel::Atomic,
                permissions: AccessPermissions {
                    atomic: operations,
                    ..AccessPermissions::default()
                },
                exposure,
            }
        }
    };
    if policy.transfer_width_bits == 0
        || policy.transfer_width_bits > 128
        || !policy.transfer_width_bits.is_multiple_of(8)
    {
        return Err(AccessPlanDiagnostic(format!(
            "field `{}` transfer width {} is not a supported whole-byte width in 8..=128",
            entry.field, policy.transfer_width_bits
        )));
    }
    Ok(Some(policy))
}

fn validate_entry_geometry(
    field: &str,
    transfer_width_bits: u16,
    layout: &LayoutPlanReport,
    layout_size: u64,
) -> Result<u64, AccessPlanDiagnostic> {
    let placements = layout
        .entries
        .iter()
        .filter(|entry| entry.field == field)
        .map(|entry| entry.placement)
        .collect::<Vec<_>>();
    if placements.is_empty() {
        return Err(AccessPlanDiagnostic(format!(
            "access field `{field}` does not exist in the layout plan"
        )));
    }

    let transfer_bytes = u64::from(transfer_width_bits / 8);
    match placements.as_slice() {
        [LayoutPlacementReport::At { offset }] => {
            let offset = *offset;
            validate_transfer_range(field, offset, transfer_bytes, layout_size)?;
            Ok(offset)
        }
        placements => {
            let mut container = None;
            for placement in placements {
                let LayoutPlacementReport::Bits {
                    container: candidate,
                    container_width,
                    ..
                } = placement
                else {
                    return Err(AccessPlanDiagnostic(format!(
                        "access field `{field}` mixes whole and fragmented placement"
                    )));
                };
                if *container_width != u64::from(transfer_width_bits) {
                    return Err(AccessPlanDiagnostic(format!(
                        "access field `{}` requests a {}-bit transfer over a {container_width}-bit container",
                        field, transfer_width_bits
                    )));
                }
                if container
                    .replace(*candidate)
                    .is_some_and(|prior| prior != *candidate)
                {
                    return Err(AccessPlanDiagnostic(format!(
                        "fragmented field `{}` spans multiple containers and cannot be projected through one exact access",
                        field
                    )));
                }
            }
            let container = container.expect("nonempty placements");
            validate_transfer_range(field, container, transfer_bytes, layout_size)?;
            Ok(container)
        }
    }
}

fn validate_transfer_range(
    field: &str,
    offset: u64,
    transfer_bytes: u64,
    layout_size: u64,
) -> Result<(), AccessPlanDiagnostic> {
    let end = offset.checked_add(transfer_bytes).ok_or_else(|| {
        AccessPlanDiagnostic(format!(
            "access field `{field}` transfer byte range overflows"
        ))
    })?;
    if end > layout_size {
        return Err(AccessPlanDiagnostic(format!(
            "access field `{}` transfer at {offset}..{end} exceeds {layout_size}-byte layout",
            field
        )));
    }
    Ok(())
}

fn authorize_descriptor(
    descriptor: &FieldAccessDescriptor,
    current_borrow: BorrowPolarity,
    source_loan: BorrowPolarity,
    operation: AccessOperation,
) -> Result<(), AccessPlanDiagnostic> {
    validate_operation_ordering(operation)?;
    let permitted = match operation {
        AccessOperation::Read => descriptor.permissions.read,
        AccessOperation::Take => {
            descriptor.permissions.take
                && current_borrow == BorrowPolarity::Exclusive
                && source_loan == BorrowPolarity::Exclusive
        }
        AccessOperation::Write => {
            descriptor.permissions.write
                && current_borrow == BorrowPolarity::Exclusive
                && source_loan == BorrowPolarity::Exclusive
        }
        AccessOperation::CompoundMutation => {
            descriptor.observation == ObservationModel::Stable
                && descriptor.permissions.read
                && descriptor.permissions.write
                && current_borrow == BorrowPolarity::Exclusive
                && source_loan == BorrowPolarity::Exclusive
        }
        AccessOperation::Atomic(AtomicAccessOperation::Load(_)) => {
            descriptor.permissions.atomic.load
        }
        AccessOperation::Atomic(AtomicAccessOperation::Store(_)) => {
            descriptor.permissions.atomic.store
        }
        AccessOperation::Atomic(AtomicAccessOperation::FetchAdd(_)) => {
            descriptor.permissions.atomic.fetch_add
        }
        AccessOperation::Atomic(AtomicAccessOperation::FetchSub(_)) => {
            descriptor.permissions.atomic.fetch_sub
        }
        AccessOperation::Atomic(AtomicAccessOperation::FetchXor(_)) => {
            descriptor.permissions.atomic.fetch_xor
        }
        AccessOperation::Atomic(AtomicAccessOperation::FetchOr(_)) => {
            descriptor.permissions.atomic.fetch_or
        }
        AccessOperation::Atomic(AtomicAccessOperation::FetchAnd(_)) => {
            descriptor.permissions.atomic.fetch_and
        }
        AccessOperation::Atomic(AtomicAccessOperation::Swap(_)) => {
            descriptor.permissions.atomic.swap
        }
        AccessOperation::Atomic(AtomicAccessOperation::CompareExchange { .. }) => {
            descriptor.permissions.atomic.compare_exchange
        }
    };
    if permitted {
        Ok(())
    } else {
        Err(AccessPlanDiagnostic(format!(
            "field `{}` does not permit {operation:?} through a {current_borrow:?} current borrow over a {source_loan:?} source loan",
            descriptor.field,
        )))
    }
}

fn validate_operation_ordering(operation: AccessOperation) -> Result<(), AccessPlanDiagnostic> {
    let AccessOperation::Atomic(operation) = operation else {
        return Ok(());
    };
    let ordering = operation.ordering_plan();
    let legal = match ordering {
        AtomicOrderingPlan::Load(ordering) => ordering.valid_for_load(),
        AtomicOrderingPlan::Store(ordering) => ordering.valid_for_store(),
        AtomicOrderingPlan::ReadModifyWrite(_) | AtomicOrderingPlan::Swap(_) => true,
        AtomicOrderingPlan::CompareExchange { success, failure } => {
            failure.valid_compare_exchange_failure(success)
        }
    };
    if legal {
        Ok(())
    } else {
        Err(AccessPlanDiagnostic(format!(
            "atomic access carries an invalid ordering plan: {ordering:?}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_layout_plans::{LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport};

    fn reach() -> BoundaryServiceReachId {
        BoundaryServiceReachId::from_normalized_identity(7).expect("normalized reach")
    }

    fn uart_reach() -> BoundaryReach {
        BoundaryReach::from_services([reach()])
    }

    fn uart_layout() -> LayoutPlanReport {
        LayoutPlanReport {
            schema_identity: 1,
            entries: vec![
                LayoutFieldEntryReport {
                    field: "status".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At { offset: 0 },
                },
                LayoutFieldEntryReport {
                    field: "transmit".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At { offset: 4 },
                },
                LayoutFieldEntryReport {
                    field: "control".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::Bits {
                        container: 8,
                        container_width: 32,
                        destination_lsb: 0,
                        source_lsb: 0,
                        width: 8,
                    },
                },
            ],
            offsets: None,
            size: Some(12),
            align: 4,
        }
    }

    fn access_plan(layout: &LayoutPlanReport, decisions: &[(&str, FieldAccess)]) -> AccessPlan {
        let mut plan = AccessPlan::inaccessible(layout).expect("inaccessible seed");
        for (field, access) in decisions {
            let key = plan
                .entries()
                .iter()
                .find(|entry| entry.field() == *field)
                .map(AccessFieldEntry::key)
                .expect("schema field key");
            plan.set(key, access.clone())
                .expect("replace field decision");
        }
        plan
    }

    fn field_key(plan: &ValidatedAccessPlan, field: &str) -> AccessFieldKey {
        plan.plan()
            .entries()
            .iter()
            .find(|entry| entry.field() == field)
            .map(AccessFieldEntry::key)
            .expect("validated schema field key")
    }

    fn uart_access_source(layout: &LayoutPlanReport) -> AccessPlan {
        access_plan(
            layout,
            &[
                (
                    "status",
                    FieldAccess::External {
                        transfer_width_bits: 32,
                        read: ExternalRead::Read,
                        write: false,
                        exposure: AccessExposure::Exported,
                    },
                ),
                (
                    "transmit",
                    FieldAccess::External {
                        transfer_width_bits: 32,
                        read: ExternalRead::None,
                        write: true,
                        exposure: AccessExposure::Exported,
                    },
                ),
                (
                    "control",
                    FieldAccess::External {
                        transfer_width_bits: 32,
                        read: ExternalRead::Read,
                        write: true,
                        exposure: AccessExposure::BindingPrivate,
                    },
                ),
            ],
        )
    }

    fn uart_access_plan() -> ValidatedAccessPlan {
        let layout = uart_layout();
        let plan = uart_access_source(&layout);
        validate_access_plan(plan, &layout).expect("UART plan")
    }

    fn uart_placement_plan() -> ValidatedPlacementPlan {
        let layout = uart_layout();
        validate_placement_plan(PlacementPlan {
            access: uart_access_source(&layout),
            layout,
            reach: uart_reach(),
        })
        .expect("UART placement plan")
    }

    #[test]
    fn inaccessible_seed_has_exact_canonical_schema_cardinality() {
        let layout = uart_layout();
        let plan = AccessPlan::inaccessible(&layout).expect("inaccessible plan");
        assert_eq!(plan.entries().len(), 3);
        assert_eq!(
            plan.entries()
                .iter()
                .map(AccessFieldEntry::field)
                .collect::<Vec<_>>(),
            vec!["control", "status", "transmit"]
        );
        assert!(
            plan.entries()
                .iter()
                .all(|entry| entry.access() == &FieldAccess::Inaccessible)
        );
        let validated = validate_access_plan(plan, &layout).expect("all-inaccessible plan");
        assert!(validated.field_descriptors().is_empty());
        assert!(
            validated
                .authorize(
                    field_key(&validated, "status"),
                    BorrowPolarity::Shared,
                    BorrowPolarity::Shared,
                    AccessOperation::Read,
                )
                .is_err()
        );
    }

    #[test]
    fn numbered_field_rename_does_not_change_access_identity() {
        let mut original_layout = LayoutPlanReport {
            schema_identity: 0x44,
            entries: vec![LayoutFieldEntryReport {
                field: "word".into(),
                member_identity: Some(7),
                placement: LayoutPlacementReport::At { offset: 0 },
            }],
            offsets: Some(vec![0]),
            size: Some(4),
            align: 4,
        };
        let original = validate_access_plan(
            access_plan(
                &original_layout,
                &[(
                    "word",
                    FieldAccess::Stable {
                        transfer_width_bits: 32,
                        read: true,
                        write: false,
                        exposure: AccessExposure::Exported,
                    },
                )],
            ),
            &original_layout,
        )
        .expect("original numbered plan");

        original_layout.entries[0].field = "renamed_word".into();
        let renamed = validate_access_plan(
            access_plan(
                &original_layout,
                &[(
                    "renamed_word",
                    FieldAccess::Stable {
                        transfer_width_bits: 32,
                        read: true,
                        write: false,
                        exposure: AccessExposure::Exported,
                    },
                )],
            ),
            &original_layout,
        )
        .expect("renamed numbered plan");
        assert_eq!(original.identity(), renamed.identity());
    }

    #[test]
    fn access_identity_covers_operation_width_and_exposure() {
        let layout = LayoutPlanReport {
            schema_identity: 1,
            entries: vec![LayoutFieldEntryReport {
                field: "word".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 0 },
            }],
            offsets: Some(vec![0]),
            size: Some(8),
            align: 8,
        };
        let validate = |access: FieldAccess| {
            validate_access_plan(access_plan(&layout, &[("word", access)]), &layout)
                .expect("identity test plan")
                .identity()
        };
        let stable_read = FieldAccess::Stable {
            transfer_width_bits: 32,
            read: true,
            write: false,
            exposure: AccessExposure::Exported,
        };
        let mut stable_write = stable_read.clone();
        let FieldAccess::Stable { read, write, .. } = &mut stable_write else {
            unreachable!()
        };
        *read = false;
        *write = true;
        let mut wider = stable_read.clone();
        let FieldAccess::Stable {
            transfer_width_bits,
            ..
        } = &mut wider
        else {
            unreachable!()
        };
        *transfer_width_bits = 64;
        let mut private = stable_read.clone();
        let FieldAccess::Stable { exposure, .. } = &mut private else {
            unreachable!()
        };
        *exposure = AccessExposure::BindingPrivate;
        let external = FieldAccess::External {
            transfer_width_bits: 32,
            read: ExternalRead::Read,
            write: false,
            exposure: AccessExposure::Exported,
        };

        let identities = [
            validate(stable_read),
            validate(stable_write),
            validate(wider),
            validate(private),
            validate(external),
        ];
        for (index, identity) in identities.iter().enumerate() {
            assert!(
                identities[index + 1..]
                    .iter()
                    .all(|other| other != identity),
                "every semantic policy change must alter normalized identity"
            );
        }
    }

    #[test]
    fn placement_identity_owns_normalized_reach() {
        let layout = uart_layout();
        let access = uart_access_source(&layout);
        let uart = validate_placement_plan(PlacementPlan {
            layout: layout.clone(),
            access: access.clone(),
            reach: BoundaryReach::from_services([reach(), reach()]),
        })
        .expect("UART placement");
        let alternate_reach =
            BoundaryServiceReachId::from_normalized_identity(8).expect("alternate reach");
        let alternate = validate_placement_plan(PlacementPlan {
            layout,
            access,
            reach: BoundaryReach::from_services([alternate_reach]),
        })
        .expect("alternate placement reach");
        assert_eq!(
            uart.reach().services().len(),
            1,
            "reach is a normalized set"
        );
        assert_eq!(uart.access().identity(), alternate.access().identity());
        assert_ne!(uart.identity(), alternate.identity());
    }

    #[test]
    fn uart_access_plan_validates_geometry_and_borrow_polarity() {
        let plan = uart_access_plan();

        let status = plan
            .authorize(
                field_key(&plan, "status"),
                BorrowPolarity::Shared,
                BorrowPolarity::Shared,
                AccessOperation::Read,
            )
            .expect("shared snapshot read");
        assert_eq!(status.descriptor().field(), "status");
        assert_eq!(status.descriptor().container_byte_offset(), 0);
        assert_eq!(status.descriptor().transfer_width_bits(), 32);
        assert_eq!(
            status.descriptor().observation(),
            ObservationModel::External
        );
        assert_eq!(status.current_borrow(), BorrowPolarity::Shared);
        assert_eq!(status.source_loan(), BorrowPolarity::Shared);
        assert_eq!(status.operation(), AccessOperation::Read);
        assert_eq!(plan.field_descriptors().len(), 3);
        assert_eq!(
            plan.field_descriptor(field_key(&plan, "control"))
                .expect("control descriptor")
                .container_byte_offset(),
            8
        );
        assert!(
            plan.authorize(
                field_key(&plan, "transmit"),
                BorrowPolarity::Shared,
                BorrowPolarity::Exclusive,
                AccessOperation::Write,
            )
            .is_err()
        );
        plan.authorize(
            field_key(&plan, "transmit"),
            BorrowPolarity::Exclusive,
            BorrowPolarity::Exclusive,
            AccessOperation::Write,
        )
        .expect("exclusive whole write");
        assert!(
            plan.authorize(
                field_key(&plan, "control"),
                BorrowPolarity::Exclusive,
                BorrowPolarity::Exclusive,
                AccessOperation::CompoundMutation,
            )
            .is_err(),
            "external storage never derives compound mutation"
        );
    }

    #[test]
    fn stable_compound_mutation_is_derived_from_permissions_and_borrow() {
        let layout = uart_layout();
        let plan = validate_access_plan(
            access_plan(
                &layout,
                &[(
                    "status",
                    FieldAccess::Stable {
                        transfer_width_bits: 32,
                        read: true,
                        write: true,
                        exposure: AccessExposure::Exported,
                    },
                )],
            ),
            &layout,
        )
        .expect("stable read-write plan");
        plan.authorize(
            field_key(&plan, "status"),
            BorrowPolarity::Exclusive,
            BorrowPolarity::Exclusive,
            AccessOperation::CompoundMutation,
        )
        .expect("exclusive stable read-write access derives compound mutation");
        assert!(
            plan.authorize(
                field_key(&plan, "status"),
                BorrowPolarity::Shared,
                BorrowPolarity::Exclusive,
                AccessOperation::CompoundMutation,
            )
            .is_err()
        );
        assert!(
            plan.authorize(
                field_key(&plan, "status"),
                BorrowPolarity::Exclusive,
                BorrowPolarity::Shared,
                AccessOperation::CompoundMutation,
            )
            .is_err(),
            "an exclusive current borrow cannot upgrade a shared source loan"
        );

        let plan = validate_access_plan(
            access_plan(
                &layout,
                &[(
                    "status",
                    FieldAccess::Stable {
                        transfer_width_bits: 32,
                        read: true,
                        write: false,
                        exposure: AccessExposure::Exported,
                    },
                )],
            ),
            &layout,
        )
        .expect("stable read-only plan");
        assert!(
            plan.authorize(
                field_key(&plan, "status"),
                BorrowPolarity::Exclusive,
                BorrowPolarity::Exclusive,
                AccessOperation::CompoundMutation,
            )
            .is_err()
        );
    }

    #[test]
    fn destructive_external_read_does_not_derive_readable() {
        let layout = uart_layout();
        let plan = validate_access_plan(
            access_plan(
                &layout,
                &[(
                    "status",
                    FieldAccess::External {
                        transfer_width_bits: 32,
                        read: ExternalRead::Take,
                        write: false,
                        exposure: AccessExposure::Exported,
                    },
                )],
            ),
            &layout,
        )
        .expect("destructive external plan");
        assert!(
            plan.authorize(
                field_key(&plan, "status"),
                BorrowPolarity::Shared,
                BorrowPolarity::Exclusive,
                AccessOperation::Read,
            )
            .is_err()
        );
        assert!(
            plan.authorize(
                field_key(&plan, "status"),
                BorrowPolarity::Shared,
                BorrowPolarity::Exclusive,
                AccessOperation::Take,
            )
            .is_err()
        );
        plan.authorize(
            field_key(&plan, "status"),
            BorrowPolarity::Exclusive,
            BorrowPolarity::Exclusive,
            AccessOperation::Take,
        )
        .expect("destructive read requires exclusive access");
    }

    #[test]
    fn external_compound_mutation_rejects() {
        let layout = uart_layout();
        let plan = validate_access_plan(
            access_plan(
                &layout,
                &[(
                    "status",
                    FieldAccess::External {
                        transfer_width_bits: 32,
                        read: ExternalRead::Read,
                        write: true,
                        exposure: AccessExposure::Exported,
                    },
                )],
            ),
            &layout,
        )
        .expect("external read-write access is valid");
        let error = plan
            .authorize(
                field_key(&plan, "status"),
                BorrowPolarity::Exclusive,
                BorrowPolarity::Exclusive,
                AccessOperation::CompoundMutation,
            )
            .expect_err("external access must never derive compound mutation");
        assert!(error.0.contains("does not permit"));
    }

    #[test]
    fn empty_access_cases_reject_in_favor_of_inaccessible() {
        let layout = uart_layout();
        for access in [
            FieldAccess::Stable {
                transfer_width_bits: 32,
                read: false,
                write: false,
                exposure: AccessExposure::Exported,
            },
            FieldAccess::External {
                transfer_width_bits: 32,
                read: ExternalRead::None,
                write: false,
                exposure: AccessExposure::Exported,
            },
            FieldAccess::Atomic {
                transfer_width_bits: 32,
                operations: AtomicPermissions::default(),
                exposure: AccessExposure::Exported,
            },
        ] {
            let error = validate_access_plan(access_plan(&layout, &[("status", access)]), &layout)
                .expect_err("empty access case must reject");
            assert!(error.0.contains("Inaccessible"));
        }
    }

    #[test]
    fn atomic_shared_page_exposes_only_atomic_mutation() {
        let layout = LayoutPlanReport {
            schema_identity: 1,
            entries: vec![LayoutFieldEntryReport {
                field: "head".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 0 },
            }],
            offsets: Some(vec![0]),
            size: Some(4),
            align: 4,
        };
        let plan = validate_access_plan(
            access_plan(
                &layout,
                &[(
                    "head",
                    FieldAccess::Atomic {
                        transfer_width_bits: 32,
                        operations: AtomicPermissions {
                            load: true,
                            store: true,
                            fetch_add: true,
                            compare_exchange: true,
                            ..AtomicPermissions::default()
                        },
                        exposure: AccessExposure::Exported,
                    },
                )],
            ),
            &layout,
        )
        .expect("atomic IPC plan");

        let mut alternate_source = plan.plan().clone();
        let FieldAccess::Atomic {
            operations: alternate_permissions,
            ..
        } = &mut alternate_source.entries[0].access
        else {
            panic!("atomic field decision")
        };
        alternate_permissions.fetch_add = false;
        alternate_permissions.fetch_sub = true;
        let alternate =
            validate_access_plan(alternate_source, &layout).expect("alternate atomic plan");
        assert_ne!(
            plan.identity(),
            alternate.identity(),
            "distinct atomic operation families must alter normalized identity"
        );

        let store = AccessOperation::Atomic(AtomicAccessOperation::Store(MemoryOrdering::Release));
        plan.authorize(
            field_key(&plan, "head"),
            BorrowPolarity::Shared,
            BorrowPolarity::Shared,
            store,
        )
        .expect("shared mutation is explicitly atomic");
        plan.authorize(
            field_key(&plan, "head"),
            BorrowPolarity::Shared,
            BorrowPolarity::Shared,
            AccessOperation::Atomic(AtomicAccessOperation::FetchAdd(MemoryOrdering::AcqRel)),
        )
        .expect("admitted fetch-add");
        assert!(
            plan.authorize(
                field_key(&plan, "head"),
                BorrowPolarity::Shared,
                BorrowPolarity::Shared,
                AccessOperation::Atomic(AtomicAccessOperation::FetchSub(MemoryOrdering::AcqRel)),
            )
            .is_err(),
            "one admitted fetch family does not imply another"
        );
        let invalid_load =
            AccessOperation::Atomic(AtomicAccessOperation::Load(MemoryOrdering::Release));
        let error = plan
            .authorize(
                field_key(&plan, "head"),
                BorrowPolarity::Shared,
                BorrowPolarity::Shared,
                invalid_load,
            )
            .expect_err("release cannot order an atomic load");
        assert!(error.0.contains("invalid ordering"));
        assert!(
            plan.authorize(
                field_key(&plan, "head"),
                BorrowPolarity::Exclusive,
                BorrowPolarity::Exclusive,
                AccessOperation::Write,
            )
            .is_err()
        );

        let placement = validate_placement_plan(PlacementPlan {
            layout: layout.clone(),
            access: plan.plan().clone(),
            reach: BoundaryReach::default(),
        })
        .expect("atomic placement plan");
        let extent = uart_extent(0x2000, 4);
        let loan = extent.loan(0, 4).expect("shared atomic loan");
        let required_rights = extent_rights(&[3]);
        let resources = ResourceProfileGrant::from_admitted_provider(
            ResourceProfileReceiptId::from_normalized_identity(11).expect("profile receipt"),
            0x2000,
            4,
            extent_id(2, AddressSpaceId::from_normalized_identity),
            extent_id(5, ExtentProvenanceId::from_normalized_identity),
            extent_id(6, omega_extents::MappingEraId::from_normalized_identity),
            required_rights,
            BoundaryReach::default(),
        )
        .expect("atomic profile grant")
        .admit(ResourceProfile {
            regions: vec![ResourceRegion {
                offset: 0,
                length: 4,
                stable: StableCapability::None,
                external: ExternalCapability::None,
                atomic: AtomicCapability::Access {
                    transfers: vec![AtomicTransferRule {
                        transfer: TransferRule {
                            width_bits: 32,
                            alignment_bytes: 4,
                        },
                        operations: AtomicPermissions {
                            load: true,
                            store: true,
                            fetch_add: true,
                            compare_exchange: true,
                            ..AtomicPermissions::default()
                        },
                    }],
                },
                reach: BoundaryReach::default(),
            }],
        })
        .expect("admitted atomic profile");
        let admission_id =
            PlacementAdmissionId::from_normalized_identity(10).expect("atomic admission");
        let admission = admit_placement(admission_id, loan, &placement, &resources)
            .expect("admitted atomic placement");
        let view = place(admission);
        let head = view
            .project(field_key(placement.access(), "head"))
            .expect("pure atomic projection");
        let request = head
            .atomic_compare_exchange(MemoryOrdering::AcqRel, MemoryOrdering::Acquire)
            .expect("authorized compare-exchange")
            .into_primitive_request();
        assert_eq!(request.plan(), placement.identity());
        assert_eq!(request.admission(), admission_id);
        assert_eq!(
            request.profile_receipt(),
            ResourceProfileReceiptId::from_normalized_identity(11).expect("profile receipt")
        );
        assert_eq!(
            request.effective_supply().kind(),
            EffectiveSupplyKind::Atomic
        );
        assert_eq!(request.effective_supply().alignment_bytes(), 4);
        assert_eq!(request.primitive_address(), 0x2000);
        assert_eq!(request.field(), "head");
        assert_eq!(request.transfer_width_bits(), 32);
        assert_eq!(request.observation(), ObservationModel::Atomic);
        assert_eq!(request.current_borrow(), BorrowPolarity::Shared);
        assert_eq!(request.source_loan(), BorrowPolarity::Shared);
        assert_eq!(
            request.operation(),
            AccessOperation::Atomic(AtomicAccessOperation::CompareExchange {
                success: MemoryOrdering::AcqRel,
                failure: MemoryOrdering::Acquire,
            })
        );
        assert_eq!(request.reach(), &BoundaryReach::default());
    }

    #[test]
    fn multi_container_fragments_are_not_one_access() {
        let layout = LayoutPlanReport {
            schema_identity: 1,
            entries: vec![
                LayoutFieldEntryReport {
                    field: "entry".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::Bits {
                        container: 0,
                        container_width: 32,
                        destination_lsb: 0,
                        source_lsb: 0,
                        width: 16,
                    },
                },
                LayoutFieldEntryReport {
                    field: "entry".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::Bits {
                        container: 4,
                        container_width: 32,
                        destination_lsb: 0,
                        source_lsb: 16,
                        width: 16,
                    },
                },
            ],
            offsets: None,
            size: Some(8),
            align: 4,
        };
        let error = validate_access_plan(
            access_plan(
                &layout,
                &[(
                    "entry",
                    FieldAccess::Stable {
                        transfer_width_bits: 32,
                        read: true,
                        write: false,
                        exposure: AccessExposure::Exported,
                    },
                )],
            ),
            &layout,
        )
        .expect_err("one token cannot hide two primitive accesses");
        assert!(error.0.contains("multiple containers"));
    }

    #[test]
    fn field_keys_reject_cross_layout_and_out_of_cardinality_use() {
        let layout = uart_layout();
        let mut plan = AccessPlan::inaccessible(&layout).expect("UART seed");
        let mut alternate_layout = layout.clone();
        alternate_layout.schema_identity = 2;
        let alternate = AccessPlan::inaccessible(&alternate_layout).expect("alternate schema seed");
        let error = plan
            .set(
                alternate.key_at(0).expect("alternate key"),
                FieldAccess::Stable {
                    transfer_width_bits: 32,
                    read: true,
                    write: false,
                    exposure: AccessExposure::Exported,
                },
            )
            .expect_err("cross-layout key must reject");
        assert!(error.0.contains("different validated layout"));

        let error = plan
            .set(
                AccessFieldKey {
                    layout_fingerprint: plan.layout_fingerprint(),
                    slot: u32::MAX,
                },
                FieldAccess::Stable {
                    transfer_width_bits: 32,
                    read: true,
                    write: false,
                    exposure: AccessExposure::Exported,
                },
            )
            .expect_err("out-of-cardinality key must reject");
        assert!(error.0.contains("outside the schema cardinality"));
    }

    fn extent_id<T>(
        identity: u64,
        constructor: fn(u64) -> Result<T, omega_extents::ExtentDiagnostic>,
    ) -> T {
        constructor(identity).expect("normalized extent identity")
    }

    fn extent_rights(identities: &[u64]) -> ExtentRights {
        ExtentRights::from_normalized_identities(identities.iter().copied().map(|identity| {
            extent_id(
                identity,
                omega_extents::ExtentRightId::from_normalized_identity,
            )
        }))
    }

    fn uart_extent(base: u64, length: u64) -> omega_extents::Extent {
        omega_extents::ExtentRootGrant::from_admitted_provider(
            extent_id(1, omega_extents::ExtentLineageId::from_normalized_identity),
            extent_id(2, AddressSpaceId::from_normalized_identity),
            extent_rights(&[3, 4]),
            extent_id(5, ExtentProvenanceId::from_normalized_identity),
            extent_id(6, omega_extents::MappingEraId::from_normalized_identity),
        )
        .mint(base, length)
        .expect("UART extent")
    }

    fn uart_resource_profile(
        base: u64,
        length: u64,
        reach: &BoundaryReach,
    ) -> AdmittedResourceProfile {
        ResourceProfileGrant::from_admitted_provider(
            ResourceProfileReceiptId::from_normalized_identity(7).expect("profile receipt"),
            base,
            length,
            extent_id(2, AddressSpaceId::from_normalized_identity),
            extent_id(5, ExtentProvenanceId::from_normalized_identity),
            extent_id(6, omega_extents::MappingEraId::from_normalized_identity),
            extent_rights(&[3]),
            reach.clone(),
        )
        .expect("UART resource-profile grant")
        .admit(ResourceProfile {
            regions: vec![ResourceRegion {
                offset: 0,
                length,
                stable: StableCapability::None,
                external: ExternalCapability::Access {
                    read: ExternalReadBehavior::Repeatable,
                    write: true,
                    transfers: vec![TransferRule {
                        width_bits: 32,
                        alignment_bytes: 4,
                    }],
                },
                atomic: AtomicCapability::None,
                reach: reach.clone(),
            }],
        })
        .expect("admitted UART resource profile")
    }

    fn admit_uart<'extent>(
        identity: u64,
        loan: ExtentLoan<'extent>,
        plan: &ValidatedPlacementPlan,
        permitted_reach: &BoundaryReach,
    ) -> Result<PlacementAdmission<'extent>, PlacementRejection<'extent>> {
        let resources = uart_resource_profile(loan.base(), loan.length(), permitted_reach);
        admit_placement(
            PlacementAdmissionId::from_normalized_identity(identity).expect("placement admission"),
            loan,
            plan,
            &resources,
        )
    }

    #[test]
    fn placed_view_derives_access_from_extent_provenance_and_actual_borrow() {
        let plan = uart_placement_plan();

        let mut shared_extent = uart_extent(0x1000, 64);
        let shared_loan = shared_extent.loan(0, 12).expect("shared UART loan");
        let admission =
            admit_uart(8, shared_loan, &plan, &uart_reach()).expect("admitted shared view");
        let mut shared_view = place(admission);
        {
            let status = shared_view
                .project(field_key(plan.access(), "status"))
                .expect("pure status projection");
            assert_eq!(status.primitive_address(), 0x1000);
            assert_eq!(status.observation(), ObservationModel::External);
            let read = status.read().expect("shared read");
            assert_eq!(read.access().current_borrow(), BorrowPolarity::Shared);
            assert_eq!(read.access().source_loan(), BorrowPolarity::Shared);
            let request = read.into_primitive_request();
            assert_eq!(request.plan(), plan.identity());
            assert_eq!(
                request.admission(),
                PlacementAdmissionId::from_normalized_identity(8).expect("admission")
            );
            assert_eq!(
                request.profile_receipt(),
                ResourceProfileReceiptId::from_normalized_identity(7).expect("profile receipt")
            );
            assert_eq!(
                request.effective_supply().kind(),
                EffectiveSupplyKind::External
            );
            assert_eq!(request.effective_supply().alignment_bytes(), 4);
            assert_eq!(request.primitive_address(), 0x1000);
            assert_eq!(request.field(), "status");
            assert_eq!(request.transfer_width_bits(), 32);
            assert_eq!(request.observation(), ObservationModel::External);
            assert_eq!(request.current_borrow(), BorrowPolarity::Shared);
            assert_eq!(request.source_loan(), BorrowPolarity::Shared);
            assert_eq!(request.operation(), AccessOperation::Read);
            assert!(request.reach().contains(reach()));
        }
        {
            let mut transmit = shared_view
                .project(field_key(plan.access(), "transmit"))
                .expect("pure shared transmit projection");
            assert!(
                transmit.write().is_err(),
                "write accessor requires an exclusive current view borrow"
            );
        }
        {
            let mut transmit = shared_view
                .project_mut(field_key(plan.access(), "transmit"))
                .expect("pure exclusive transmit projection");
            assert!(
                transmit.write().is_err(),
                "exclusive reborrow cannot upgrade a shared source loan"
            );
        }

        let exclusive_loan = shared_extent.loan_mut(4, 12).expect("exclusive UART loan");
        let admission =
            admit_uart(9, exclusive_loan, &plan, &uart_reach()).expect("admitted exclusive view");
        let mut exclusive_view = place(admission);
        {
            let mut transmit = exclusive_view
                .project(field_key(plan.access(), "transmit"))
                .expect("pure shared transmit projection");
            assert!(
                transmit.write().is_err(),
                "ordinary write requires an exclusive current view borrow"
            );
        }
        {
            let mut transmit = exclusive_view
                .project_mut(field_key(plan.access(), "transmit"))
                .expect("pure exclusive transmit projection");
            let write = transmit.write().expect("exclusive write");
            assert_eq!(write.primitive_address(), 0x1008);
            assert_eq!(write.access().current_borrow(), BorrowPolarity::Exclusive);
            assert_eq!(write.access().source_loan(), BorrowPolarity::Exclusive);
        }
    }

    #[test]
    fn placed_projection_exposes_only_granular_authorized_events() {
        let layout = LayoutPlanReport {
            schema_identity: 95,
            entries: ["stable", "fifo", "counter", "hidden"]
                .into_iter()
                .enumerate()
                .map(|(index, field)| LayoutFieldEntryReport {
                    field: field.into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At {
                        offset: u64::try_from(index).expect("field index") * 4,
                    },
                })
                .collect(),
            offsets: Some(vec![0, 4, 8, 12]),
            size: Some(16),
            align: 4,
        };
        let placement = validate_placement_plan(PlacementPlan {
            access: access_plan(
                &layout,
                &[
                    (
                        "stable",
                        FieldAccess::Stable {
                            transfer_width_bits: 32,
                            read: true,
                            write: true,
                            exposure: AccessExposure::Exported,
                        },
                    ),
                    (
                        "fifo",
                        FieldAccess::External {
                            transfer_width_bits: 32,
                            read: ExternalRead::Take,
                            write: false,
                            exposure: AccessExposure::Exported,
                        },
                    ),
                    (
                        "counter",
                        FieldAccess::Atomic {
                            transfer_width_bits: 32,
                            operations: AtomicPermissions {
                                load: true,
                                fetch_add: true,
                                ..AtomicPermissions::default()
                            },
                            exposure: AccessExposure::Exported,
                        },
                    ),
                ],
            ),
            layout,
            reach: BoundaryReach::default(),
        })
        .expect("heterogeneous placement");
        let mut extent = uart_extent(0x5000, 16);
        let loan = extent.loan_mut(0, 16).expect("exclusive placed loan");
        let resources = ResourceProfileGrant::from_admitted_provider(
            ResourceProfileReceiptId::from_normalized_identity(51).expect("profile receipt"),
            0x5000,
            16,
            extent_id(2, AddressSpaceId::from_normalized_identity),
            extent_id(5, ExtentProvenanceId::from_normalized_identity),
            extent_id(6, omega_extents::MappingEraId::from_normalized_identity),
            extent_rights(&[3]),
            BoundaryReach::default(),
        )
        .expect("profile grant")
        .admit(ResourceProfile {
            regions: vec![
                ResourceRegion {
                    offset: 0,
                    length: 4,
                    stable: StableCapability::ReadWrite,
                    external: ExternalCapability::None,
                    atomic: AtomicCapability::None,
                    reach: BoundaryReach::default(),
                },
                ResourceRegion {
                    offset: 4,
                    length: 4,
                    stable: StableCapability::None,
                    external: ExternalCapability::Access {
                        read: ExternalReadBehavior::Destructive,
                        write: false,
                        transfers: vec![TransferRule {
                            width_bits: 32,
                            alignment_bytes: 4,
                        }],
                    },
                    atomic: AtomicCapability::None,
                    reach: BoundaryReach::default(),
                },
                ResourceRegion {
                    offset: 8,
                    length: 4,
                    stable: StableCapability::None,
                    external: ExternalCapability::None,
                    atomic: AtomicCapability::Access {
                        transfers: vec![AtomicTransferRule {
                            transfer: TransferRule {
                                width_bits: 32,
                                alignment_bytes: 4,
                            },
                            operations: AtomicPermissions {
                                load: true,
                                fetch_add: true,
                                ..AtomicPermissions::default()
                            },
                        }],
                    },
                    reach: BoundaryReach::default(),
                },
            ],
        })
        .expect("heterogeneous profile");
        let admission = admit_placement(
            PlacementAdmissionId::from_normalized_identity(52).expect("admission"),
            loan,
            &placement,
            &resources,
        )
        .expect("heterogeneous placement admission");
        let mut view = place(admission);

        {
            let mut stable = view
                .project_mut(field_key(placement.access(), "stable"))
                .expect("stable projection");
            assert_eq!(
                stable.read().expect("stable read").access().operation(),
                AccessOperation::Read
            );
            assert_eq!(
                stable.write().expect("stable write").access().operation(),
                AccessOperation::Write
            );
            assert_eq!(
                stable
                    .compound_mutation()
                    .expect("stable compound mutation")
                    .access()
                    .operation(),
                AccessOperation::CompoundMutation
            );
        }
        {
            let mut fifo = view
                .project_mut(field_key(placement.access(), "fifo"))
                .expect("destructive projection");
            assert!(
                fifo.read().is_err(),
                "destructive observation must not derive Readable"
            );
            assert_eq!(
                fifo.take().expect("destructive take").access().operation(),
                AccessOperation::Take
            );
        }
        {
            let counter = view
                .project(field_key(placement.access(), "counter"))
                .expect("atomic projection");
            assert_eq!(
                counter
                    .atomic_load(MemoryOrdering::Acquire)
                    .expect("atomic load")
                    .access()
                    .operation(),
                AccessOperation::Atomic(AtomicAccessOperation::Load(MemoryOrdering::Acquire))
            );
            assert_eq!(
                counter
                    .atomic_fetch_add(MemoryOrdering::AcqRel)
                    .expect("atomic fetch-add")
                    .access()
                    .operation(),
                AccessOperation::Atomic(AtomicAccessOperation::FetchAdd(MemoryOrdering::AcqRel))
            );
            assert!(
                counter.atomic_fetch_sub(MemoryOrdering::AcqRel).is_err(),
                "unlisted atomic families must remain absent"
            );
            assert!(
                counter.atomic_load(MemoryOrdering::Release).is_err(),
                "operation-specific ordering legality remains sealed"
            );
        }
        assert!(
            view.project(field_key(placement.access(), "hidden"))
                .is_err(),
            "an inaccessible field must not project to an accessor"
        );
    }

    #[test]
    fn placed_view_rejects_unqualified_extent_or_unadmitted_reach() {
        let plan = uart_placement_plan();
        let short = uart_extent(0x1000, 8);
        let short_loan = short.loan(0, 8).expect("short loan");
        let rejection = admit_uart(8, short_loan, &plan, &uart_reach())
            .expect_err("layout must fit extent loan");
        assert!(rejection.diagnostic().0.contains("exceeds"));
        let (returned_loan, _) = rejection.into_parts();
        assert_eq!(
            returned_loan.length(),
            8,
            "rejection returns the exact loan"
        );

        let extent = uart_extent(0x1000, 64);
        let loan = extent.loan(0, 12).expect("UART loan");
        let rejection = admit_uart(9, loan, &plan, &BoundaryReach::default())
            .expect_err("service reach must agree with provenance admission");
        assert!(
            rejection
                .diagnostic()
                .0
                .contains("does not supply the placement's complete boundary reach")
        );
    }

    #[test]
    fn access_keys_and_placement_identity_bind_exact_layout_geometry() {
        let plan = uart_placement_plan();
        let mut alternate_layout = uart_layout();
        alternate_layout
            .entries
            .iter_mut()
            .find(|entry| entry.field == "status")
            .expect("status layout entry")
            .placement = LayoutPlacementReport::At { offset: 12 };
        alternate_layout.size = Some(16);
        let error = validate_access_plan(plan.access().plan().clone(), &alternate_layout)
            .expect_err("plan keys bind their exact layout");
        assert!(error.0.contains("different validated layout"));
        let alternate = validate_placement_plan(PlacementPlan {
            access: uart_access_source(&alternate_layout),
            layout: alternate_layout,
            reach: uart_reach(),
        })
        .expect("fresh plan over non-overlapping alternate geometry");
        assert_ne!(plan.access().identity(), alternate.access().identity());
        assert_ne!(plan.identity(), alternate.identity());
        assert_ne!(
            plan.access().layout_fingerprint(),
            alternate.access().layout_fingerprint(),
            "layout geometry is part of access-policy identity"
        );
    }

    #[test]
    fn resource_profiles_normalize_disjoint_regions_and_restrict_subranges() {
        let alternate_reach =
            BoundaryServiceReachId::from_normalized_identity(8).expect("alternate reach");
        let broad_reach = BoundaryReach::from_services([reach(), alternate_reach]);
        let stable = ResourceRegion {
            offset: 0,
            length: 4,
            stable: StableCapability::ReadWrite,
            external: ExternalCapability::None,
            atomic: AtomicCapability::None,
            reach: broad_reach.clone(),
        };
        let profile = validate_resource_profile(
            ResourceProfile {
                regions: vec![
                    ResourceRegion {
                        offset: 4,
                        ..stable.clone()
                    },
                    stable.clone(),
                    ResourceRegion {
                        offset: 8,
                        length: 8,
                        stable: StableCapability::None,
                        external: ExternalCapability::Access {
                            read: ExternalReadBehavior::Repeatable,
                            write: false,
                            transfers: vec![TransferRule {
                                width_bits: 32,
                                alignment_bytes: 4,
                            }],
                        },
                        atomic: AtomicCapability::None,
                        reach: broad_reach,
                    },
                ],
            },
            16,
        )
        .expect("disjoint profile");
        assert_eq!(
            profile.regions().len(),
            2,
            "adjacent identical regions normalize into one interval"
        );
        assert_eq!(profile.regions()[0].offset, 0);
        assert_eq!(profile.regions()[0].length, 8);

        let child = profile
            .restrict(4, 8, &uart_reach())
            .expect("subrange restriction");
        assert_eq!(child.length(), 8);
        assert_eq!(child.regions().len(), 2);
        assert_eq!(
            (child.regions()[0].offset, child.regions()[0].length),
            (0, 4)
        );
        assert_eq!(
            (child.regions()[1].offset, child.regions()[1].length),
            (4, 4)
        );
        assert!(child.regions().iter().all(|region| {
            region.reach.services().len() == 1 && region.reach.contains(reach())
        }));

        let overlap = validate_resource_profile(
            ResourceProfile {
                regions: vec![
                    stable,
                    ResourceRegion {
                        offset: 2,
                        length: 4,
                        stable: StableCapability::Read,
                        external: ExternalCapability::None,
                        atomic: AtomicCapability::None,
                        reach: BoundaryReach::default(),
                    },
                ],
            },
            8,
        )
        .expect_err("overlapping resource regions must reject");
        assert!(overlap.0.contains("overlap"));
    }

    #[test]
    fn resource_compatibility_joins_observation_operations_width_and_reach() {
        let plan = uart_placement_plan();
        let stable_profile = validate_resource_profile(
            ResourceProfile {
                regions: vec![ResourceRegion {
                    offset: 0,
                    length: 12,
                    stable: StableCapability::ReadWrite,
                    external: ExternalCapability::None,
                    atomic: AtomicCapability::None,
                    reach: uart_reach(),
                }],
            },
            12,
        )
        .expect("stable profile");
        let compatibility = validate_placement_resources(&plan, &stable_profile)
            .expect("stable supply may conservatively satisfy external demand");
        assert!(
            compatibility
                .fields()
                .iter()
                .all(|field| field.kind() == EffectiveSupplyKind::Stable)
        );
        assert_eq!(compatibility.base_congruence().modulus(), 4);
        assert_eq!(compatibility.base_congruence().residue(), 0);

        let read_only_external = validate_resource_profile(
            ResourceProfile {
                regions: vec![ResourceRegion {
                    offset: 0,
                    length: 12,
                    stable: StableCapability::None,
                    external: ExternalCapability::Access {
                        read: ExternalReadBehavior::Repeatable,
                        write: false,
                        transfers: vec![TransferRule {
                            width_bits: 32,
                            alignment_bytes: 4,
                        }],
                    },
                    atomic: AtomicCapability::None,
                    reach: uart_reach(),
                }],
            },
            12,
        )
        .expect("read-only external profile");
        let error = validate_placement_resources(&plan, &read_only_external)
            .expect_err("read-only external supply cannot satisfy UART writes");
        assert!(
            error.0.contains("control") && error.0.contains("incompatible External"),
            "canonical field order reports the first unsupported UART write: {error}"
        );

        let wrong_width = validate_resource_profile(
            ResourceProfile {
                regions: vec![ResourceRegion {
                    offset: 0,
                    length: 12,
                    stable: StableCapability::None,
                    external: ExternalCapability::Access {
                        read: ExternalReadBehavior::Repeatable,
                        write: true,
                        transfers: vec![TransferRule {
                            width_bits: 64,
                            alignment_bytes: 8,
                        }],
                    },
                    atomic: AtomicCapability::None,
                    reach: uart_reach(),
                }],
            },
            12,
        )
        .expect("wrong-width profile remains structurally valid");
        let error = validate_placement_resources(&plan, &wrong_width)
            .expect_err("transfer width must match exactly");
        assert!(
            error.0.contains("control") && error.0.contains("32-bit"),
            "canonical field order reports the first width mismatch: {error}"
        );

        let stable_demand = validate_placement_plan(PlacementPlan {
            layout: uart_layout(),
            access: access_plan(
                &uart_layout(),
                &[(
                    "status",
                    FieldAccess::Stable {
                        transfer_width_bits: 32,
                        read: true,
                        write: false,
                        exposure: AccessExposure::Exported,
                    },
                )],
            ),
            reach: uart_reach(),
        })
        .expect("stable demand");
        let error = validate_placement_resources(&stable_demand, &read_only_external)
            .expect_err("external supply cannot satisfy Stable demand");
        assert!(error.0.contains("requests Stable"));

        let destructive_demand = validate_placement_plan(PlacementPlan {
            layout: uart_layout(),
            access: access_plan(
                &uart_layout(),
                &[(
                    "status",
                    FieldAccess::External {
                        transfer_width_bits: 32,
                        read: ExternalRead::Take,
                        write: false,
                        exposure: AccessExposure::Exported,
                    },
                )],
            ),
            reach: uart_reach(),
        })
        .expect("destructive external demand");
        let error = validate_placement_resources(&destructive_demand, &read_only_external)
            .expect_err("repeatable reads cannot satisfy destructive observation");
        assert!(
            error.0.contains("status") && error.0.contains("Take"),
            "observation mismatch must name the destructive demand: {error}"
        );

        let atomic_layout = LayoutPlanReport {
            schema_identity: 93,
            entries: vec![LayoutFieldEntryReport {
                field: "head".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 0 },
            }],
            offsets: Some(vec![0]),
            size: Some(4),
            align: 4,
        };
        let atomic_demand = validate_placement_plan(PlacementPlan {
            access: access_plan(
                &atomic_layout,
                &[(
                    "head",
                    FieldAccess::Atomic {
                        transfer_width_bits: 32,
                        operations: AtomicPermissions {
                            load: true,
                            fetch_add: true,
                            ..AtomicPermissions::default()
                        },
                        exposure: AccessExposure::Exported,
                    },
                )],
            ),
            layout: atomic_layout,
            reach: BoundaryReach::default(),
        })
        .expect("atomic demand");
        let load_only = validate_resource_profile(
            ResourceProfile {
                regions: vec![ResourceRegion {
                    offset: 0,
                    length: 4,
                    stable: StableCapability::None,
                    external: ExternalCapability::None,
                    atomic: AtomicCapability::Access {
                        transfers: vec![AtomicTransferRule {
                            transfer: TransferRule {
                                width_bits: 32,
                                alignment_bytes: 4,
                            },
                            operations: AtomicPermissions {
                                load: true,
                                ..AtomicPermissions::default()
                            },
                        }],
                    },
                    reach: BoundaryReach::default(),
                }],
            },
            4,
        )
        .expect("load-only atomic profile");
        let error = validate_placement_resources(&atomic_demand, &load_only)
            .expect_err("atomic operation demand must be an exact supply subset");
        assert!(
            error.0.contains("head") && error.0.contains("operation families"),
            "atomic mismatch must name the field and operation family: {error}"
        );
    }

    #[test]
    fn subrange_loan_rebases_profile_and_preserves_denied_bytes() {
        let layout = LayoutPlanReport {
            schema_identity: 94,
            entries: vec![LayoutFieldEntryReport {
                field: "word".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 0 },
            }],
            offsets: Some(vec![0]),
            size: Some(4),
            align: 4,
        };
        let placement = validate_placement_plan(PlacementPlan {
            access: access_plan(
                &layout,
                &[(
                    "word",
                    FieldAccess::External {
                        transfer_width_bits: 32,
                        read: ExternalRead::Read,
                        write: false,
                        exposure: AccessExposure::Exported,
                    },
                )],
            ),
            layout,
            reach: BoundaryReach::default(),
        })
        .expect("subrange placement");
        let extent = uart_extent(0x4000, 16);
        let profile = ResourceProfileGrant::from_admitted_provider(
            ResourceProfileReceiptId::from_normalized_identity(41).expect("profile receipt"),
            0x4000,
            16,
            extent_id(2, AddressSpaceId::from_normalized_identity),
            extent_id(5, ExtentProvenanceId::from_normalized_identity),
            extent_id(6, omega_extents::MappingEraId::from_normalized_identity),
            extent_rights(&[3]),
            BoundaryReach::default(),
        )
        .expect("profile grant")
        .admit(ResourceProfile {
            regions: vec![ResourceRegion {
                offset: 4,
                length: 4,
                stable: StableCapability::None,
                external: ExternalCapability::Access {
                    read: ExternalReadBehavior::Repeatable,
                    write: false,
                    transfers: vec![TransferRule {
                        width_bits: 32,
                        alignment_bytes: 4,
                    }],
                },
                atomic: AtomicCapability::None,
                reach: BoundaryReach::default(),
            }],
        })
        .expect("sparse admitted profile");

        {
            let loan = extent.loan(4, 4).expect("covered subrange loan");
            let admission = admit_placement(
                PlacementAdmissionId::from_normalized_identity(42).expect("admission"),
                loan,
                &placement,
                &profile,
            )
            .expect("resource region must rebase to the subrange loan");
            assert_eq!(admission.resources().fields()[0].offset(), 0);
            let view = place(admission);
            assert_eq!(view.base(), 0x4004);
        }

        let loan = extent.loan(0, 4).expect("uncovered subrange loan");
        let rejection = admit_placement(
            PlacementAdmissionId::from_normalized_identity(43).expect("admission"),
            loan,
            &placement,
            &profile,
        )
        .expect_err("profile restriction must not fill uncovered parent bytes");
        assert!(
            rejection.diagnostic().0.contains("not covered"),
            "uncovered subrange rejection must report missing supply"
        );
    }

    #[test]
    fn transfer_alignment_derives_build_time_and_runtime_base_checks() {
        let conflicting_layout = LayoutPlanReport {
            schema_identity: 91,
            entries: vec![
                LayoutFieldEntryReport {
                    field: "left".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At { offset: 0 },
                },
                LayoutFieldEntryReport {
                    field: "right".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At { offset: 2 },
                },
            ],
            offsets: Some(vec![0, 2]),
            size: Some(8),
            align: 2,
        };
        let conflicting = validate_placement_plan(PlacementPlan {
            access: access_plan(
                &conflicting_layout,
                &[
                    (
                        "left",
                        FieldAccess::External {
                            transfer_width_bits: 32,
                            read: ExternalRead::Read,
                            write: false,
                            exposure: AccessExposure::Exported,
                        },
                    ),
                    (
                        "right",
                        FieldAccess::External {
                            transfer_width_bits: 32,
                            read: ExternalRead::Read,
                            write: false,
                            exposure: AccessExposure::Exported,
                        },
                    ),
                ],
            ),
            layout: conflicting_layout,
            reach: BoundaryReach::default(),
        })
        .expect("relative geometry is structurally valid");
        let profile = validate_resource_profile(
            ResourceProfile {
                regions: vec![ResourceRegion {
                    offset: 0,
                    length: 8,
                    stable: StableCapability::None,
                    external: ExternalCapability::Access {
                        read: ExternalReadBehavior::Repeatable,
                        write: false,
                        transfers: vec![TransferRule {
                            width_bits: 32,
                            alignment_bytes: 4,
                        }],
                    },
                    atomic: AtomicCapability::None,
                    reach: BoundaryReach::default(),
                }],
            },
            8,
        )
        .expect("alignment profile");
        let error = validate_placement_resources(&conflicting, &profile)
            .expect_err("inconsistent field congruences must reject before admission");
        assert!(
            error.0.contains("right")
                && error.0.contains("offset 2")
                && error.0.contains("conflicts")
        );

        let layout = LayoutPlanReport {
            schema_identity: 92,
            entries: vec![LayoutFieldEntryReport {
                field: "word".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 0 },
            }],
            offsets: Some(vec![0]),
            size: Some(4),
            align: 1,
        };
        let placement = validate_placement_plan(PlacementPlan {
            access: access_plan(
                &layout,
                &[(
                    "word",
                    FieldAccess::External {
                        transfer_width_bits: 32,
                        read: ExternalRead::Read,
                        write: false,
                        exposure: AccessExposure::Exported,
                    },
                )],
            ),
            layout,
            reach: BoundaryReach::default(),
        })
        .expect("single-field placement");
        let extent = uart_extent(0x1002, 4);
        let loan = extent.loan(0, 4).expect("misaligned loan");
        let resources = ResourceProfileGrant::from_admitted_provider(
            ResourceProfileReceiptId::from_normalized_identity(22).expect("profile receipt"),
            0x1002,
            4,
            extent_id(2, AddressSpaceId::from_normalized_identity),
            extent_id(5, ExtentProvenanceId::from_normalized_identity),
            extent_id(6, omega_extents::MappingEraId::from_normalized_identity),
            extent_rights(&[3]),
            BoundaryReach::default(),
        )
        .expect("profile grant")
        .admit(ResourceProfile {
            regions: vec![ResourceRegion {
                offset: 0,
                length: 4,
                stable: StableCapability::None,
                external: ExternalCapability::Access {
                    read: ExternalReadBehavior::Repeatable,
                    write: false,
                    transfers: vec![TransferRule {
                        width_bits: 32,
                        alignment_bytes: 4,
                    }],
                },
                atomic: AtomicCapability::None,
                reach: BoundaryReach::default(),
            }],
        })
        .expect("admitted profile");
        let rejection = admit_placement(
            PlacementAdmissionId::from_normalized_identity(23).expect("admission"),
            loan,
            &placement,
            &resources,
        )
        .expect_err("actual base must discharge the derived congruence");
        assert!(rejection.diagnostic().0.contains("base mod 4 must equal 0"));
    }

    #[test]
    fn admitted_profile_binds_rights_provenance_era_and_returns_rejected_loan() {
        let plan = uart_placement_plan();
        let extent = uart_extent(0x3000, 12)
            .attenuate(extent_rights(&[3]))
            .expect("attenuated extent");
        let loan = extent.loan(0, 12).expect("UART loan");
        let profile = ResourceProfileGrant::from_admitted_provider(
            ResourceProfileReceiptId::from_normalized_identity(31).expect("profile receipt"),
            0x3000,
            12,
            extent_id(2, AddressSpaceId::from_normalized_identity),
            extent_id(5, ExtentProvenanceId::from_normalized_identity),
            extent_id(6, omega_extents::MappingEraId::from_normalized_identity),
            extent_rights(&[4]),
            uart_reach(),
        )
        .expect("profile grant")
        .admit(ResourceProfile {
            regions: vec![ResourceRegion {
                offset: 0,
                length: 12,
                stable: StableCapability::None,
                external: ExternalCapability::Access {
                    read: ExternalReadBehavior::Repeatable,
                    write: true,
                    transfers: vec![TransferRule {
                        width_bits: 32,
                        alignment_bytes: 4,
                    }],
                },
                atomic: AtomicCapability::None,
                reach: uart_reach(),
            }],
        })
        .expect("admitted profile");
        let rejection = admit_placement(
            PlacementAdmissionId::from_normalized_identity(32).expect("admission"),
            loan,
            &plan,
            &profile,
        )
        .expect_err("attenuated loan cannot recover profile-bound rights");
        assert!(rejection.diagnostic().0.contains("lacks rights"));
        let (returned, _) = rejection.into_parts();
        assert_eq!(returned.base(), 0x3000);
        assert_eq!(returned.length(), 12);
    }
}
