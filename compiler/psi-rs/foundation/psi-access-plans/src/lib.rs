#![forbid(unsafe_code)]

//! Normalized access policy for placed views.
//!
//! `LayoutPlan` owns geometry. `AccessPlan` owns observation and the exact
//! primitive operations permitted over that geometry. Keeping them separate
//! prevents wire layouts from acquiring MMIO vocabulary and prevents an
//! arbitrary-offset volatile escape hatch from bypassing plan validation.

use std::collections::{BTreeMap, BTreeSet};

use psi_extents::{
    AddressSpaceId, Extent, ExtentContentCustodyReceiptId, ExtentContentValidityReceiptId,
    ExtentLineageId, ExtentLoan, ExtentProvenanceId, ExtentRights, ExtentRootOrigin, LoanPolarity,
    MappingEraId, ProviderExistingContentGrant, ResidentClaimId,
};
use psi_language_core::atomic::{AtomicOrderingPlan, MemoryOrdering};
use psi_layout_plans::{
    LayoutPlacementReport, LayoutPlanReport, layout_plan_reports_match_for_replay,
    normalized_layout_plan_fingerprint,
};

mod corresponded_atomic;
mod corresponded_external;
mod corresponded_stable;
mod corresponded_stable_compound;
mod resident_views;
mod schema_correspondence;

pub use corresponded_atomic::{
    CorrespondedAtomicPrimitiveAccessRejection, CorrespondedAtomicPrimitiveAccessRequest,
};
pub use corresponded_external::{
    CorrespondedExternalPrimitiveAccessRejection, CorrespondedExternalPrimitiveAccessRequest,
};
pub use corresponded_stable::{
    CorrespondedStablePrimitiveAccessRejection, CorrespondedStablePrimitiveAccessRequest,
};
pub use corresponded_stable_compound::{
    CorrespondedStableCompoundMutationAccessRejection,
    CorrespondedStableCompoundMutationAccessRequest,
};
pub use resident_views::{BorrowedResidentRetirementError, EstablishedBorrowedResidentPlacement};
pub use schema_correspondence::{
    AdmittedSchemaDeviceCorrespondence, DeviceRevisionPredicateId, RuntimeDeviceRevisionEvidence,
    RuntimeDeviceRevisionObservationId, SchemaCorrespondedPlaceEstablishmentError,
    SchemaCorrespondedPlaceRetirementError, SchemaCorrespondedPlacedView,
    SchemaCorrespondedPlacementAdmission, SchemaCorrespondencePlacementBindingError,
    SchemaCorrespondenceProviderId, SchemaCorrespondenceSourceId,
    SchemaDeviceCorrespondenceAdmissionError, SchemaDeviceCorrespondenceGrant,
    SchemaDeviceCorrespondenceGrantError, StableDeviceInstanceId,
    bind_schema_correspondence_to_placement,
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
    /// Observing decisive compare-exchange.
    pub compare_exchange: bool,
    /// Observing single-attempt compare-exchange.
    pub compare_exchange_once: bool,
    /// Non-observing decisive compare-exchange.
    pub try_exchange: bool,
    /// Non-observing single-attempt compare-exchange.
    pub try_exchange_once: bool,
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
            || self.compare_exchange_once
            || self.try_exchange
            || self.try_exchange_once
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
            && (!required.compare_exchange_once || self.compare_exchange_once)
            && (!required.try_exchange || self.try_exchange)
            && (!required.try_exchange_once || self.try_exchange_once)
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessPlan {
    layout_fingerprint: u64,
    retained_layout: LayoutPlanReport,
    entries: Vec<AccessFieldEntry>,
}

impl std::hash::Hash for AccessPlan {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.layout_fingerprint, state);
        std::hash::Hash::hash(&self.entries, state);
    }
}

