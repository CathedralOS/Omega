//! Normalized access policy for placed views.
//!
//! `LayoutPlan` owns geometry. `AccessPlan` owns observation and the exact
//! primitive operations permitted over that geometry. Keeping them separate
//! prevents wire layouts from acquiring MMIO vocabulary and prevents an
//! arbitrary-offset volatile escape hatch from bypassing plan validation.

use std::collections::{BTreeMap, BTreeSet};

use omega_core::atomic::{AtomicOrderingPlan, MemoryOrdering};
use omega_extents::{AddressSpaceId, ExtentLoan, ExtentProvenanceId, ExtentRights, LoanPolarity};
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
        /// Bootstrap owner for reach until `PlacementPlan` carries this fact.
        service_reach: BoundaryServiceReachId,
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum CanonicalFieldIdentity {
    Numbered(u64),
    Positional(String),
}

/// Normalizer-owned identity of one validated access policy.
///
/// The plan contains exactly one canonical slot per layout schema field,
/// including inaccessible fields. Its identity includes every operation,
/// observation, exposure, transfer-width, and temporary per-entry reach fact
/// that lowering is allowed to consume.
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
    service_reach: Option<BoundaryServiceReachId>,
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

    pub const fn service_reach(&self) -> Option<BoundaryServiceReachId> {
        self.service_reach
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
    borrow: BorrowPolarity,
    operation: AccessOperation,
}

impl AuthorizedFieldAccess {
    pub const fn descriptor(&self) -> &FieldAccessDescriptor {
        &self.descriptor
    }

