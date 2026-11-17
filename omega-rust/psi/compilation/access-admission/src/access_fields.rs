/*
We define the per-field access decisions (`AccessFields`, `FieldAccess`,
`ObservationModel`), logical field extents and effect footprints, sealed
`ValidatedAccessFields` descriptors, and `AccessPlanDiagnostic`.
*/

pub(crate) mod diagnostic {
    macro_rules! reject_when {
        ($($condition:expr => $message:expr),+ $(,)?) => {
            $(
                if $condition {
                    return Err(AccessPlanDiagnostic($message.into()));
                }
            )+
        };
    }

    pub(crate) use reject_when;
}

use self::diagnostic::reject_when;
use crate::plan_policy::normalized_identities::authoritative_access_layout_commitment;
use crate::{
    AccessExposure, AccessPermissions, AccessPlanDiagnostic, ExternalRead, FieldAccess, ObservationModel,
};
use layout_plans::{LayoutPlanReport, normalized_layout_plan_report_fingerprint};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub(crate) struct AccessLayoutCommitment(pub(crate) [u8; 32]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, compiler_storage::Record)]
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

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct AccessFields {
    pub(crate) layout_report_fingerprint: u64,
    pub(crate) layout_commitment: AccessLayoutCommitment,
    pub(crate) retained_layout: LayoutPlanReport,
    pub(crate) entries: Vec<AccessFieldEntry>,
}

impl std::hash::Hash for AccessFields {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.layout_report_fingerprint, state);
        std::hash::Hash::hash(&self.layout_commitment, state);
        std::hash::Hash::hash(&self.entries, state);
    }
}

