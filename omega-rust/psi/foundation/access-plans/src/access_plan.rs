//! The access plan: one canonical slot per layout schema field, the policy
//! each slot carries, and the validated plan with its sealed field
//! descriptors that lowering consumes.

use crate::plan_policy::authorization::authorize_descriptor;
use crate::plan_policy::normalized_identities::authoritative_access_layout_commitment;
use crate::{AccessOperation, AccessPlanDiagnostic, BorrowPolarity};
use layout_plans::{LayoutPlanReport, normalized_layout_plan_report_fingerprint};
use std::collections::BTreeMap;

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
pub(crate) struct AccessLayoutCommitment(pub(crate) [u8; 32]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AccessFieldKey {
    pub(crate) layout_report_fingerprint: u64,
    pub(crate) layout_commitment: AccessLayoutCommitment,
    pub(crate) slot: u32,
}

impl AccessFieldKey {
    pub const fn slot(self) -> u32 {
        self.slot
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AccessFieldEntry {
    pub(crate) key: AccessFieldKey,
    pub(crate) field: String,
    pub(crate) access: FieldAccess,
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
    pub(crate) layout_report_fingerprint: u64,
    pub(crate) layout_commitment: AccessLayoutCommitment,
    pub(crate) retained_layout: LayoutPlanReport,
    pub(crate) entries: Vec<AccessFieldEntry>,
}

impl std::hash::Hash for AccessPlan {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.layout_report_fingerprint, state);
        std::hash::Hash::hash(&self.layout_commitment, state);
        std::hash::Hash::hash(&self.entries, state);
    }
}

impl AccessPlan {
    pub fn inaccessible(layout: &LayoutPlanReport) -> Result<Self, AccessPlanDiagnostic> {
        let layout_report_fingerprint = normalized_layout_plan_report_fingerprint(layout);
        let layout_commitment = authoritative_access_layout_commitment(layout);
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
                        layout_report_fingerprint,
                        layout_commitment,
                        slot,
                    },
                    field,
                    access: FieldAccess::Inaccessible,
                })
            })
            .collect::<Result<Vec<_>, AccessPlanDiagnostic>>()?;
        Ok(Self {
            layout_report_fingerprint,
            layout_commitment,
            retained_layout: layout.clone(),
            entries,
        })
    }

    pub const fn layout_report_fingerprint(&self) -> u64 {
        self.layout_report_fingerprint
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
        if key.layout_report_fingerprint != self.layout_report_fingerprint
            || key.layout_commitment != self.layout_commitment
        {
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
pub struct AccessPlanId(pub(crate) u64);

impl AccessPlanId {
    /// Compact report/cache coordinate. Access authority retains the complete
    /// validated layout and access decisions rather than trusting this value.
    pub const fn compatibility_fingerprint(self) -> u64 {
        self.0
    }
}

/// Sealed geometry and policy for one projected field.
///
/// The offset is intentionally private. Only plan validation can construct a
/// descriptor, so later lowering never accepts an author-supplied byte offset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldAccessDescriptor {
    pub(crate) key: AccessFieldKey,
    pub(crate) field: String,
    pub(crate) container_byte_offset: u64,
    pub(crate) transfer_width_bits: u16,
    pub(crate) logical_extent: LogicalFieldExtent,
    pub(crate) effect_footprint: RelativeEffectFootprint,
    pub(crate) observation: ObservationModel,
    pub(crate) permissions: AccessPermissions,
    pub(crate) exposure: AccessExposure,
}

/// The exact laid bits that represent one logical field value.
///
/// A fragmented field may contain several pieces, but every piece remains in
/// the one transfer container admitted for primitive placed access. The source
/// bit offset names the corresponding position in the logical field value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalFieldExtent {
    pub(crate) fragments: Vec<LogicalFieldFragment>,
}

impl LogicalFieldExtent {
    pub fn fragments(&self) -> &[LogicalFieldFragment] {
        &self.fragments
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LogicalFieldFragment {
    pub(crate) layout_bit_offset: u64,
    pub(crate) source_bit_offset: u64,
    pub(crate) width_bits: u64,
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
    pub(crate) byte_offset: u64,
    pub(crate) length_bytes: u64,
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
    pub(crate) address: u64,
    pub(crate) length_bytes: u64,
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
    pub(crate) descriptor: FieldAccessDescriptor,
    pub(crate) current_borrow: BorrowPolarity,
    pub(crate) source_loan: BorrowPolarity,
    pub(crate) operation: AccessOperation,
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
    pub(crate) identity: AccessPlanId,
    pub(crate) layout_report_fingerprint: u64,
    pub(crate) layout_commitment: AccessLayoutCommitment,
    pub(crate) plan: AccessPlan,
    pub(crate) fields: Vec<FieldAccessDescriptor>,
    pub(crate) layout_size_bytes: u64,
}

impl ValidatedAccessPlan {
    pub const fn identity(&self) -> AccessPlanId {
        self.identity
    }

    pub const fn layout_report_fingerprint(&self) -> u64 {
        self.layout_report_fingerprint
    }

    pub const fn plan(&self) -> &AccessPlan {
        &self.plan
    }

    pub fn field(&self, key: AccessFieldKey) -> Option<&AccessFieldEntry> {
        if key.layout_report_fingerprint != self.layout_report_fingerprint
            || key.layout_commitment != self.layout_commitment
        {
            return None;
        }
        self.plan
            .entries
            .get(key.slot as usize)
            .filter(|entry| entry.key == key)
    }

    pub fn field_descriptor(&self, key: AccessFieldKey) -> Option<&FieldAccessDescriptor> {
        if key.layout_report_fingerprint != self.layout_report_fingerprint
            || key.layout_commitment != self.layout_commitment
        {
            return None;
        }
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