    pub const fn borrow(&self) -> BorrowPolarity {
        self.borrow
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
        borrow: BorrowPolarity,
        operation: AccessOperation,
    ) -> Result<AuthorizedFieldAccess, AccessPlanDiagnostic> {
        let entry = self.field(key).ok_or_else(|| {
            AccessPlanDiagnostic("field key does not belong to the validated access plan".into())
        })?;
        let descriptor = self.field_descriptor(key).ok_or_else(|| {
            AccessPlanDiagnostic(format!("field `{}` is inaccessible", entry.field))
        })?;
        authorize_descriptor(descriptor, borrow, operation)?;
        Ok(AuthorizedFieldAccess {
            descriptor: descriptor.clone(),
            borrow,
            operation,
        })
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
            service_reach: policy.service_reach,
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
                service_reach,
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
                let reach = *service_reach;
                hash_u64(&mut hash, reach.normalized_identity());
            }
            FieldAccess::Atomic {
                transfer_width_bits,
                operations,
                exposure,
            } => {
                hash_byte(&mut hash, 3);
                hash_u64(&mut hash, u64::from(*transfer_width_bits));
                for enabled in [
                    operations.load,
                    operations.store,
                    operations.fetch_add,
                    operations.fetch_sub,
                    operations.fetch_xor,
                    operations.fetch_or,
                    operations.fetch_and,
                    operations.swap,
                    operations.compare_exchange,
                ] {
                    hash_byte(&mut hash, u8::from(enabled));
                }
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
pub struct PlacedViewGrantId(u64);

impl PlacedViewGrantId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, AccessPlanDiagnostic> {
        if identity == 0 {
            return Err(AccessPlanDiagnostic(
                "placed-view grant identity cannot be zero".into(),
            ));
        }
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

/// Provider-admitted agreement between an extent provenance and a static
/// access policy. It is reusable; the borrow-carrying extent loan supplies the
/// per-view lifetime and polarity. The complete canonical plan is retained:
/// its compact identity is useful for reports and caches, but is never the sole
/// authorization check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedViewGrant {
    identity: PlacedViewGrantId,
    access_plan: ValidatedAccessPlan,
    address_space: AddressSpaceId,
    provenance: ExtentProvenanceId,
    required_rights: ExtentRights,
    permitted_reaches: BTreeSet<BoundaryServiceReachId>,
}

impl PlacedViewGrant {
    pub fn from_admitted_provider(
        identity: PlacedViewGrantId,
        access_plan: &ValidatedAccessPlan,
        address_space: AddressSpaceId,
        provenance: ExtentProvenanceId,
        required_rights: ExtentRights,
        permitted_reaches: impl IntoIterator<Item = BoundaryServiceReachId>,
    ) -> Self {
        Self {
            identity,
            access_plan: access_plan.clone(),
            address_space,
            provenance,
            required_rights,
            permitted_reaches: permitted_reaches.into_iter().collect(),
        }
    }

    pub const fn identity(&self) -> PlacedViewGrantId {
        self.identity
    }
}

/// A plan-qualified interpretation of one borrowed concrete range.
#[derive(Debug)]
pub struct PlacedView<'extent, 'plan> {
    loan: ExtentLoan<'extent>,
    plan: &'plan ValidatedAccessPlan,
    grant: PlacedViewGrantId,
}

impl<'extent, 'plan> PlacedView<'extent, 'plan> {
    pub const fn grant(&self) -> PlacedViewGrantId {
        self.grant
    }

    pub const fn base(&self) -> u64 {
        self.loan.base()
    }

    pub const fn length(&self) -> u64 {
        self.loan.length()
    }

    pub fn authorize<'view>(
        &'view self,
        key: AccessFieldKey,
        operation: AccessOperation,
    ) -> Result<PlacedFieldAccess<'view, 'extent>, AccessPlanDiagnostic> {
        let borrow = match self.loan.polarity() {
            LoanPolarity::Shared => BorrowPolarity::Shared,
            LoanPolarity::Exclusive => BorrowPolarity::Exclusive,
        };
        let access = self.plan.authorize(key, borrow, operation)?;
        let primitive_address = self
            .loan
            .base()
            .checked_add(access.descriptor().container_byte_offset())
            .ok_or_else(|| {
                AccessPlanDiagnostic(format!(
                    "field `{}` primitive address overflows address width",
                    access.descriptor().field()
                ))
            })?;
        Ok(PlacedFieldAccess {
            access,
            primitive_address,
            plan: self.plan.identity(),
            grant: self.grant,
            _loan: &self.loan,
        })
    }
}

/// Sealed lowering input carrying both authorized field geometry and the
/// actual extent borrow from which its polarity was derived.
#[derive(Debug)]
pub struct PlacedFieldAccess<'view, 'extent> {
    access: AuthorizedFieldAccess,
    primitive_address: u64,
    plan: AccessPlanId,
    grant: PlacedViewGrantId,
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
    /// The request remains bound to the normalized plan, admitted grant,
    /// exact address, width, observation model, operation ordering, and static
    /// service reach that produced it. It contains no author-supplied offset.
    pub fn into_primitive_request(self) -> PrimitiveAccessRequest<'view, 'extent> {
        let AuthorizedFieldAccess {
            descriptor,
            borrow,
            operation,
        } = self.access;
        PrimitiveAccessRequest {
            plan: self.plan,
            grant: self.grant,
            primitive_address: self.primitive_address,
            field: descriptor.field,
            transfer_width_bits: descriptor.transfer_width_bits,
            observation: descriptor.observation,
            borrow,
            operation,
            service_reach: descriptor.service_reach,
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
    plan: AccessPlanId,
    grant: PlacedViewGrantId,
    primitive_address: u64,
    field: String,
    transfer_width_bits: u16,
    observation: ObservationModel,
    borrow: BorrowPolarity,
    operation: AccessOperation,
    service_reach: Option<BoundaryServiceReachId>,
    _loan: &'view ExtentLoan<'extent>,
}

impl PrimitiveAccessRequest<'_, '_> {
    pub const fn plan(&self) -> AccessPlanId {
        self.plan
    }

    pub const fn grant(&self) -> PlacedViewGrantId {
        self.grant
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

    pub const fn borrow(&self) -> BorrowPolarity {
        self.borrow
    }

    pub const fn operation(&self) -> AccessOperation {
        self.operation
    }

    pub const fn service_reach(&self) -> Option<BoundaryServiceReachId> {
        self.service_reach
    }
}

pub fn derive_placed_view<'extent, 'plan>(
    loan: ExtentLoan<'extent>,
    plan: &'plan ValidatedAccessPlan,
    grant: &PlacedViewGrant,
) -> Result<PlacedView<'extent, 'plan>, AccessPlanDiagnostic> {
    if plan != &grant.access_plan {
        return Err(AccessPlanDiagnostic(
            "placed-view grant does not bind the exact validated access plan".into(),
        ));
    }
    if loan.address_space() != grant.address_space {
        return Err(AccessPlanDiagnostic(
            "extent address space does not match placed-view grant".into(),
        ));
    }
    if loan.provenance() != grant.provenance {
        return Err(AccessPlanDiagnostic(
            "extent provenance does not match placed-view grant".into(),
        ));
    }
    if !loan.rights().contains(&grant.required_rights) {
        return Err(AccessPlanDiagnostic(
            "extent lacks rights required by placed-view grant".into(),
        ));
    }
    if loan.length() < plan.layout_size_bytes {
        return Err(AccessPlanDiagnostic(format!(
            "{}-byte placed layout exceeds {}-byte extent loan",
            plan.layout_size_bytes,
            loan.length()
        )));
    }
    for field in plan.field_descriptors() {
        if let Some(reach) = field.service_reach()
            && !grant.permitted_reaches.contains(&reach)
        {
            return Err(AccessPlanDiagnostic(format!(
                "field `{}` reaches a service not admitted for this extent provenance",
                field.field()
            )));
        }
    }

    Ok(PlacedView {
        loan,
        plan,
        grant: grant.identity,
    })
}

#[derive(Debug, Clone, Copy)]
struct ValidatedEntryPolicy {
    transfer_width_bits: u16,
    observation: ObservationModel,
    permissions: AccessPermissions,
    exposure: AccessExposure,
    service_reach: Option<BoundaryServiceReachId>,
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
                service_reach: None,
            }
        }
        FieldAccess::External {
            transfer_width_bits,
            read,
            write,
            exposure,
            service_reach,
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
                service_reach: Some(service_reach),
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
                service_reach: None,
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
    borrow: BorrowPolarity,
    operation: AccessOperation,
) -> Result<(), AccessPlanDiagnostic> {
    validate_operation_ordering(operation)?;
    let permitted = match operation {
        AccessOperation::Read => descriptor.permissions.read,
        AccessOperation::Take => descriptor.permissions.take && borrow == BorrowPolarity::Exclusive,
        AccessOperation::Write => {
            descriptor.permissions.write && borrow == BorrowPolarity::Exclusive
        }
        AccessOperation::CompoundMutation => {
            descriptor.observation == ObservationModel::Stable
                && descriptor.permissions.read
                && descriptor.permissions.write
                && borrow == BorrowPolarity::Exclusive
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
            "field `{}` does not permit {operation:?} through a {borrow:?} borrow",
            descriptor.field
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
                        service_reach: reach(),
                    },
                ),
                (
                    "transmit",
                    FieldAccess::External {
                        transfer_width_bits: 32,
                        read: ExternalRead::None,
                        write: true,
                        exposure: AccessExposure::Exported,
                        service_reach: reach(),
                    },
                ),
                (
                    "control",
                    FieldAccess::External {
                        transfer_width_bits: 32,
                        read: ExternalRead::Read,
                        write: true,
                        exposure: AccessExposure::BindingPrivate,
                        service_reach: reach(),
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
    fn normalized_identity_covers_operation_width_exposure_and_reach() {
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
            service_reach: reach(),
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
    fn uart_access_plan_validates_geometry_reach_and_borrow_polarity() {
        let plan = uart_access_plan();

        let status = plan
            .authorize(
                field_key(&plan, "status"),
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
        assert_eq!(status.descriptor().service_reach(), Some(reach()));
        assert_eq!(status.borrow(), BorrowPolarity::Shared);
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
                AccessOperation::Write,
            )
            .is_err()
        );
        plan.authorize(
            field_key(&plan, "transmit"),
            BorrowPolarity::Exclusive,
            AccessOperation::Write,
        )
        .expect("exclusive whole write");
        assert!(
            plan.authorize(
                field_key(&plan, "control"),
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
            AccessOperation::CompoundMutation,
        )
        .expect("exclusive stable read-write access derives compound mutation");
        assert!(
            plan.authorize(
                field_key(&plan, "status"),
                BorrowPolarity::Shared,
                AccessOperation::CompoundMutation,
            )
            .is_err()
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
                        service_reach: reach(),
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
                AccessOperation::Read,
            )
            .is_err()
        );
        assert!(
            plan.authorize(
                field_key(&plan, "status"),
                BorrowPolarity::Shared,
                AccessOperation::Take,
            )
            .is_err()
        );
        plan.authorize(
            field_key(&plan, "status"),
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
                        service_reach: reach(),
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
                service_reach: reach(),
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
        plan.authorize(field_key(&plan, "head"), BorrowPolarity::Shared, store)
            .expect("shared mutation is explicitly atomic");
        plan.authorize(
            field_key(&plan, "head"),
            BorrowPolarity::Shared,
            AccessOperation::Atomic(AtomicAccessOperation::FetchAdd(MemoryOrdering::AcqRel)),
        )
        .expect("admitted fetch-add");
        assert!(
            plan.authorize(
                field_key(&plan, "head"),
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
                invalid_load,
            )
            .expect_err("release cannot order an atomic load");
        assert!(error.0.contains("invalid ordering"));
        assert!(
            plan.authorize(
                field_key(&plan, "head"),
                BorrowPolarity::Exclusive,
                AccessOperation::Write,
            )
            .is_err()
        );

        let grant = PlacedViewGrant::from_admitted_provider(
            PlacedViewGrantId::from_normalized_identity(10).expect("atomic view grant"),
            &plan,
            extent_id(2, AddressSpaceId::from_normalized_identity),
            extent_id(5, ExtentProvenanceId::from_normalized_identity),
            extent_rights(&[3]),
            [],
        );
        let extent = uart_extent(0x2000, 4);
        let loan = extent.loan(0, 4).expect("shared atomic loan");
        let view = derive_placed_view(loan, &plan, &grant).expect("admitted atomic view");
        let request = view
            .authorize(
                field_key(&plan, "head"),
                AccessOperation::Atomic(AtomicAccessOperation::CompareExchange {
                    success: MemoryOrdering::AcqRel,
                    failure: MemoryOrdering::Acquire,
                }),
            )
            .expect("authorized compare-exchange")
            .into_primitive_request();
        assert_eq!(request.plan(), plan.identity());
        assert_eq!(request.grant(), grant.identity());
        assert_eq!(request.primitive_address(), 0x2000);
        assert_eq!(request.field(), "head");
        assert_eq!(request.transfer_width_bits(), 32);
        assert_eq!(request.observation(), ObservationModel::Atomic);
        assert_eq!(request.borrow(), BorrowPolarity::Shared);
        assert_eq!(
            request.operation(),
            AccessOperation::Atomic(AtomicAccessOperation::CompareExchange {
                success: MemoryOrdering::AcqRel,
                failure: MemoryOrdering::Acquire,
            })
        );
        assert_eq!(request.service_reach(), None);
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

    fn uart_view_grant(plan: &ValidatedAccessPlan) -> PlacedViewGrant {
        PlacedViewGrant::from_admitted_provider(
            PlacedViewGrantId::from_normalized_identity(8).expect("view grant"),
            plan,
            extent_id(2, AddressSpaceId::from_normalized_identity),
            extent_id(5, ExtentProvenanceId::from_normalized_identity),
            extent_rights(&[3]),
            [reach()],
        )
    }

    #[test]
    fn placed_view_derives_access_from_extent_provenance_and_actual_borrow() {
        let plan = uart_access_plan();
        let grant = uart_view_grant(&plan);

        let mut shared_extent = uart_extent(0x1000, 64);
        let shared_loan = shared_extent.loan(0, 12).expect("shared UART loan");
        let shared_view =
            derive_placed_view(shared_loan, &plan, &grant).expect("admitted shared view");
        let status = shared_view
            .authorize(field_key(&plan, "status"), AccessOperation::Read)
            .expect("shared read");
        assert_eq!(status.primitive_address(), 0x1000);
        assert_eq!(status.access().borrow(), BorrowPolarity::Shared);
        assert!(
            shared_view
                .authorize(field_key(&plan, "transmit"), AccessOperation::Write)
                .is_err()
        );
        let request = status.into_primitive_request();
        assert_eq!(request.plan(), plan.identity());
        assert_eq!(request.grant(), grant.identity());
        assert_eq!(request.primitive_address(), 0x1000);
        assert_eq!(request.field(), "status");
        assert_eq!(request.transfer_width_bits(), 32);
        assert_eq!(request.observation(), ObservationModel::External);
        assert_eq!(request.borrow(), BorrowPolarity::Shared);
        assert_eq!(request.operation(), AccessOperation::Read);
        assert_eq!(request.service_reach(), Some(reach()));
        drop(shared_view);

        let exclusive_loan = shared_extent.loan_mut(4, 12).expect("exclusive UART loan");
        let exclusive_view =
            derive_placed_view(exclusive_loan, &plan, &grant).expect("admitted exclusive view");
        let transmit = exclusive_view
            .authorize(field_key(&plan, "transmit"), AccessOperation::Write)
            .expect("exclusive write");
        assert_eq!(transmit.primitive_address(), 0x1008);
        assert_eq!(transmit.access().borrow(), BorrowPolarity::Exclusive);
    }

    #[test]
    fn placed_view_rejects_unqualified_extent_or_unadmitted_reach() {
        let plan = uart_access_plan();
        let grant = uart_view_grant(&plan);
        let short = uart_extent(0x1000, 8);
        let short_loan = short.loan(0, 8).expect("short loan");
        let error =
            derive_placed_view(short_loan, &plan, &grant).expect_err("layout must fit extent loan");
        assert!(error.0.contains("exceeds"));

        let extent = uart_extent(0x1000, 64);
        let loan = extent.loan(0, 12).expect("UART loan");
        let wrong_reach = PlacedViewGrant::from_admitted_provider(
            PlacedViewGrantId::from_normalized_identity(9).expect("view grant"),
            &plan,
            extent_id(2, AddressSpaceId::from_normalized_identity),
            extent_id(5, ExtentProvenanceId::from_normalized_identity),
            extent_rights(&[3]),
            [],
        );
        let error = derive_placed_view(loan, &plan, &wrong_reach)
            .expect_err("service reach must agree with provenance grant");
        assert!(error.0.contains("not admitted"));
    }

    #[test]
    fn access_identity_and_grant_bind_exact_layout_geometry() {
        let plan = uart_access_plan();
        let mut alternate_layout = uart_layout();
        alternate_layout
            .entries
            .iter_mut()
            .find(|entry| entry.field == "status")
            .expect("status layout entry")
            .placement = LayoutPlacementReport::At { offset: 12 };
        alternate_layout.size = Some(16);
        let error = validate_access_plan(plan.plan().clone(), &alternate_layout)
            .expect_err("plan keys bind their exact layout");
        assert!(error.0.contains("different validated layout"));
        let alternate =
            validate_access_plan(uart_access_source(&alternate_layout), &alternate_layout)
                .expect("fresh plan over non-overlapping alternate geometry");
        assert_ne!(plan.identity(), alternate.identity());
        assert_ne!(
            plan.layout_fingerprint(),
            alternate.layout_fingerprint(),
            "layout geometry is part of access-policy identity"
        );

        let grant = uart_view_grant(&plan);
        let extent = uart_extent(0x1000, 64);
        let loan = extent.loan(0, 16).expect("alternate-layout loan");
        let error = derive_placed_view(loan, &alternate, &grant)
            .expect_err("grant for one geometry cannot authorize another");
        assert!(error.0.contains("exact validated access plan"));
    }
}
