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
mod tests {
    use super::*;
    use psi_layout_plans::{
        IntegerInterpretation, LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport,
    };

    #[test]
    fn stored_integer_geometry_uses_the_exact_encoded_width() {
        let layout = LayoutPlanReport {
            schema_identity: 1,
            entries: vec![LayoutFieldEntryReport {
                field: "value".into(),
                member_identity: None,
                placement: LayoutPlacementReport::IntegerAt {
                    offset: 3,
                    stored_width: 16,
                    interpretation: IntegerInterpretation::Signed,
                },
            }],
            offsets: None,
            size: Some(8),
            align: 1,
        };
        let (offset, logical, effect) = validate_entry_geometry("value", 16, &layout, 8)
            .expect("the access transfer matches the stored integer width");
        assert_eq!(offset, 3);
        assert_eq!(logical.fragments[0].layout_bit_offset, 24);
        assert_eq!(logical.fragments[0].width_bits, 16);
        assert_eq!(effect.length_bytes, 2);
        let error = validate_entry_geometry("value", 32, &layout, 8)
            .expect_err("semantic carrier width is not the stored transfer width");
        assert!(
            error
                .0
                .contains("32-bit transfer over a 16-bit stored integer")
        );
    }

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
                        write: false,
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
    fn inaccessible_plan_rejects_one_name_for_multiple_field_identities() {
        let layout = LayoutPlanReport {
            schema_identity: 0x45,
            entries: vec![
                LayoutFieldEntryReport {
                    field: "word".into(),
                    member_identity: Some(7),
                    placement: LayoutPlacementReport::At { offset: 0 },
                },
                LayoutFieldEntryReport {
                    field: "word".into(),
                    member_identity: Some(8),
                    placement: LayoutPlacementReport::At { offset: 4 },
                },
            ],
            offsets: Some(vec![0, 4]),
            size: Some(8),
            align: 4,
        };

        let error = AccessPlan::inaccessible(&layout)
            .expect_err("one presentation name cannot select two stable field identities");
        assert!(
            error.0.contains(
                "layout field `word` identifies both stable member identity #7 and stable member identity #8"
            ),
            "{}",
            error.0
        );
    }

    #[test]
    fn access_validation_replays_retained_layout_structure_not_only_fingerprint() {
        let layout = LayoutPlanReport {
            schema_identity: 0x46,
            entries: vec![LayoutFieldEntryReport {
                field: "word".into(),
                member_identity: Some(7),
                placement: LayoutPlacementReport::At { offset: 0 },
            }],
            offsets: Some(vec![0]),
            size: Some(8),
            align: 8,
        };
        let mut plan = AccessPlan::inaccessible(&layout).expect("canonical access seed");
        let compact_identity = plan.layout_fingerprint;
        plan.retained_layout.entries[0].placement = LayoutPlacementReport::At { offset: 4 };
        assert_eq!(
            plan.layout_fingerprint, compact_identity,
            "the simulated carrier drift deliberately leaves its compact identity unchanged"
        );

        let error = validate_access_plan(plan, &layout)
            .expect_err("structural carrier drift must reject before access-plan sealing");
        assert!(
            error.0.contains("different validated layout"),
            "{}",
            error.0
        );
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
        let control = plan
            .field_descriptor(field_key(&plan, "control"))
            .expect("control descriptor");
        assert_eq!(control.container_byte_offset(), 8);
        assert_eq!(
            control.logical_extent().fragments(),
            &[LogicalFieldFragment {
                layout_bit_offset: 64,
                source_bit_offset: 0,
                width_bits: 8,
            }]
        );
        assert_eq!(
            control.effect_footprint(),
            RelativeEffectFootprint {
                byte_offset: 8,
                length_bytes: 4,
            },
            "a narrow logical bitfield retains its whole transfer container"
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
    fn narrow_external_write_rejects_before_admission() {
        let layout = uart_layout();
        let error = validate_access_plan(
            access_plan(
                &layout,
                &[(
                    "control",
                    FieldAccess::External {
                        transfer_width_bits: 32,
                        read: ExternalRead::Read,
                        write: true,
                        exposure: AccessExposure::BindingPrivate,
                    },
                )],
            ),
            &layout,
        )
        .expect_err("a narrow External write would require a generic RMW");
        assert!(
            error.0.contains("complete admitted container"),
            "diagnostic must explain the whole-transfer requirement: {error}"
        );
    }

    #[test]
    fn destructive_access_requires_one_whole_snapshot_accessor() {
        let layout = uart_layout();
        let error = validate_access_plan(
            access_plan(
                &layout,
                &[(
                    "control",
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
        .expect_err("a narrow field cannot independently consume its container");
        assert!(
            error
                .0
                .contains("only part of its 4-byte transfer container")
        );

        let aliased_layout = LayoutPlanReport {
            schema_identity: 0xdead,
            entries: vec![
                LayoutFieldEntryReport {
                    field: "snapshot".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At { offset: 0 },
                },
                LayoutFieldEntryReport {
                    field: "status".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At { offset: 0 },
                },
            ],
            offsets: Some(vec![0, 0]),
            size: Some(4),
            align: 4,
        };
        let error = validate_access_plan(
            access_plan(
                &aliased_layout,
                &[
                    (
                        "snapshot",
                        FieldAccess::External {
                            transfer_width_bits: 32,
                            read: ExternalRead::Take,
                            write: false,
                            exposure: AccessExposure::Exported,
                        },
                    ),
                    (
                        "status",
                        FieldAccess::Stable {
                            transfer_width_bits: 32,
                            read: true,
                            write: false,
                            exposure: AccessExposure::Exported,
                        },
                    ),
                ],
            ),
            &aliased_layout,
        )
        .expect_err("one destructive unit cannot expose a second field accessor");
        assert!(error.0.contains("one whole-snapshot take"));
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

        let store = AccessOperation::Atomic(AtomicAccessOperation::Store(MemoryOrdering::Publish));
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
            AccessOperation::Atomic(AtomicAccessOperation::FetchAdd(
                MemoryOrdering::ReceivePublish,
            )),
        )
        .expect("admitted fetch-add");
        assert!(
            plan.authorize(
                field_key(&plan, "head"),
                BorrowPolarity::Shared,
                BorrowPolarity::Shared,
                AccessOperation::Atomic(AtomicAccessOperation::FetchSub(
                    MemoryOrdering::ReceivePublish
                )),
            )
            .is_err(),
            "one admitted fetch family does not imply another"
        );
        let invalid_load =
            AccessOperation::Atomic(AtomicAccessOperation::Load(MemoryOrdering::Publish));
        let error = plan
            .authorize(
                field_key(&plan, "head"),
                BorrowPolarity::Shared,
                BorrowPolarity::Shared,
                invalid_load,
            )
            .expect_err("Publish cannot order an atomic load");
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
            &extent,
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
        let view = place(admission).expect("atomic placed-view establishment");
        let head = view
            .project(field_key(placement.access(), "head"))
            .expect("pure atomic projection");
        let request = head
            .atomic_compare_exchange(MemoryOrdering::ReceivePublish, MemoryOrdering::Receive)
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
                success: MemoryOrdering::ReceivePublish,
                failure: MemoryOrdering::Receive,
            })
        );
        assert_eq!(request.reach(), &BoundaryReach::default());
    }

    #[test]
    fn compare_exchange_permissions_keep_both_axes_distinct() {
        let permissions = [
            AtomicPermissions {
                compare_exchange: true,
                ..AtomicPermissions::default()
            },
            AtomicPermissions {
                compare_exchange_once: true,
                ..AtomicPermissions::default()
            },
            AtomicPermissions {
                try_exchange: true,
                ..AtomicPermissions::default()
            },
            AtomicPermissions {
                try_exchange_once: true,
                ..AtomicPermissions::default()
            },
        ];
        for (provided_index, provided) in permissions.iter().copied().enumerate() {
            assert!(provided.any());
            assert!(provided.contains(provided));
            for (required_index, required) in permissions.iter().copied().enumerate() {
                if provided_index != required_index {
                    assert!(
                        !provided.contains(required),
                        "compare-exchange permission row {provided_index} must not cover row {required_index}"
                    );
                }
            }
        }
    }

    #[test]
    fn overlapping_atomic_fields_cannot_select_mixed_widths() {
        let layout = LayoutPlanReport {
            schema_identity: 0xa70,
            entries: vec![
                LayoutFieldEntryReport {
                    field: "wide".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At { offset: 0 },
                },
                LayoutFieldEntryReport {
                    field: "upper".into(),
                    member_identity: None,
                    placement: LayoutPlacementReport::At { offset: 4 },
                },
            ],
            offsets: Some(vec![0, 4]),
            size: Some(8),
            align: 8,
        };
        let atomic_load = |transfer_width_bits| FieldAccess::Atomic {
            transfer_width_bits,
            operations: AtomicPermissions {
                load: true,
                ..AtomicPermissions::default()
            },
            exposure: AccessExposure::Exported,
        };
        let error = validate_access_plan(
            access_plan(
                &layout,
                &[("wide", atomic_load(64)), ("upper", atomic_load(32))],
            ),
            &layout,
        )
        .expect_err("one active placement cannot mix overlapping atomic widths");
        assert!(
            error.0.contains("overlapping transfer containers") && error.0.contains("mix widths"),
            "diagnostic must identify both the overlap and granularity conflict: {error}"
        );
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
        constructor: fn(u64) -> Result<T, psi_extents::ExtentDiagnostic>,
    ) -> T {
        constructor(identity).expect("normalized extent identity")
    }

    fn provider_issuance(seed: u64) -> psi_extents::ExtentProviderIssuance {
        let base = seed * 16;
        psi_extents::ExtentProviderIssuance::from_normalized_identities([
            base + 1,
            base + 2,
            base + 3,
            base + 4,
            base + 5,
            base + 6,
            base + 7,
            base + 8,
            base + 9,
            base + 10,
            base + 11,
            base + 12,
            base + 13,
        ])
        .expect("normalized provider issuance")
    }

    fn extent_rights(identities: &[u64]) -> ExtentRights {
        ExtentRights::from_normalized_identities(identities.iter().copied().map(|identity| {
            extent_id(
                identity,
                psi_extents::ExtentRightId::from_normalized_identity,
            )
        }))
    }

    fn uart_extent(base: u64, length: u64) -> psi_extents::Extent {
        uart_extent_with_lineage(base, length, 1)
    }

    fn uart_extent_with_lineage(base: u64, length: u64, lineage: u64) -> psi_extents::Extent {
        uart_extent_with_root(base, length, 1, lineage)
    }

    fn uart_extent_with_root(
        base: u64,
        length: u64,
        provider: u64,
        lineage: u64,
    ) -> psi_extents::Extent {
        uart_root_grant(provider, lineage)
            .mint(base, length)
            .expect("UART extent")
    }

    fn uart_root_grant(provider: u64, lineage: u64) -> psi_extents::ExtentRootGrant {
        uart_root_grant_with_mapping(provider, lineage, 5, 6)
    }

    fn uart_root_grant_with_mapping(
        provider: u64,
        lineage: u64,
        provenance: u64,
        era: u64,
    ) -> psi_extents::ExtentRootGrant {
        psi_extents::ExtentRootGrant::from_admitted_provider(
            provider_issuance(provider),
            extent_id(
                lineage,
                psi_extents::ExtentLineageId::from_normalized_identity,
            ),
            extent_id(2, AddressSpaceId::from_normalized_identity),
            extent_rights(&[3, 4]),
            extent_id(provenance, ExtentProvenanceId::from_normalized_identity),
            extent_id(era, psi_extents::MappingEraId::from_normalized_identity),
        )
    }

    fn uart_resource_profile(
        loan: &ExtentLoan<'_>,
        reach: &BoundaryReach,
    ) -> AdmittedResourceProfile {
        ResourceProfileGrant::from_admitted_provider_loan(
            ResourceProfileReceiptId::from_normalized_identity(7).expect("profile receipt"),
            loan,
            extent_rights(&[3]),
            reach.clone(),
        )
        .expect("UART resource-profile grant")
        .admit(uart_resource_profile_data(loan.length(), reach))
        .expect("admitted UART resource profile")
    }

    fn uart_resource_profile_for_extent(
        extent: &Extent,
        reach: &BoundaryReach,
    ) -> AdmittedResourceProfile {
        ResourceProfileGrant::from_admitted_provider(
            ResourceProfileReceiptId::from_normalized_identity(71).expect("profile receipt"),
            extent,
            extent_rights(&[3]),
            reach.clone(),
        )
        .expect("UART resource-profile grant")
        .admit(uart_resource_profile_data(extent.length(), reach))
        .expect("admitted UART resource profile")
    }

    fn uart_resource_profile_data(length: u64, reach: &BoundaryReach) -> ResourceProfile {
        ResourceProfile {
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
        }
    }

    fn stable_word_placement() -> ValidatedPlacementPlan {
        let layout = LayoutPlanReport {
            schema_identity: 0x5ab1e,
            entries: vec![LayoutFieldEntryReport {
                field: "word".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 0 },
            }],
            offsets: Some(vec![0]),
            size: Some(4),
            align: 4,
        };
        validate_placement_plan(PlacementPlan {
            access: access_plan(
                &layout,
                &[(
                    "word",
                    FieldAccess::Stable {
                        transfer_width_bits: 32,
                        read: true,
                        write: true,
                        exposure: AccessExposure::Exported,
                    },
                )],
            ),
            layout,
            reach: BoundaryReach::default(),
        })
        .expect("Stable word placement")
    }

    fn stable_word_profile(extent: &Extent) -> AdmittedResourceProfile {
        ResourceProfileGrant::from_admitted_provider(
            ResourceProfileReceiptId::from_normalized_identity(91).expect("profile receipt"),
            extent,
            extent_rights(&[3]),
            BoundaryReach::default(),
        )
        .expect("Stable resource-profile grant")
        .admit(ResourceProfile {
            regions: vec![ResourceRegion {
                offset: 0,
                length: extent.length(),
                stable: StableCapability::ReadWrite,
                external: ExternalCapability::None,
                atomic: AtomicCapability::None,
                reach: BoundaryReach::default(),
            }],
        })
        .expect("admitted Stable resource profile")
    }

    fn stable_uart_resource_profile(
        loan: &ExtentLoan<'_>,
        reach: &BoundaryReach,
    ) -> AdmittedResourceProfile {
        ResourceProfileGrant::from_admitted_provider_loan(
            ResourceProfileReceiptId::from_normalized_identity(141).expect("profile receipt"),
            loan,
            extent_rights(&[3]),
            reach.clone(),
        )
        .expect("Stable UART resource-profile grant")
        .admit(ResourceProfile {
            regions: vec![ResourceRegion {
                offset: 0,
                length: loan.length(),
                stable: StableCapability::ReadWrite,
                external: ExternalCapability::None,
                atomic: AtomicCapability::None,
                reach: reach.clone(),
            }],
        })
        .expect("admitted Stable UART resource profile")
    }

    fn destructive_word_placement() -> ValidatedPlacementPlan {
        let layout = LayoutPlanReport {
            schema_identity: 0xe17e_7a4e,
            entries: vec![LayoutFieldEntryReport {
                field: "fifo".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 0 },
            }],
            offsets: Some(vec![0]),
            size: Some(4),
            align: 4,
        };
        validate_placement_plan(PlacementPlan {
            access: access_plan(
                &layout,
                &[(
                    "fifo",
                    FieldAccess::External {
                        transfer_width_bits: 32,
                        read: ExternalRead::Take,
                        write: false,
                        exposure: AccessExposure::Exported,
                    },
                )],
            ),
            layout,
            reach: BoundaryReach::default(),
        })
        .expect("destructive External word placement")
    }

    fn destructive_word_profile(loan: &ExtentLoan<'_>) -> AdmittedResourceProfile {
        ResourceProfileGrant::from_admitted_provider_loan(
            ResourceProfileReceiptId::from_normalized_identity(142).expect("profile receipt"),
            loan,
            extent_rights(&[3]),
            BoundaryReach::default(),
        )
        .expect("destructive External resource-profile grant")
        .admit(ResourceProfile {
            regions: vec![ResourceRegion {
                offset: 0,
                length: loan.length(),
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
            }],
        })
        .expect("admitted destructive External resource profile")
    }

    const fn all_atomic_operations() -> AtomicPermissions {
        AtomicPermissions {
            load: true,
            store: true,
            fetch_add: true,
            fetch_sub: true,
            fetch_xor: true,
            fetch_or: true,
            fetch_and: true,
            swap: true,
            compare_exchange: true,
            compare_exchange_once: true,
            try_exchange: true,
            try_exchange_once: true,
        }
    }

    fn atomic_word_placement() -> ValidatedPlacementPlan {
        let layout = LayoutPlanReport {
            schema_identity: 0xa70_1c,
            entries: vec![LayoutFieldEntryReport {
                field: "head".into(),
                member_identity: None,
                placement: LayoutPlacementReport::At { offset: 0 },
            }],
            offsets: Some(vec![0]),
            size: Some(4),
            align: 4,
        };
        validate_placement_plan(PlacementPlan {
            access: access_plan(
                &layout,
                &[(
                    "head",
                    FieldAccess::Atomic {
                        transfer_width_bits: 32,
                        operations: all_atomic_operations(),
                        exposure: AccessExposure::Exported,
                    },
                )],
            ),
            layout,
            reach: BoundaryReach::default(),
        })
        .expect("all-family Atomic word placement")
    }