impl AccessPlan {
    pub fn inaccessible(layout: &LayoutPlanReport) -> Result<Self, AccessPlanDiagnostic> {
        let layout_fingerprint = normalized_layout_plan_fingerprint(layout);
        let mut canonical_fields = BTreeMap::new();
        let mut presentation_names = BTreeMap::new();
        let mut presentation_identities = BTreeMap::new();
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
            if let Some(prior) =
                presentation_identities.insert(entry.field.clone(), identity.clone())
                && prior != identity
            {
                return Err(AccessPlanDiagnostic(format!(
                    "layout field `{}` identifies both {} and {}",
                    entry.field,
                    canonical_field_identity_label(&prior),
                    canonical_field_identity_label(&identity),
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
            retained_layout: layout.clone(),
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

fn canonical_field_identity_label(identity: &CanonicalFieldIdentity) -> String {
    match identity {
        CanonicalFieldIdentity::Numbered(identity) => {
            format!("stable member identity #{identity}")
        }
        CanonicalFieldIdentity::Positional(field) => {
            format!("positional field identity `{field}`")
        }
    }
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
    logical_extent: LogicalFieldExtent,
    effect_footprint: RelativeEffectFootprint,
    observation: ObservationModel,
    permissions: AccessPermissions,
    exposure: AccessExposure,
}

/// The exact laid bits that represent one logical field value.
///
/// A fragmented field may contain several pieces, but every piece remains in
/// the one transfer container admitted for primitive placed access. The source
/// bit offset names the corresponding position in the logical field value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalFieldExtent {
    fragments: Vec<LogicalFieldFragment>,
}

impl LogicalFieldExtent {
    pub fn fragments(&self) -> &[LogicalFieldFragment] {
        &self.fragments
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LogicalFieldFragment {
    layout_bit_offset: u64,
    source_bit_offset: u64,
    width_bits: u64,
}

impl LogicalFieldFragment {
    pub const fn layout_bit_offset(self) -> u64 {
        self.layout_bit_offset
    }

    pub const fn source_bit_offset(self) -> u64 {
        self.source_bit_offset
    }

    pub const fn width_bits(self) -> u64 {
        self.width_bits
    }
}

/// The complete relative transfer container observed or changed by an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RelativeEffectFootprint {
    byte_offset: u64,
    length_bytes: u64,
}

impl RelativeEffectFootprint {
    pub const fn byte_offset(self) -> u64 {
        self.byte_offset
    }

    pub const fn length_bytes(self) -> u64 {
        self.length_bytes
    }

    pub const fn end(self) -> u64 {
        self.byte_offset + self.length_bytes
    }

    pub const fn overlaps(self, other: Self) -> bool {
        self.byte_offset < other.end() && other.byte_offset < self.end()
    }
}

/// The complete concrete transfer container observed or changed by an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EffectFootprint {
    address: u64,
    length_bytes: u64,
}

impl EffectFootprint {
    pub const fn address(self) -> u64 {
        self.address
    }

    pub const fn length_bytes(self) -> u64 {
        self.length_bytes
    }

    pub const fn end(self) -> u64 {
        self.address + self.length_bytes
    }

    pub const fn overlaps(self, other: Self) -> bool {
        self.address < other.end() && other.address < self.end()
    }
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

    pub const fn logical_extent(&self) -> &LogicalFieldExtent {
        &self.logical_extent
    }

    pub const fn effect_footprint(&self) -> RelativeEffectFootprint {
        self.effect_footprint
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
    if plan.layout_fingerprint != expected.layout_fingerprint
        || !layout_plan_reports_match_for_replay(&plan.retained_layout, layout)
    {
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
        let (container_byte_offset, logical_extent, effect_footprint) = validate_entry_geometry(
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
            logical_extent,
            effect_footprint,
            observation: policy.observation,
            permissions: policy.permissions,
            exposure: policy.exposure,
        });
    }
    validate_external_write_units(&descriptors)?;
    validate_destructive_access_units(&descriptors)?;
    validate_atomic_transfer_units(&descriptors)?;

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

fn validate_atomic_transfer_units(
    descriptors: &[FieldAccessDescriptor],
) -> Result<(), AccessPlanDiagnostic> {
    let atomic = descriptors
        .iter()
        .filter(|descriptor| descriptor.observation == ObservationModel::Atomic)
        .collect::<Vec<_>>();
    for (index, left) in atomic.iter().enumerate() {
        if let Some(right) = atomic[index + 1..].iter().find(|right| {
            left.effect_footprint.overlaps(right.effect_footprint)
                && left.effect_footprint != right.effect_footprint
        }) {
            return Err(AccessPlanDiagnostic(format!(
                "atomic fields `{}` and `{}` select overlapping transfer containers {}..{} and {}..{}; one active atomic placement cannot mix widths over the same bytes",
                left.field,
                right.field,
                left.effect_footprint.byte_offset,
                left.effect_footprint.end(),
                right.effect_footprint.byte_offset,
                right.effect_footprint.end(),
            )));
        }
    }
    Ok(())
}

fn validate_external_write_units(
    descriptors: &[FieldAccessDescriptor],
) -> Result<(), AccessPlanDiagnostic> {
    for descriptor in descriptors.iter().filter(|descriptor| {
        descriptor.observation == ObservationModel::External && descriptor.permissions.write
    }) {
        if !logical_extent_covers_effect(&descriptor.logical_extent, descriptor.effect_footprint) {
            return Err(AccessPlanDiagnostic(format!(
                "external field `{}` names only part of its {}-byte transfer container; a generic External write must cover the complete admitted container",
                descriptor.field, descriptor.effect_footprint.length_bytes
            )));
        }
    }
    Ok(())
}

fn validate_destructive_access_units(
    descriptors: &[FieldAccessDescriptor],
) -> Result<(), AccessPlanDiagnostic> {
    for destructive in descriptors
        .iter()
        .filter(|descriptor| descriptor.permissions.take)
    {
        if !logical_extent_covers_effect(&destructive.logical_extent, destructive.effect_footprint)
        {
            return Err(AccessPlanDiagnostic(format!(
                "destructive field `{}` names only part of its {}-byte transfer container; expose one whole-container snapshot and project fields from the owned result",
                destructive.field, destructive.effect_footprint.length_bytes
            )));
        }
        if let Some(overlapping) = descriptors.iter().find(|candidate| {
            candidate.key != destructive.key
                && candidate
                    .effect_footprint
                    .overlaps(destructive.effect_footprint)
        }) {
            return Err(AccessPlanDiagnostic(format!(
                "destructive field `{}` and field `{}` expose overlapping transfer containers; one destructive unit derives one whole-snapshot take",
                destructive.field, overlapping.field
            )));
        }
    }
    Ok(())
}

fn logical_extent_covers_effect(
    logical: &LogicalFieldExtent,
    effect: RelativeEffectFootprint,
) -> bool {
    let Some(effect_start) = effect.byte_offset.checked_mul(8) else {
        return false;
    };
    let Some(effect_end) = effect.end().checked_mul(8) else {
        return false;
    };
    let mut by_layout = logical.fragments.iter().copied().collect::<Vec<_>>();
    by_layout.sort_unstable_by_key(|fragment| fragment.layout_bit_offset);
    let mut next_bit = effect_start;
    for fragment in by_layout {
        if fragment.layout_bit_offset != next_bit {
            return false;
        }
        let Some(end) = fragment.layout_bit_offset.checked_add(fragment.width_bits) else {
            return false;
        };
        if end > effect_end {
            return false;
        }
        next_bit = end;
    }
    if next_bit != effect_end {
        return false;
    }

    let mut by_source = logical.fragments.iter().copied().collect::<Vec<_>>();
    by_source.sort_unstable_by_key(|fragment| fragment.source_bit_offset);
    let mut next_source_bit = 0;
    for fragment in by_source {
        if fragment.source_bit_offset != next_source_bit {
            return false;
        }
        let Some(end) = fragment.source_bit_offset.checked_add(fragment.width_bits) else {
            return false;
        };
        next_source_bit = end;
    }
    next_source_bit == effect_end - effect_start
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
        permissions.compare_exchange_once,
        permissions.try_exchange,
        permissions.try_exchange_once,
    ] {
        hash_byte(hash, u8::from(enabled));
    }
}

fn normalized_access_plan_identity(plan: &AccessPlan, layout_fingerprint: u64) -> AccessPlanId {
    // FNV-1a is used as a compact deterministic artifact identity here, never
    // as authorization or collision-resistant evidence. The versioned prefix
    // makes any future vocabulary change an explicit identity migration.
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(&mut hash, b"omega.access-plan.v5");
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
    origin: ExtentRootOrigin,
    lineage_root: ExtentLineageId,
    required_rights: ExtentRights,
    permitted_reach: BoundaryReach,
}

impl ResourceProfileGrant {
    /// Bind provider supply to one exact granted Extent authority account.
    ///
    /// Taking the opaque Extent instead of a restated geometry/provenance
    /// tuple prevents a profile receipt from being replayed against a
    /// coincident but independently introduced root.
    pub fn from_admitted_provider(
        receipt: ResourceProfileReceiptId,
        extent: &Extent,
        required_rights: ExtentRights,
        permitted_reach: BoundaryReach,
    ) -> Result<Self, AccessPlanDiagnostic> {
        Self::from_bound_extent(
            receipt,
            extent.base(),
            extent.length(),
            extent.address_space(),
            extent.provenance(),
            extent.era(),
            extent.origin(),
            extent.lineage_root(),
            required_rights,
            permitted_reach,
        )
    }

    /// Bind provider supply directly to the exact qualified subrange loan
    /// that will feed placement admission.
    pub fn from_admitted_provider_loan(
        receipt: ResourceProfileReceiptId,
        loan: &ExtentLoan<'_>,
        required_rights: ExtentRights,
        permitted_reach: BoundaryReach,
    ) -> Result<Self, AccessPlanDiagnostic> {
        Self::from_bound_extent(
            receipt,
            loan.base(),
            loan.length(),
            loan.address_space(),
            loan.provenance(),
            loan.era(),
            loan.origin(),
            loan.lineage_root(),
            required_rights,
            permitted_reach,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn from_bound_extent(
        receipt: ResourceProfileReceiptId,
        base: u64,
        length: u64,
        address_space: AddressSpaceId,
        provenance: ExtentProvenanceId,
        era: MappingEraId,
        origin: ExtentRootOrigin,
        lineage_root: ExtentLineageId,
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
            origin,
            lineage_root,
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
            origin: self.origin,
            lineage_root: self.lineage_root,
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
    origin: ExtentRootOrigin,
    lineage_root: ExtentLineageId,
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
        if loan.origin() != self.origin {
            return Err(AccessPlanDiagnostic(
                "extent sealed root origin does not match admitted resource profile".into(),
            ));
        }
        if loan.lineage_root() != self.lineage_root {
            return Err(AccessPlanDiagnostic(
                "extent root lineage does not match admitted resource profile".into(),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlacedOccurrenceId(u64);

impl PlacedOccurrenceId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, AccessPlanDiagnostic> {
        if identity == 0 {
            return Err(AccessPlanDiagnostic(
                "placed-occurrence identity cannot be zero".into(),
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
    profile: AdmittedResourceProfile,
    resources: PlacementResourceCompatibility,
    loan: ExtentLoan<'extent>,
}

impl<'extent> PlacementAdmission<'extent> {
    pub const fn identity(&self) -> PlacementAdmissionId {
        self.identity
    }

    pub const fn profile_receipt(&self) -> ResourceProfileReceiptId {
        self.profile_receipt
    }

    pub const fn resources(&self) -> &PlacementResourceCompatibility {
        &self.resources
    }

    /// Cancel permission-only admission and recover the exact source loan.
    ///
    /// No placed content has been established at this stage, so withdrawal
    /// makes no content, destruction, vacancy, or allocator-release claim.
    pub fn withdraw(self) -> ExtentLoan<'extent> {
        self.loan
    }
}

#[derive(Debug)]
pub struct PlacementRejection<'extent> {
    loan: ExtentLoan<'extent>,
    diagnostic: AccessPlanDiagnostic,
}

/// Failed borrowed placed-view establishment returns the highest valid
/// loan-bearing admission intact for corrected retry or withdrawal.
#[derive(Debug)]
pub struct PlaceEstablishmentError<'extent> {
    admission: PlacementAdmission<'extent>,
    diagnostic: AccessPlanDiagnostic,
}

impl<'extent> PlaceEstablishmentError<'extent> {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (PlacementAdmission<'extent>, AccessPlanDiagnostic) {
        (self.admission, self.diagnostic)
    }
}

impl<'extent> PlacementRejection<'extent> {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ExtentLoan<'extent>, AccessPlanDiagnostic) {
        (self.loan, self.diagnostic)
    }
}

/// One accepted whole-range placement admission that retains the exact owned
/// Extent checked against provider supply.
///
/// This is permission to establish placed content, not evidence that content
/// already exists. A later explicit Stable initialize/validate/adopt or
/// External adopt route must consume this carrier. Withdrawing it therefore
/// returns only the original granted Extent and establishes no `Vacant` fact.
#[derive(Debug)]
#[must_use = "an owned placement admission retains linear Extent authority"]
pub struct OwnedPlacementAdmission {
    identity: PlacementAdmissionId,
    placement_plan: ValidatedPlacementPlan,
    profile_receipt: ResourceProfileReceiptId,
    profile: AdmittedResourceProfile,
    resources: PlacementResourceCompatibility,
    extent: Extent,
}

impl OwnedPlacementAdmission {
    pub const fn identity(&self) -> PlacementAdmissionId {
        self.identity
    }

    pub const fn profile_receipt(&self) -> ResourceProfileReceiptId {
        self.profile_receipt
    }

    pub const fn resources(&self) -> &PlacementResourceCompatibility {
        &self.resources
    }

    pub const fn extent(&self) -> &Extent {
        &self.extent
    }

    pub const fn placement_plan(&self) -> &ValidatedPlacementPlan {
        &self.placement_plan
    }

    /// Cancel permission-only admission without claiming content
    /// establishment, destruction, vacancy, or allocator release.
    pub fn withdraw(self) -> Extent {
        self.extent
    }
}

/// Failed owned admission returns the exact moved Extent rather than losing
/// or reconstructing its authority account.
#[derive(Debug)]
pub struct OwnedPlacementRejection {
    extent: Extent,
    diagnostic: AccessPlanDiagnostic,
}

impl OwnedPlacementRejection {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (Extent, AccessPlanDiagnostic) {
        (self.extent, self.diagnostic)
    }
}

/// Dormant provider-validated Stable content whose exact Extent authority and
/// resident claim are retained by the accepted placement admission.
///
/// This is the first content-establishing owned carrier. It deliberately has
/// neither field projection nor a route back to a bare Extent. An explicit
/// view transition creates one fresh active placed occurrence; checked
/// destruction or move-out must land before another retirement route can
/// establish `Vacant` and release storage authority.
#[derive(Debug)]
#[must_use = "dormant resident content retains linear Extent and content custody"]
pub struct DormantOwnedResident {
    admission: OwnedPlacementAdmission,
    content: ProviderExistingContentGrant,
}

/// Failed owned resident-view establishment preserves the complete dormant
/// content authority and the exact requested occurrence for corrected retry.
#[derive(Debug)]
pub struct OwnedResidentViewEstablishmentError {
    resident: DormantOwnedResident,
    occurrence: PlacedOccurrenceId,
    diagnostic: AccessPlanDiagnostic,
}

impl OwnedResidentViewEstablishmentError {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        DormantOwnedResident,
        PlacedOccurrenceId,
        AccessPlanDiagnostic,
    ) {
        (self.resident, self.occurrence, self.diagnostic)
    }
}

impl DormantOwnedResident {
    pub const fn admission(&self) -> PlacementAdmissionId {
        self.admission.identity
    }

    pub const fn placement_plan(&self) -> &ValidatedPlacementPlan {
        &self.admission.placement_plan
    }

    pub const fn profile_receipt(&self) -> ResourceProfileReceiptId {
        self.admission.profile_receipt
    }

    pub const fn resources(&self) -> &PlacementResourceCompatibility {
        &self.admission.resources
    }

    pub const fn extent(&self) -> &Extent {
        &self.admission.extent
    }

    pub const fn validity_receipt(&self) -> ExtentContentValidityReceiptId {
        self.content.validity_receipt()
    }

    pub const fn custody_receipt(&self) -> ExtentContentCustodyReceiptId {
        self.content.custody_receipt()
    }

    pub const fn resident_claim(&self) -> ResidentClaimId {
        self.content.resident_claim()
    }

    /// Transfer dormant resident custody into one requested active placed
    /// occurrence after replaying the retained owned placement authority.
    /// The resident claim and provider receipts are forwarded unchanged; the
    /// occurrence issuer remains responsible for global freshness.
    pub fn view(
        self,
        occurrence: PlacedOccurrenceId,
    ) -> Result<EstablishedOwnedPlacement, OwnedResidentViewEstablishmentError> {
        if let Err(diagnostic) =
            validate_owned_resident_authority(&self.admission, &self.content, "owned resident view")
        {
            return Err(OwnedResidentViewEstablishmentError {
                resident: self,
                occurrence,
                diagnostic,
            });
        }
        Ok(EstablishedOwnedPlacement {
            admission: self.admission,
            content: self.content,
            occurrence,
        })
    }
}

/// One active owned view of provider-established Stable resident content.
/// The occurrence is fresh for this view while `resident_claim` remains the
/// identity of the same dormant content across view/retirement cycles.
#[derive(Debug)]
#[must_use = "active owned placed content retains linear resident custody"]
pub struct EstablishedOwnedPlacement {
    admission: OwnedPlacementAdmission,
    content: ProviderExistingContentGrant,
    occurrence: PlacedOccurrenceId,
}

/// Failed resident-preserving retirement returns the complete active carrier;
/// no dormant claim is minted from drifted placement authority.
#[derive(Debug)]
pub struct OwnedResidentRetirementError {
    established: EstablishedOwnedPlacement,
    diagnostic: AccessPlanDiagnostic,
}

impl OwnedResidentRetirementError {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (EstablishedOwnedPlacement, AccessPlanDiagnostic) {
        (self.established, self.diagnostic)
    }
}

impl EstablishedOwnedPlacement {
    pub const fn admission(&self) -> PlacementAdmissionId {
        self.admission.identity
    }

    pub const fn placement_plan(&self) -> &ValidatedPlacementPlan {
        &self.admission.placement_plan
    }

    pub const fn profile_receipt(&self) -> ResourceProfileReceiptId {
        self.admission.profile_receipt
    }

    pub const fn resources(&self) -> &PlacementResourceCompatibility {
        &self.admission.resources
    }

    pub const fn extent(&self) -> &Extent {
        &self.admission.extent
    }

    pub const fn validity_receipt(&self) -> ExtentContentValidityReceiptId {
        self.content.validity_receipt()
    }

    pub const fn custody_receipt(&self) -> ExtentContentCustodyReceiptId {
        self.content.custody_receipt()
    }

    pub const fn resident_claim(&self) -> ResidentClaimId {
        self.content.resident_claim()
    }

    pub const fn occurrence(&self) -> PlacedOccurrenceId {
        self.occurrence
    }

    /// End this active owned view without destroying or moving out its
    /// content. The exact resident claim and provider receipts return to the
    /// dormant carrier; the active occurrence ends here.
    pub fn retire_resident(self) -> Result<DormantOwnedResident, OwnedResidentRetirementError> {
        if let Err(diagnostic) = validate_owned_resident_authority(
            &self.admission,
            &self.content,
            "resident-preserving retirement",
        ) {
            return Err(OwnedResidentRetirementError {
                established: self,
                diagnostic,
            });
        }
        Ok(DormantOwnedResident {
            admission: self.admission,
            content: self.content,
        })
    }

    /// Purely project one accepted Stable field through a shared borrow of
    /// this provider-established owned placement.
    ///
    /// The returned accessor retains this entire carrier, including its
    /// content-validity and custody evidence, through any sealed primitive
    /// request derived from it.
    pub fn project<'view>(
        &'view self,
        key: AccessFieldKey,
    ) -> Result<PlacedFieldProjection<'view, 'view>, AccessPlanDiagnostic> {
        self.project_with(key, BorrowPolarity::Shared)
    }

    /// Purely project one accepted Stable field through an exclusive borrow
    /// of this provider-established owned placement.
    pub fn project_mut<'view>(
        &'view mut self,
        key: AccessFieldKey,
    ) -> Result<PlacedFieldProjection<'view, 'view>, AccessPlanDiagnostic> {
        self.project_with(key, BorrowPolarity::Exclusive)
    }

    fn project_with<'view>(
        &'view self,
        key: AccessFieldKey,
        current_borrow: BorrowPolarity,
    ) -> Result<PlacedFieldProjection<'view, 'view>, AccessPlanDiagnostic> {
        project_placed_field(
            &self.admission.placement_plan,
            self.admission.profile_receipt,
            &self.admission.resources,
            self.admission.identity,
            self.admission.extent.base(),
            key,
            current_borrow,
            BorrowPolarity::Exclusive,
            Some(ObservationModel::Stable),
            PlacementAuthorityRef::EstablishedOwned(self),
        )
    }
}

/// Failed Stable adoption preserves both linear inputs for a corrected retry
/// or explicit cancellation.
#[derive(Debug)]
pub struct OwnedStableAdoptionError {
    admission: OwnedPlacementAdmission,
    content: ProviderExistingContentGrant,
    diagnostic: AccessPlanDiagnostic,
}

impl OwnedStableAdoptionError {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        OwnedPlacementAdmission,
        ProviderExistingContentGrant,
        AccessPlanDiagnostic,
    ) {
        (self.admission, self.content, self.diagnostic)
    }
}

/// A plan-qualified interpretation of one borrowed concrete range.
#[derive(Debug)]
pub struct PlacedView<'extent> {
    loan: ExtentLoan<'extent>,
    plan: ValidatedPlacementPlan,
    profile_receipt: ResourceProfileReceiptId,
    profile: AdmittedResourceProfile,
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

    /// End this ordinary borrowed view after independently replaying its exact
    /// loan, placement, admitted profile, receipt, and resource compatibility.
    /// Success returns the original loan; rejection returns this complete view
    /// for repair and retry. No content, vacancy, or destruction is claimed.
    pub fn retire(self) -> Result<ExtentLoan<'extent>, PlacedViewRetirementError<'extent>> {
        if let Err(diagnostic) = self.validate_authority("borrowed placed-view retirement") {
            return Err(PlacedViewRetirementError {
                view: self,
                diagnostic,
            });
        }
        Ok(self.loan)
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
        project_placed_field(
            &self.plan,
            self.profile_receipt,
            &self.resources,
            self.admission,
            self.loan.base(),
            key,
            current_borrow,
            source_loan,
            None,
            PlacementAuthorityRef::Borrowed(self),
        )
    }

    fn validate_authority(&self, transition: &str) -> Result<(), AccessPlanDiagnostic> {
        if self.profile.receipt() != self.profile_receipt {
            return Err(AccessPlanDiagnostic(format!(
                "{transition} could not replay the exact admitted resource-profile receipt"
            )));
        }
        let replayed = validate_placement_admission(&self.loan, &self.plan, &self.profile)
            .map_err(|diagnostic| {
                AccessPlanDiagnostic(format!(
                    "{transition} could not replay the retained placement authority: {diagnostic}"
                ))
            })?;
        if replayed != self.resources {
            return Err(AccessPlanDiagnostic(format!(
                "{transition} replayed resource compatibility differs from the retained view"
            )));
        }
        Ok(())
    }
}

/// Failed ordinary borrowed-view retirement preserves the complete
/// loan-bearing view for corrected retry.
#[derive(Debug)]
pub struct PlacedViewRetirementError<'extent> {
    view: PlacedView<'extent>,
    diagnostic: AccessPlanDiagnostic,
}

impl<'extent> PlacedViewRetirementError<'extent> {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (PlacedView<'extent>, AccessPlanDiagnostic) {
        (self.view, self.diagnostic)
    }
}

/// Private lifetime witness for the exact authority that justified a placed
/// access. Owned Stable access retains the whole established carrier rather
/// than reducing provider content custody to a bare Extent reference.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
enum PlacementAuthorityRef<'view, 'extent> {
    Borrowed(&'view PlacedView<'extent>),
    CorrespondedBorrowed(&'view SchemaCorrespondedPlacedView<'extent>),
    BorrowedResident(&'view EstablishedBorrowedResidentPlacement<'extent>),
    EstablishedOwned(&'view EstablishedOwnedPlacement),
}

impl<'view, 'extent> PlacementAuthorityRef<'view, 'extent> {
    const fn base(self) -> u64 {
        match self {
            Self::Borrowed(view) => view.loan.base(),
            Self::CorrespondedBorrowed(view) => view.view().loan.base(),
            Self::BorrowedResident(established) => established.base(),
            Self::EstablishedOwned(established) => established.extent().base(),
        }
    }

    const fn placement_plan(self) -> &'view ValidatedPlacementPlan {
        match self {
            Self::Borrowed(view) => &view.plan,
            Self::CorrespondedBorrowed(view) => &view.view().plan,
            Self::BorrowedResident(established) => established.placement_plan(),
            Self::EstablishedOwned(established) => established.placement_plan(),
        }
    }

    const fn profile_receipt(self) -> ResourceProfileReceiptId {
        match self {
            Self::Borrowed(view) => view.profile_receipt,
            Self::CorrespondedBorrowed(view) => view.view().profile_receipt,
            Self::BorrowedResident(established) => established.profile_receipt(),
            Self::EstablishedOwned(established) => established.profile_receipt(),
        }
    }

    const fn profile(self) -> &'view AdmittedResourceProfile {
        match self {
            Self::Borrowed(view) => &view.profile,
            Self::CorrespondedBorrowed(view) => &view.view().profile,
            Self::BorrowedResident(established) => established.profile(),
            Self::EstablishedOwned(established) => &established.admission.profile,
        }
    }

    fn replay_resources(self) -> Result<PlacementResourceCompatibility, AccessPlanDiagnostic> {
        match self {
            Self::Borrowed(view) => {
                validate_placement_admission(&view.loan, &view.plan, &view.profile)
            }
            Self::CorrespondedBorrowed(view) => validate_placement_admission(
                &view.view().loan,
                &view.view().plan,
                &view.view().profile,
            ),
            Self::BorrowedResident(established) => validate_placement_admission(
                established.loan(),
                established.placement_plan(),
                established.profile(),
            ),
            Self::EstablishedOwned(established) => {
                replay_owned_admission_resources(&established.admission)
            }
        }
    }

    fn replay_resident_content(self, transition: &str) -> Result<(), AccessPlanDiagnostic> {
        let replay = match self {
            Self::Borrowed(_) | Self::CorrespondedBorrowed(_) => return Ok(()),
            Self::BorrowedResident(established) => validate_provider_content_binding(
                established.placement_plan(),
                established.loan(),
                established.content(),
            ),
            Self::EstablishedOwned(established) => {
                validate_owned_content_binding(&established.admission, &established.content)
            }
        };
        replay.map_err(|diagnostic| {
            AccessPlanDiagnostic(format!(
                "{transition} could not replay the retained resident content grant: {diagnostic}"
            ))
        })
    }

    fn replay_correspondence(self, transition: &str) -> Result<(), AccessPlanDiagnostic> {
        match self {
            Self::CorrespondedBorrowed(view) => view.validate_correspondence().map_err(|diagnostic| {
                AccessPlanDiagnostic(format!(
                    "{transition} could not replay the retained schema/device correspondence: {diagnostic}"
                ))
            }),
            _ => Ok(()),
        }
    }

    const fn correspondence(self) -> Option<&'view AdmittedSchemaDeviceCorrespondence> {
        match self {
            Self::CorrespondedBorrowed(view) => Some(view.correspondence()),
            _ => None,
        }
    }

    const fn resources(self) -> &'view PlacementResourceCompatibility {
        match self {
            Self::Borrowed(view) => &view.resources,
            Self::CorrespondedBorrowed(view) => &view.view().resources,
            Self::BorrowedResident(established) => established.resources(),
            Self::EstablishedOwned(established) => established.resources(),
        }
    }

    const fn admission(self) -> PlacementAdmissionId {
        match self {
            Self::Borrowed(view) => view.admission,
            Self::CorrespondedBorrowed(view) => view.view().admission,
            Self::BorrowedResident(established) => established.admission(),
            Self::EstablishedOwned(established) => established.admission(),
        }
    }

    const fn source_loan(self) -> BorrowPolarity {
        let polarity = match self {
            Self::Borrowed(view) => view.loan.polarity(),
            Self::CorrespondedBorrowed(view) => view.view().loan.polarity(),
            Self::BorrowedResident(established) => established.loan_polarity(),
            Self::EstablishedOwned(_) => LoanPolarity::Exclusive,
        };
        match polarity {
            LoanPolarity::Shared => BorrowPolarity::Shared,
            LoanPolarity::Exclusive => BorrowPolarity::Exclusive,
        }
    }

    const fn resident_claim(self) -> Option<ResidentClaimId> {
        match self {
            Self::Borrowed(_) | Self::CorrespondedBorrowed(_) => None,
            Self::BorrowedResident(established) => Some(established.resident_claim()),
            Self::EstablishedOwned(established) => Some(established.resident_claim()),
        }
    }

    const fn placed_occurrence(self) -> Option<PlacedOccurrenceId> {
        match self {
            Self::Borrowed(_) | Self::CorrespondedBorrowed(_) => None,
            Self::BorrowedResident(established) => Some(established.occurrence()),
            Self::EstablishedOwned(established) => Some(established.occurrence),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn project_placed_field<'view, 'extent>(
    plan: &ValidatedPlacementPlan,
    profile_receipt: ResourceProfileReceiptId,
    resources: &PlacementResourceCompatibility,
    admission: PlacementAdmissionId,
    base: u64,
    key: AccessFieldKey,
    current_borrow: BorrowPolarity,
    source_loan: BorrowPolarity,
    required_observation: Option<ObservationModel>,
    authority: PlacementAuthorityRef<'view, 'extent>,
) -> Result<PlacedFieldProjection<'view, 'extent>, AccessPlanDiagnostic> {
    if authority.placement_plan() != plan
        || authority.profile_receipt() != profile_receipt
        || authority.resources() != resources
        || authority.admission() != admission
        || authority.base() != base
        || authority.source_loan() != source_loan
    {
        return Err(AccessPlanDiagnostic(
            "placed field projection arguments do not match the retained placement authority"
                .into(),
        ));
    }
    if authority.profile().receipt() != profile_receipt {
        return Err(AccessPlanDiagnostic(
            "placed field projection profile receipt differs from its retained admitted profile"
                .into(),
        ));
    }
    let replayed_resources = authority.replay_resources().map_err(|diagnostic| {
        AccessPlanDiagnostic(format!(
            "placed field projection could not replay the retained placement authority: {diagnostic}"
        ))
    })?;
    if &replayed_resources != resources {
        return Err(AccessPlanDiagnostic(
            "placed field projection replayed resource compatibility differs from the retained authority"
                .into(),
        ));
    }
    authority.replay_correspondence("placed field projection")?;
    authority.replay_resident_content("placed field projection")?;
    let descriptor = plan.access.field_descriptor(key).cloned().ok_or_else(|| {
        AccessPlanDiagnostic(format!(
            "field key in canonical slot {} does not expose a placed accessor",
            key.slot()
        ))
    })?;
    if required_observation.is_some_and(|required| descriptor.observation() != required) {
        return Err(AccessPlanDiagnostic(format!(
            "field `{}` observation is not valid for established owned Stable access",
            descriptor.field()
        )));
    }
    let supply = resources.field(key).ok_or_else(|| {
        AccessPlanDiagnostic(format!(
            "field `{}` has no sealed resource compatibility",
            descriptor.field()
        ))
    })?;
    let primitive_address = base
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
        plan: plan.identity(),
        profile_receipt,
        supply: supply.clone(),
        reach: plan.reach.clone(),
        admission,
        resident_claim: authority.resident_claim(),
        placed_occurrence: authority.placed_occurrence(),
        _authority: authority,
    })
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
    resident_claim: Option<ResidentClaimId>,
    placed_occurrence: Option<PlacedOccurrenceId>,
    _authority: PlacementAuthorityRef<'view, 'extent>,
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

    pub const fn resident_claim(&self) -> Option<ResidentClaimId> {
        self.resident_claim
    }

    pub const fn placed_occurrence(&self) -> Option<PlacedOccurrenceId> {
        self.placed_occurrence
    }

    pub const fn correspondence(&self) -> Option<&AdmittedSchemaDeviceCorrespondence> {
        self._authority.correspondence()
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

    fn validate_authority_binding(&self) -> Result<(), AccessPlanDiagnostic> {
        let authority = self._authority;
        let placement = authority.placement_plan();
        if placement.identity() != self.plan
            || authority.profile_receipt() != self.profile_receipt
            || authority.profile().receipt() != self.profile_receipt
            || authority.admission() != self.admission
            || placement.reach() != &self.reach
            || authority.source_loan() != self.source_loan
            || authority.resident_claim() != self.resident_claim
            || authority.placed_occurrence() != self.placed_occurrence
        {
            return Err(AccessPlanDiagnostic(
                "placed field authorization requires copied placement, profile, admission, reach, loan, and resident identities to match the retained authority"
                    .into(),
            ));
        }

        let replayed_resources = authority.replay_resources().map_err(|diagnostic| {
            AccessPlanDiagnostic(format!(
                "placed field authorization could not replay the retained placement authority: {diagnostic}"
            ))
        })?;
        if &replayed_resources != authority.resources()
            || authority.resources().placement != placement.identity()
            || authority.resources().field(self.descriptor.key()) != Some(&self.supply)
            || placement.access().field_descriptor(self.descriptor.key()) != Some(&self.descriptor)
        {
            return Err(AccessPlanDiagnostic(
                "placed field authorization requires the exact replayed resource row and descriptor from the retained placement authority"
                    .into(),
            ));
        }
        authority.replay_correspondence("placed field authorization")?;
        authority.replay_resident_content("placed field authorization")?;

        let descriptor_address = authority
            .base()
            .checked_add(self.descriptor.container_byte_offset())
            .ok_or_else(|| {
                AccessPlanDiagnostic(
                    "placed field authorization descriptor address overflows the retained authority base"
                        .into(),
                )
            })?;
        let supply_address = authority
            .base()
            .checked_add(self.supply.offset())
            .ok_or_else(|| {
                AccessPlanDiagnostic(
                    "placed field authorization supply address overflows the retained authority base"
                        .into(),
                )
            })?;
        if descriptor_address != self.primitive_address || supply_address != self.primitive_address
        {
            return Err(AccessPlanDiagnostic(
                "placed field authorization requires descriptor and supply geometry to reproduce the projected primitive address"
                    .into(),
            ));
        }
        Ok(())
    }

    fn authorize<'access>(
        &'access self,
        operation: AccessOperation,
    ) -> Result<PlacedFieldAccess<'access, 'extent>, AccessPlanDiagnostic> {
        self.validate_authority_binding()?;
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
            resident_claim: self.resident_claim,
            placed_occurrence: self.placed_occurrence,
            _authority: self._authority,
        })
    }
}