impl AccessFields {
    pub fn inaccessible(layout: &LayoutPlanReport) -> Result<Self, AccessPlanDiagnostic> {
        let layout_report_fingerprint = normalized_layout_plan_report_fingerprint(layout);
        let layout_commitment = authoritative_access_layout_commitment(layout);
        let mut canonical_fields = BTreeMap::new();
        let mut presentation_names = BTreeMap::new();
        let mut presentation_identities = BTreeMap::new();
        for entry in &layout.entries {
            reject_when! { entry.field.is_empty() => "layout field name cannot be empty" }
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
        reject_when! { entry.key != key => "access field key does not identify this schema slot" }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub struct AccessFieldsId(pub(crate) u64);

impl AccessFieldsId {
    pub const fn compatibility_fingerprint(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
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

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct LogicalFieldExtent {
    pub(crate) fragments: Vec<LogicalFieldFragment>,
}

impl LogicalFieldExtent {
    pub fn fragments(&self) -> &[LogicalFieldFragment] {
        &self.fragments
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compiler_storage::Record)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compiler_storage::Record)]
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

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct ValidatedAccessFields {
    pub(crate) identity: AccessFieldsId,
    pub(crate) layout_report_fingerprint: u64,
    pub(crate) layout_commitment: AccessLayoutCommitment,
    pub(crate) plan: AccessFields,
    pub(crate) fields: Vec<FieldAccessDescriptor>,
    pub(crate) field_regions: Vec<FieldRegion>,
    pub(crate) layout_size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct FieldRegion {
    pub(crate) key: AccessFieldKey,
    pub(crate) field: String,
    pub(crate) byte_offset: u64,
    pub(crate) byte_length: u64,
    pub(crate) byte_alignment: u64,
    pub(crate) transfer_width_bits: u16,
    pub(crate) observation: ObservationModel,
    pub(crate) permissions: AccessPermissions,
    pub(crate) exposure: AccessExposure,
}

impl FieldRegion {
    pub const fn key(&self) -> AccessFieldKey {
        self.key
    }
    pub fn field(&self) -> &str {
        &self.field
    }
    pub const fn byte_offset(&self) -> u64 {
        self.byte_offset
    }
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }
    pub const fn byte_alignment(&self) -> u64 {
        self.byte_alignment
    }
    pub const fn permits_read(&self) -> bool {
        self.permissions.read
    }
    pub const fn permits_write(&self) -> bool {
        self.permissions.write
    }
    pub const fn observation(&self) -> ObservationModel {
        self.observation
    }
    pub const fn transfer_width_bits(&self) -> u16 {
        self.transfer_width_bits
    }
    pub const fn permissions(&self) -> AccessPermissions {
        self.permissions
    }
    pub fn scalar_access(&self) -> FieldAccess {
        match self.observation {
            ObservationModel::Stable => FieldAccess::Stable {
                transfer_width_bits: self.transfer_width_bits,
                read: self.permissions.read,
                write: self.permissions.write,
                exposure: self.exposure,
            },
            ObservationModel::External => FieldAccess::External {
                transfer_width_bits: self.transfer_width_bits,
                read: if self.permissions.take {
                    ExternalRead::Take
                } else if self.permissions.read {
                    ExternalRead::Read
                } else {
                    ExternalRead::None
                },
                write: self.permissions.write,
                exposure: self.exposure,
            },
            ObservationModel::Atomic => FieldAccess::Atomic {
                transfer_width_bits: self.transfer_width_bits,
                operations: self.permissions.atomic,
                exposure: self.exposure,
            },
        }
    }
    pub const fn exposure(&self) -> AccessExposure {
        self.exposure
    }
}

impl ValidatedAccessFields {
    pub const fn identity(&self) -> AccessFieldsId {
        self.identity
    }

    pub const fn layout_report_fingerprint(&self) -> u64 {
        self.layout_report_fingerprint
    }

    pub const fn plan(&self) -> &AccessFields {
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

    pub fn field_region(&self, key: AccessFieldKey) -> Option<&FieldRegion> {
        self.field(key)?;
        self.field_regions.iter().find(|region| region.key == key)
    }

    pub fn field_regions(&self) -> &[FieldRegion] {
        &self.field_regions
    }

    pub const fn layout_size_bytes(&self) -> u64 {
        self.layout_size_bytes
    }

    pub fn validate_region_projection(
        &self,
        key: AccessFieldKey,
        byte_offset: u64,
        transfer_width_bits: u16,
    ) -> Result<FieldAccessDescriptor, AccessPlanDiagnostic> {
        let region = self.field_region(key).ok_or_else(|| {
            AccessPlanDiagnostic(
                "projected field key does not name a repeated region in this plan".into(),
            )
        })?;
        let length_bytes = u64::from(transfer_width_bits / 8);
        if transfer_width_bits != region.transfer_width_bits
            || transfer_width_bits == 0
            || transfer_width_bits > 128
            || !transfer_width_bits.is_multiple_of(8)
            || !length_bytes.is_power_of_two()
            || byte_offset < region.byte_offset
            || byte_offset
                .checked_add(length_bytes)
                .is_none_or(|end| end > region.byte_offset + region.byte_length)
            || !(byte_offset - region.byte_offset).is_multiple_of(length_bytes)
            || region.byte_alignment % length_bytes != 0
        {
            return Err(AccessPlanDiagnostic(
                "projection exceeds its parent region, exact leaf width or alignment".into(),
            ));
        }
        let layout_bit_offset = byte_offset
            .checked_mul(8)
            .ok_or_else(|| AccessPlanDiagnostic("region projection bit offset overflows".into()))?;
        let descriptor = FieldAccessDescriptor {
            key,
            field: region.field.clone(),
            container_byte_offset: byte_offset,
            transfer_width_bits,
            logical_extent: LogicalFieldExtent {
                fragments: vec![LogicalFieldFragment {
                    layout_bit_offset,
                    source_bit_offset: 0,
                    width_bits: u64::from(transfer_width_bits),
                }],
            },
            effect_footprint: RelativeEffectFootprint {
                byte_offset,
                length_bytes,
            },
            observation: region.observation,
            permissions: region.permissions,
            exposure: region.exposure,
        };
        Ok(descriptor)
    }
}