    fn atomic_word_profile(loan: &ExtentLoan<'_>) -> AdmittedResourceProfile {
        ResourceProfileGrant::from_admitted_provider_loan(
            ResourceProfileReceiptId::from_normalized_identity(155).expect("profile receipt"),
            loan,
            extent_rights(&[3]),
            BoundaryReach::default(),
        )
        .expect("Atomic resource-profile grant")
        .admit(ResourceProfile {
            regions: vec![ResourceRegion {
                offset: 0,
                length: loan.length(),
                stable: StableCapability::None,
                external: ExternalCapability::None,
                atomic: AtomicCapability::Access {
                    transfers: vec![AtomicTransferRule {
                        transfer: TransferRule {
                            width_bits: 32,
                            alignment_bytes: 4,
                        },
                        operations: all_atomic_operations(),
                    }],
                },
                reach: BoundaryReach::default(),
            }],
        })
        .expect("admitted all-family Atomic resource profile")
    }

    #[derive(Debug, PartialEq, Eq)]
    struct PrimitiveRequestSnapshot {
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
        authority_kind: &'static str,
        authority_identity: *const (),
    }

    fn primitive_request_snapshot(
        request: &PrimitiveAccessRequest<'_, '_>,
    ) -> PrimitiveRequestSnapshot {
        let (authority_kind, authority_identity) = match request._authority {
            PlacementAuthorityRef::Borrowed(view) => {
                ("borrowed", std::ptr::from_ref(view).cast::<()>())
            }
            PlacementAuthorityRef::CorrespondedBorrowed(view) => (
                "corresponded-borrowed",
                std::ptr::from_ref(view).cast::<()>(),
            ),
            PlacementAuthorityRef::BorrowedResident(established) => (
                "borrowed-resident",
                std::ptr::from_ref(established).cast::<()>(),
            ),
            PlacementAuthorityRef::EstablishedOwned(established) => (
                "established-owned",
                std::ptr::from_ref(established).cast::<()>(),
            ),
        };
        PrimitiveRequestSnapshot {
            plan: request.plan,
            profile_receipt: request.profile_receipt,
            effective_supply: request.effective_supply.clone(),
            admission: request.admission,
            primitive_address: request.primitive_address,
            key: request.key,
            field: request.field.clone(),
            transfer_width_bits: request.transfer_width_bits,
            logical_extent: request.logical_extent.clone(),
            effect_footprint: request.effect_footprint,
            observation: request.observation,
            current_borrow: request.current_borrow,
            source_loan: request.source_loan,
            operation: request.operation,
            reach: request.reach.clone(),
            resident_claim: request.resident_claim,
            placed_occurrence: request.placed_occurrence,
            descriptor: request.descriptor.clone(),
            authorization: request.authorization.clone(),
            authority_kind,
            authority_identity,
        }
    }

    fn assert_atomic_specialization(
        request: PrimitiveAccessRequest<'_, '_>,
        expected: AtomicAccessOperation,
        plan: PlacementPlanId,
        admission: PlacementAdmissionId,
    ) {
        let atomic = request
            .into_atomic_primitive_access()
            .expect("Atomic primitive specialization");
        assert_eq!(atomic.operation(), expected);
        assert_eq!(atomic.ordering_plan(), expected.ordering_plan());
        assert_eq!(atomic.primitive_address(), 0xc000);
        assert_eq!(atomic.transfer_width_bits(), 32);
        assert_eq!(atomic.logical_extent().fragments().len(), 1);
        assert_eq!(atomic.effect_footprint().address(), 0xc000);
        assert_eq!(atomic.effect_footprint().length_bytes(), 4);

        let request = atomic.into_primitive_request();
        assert_eq!(request.plan(), plan);
        assert_eq!(request.admission(), admission);
        assert_eq!(request.profile_receipt().normalized_identity(), 155);
        assert_eq!(
            request.effective_supply().kind(),
            EffectiveSupplyKind::Atomic
        );
        assert_eq!(request.effective_supply().key(), request.key);
        assert_eq!(request.effective_supply().width_bits(), 32);
        assert_eq!(request.effective_supply().alignment_bytes(), 4);
        assert_eq!(request.observation(), ObservationModel::Atomic);
        assert_eq!(request.current_borrow(), BorrowPolarity::Shared);
        assert_eq!(request.source_loan(), BorrowPolarity::Shared);
        assert_eq!(request.operation(), AccessOperation::Atomic(expected));
    }

    fn expect_exact_atomic_rejection<'view, 'extent>(
        request: PrimitiveAccessRequest<'view, 'extent>,
        diagnostic_fragment: &str,
    ) -> PrimitiveAccessRequest<'view, 'extent> {
        let before = primitive_request_snapshot(&request);
        let rejection = request
            .into_atomic_primitive_access()
            .expect_err("corrupt request must fail Atomic specialization");
        assert!(
            rejection.diagnostic().0.contains(diagnostic_fragment),
            "unexpected Atomic rejection: {}",
            rejection.diagnostic()
        );
        let (request, diagnostic) = rejection.into_parts();
        assert!(diagnostic.0.contains(diagnostic_fragment));
        assert_eq!(primitive_request_snapshot(&request), before);
        request
    }

    fn expect_exact_stable_primitive_rejection<'view, 'extent>(
        request: PrimitiveAccessRequest<'view, 'extent>,
        diagnostic_fragment: &str,
    ) -> PrimitiveAccessRequest<'view, 'extent> {
        let before = primitive_request_snapshot(&request);
        let rejection = request
            .into_stable_primitive_access()
            .expect_err("corrupt request must fail Stable primitive specialization");
        assert!(
            rejection.diagnostic().0.contains(diagnostic_fragment),
            "unexpected Stable primitive rejection: {}",
            rejection.diagnostic()
        );
        let (request, diagnostic) = rejection.into_parts();
        assert!(diagnostic.0.contains(diagnostic_fragment));
        assert_eq!(primitive_request_snapshot(&request), before);
        request
    }

    fn expect_exact_stable_compound_rejection<'view, 'extent>(
        request: PrimitiveAccessRequest<'view, 'extent>,
        diagnostic_fragment: &str,
    ) -> PrimitiveAccessRequest<'view, 'extent> {
        let before = primitive_request_snapshot(&request);
        let rejection = request
            .into_stable_compound_mutation_access()
            .expect_err("corrupt request must fail Stable compound specialization");
        assert!(
            rejection.diagnostic().0.contains(diagnostic_fragment),
            "unexpected Stable compound rejection: {}",
            rejection.diagnostic()
        );
        let (request, diagnostic) = rejection.into_parts();
        assert!(diagnostic.0.contains(diagnostic_fragment));
        assert_eq!(primitive_request_snapshot(&request), before);
        request
    }

    fn provider_existing_content(
        plan: &ValidatedPlacementPlan,
        base: u64,
        length: u64,
        lineage: u64,
        receipt_seed: u64,
    ) -> (Extent, ProviderExistingContentGrant) {
        uart_root_grant(1, lineage)
            .mint_provider_existing_content(
                base,
                length,
                extent_id(
                    plan.identity().normalized_identity(),
                    psi_extents::ExtentContentInterpretationId::from_normalized_identity,
                ),
                extent_id(receipt_seed + 2, ResidentClaimId::from_normalized_identity),
                extent_id(
                    receipt_seed,
                    ExtentContentValidityReceiptId::from_normalized_identity,
                ),
                extent_id(
                    receipt_seed + 1,
                    ExtentContentCustodyReceiptId::from_normalized_identity,
                ),
            )
            .expect("provider existing-content extent")
    }

    fn established_stable_word(
        base: u64,
        lineage: u64,
        receipt_seed: u64,
        admission_identity: u64,
    ) -> (ValidatedPlacementPlan, EstablishedOwnedPlacement) {
        let plan = stable_word_placement();
        let (extent, content) = provider_existing_content(&plan, base, 4, lineage, receipt_seed);
        let profile = stable_word_profile(&extent);
        let admission = admit_owned_placement(
            PlacementAdmissionId::from_normalized_identity(admission_identity).expect("admission"),
            extent,
            &plan,
            &profile,
        )
        .expect("owned Stable admission");
        let dormant =
            adopt_owned_stable(admission, content).expect("provider-evidenced Stable adoption");
        let established = dormant
            .view(
                PlacedOccurrenceId::from_normalized_identity(admission_identity + 10_000)
                    .expect("placed occurrence"),
            )
            .expect("owned resident-view establishment");
        (plan, established)
    }