/// Sealed lowering input carrying both authorized field geometry and the
/// exact borrowed or established-owned authority from which its polarity was
/// derived.
#[derive(Debug)]
pub struct PlacedFieldAccess<'view, 'extent> {
    access: AuthorizedFieldAccess,
    primitive_address: u64,
    plan: PlacementPlanId,
    profile_receipt: ResourceProfileReceiptId,
    supply: EffectiveFieldSupply,
    reach: BoundaryReach,
    admission: PlacementAdmissionId,
    resident_claim: Option<ResidentClaimId>,
    placed_occurrence: Option<PlacedOccurrenceId>,
    _authority: PlacementAuthorityRef<'view, 'extent>,
}

impl<'view, 'extent> PlacedFieldAccess<'view, 'extent> {
    pub const fn access(&self) -> &AuthorizedFieldAccess {
        &self.access
    }

    pub const fn primitive_address(&self) -> u64 {
        self.primitive_address
    }

    pub const fn resident_claim(&self) -> Option<ResidentClaimId> {
        self.resident_claim
    }

    pub const fn placed_occurrence(&self) -> Option<PlacedOccurrenceId> {
        self.placed_occurrence
    }

    pub const fn correspondence(&self) -> Option<&AdmittedSchemaDeviceCorrespondence> {
        self._authority.correspondence()
    }

