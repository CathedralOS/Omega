use psi_layout_plans::{
    LayoutPlacementReport, LayoutPlanReport, layout_plan_reports_match_for_replay,
};

use super::{
    AccessExposure, AccessFieldEntry, AccessPermissions, AccessPlan, AccessPlanDiagnostic,
    ExternalRead, FieldAccess, FieldAccessDescriptor, LogicalFieldExtent, LogicalFieldFragment,
    ObservationModel, RelativeEffectFootprint, ValidatedAccessPlan,
    normalized_access_plan_identity,
};

/// Validate one complete normalized access policy against its retained layout.
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

pub(super) fn validate_entry_geometry(
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
