//! Resident field events join retained custody to an admitted byte footprint.
//!
//! Reference formation already captured the original owner and allocation;
//! each event checks that binding is still live and uses the initialization's
//! immutable resource compatibility. Canonical access slots select geometry,
//! never a source name or an address lookup. The codec operates on one staged
//! transfer window, so unrelated fields, padding and the allocation tail stay
//! untouched. Snapshot qualifications and capability bindings cannot survive
//! an arbitrary write: unsupported preservation refuses before changing bytes.
//! The transfer window reserves its entire byte payload before allocation and
//! retains that lease through the single commit. Scalar decoding uses bounded
//! inline storage; neither codec hides another heap payload. Geometry metadata
//! remains separate from this payload account, not a general RSS bound.

use super::{ProviderStorageBinding, ProviderStorageSlot};
use crate::byte_sequences::storage::ByteSequenceStorage;
use crate::effects::results::TerminalEffectResult;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::values::{StructuralRuntimePlace, TerminalScalarValue};
use access_plans::{AccessOperation, BorrowPolarity, EffectiveSupplyKind, ObservationModel};
use layout_plans::{
    AggregateFieldSchema, ByteOrder, LayoutPlacementReport, LayoutPlanReport, ScalarFieldValue,
};
use terminal_psi::{
    StructuralAccess, StructuralPathSegment, StructuralTypeShape, TerminalPlacedAccessorKind,
    TerminalPlacedFieldPlan, TerminalPlacedViewPlan,
};

pub(crate) enum ProviderFieldContent<'view> {
    Resident,
    Borrowed(&'view access_plans::PlacementResourceCompatibility),
}