    /// Consume one authorized access event into the only request primitive
    /// lowering accepts.
    ///
    /// The request remains bound to the normalized plan, exact admission and
    /// source loan, address, width, observation model, operation ordering, and
    /// static service reach that produced it. It contains no author-supplied
    /// offset.
    pub fn into_primitive_request(self) -> PrimitiveAccessRequest<'view, 'extent> {
        let descriptor = self.access.descriptor.clone();
        PrimitiveAccessRequest {
            plan: self.plan,
            profile_receipt: self.profile_receipt,
            effective_supply: self.supply,
            admission: self.admission,
            primitive_address: self.primitive_address,
            key: descriptor.key,
            field: descriptor.field.clone(),
            transfer_width_bits: descriptor.transfer_width_bits,
            logical_extent: descriptor.logical_extent.clone(),
            effect_footprint: EffectFootprint {
                address: self.primitive_address,
                length_bytes: descriptor.effect_footprint.length_bytes,
            },
            observation: descriptor.observation,
            current_borrow: self.access.current_borrow,
            source_loan: self.access.source_loan,
            operation: self.access.operation,
            reach: self.reach,
            resident_claim: self.resident_claim,
            placed_occurrence: self.placed_occurrence,
            descriptor,
            authorization: self.access,
            _authority: self._authority,
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
    key: AccessFieldKey,
    field: String,
    transfer_width_bits: u16,
    logical_extent: LogicalFieldExtent,
    effect_footprint: EffectFootprint,
    observation: ObservationModel,
    current_borrow: BorrowPolarity,
    source_loan: BorrowPolarity,
    operation: AccessOperation,
    reach: BoundaryReach,
    resident_claim: Option<ResidentClaimId>,
    placed_occurrence: Option<PlacedOccurrenceId>,
    descriptor: FieldAccessDescriptor,
    authorization: AuthorizedFieldAccess,
    _authority: PlacementAuthorityRef<'view, 'extent>,
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

    pub const fn logical_extent(&self) -> &LogicalFieldExtent {
        &self.logical_extent
    }

    pub const fn effect_footprint(&self) -> EffectFootprint {
        self.effect_footprint
    }

    /// Whether this event and another event require mutually exclusive effect
    /// footprints.
    ///
    /// Repeatable reads may share an overlapping transfer container. Atomic
    /// events may share only the same exact atomic container; a partial or
    /// mixed-width overlap rejects. Every destructive read, ordinary write,
    /// or stable read-modify-write reserves its complete transfer footprint.
    pub const fn conflicts_with(&self, other: &Self) -> bool {
        effect_footprints_conflict(
            self.effect_footprint,
            self.operation,
            other.effect_footprint,
            other.operation,
        )
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

    pub const fn resident_claim(&self) -> Option<ResidentClaimId> {
        self.resident_claim
    }

    pub const fn placed_occurrence(&self) -> Option<PlacedOccurrenceId> {
        self.placed_occurrence
    }

    pub const fn correspondence(&self) -> Option<&AdmittedSchemaDeviceCorrespondence> {
        self._authority.correspondence()
    }

    fn validate_effective_supply_binding(&self) -> Result<(), AccessPlanDiagnostic> {
        if self.effective_supply.key != self.key
            || self.effective_supply.field != self.field
            || self.effective_supply.width_bits != self.transfer_width_bits
        {
            return Err(AccessPlanDiagnostic(
                "primitive lowering requires the admitted supply key and width to match the sealed request, including its field identity"
                    .into(),
            ));
        }

        let expected_address = self
            ._authority
            .base()
            .checked_add(self.effective_supply.offset)
            .ok_or_else(|| {
                AccessPlanDiagnostic(
                    "primitive lowering supply offset overflows the retained authority base".into(),
                )
            })?;
        if expected_address != self.primitive_address {
            return Err(AccessPlanDiagnostic(
                "primitive lowering requires the admitted supply offset to reproduce the sealed primitive address"
                    .into(),
            ));
        }

        let alignment = self.effective_supply.alignment_bytes;
        if alignment == 0 || !alignment.is_power_of_two() || self.primitive_address % alignment != 0
        {
            return Err(AccessPlanDiagnostic(
                "primitive lowering requires the admitted supply alignment to hold at the sealed primitive address"
                    .into(),
            ));
        }
        Ok(())
    }

    fn validate_descriptor_binding(&self) -> Result<(), AccessPlanDiagnostic> {
        if self.descriptor.key != self.key
            || self.descriptor.field != self.field
            || self.descriptor.container_byte_offset != self.effective_supply.offset
            || self.descriptor.transfer_width_bits != self.transfer_width_bits
            || self.descriptor.logical_extent != self.logical_extent
            || self.descriptor.observation != self.observation
        {
            return Err(AccessPlanDiagnostic(
                "primitive lowering requires the copied request facts to match the retained validated field descriptor"
                    .into(),
            ));
        }

        let expected_footprint_address = self
            ._authority
            .base()
            .checked_add(self.descriptor.effect_footprint.byte_offset)
            .ok_or_else(|| {
                AccessPlanDiagnostic(
                    "primitive lowering descriptor footprint overflows the retained authority base"
                        .into(),
                )
            })?;
        if self.effect_footprint.address != expected_footprint_address
            || self.effect_footprint.length_bytes != self.descriptor.effect_footprint.length_bytes
        {
            return Err(AccessPlanDiagnostic(
                "primitive lowering requires the concrete effect footprint to match the retained validated field descriptor"
                    .into(),
            ));
        }

        authorize_descriptor(
            &self.descriptor,
            self.current_borrow,
            self.source_loan,
            self.operation,
        )
    }

    fn validate_authority_binding(&self) -> Result<(), AccessPlanDiagnostic> {
        let authority = self._authority;
        let placement = authority.placement_plan();
        if placement.identity() != self.plan
            || authority.profile_receipt() != self.profile_receipt
            || authority.profile().receipt() != self.profile_receipt
            || authority.admission() != self.admission
            || placement.reach() != &self.reach
        {
            return Err(AccessPlanDiagnostic(
                "primitive lowering requires the copied plan, profile, admission, and reach to match the retained placement authority"
                    .into(),
            ));
        }

        if authority.resources().placement != placement.identity()
            || authority.resources().field(self.key) != Some(&self.effective_supply)
            || placement.access().field_descriptor(self.key) != Some(&self.descriptor)
        {
            return Err(AccessPlanDiagnostic(
                "primitive lowering requires the retained resource row and field descriptor to belong to the exact placement authority"
                    .into(),
            ));
        }

        let replayed_resources = authority.replay_resources().map_err(|diagnostic| {
            AccessPlanDiagnostic(format!(
                "primitive lowering could not replay the retained admitted resource profile: {diagnostic}"
            ))
        })?;
        if &replayed_resources != authority.resources() {
            return Err(AccessPlanDiagnostic(
                "primitive lowering replayed resource compatibility differs from the retained placement authority"
                    .into(),
            ));
        }
        authority.replay_correspondence("primitive lowering")?;
        authority.replay_resident_content("primitive lowering")?;

        if authority.source_loan() != self.source_loan
            || authority.resident_claim() != self.resident_claim
            || authority.placed_occurrence() != self.placed_occurrence
        {
            return Err(AccessPlanDiagnostic(
                "primitive lowering requires source-loan and resident identities to match the retained placement authority"
                    .into(),
            ));
        }
        Ok(())
    }

    fn validate_authorization_binding(&self) -> Result<(), AccessPlanDiagnostic> {
        if self.authorization.descriptor != self.descriptor
            || self.authorization.current_borrow != self.current_borrow
            || self.authorization.source_loan != self.source_loan
            || self.authorization.operation != self.operation
        {
            return Err(AccessPlanDiagnostic(
                "primitive lowering requires copied operation and borrow facts to match the retained field authorization"
                    .into(),
            ));
        }
        authorize_descriptor(
            &self.authorization.descriptor,
            self.authorization.current_borrow,
            self.authorization.source_loan,
            self.authorization.operation,
        )
    }
}

/// Operation subset accepted by ordinary Stable primitive lowering.
///
/// Compound mutation needs its distinct bounded read-patch-write realization;
/// External and atomic events retain their own transfer laws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StablePrimitiveOperation {
    Read,
    Write,
}

/// Stable-only consumer contract for a sealed primitive access.
///
/// The original request remains intact inside this carrier, retaining its
/// exact admission, profile, geometry, and lifetime authority. A future
/// interpreter or native execution binding adds its result/value operand and
/// target-owned storage realization to this already-specialized event.
#[derive(Debug)]
#[must_use = "Stable primitive access retains its exact placed authority"]
pub struct StablePrimitiveAccessRequest<'view, 'extent> {
    request: PrimitiveAccessRequest<'view, 'extent>,
    operation: StablePrimitiveOperation,
}

impl<'view, 'extent> StablePrimitiveAccessRequest<'view, 'extent> {
    pub const fn operation(&self) -> StablePrimitiveOperation {
        self.operation
    }

    pub const fn primitive_address(&self) -> u64 {
        self.request.primitive_address
    }

    pub const fn transfer_width_bits(&self) -> u16 {
        self.request.transfer_width_bits
    }

    pub const fn logical_extent(&self) -> &LogicalFieldExtent {
        &self.request.logical_extent
    }

    pub const fn effect_footprint(&self) -> EffectFootprint {
        self.request.effect_footprint
    }

    /// Retained physical correspondence, when the originating placed view
    /// was provider-corresponded. This borrows the exact admitted fact; it
    /// does not copy provider/device identities or require correspondence for
    /// ordinary Stable storage.
    pub const fn correspondence(&self) -> Option<&AdmittedSchemaDeviceCorrespondence> {
        self.request.correspondence()
    }

    /// Borrow the exact sealed primitive request retained by this
    /// specialization. Consumers may inspect its complete placement and
    /// lifetime authority but cannot reconstruct, mutate, or respecialize it.
    pub const fn primitive_request(&self) -> &PrimitiveAccessRequest<'view, 'extent> {
        &self.request
    }

    /// Independently replay the complete placed authority and Stable
    /// operation specialization before an outward lowering consumer accepts
    /// this request. Rejection only borrows the carrier, so its exact loan,
    /// resident content, and authorization remain available for corrected
    /// retry.
    pub fn validate_for_lowering(&self) -> Result<(), AccessPlanDiagnostic> {
        let operation = validate_stable_primitive_request(&self.request)?;
        if operation != self.operation {
            return Err(AccessPlanDiagnostic(
                "Stable primitive lowering operation differs from its retained specialization"
                    .into(),
            ));
        }
        Ok(())
    }

    pub fn into_primitive_request(self) -> PrimitiveAccessRequest<'view, 'extent> {
        self.request
    }
}

/// Failed specialization returns the exact sealed request so its authority
/// and content-custody lifetime remain available to the caller.
#[derive(Debug)]
pub struct StablePrimitiveAccessRejection<'view, 'extent> {
    request: PrimitiveAccessRequest<'view, 'extent>,
    diagnostic: AccessPlanDiagnostic,
}

impl<'view, 'extent> StablePrimitiveAccessRejection<'view, 'extent> {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (PrimitiveAccessRequest<'view, 'extent>, AccessPlanDiagnostic) {
        (self.request, self.diagnostic)
    }
}

impl<'view, 'extent> PrimitiveAccessRequest<'view, 'extent> {
    /// Consume this general request into the narrow contract accepted by
    /// ordinary Stable read/write lowering.
    pub fn into_stable_primitive_access(
        self,
    ) -> Result<
        StablePrimitiveAccessRequest<'view, 'extent>,
        StablePrimitiveAccessRejection<'view, 'extent>,
    > {
        let operation = match validate_stable_primitive_request(&self) {
            Ok(operation) => operation,
            Err(diagnostic) => {
                return Err(StablePrimitiveAccessRejection {
                    request: self,
                    diagnostic,
                });
            }
        };
        Ok(StablePrimitiveAccessRequest {
            request: self,
            operation,
        })
    }
}

fn validate_stable_primitive_request(
    request: &PrimitiveAccessRequest<'_, '_>,
) -> Result<StablePrimitiveOperation, AccessPlanDiagnostic> {
    if request.observation != ObservationModel::Stable {
        return Err(AccessPlanDiagnostic(
            "ordinary Stable lowering requires a Stable observation".into(),
        ));
    }
    if request.effective_supply.kind() != EffectiveSupplyKind::Stable {
        return Err(AccessPlanDiagnostic(
            "ordinary Stable lowering requires admitted Stable supply".into(),
        ));
    }
    let operation = match request.operation {
        AccessOperation::Read => StablePrimitiveOperation::Read,
        AccessOperation::Write => StablePrimitiveOperation::Write,
        _ => {
            return Err(AccessPlanDiagnostic(
                "ordinary Stable lowering accepts only one sealed Read or Write event".into(),
            ));
        }
    };
    request.validate_effective_supply_binding()?;
    request.validate_descriptor_binding()?;
    request.validate_authority_binding()?;
    request.validate_authorization_binding()?;
    Ok(operation)
}

/// Stable-only consumer contract for one bounded compound mutation.
///
/// This carrier remains distinct from an ordinary Stable read or write: its
/// consumer must realize one read-patch-write sequence over the complete
/// retained effect footprint without weakening either exclusive borrow.
#[derive(Debug)]
#[must_use = "Stable compound mutation retains its exact placed authority"]
pub struct StableCompoundMutationAccessRequest<'view, 'extent> {
    request: PrimitiveAccessRequest<'view, 'extent>,
}

impl<'view, 'extent> StableCompoundMutationAccessRequest<'view, 'extent> {
    pub const fn primitive_address(&self) -> u64 {
        self.request.primitive_address
    }

    pub const fn transfer_width_bits(&self) -> u16 {
        self.request.transfer_width_bits
    }

    pub const fn logical_extent(&self) -> &LogicalFieldExtent {
        &self.request.logical_extent
    }

    pub const fn effect_footprint(&self) -> EffectFootprint {
        self.request.effect_footprint
    }

    /// Retained physical correspondence, when present on the originating
    /// placed view. The bounded mutation specialization does not manufacture
    /// or require such evidence.
    pub const fn correspondence(&self) -> Option<&AdmittedSchemaDeviceCorrespondence> {
        self.request.correspondence()
    }

    /// Borrow the exact sealed primitive request retained by this bounded
    /// mutation specialization without weakening its exclusive authority.
    pub const fn primitive_request(&self) -> &PrimitiveAccessRequest<'view, 'extent> {
        &self.request
    }

    /// Independently replay the complete placed authority and bounded
    /// read-patch-write specialization before an outward lowering consumer
    /// accepts this request. Rejection only borrows the carrier, preserving
    /// its exact exclusive loan and resident-content custody for retry.
    pub fn validate_for_lowering(&self) -> Result<(), AccessPlanDiagnostic> {
        validate_stable_compound_mutation_request(&self.request)
    }

    pub fn into_primitive_request(self) -> PrimitiveAccessRequest<'view, 'extent> {
        self.request
    }
}

/// Failed compound specialization returns the exact sealed request so its
/// content-custody lifetime and exclusive authority remain available.
#[derive(Debug)]
pub struct StableCompoundMutationAccessRejection<'view, 'extent> {
    request: PrimitiveAccessRequest<'view, 'extent>,
    diagnostic: AccessPlanDiagnostic,
}

impl<'view, 'extent> StableCompoundMutationAccessRejection<'view, 'extent> {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (PrimitiveAccessRequest<'view, 'extent>, AccessPlanDiagnostic) {
        (self.request, self.diagnostic)
    }
}

impl<'view, 'extent> PrimitiveAccessRequest<'view, 'extent> {
    /// Consume this general request into the exact contract accepted by one
    /// bounded Stable read-patch-write realization.
    pub fn into_stable_compound_mutation_access(
        self,
    ) -> Result<
        StableCompoundMutationAccessRequest<'view, 'extent>,
        StableCompoundMutationAccessRejection<'view, 'extent>,
    > {
        if let Err(diagnostic) = validate_stable_compound_mutation_request(&self) {
            return Err(StableCompoundMutationAccessRejection {
                request: self,
                diagnostic,
            });
        }
        Ok(StableCompoundMutationAccessRequest { request: self })
    }
}

fn validate_stable_compound_mutation_request(
    request: &PrimitiveAccessRequest<'_, '_>,
) -> Result<(), AccessPlanDiagnostic> {
    if request.observation != ObservationModel::Stable {
        return Err(AccessPlanDiagnostic(
            "Stable compound mutation requires a Stable observation".into(),
        ));
    }
    if request.effective_supply.kind() != EffectiveSupplyKind::Stable {
        return Err(AccessPlanDiagnostic(
            "Stable compound mutation requires admitted Stable supply".into(),
        ));
    }
    request.validate_effective_supply_binding()?;
    if request.current_borrow != BorrowPolarity::Exclusive
        || request.source_loan != BorrowPolarity::Exclusive
    {
        return Err(AccessPlanDiagnostic(
            "Stable compound mutation requires exclusive current and source borrows".into(),
        ));
    }
    if request.operation != AccessOperation::CompoundMutation {
        return Err(AccessPlanDiagnostic(
            "Stable compound lowering accepts only one sealed CompoundMutation event".into(),
        ));
    }
    request.validate_descriptor_binding()?;
    request.validate_authority_binding()?;
    request.validate_authorization_binding()
}

/// Operation subset accepted by one exact External primitive transfer.
///
/// Repeatable reads, destructive reads, and whole-container writes remain
/// distinct. External compound mutation and atomic operations have no member
/// in this closed lowering contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalPrimitiveOperation {
    Read,
    Take,
    Write,
}

/// External-only consumer contract for one sealed primitive access.
///
/// A conservative External demand may have been satisfied by admitted Stable
/// supply, but the requested observation remains External: lowering must still
/// emit one non-elided exact-width transfer. The original request remains
/// intact inside this linear carrier.
#[derive(Debug)]
#[must_use = "External primitive access retains its exact placed authority"]
pub struct ExternalPrimitiveAccessRequest<'view, 'extent> {
    request: PrimitiveAccessRequest<'view, 'extent>,
    operation: ExternalPrimitiveOperation,
}

impl<'view, 'extent> ExternalPrimitiveAccessRequest<'view, 'extent> {
    pub const fn operation(&self) -> ExternalPrimitiveOperation {
        self.operation
    }

