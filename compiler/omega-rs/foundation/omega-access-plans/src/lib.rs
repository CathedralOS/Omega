//! Normalized access policy for placed views.
//!
//! `LayoutPlan` owns geometry. `AccessPlan` owns observation and the exact
//! primitive operations permitted over that geometry. Keeping them separate
//! prevents wire layouts from acquiring MMIO vocabulary and prevents an
//! arbitrary-offset volatile escape hatch from bypassing plan validation.

use std::collections::BTreeSet;

use omega_core::atomic::AtomicOrderingPlan;
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
    ProviderPrivate,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AtomicPermissions {
    pub load: bool,
    pub store: bool,
    pub compare_exchange: bool,
    pub read_modify_write: bool,
}

impl AtomicPermissions {
    pub const fn any(self) -> bool {
        self.load || self.store || self.compare_exchange || self.read_modify_write
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AccessPermissions {
    pub read: bool,
    pub write: bool,
    /// Explicit permission for a read-followed-by-write primitive. This is not
    /// inferred merely because both read and write are present.
    pub read_modify_write: bool,
    pub atomic: AtomicPermissions,
}

impl AccessPermissions {
    pub const fn any(self) -> bool {
        self.read || self.write || self.read_modify_write || self.atomic.any()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AccessFieldEntry {
    pub field: String,
    /// Width of the actual memory transaction, not the logical bit fragment.
    pub transfer_width_bits: u16,
    pub observation: ObservationModel,
    pub permissions: AccessPermissions,
    pub exposure: AccessExposure,
    /// Statically normalized service reach contributed by every primitive
    /// access. Runtime extent provenance cannot rewrite this value.
    pub service_reach: Option<BoundaryServiceReachId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AccessPlan {
    pub entries: Vec<AccessFieldEntry>,
}

/// Normalizer-owned identity of one validated access policy.
///
/// Authored entry order is not semantic: fields are name-keyed, so validation
/// canonicalizes them before computing this identity. The identity includes
/// every operation, observation, exposure, transfer-width, and service-reach
/// fact that lowering is allowed to consume.
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
    field: String,
    container_byte_offset: u64,
    transfer_width_bits: u16,
    observation: ObservationModel,
    permissions: AccessPermissions,
    exposure: AccessExposure,
    service_reach: Option<BoundaryServiceReachId>,
}

impl FieldAccessDescriptor {
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
/// Authors can name fields and operations but cannot construct this token.
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

    pub fn field(&self, field: &str) -> Option<&AccessFieldEntry> {
        self.plan.entries.iter().find(|entry| entry.field == field)
    }

    pub fn field_descriptor(&self, field: &str) -> Option<&FieldAccessDescriptor> {
        self.fields.iter().find(|entry| entry.field == field)
    }

    pub fn field_descriptors(&self) -> &[FieldAccessDescriptor] {
        &self.fields
    }

    pub const fn layout_size_bytes(&self) -> u64 {
        self.layout_size_bytes
    }

    pub fn authorize(
        &self,
        field: &str,
        borrow: BorrowPolarity,
        operation: AccessOperation,
    ) -> Result<AuthorizedFieldAccess, AccessPlanDiagnostic> {
        let descriptor = self.field_descriptor(field).ok_or_else(|| {
            AccessPlanDiagnostic(format!(
                "field `{field}` has no access entry in the validated plan"
            ))
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
    Write,
    ReadModifyWrite,
    /// One atomic operation carrying the exact source-selected ordering plan.
    ///
    /// The operation family is retained inside `AtomicOrderingPlan`; lowering
    /// never has to reconstruct it from permissions or expression shape.
    Atomic(AtomicOrderingPlan),
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
    mut plan: AccessPlan,
    layout: &LayoutPlanReport,
) -> Result<ValidatedAccessPlan, AccessPlanDiagnostic> {
    let layout_size = layout.size.ok_or_else(|| {
        AccessPlanDiagnostic("placed access requires a fixed-size layout plan".into())
    })?;

    let mut fields = BTreeSet::new();
    for entry in &plan.entries {
        if entry.field.is_empty() {
            return Err(AccessPlanDiagnostic(
                "access entry field name cannot be empty".into(),
            ));
        }
        if !fields.insert(entry.field.as_str()) {
            return Err(AccessPlanDiagnostic(format!(
                "field `{}` has more than one access entry",
                entry.field
            )));
        }
    }
    // Field identity, not authored list position, owns access semantics.
    // Canonical ordering makes equivalent policy machines normalize to the
    // same plan and gives every later consumer one deterministic traversal.
    plan.entries
        .sort_unstable_by(|left, right| left.field.cmp(&right.field));

    let mut descriptors = Vec::with_capacity(plan.entries.len());
    for entry in &plan.entries {
        validate_entry_policy(entry)?;
        let container_byte_offset = validate_entry_geometry(entry, layout, layout_size)?;
        descriptors.push(FieldAccessDescriptor {
            field: entry.field.clone(),
            container_byte_offset,
            transfer_width_bits: entry.transfer_width_bits,
            observation: entry.observation,
            permissions: entry.permissions,
            exposure: entry.exposure,
            service_reach: entry.service_reach,
        });
    }

    let layout_fingerprint = normalized_layout_plan_fingerprint(layout);
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
    hash_bytes(&mut hash, b"omega.access-plan.v2");
    hash_u64(&mut hash, layout_fingerprint);
    hash_u64(&mut hash, plan.entries.len() as u64);
    for entry in &plan.entries {
        hash_u64(&mut hash, entry.field.len() as u64);
        hash_bytes(&mut hash, entry.field.as_bytes());
        hash_u64(&mut hash, u64::from(entry.transfer_width_bits));
        hash_byte(
            &mut hash,
            match entry.observation {
                ObservationModel::Stable => 0,
                ObservationModel::External => 1,
                ObservationModel::Atomic => 2,
            },
        );
        hash_byte(&mut hash, u8::from(entry.permissions.read));
        hash_byte(&mut hash, u8::from(entry.permissions.write));
        hash_byte(&mut hash, u8::from(entry.permissions.read_modify_write));
        hash_byte(&mut hash, u8::from(entry.permissions.atomic.load));
        hash_byte(&mut hash, u8::from(entry.permissions.atomic.store));
        hash_byte(
            &mut hash,
            u8::from(entry.permissions.atomic.compare_exchange),
        );
        hash_byte(
            &mut hash,
            u8::from(entry.permissions.atomic.read_modify_write),
        );
        hash_byte(
            &mut hash,
            match entry.exposure {
                AccessExposure::Exported => 0,
                AccessExposure::ProviderPrivate => 1,
            },
        );
        match entry.service_reach {
            Some(reach) => {
                hash_byte(&mut hash, 1);
                hash_u64(&mut hash, reach.normalized_identity());
            }
            None => hash_byte(&mut hash, 0),
        }
    }
    // Zero is reserved as the inert/no-plan identity throughout the semantic
    // spine. A hash hitting it remains deterministic but is remapped out of
    // the reserved value.
    AccessPlanId(if hash == 0 { 1 } else { hash })
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
        field: &str,
        operation: AccessOperation,
    ) -> Result<PlacedFieldAccess<'view, 'extent>, AccessPlanDiagnostic> {
        let borrow = match self.loan.polarity() {
            LoanPolarity::Shared => BorrowPolarity::Shared,
            LoanPolarity::Exclusive => BorrowPolarity::Exclusive,
        };
        let access = self.plan.authorize(field, borrow, operation)?;
        let primitive_address = self
            .loan
            .base()
            .checked_add(access.descriptor().container_byte_offset())
            .ok_or_else(|| {
                AccessPlanDiagnostic(format!(
                    "field `{field}` primitive address overflows address width"
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

fn validate_entry_policy(entry: &AccessFieldEntry) -> Result<(), AccessPlanDiagnostic> {
    if entry.transfer_width_bits == 0
        || entry.transfer_width_bits > 128
        || !entry.transfer_width_bits.is_multiple_of(8)
    {
        return Err(AccessPlanDiagnostic(format!(
            "field `{}` transfer width {} is not a supported whole-byte width in 8..=128",
            entry.field, entry.transfer_width_bits
        )));
    }
    if !entry.permissions.any() {
        return Err(AccessPlanDiagnostic(format!(
            "field `{}` exposes no primitive operation",
            entry.field
        )));
    }

    match entry.observation {
        ObservationModel::Atomic => {
            if entry.permissions.read
                || entry.permissions.write
                || entry.permissions.read_modify_write
            {
                return Err(AccessPlanDiagnostic(format!(
                    "atomic field `{}` cannot also expose ordinary read/write/RMW",
                    entry.field
                )));
            }
            if !entry.permissions.atomic.any() {
                return Err(AccessPlanDiagnostic(format!(
                    "atomic field `{}` exposes no atomic operation",
                    entry.field
                )));
            }
        }
        ObservationModel::Stable | ObservationModel::External => {
            if entry.permissions.atomic.any() {
                return Err(AccessPlanDiagnostic(format!(
                    "non-atomic field `{}` cannot expose atomic operations",
                    entry.field
                )));
            }
            if entry.permissions.read_modify_write
                && !(entry.permissions.read && entry.permissions.write)
            {
                return Err(AccessPlanDiagnostic(format!(
                    "field `{}` RMW requires both read and write permission",
                    entry.field
                )));
            }
        }
    }

    if entry.observation == ObservationModel::External && entry.service_reach.is_none() {
        return Err(AccessPlanDiagnostic(format!(
            "external field `{}` must pin boundary-service reach",
            entry.field
        )));
    }
    if entry.observation == ObservationModel::Stable && entry.service_reach.is_some() {
        return Err(AccessPlanDiagnostic(format!(
            "stable field `{}` cannot disguise a boundary event as ordinary access",
            entry.field
        )));
    }
    if entry.observation == ObservationModel::External
        && entry.permissions.read_modify_write
        && entry.exposure != AccessExposure::ProviderPrivate
    {
        return Err(AccessPlanDiagnostic(format!(
            "external field `{}` may expose RMW only through a provider-private primitive",
            entry.field
        )));
    }
    Ok(())
}

fn validate_entry_geometry(
    access: &AccessFieldEntry,
    layout: &LayoutPlanReport,
    layout_size: u64,
) -> Result<u64, AccessPlanDiagnostic> {
    let placements = layout
        .entries
        .iter()
        .filter(|entry| entry.field == access.field)
        .map(|entry| entry.placement)
        .collect::<Vec<_>>();
    if placements.is_empty() {
        return Err(AccessPlanDiagnostic(format!(
            "access field `{}` does not exist in the layout plan",
            access.field
        )));
    }

    let transfer_bytes = u64::from(access.transfer_width_bits / 8);
    match placements.as_slice() {
        [LayoutPlacementReport::At { offset }] => {
            let offset = *offset;
            validate_transfer_range(access, offset, transfer_bytes, layout_size)?;
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
                        "access field `{}` mixes whole and fragmented placement",
                        access.field
                    )));
                };
                if *container_width != u64::from(access.transfer_width_bits) {
                    return Err(AccessPlanDiagnostic(format!(
                        "access field `{}` requests a {}-bit transfer over a {container_width}-bit container",
                        access.field, access.transfer_width_bits
                    )));
                }
                if container
                    .replace(*candidate)
                    .is_some_and(|prior| prior != *candidate)
                {
                    return Err(AccessPlanDiagnostic(format!(
                        "fragmented field `{}` spans multiple containers and cannot be projected through one exact access",
                        access.field
                    )));
                }
            }
            let container = container.expect("nonempty placements");
            validate_transfer_range(access, container, transfer_bytes, layout_size)?;
            Ok(container)
        }
    }
}

fn validate_transfer_range(
    access: &AccessFieldEntry,
    offset: u64,
    transfer_bytes: u64,
    layout_size: u64,
) -> Result<(), AccessPlanDiagnostic> {
    let end = offset.checked_add(transfer_bytes).ok_or_else(|| {
        AccessPlanDiagnostic(format!(
            "access field `{}` transfer byte range overflows",
            access.field
        ))
    })?;
    if end > layout_size {
        return Err(AccessPlanDiagnostic(format!(
            "access field `{}` transfer at {offset}..{end} exceeds {layout_size}-byte layout",
            access.field
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
        AccessOperation::Write => {
            descriptor.permissions.write && borrow == BorrowPolarity::Exclusive
        }
        AccessOperation::ReadModifyWrite => {
            descriptor.permissions.read_modify_write && borrow == BorrowPolarity::Exclusive
        }
        AccessOperation::Atomic(AtomicOrderingPlan::Load(_)) => descriptor.permissions.atomic.load,
        AccessOperation::Atomic(AtomicOrderingPlan::Store(_)) => {
            descriptor.permissions.atomic.store
        }
        AccessOperation::Atomic(AtomicOrderingPlan::CompareExchange { .. }) => {
            descriptor.permissions.atomic.compare_exchange
        }
        AccessOperation::Atomic(
            AtomicOrderingPlan::ReadModifyWrite(_) | AtomicOrderingPlan::Swap(_),
        ) => descriptor.permissions.atomic.read_modify_write,
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
    let AccessOperation::Atomic(ordering) = operation else {
        return Ok(());
    };
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
            entries: vec![
                LayoutFieldEntryReport {
                    field: "status".into(),
                    placement: LayoutPlacementReport::At { offset: 0 },
                },
                LayoutFieldEntryReport {
                    field: "transmit".into(),
                    placement: LayoutPlacementReport::At { offset: 4 },
                },
                LayoutFieldEntryReport {
                    field: "control".into(),
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

    fn ordinary(read: bool, write: bool, rmw: bool) -> AccessPermissions {
        AccessPermissions {
            read,
            write,
            read_modify_write: rmw,
            atomic: AtomicPermissions::default(),
        }
    }

    fn uart_access_plan() -> ValidatedAccessPlan {
        validate_access_plan(
            AccessPlan {
                entries: vec![
                    AccessFieldEntry {
                        field: "status".into(),
                        transfer_width_bits: 32,
                        observation: ObservationModel::External,
                        permissions: ordinary(true, false, false),
                        exposure: AccessExposure::Exported,
                        service_reach: Some(reach()),
                    },
                    AccessFieldEntry {
                        field: "transmit".into(),
                        transfer_width_bits: 32,
                        observation: ObservationModel::External,
                        permissions: ordinary(false, true, false),
                        exposure: AccessExposure::Exported,
                        service_reach: Some(reach()),
                    },
                    AccessFieldEntry {
                        field: "control".into(),
                        transfer_width_bits: 32,
                        observation: ObservationModel::External,
                        permissions: ordinary(true, true, true),
                        exposure: AccessExposure::ProviderPrivate,
                        service_reach: Some(reach()),
                    },
                ],
            },
            &uart_layout(),
        )
        .expect("UART plan")
    }

    #[test]
    fn normalized_identity_is_name_keyed_and_order_independent() {
        let forward = uart_access_plan();
        let mut reversed_source = forward.plan().clone();
        reversed_source.entries.reverse();
        let reversed =
            validate_access_plan(reversed_source, &uart_layout()).expect("reordered UART plan");

        assert_eq!(forward.identity(), reversed.identity());
        assert_eq!(forward.plan(), reversed.plan());
        assert_eq!(
            forward
                .plan()
                .entries
                .iter()
                .map(|entry| entry.field.as_str())
                .collect::<Vec<_>>(),
            vec!["control", "status", "transmit"]
        );
        assert_ne!(forward.identity().normalized_identity(), 0);
    }

    #[test]
    fn normalized_identity_covers_operation_width_exposure_and_reach() {
        let layout = LayoutPlanReport {
            entries: vec![LayoutFieldEntryReport {
                field: "word".into(),
                placement: LayoutPlacementReport::At { offset: 0 },
            }],
            offsets: Some(vec![0]),
            size: Some(8),
            align: 8,
        };
        let validate = |entry: AccessFieldEntry| {
            validate_access_plan(
                AccessPlan {
                    entries: vec![entry],
                },
                &layout,
            )
            .expect("identity test plan")
            .identity()
        };
        let stable_read = AccessFieldEntry {
            field: "word".into(),
            transfer_width_bits: 32,
            observation: ObservationModel::Stable,
            permissions: ordinary(true, false, false),
            exposure: AccessExposure::Exported,
            service_reach: None,
        };
        let mut stable_write = stable_read.clone();
        stable_write.permissions = ordinary(false, true, false);
        let mut wider = stable_read.clone();
        wider.transfer_width_bits = 64;
        let mut private = stable_read.clone();
        private.exposure = AccessExposure::ProviderPrivate;
        let mut external = stable_read.clone();
        external.observation = ObservationModel::External;
        external.service_reach = Some(reach());

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
            .authorize("status", BorrowPolarity::Shared, AccessOperation::Read)
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
            plan.field_descriptor("control")
                .expect("control descriptor")
                .container_byte_offset(),
            8
        );
        assert!(
            plan.authorize("transmit", BorrowPolarity::Shared, AccessOperation::Write)
                .is_err()
        );
        plan.authorize(
            "transmit",
            BorrowPolarity::Exclusive,
            AccessOperation::Write,
        )
        .expect("exclusive whole write");
        plan.authorize(
            "control",
            BorrowPolarity::Exclusive,
            AccessOperation::ReadModifyWrite,
        )
        .expect("provider-private explicit RMW");
    }

    #[test]
    fn exported_external_rmw_and_missing_reach_reject() {
        let mut entry = AccessFieldEntry {
            field: "status".into(),
            transfer_width_bits: 32,
            observation: ObservationModel::External,
            permissions: ordinary(true, true, true),
            exposure: AccessExposure::Exported,
            service_reach: Some(reach()),
        };
        let error = validate_access_plan(
            AccessPlan {
                entries: vec![entry.clone()],
            },
            &uart_layout(),
        )
        .expect_err("public MMIO RMW must not be derived");
        assert!(error.0.contains("provider-private"));

        entry.permissions.read_modify_write = false;
        entry.service_reach = None;
        let error = validate_access_plan(
            AccessPlan {
                entries: vec![entry],
            },
            &uart_layout(),
        )
        .expect_err("external access cannot launder service reach");
        assert!(error.0.contains("reach"));
    }

    #[test]
    fn atomic_shared_page_exposes_only_atomic_mutation() {
        let layout = LayoutPlanReport {
            entries: vec![LayoutFieldEntryReport {
                field: "head".into(),
                placement: LayoutPlacementReport::At { offset: 0 },
            }],
            offsets: Some(vec![0]),
            size: Some(4),
            align: 4,
        };
        let plan = validate_access_plan(
            AccessPlan {
                entries: vec![AccessFieldEntry {
                    field: "head".into(),
                    transfer_width_bits: 32,
                    observation: ObservationModel::Atomic,
                    permissions: AccessPermissions {
                        atomic: AtomicPermissions {
                            load: true,
                            store: true,
                            compare_exchange: true,
                            read_modify_write: true,
                        },
                        ..AccessPermissions::default()
                    },
                    exposure: AccessExposure::Exported,
                    service_reach: None,
                }],
            },
            &layout,
        )
        .expect("atomic IPC plan");

        let store = AccessOperation::Atomic(AtomicOrderingPlan::Store(
            omega_core::atomic::MemoryOrdering::Release,
        ));
        plan.authorize("head", BorrowPolarity::Shared, store)
            .expect("shared mutation is explicitly atomic");
        let invalid_load = AccessOperation::Atomic(AtomicOrderingPlan::Load(
            omega_core::atomic::MemoryOrdering::Release,
        ));
        let error = plan
            .authorize("head", BorrowPolarity::Shared, invalid_load)
            .expect_err("release cannot order an atomic load");
        assert!(error.0.contains("invalid ordering"));
        assert!(
            plan.authorize("head", BorrowPolarity::Exclusive, AccessOperation::Write)
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
                "head",
                AccessOperation::Atomic(AtomicOrderingPlan::CompareExchange {
                    success: omega_core::atomic::MemoryOrdering::AcqRel,
                    failure: omega_core::atomic::MemoryOrdering::Acquire,
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
            AccessOperation::Atomic(AtomicOrderingPlan::CompareExchange {
                success: omega_core::atomic::MemoryOrdering::AcqRel,
                failure: omega_core::atomic::MemoryOrdering::Acquire,
            })
        );
        assert_eq!(request.service_reach(), None);
    }

    #[test]
    fn multi_container_fragments_are_not_one_access() {
        let layout = LayoutPlanReport {
            entries: vec![
                LayoutFieldEntryReport {
                    field: "entry".into(),
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
            AccessPlan {
                entries: vec![AccessFieldEntry {
                    field: "entry".into(),
                    transfer_width_bits: 32,
                    observation: ObservationModel::Stable,
                    permissions: ordinary(true, false, false),
                    exposure: AccessExposure::Exported,
                    service_reach: None,
                }],
            },
            &layout,
        )
        .expect_err("one token cannot hide two primitive accesses");
        assert!(error.0.contains("multiple containers"));
    }

    #[test]
    fn duplicate_and_unknown_access_fields_reject() {
        let entry = AccessFieldEntry {
            field: "missing".into(),
            transfer_width_bits: 32,
            observation: ObservationModel::Stable,
            permissions: ordinary(true, false, false),
            exposure: AccessExposure::Exported,
            service_reach: None,
        };
        let error = validate_access_plan(
            AccessPlan {
                entries: vec![entry.clone(), entry],
            },
            &uart_layout(),
        )
        .expect_err("duplicate fields reject before geometry");
        assert!(error.0.contains("more than one"));

        let error = validate_access_plan(
            AccessPlan {
                entries: vec![AccessFieldEntry {
                    field: "missing".into(),
                    transfer_width_bits: 32,
                    observation: ObservationModel::Stable,
                    permissions: ordinary(true, false, false),
                    exposure: AccessExposure::Exported,
                    service_reach: None,
                }],
            },
            &uart_layout(),
        )
        .expect_err("unknown field rejects");
        assert!(error.0.contains("does not exist"));
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
            .authorize("status", AccessOperation::Read)
            .expect("shared read");
        assert_eq!(status.primitive_address(), 0x1000);
        assert_eq!(status.access().borrow(), BorrowPolarity::Shared);
        assert!(
            shared_view
                .authorize("transmit", AccessOperation::Write)
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
            .authorize("transmit", AccessOperation::Write)
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
        let alternate = validate_access_plan(plan.plan().clone(), &alternate_layout)
            .expect("non-overlapping alternate geometry");
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