impl TerminalExecution<'_> {
    pub(crate) fn execute_provider_field(
        &mut self,
        binding: ProviderStorageBinding,
        content: ProviderFieldContent<'_>,
        plan: &TerminalPlacedViewPlan,
        field: &TerminalPlacedFieldPlan,
        projection: &[StructuralPathSegment],
        kind: TerminalPlacedAccessorKind,
        source_access: StructuralAccess,
        arguments: &[TerminalScalarValue],
    ) -> Result<TerminalEffectResult, TerminalInterpretError> {
        if !self.provider_storage.binding_is_live(binding) {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "resident field lost its original allocation",
            ));
        }
        let access = plan.placement.access();
        let entry = access
            .plan()
            .entries()
            .get(field.access_slot as usize)
            .ok_or_else(malformed)?;
        let operation = match kind {
            TerminalPlacedAccessorKind::Read => AccessOperation::Read,
            TerminalPlacedAccessorKind::Write => AccessOperation::Write,
            TerminalPlacedAccessorKind::Take => {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "resident take requires partial custody transfer",
                ));
            }
        };
        let source_borrow = match source_access {
            StructuralAccess::SharedBorrow => BorrowPolarity::Shared,
            StructuralAccess::MutableBorrow => BorrowPolarity::Exclusive,
            _ => return Err(malformed()),
        };
        let projected = if access.field_region(entry.key()).is_some() {
            Some(
                terminal_semantics::placed_region_projection(
                    &self.program.catalogs.structural_types,
                    plan,
                    field,
                    projection,
                )
                .map_err(TerminalInterpretError::PlacedReference)?,
            )
        } else {
            if !projection.is_empty() {
                return Err(malformed());
            }
            None
        };
        let authorized = if let Some(projected) = projected {
            access.authorize_region_projection(
                entry.key(),
                projected.byte_offset,
                u16::from(
                    terminal_semantics::reference_storage_byte_width(projected.stored_scalar)
                        .ok_or_else(malformed)?,
                ) * 8,
                operation.receiver_polarity(),
                source_borrow,
                operation,
            )
        } else {
            access.authorize(
                entry.key(),
                operation.receiver_polarity(),
                source_borrow,
                operation,
            )
        }
        .map_err(|reason| TerminalInterpretError::PlacedReference(reason.to_string()))?;
        let descriptor = authorized.descriptor();
        if matches!(content, ProviderFieldContent::Resident)
            && descriptor.observation() != ObservationModel::Stable
        {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "resident scalar events require Stable observation",
            ));
        }
        let (compatibility, initialized, custody, content_bytes) = match content {
            ProviderFieldContent::Borrowed(compatibility) => {
                (compatibility, None, None, binding.length)
            }
            ProviderFieldContent::Resident => {
                let (initialized, custody, compatibility) =
                    self.live_resident_custody(binding.resident)?;
                if initialized.placement != plan.placement
                    || initialized.schema != plan.schema
                    || initialized.policy_identity != plan.policy_identity
                    || initialized.policy_plan_machine_identity != plan.policy_plan_machine_identity
                    || initialized.schema_identity != plan.schema_identity
                    || custody.value.structural_type != plan.schema
                {
                    return Err(TerminalInterpretError::ContentTransformationMismatch(
                        "resident event changed its initialization contract",
                    ));
                }
                (
                    compatibility.placement(),
                    Some(initialized),
                    Some(custody),
                    compatibility.byte_length(),
                )
            }
        };
        let supply = compatibility
            .fields()
            .iter()
            .find(|row| row.key() == entry.key());
        let footprint = descriptor.effect_footprint();
        let supplied = if let Some(supply) = supply {
            (descriptor.observation() != ObservationModel::Stable
                || supply.kind() == EffectiveSupplyKind::Stable)
                && supply.offset() == footprint.byte_offset()
                && u64::from(supply.width_bits())
                    == footprint
                        .length_bytes()
                        .checked_mul(8)
                        .ok_or_else(malformed)?
        } else {
            compatibility.field_regions().iter().any(|region| {
                region.key() == entry.key()
                    && footprint.byte_offset() >= region.byte_offset()
                    && footprint.end() <= region.byte_offset() + region.byte_length()
            })
        };
        if !supplied || footprint.end() > content_bytes || footprint.end() > binding.length {
            return Err(malformed());
        }
        let StructuralTypeShape::Record { fields, .. } = &self
            .program
            .structural_types()
            .get(&plan.schema)
            .ok_or_else(malformed)?
            .shape
        else {
            return Err(malformed());
        };
        let source_field = fields
            .iter()
            .find(|row| row.identity == field.schema_field)
            .ok_or_else(malformed)?;
        if initialized.is_some_and(|initialized| {
            initialized.image_fields.get(field.access_slot as usize) != Some(&source_field.id)
        }) {
            return Err(malformed());
        }
        let value_type = projected.map_or_else(
            || source_field.field_type.clone(),
            |projection| terminal_psi::StructuralFieldType::Scalar(projection.logical_scalar),
        );
        let scalar =
            value_type
                .scalar_type()
                .ok_or(TerminalInterpretError::UnsupportedSemanticVariant(
                    "resident accessor requires a scalar value contract",
                ))?;
        let width =
            terminal_semantics::reference_storage_byte_width(scalar).ok_or_else(malformed)?;
        let write = match (kind, arguments) {
            (TerminalPlacedAccessorKind::Read, []) => None,
            (TerminalPlacedAccessorKind::Write, [value])
                if value.scalar_type() == scalar && value.satisfies_field(&value_type) =>
            {
                Some(*value)
            }
            _ => return Err(malformed()),
        };
        if write.is_some()
            && let Some(custody) = custody
        {
            let mut path = vec![StructuralPathSegment::Field(source_field.identity)];
            path.extend_from_slice(projection);
            if !custody.value.qualifications.is_empty()
                || custody
                    .value
                    .projected_qualifications
                    .iter()
                    .any(|row| terminal_semantics::structural_paths_may_overlap(&row.path, &path))
                || custody.storage.resident_field_has_bindings(
                    &StructuralRuntimePlace::from(&custody.value),
                    source_field.identity,
                )
            {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "resident field write requires preservation of retained qualifications and bindings",
                ));
            }
        }
        // Partial writes preserve other fields' bits in the physical container.
        // That physical read needs actual read supply even if the logical field
        // exposes only a write operation. Full replacement never reads bytes.
        let represented_bits = descriptor
            .logical_extent()
            .fragments()
            .iter()
            .try_fold(0_u64, |sum, fragment| {
                sum.checked_add(fragment.width_bits())
            })
            .ok_or_else(malformed)?;
        let needs_read = write.is_none() || represented_bits != footprint.length_bytes() * 8;
        let ProviderStorageSlot::Live {
            allocation,
            profile,
            ..
        } = self.provider_storage.allocations.get(binding.allocation)
        else {
            return Err(malformed());
        };
        let loan = allocation
            .extent
            .loan(binding.offset, binding.length)
            .map_err(|reason| TerminalInterpretError::PlacedReference(reason.to_string()))?;
        let actual = profile
            .restrict_to_loan(&loan)
            .map_err(|reason| TerminalInterpretError::PlacedReference(reason.to_string()))?;
        // Resident mutation still requires Stable supply. External views use
        // the separately captured complete plan/profile compatibility and may
        // not synthesize a read-patch-write operation from a write permission.
        if descriptor.observation() == ObservationModel::Stable {
            access_plans::require_stable_region_supply(
                &actual,
                footprint.byte_offset(),
                footprint.length_bytes(),
                needs_read,
                write.is_some(),
                plan.placement.reach(),
                "resident field event",
            )
            .map_err(|reason| TerminalInterpretError::PlacedReference(reason.to_string()))?;
        } else if write.is_some() && needs_read {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "external field write cannot synthesize an unadmitted read-patch-write",
            ));
        }
        let start = binding
            .offset
            .checked_add(footprint.byte_offset())
            .and_then(|offset| usize::try_from(offset).ok())
            .ok_or_else(malformed)?;
        let length = usize::try_from(footprint.length_bytes()).map_err(|_| malformed())?;
        let end = start.checked_add(length).ok_or_else(malformed)?;
        let bytes = allocation.bytes.get(start..end).ok_or_else(malformed)?;
        let mut transfer = if needs_read {
            ByteSequenceStorage::copy(bytes, self.account.as_ref())?
        } else {
            ByteSequenceStorage::zeroed(length, self.account.as_ref())?
        };
        let layout = if projected.is_some() {
            let mut layout = field_window(
                plan.placement.layout(),
                entry.field(),
                access
                    .field_region(entry.key())
                    .ok_or_else(malformed)?
                    .byte_offset(),
                footprint.length_bytes(),
            )?;
            layout.size = Some(footprint.length_bytes());
            layout
        } else {
            field_window(
                plan.placement.layout(),
                entry.field(),
                footprint.byte_offset(),
                footprint.length_bytes(),
            )?
        };
        let member = layout
            .entries
            .first()
            .ok_or_else(malformed)?
            .member_identity;
        let result = if let Some(value) = write {
            let bits = crate::primitive_storage::encoding::encode(scalar, value)?;
            let bits = u64::try_from(bits).map_err(|_| {
                TerminalInterpretError::UnsupportedSemanticVariant(
                    "resident scalar codec exceeds its 64-bit materialization carrier",
                )
            })?;
            let value = match member {
                Some(member) => ScalarFieldValue::new_numbered(
                    entry.field(),
                    member,
                    u16::from(width) * 8,
                    bits,
                ),
                None => ScalarFieldValue::new(entry.field(), u16::from(width) * 8, bits),
            }
            .map_err(TerminalInterpretError::ValueImage)?;
            layout_plans::stage_scalar_field(
                &layout,
                &value,
                ByteOrder::LittleEndian,
                transfer.bytes_mut(),
            )
            .map_err(TerminalInterpretError::ValueImage)?;
            TerminalEffectResult::Unit
        } else {
            let schema = match member {
                Some(member) => {
                    AggregateFieldSchema::new_numbered(entry.field(), member, u64::from(width))
                }
                None => AggregateFieldSchema::new(entry.field(), u64::from(width)),
            }
            .map_err(TerminalInterpretError::ValueImage)?;
            let mut encoded = [0_u8; 16];
            let decoded = encoded
                .get_mut(..usize::from(width))
                .ok_or_else(malformed)?;
            layout_plans::decode_aggregate_field_into(
                &layout,
                &schema,
                ByteOrder::LittleEndian,
                transfer.bytes(),
                decoded,
            )
            .map_err(TerminalInterpretError::ValueImage)?;
            let value =
                crate::primitive_storage::encoding::decode(scalar, u128::from_le_bytes(encoded))?;
            if !value.satisfies_field(&value_type) {
                return Err(malformed());
            }
            TerminalEffectResult::Scalar(value)
        };
        if write.is_some() {
            // All semantic and codec refusals precede the single exact-footprint
            // commit. The original resident and its allocation remain in place.
            let ProviderStorageSlot::Live { allocation, .. } = self
                .provider_storage
                .allocations
                .get_mut(binding.allocation)
            else {
                return Err(malformed());
            };
            allocation
                .bytes
                .get_mut(start..end)
                .ok_or_else(malformed)?
                .copy_from_slice(transfer.bytes());
        }
        Ok(result)
    }
}

/// Rebase the already-normalized field geometry into its transfer window.
/// The key comes from the retained access-slot roster, not source lookup.
fn field_window(
    layout: &LayoutPlanReport,
    field: &str,
    base: u64,
    length: u64,
) -> Result<LayoutPlanReport, TerminalInterpretError> {
    let mut entries = Vec::new();
    for entry in layout.entries.iter().filter(|entry| entry.field == field) {
        let mut entry = entry.clone();
        let offset = match &mut entry.placement {
            LayoutPlacementReport::At { offset }
            | LayoutPlacementReport::IntegerAt { offset, .. } => offset,
            LayoutPlacementReport::Bits { container, .. } => container,
        };
        *offset = offset.checked_sub(base).ok_or_else(malformed)?;
        entries.push(entry);
    }
    if entries.is_empty() {
        return Err(malformed());
    }
    Ok(LayoutPlanReport {
        entries,
        offsets: None,
        size: Some(length),
        align: 1,
        ..layout.clone()
    })
}