    pub const fn primitive_address(&self) -> u64 {
        self.request.primitive_address
    }

    pub const fn transfer_width_bits(&self) -> u16 {
        self.request.transfer_width_bits
    }

    pub const fn logical_extent(&self) -> &LogicalFieldExtent {
        &self.request.logical_extent
    }

    pub const fn effect_footprint(&self) -> EffectFootprint {
        self.request.effect_footprint
    }

    /// Retained physical correspondence, when the originating placed view
    /// was provider-corresponded. This remains distinct from External supply
    /// compatibility and establishes no device operation.
    pub const fn correspondence(&self) -> Option<&AdmittedSchemaDeviceCorrespondence> {
        self.request.correspondence()
    }

    /// Borrow the exact sealed primitive request retained by this External
    /// specialization. The borrow exposes provenance for a later consumer but
    /// establishes no transfer or device operation.
    pub const fn primitive_request(&self) -> &PrimitiveAccessRequest<'view, 'extent> {
        &self.request
    }

    /// Independently replay the complete placed authority, admitted supply
    /// substitution, and exact External operation before an outward lowering
    /// consumer accepts this request. Rejection only borrows the carrier, so
    /// no external event occurs and the same request remains available for
    /// corrected retry.
    pub fn validate_for_lowering(&self) -> Result<(), AccessPlanDiagnostic> {
        let operation = validate_external_primitive_request(&self.request)?;
        if operation != self.operation {
            return Err(AccessPlanDiagnostic(
                "External primitive lowering operation differs from its retained specialization"
                    .into(),
            ));
        }
        Ok(())
    }