    fn admit_uart<'extent>(
        identity: u64,
        loan: ExtentLoan<'extent>,
        plan: &ValidatedPlacementPlan,
        permitted_reach: &BoundaryReach,
    ) -> Result<PlacementAdmission<'extent>, PlacementRejection<'extent>> {
        let resources = uart_resource_profile(&loan, permitted_reach);
        admit_placement(
            PlacementAdmissionId::from_normalized_identity(identity).expect("placement admission"),
            loan,
            plan,
            &resources,
        )
    }

    #[test]
    fn provider_correspondence_admits_against_exact_plan_and_profile_without_storage_join() {
        let plan = uart_placement_plan();
        let extent = uart_extent_with_lineage(0x7180, 12, 236);
        let loan = extent.loan(0, 12).expect("shared UART loan");
        let profile = uart_resource_profile(&loan, &uart_reach());
        let provider = SchemaCorrespondenceProviderId::from_normalized_identity(237)
            .expect("correspondence provider");
        let device = StableDeviceInstanceId::from_normalized_identity(238).expect("stable device");
        let revision = RuntimeDeviceRevisionEvidence::from_admitted_provider(
            RuntimeDeviceRevisionObservationId::from_normalized_identity(239)
                .expect("revision observation"),
            DeviceRevisionPredicateId::from_normalized_identity(240).expect("revision predicate"),
            provider,
            device,
            profile.receipt(),
            3,
        );
        let grant = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
            provider,
            device,
            SchemaCorrespondenceSourceId::from_normalized_identity(241)
                .expect("datasheet provenance"),
            &plan,
            profile.receipt(),
            Some(revision),
        )
        .expect("provider correspondence grant");

        let mut colliding = plan.clone();
        colliding.layout.schema_identity ^= 1;
        assert_eq!(colliding.identity(), plan.identity());
        assert_ne!(colliding.layout(), plan.layout());
        let rejection = grant
            .admit(&colliding, &profile)
            .expect_err("compact placement identity cannot substitute exact plan structure");
        assert!(
            rejection
                .diagnostic()
                .0
                .contains("exact validated placement")
        );
        let (grant, _) = rejection.into_parts();

        let admitted = grant
            .admit(&plan, &profile)
            .expect("exact plan/profile correspondence admission");
        assert_eq!(admitted.placement(), plan.identity());
        assert_eq!(admitted.profile_receipt(), profile.receipt());
        assert_eq!(admitted.provider(), provider);
        assert_eq!(admitted.device(), device);
        assert_eq!(
            admitted
                .revision()
                .expect("runtime revision evidence")
                .observed_revision(),
            3
        );
    }

    #[test]
    fn correspondence_binding_replays_placement_and_returns_both_inputs_for_retry() {
        let plan = uart_placement_plan();
        let extent = uart_extent_with_lineage(0x7190, 12, 242);
        let loan = extent.loan(0, 12).expect("shared UART loan");
        let profile = uart_resource_profile(&loan, &uart_reach());
        let admission_id =
            PlacementAdmissionId::from_normalized_identity(243).expect("placement admission");
        let mut admission = admit_placement(admission_id, loan, &plan, &profile)
            .expect("borrowed placement admission");
        let provider = SchemaCorrespondenceProviderId::from_normalized_identity(244)
            .expect("correspondence provider");
        let device = StableDeviceInstanceId::from_normalized_identity(245).expect("stable device");
        let grant = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
            provider,
            device,
            SchemaCorrespondenceSourceId::from_normalized_identity(246)
                .expect("datasheet provenance"),
            &plan,
            profile.receipt(),
            None,
        )
        .expect("provider correspondence grant");
        let correspondence = grant
            .admit(&plan, &profile)
            .expect("schema correspondence admission");

        admission.placement_plan.layout.schema_identity ^= 1;
        assert_eq!(admission.placement_plan.identity(), plan.identity());
        let rejection = bind_schema_correspondence_to_placement(admission, correspondence)
            .expect_err("same compact identity cannot hide placement structure drift");
        assert!(rejection.diagnostic().0.contains("exact plan"));
        let (mut admission, mut correspondence, _) = rejection.into_parts();
        admission.placement_plan.layout.schema_identity = plan.layout().schema_identity;

        correspondence.replace_placement_for_test(PlacementPlanId(plan.identity().0 ^ 1));
        let rejection = bind_schema_correspondence_to_placement(admission, correspondence)
            .expect_err("placement identity drift must reject");
        assert!(rejection.diagnostic().0.contains("exact plan"));
        let (mut admission, mut correspondence, _) = rejection.into_parts();
        assert_eq!(admission.identity(), admission_id);
        correspondence.replace_placement_for_test(plan.identity());

        admission.profile_receipt =
            ResourceProfileReceiptId::from_normalized_identity(999).expect("drifted receipt");
        let rejection = bind_schema_correspondence_to_placement(admission, correspondence)
            .expect_err("admission receipt drift must reject");
        assert!(rejection.diagnostic().0.contains("exact plan"));
        let (mut admission, correspondence, _) = rejection.into_parts();
        admission.profile_receipt = profile.receipt();

        let bound = bind_schema_correspondence_to_placement(admission, correspondence)
            .expect("repaired inputs remain valid for retry");
        assert_eq!(bound.admission(), admission_id);
        assert_eq!(bound.correspondence().provider(), provider);
        assert_eq!(bound.correspondence().device(), device);
        let (loan, correspondence) = bound.withdraw();
        assert_eq!(loan.base(), 0x7190);
        assert_eq!(loan.length(), 12);
        assert_eq!(correspondence.placement(), plan.identity());
    }

    #[test]
    fn corresponded_view_establishment_replays_both_inputs_and_preserves_retry() {
        let plan = uart_placement_plan();
        let extent = uart_extent_with_lineage(0x71a0, 12, 247);
        let loan = extent.loan(0, 12).expect("shared UART loan");
        let profile = uart_resource_profile(&loan, &uart_reach());
        let admission_id =
            PlacementAdmissionId::from_normalized_identity(248).expect("placement admission");
        let admission = admit_placement(admission_id, loan, &plan, &profile)
            .expect("borrowed placement admission");
        let provider = SchemaCorrespondenceProviderId::from_normalized_identity(249)
            .expect("correspondence provider");
        let device = StableDeviceInstanceId::from_normalized_identity(250).expect("stable device");
        let grant = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
            provider,
            device,
            SchemaCorrespondenceSourceId::from_normalized_identity(251)
                .expect("datasheet provenance"),
            &plan,
            profile.receipt(),
            None,
        )
        .expect("provider correspondence grant");
        let correspondence = grant
            .admit(&plan, &profile)
            .expect("schema correspondence admission");
        let mut bound = bind_schema_correspondence_to_placement(admission, correspondence)
            .expect("correspondence placement binding");

        bound.replace_correspondence_placement_for_test(PlacementPlanId(plan.identity().0 ^ 1));
        let rejection = bound
            .establish_view()
            .expect_err("establishment must independently replay correspondence");
        assert!(rejection.diagnostic().0.contains("exact plan"));
        let (mut bound, _) = rejection.into_parts();
        assert_eq!(bound.admission(), admission_id);
        bound.replace_correspondence_placement_for_test(plan.identity());

        let view = bound
            .establish_view()
            .expect("repaired bound carrier remains valid for retry");
        assert_eq!(view.admission(), admission_id);
        assert_eq!(view.base(), 0x71a0);
        assert_eq!(view.length(), 12);
        assert_eq!(view.correspondence().provider(), provider);
        assert_eq!(view.correspondence().device(), device);
        assert_eq!(view.correspondence().placement(), plan.identity());
    }

    #[test]
    fn corresponded_view_retirement_replays_both_authorities_and_returns_exact_inputs() {
        let plan = uart_placement_plan();
        let extent = uart_extent_with_lineage(0x71a8, 12, 259);
        let origin = extent.origin();
        let lineage = extent.lineage_root();
        let loan = extent.loan(0, 12).expect("shared UART loan");
        let profile = uart_resource_profile(&loan, &uart_reach());
        let admission = admit_placement(
            PlacementAdmissionId::from_normalized_identity(260).expect("placement admission"),
            loan,
            &plan,
            &profile,
        )
        .expect("borrowed placement admission");
        let provider = SchemaCorrespondenceProviderId::from_normalized_identity(261)
            .expect("correspondence provider");
        let device =
            StableDeviceInstanceId::from_normalized_identity(262).expect("stable device instance");
        let correspondence = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
            provider,
            device,
            SchemaCorrespondenceSourceId::from_normalized_identity(263)
                .expect("datasheet provenance"),
            &plan,
            profile.receipt(),
            None,
        )
        .expect("provider correspondence grant")
        .admit(&plan, &profile)
        .expect("schema correspondence admission");
        let mut view = bind_schema_correspondence_to_placement(admission, correspondence)
            .expect("correspondence placement binding")
            .establish_view()
            .expect("corresponded view establishment");

        let drifted_receipt =
            ResourceProfileReceiptId::from_normalized_identity(264).expect("drifted receipt");
        view.replace_view_profile_receipt_for_test(drifted_receipt);
        view.replace_correspondence_profile_receipt_for_test(drifted_receipt);
        let rejection = view
            .retire()
            .expect_err("coordinated copied receipt drift must reject retirement");
        assert!(
            rejection
                .diagnostic()
                .0
                .contains("admitted resource-profile receipt")
        );
        let (mut view, _) = rejection.into_parts();
        view.replace_view_profile_receipt_for_test(profile.receipt());
        view.replace_correspondence_profile_receipt_for_test(profile.receipt());

        view.replace_correspondence_placement_for_test(PlacementPlanId(plan.identity().0 ^ 1));
        let rejection = view
            .retire()
            .expect_err("physical correspondence drift must reject retirement");
        assert!(rejection.diagnostic().0.contains("exact placement"));
        let (mut view, _) = rejection.into_parts();
        view.replace_correspondence_placement_for_test(plan.identity());

        let (loan, correspondence) = view
            .retire()
            .expect("repaired view remains valid for retirement retry");
        assert_eq!(loan.origin(), origin);
        assert_eq!(loan.lineage_root(), lineage);
        assert_eq!(loan.base(), 0x71a8);
        assert_eq!(loan.length(), 12);
        assert_eq!(loan.polarity(), LoanPolarity::Shared);
        assert_eq!(correspondence.provider(), provider);
        assert_eq!(correspondence.device(), device);
        assert_eq!(correspondence.placement(), plan.identity());
        assert_eq!(correspondence.profile_receipt(), profile.receipt());
    }

    #[test]
    fn corresponded_view_retains_and_replays_evidence_through_primitive_specialization() {
        let plan = uart_placement_plan();
        let extent = uart_extent_with_lineage(0x71b0, 12, 252);
        let loan = extent.loan(0, 12).expect("shared UART loan");
        let profile = uart_resource_profile(&loan, &uart_reach());
        let admission = admit_placement(
            PlacementAdmissionId::from_normalized_identity(253).expect("placement admission"),
            loan,
            &plan,
            &profile,
        )
        .expect("borrowed placement admission");
        let provider = SchemaCorrespondenceProviderId::from_normalized_identity(254)
            .expect("correspondence provider");
        let device = StableDeviceInstanceId::from_normalized_identity(255).expect("stable device");
        let correspondence = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
            provider,
            device,
            SchemaCorrespondenceSourceId::from_normalized_identity(256)
                .expect("datasheet provenance"),
            &plan,
            profile.receipt(),
            None,
        )
        .expect("provider correspondence grant")
        .admit(&plan, &profile)
        .expect("schema correspondence admission");
        let mut view = bind_schema_correspondence_to_placement(admission, correspondence)
            .expect("correspondence placement binding")
            .establish_view()
            .expect("corresponded view establishment");
        let status = field_key(plan.access(), "status");

        view.replace_correspondence_placement_for_test(PlacementPlanId(plan.identity().0 ^ 1));
        let rejection = view
            .project(status)
            .expect_err("projection must replay retained correspondence");
        assert!(rejection.0.contains("schema/device correspondence"));
        view.replace_correspondence_placement_for_test(plan.identity());

        let projection = view
            .project(status)
            .expect("repaired correspondence remains available for retry");
        assert_eq!(
            projection
                .correspondence()
                .expect("corresponded projection")
                .provider(),
            provider
        );
        let access = projection.read().expect("External status read");
        assert_eq!(
            access
                .correspondence()
                .expect("corresponded authorized access")
                .device(),
            device
        );
        let request = access.into_primitive_request();
        assert_eq!(
            request
                .correspondence()
                .expect("corresponded primitive request")
                .placement(),
            plan.identity()
        );
        let exact_request = primitive_request_snapshot(&request);
        let external = request
            .into_external_primitive_access()
            .expect("External specialization replays correspondence");
        assert_eq!(
            primitive_request_snapshot(external.primitive_request()),
            exact_request,
            "outward specialization must retain the exact sealed request"
        );
        assert_eq!(
            external
                .correspondence()
                .expect("correspondence reaches outward specialization")
                .device(),
            device
        );
        let alternate_correspondence = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
            SchemaCorrespondenceProviderId::from_normalized_identity(259)
                .expect("alternate correspondence provider"),
            StableDeviceInstanceId::from_normalized_identity(260).expect("alternate stable device"),
            SchemaCorrespondenceSourceId::from_normalized_identity(261)
                .expect("alternate datasheet provenance"),
            &plan,
            profile.receipt(),
            None,
        )
        .expect("alternate provider correspondence grant")
        .admit(&plan, &profile)
        .expect("alternate schema correspondence admission");
        let mut corresponded = external
            .into_corresponded_external_access()
            .expect("provider/device preflight requires retained correspondence");
        assert_eq!(corresponded.correspondence().provider(), provider);
        assert_eq!(
            primitive_request_snapshot(corresponded.external_access().primitive_request()),
            exact_request
        );
        let retained_correspondence =
            corresponded.replace_correspondence_for_test(&alternate_correspondence);
        let rejection = corresponded
            .validate_for_provider_lowering()
            .expect_err("a distinct correspondence carrier cannot replace retained authority");
        assert!(
            rejection
                .0
                .contains("different schema/device correspondence")
        );
        corresponded.replace_correspondence_for_test(retained_correspondence);
        corresponded
            .validate_for_provider_lowering()
            .expect("restoring the exact correspondence carrier permits retry");

        corresponded.replace_request_plan_for_test(PlacementPlanId(plan.identity().0 ^ 1));
        let rejection = corresponded
            .validate_for_provider_lowering()
            .expect_err("provider/device preflight must replay the retained placement");
        assert!(rejection.0.contains("copied plan"));
        assert_eq!(corresponded.correspondence().provider(), provider);
        assert_eq!(
            corresponded.external_access().primitive_request().plan(),
            PlacementPlanId(plan.identity().0 ^ 1),
            "borrowed request inspection reflects the still-retained drifted carrier"
        );
        corresponded.replace_request_plan_for_test(plan.identity());
        corresponded
            .validate_for_provider_lowering()
            .expect("repaired outward carrier remains available for retry");
        assert_eq!(
            primitive_request_snapshot(corresponded.external_access().primitive_request()),
            exact_request
        );
        let request = corresponded.into_external_access().into_primitive_request();
        assert_eq!(
            request
                .correspondence()
                .expect("retained evidence")
                .provider(),
            provider
        );

        let ordinary_extent = uart_extent_with_lineage(0x71c0, 12, 257);
        let ordinary_loan = ordinary_extent.loan(0, 12).expect("ordinary shared loan");
        let ordinary = place(
            admit_uart(258, ordinary_loan, &plan, &uart_reach())
                .expect("ordinary placement admission"),
        )
        .expect("ordinary view establishment");
        let ordinary_projection = ordinary.project(status).expect("ordinary projection");
        assert!(ordinary_projection.correspondence().is_none());
        let ordinary_request = ordinary_projection
            .read()
            .expect("ordinary External read")
            .into_primitive_request();
        let ordinary_snapshot = primitive_request_snapshot(&ordinary_request);
        let rejection = ordinary_request
            .into_external_primitive_access()
            .expect("ordinary External specialization remains valid")
            .into_corresponded_external_access()
            .expect_err("device/provider preflight must reject correspondence-free storage");
        assert!(rejection.diagnostic().0.contains("requires admitted"));
        let (ordinary_external, _) = rejection.into_parts();
        assert_eq!(
            primitive_request_snapshot(ordinary_external.primitive_request()),
            ordinary_snapshot,
            "rejection must return the exact already-specialized External request"
        );
        ordinary_external
            .validate_for_lowering()
            .expect("returned correspondence-free request remains valid for another consumer");
    }

    #[test]
    fn borrowed_admission_withdraws_the_exact_shared_loan() {
        let plan = uart_placement_plan();
        let mut extent = uart_extent_with_lineage(0x7200, 32, 76);
        let origin = extent.origin();
        let lineage = extent.lineage_root();
        let address_space = extent.address_space();
        let rights = extent.rights().clone();
        let provenance = extent.provenance();
        let era = extent.era();
        let loan = extent.loan(4, 12).expect("shared placement loan");
        let profile = uart_resource_profile(&loan, &uart_reach());

        let admission = admit_placement(
            PlacementAdmissionId::from_normalized_identity(77).expect("admission"),
            loan,
            &plan,
            &profile,
        )
        .expect("borrowed shared placement admission");
        let returned = admission.withdraw();

        assert_eq!(returned.base(), 0x7204);
        assert_eq!(returned.length(), 12);
        assert_eq!(returned.polarity(), LoanPolarity::Shared);
        assert_eq!(returned.origin(), origin);
        assert_eq!(returned.lineage_root(), lineage);
        assert_eq!(returned.address_space(), address_space);
        assert_eq!(returned.rights(), &rights);
        assert_eq!(returned.provenance(), provenance);
        assert_eq!(returned.era(), era);

        drop(returned);
        drop(
            extent
                .loan_mut(0, 32)
                .expect("dropping the returned loan restores exclusive parent access"),
        );
    }

    #[test]
    fn borrowed_admission_withdraws_the_exact_exclusive_loan() {
        let plan = uart_placement_plan();
        let mut extent = uart_extent_with_lineage(0x7300, 32, 78);
        let origin = extent.origin();
        let lineage = extent.lineage_root();
        let address_space = extent.address_space();
        let rights = extent.rights().clone();
        let provenance = extent.provenance();
        let era = extent.era();
        let loan = extent.loan_mut(8, 12).expect("exclusive placement loan");
        let profile = uart_resource_profile(&loan, &uart_reach());

        let admission = admit_placement(
            PlacementAdmissionId::from_normalized_identity(79).expect("admission"),
            loan,
            &plan,
            &profile,
        )
        .expect("borrowed exclusive placement admission");
        let returned = admission.withdraw();

        assert_eq!(returned.base(), 0x7308);
        assert_eq!(returned.length(), 12);
        assert_eq!(returned.polarity(), LoanPolarity::Exclusive);
        assert_eq!(returned.origin(), origin);
        assert_eq!(returned.lineage_root(), lineage);
        assert_eq!(returned.address_space(), address_space);
        assert_eq!(returned.rights(), &rights);
        assert_eq!(returned.provenance(), provenance);
        assert_eq!(returned.era(), era);

        drop(returned);
        drop(
            extent
                .loan(0, 32)
                .expect("dropping the returned loan restores shared parent access"),
        );
    }

    #[test]
    fn owned_admission_retains_and_withdraws_the_exact_extent() {
        let plan = uart_placement_plan();
        let extent = uart_extent_with_lineage(0x7000, 12, 72);
        let origin = extent.origin();
        let lineage = extent.lineage_root();
        let profile = uart_resource_profile_for_extent(&extent, &uart_reach());

        let admission = admit_owned_placement(
            PlacementAdmissionId::from_normalized_identity(73).expect("admission"),
            extent,
            &plan,
            &profile,
        )
        .expect("owned whole-range placement admission");
        assert_eq!(admission.identity().normalized_identity(), 73);
        assert_eq!(admission.extent().base(), 0x7000);
        assert_eq!(admission.extent().length(), 12);
        assert_eq!(admission.extent().origin(), origin);
        assert_eq!(admission.extent().lineage_root(), lineage);
        assert_eq!(admission.placement_plan().identity(), plan.identity());

        let returned = admission.withdraw();
        assert_eq!(returned.base(), 0x7000);
        assert_eq!(returned.length(), 12);
        assert_eq!(returned.origin(), origin);
        assert_eq!(returned.lineage_root(), lineage);
    }

    #[test]
    fn owned_admission_rejection_returns_the_exact_extent() {
        let plan = uart_placement_plan();
        let extent = uart_extent_with_lineage(0x7100, 8, 74);
        let origin = extent.origin();
        let lineage = extent.lineage_root();
        let profile = uart_resource_profile_for_extent(&extent, &uart_reach());

        let rejection = admit_owned_placement(
            PlacementAdmissionId::from_normalized_identity(75).expect("admission"),
            extent,
            &plan,
            &profile,
        )
        .expect_err("the complete placement must fit the owned extent");
        assert!(rejection.diagnostic().0.contains("exceeds"));
        let (returned, diagnostic) = rejection.into_parts();
        assert!(diagnostic.0.contains("exceeds"));
        assert_eq!(returned.base(), 0x7100);
        assert_eq!(returned.length(), 8);
        assert_eq!(returned.origin(), origin);
        assert_eq!(returned.lineage_root(), lineage);
    }

    #[test]
    fn provider_existing_content_establishes_owned_stable_placement() {
        let plan = stable_word_placement();
        let (extent, content) = provider_existing_content(&plan, 0xa000, 4, 92, 93);
        let origin = extent.origin();
        let lineage = extent.lineage_root();
        let address_space = extent.address_space();
        let provenance = extent.provenance();
        let era = extent.era();
        let profile = stable_word_profile(&extent);
        let admission = admit_owned_placement(
            PlacementAdmissionId::from_normalized_identity(95).expect("admission"),
            extent,
            &plan,
            &profile,
        )
        .expect("owned Stable admission");

        let dormant =
            adopt_owned_stable(admission, content).expect("provider-evidenced Stable adoption");
        assert_eq!(dormant.admission().normalized_identity(), 95);
        assert_eq!(dormant.placement_plan().identity(), plan.identity());
        assert_eq!(dormant.extent().base(), 0xa000);
        assert_eq!(dormant.extent().length(), 4);
        assert_eq!(dormant.extent().origin(), origin);
        assert_eq!(dormant.extent().lineage_root(), lineage);
        assert_eq!(dormant.extent().address_space(), address_space);
        assert_eq!(dormant.extent().provenance(), provenance);
        assert_eq!(dormant.extent().era(), era);
        assert_eq!(dormant.profile_receipt().normalized_identity(), 91);
        assert_eq!(dormant.resident_claim().normalized_identity(), 95);
        assert_eq!(dormant.validity_receipt().normalized_identity(), 93);
        assert_eq!(dormant.custody_receipt().normalized_identity(), 94);

        let established = dormant
            .view(PlacedOccurrenceId::from_normalized_identity(96).expect("placed occurrence"))
            .expect("owned resident-view establishment");
        assert_eq!(established.admission().normalized_identity(), 95);
        assert_eq!(established.placement_plan().identity(), plan.identity());
        assert_eq!(established.extent().base(), 0xa000);
        assert_eq!(established.extent().length(), 4);
        assert_eq!(established.resident_claim().normalized_identity(), 95);
        assert_eq!(established.occurrence().normalized_identity(), 96);
        assert_eq!(established.validity_receipt().normalized_identity(), 93);
        assert_eq!(established.custody_receipt().normalized_identity(), 94);
    }

    #[test]
    fn stable_adoption_replays_profile_and_returns_both_inputs_for_retry() {
        let plan = stable_word_placement();
        let (extent, content) = provider_existing_content(&plan, 0xad80, 4, 191, 192);
        let extent_origin = extent.origin();
        let extent_lineage = extent.lineage_root();
        let profile = stable_word_profile(&extent);
        let admission = admit_owned_placement(
            PlacementAdmissionId::from_normalized_identity(195).expect("admission"),
            extent,
            &plan,
            &profile,
        )
        .expect("owned Stable admission");

        let coincident = uart_extent_with_lineage(0xad80, 4, 196);
        let wrong_profile = stable_word_profile(&coincident);
        let OwnedPlacementAdmission {
            identity,
            placement_plan,
            profile_receipt,
            profile: _,
            resources,
            extent,
        } = admission;
        let corrupt = OwnedPlacementAdmission {
            identity,
            placement_plan,
            profile_receipt,
            profile: wrong_profile,
            resources,
            extent,
        };

        let rejection = adopt_owned_stable(corrupt, content)
            .expect_err("Stable adoption must replay admitted profile root facts");
        assert!(
            rejection
                .diagnostic()
                .0
                .contains("replay the admitted resource profile"),
            "{}",
            rejection.diagnostic()
        );
        let (returned, content, _) = rejection.into_parts();
        assert_eq!(returned.extent().origin(), extent_origin);
        assert_eq!(returned.extent().lineage_root(), extent_lineage);
        assert_eq!(content.resident_claim().normalized_identity(), 194);
        assert_eq!(content.validity_receipt().normalized_identity(), 192);
        assert_eq!(content.custody_receipt().normalized_identity(), 193);

        let OwnedPlacementAdmission {
            identity,
            placement_plan,
            profile_receipt,
            profile: _,
            resources,
            extent,
        } = returned;
        let repaired = OwnedPlacementAdmission {
            identity,
            placement_plan,
            profile_receipt,
            profile,
            resources,
            extent,
        };
        let dormant = adopt_owned_stable(repaired, content)
            .expect("returned admission and content remain valid for corrected retry");
        assert_eq!(dormant.admission().normalized_identity(), 195);
        assert_eq!(dormant.resident_claim().normalized_identity(), 194);
    }

    #[test]
    fn owned_resident_lifecycle_replays_full_provider_content_grant() {
        let plan = stable_word_placement();
        let (extent, content) = provider_existing_content(&plan, 0xad90, 4, 204, 205);
        let profile = stable_word_profile(&extent);
        let admission = admit_owned_placement(
            PlacementAdmissionId::from_normalized_identity(208).expect("admission"),
            extent,
            &plan,
            &profile,
        )
        .expect("owned Stable admission");
        let mut dormant =
            adopt_owned_stable(admission, content).expect("provider resident adoption");
        let claim = dormant.resident_claim();
        let validity = dormant.validity_receipt();
        let custody = dormant.custody_receipt();

        let (replacement_extent, _replacement_content) =
            provider_existing_content(&plan, 0xad90, 4, 209, 210);
        let replacement_profile = stable_word_profile(&replacement_extent);
        let replacement_admission = admit_owned_placement(
            PlacementAdmissionId::from_normalized_identity(213).expect("replacement admission"),
            replacement_extent,
            &plan,
            &replacement_profile,
        )
        .expect("coincident replacement placement");
        let retained_admission = std::mem::replace(&mut dormant.admission, replacement_admission);

        let occurrence =
            PlacedOccurrenceId::from_normalized_identity(214).expect("placed occurrence");
        let rejection = dormant
            .view(occurrence)
            .expect_err("resident view must replay the complete provider content grant");
        assert!(rejection.diagnostic().0.contains("provider content grant"));
        let (mut dormant, returned_occurrence, _) = rejection.into_parts();
        assert_eq!(returned_occurrence, occurrence);
        assert_eq!(dormant.resident_claim(), claim);
        assert_eq!(dormant.validity_receipt(), validity);
        assert_eq!(dormant.custody_receipt(), custody);
        let replacement_admission = std::mem::replace(&mut dormant.admission, retained_admission);

        let mut established = dormant
            .view(returned_occurrence)
            .expect("repaired dormant carrier supports corrected view");
        let retained_admission =
            std::mem::replace(&mut established.admission, replacement_admission);
        let rejection = established
            .retire_resident()
            .expect_err("resident retirement must replay the complete provider content grant");
        assert!(rejection.diagnostic().0.contains("provider content grant"));
        let (mut established, _) = rejection.into_parts();
        assert_eq!(established.occurrence(), occurrence);
        assert_eq!(established.resident_claim(), claim);
        assert_eq!(established.validity_receipt(), validity);
        assert_eq!(established.custody_receipt(), custody);
        established.admission = retained_admission;

        let dormant = established
            .retire_resident()
            .expect("returned active carrier supports corrected retirement");
        assert_eq!(dormant.resident_claim(), claim);
        assert_eq!(dormant.validity_receipt(), validity);
        assert_eq!(dormant.custody_receipt(), custody);
        assert_eq!(dormant.admission().normalized_identity(), 208);
    }

    #[test]
    fn owned_resident_view_and_retirement_preserve_claim_and_rotate_occurrence() {
        let plan = stable_word_placement();
        let (extent, content) = provider_existing_content(&plan, 0xa080, 4, 97, 98);
        let profile = stable_word_profile(&extent);
        let admission = admit_owned_placement(
            PlacementAdmissionId::from_normalized_identity(101).expect("admission"),
            extent,
            &plan,
            &profile,
        )
        .expect("owned Stable admission");
        let mut dormant =
            adopt_owned_stable(admission, content).expect("provider resident adoption");
        let claim = dormant.resident_claim();
        let validity = dormant.validity_receipt();
        let custody = dormant.custody_receipt();
        assert_eq!(claim.normalized_identity(), 100);

        let first_occurrence =
            PlacedOccurrenceId::from_normalized_identity(102).expect("first occurrence");
        let coincident = uart_extent_with_lineage(0xa080, 4, 199);
        dormant.admission.profile = stable_word_profile(&coincident);
        let rejection = dormant
            .view(first_occurrence)
            .expect_err("owned resident view must replay retained placement authority");
        assert!(
            rejection
                .diagnostic()
                .0
                .contains("could not replay the retained placement authority"),
            "{}",
            rejection.diagnostic()
        );
        let (mut dormant, returned_occurrence, _) = rejection.into_parts();
        assert_eq!(returned_occurrence, first_occurrence);
        assert_eq!(dormant.resident_claim(), claim);
        assert_eq!(dormant.validity_receipt(), validity);
        assert_eq!(dormant.custody_receipt(), custody);
        assert_eq!(dormant.extent().base(), 0xa080);
        dormant.admission.profile = profile;
        let mut first = dormant
            .view(returned_occurrence)
            .expect("first owned resident-view establishment");
        assert_eq!(first.resident_claim(), claim);
        assert_eq!(first.occurrence(), first_occurrence);
        {
            let projection = first
                .project(field_key(plan.access(), "word"))
                .expect("resident field projection");
            assert_eq!(projection.resident_claim(), Some(claim));
            assert_eq!(projection.placed_occurrence(), Some(first_occurrence));
            let access = projection.read().expect("resident Stable read");
            assert_eq!(access.resident_claim(), Some(claim));
            assert_eq!(access.placed_occurrence(), Some(first_occurrence));
            let request = access.into_primitive_request();
            assert_eq!(request.resident_claim(), Some(claim));
            assert_eq!(request.placed_occurrence(), Some(first_occurrence));
        }

        let retained_profile = first.admission.profile.clone();
        let coincident = uart_extent_with_lineage(0xa080, 4, 200);
        first.admission.profile = stable_word_profile(&coincident);
        let rejection = first
            .retire_resident()
            .expect_err("resident retirement must replay retained placement authority");
        assert!(
            rejection
                .diagnostic()
                .0
                .contains("could not replay the retained placement authority"),
            "{}",
            rejection.diagnostic()
        );
        let (mut first, _) = rejection.into_parts();
        assert_eq!(first.resident_claim(), claim);
        assert_eq!(first.occurrence(), first_occurrence);
        assert_eq!(first.validity_receipt(), validity);
        assert_eq!(first.custody_receipt(), custody);
        first.admission.profile = retained_profile;
        let dormant = first
            .retire_resident()
            .expect("returned active resident supports corrected retirement");
        assert_eq!(dormant.resident_claim(), claim);
        assert_eq!(dormant.validity_receipt(), validity);
        assert_eq!(dormant.custody_receipt(), custody);
        assert_eq!(dormant.extent().base(), 0xa080);
        assert_eq!(dormant.placement_plan().identity(), plan.identity());

        let second_occurrence =
            PlacedOccurrenceId::from_normalized_identity(103).expect("second occurrence");
        let second = dormant
            .view(second_occurrence)
            .expect("second owned resident-view establishment");
        assert_eq!(second.resident_claim(), claim);
        assert_eq!(second.occurrence(), second_occurrence);
        assert_ne!(second.occurrence(), first_occurrence);
    }

    #[test]
    fn borrowed_resident_views_retain_claim_receipts_and_exact_loan_polarity() {
        let plan = stable_word_placement();
        let (extent, content) = provider_existing_content(&plan, 0xa100, 4, 104, 105);
        let profile = stable_word_profile(&extent);
        let admission = admit_owned_placement(
            PlacementAdmissionId::from_normalized_identity(108).expect("admission"),
            extent,
            &plan,
            &profile,
        )
        .expect("owned Stable admission");
        let mut dormant =
            adopt_owned_stable(admission, content).expect("provider resident adoption");
        let claim = dormant.resident_claim();
        let validity = dormant.validity_receipt();
        let custody = dormant.custody_receipt();
        let retained_profile = profile.clone();

        let shared_occurrence =
            PlacedOccurrenceId::from_normalized_identity(109).expect("shared occurrence");
        let coincident = uart_extent_with_lineage(0xa100, 4, 201);
        dormant.admission.profile = stable_word_profile(&coincident);
        let diagnostic = dormant
            .borrow_view(shared_occurrence)
            .expect_err("shared resident view must replay retained placement authority");
        assert!(diagnostic.0.contains("shared-view establishment"));
        assert!(diagnostic.0.contains("retained placement authority"));
        assert_eq!(dormant.resident_claim(), claim);
        assert_eq!(dormant.validity_receipt(), validity);
        assert_eq!(dormant.custody_receipt(), custody);
        assert_eq!(dormant.extent().base(), 0xa100);
        dormant.admission.profile = retained_profile.clone();
        {
            let mut borrowed = dormant
                .borrow_view(shared_occurrence)
                .expect("shared resident loan");
            assert_eq!(borrowed.base(), 0xa100);
            assert_eq!(borrowed.length(), 4);
            assert_eq!(borrowed.loan_polarity(), LoanPolarity::Shared);
            assert_eq!(borrowed.resident_claim(), claim);
            assert_eq!(borrowed.occurrence(), shared_occurrence);
            assert_eq!(borrowed.validity_receipt(), validity);
            assert_eq!(borrowed.custody_receipt(), custody);

            let projection = borrowed
                .project(field_key(plan.access(), "word"))
                .expect("shared resident field projection");
            let request = projection
                .read()
                .expect("shared resident read")
                .into_primitive_request();
            assert_eq!(request.source_loan(), BorrowPolarity::Shared);
            assert_eq!(request.resident_claim(), Some(claim));
            assert_eq!(request.placed_occurrence(), Some(shared_occurrence));
            assert_eq!(
                primitive_request_snapshot(&request).authority_kind,
                "borrowed-resident"
            );
            drop(request);

            let mut projection = borrowed
                .project_mut(field_key(plan.access(), "word"))
                .expect("exclusive projection borrow over shared resident loan");
            let diagnostic = projection
                .write()
                .expect_err("shared resident loan cannot authorize a write");
            assert!(diagnostic.0.contains("Shared source loan"));

            let coincident = uart_extent_with_lineage(0xa100, 4, 203);
            let wrong_profile = stable_word_profile(&coincident);
            let correct_profile = borrowed.replace_profile_for_test(wrong_profile);
            let rejection = borrowed
                .retire()
                .expect_err("shared resident retirement must replay exact loan authority");
            assert!(rejection.diagnostic().0.contains("retirement"));
            assert!(
                rejection
                    .diagnostic()
                    .0
                    .contains("retained placement authority")
            );
            let (mut borrowed, _) = rejection.into_parts();
            assert_eq!(borrowed.resident_claim(), claim);
            assert_eq!(borrowed.occurrence(), shared_occurrence);
            assert_eq!(borrowed.validity_receipt(), validity);
            assert_eq!(borrowed.custody_receipt(), custody);
            borrowed.replace_profile_for_test(correct_profile);

            let (_coincident_extent, coincident_content) =
                provider_existing_content(&plan, 0xa100, 4, 215, 216);
            let correct_content = borrowed.replace_content_for_test(&coincident_content);
            let diagnostic = borrowed
                .project(field_key(plan.access(), "word"))
                .expect_err("borrowed projection must replay the exact resident content grant");
            assert!(diagnostic.0.contains("resident content grant"));
            let rejection = borrowed
                .retire()
                .expect_err("shared retirement must replay the exact borrowed content grant");
            assert!(rejection.diagnostic().0.contains("provider content grant"));
            let (mut borrowed, _) = rejection.into_parts();
            borrowed.replace_content_for_test(correct_content);
            assert_eq!(borrowed.resident_claim(), claim);
            assert_eq!(borrowed.validity_receipt(), validity);
            assert_eq!(borrowed.custody_receipt(), custody);
            borrowed
                .retire()
                .expect("returned shared resident carrier supports corrected retirement");
        }
        assert_eq!(dormant.resident_claim(), claim);
        assert_eq!(dormant.validity_receipt(), validity);
        assert_eq!(dormant.custody_receipt(), custody);

        let exclusive_occurrence =
            PlacedOccurrenceId::from_normalized_identity(110).expect("exclusive occurrence");
        let coincident = uart_extent_with_lineage(0xa100, 4, 202);
        dormant.admission.profile = stable_word_profile(&coincident);
        let diagnostic = dormant
            .borrow_view_mut(exclusive_occurrence)
            .expect_err("exclusive resident view must replay retained placement authority");
        assert!(diagnostic.0.contains("exclusive-view establishment"));
        assert!(diagnostic.0.contains("retained placement authority"));
        assert_eq!(dormant.resident_claim(), claim);
        assert_eq!(dormant.validity_receipt(), validity);
        assert_eq!(dormant.custody_receipt(), custody);
        assert_eq!(dormant.extent().base(), 0xa100);
        dormant.admission.profile = retained_profile;
        {
            let mut borrowed = dormant
                .borrow_view_mut(exclusive_occurrence)
                .expect("exclusive resident loan");
            assert_eq!(borrowed.loan_polarity(), LoanPolarity::Exclusive);
            let mut projection = borrowed
                .project_mut(field_key(plan.access(), "word"))
                .expect("exclusive resident field projection");
            let request = projection
                .write()
                .expect("exclusive resident write")
                .into_primitive_request();
            assert_eq!(request.source_loan(), BorrowPolarity::Exclusive);
            assert_eq!(request.resident_claim(), Some(claim));
            assert_eq!(request.placed_occurrence(), Some(exclusive_occurrence));
            drop(request);
            borrowed
                .retire()
                .expect("exclusive resident retirement replays exact loan authority");
        }

        assert_eq!(dormant.resident_claim(), claim);
        assert_eq!(dormant.extent().base(), 0xa100);
        let owned_occurrence =
            PlacedOccurrenceId::from_normalized_identity(111).expect("owned occurrence");
        let owned = dormant
            .view(owned_occurrence)
            .expect("owned resident-view establishment");
        assert_eq!(owned.resident_claim(), claim);
        assert_eq!(owned.occurrence(), owned_occurrence);
    }

    #[test]
    fn established_owned_stable_shared_projection_seals_a_read_request() {
        let (plan, established) = established_stable_word(0xa400, 112, 113, 115);
        let projection = established
            .project(field_key(plan.access(), "word"))
            .expect("shared Stable projection");
        let request = projection
            .read()
            .expect("Stable shared read")
            .into_primitive_request();

        assert_eq!(request.plan(), plan.identity());
        assert_eq!(request.admission().normalized_identity(), 115);
        assert_eq!(request.profile_receipt().normalized_identity(), 91);
        assert_eq!(
            request.effective_supply().kind(),
            EffectiveSupplyKind::Stable
        );
        assert_eq!(request.primitive_address(), 0xa400);
        assert_eq!(request.field(), "word");
        assert_eq!(request.observation(), ObservationModel::Stable);
        assert_eq!(request.current_borrow(), BorrowPolarity::Shared);
        assert_eq!(request.source_loan(), BorrowPolarity::Exclusive);
        assert_eq!(request.operation(), AccessOperation::Read);
    }

    #[test]
    fn established_owned_stable_exclusive_projection_seals_a_write_request() {
        let (plan, mut established) = established_stable_word(0xa500, 116, 117, 119);
        let mut projection = established
            .project_mut(field_key(plan.access(), "word"))
            .expect("exclusive Stable projection");
        let request = projection
            .write()
            .expect("Stable exclusive write")
            .into_primitive_request();

        assert_eq!(request.primitive_address(), 0xa500);
        assert_eq!(request.observation(), ObservationModel::Stable);
        assert_eq!(request.current_borrow(), BorrowPolarity::Exclusive);
        assert_eq!(request.source_loan(), BorrowPolarity::Exclusive);
        assert_eq!(request.operation(), AccessOperation::Write);
    }

    #[test]
    fn established_owned_stable_shared_projection_rejects_write() {
        let (plan, established) = established_stable_word(0xa600, 120, 121, 123);
        let mut projection = established
            .project(field_key(plan.access(), "word"))
            .expect("shared Stable projection");

        let rejection = projection
            .write()
            .expect_err("shared current borrow must not authorize Stable write");
        assert!(rejection.0.contains("Shared current borrow"));
        assert_eq!(established.validity_receipt().normalized_identity(), 121);
        assert_eq!(established.custody_receipt().normalized_identity(), 122);
    }

    #[test]
    fn established_owned_read_specializes_for_stable_primitive_lowering() {
        let (plan, established) = established_stable_word(0xa700, 124, 125, 127);
        let projection = established
            .project(field_key(plan.access(), "word"))
            .expect("shared Stable projection");
        let request = projection
            .read()
            .expect("Stable read")
            .into_primitive_request();
        let stable = request
            .into_stable_primitive_access()
            .expect("Stable read specialization");

        assert_eq!(stable.operation(), StablePrimitiveOperation::Read);
        assert_eq!(stable.primitive_address(), 0xa700);
        assert_eq!(stable.transfer_width_bits(), 32);
        assert_eq!(stable.effect_footprint().address(), 0xa700);
        assert_eq!(stable.effect_footprint().length_bytes(), 4);
        assert_eq!(stable.logical_extent().fragments().len(), 1);
        let request = stable.into_primitive_request();
        assert_eq!(request.plan(), plan.identity());
        assert_eq!(request.admission().normalized_identity(), 127);
        assert_eq!(request.profile_receipt().normalized_identity(), 91);
        assert_eq!(request.source_loan(), BorrowPolarity::Exclusive);
    }

    #[test]
    fn stable_primitive_lowering_replays_authority_without_consuming_retry() {
        let (plan, established) = established_stable_word(0xa740, 224, 225, 227);
        let projection = established
            .project(field_key(plan.access(), "word"))
            .expect("shared Stable projection");
        let request = projection
            .read()
            .expect("Stable read")
            .into_primitive_request();
        let mut stable = request
            .into_stable_primitive_access()
            .expect("Stable read specialization");
        let expected = primitive_request_snapshot(&stable.request);

        stable.request.profile_receipt =
            ResourceProfileReceiptId::from_normalized_identity(999).expect("drifted receipt");
        let diagnostic = stable
            .validate_for_lowering()
            .expect_err("outward preflight must reject copied receipt drift");
        assert!(diagnostic.0.contains("retained placement authority"));
        stable.request.profile_receipt =
            ResourceProfileReceiptId::from_normalized_identity(91).expect("profile receipt");

        stable.operation = StablePrimitiveOperation::Write;
        let diagnostic = stable
            .validate_for_lowering()
            .expect_err("outward preflight must reject specialization drift");
        assert!(diagnostic.0.contains("retained specialization"));
        stable.operation = StablePrimitiveOperation::Read;

        stable
            .validate_for_lowering()
            .expect("corrected carrier must remain valid for retry");
        assert_eq!(primitive_request_snapshot(&stable.request), expected);
        assert_eq!(stable.operation(), StablePrimitiveOperation::Read);
    }

    #[test]
    fn provider_stable_preflight_requires_and_retains_exact_correspondence() {
        let plan = stable_word_placement();
        let extent = uart_extent_with_lineage(0xa780, 4, 272);
        let profile = stable_word_profile(&extent);
        let loan = extent.loan(0, 4).expect("shared Stable loan");
        let admission = admit_placement(
            PlacementAdmissionId::from_normalized_identity(273).expect("admission"),
            loan,
            &plan,
            &profile,
        )
        .expect("Stable placement admission");
        let provider = SchemaCorrespondenceProviderId::from_normalized_identity(274)
            .expect("correspondence provider");
        let device = StableDeviceInstanceId::from_normalized_identity(275).expect("stable device");
        let correspondence = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
            provider,
            device,
            SchemaCorrespondenceSourceId::from_normalized_identity(276)
                .expect("provider provenance"),
            &plan,
            profile.receipt(),
            None,
        )
        .expect("provider correspondence grant")
        .admit(&plan, &profile)
        .expect("schema correspondence admission");
        let view = bind_schema_correspondence_to_placement(admission, correspondence)
            .expect("correspondence placement binding")
            .establish_view()
            .expect("corresponded view establishment");
        let word = view
            .project(field_key(plan.access(), "word"))
            .expect("Stable word projection");
        let request = word.read().expect("Stable read").into_primitive_request();
        let expected = primitive_request_snapshot(&request);
        let stable = request
            .into_stable_primitive_access()
            .expect("Stable read specialization");

        let alternate_correspondence = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
            SchemaCorrespondenceProviderId::from_normalized_identity(277)
                .expect("alternate correspondence provider"),
            StableDeviceInstanceId::from_normalized_identity(278).expect("alternate stable device"),
            SchemaCorrespondenceSourceId::from_normalized_identity(279)
                .expect("alternate provider provenance"),
            &plan,
            profile.receipt(),
            None,
        )
        .expect("alternate provider correspondence grant")
        .admit(&plan, &profile)
        .expect("alternate schema correspondence admission");
        let mut corresponded = stable
            .into_corresponded_stable_access()
            .expect("provider/device Stable preflight requires retained correspondence");
        assert_eq!(corresponded.correspondence().provider(), provider);
        assert_eq!(
            corresponded.stable_access().operation(),
            StablePrimitiveOperation::Read
        );
        assert_eq!(
            primitive_request_snapshot(corresponded.stable_access().primitive_request()),
            expected
        );

        let retained_correspondence =
            corresponded.replace_correspondence_for_test(&alternate_correspondence);
        let diagnostic = corresponded
            .validate_for_provider_lowering()
            .expect_err("a distinct correspondence carrier cannot replace retained authority");
        assert!(
            diagnostic
                .0
                .contains("different schema/device correspondence")
        );
        corresponded.replace_correspondence_for_test(retained_correspondence);

        corresponded.replace_request_plan_for_test(PlacementPlanId(plan.identity().0 ^ 1));
        let diagnostic = corresponded
            .validate_for_provider_lowering()
            .expect_err("provider/device Stable preflight must replay placement authority");
        assert!(diagnostic.0.contains("copied plan"));
        corresponded.replace_request_plan_for_test(plan.identity());
        corresponded
            .validate_for_provider_lowering()
            .expect("restored exact carrier remains available for retry");
        assert_eq!(
            primitive_request_snapshot(corresponded.into_stable_access().primitive_request()),
            expected
        );

        let ordinary_extent = uart_extent_with_lineage(0xa790, 4, 280);
        let ordinary_profile = stable_word_profile(&ordinary_extent);
        let ordinary_loan = ordinary_extent
            .loan(0, 4)
            .expect("ordinary shared Stable loan");
        let ordinary = place(
            admit_placement(
                PlacementAdmissionId::from_normalized_identity(281).expect("ordinary admission"),
                ordinary_loan,
                &plan,
                &ordinary_profile,
            )
            .expect("ordinary Stable placement admission"),
        )
        .expect("ordinary Stable view establishment");
        let ordinary_projection = ordinary
            .project(field_key(plan.access(), "word"))
            .expect("ordinary Stable projection");
        let ordinary_request = ordinary_projection
            .read()
            .expect("ordinary Stable read")
            .into_primitive_request();
        let ordinary_snapshot = primitive_request_snapshot(&ordinary_request);
        let rejection = ordinary_request
            .into_stable_primitive_access()
            .expect("ordinary Stable specialization remains valid")
            .into_corresponded_stable_access()
            .expect_err("provider/device preflight rejects correspondence-free Stable storage");
        assert!(rejection.diagnostic().0.contains("requires admitted"));
        let (ordinary_stable, _) = rejection.into_parts();
        assert_eq!(
            primitive_request_snapshot(ordinary_stable.primitive_request()),
            ordinary_snapshot,
            "rejection returns the exact already-specialized Stable request"
        );
        ordinary_stable
            .validate_for_lowering()
            .expect("returned correspondence-free Stable request remains usable elsewhere");
    }

    #[test]
    fn established_owned_write_specializes_for_stable_primitive_lowering() {
        let (plan, mut established) = established_stable_word(0xa800, 128, 129, 131);
        let mut projection = established
            .project_mut(field_key(plan.access(), "word"))
            .expect("exclusive Stable projection");
        let request = projection
            .write()
            .expect("Stable write")
            .into_primitive_request();
        let stable = request
            .into_stable_primitive_access()
            .expect("Stable write specialization");

        assert_eq!(stable.operation(), StablePrimitiveOperation::Write);
        assert_eq!(stable.primitive_address(), 0xa800);
        let request = stable.into_primitive_request();
        assert_eq!(request.plan(), plan.identity());
        assert_eq!(request.admission().normalized_identity(), 131);
        assert_eq!(request.current_borrow(), BorrowPolarity::Exclusive);
        assert_eq!(request.source_loan(), BorrowPolarity::Exclusive);
    }

    #[test]
    fn established_owned_compound_mutation_specializes_with_exact_custody() {
        let (plan, mut established) = established_stable_word(0xad00, 160, 161, 163);
        let mut projection = established
            .project_mut(field_key(plan.access(), "word"))
            .expect("exclusive Stable projection");
        let request = projection
            .compound_mutation()
            .expect("authorized Stable compound mutation")
            .into_primitive_request();
        let before = primitive_request_snapshot(&request);
        let compound = request
            .into_stable_compound_mutation_access()
            .expect("Stable compound specialization");

        assert_eq!(compound.primitive_address(), 0xad00);
        assert_eq!(compound.transfer_width_bits(), 32);
        assert_eq!(compound.logical_extent().fragments().len(), 1);
        assert_eq!(compound.effect_footprint().address(), 0xad00);
        assert_eq!(compound.effect_footprint().length_bytes(), 4);
        let request = compound.into_primitive_request();
        assert_eq!(primitive_request_snapshot(&request), before);
        assert_eq!(request.plan(), plan.identity());
        assert_eq!(request.admission().normalized_identity(), 163);
        assert_eq!(request.effective_supply().key(), request.key);
        assert_eq!(request.effective_supply().width_bits(), 32);
        assert_eq!(request.current_borrow(), BorrowPolarity::Exclusive);
        assert_eq!(request.source_loan(), BorrowPolarity::Exclusive);
        assert_eq!(request.operation(), AccessOperation::CompoundMutation);
        drop(request);
        assert_eq!(established.validity_receipt().normalized_identity(), 161);
        assert_eq!(established.custody_receipt().normalized_identity(), 162);
    }

    #[test]
    fn stable_compound_lowering_replays_authority_without_consuming_retry() {
        let (plan, mut established) = established_stable_word(0xad10, 228, 229, 231);
        let mut projection = established
            .project_mut(field_key(plan.access(), "word"))
            .expect("exclusive Stable projection");
        let request = projection
            .compound_mutation()
            .expect("authorized Stable compound mutation")
            .into_primitive_request();
        let mut compound = request
            .into_stable_compound_mutation_access()
            .expect("Stable compound specialization");
        let expected = primitive_request_snapshot(&compound.request);

        compound.request.profile_receipt =
            ResourceProfileReceiptId::from_normalized_identity(999).expect("drifted receipt");
        let diagnostic = compound
            .validate_for_lowering()
            .expect_err("outward preflight must reject copied receipt drift");
        assert!(diagnostic.0.contains("retained placement authority"));
        compound.request.profile_receipt =
            ResourceProfileReceiptId::from_normalized_identity(91).expect("profile receipt");

        compound.request.operation = AccessOperation::Write;
        let diagnostic = compound
            .validate_for_lowering()
            .expect_err("outward preflight must reject operation drift");
        assert!(diagnostic.0.contains("CompoundMutation"));
        compound.request.operation = AccessOperation::CompoundMutation;

        compound
            .validate_for_lowering()
            .expect("corrected carrier must remain valid for retry");
        assert_eq!(primitive_request_snapshot(&compound.request), expected);
    }

    #[test]
    fn provider_stable_compound_preflight_requires_exact_correspondence() {
        let plan = stable_word_placement();
        let mut extent = uart_extent_with_lineage(0xad18, 4, 282);
        let profile = stable_word_profile(&extent);
        let loan = extent.loan_mut(0, 4).expect("exclusive Stable loan");
        let admission = admit_placement(
            PlacementAdmissionId::from_normalized_identity(283).expect("admission"),
            loan,
            &plan,
            &profile,
        )
        .expect("Stable placement admission");
        let provider = SchemaCorrespondenceProviderId::from_normalized_identity(284)
            .expect("correspondence provider");
        let device = StableDeviceInstanceId::from_normalized_identity(285).expect("stable device");
        let correspondence = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
            provider,
            device,
            SchemaCorrespondenceSourceId::from_normalized_identity(286)
                .expect("provider provenance"),
            &plan,
            profile.receipt(),
            None,
        )
        .expect("provider correspondence grant")
        .admit(&plan, &profile)
        .expect("schema correspondence admission");
        let mut view = bind_schema_correspondence_to_placement(admission, correspondence)
            .expect("correspondence placement binding")
            .establish_view()
            .expect("corresponded view establishment");
        let mut word = view
            .project_mut(field_key(plan.access(), "word"))
            .expect("exclusive Stable word projection");
        let request = word
            .compound_mutation()
            .expect("Stable compound mutation")
            .into_primitive_request();
        let expected = primitive_request_snapshot(&request);
        let compound = request
            .into_stable_compound_mutation_access()
            .expect("Stable compound specialization");

        let alternate_correspondence = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
            SchemaCorrespondenceProviderId::from_normalized_identity(287)
                .expect("alternate correspondence provider"),
            StableDeviceInstanceId::from_normalized_identity(288).expect("alternate stable device"),
            SchemaCorrespondenceSourceId::from_normalized_identity(289)
                .expect("alternate provider provenance"),
            &plan,
            profile.receipt(),
            None,
        )
        .expect("alternate provider correspondence grant")
        .admit(&plan, &profile)
        .expect("alternate schema correspondence admission");
        let mut corresponded = compound
            .into_corresponded_stable_compound_access()
            .expect("provider/device compound preflight requires retained correspondence");
        assert_eq!(corresponded.correspondence().provider(), provider);
        assert_eq!(
            primitive_request_snapshot(corresponded.compound_access().primitive_request()),
            expected
        );

        let retained_correspondence =
            corresponded.replace_correspondence_for_test(&alternate_correspondence);
        let diagnostic = corresponded
            .validate_for_provider_lowering()
            .expect_err("a distinct correspondence carrier cannot replace retained authority");
        assert!(
            diagnostic
                .0
                .contains("different schema/device correspondence")
        );
        corresponded.replace_correspondence_for_test(retained_correspondence);

        corresponded.replace_request_plan_for_test(PlacementPlanId(plan.identity().0 ^ 1));
        let diagnostic = corresponded
            .validate_for_provider_lowering()
            .expect_err("provider/device compound preflight must replay placement authority");
        assert!(diagnostic.0.contains("copied plan"));
        corresponded.replace_request_plan_for_test(plan.identity());
        corresponded
            .validate_for_provider_lowering()
            .expect("restored exact carrier remains available for retry");
        assert_eq!(
            primitive_request_snapshot(corresponded.into_compound_access().primitive_request()),
            expected
        );

        let mut ordinary_extent = uart_extent_with_lineage(0xad28, 4, 290);
        let ordinary_profile = stable_word_profile(&ordinary_extent);
        let ordinary_loan = ordinary_extent
            .loan_mut(0, 4)
            .expect("ordinary exclusive Stable loan");
        let mut ordinary = place(
            admit_placement(
                PlacementAdmissionId::from_normalized_identity(291).expect("ordinary admission"),
                ordinary_loan,
                &plan,
                &ordinary_profile,
            )
            .expect("ordinary Stable placement admission"),
        )
        .expect("ordinary Stable view establishment");
        let mut ordinary_projection = ordinary
            .project_mut(field_key(plan.access(), "word"))
            .expect("ordinary exclusive Stable projection");
        let ordinary_request = ordinary_projection
            .compound_mutation()
            .expect("ordinary Stable compound mutation")
            .into_primitive_request();
        let ordinary_snapshot = primitive_request_snapshot(&ordinary_request);
        let rejection = ordinary_request
            .into_stable_compound_mutation_access()
            .expect("ordinary compound specialization remains valid")
            .into_corresponded_stable_compound_access()
            .expect_err("provider/device preflight rejects correspondence-free Stable storage");
        assert!(rejection.diagnostic().0.contains("requires admitted"));
        let (ordinary_compound, _) = rejection.into_parts();
        assert_eq!(
            primitive_request_snapshot(ordinary_compound.primitive_request()),
            ordinary_snapshot,
            "rejection returns the exact already-specialized compound request"
        );
        ordinary_compound
            .validate_for_lowering()
            .expect("returned correspondence-free compound request remains usable elsewhere");
    }

    #[test]
    fn placed_field_authorization_replays_projection_authority_and_allows_retry() {
        let (plan, established) = established_stable_word(0xad20, 164, 165, 167);
        let mut projection = established
            .project(field_key(plan.access(), "word"))
            .expect("shared Stable projection");

        projection.plan.0 ^= 1;
        let diagnostic = projection
            .read()
            .expect_err("authorization must reject copied placement identity drift");
        assert!(diagnostic.0.contains("placed field authorization"));
        assert!(diagnostic.0.contains("retained authority"));
        projection.plan = plan.identity();

        projection.supply.offset = 4;
        let diagnostic = projection
            .read()
            .expect_err("authorization must reject copied supply-row drift");
        assert!(diagnostic.0.contains("replayed resource row"));
        projection.supply.offset = 0;

        projection.primitive_address += 4;
        let diagnostic = projection
            .read()
            .expect_err("authorization must reject copied primitive-address drift");
        assert!(
            diagnostic
                .0
                .contains("reproduce the projected primitive address")
        );
        projection.primitive_address -= 4;

        let request = projection
            .read()
            .expect("repaired projection remains authorizable")
            .into_primitive_request();
        let stable = request
            .into_stable_primitive_access()
            .expect("repaired projection remains valid through specialization");
        assert_eq!(stable.primitive_address(), 0xad20);
        let request = stable.into_primitive_request();
        assert_eq!(request.plan(), plan.identity());
        assert_eq!(request.admission().normalized_identity(), 167);
    }

    #[test]
    fn stable_primitive_specialization_replays_exact_supply_row_and_returns_custody() {
        let (plan, established) = established_stable_word(0xad40, 168, 169, 171);
        let projection = established
            .project(field_key(plan.access(), "word"))
            .expect("shared Stable projection");
        let mut request = projection
            .read()
            .expect("authorized Stable read")
            .into_primitive_request();

        request.effective_supply.key.slot ^= 1;
        request = expect_exact_stable_primitive_rejection(request, "supply key and width");
        request.effective_supply.key = request.key;

        request.effective_supply.field.push_str("_drift");
        request = expect_exact_stable_primitive_rejection(request, "field identity");
        request.effective_supply.field = request.field.clone();

        request.effective_supply.width_bits = 64;
        request = expect_exact_stable_primitive_rejection(request, "supply key and width");
        request.effective_supply.width_bits = request.transfer_width_bits;

        request.effective_supply.offset = 4;
        request = expect_exact_stable_primitive_rejection(request, "supply offset");
        request.effective_supply.offset = 0;

        request.effective_supply.alignment_bytes = 0;
        request = expect_exact_stable_primitive_rejection(request, "supply alignment");
        request.effective_supply.alignment_bytes = 4;

        request.primitive_address += 4;
        let request = expect_exact_stable_primitive_rejection(request, "supply offset");
        assert_eq!(request.admission().normalized_identity(), 171);
        drop(request);
        assert_eq!(established.validity_receipt().normalized_identity(), 169);
        assert_eq!(established.custody_receipt().normalized_identity(), 170);
    }

    #[test]
    fn placed_authorization_and_specialization_replay_resident_content_grant() {
        let (plan, established) = established_stable_word(0xad50, 220, 221, 223);

        let (replacement_extent, replacement_content) =
            provider_existing_content(&plan, 0xad50, 4, 224, 225);
        let replacement_profile = stable_word_profile(&replacement_extent);
        let replacement_admission = admit_owned_placement(
            PlacementAdmissionId::from_normalized_identity(223).expect("matching admission"),
            replacement_extent,
            &plan,
            &replacement_profile,
        )
        .expect("matching replacement placement");
        let replacement_dormant = adopt_owned_stable(replacement_admission, replacement_content)
            .expect("replacement resident adoption");
        let mut corrupt = replacement_dormant
            .view(
                PlacedOccurrenceId::from_normalized_identity(10_223).expect("matching occurrence"),
            )
            .expect("replacement resident view");
        let (_unrelated_extent, unrelated_content) =
            provider_existing_content(&plan, 0xad50, 4, 228, 229);
        corrupt.content = unrelated_content;

        let mut projection = established
            .project(field_key(plan.access(), "word"))
            .expect("shared Stable projection");
        projection._authority = PlacementAuthorityRef::EstablishedOwned(&corrupt);
        projection.resident_claim = Some(corrupt.resident_claim());
        projection.placed_occurrence = Some(corrupt.occurrence());
        let diagnostic = projection
            .read()
            .expect_err("authorization must replay resident content beyond copied identities");
        assert!(diagnostic.0.contains("resident content grant"));

        projection._authority = PlacementAuthorityRef::EstablishedOwned(&established);
        projection.resident_claim = Some(established.resident_claim());
        projection.placed_occurrence = Some(established.occurrence());
        let mut request = projection
            .read()
            .expect("repaired projection remains authorizable")
            .into_primitive_request();
        request._authority = PlacementAuthorityRef::EstablishedOwned(&corrupt);
        request.resident_claim = Some(corrupt.resident_claim());
        request.placed_occurrence = Some(corrupt.occurrence());
        request = expect_exact_stable_primitive_rejection(request, "resident content grant");

        request._authority = PlacementAuthorityRef::EstablishedOwned(&established);
        request.resident_claim = Some(established.resident_claim());
        request.placed_occurrence = Some(established.occurrence());
        let stable = request
            .into_stable_primitive_access()
            .expect("repaired resident content authority supports specialization");
        assert_eq!(stable.primitive_address(), 0xad50);
    }

    #[test]
    fn stable_primitive_specialization_replays_descriptor_geometry_and_authorization() {
        let (plan, established) = established_stable_word(0xad60, 172, 173, 175);
        let projection = established
            .project(field_key(plan.access(), "word"))
            .expect("shared Stable projection");
        let mut request = projection
            .read()
            .expect("authorized Stable read")
            .into_primitive_request();

        request.logical_extent.fragments[0].source_bit_offset ^= 1;
        request = expect_exact_stable_primitive_rejection(request, "field descriptor");
        request.logical_extent = request.descriptor.logical_extent.clone();

        request.effect_footprint.address += 4;
        request = expect_exact_stable_primitive_rejection(request, "effect footprint");
        request.effect_footprint.address = request.primitive_address;

        request.effect_footprint.length_bytes = 8;
        request = expect_exact_stable_primitive_rejection(request, "effect footprint");
        request.effect_footprint.length_bytes = request.descriptor.effect_footprint.length_bytes;

        request.operation = AccessOperation::Write;
        let request = expect_exact_stable_primitive_rejection(request, "does not permit Write");
        assert_eq!(request.admission().normalized_identity(), 175);
        drop(request);
        assert_eq!(established.validity_receipt().normalized_identity(), 173);
        assert_eq!(established.custody_receipt().normalized_identity(), 174);
    }

    #[test]
    fn stable_primitive_specialization_replays_exact_placement_authority() {
        let plan = stable_word_placement();
        let extent = uart_extent_with_lineage(0xad70, 4, 176);
        let profile = stable_word_profile(&extent);
        let loan = extent.loan(0, 4).expect("shared Stable loan");
        let admission_id = PlacementAdmissionId::from_normalized_identity(177).expect("admission");
        let admission = admit_placement(admission_id, loan, &plan, &profile)
            .expect("borrowed Stable admission");
        let view = place(admission).expect("Stable placed-view establishment");
        let projection = view
            .project(field_key(plan.access(), "word"))
            .expect("shared Stable projection");
        let mut request = projection
            .read()
            .expect("authorized Stable read")
            .into_primitive_request();

        request.plan.0 ^= 1;
        request = expect_exact_stable_primitive_rejection(request, "placement authority");
        request.plan = plan.identity();

        request.profile_receipt.0 ^= 1;
        request = expect_exact_stable_primitive_rejection(request, "placement authority");
        request.profile_receipt = profile.receipt();

        request.admission.0 ^= 1;
        request = expect_exact_stable_primitive_rejection(request, "placement authority");
        request.admission = admission_id;

        request.reach = BoundaryReach::from_services([
            BoundaryServiceReachId::from_normalized_identity(178).expect("reach"),
        ]);
        request = expect_exact_stable_primitive_rejection(request, "placement authority");
        request.reach = plan.reach().clone();

        request.source_loan = BorrowPolarity::Exclusive;
        request = expect_exact_stable_primitive_rejection(request, "source-loan");
        request.source_loan = BorrowPolarity::Shared;

        request.resident_claim =
            Some(ResidentClaimId::from_normalized_identity(179).expect("spurious resident claim"));
        request = expect_exact_stable_primitive_rejection(request, "resident identities");
        request.resident_claim = None;

        request.descriptor.field.push_str("_drift");
        request.field.push_str("_drift");
        request.effective_supply.field.push_str("_drift");
        let request = expect_exact_stable_primitive_rejection(request, "resource row");
        assert_eq!(request.admission(), admission_id);
    }

    #[test]
    fn stable_primitive_specialization_rejects_coherent_authorization_rewrite() {
        let plan = stable_word_placement();
        let mut extent = uart_extent_with_lineage(0xad74, 4, 186);
        let profile = stable_word_profile(&extent);
        let loan = extent.loan_mut(0, 4).expect("exclusive Stable loan");
        let admission_id = PlacementAdmissionId::from_normalized_identity(187).expect("admission");
        let admission = admit_placement(admission_id, loan, &plan, &profile)
            .expect("borrowed Stable admission");
        let view = place(admission).expect("Stable placed-view establishment");
        let projection = view
            .project(field_key(plan.access(), "word"))
            .expect("shared projection over exclusive source loan");
        let mut request = projection
            .read()
            .expect("authorized Stable read")
            .into_primitive_request();

        request.current_borrow = BorrowPolarity::Exclusive;
        request.operation = AccessOperation::Write;
        let request = expect_exact_stable_primitive_rejection(request, "field authorization");
        assert_eq!(request.admission(), admission_id);
        assert_eq!(
            request.authorization.current_borrow(),
            BorrowPolarity::Shared
        );
        assert_eq!(request.authorization.operation(), AccessOperation::Read);
    }

    #[test]
    fn borrowed_view_establishment_replays_profile_and_returns_admission_for_retry() {
        let plan = stable_word_placement();
        let extent = uart_extent_with_lineage(0xad7c, 4, 188);
        let profile = stable_word_profile(&extent);
        let loan = extent.loan(0, 4).expect("shared Stable loan");
        let admission_id = PlacementAdmissionId::from_normalized_identity(189).expect("admission");
        let admission = admit_placement(admission_id, loan, &plan, &profile)
            .expect("borrowed Stable admission");

        let coincident = uart_extent_with_lineage(0xad7c, 4, 190);
        let wrong_profile = stable_word_profile(&coincident);
        assert_eq!(wrong_profile.receipt(), profile.receipt());
        let PlacementAdmission {
            identity,
            placement_plan,
            profile_receipt,
            profile: _,
            resources,
            loan,
        } = admission;
        let corrupt = PlacementAdmission {
            identity,
            placement_plan,
            profile_receipt,
            profile: wrong_profile,
            resources,
            loan,
        };
        let rejection = place(corrupt)
            .expect_err("borrowed view establishment must replay admitted profile root facts");
        assert!(
            rejection
                .diagnostic()
                .0
                .contains("could not replay the admitted resource profile"),
            "{}",
            rejection.diagnostic()
        );
        let (returned, _) = rejection.into_parts();
        assert_eq!(returned.identity(), admission_id);
        assert_eq!(returned.profile_receipt(), profile.receipt());
        let PlacementAdmission {
            identity,
            placement_plan,
            profile_receipt,
            profile: _,
            resources,
            loan,
        } = returned;
        let repaired = PlacementAdmission {
            identity,
            placement_plan,
            profile_receipt,
            profile,
            resources,
            loan,
        };
        let mut view = place(repaired).expect("returned admission supports corrected retry");
        let retained_profile = view.profile.clone();
        let coincident = uart_extent_with_lineage(0xad7c, 4, 203);
        view.profile = stable_word_profile(&coincident);
        let diagnostic = view
            .project(field_key(plan.access(), "word"))
            .expect_err("field projection must replay retained placement authority");
        assert!(diagnostic.0.contains("field projection"));
        assert!(diagnostic.0.contains("retained placement authority"));
        assert_eq!(view.admission(), admission_id);
        assert_eq!(view.base(), 0xad7c);
        assert_eq!(view.length(), 4);
        view.profile = retained_profile;
        let projection = view
            .project(field_key(plan.access(), "word"))
            .expect("shared Stable projection");
        let request = projection
            .read()
            .expect("authorized Stable read")
            .into_primitive_request();
        let stable = request
            .into_stable_primitive_access()
            .expect("repaired view remains valid through specialization");
        let request = stable.into_primitive_request();
        assert_eq!(request.admission(), admission_id);
        assert_eq!(request.profile_receipt(), profile_receipt);
    }

    #[test]
    fn borrowed_view_retirement_replays_authority_and_returns_exact_loan() {
        let plan = uart_placement_plan();
        let extent = uart_extent_with_lineage(0xad88, 12, 265);
        let origin = extent.origin();
        let lineage = extent.lineage_root();
        let loan = extent.loan(0, 12).expect("shared UART loan");
        let profile = uart_resource_profile(&loan, &uart_reach());
        let admission_id =
            PlacementAdmissionId::from_normalized_identity(266).expect("placement admission");
        let admission = admit_placement(admission_id, loan, &plan, &profile)
            .expect("borrowed placement admission");
        let mut view = place(admission).expect("borrowed view establishment");
        let exact_resources = view.resources.clone();

        view.resources.fields[0].offset ^= 4;
        let rejection = view
            .retire()
            .expect_err("retirement must reject drifted resource compatibility");
        assert!(
            rejection
                .diagnostic()
                .0
                .contains("resource compatibility differs")
        );
        let (mut view, _) = rejection.into_parts();
        assert_eq!(view.admission(), admission_id);
        view.resources = exact_resources;

        let loan = view
            .retire()
            .expect("repaired view remains valid for retirement retry");
        assert_eq!(loan.origin(), origin);
        assert_eq!(loan.lineage_root(), lineage);
        assert_eq!(loan.base(), 0xad88);
        assert_eq!(loan.length(), 12);
        assert_eq!(loan.polarity(), LoanPolarity::Shared);
    }

    #[test]
    fn established_primitive_specialization_replays_resident_identities() {
        let (plan, established) = established_stable_word(0xad78, 180, 181, 183);
        let projection = established
            .project(field_key(plan.access(), "word"))
            .expect("shared established projection");
        let mut request = projection
            .read()
            .expect("authorized Stable read")
            .into_primitive_request();

        request.resident_claim =
            Some(ResidentClaimId::from_normalized_identity(184).expect("drifting resident claim"));
        request = expect_exact_stable_primitive_rejection(request, "resident identities");
        request.resident_claim = Some(established.resident_claim());

        request.placed_occurrence =
            Some(PlacedOccurrenceId::from_normalized_identity(185).expect("drifting occurrence"));
        let request = expect_exact_stable_primitive_rejection(request, "resident identities");
        drop(request);
        assert_eq!(established.validity_receipt().normalized_identity(), 181);
        assert_eq!(established.custody_receipt().normalized_identity(), 182);
    }

    #[test]
    fn stable_compound_specialization_fails_closed_and_returns_exact_request() {
        let (plan, mut established) = established_stable_word(0xad80, 164, 165, 167);
        let mut projection = established
            .project_mut(field_key(plan.access(), "word"))
            .expect("exclusive Stable projection");
        let mut request = projection
            .compound_mutation()
            .expect("authorized Stable compound mutation")
            .into_primitive_request();

        request.observation = ObservationModel::External;
        request = expect_exact_stable_compound_rejection(request, "Stable observation");
        request.observation = ObservationModel::Stable;

        request.effective_supply.kind = EffectiveSupplyKind::External;
        request = expect_exact_stable_compound_rejection(request, "Stable supply");
        request.effective_supply.kind = EffectiveSupplyKind::Stable;

        request.key.slot ^= 1;
        request = expect_exact_stable_compound_rejection(request, "supply key and width");
        request.key = request.effective_supply.key;

        request.effective_supply.width_bits = 64;
        request = expect_exact_stable_compound_rejection(request, "supply key and width");
        request.effective_supply.width_bits = request.transfer_width_bits;

        request.current_borrow = BorrowPolarity::Shared;
        request = expect_exact_stable_compound_rejection(request, "exclusive current and source");
        request.current_borrow = BorrowPolarity::Exclusive;

        request.source_loan = BorrowPolarity::Shared;
        request = expect_exact_stable_compound_rejection(request, "exclusive current and source");
        request.source_loan = BorrowPolarity::Exclusive;

        request.operation = AccessOperation::Write;
        let request =
            expect_exact_stable_compound_rejection(request, "sealed CompoundMutation event");
        assert_eq!(request.admission().normalized_identity(), 167);
        drop(request);
        assert_eq!(established.validity_receipt().normalized_identity(), 165);
        assert_eq!(established.custody_receipt().normalized_identity(), 166);
    }

    #[test]
    fn external_primitive_specialization_accepts_each_operation_and_supply_kind() {
        let plan = uart_placement_plan();
        let read_extent = uart_extent_with_lineage(0xae00, 12, 143);
        let read_loan = read_extent.loan(0, 12).expect("shared UART loan");
        let read_admission =
            admit_uart(144, read_loan, &plan, &uart_reach()).expect("External UART admission");
        let read_view = place(read_admission).expect("External read-view establishment");
        let read_projection = read_view
            .project(field_key(plan.access(), "status"))
            .expect("External status projection");
        let read_request = read_projection
            .read()
            .expect("repeatable External read")
            .into_primitive_request();
        let read = read_request
            .into_external_primitive_access()
            .expect("External read specialization");
        assert_eq!(read.operation(), ExternalPrimitiveOperation::Read);
        assert_eq!(read.primitive_address(), 0xae00);
        assert_eq!(read.transfer_width_bits(), 32);
        assert_eq!(read.effect_footprint().address(), 0xae00);
        assert_eq!(read.effect_footprint().length_bytes(), 4);
        assert_eq!(read.logical_extent().fragments().len(), 1);
        let read_request = read.into_primitive_request();
        assert_eq!(
            read_request.effective_supply().kind(),
            EffectiveSupplyKind::External
        );

        let mut write_extent = uart_extent_with_lineage(0xaf00, 12, 145);
        let write_loan = write_extent.loan_mut(0, 12).expect("exclusive UART loan");
        let stable_resources = stable_uart_resource_profile(&write_loan, &uart_reach());
        let write_admission = admit_placement(
            PlacementAdmissionId::from_normalized_identity(146).expect("admission"),
            write_loan,
            &plan,
            &stable_resources,
        )
        .expect("Stable-backed External UART admission");
        let mut write_view = place(write_admission).expect("External write-view establishment");
        let mut write_projection = write_view
            .project_mut(field_key(plan.access(), "transmit"))
            .expect("External transmit projection");
        let write_request = write_projection
            .write()
            .expect("whole External write")
            .into_primitive_request();
        let write = write_request
            .into_external_primitive_access()
            .expect("conservatively Stable-backed External write specialization");
        assert_eq!(write.operation(), ExternalPrimitiveOperation::Write);
        assert_eq!(write.primitive_address(), 0xaf04);
        let write_request = write.into_primitive_request();
        assert_eq!(
            write_request.effective_supply().kind(),
            EffectiveSupplyKind::Stable
        );
        assert_eq!(write_request.observation(), ObservationModel::External);
        assert_eq!(write_request.operation(), AccessOperation::Write);
        assert_eq!(write_request.current_borrow(), BorrowPolarity::Exclusive);
        assert_eq!(write_request.source_loan(), BorrowPolarity::Exclusive);

        let take_plan = destructive_word_placement();
        let mut take_extent = uart_extent_with_lineage(0xb000, 4, 147);
        let take_loan = take_extent.loan_mut(0, 4).expect("exclusive FIFO loan");
        let take_resources = destructive_word_profile(&take_loan);
        let take_admission = admit_placement(
            PlacementAdmissionId::from_normalized_identity(148).expect("admission"),
            take_loan,
            &take_plan,
            &take_resources,
        )
        .expect("destructive External admission");
        let mut take_view = place(take_admission).expect("External take-view establishment");
        let mut take_projection = take_view
            .project_mut(field_key(take_plan.access(), "fifo"))
            .expect("destructive External projection");
        let take_request = take_projection
            .take()
            .expect("destructive External read")
            .into_primitive_request();
        let take = take_request
            .into_external_primitive_access()
            .expect("External take specialization");
        assert_eq!(take.operation(), ExternalPrimitiveOperation::Take);
        assert_eq!(take.primitive_address(), 0xb000);
        let take_request = take.into_primitive_request();
        assert_eq!(
            take_request.effective_supply().kind(),
            EffectiveSupplyKind::External
        );
        assert_eq!(take_request.operation(), AccessOperation::Take);
        assert_eq!(take_request.current_borrow(), BorrowPolarity::Exclusive);
        assert_eq!(take_request.source_loan(), BorrowPolarity::Exclusive);
    }

    #[test]
    fn external_primitive_lowering_replays_authority_without_observing_storage() {
        let plan = uart_placement_plan();
        let extent = uart_extent_with_lineage(0xb080, 12, 232);
        let loan = extent.loan(0, 12).expect("shared UART loan");
        let admission =
            admit_uart(233, loan, &plan, &uart_reach()).expect("External UART admission");
        let view = place(admission).expect("External read-view establishment");
        let projection = view
            .project(field_key(plan.access(), "status"))
            .expect("External status projection");
        let request = projection
            .read()
            .expect("repeatable External read")
            .into_primitive_request();
        let mut external = request
            .into_external_primitive_access()
            .expect("External read specialization");
        let expected = primitive_request_snapshot(&external.request);
        let profile_receipt = external.request.profile_receipt;

        external.request.profile_receipt =
            ResourceProfileReceiptId::from_normalized_identity(999).expect("drifted receipt");
        let diagnostic = external
            .validate_for_lowering()
            .expect_err("outward preflight must reject copied receipt drift");
        assert!(diagnostic.0.contains("retained placement authority"));
        external.request.profile_receipt = profile_receipt;

        external.operation = ExternalPrimitiveOperation::Write;
        let diagnostic = external
            .validate_for_lowering()
            .expect_err("outward preflight must reject specialization drift");
        assert!(diagnostic.0.contains("retained specialization"));
        external.operation = ExternalPrimitiveOperation::Read;

        external
            .validate_for_lowering()
            .expect("corrected carrier must remain valid for retry");
        assert_eq!(primitive_request_snapshot(&external.request), expected);
        assert_eq!(external.operation(), ExternalPrimitiveOperation::Read);
    }

    #[test]
    fn external_specialization_rejection_returns_the_exact_sealed_request() {
        let (plan, established) = established_stable_word(0xb100, 149, 150, 152);
        let projection = established
            .project(field_key(plan.access(), "word"))
            .expect("shared Stable projection");
        let request = projection
            .read()
            .expect("Stable read")
            .into_primitive_request();
        let before = primitive_request_snapshot(&request);

        let rejection = request
            .into_external_primitive_access()
            .expect_err("Stable observation must not enter External lowering");
        assert!(rejection.diagnostic().0.contains("External observation"));
        let (request, diagnostic) = rejection.into_parts();
        assert!(diagnostic.0.contains("External observation"));
        assert_eq!(primitive_request_snapshot(&request), before);
        drop(request);
        assert_eq!(established.validity_receipt().normalized_identity(), 150);
        assert_eq!(established.custody_receipt().normalized_identity(), 151);
    }

    #[test]
    fn external_specialization_fails_closed_without_losing_corrupt_request_custody() {
        let plan = uart_placement_plan();
        let extent = uart_extent_with_lineage(0xb200, 12, 153);
        let loan = extent.loan(0, 12).expect("shared UART loan");
        let admission =
            admit_uart(154, loan, &plan, &uart_reach()).expect("External UART admission");
        let view = place(admission).expect("External placed-view establishment");
        let projection = view
            .project(field_key(plan.access(), "status"))
            .expect("External status projection");
        let mut request = projection
            .read()
            .expect("repeatable External read")
            .into_primitive_request();

        request.effective_supply.field.push_str("_drift");
        let field_drift = primitive_request_snapshot(&request);
        let rejection = request
            .into_external_primitive_access()
            .expect_err("drifting supply field must not enter External lowering");
        assert!(rejection.diagnostic().0.contains("field identity"));
        let (mut request, _) = rejection.into_parts();
        assert_eq!(primitive_request_snapshot(&request), field_drift);
        request.effective_supply.field = request.field.clone();

        request.effective_supply.kind = EffectiveSupplyKind::Atomic;
        let atomic_supply = primitive_request_snapshot(&request);
        let rejection = request
            .into_external_primitive_access()
            .expect_err("Atomic supply must not enter External lowering");
        assert!(
            rejection
                .diagnostic()
                .0
                .contains("External supply, or conservative Stable supply")
        );
        let (mut request, _) = rejection.into_parts();
        assert_eq!(primitive_request_snapshot(&request), atomic_supply);

        request.effective_supply.kind = EffectiveSupplyKind::Stable;
        request.operation = AccessOperation::Take;
        let stable_take = primitive_request_snapshot(&request);
        let rejection = request
            .into_external_primitive_access()
            .expect_err("Stable supply cannot satisfy a destructive External take");
        assert!(rejection.diagnostic().0.contains("for Read or Write"));
        let (mut request, _) = rejection.into_parts();
        assert_eq!(primitive_request_snapshot(&request), stable_take);

        request.effective_supply.kind = EffectiveSupplyKind::External;
        request.operation = AccessOperation::CompoundMutation;
        let compound = primitive_request_snapshot(&request);
        let rejection = request
            .into_external_primitive_access()
            .expect_err("compound mutation must not enter External lowering");
        assert!(rejection.diagnostic().0.contains("Read, Take, or Write"));
        let (request, _) = rejection.into_parts();
        assert_eq!(primitive_request_snapshot(&request), compound);
    }

    #[test]
    fn atomic_primitive_specialization_retains_all_nine_families_and_orderings() {
        let plan = atomic_word_placement();
        let extent = uart_extent_with_lineage(0xc000, 4, 156);
        let loan = extent.loan(0, 4).expect("shared Atomic loan");
        let resources = atomic_word_profile(&loan);
        let admission_id = PlacementAdmissionId::from_normalized_identity(157).expect("admission");
        let admission = admit_placement(admission_id, loan, &plan, &resources)
            .expect("all-family Atomic admission");
        let view = place(admission).expect("Atomic placed-view establishment");
        let head = view
            .project(field_key(plan.access(), "head"))
            .expect("Atomic head projection");

        let requests = [
            (
                head.atomic_load(MemoryOrdering::Receive)
                    .expect("Atomic load")
                    .into_primitive_request(),
                AtomicAccessOperation::Load(MemoryOrdering::Receive),
            ),
            (
                head.atomic_store(MemoryOrdering::Publish)
                    .expect("Atomic store")
                    .into_primitive_request(),
                AtomicAccessOperation::Store(MemoryOrdering::Publish),
            ),
            (
                head.atomic_fetch_add(MemoryOrdering::ReceivePublish)
                    .expect("Atomic fetch-add")
                    .into_primitive_request(),
                AtomicAccessOperation::FetchAdd(MemoryOrdering::ReceivePublish),
            ),
            (
                head.atomic_fetch_sub(MemoryOrdering::NoOrdering)
                    .expect("Atomic fetch-sub")
                    .into_primitive_request(),
                AtomicAccessOperation::FetchSub(MemoryOrdering::NoOrdering),
            ),
            (
                head.atomic_fetch_xor(MemoryOrdering::GlobalOrder)
                    .expect("Atomic fetch-xor")
                    .into_primitive_request(),
                AtomicAccessOperation::FetchXor(MemoryOrdering::GlobalOrder),
            ),
            (
                head.atomic_fetch_or(MemoryOrdering::Receive)
                    .expect("Atomic fetch-or")
                    .into_primitive_request(),
                AtomicAccessOperation::FetchOr(MemoryOrdering::Receive),
            ),
            (
                head.atomic_fetch_and(MemoryOrdering::Publish)
                    .expect("Atomic fetch-and")
                    .into_primitive_request(),
                AtomicAccessOperation::FetchAnd(MemoryOrdering::Publish),
            ),
            (
                head.atomic_swap(MemoryOrdering::GlobalOrder)
                    .expect("Atomic swap")
                    .into_primitive_request(),
                AtomicAccessOperation::Swap(MemoryOrdering::GlobalOrder),
            ),
            (
                head.atomic_compare_exchange(
                    MemoryOrdering::ReceivePublish,
                    MemoryOrdering::Receive,
                )
                .expect("Atomic compare-exchange")
                .into_primitive_request(),
                AtomicAccessOperation::CompareExchange {
                    success: MemoryOrdering::ReceivePublish,
                    failure: MemoryOrdering::Receive,
                },
            ),
        ];
        for (request, operation) in requests {
            assert_atomic_specialization(request, operation, plan.identity(), admission_id);
        }
    }

    #[test]
    fn atomic_primitive_lowering_replays_authority_and_ordering_without_attempt() {
        let plan = atomic_word_placement();
        let extent = uart_extent_with_lineage(0xc080, 4, 234);
        let loan = extent.loan(0, 4).expect("shared Atomic loan");
        let resources = atomic_word_profile(&loan);
        let admission = admit_placement(
            PlacementAdmissionId::from_normalized_identity(235).expect("admission"),
            loan,
            &plan,
            &resources,
        )
        .expect("all-family Atomic admission");
        let view = place(admission).expect("Atomic placed-view establishment");
        let head = view
            .project(field_key(plan.access(), "head"))
            .expect("Atomic head projection");
        let request = head
            .atomic_load(MemoryOrdering::Receive)
            .expect("Atomic load")
            .into_primitive_request();
        let mut atomic = request
            .into_atomic_primitive_access()
            .expect("Atomic load specialization");
        let expected = primitive_request_snapshot(&atomic.request);
        let profile_receipt = atomic.request.profile_receipt;

        atomic.request.profile_receipt =
            ResourceProfileReceiptId::from_normalized_identity(999).expect("drifted receipt");
        let diagnostic = atomic
            .validate_for_lowering()
            .expect_err("outward preflight must reject copied receipt drift");
        assert!(diagnostic.0.contains("retained placement authority"));
        atomic.request.profile_receipt = profile_receipt;

        atomic.request.operation =
            AccessOperation::Atomic(AtomicAccessOperation::Load(MemoryOrdering::Publish));
        let diagnostic = atomic
            .validate_for_lowering()
            .expect_err("outward preflight must reject invalid ordering drift");
        assert!(diagnostic.0.contains("invalid ordering plan"));
        atomic.request.operation =
            AccessOperation::Atomic(AtomicAccessOperation::Load(MemoryOrdering::Receive));

        atomic.operation = AtomicAccessOperation::FetchAdd(MemoryOrdering::Receive);
        let diagnostic = atomic
            .validate_for_lowering()
            .expect_err("outward preflight must reject specialization drift");
        assert!(diagnostic.0.contains("retained specialization"));
        atomic.operation = AtomicAccessOperation::Load(MemoryOrdering::Receive);

        atomic
            .validate_for_lowering()
            .expect("corrected carrier must remain valid for retry");
        assert_eq!(primitive_request_snapshot(&atomic.request), expected);
        assert_eq!(
            atomic.operation(),
            AtomicAccessOperation::Load(MemoryOrdering::Receive)
        );
    }

    #[test]
    fn provider_atomic_preflight_requires_and_retains_exact_correspondence() {
        let plan = atomic_word_placement();
        let extent = uart_extent_with_lineage(0xc0c0, 4, 262);
        let loan = extent.loan(0, 4).expect("shared Atomic loan");
        let profile = atomic_word_profile(&loan);
        let admission = admit_placement(
            PlacementAdmissionId::from_normalized_identity(263).expect("admission"),
            loan,
            &plan,
            &profile,
        )
        .expect("Atomic placement admission");
        let provider = SchemaCorrespondenceProviderId::from_normalized_identity(264)
            .expect("correspondence provider");
        let device = StableDeviceInstanceId::from_normalized_identity(265).expect("stable device");
        let correspondence = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
            provider,
            device,
            SchemaCorrespondenceSourceId::from_normalized_identity(266)
                .expect("datasheet provenance"),
            &plan,
            profile.receipt(),
            None,
        )
        .expect("provider correspondence grant")
        .admit(&plan, &profile)
        .expect("schema correspondence admission");
        let view = bind_schema_correspondence_to_placement(admission, correspondence)
            .expect("correspondence placement binding")
            .establish_view()
            .expect("corresponded view establishment");
        let head = view
            .project(field_key(plan.access(), "head"))
            .expect("Atomic head projection");
        let request = head
            .atomic_fetch_add(MemoryOrdering::ReceivePublish)
            .expect("Atomic fetch-add")
            .into_primitive_request();
        let expected = primitive_request_snapshot(&request);
        let atomic = request
            .into_atomic_primitive_access()
            .expect("Atomic fetch-add specialization");

        let alternate_correspondence = SchemaDeviceCorrespondenceGrant::from_admitted_provider(
            SchemaCorrespondenceProviderId::from_normalized_identity(267)
                .expect("alternate correspondence provider"),
            StableDeviceInstanceId::from_normalized_identity(268).expect("alternate stable device"),
            SchemaCorrespondenceSourceId::from_normalized_identity(269)
                .expect("alternate datasheet provenance"),
            &plan,
            profile.receipt(),
            None,
        )
        .expect("alternate provider correspondence grant")
        .admit(&plan, &profile)
        .expect("alternate schema correspondence admission");
        let mut corresponded = atomic
            .into_corresponded_atomic_access()
            .expect("provider/device Atomic preflight requires retained correspondence");
        assert_eq!(corresponded.correspondence().provider(), provider);
        assert_eq!(
            corresponded.atomic_access().operation(),
            AtomicAccessOperation::FetchAdd(MemoryOrdering::ReceivePublish)
        );
        assert_eq!(
            primitive_request_snapshot(corresponded.atomic_access().primitive_request()),
            expected
        );

        let retained_correspondence =
            corresponded.replace_correspondence_for_test(&alternate_correspondence);
        let diagnostic = corresponded
            .validate_for_provider_lowering()
            .expect_err("a distinct correspondence carrier cannot replace retained authority");
        assert!(
            diagnostic
                .0
                .contains("different schema/device correspondence")
        );
        corresponded.replace_correspondence_for_test(retained_correspondence);

        corresponded.replace_request_plan_for_test(PlacementPlanId(plan.identity().0 ^ 1));
        let diagnostic = corresponded
            .validate_for_provider_lowering()
            .expect_err("provider/device Atomic preflight must replay placement authority");
        assert!(diagnostic.0.contains("copied plan"));
        corresponded.replace_request_plan_for_test(plan.identity());
        corresponded
            .validate_for_provider_lowering()
            .expect("restored exact carrier remains available for retry");
        assert_eq!(
            primitive_request_snapshot(corresponded.into_atomic_access().primitive_request()),
            expected
        );

        let ordinary_extent = uart_extent_with_lineage(0xc0d0, 4, 270);
        let ordinary_loan = ordinary_extent
            .loan(0, 4)
            .expect("ordinary shared Atomic loan");
        let ordinary_profile = atomic_word_profile(&ordinary_loan);
        let ordinary = place(
            admit_placement(
                PlacementAdmissionId::from_normalized_identity(271).expect("ordinary admission"),
                ordinary_loan,
                &plan,
                &ordinary_profile,
            )
            .expect("ordinary Atomic placement admission"),
        )
        .expect("ordinary Atomic view establishment");
        let ordinary_projection = ordinary
            .project(field_key(plan.access(), "head"))
            .expect("ordinary Atomic projection");
        let ordinary_request = ordinary_projection
            .atomic_load(MemoryOrdering::Receive)
            .expect("ordinary Atomic load")
            .into_primitive_request();
        let ordinary_snapshot = primitive_request_snapshot(&ordinary_request);
        let rejection = ordinary_request
            .into_atomic_primitive_access()
            .expect("ordinary Atomic specialization remains valid")
            .into_corresponded_atomic_access()
            .expect_err("provider/device preflight rejects correspondence-free atomic storage");
        assert!(rejection.diagnostic().0.contains("requires admitted"));
        let (ordinary_atomic, _) = rejection.into_parts();
        assert_eq!(
            primitive_request_snapshot(ordinary_atomic.primitive_request()),
            ordinary_snapshot,
            "rejection returns the exact already-specialized Atomic request"
        );
        ordinary_atomic
            .validate_for_lowering()
            .expect("returned correspondence-free Atomic request remains usable elsewhere");
    }

    #[test]
    fn atomic_specialization_fails_closed_and_returns_exact_request() {
        let plan = atomic_word_placement();
        let extent = uart_extent_with_lineage(0xc100, 4, 158);
        let loan = extent.loan(0, 4).expect("shared Atomic loan");
        let resources = atomic_word_profile(&loan);
        let admission = admit_placement(
            PlacementAdmissionId::from_normalized_identity(159).expect("admission"),
            loan,
            &plan,
            &resources,
        )
        .expect("all-family Atomic admission");
        let view = place(admission).expect("Atomic placed-view establishment");
        let head = view
            .project(field_key(plan.access(), "head"))
            .expect("Atomic head projection");
        let mut request = head
            .atomic_load(MemoryOrdering::NoOrdering)
            .expect("Atomic load")
            .into_primitive_request();

        request.observation = ObservationModel::Stable;
        request = expect_exact_atomic_rejection(request, "Atomic observation");
        request.observation = ObservationModel::Atomic;

        request.effective_supply.kind = EffectiveSupplyKind::External;
        request = expect_exact_atomic_rejection(request, "Atomic supply");
        request.effective_supply.kind = EffectiveSupplyKind::Atomic;

        request.key.slot ^= 1;
        request = expect_exact_atomic_rejection(request, "supply key and width");
        request.key = request.effective_supply.key;

        request.effective_supply.width_bits = 64;
        request = expect_exact_atomic_rejection(request, "supply key and width");
        request.effective_supply.width_bits = request.transfer_width_bits;

        request.operation = AccessOperation::Read;
        request = expect_exact_atomic_rejection(request, "sealed Atomic operation");

        request.operation =
            AccessOperation::Atomic(AtomicAccessOperation::Load(MemoryOrdering::Publish));
        request = expect_exact_atomic_rejection(request, "invalid ordering plan");
        request.operation =
            AccessOperation::Atomic(AtomicAccessOperation::Store(MemoryOrdering::Receive));
        request = expect_exact_atomic_rejection(request, "invalid ordering plan");
        request.operation = AccessOperation::Atomic(AtomicAccessOperation::CompareExchange {
            success: MemoryOrdering::Receive,
            failure: MemoryOrdering::GlobalOrder,
        });
        let request = expect_exact_atomic_rejection(request, "invalid ordering plan");
        assert_eq!(request.admission().normalized_identity(), 159);
    }

    #[test]
    fn external_request_rejects_stable_specialization_and_returns_exact_request() {
        let plan = uart_placement_plan();
        let extent = uart_extent(0xb000, 12);
        let loan = extent.loan(0, 12).expect("shared UART loan");
        let admission =
            admit_uart(132, loan, &plan, &uart_reach()).expect("admitted shared UART view");
        let view = place(admission).expect("shared UART placed-view establishment");
        let projection = view
            .project(field_key(plan.access(), "status"))
            .expect("External status projection");
        let request = projection
            .read()
            .expect("External status read")
            .into_primitive_request();

        let rejection = request
            .into_stable_primitive_access()
            .expect_err("External observation must not enter Stable lowering");
        assert!(rejection.diagnostic().0.contains("Stable observation"));
        let (request, diagnostic) = rejection.into_parts();
        assert!(diagnostic.0.contains("Stable observation"));
        assert_eq!(request.plan(), plan.identity());
        assert_eq!(request.admission().normalized_identity(), 132);
        assert_eq!(request.primitive_address(), 0xb000);
        assert_eq!(request.observation(), ObservationModel::External);
        assert_eq!(request.operation(), AccessOperation::Read);
        assert_eq!(request.source_loan(), BorrowPolarity::Shared);
    }

    #[test]
    fn compound_request_rejects_stable_specialization_and_returns_custody() {
        let (plan, mut established) = established_stable_word(0xb100, 133, 134, 136);
        let mut projection = established
            .project_mut(field_key(plan.access(), "word"))
            .expect("exclusive Stable projection");
        let request = projection
            .compound_mutation()
            .expect("authorized Stable compound mutation")
            .into_primitive_request();

        let rejection = request
            .into_stable_primitive_access()
            .expect_err("compound mutation needs its distinct bounded lowering");
        assert!(rejection.diagnostic().0.contains("Read or Write"));
        let (request, diagnostic) = rejection.into_parts();
        assert!(diagnostic.0.contains("Read or Write"));
        assert_eq!(request.plan(), plan.identity());
        assert_eq!(request.admission().normalized_identity(), 136);
        assert_eq!(request.operation(), AccessOperation::CompoundMutation);
        drop(request);
        assert_eq!(established.validity_receipt().normalized_identity(), 134);
        assert_eq!(established.custody_receipt().normalized_identity(), 135);
    }

    #[test]
    fn provider_existing_content_cannot_replay_across_extent_roots() {
        let plan = stable_word_placement();
        let (_source_extent, content) = provider_existing_content(&plan, 0xa100, 4, 96, 97);
        let coincident = uart_extent_with_lineage(0xa100, 4, 98);
        let returned_origin = coincident.origin();
        let returned_lineage = coincident.lineage_root();
        let profile = stable_word_profile(&coincident);
        let admission = admit_owned_placement(
            PlacementAdmissionId::from_normalized_identity(99).expect("admission"),
            coincident,
            &plan,
            &profile,
        )
        .expect("coincident root admission");

        let rejection = adopt_owned_stable(admission, content)
            .expect_err("existing-content authority must not replay across roots");
        assert!(rejection.diagnostic().0.contains("lineage"));
        let (admission, content, diagnostic) = rejection.into_parts();
        assert!(diagnostic.0.contains("lineage"));
        assert_eq!(content.lineage_root().normalized_identity(), 96);
        assert_eq!(content.resident_claim().normalized_identity(), 99);
        let returned = admission.withdraw();
        assert_eq!(returned.origin(), returned_origin);
        assert_eq!(returned.lineage_root(), returned_lineage);
    }

    #[test]
    fn provider_existing_content_cannot_replay_after_mapping_era_drift() {
        let plan = stable_word_placement();
        let (_source_extent, content) = provider_existing_content(&plan, 0xa180, 4, 108, 109);
        let drifted = uart_root_grant_with_mapping(1, 108, 5, 110)
            .mint(0xa180, 4)
            .expect("same-root geometry in a later mapping era");
        assert_eq!(content.origin(), drifted.origin());
        assert_eq!(content.lineage_root(), drifted.lineage_root());
        assert_eq!(
            (content.base(), content.length()),
            (drifted.base(), drifted.length())
        );
        assert_eq!(content.address_space(), drifted.address_space());
        assert_eq!(content.provenance(), drifted.provenance());
        assert_ne!(content.era(), drifted.era());
        let profile = stable_word_profile(&drifted);
        let admission = admit_owned_placement(
            PlacementAdmissionId::from_normalized_identity(111).expect("admission"),
            drifted,
            &plan,
            &profile,
        )
        .expect("drifted mapping admission");

        let rejection = adopt_owned_stable(admission, content)
            .expect_err("existing-content authority must not replay after mapping-era drift");
        assert!(rejection.diagnostic().0.contains("mapping era"));
        let (admission, content, _) = rejection.into_parts();
        assert_eq!(content.era().normalized_identity(), 6);
        assert_eq!(admission.withdraw().era().normalized_identity(), 110);
    }

    #[test]
    fn provider_existing_content_must_name_the_actual_placement() {
        let plan = stable_word_placement();
        let (extent, content) = uart_root_grant(1, 100)
            .mint_provider_existing_content(
                0xa200,
                4,
                extent_id(
                    plan.identity().normalized_identity() + 1,
                    psi_extents::ExtentContentInterpretationId::from_normalized_identity,
                ),
                extent_id(104, ResidentClaimId::from_normalized_identity),
                extent_id(
                    101,
                    ExtentContentValidityReceiptId::from_normalized_identity,
                ),
                extent_id(102, ExtentContentCustodyReceiptId::from_normalized_identity),
            )
            .expect("provider existing-content extent");
        let profile = stable_word_profile(&extent);
        let admission = admit_owned_placement(
            PlacementAdmissionId::from_normalized_identity(103).expect("admission"),
            extent,
            &plan,
            &profile,
        )
        .expect("owned Stable admission");

        let rejection = adopt_owned_stable(admission, content)
            .expect_err("provider interpretation must match the actual admitted placement");
        assert!(rejection.diagnostic().0.contains("interpretation"));
    }

    #[test]
    fn provider_content_does_not_turn_external_placement_into_stable_adoption() {
        let plan = uart_placement_plan();
        let (extent, content) = provider_existing_content(&plan, 0xa300, 12, 104, 105);
        let profile = uart_resource_profile_for_extent(&extent, &uart_reach());
        let admission = admit_owned_placement(
            PlacementAdmissionId::from_normalized_identity(107).expect("admission"),
            extent,
            &plan,
            &profile,
        )
        .expect("owned External admission");

        let rejection = adopt_owned_stable(admission, content)
            .expect_err("External observation needs its distinct adopt route");
        assert!(
            rejection.diagnostic().0.contains("External")
                && rejection.diagnostic().0.contains("Stable adoption")
        );
    }

    #[test]
    fn placed_view_derives_access_from_extent_provenance_and_actual_borrow() {
        let plan = uart_placement_plan();

        let mut shared_extent = uart_extent(0x1000, 64);
        let shared_loan = shared_extent.loan(0, 12).expect("shared UART loan");
        let admission =
            admit_uart(8, shared_loan, &plan, &uart_reach()).expect("admitted shared view");
        let mut shared_view = place(admission).expect("shared placed-view establishment");
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
            assert_eq!(
                request.effect_footprint(),
                EffectFootprint {
                    address: 0x1000,
                    length_bytes: 4,
                }
            );
            assert_eq!(request.observation(), ObservationModel::External);
            assert_eq!(request.current_borrow(), BorrowPolarity::Shared);
            assert_eq!(request.source_loan(), BorrowPolarity::Shared);
            assert_eq!(request.operation(), AccessOperation::Read);
            assert_eq!(request.resident_claim(), None);
            assert_eq!(request.placed_occurrence(), None);
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
        let mut exclusive_view = place(admission).expect("exclusive placed-view establishment");
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
        let resources = ResourceProfileGrant::from_admitted_provider(
            ResourceProfileReceiptId::from_normalized_identity(51).expect("profile receipt"),
            &extent,
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
        let loan = extent.loan_mut(0, 16).expect("exclusive placed loan");
        let admission = admit_placement(
            PlacementAdmissionId::from_normalized_identity(52).expect("admission"),
            loan,
            &placement,
            &resources,
        )
        .expect("heterogeneous placement admission");
        let mut view = place(admission).expect("heterogeneous placed-view establishment");

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
                    .atomic_load(MemoryOrdering::Receive)
                    .expect("atomic load")
                    .access()
                    .operation(),
                AccessOperation::Atomic(AtomicAccessOperation::Load(MemoryOrdering::Receive))
            );
            assert_eq!(
                counter
                    .atomic_fetch_add(MemoryOrdering::ReceivePublish)
                    .expect("atomic fetch-add")
                    .access()
                    .operation(),
                AccessOperation::Atomic(AtomicAccessOperation::FetchAdd(
                    MemoryOrdering::ReceivePublish
                ))
            );
            assert!(
                counter
                    .atomic_fetch_sub(MemoryOrdering::ReceivePublish)
                    .is_err(),
                "unlisted atomic families must remain absent"
            );
            assert!(
                counter.atomic_load(MemoryOrdering::Publish).is_err(),
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
            error.0.contains("transmit") && error.0.contains("incompatible External"),
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
            &extent,
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
            let view = place(admission).expect("split placed-view establishment");
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
        drop(rejection);

        let partition = extent
            .partition_owned(4, 4)
            .expect("owned subrange partition");
        {
            let loan = partition
                .selected()
                .loan(0, 4)
                .expect("selected split loan");
            let admission = admit_placement(
                PlacementAdmissionId::from_normalized_identity(44).expect("admission"),
                loan,
                &placement,
                &profile,
            )
            .expect("a conserved split must retain its root profile binding");
            let view = place(admission).expect("split placed-view establishment");
            assert_eq!(view.base(), 0x4004);
        }
        let restored = partition.rejoin();
        assert_eq!(restored.base(), 0x4000);
        assert_eq!(restored.length(), 16);
    }

    #[test]
    fn admitted_profile_rejects_coincident_independent_extent_root() {
        let plan = uart_placement_plan();
        let admitted_root = uart_extent_with_lineage(0x6000, 12, 61);
        let profile = ResourceProfileGrant::from_admitted_provider(
            ResourceProfileReceiptId::from_normalized_identity(62).expect("profile receipt"),
            &admitted_root,
            extent_rights(&[3]),
            uart_reach(),
        )
        .expect("root-bound profile grant")
        .admit(uart_resource_profile_data(12, &uart_reach()))
        .expect("admitted root-bound profile");

        let foreign_origin = uart_extent_with_root(0x6000, 12, 2, 61);
        assert_ne!(foreign_origin.origin(), admitted_root.origin());
        assert_eq!(foreign_origin.lineage_root(), admitted_root.lineage_root());
        let loan = foreign_origin
            .loan(0, 12)
            .expect("coincident foreign-origin loan");
        let rejection = admit_placement(
            PlacementAdmissionId::from_normalized_identity(63).expect("admission"),
            loan,
            &plan,
            &profile,
        )
        .expect_err("coincident geometry and lineage must not replay another origin's profile");
        assert!(
            rejection.diagnostic().0.contains("sealed root origin"),
            "cross-origin replay must identify the sealed-origin mismatch"
        );

        let coincident_root = uart_extent_with_lineage(0x6000, 12, 63);
        assert_eq!(coincident_root.origin(), admitted_root.origin());
        assert_ne!(coincident_root.lineage_root(), admitted_root.lineage_root());
        let loan = coincident_root
            .loan(0, 12)
            .expect("coincident independent loan");
        let rejection = admit_placement(
            PlacementAdmissionId::from_normalized_identity(64).expect("admission"),
            loan,
            &plan,
            &profile,
        )
        .expect_err("coincident geometry and provenance must not replay another root's profile");
        assert!(
            rejection.diagnostic().0.contains("root lineage"),
            "cross-root replay must identify the root-lineage mismatch"
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
            &extent,
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
        let extent = uart_extent(0x3000, 12);
        let profile = ResourceProfileGrant::from_admitted_provider(
            ResourceProfileReceiptId::from_normalized_identity(31).expect("profile receipt"),
            &extent,
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
        let extent = extent
            .attenuate(extent_rights(&[3]))
            .expect("attenuated extent");
        let loan = extent.loan(0, 12).expect("UART loan");
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

    #[test]
    fn effect_conflicts_use_whole_transfer_containers() {
        let word = EffectFootprint {
            address: 0x1000,
            length_bytes: 4,
        };
        let overlapping_half = EffectFootprint {
            address: 0x1002,
            length_bytes: 2,
        };
        let next_word = EffectFootprint {
            address: 0x1004,
            length_bytes: 4,
        };

        assert!(!effect_footprints_conflict(
            word,
            AccessOperation::Read,
            word,
            AccessOperation::Read,
        ));
        assert!(effect_footprints_conflict(
            word,
            AccessOperation::Read,
            word,
            AccessOperation::Take,
        ));
        assert!(effect_footprints_conflict(
            word,
            AccessOperation::CompoundMutation,
            word,
            AccessOperation::Read,
        ));

        let atomic_load =
            AccessOperation::Atomic(AtomicAccessOperation::Load(MemoryOrdering::Receive));
        let atomic_store =
            AccessOperation::Atomic(AtomicAccessOperation::Store(MemoryOrdering::Publish));
        assert!(!effect_footprints_conflict(
            word,
            atomic_load,
            word,
            atomic_store,
        ));
        assert!(effect_footprints_conflict(
            word,
            atomic_load,
            overlapping_half,
            atomic_store,
        ));
        assert!(!effect_footprints_conflict(
            word,
            AccessOperation::Write,
            next_word,
            AccessOperation::Write,
        ));
    }
}
