#![forbid(unsafe_code)]

//! Normalized access policy for placed views.
//!
//! `LayoutPlan` owns geometry. `AccessPlan` owns observation and the exact
//! primitive operations permitted over that geometry. Keeping them separate
//! prevents wire layouts from acquiring MMIO vocabulary and prevents an
//! arbitrary-offset volatile escape hatch from bypassing plan validation.

use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
use psi_extents::LoanPolarity;
use psi_extents::{Extent, ExtentLoan, ProviderExistingContentGrant, ResidentClaimId};
use psi_language_core::atomic::{AtomicOrderingPlan, MemoryOrdering};
use psi_layout_plans::{LayoutPlanReport, normalized_layout_plan_fingerprint};

mod access_plan_validation;
mod atomic_resident_views;
mod authorization;
mod borrowed_view;
mod corresponded_atomic;
mod corresponded_external;
mod corresponded_stable;
mod corresponded_stable_compound;
mod field_projection;
mod normalized_identities;
mod owned_atomic_resident_custody;
mod owned_placement_lifecycle;
mod owned_resident_custody;
mod placement_admission;
mod placement_authority;
mod primitive_request;
mod primitive_specialization;
mod resident_views;
mod resource_compatibility;
mod resource_profile_admission;
mod resource_profile_validation;
mod schema_correspondence;

pub use access_plan_validation::validate_access_plan;
pub use atomic_resident_views::{
    BorrowedAtomicResidentRetirementError, EstablishedBorrowedAtomicResidentPlacement,
};
pub use authorization::effect_footprints_conflict;
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
pub use field_projection::{PlacedFieldAccess, PlacedFieldProjection};
pub use owned_atomic_resident_custody::{
    DormantOwnedAtomicResident, EstablishedOwnedAtomicPlacement, OwnedAtomicAdoptionError,
    OwnedAtomicResidentRetirementError, OwnedAtomicResidentViewEstablishmentError,
    adopt_owned_atomic,
};
pub use owned_resident_custody::adopt_owned_stable;
pub use placement_admission::{admit_owned_placement, admit_placement, place};
pub use primitive_specialization::{
    AtomicPrimitiveAccessRejection, AtomicPrimitiveAccessRequest, ExternalPrimitiveAccessRejection,
    ExternalPrimitiveAccessRequest, ExternalPrimitiveOperation,
    StableCompoundMutationAccessRejection, StableCompoundMutationAccessRequest,
    StablePrimitiveAccessRejection, StablePrimitiveAccessRequest, StablePrimitiveOperation,
};
pub use resident_views::{BorrowedResidentRetirementError, EstablishedBorrowedResidentPlacement};
pub use resource_compatibility::validate_placement_resources;
pub use resource_profile_admission::{
    AdmittedResourceProfile, ResourceProfileAdmissionError, ResourceProfileGrant,
    ResourceProfileReceiptId,
};
pub use resource_profile_validation::validate_resource_profile;
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

#[cfg(test)]
use access_plan_validation::validate_entry_geometry;
use authorization::{authorize_descriptor, validate_operation_ordering};
use field_projection::project_placed_field;
use normalized_identities::{
    normalized_access_plan_identity, normalized_placement_plan_identity,
    normalized_resource_profile_identity,
};
use owned_resident_custody::{
    replay_owned_admission_resources, validate_owned_content_binding,
    validate_owned_resident_authority, validate_provider_content_binding,
};
use placement_admission::validate_placement_admission;
use placement_authority::PlacementAuthorityRef;

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
    CompareExchangeOnce {
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
            Self::CompareExchangeOnce { success, failure } => {
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

/// Failed owned admission returns the exact moved Extent rather than losing
/// or reconstructing its authority account.
#[derive(Debug)]
pub struct OwnedPlacementRejection {
    extent: Extent,
    diagnostic: AccessPlanDiagnostic,
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

/// Failed Stable adoption preserves both linear inputs for a corrected retry
/// or explicit cancellation.
#[derive(Debug)]
pub struct OwnedStableAdoptionError {
    admission: OwnedPlacementAdmission,
    content: ProviderExistingContentGrant,
    diagnostic: AccessPlanDiagnostic,
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

/// Failed ordinary borrowed-view retirement preserves the complete
/// loan-bearing view for corrected retry.
#[derive(Debug)]
pub struct PlacedViewRetirementError<'extent> {
    view: PlacedView<'extent>,
    diagnostic: AccessPlanDiagnostic,
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

#[cfg(test)]
mod tests;