    pub fn into_primitive_request(self) -> PrimitiveAccessRequest<'view, 'extent> {
        self.request
    }
}

/// Failed External specialization returns the exact sealed request so its
/// range authority and content-custody lifetime remain available to the
/// caller.
#[derive(Debug)]
pub struct ExternalPrimitiveAccessRejection<'view, 'extent> {
    request: PrimitiveAccessRequest<'view, 'extent>,
    diagnostic: AccessPlanDiagnostic,
}

impl<'view, 'extent> ExternalPrimitiveAccessRejection<'view, 'extent> {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (PrimitiveAccessRequest<'view, 'extent>, AccessPlanDiagnostic) {
        (self.request, self.diagnostic)
    }
}

impl<'view, 'extent> PrimitiveAccessRequest<'view, 'extent> {
    /// Consume this general request into the narrow contract accepted by one
    /// exact External read, destructive read, or whole-container write.
    pub fn into_external_primitive_access(
        self,
    ) -> Result<
        ExternalPrimitiveAccessRequest<'view, 'extent>,
        ExternalPrimitiveAccessRejection<'view, 'extent>,
    > {
        let operation = match validate_external_primitive_request(&self) {
            Ok(operation) => operation,
            Err(diagnostic) => {
                return Err(ExternalPrimitiveAccessRejection {
                    request: self,
                    diagnostic,
                });
            }
        };
        Ok(ExternalPrimitiveAccessRequest {
            request: self,
            operation,
        })
    }
}

fn validate_external_primitive_request(
    request: &PrimitiveAccessRequest<'_, '_>,
) -> Result<ExternalPrimitiveOperation, AccessPlanDiagnostic> {
    if request.observation != ObservationModel::External {
        return Err(AccessPlanDiagnostic(
            "External lowering requires an External observation".into(),
        ));
    }
    let operation = match request.operation {
        AccessOperation::Read => ExternalPrimitiveOperation::Read,
        AccessOperation::Take => ExternalPrimitiveOperation::Take,
        AccessOperation::Write => ExternalPrimitiveOperation::Write,
        AccessOperation::CompoundMutation | AccessOperation::Atomic(_) => {
            return Err(AccessPlanDiagnostic(
                "External lowering accepts only one sealed Read, Take, or Write event".into(),
            ));
        }
    };
    let supply_is_compatible = match request.effective_supply.kind() {
        EffectiveSupplyKind::External => true,
        EffectiveSupplyKind::Stable => matches!(
            operation,
            ExternalPrimitiveOperation::Read | ExternalPrimitiveOperation::Write
        ),
        EffectiveSupplyKind::Atomic => false,
    };
    if !supply_is_compatible {
        return Err(AccessPlanDiagnostic(
            "External lowering requires admitted External supply, or conservative Stable supply for Read or Write"
                .into(),
        ));
    }
    request.validate_effective_supply_binding()?;
    request.validate_descriptor_binding()?;
    request.validate_authority_binding()?;
    request.validate_authorization_binding()?;
    Ok(operation)
}

/// Atomic operation and proof-static ordering accepted by primitive lowering.
///
/// Each family remains distinct, including the independent success and
/// failure orderings of compare-exchange. No ordinary read, write, or
/// synthesized retry operation has a member in this closed contract.
#[derive(Debug)]
#[must_use = "Atomic primitive access retains its exact placed authority"]
pub struct AtomicPrimitiveAccessRequest<'view, 'extent> {
    request: PrimitiveAccessRequest<'view, 'extent>,
    operation: AtomicAccessOperation,
}

impl<'view, 'extent> AtomicPrimitiveAccessRequest<'view, 'extent> {
    pub const fn operation(&self) -> AtomicAccessOperation {
        self.operation
    }

    pub const fn ordering_plan(&self) -> AtomicOrderingPlan {
        self.operation.ordering_plan()
    }

    pub const fn primitive_address(&self) -> u64 {
        self.request.primitive_address
    }

    pub const fn transfer_width_bits(&self) -> u16 {
        self.request.transfer_width_bits
    }

    pub const fn logical_extent(&self) -> &LogicalFieldExtent {
        &self.request.logical_extent
    }

    pub const fn effect_footprint(&self) -> EffectFootprint {
        self.request.effect_footprint
    }

    /// Retained physical correspondence, when present on the originating
    /// placed view. Atomic specialization neither manufactures nor requires
    /// this separate provider-issued fact.
    pub const fn correspondence(&self) -> Option<&AdmittedSchemaDeviceCorrespondence> {
        self.request.correspondence()
    }

    /// Borrow the exact sealed primitive request retained by this Atomic
    /// specialization without weakening its operation or ordering identity.
    pub const fn primitive_request(&self) -> &PrimitiveAccessRequest<'view, 'extent> {
        &self.request
    }

    /// Independently replay the complete placed authority, admitted Atomic
    /// supply, operation family, and ordering law before an outward lowering
    /// consumer accepts this request. Rejection only borrows the carrier; it
    /// performs no atomic attempt and preserves the same request for retry.
    pub fn validate_for_lowering(&self) -> Result<(), AccessPlanDiagnostic> {
        let operation = validate_atomic_primitive_request(&self.request)?;
        if operation != self.operation {
            return Err(AccessPlanDiagnostic(
                "Atomic primitive lowering operation differs from its retained specialization"
                    .into(),
            ));
        }
        Ok(())
    }

    pub fn into_primitive_request(self) -> PrimitiveAccessRequest<'view, 'extent> {
        self.request
    }
}

/// Failed Atomic specialization returns the exact sealed request so its range
/// authority and operation-specific custody remain available to the caller.
#[derive(Debug)]
pub struct AtomicPrimitiveAccessRejection<'view, 'extent> {
    request: PrimitiveAccessRequest<'view, 'extent>,
    diagnostic: AccessPlanDiagnostic,
}

impl<'view, 'extent> AtomicPrimitiveAccessRejection<'view, 'extent> {
    pub const fn diagnostic(&self) -> &AccessPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (PrimitiveAccessRequest<'view, 'extent>, AccessPlanDiagnostic) {
        (self.request, self.diagnostic)
    }
}

impl<'view, 'extent> PrimitiveAccessRequest<'view, 'extent> {
    /// Consume this general request into the narrow contract accepted by one
    /// exact, explicitly admitted Atomic operation.
    pub fn into_atomic_primitive_access(
        self,
    ) -> Result<
        AtomicPrimitiveAccessRequest<'view, 'extent>,
        AtomicPrimitiveAccessRejection<'view, 'extent>,
    > {
        let operation = match validate_atomic_primitive_request(&self) {
            Ok(operation) => operation,
            Err(diagnostic) => {
                return Err(AtomicPrimitiveAccessRejection {
                    request: self,
                    diagnostic,
                });
            }
        };
        Ok(AtomicPrimitiveAccessRequest {
            request: self,
            operation,
        })
    }
}

fn validate_atomic_primitive_request(
    request: &PrimitiveAccessRequest<'_, '_>,
) -> Result<AtomicAccessOperation, AccessPlanDiagnostic> {
    if request.observation != ObservationModel::Atomic {
        return Err(AccessPlanDiagnostic(
            "Atomic lowering requires an Atomic observation".into(),
        ));
    }
    if request.effective_supply.kind() != EffectiveSupplyKind::Atomic {
        return Err(AccessPlanDiagnostic(
            "Atomic lowering requires explicitly admitted Atomic supply".into(),
        ));
    }
    request.validate_effective_supply_binding()?;
    let AccessOperation::Atomic(operation) = request.operation else {
        return Err(AccessPlanDiagnostic(
            "Atomic lowering accepts only one sealed Atomic operation".into(),
        ));
    };
    validate_operation_ordering(request.operation)?;
    request.validate_descriptor_binding()?;
    request.validate_authority_binding()?;
    request.validate_authorization_binding()?;
    Ok(operation)
}

pub const fn effect_footprints_conflict(
    left: EffectFootprint,
    left_operation: AccessOperation,
    right: EffectFootprint,
    right_operation: AccessOperation,
) -> bool {
    if !left.overlaps(right) {
        return false;
    }
    match (left_operation, right_operation) {
        (AccessOperation::Read, AccessOperation::Read) => false,
        (AccessOperation::Atomic(_), AccessOperation::Atomic(_)) => {
            left.address != right.address || left.length_bytes != right.length_bytes
        }
        _ => true,
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
            profile: profile.clone(),
            resources,
            loan,
        }),
        Err(diagnostic) => Err(PlacementRejection { loan, diagnostic }),
    }
}

/// Admit one complete owned Extent without manufacturing an owned loan or a
/// second authority root.
///
/// Validation borrows the full range only for the duration of the check. The
/// accepted carrier then retains the original Extent; rejection returns that
/// same value with its sealed origin and lineage unchanged.
pub fn admit_owned_placement(
    identity: PlacementAdmissionId,
    extent: Extent,
    plan: &ValidatedPlacementPlan,
    profile: &AdmittedResourceProfile,
) -> Result<OwnedPlacementAdmission, OwnedPlacementRejection> {
    let validation = match extent.loan(0, extent.length()) {
        Ok(loan) => validate_placement_admission(&loan, plan, profile),
        Err(diagnostic) => Err(AccessPlanDiagnostic(format!(
            "owned extent could not produce its internal whole-range loan: {diagnostic}"
        ))),
    };
    match validation {
        Ok(resources) => Ok(OwnedPlacementAdmission {
            identity,
            placement_plan: plan.clone(),
            profile_receipt: profile.receipt,
            profile: profile.clone(),
            resources,
            extent,
        }),
        Err(diagnostic) => Err(OwnedPlacementRejection { extent, diagnostic }),
    }
}

/// Establish provider-validated existing content through the Stable adoption
/// route.
///
/// The content grant was minted only while the corresponding provider root
/// authority was consumed. Adoption independently binds its admitted
/// interpretation to the actual normalized placement and rejects any drift in
/// origin, lineage, or geometry. External and Atomic observations use their
/// own future adoption routes and cannot pass through this Stable transition.
pub fn adopt_owned_stable(
    admission: OwnedPlacementAdmission,
    content: ProviderExistingContentGrant,
) -> Result<DormantOwnedResident, OwnedStableAdoptionError> {
    let diagnostic = validate_owned_stable_adoption(&admission, &content);
    if let Err(diagnostic) = diagnostic {
        return Err(OwnedStableAdoptionError {
            admission,
            content,
            diagnostic,
        });
    }
    Ok(DormantOwnedResident { admission, content })
}

fn validate_owned_stable_adoption(
    admission: &OwnedPlacementAdmission,
    content: &ProviderExistingContentGrant,
) -> Result<(), AccessPlanDiagnostic> {
    let replayed_resources = replay_owned_admission_resources(admission).map_err(|diagnostic| {
        AccessPlanDiagnostic(format!(
            "Stable adoption could not replay the admitted resource profile: {diagnostic}"
        ))
    })?;
    if replayed_resources != admission.resources {
        return Err(AccessPlanDiagnostic(
            "Stable adoption replayed resource compatibility differs from the owned admission"
                .into(),
        ));
    }

    validate_owned_content_binding(admission, content)
}

fn validate_owned_content_binding(
    admission: &OwnedPlacementAdmission,
    content: &ProviderExistingContentGrant,
) -> Result<(), AccessPlanDiagnostic> {
    let extent = &admission.extent;
    let loan = extent.loan(0, extent.length()).map_err(|diagnostic| {
        AccessPlanDiagnostic(format!(
            "owned content binding could not replay its whole-range loan: {diagnostic}"
        ))
    })?;
    validate_provider_content_binding(&admission.placement_plan, &loan, content)
}

fn validate_provider_content_binding(
    plan: &ValidatedPlacementPlan,
    loan: &ExtentLoan<'_>,
    content: &ProviderExistingContentGrant,
) -> Result<(), AccessPlanDiagnostic> {
    if content.interpretation().normalized_identity() != plan.identity().normalized_identity() {
        return Err(AccessPlanDiagnostic(
            "provider existing-content interpretation does not match the admitted placement".into(),
        ));
    }
    if content.origin() != loan.origin() {
        return Err(AccessPlanDiagnostic(
            "provider existing-content origin does not match the admitted Extent".into(),
        ));
    }
    if content.lineage_root() != loan.lineage_root() {
        return Err(AccessPlanDiagnostic(
            "provider existing-content lineage does not match the admitted Extent".into(),
        ));
    }
    if content.base() != loan.base() || content.length() != loan.length() {
        return Err(AccessPlanDiagnostic(
            "provider existing-content geometry does not match the admitted Extent".into(),
        ));
    }
    if content.address_space() != loan.address_space() {
        return Err(AccessPlanDiagnostic(
            "provider existing-content address space does not match the admitted Extent".into(),
        ));
    }
    if content.provenance() != loan.provenance() {
        return Err(AccessPlanDiagnostic(
            "provider existing-content provenance does not match the admitted Extent".into(),
        ));
    }
    if content.era() != loan.era() {
        return Err(AccessPlanDiagnostic(
            "provider existing-content mapping era does not match the admitted Extent".into(),
        ));
    }
    if let Some(descriptor) = plan
        .access()
        .field_descriptors()
        .iter()
        .find(|descriptor| descriptor.observation() != ObservationModel::Stable)
    {
        return Err(AccessPlanDiagnostic(format!(
            "field `{}` uses {:?} observation and cannot enter the Stable adoption route",
            descriptor.field(),
            descriptor.observation()
        )));
    }
    Ok(())
}

fn replay_owned_admission_resources(
    admission: &OwnedPlacementAdmission,
) -> Result<PlacementResourceCompatibility, AccessPlanDiagnostic> {
    if admission.profile.receipt() != admission.profile_receipt {
        return Err(AccessPlanDiagnostic(
            "owned placement profile receipt differs from its retained admitted profile".into(),
        ));
    }
    let extent = &admission.extent;
    let loan = extent.loan(0, extent.length()).map_err(|diagnostic| {
        AccessPlanDiagnostic(format!(
            "owned placement could not replay its whole-range loan: {diagnostic}"
        ))
    })?;
    validate_placement_admission(&loan, &admission.placement_plan, &admission.profile)
}

fn validate_owned_resident_authority(
    admission: &OwnedPlacementAdmission,
    content: &ProviderExistingContentGrant,
    transition: &str,
) -> Result<(), AccessPlanDiagnostic> {
    let resources = replay_owned_admission_resources(admission).map_err(|diagnostic| {
        AccessPlanDiagnostic(format!(
            "{transition} could not replay the retained placement authority: {diagnostic}"
        ))
    })?;
    if resources != admission.resources {
        return Err(AccessPlanDiagnostic(format!(
            "{transition} replayed resource compatibility differs from the retained admission"
        )));
    }
    validate_owned_content_binding(admission, content).map_err(|diagnostic| {
        AccessPlanDiagnostic(format!(
            "{transition} could not replay the retained provider content grant: {diagnostic}"
        ))
    })?;
    Ok(())
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

/// Establish one borrowed placed view only after independently replaying the
/// retained placement, admitted profile, and exact resource compatibility.
/// Rejection returns the complete loan-bearing admission unchanged.
pub fn place<'extent>(
    admission: PlacementAdmission<'extent>,
) -> Result<PlacedView<'extent>, PlaceEstablishmentError<'extent>> {
    let diagnostic = if admission.profile.receipt() != admission.profile_receipt {
        Some(AccessPlanDiagnostic(
            "borrowed placement profile receipt differs from its retained admitted profile".into(),
        ))
    } else {
        match validate_placement_admission(
            &admission.loan,
            &admission.placement_plan,
            &admission.profile,
        ) {
            Ok(resources) if resources == admission.resources => None,
            Ok(_) => Some(AccessPlanDiagnostic(
                "borrowed placement replayed resource compatibility differs from the retained admission"
                    .into(),
            )),
            Err(diagnostic) => Some(AccessPlanDiagnostic(format!(
                "borrowed placed-view establishment could not replay the admitted resource profile: {diagnostic}"
            ))),
        }
    };
    if let Some(diagnostic) = diagnostic {
        return Err(PlaceEstablishmentError {
            admission,
            diagnostic,
        });
    }
    Ok(PlacedView {
        loan: admission.loan,
        plan: admission.placement_plan,
        profile_receipt: admission.profile_receipt,
        profile: admission.profile,
        resources: admission.resources,
        admission: admission.identity,
    })
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
) -> Result<(u64, LogicalFieldExtent, RelativeEffectFootprint), AccessPlanDiagnostic> {
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
            Ok((
                offset,
                LogicalFieldExtent {
                    fragments: vec![LogicalFieldFragment {
                        layout_bit_offset: offset * 8,
                        source_bit_offset: 0,
                        width_bits: u64::from(transfer_width_bits),
                    }],
                },
                RelativeEffectFootprint {
                    byte_offset: offset,
                    length_bytes: transfer_bytes,
                },
            ))
        }
        [
            LayoutPlacementReport::IntegerAt {
                offset,
                stored_width,
                ..
            },
        ] => {
            if *stored_width != u64::from(transfer_width_bits) {
                return Err(AccessPlanDiagnostic(format!(
                    "access field `{field}` requests a {transfer_width_bits}-bit transfer over a {stored_width}-bit stored integer"
                )));
            }
            let offset = *offset;
            validate_transfer_range(field, offset, transfer_bytes, layout_size)?;
            Ok((
                offset,
                LogicalFieldExtent {
                    fragments: vec![LogicalFieldFragment {
                        layout_bit_offset: offset * 8,
                        source_bit_offset: 0,
                        width_bits: *stored_width,
                    }],
                },
                RelativeEffectFootprint {
                    byte_offset: offset,
                    length_bytes: transfer_bytes,
                },
            ))
        }
        placements => {
            let mut container = None;
            let mut fragments = Vec::with_capacity(placements.len());
            for placement in placements {
                let LayoutPlacementReport::Bits {
                    container: candidate,
                    container_width,
                    destination_lsb,
                    source_lsb,
                    width,
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
                fragments.push(LogicalFieldFragment {
                    layout_bit_offset: candidate * 8 + destination_lsb,
                    source_bit_offset: *source_lsb,
                    width_bits: *width,
                });
            }
            let container = container.expect("nonempty placements");
            validate_transfer_range(field, container, transfer_bytes, layout_size)?;
            fragments.sort_unstable_by_key(|fragment| {
                (
                    fragment.source_bit_offset,
                    fragment.layout_bit_offset,
                    fragment.width_bits,
                )
            });
            Ok((
                container,
                LogicalFieldExtent { fragments },
                RelativeEffectFootprint {
                    byte_offset: container,
                    length_bytes: transfer_bytes,
                },
            ))
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
mod tests;
