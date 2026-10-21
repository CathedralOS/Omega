//! Match placement demand against complete normalized runtime supply.
//!
//! Normalization is observational here: order is ignored, service reach is a
//! set, and adjacent regions coalesce only when all capabilities and reach
//! agree. Bounded walks implement that relation without copying the provider's
//! large profile or confusing identical rights for a contiguous transfer with
//! the weaker complete-image requirement. This result remains data, not backing
//! authority. The caller must treat malformed data separately from rejection.

use super::*;
use access_plans::{
    AtomicPermissions, BoundaryReach, ExternalRead, FieldAccess, ValidatedPlacementPlan,
};

pub(in super::super) const COMPATIBILITY_SCRATCH_BYTES: u32 = 176;
const COMPATIBLE: u32 = 48;
const MODULUS: u32 = 56;
const RESIDUE: u32 = 64;
const COVERED: u32 = 72;
const WALK: u32 = 80;
const CANDIDATE: u32 = 88;
const EQUAL: u32 = 96;
const MATCH: u32 = 104;
const LEFT_RULE: u32 = 112;
const RIGHT_RULE: u32 = 120;
const LEFT_SERVICE: u32 = 128;
const RIGHT_SERVICE: u32 = 136;
const ALIGNMENT: u32 = 144;
const FOUND: u32 = 152;
const CONTAINS: u32 = 160;
const SHARED_MODULUS: u32 = 168;

pub(in super::super) struct Compatibility {
    // False is malformed provider data, not an ordinary placement rejection.
    pub(in super::super) well_formed: VirtualRegisterId,
    pub(in super::super) compatible: VirtualRegisterId,
    pub(in super::super) modulus: VirtualRegisterId,
    pub(in super::super) residue: VirtualRegisterId,
}

fn intersect(
    emission: Emission<'_>,
    builder: &mut Builder<'_>,
    cell: u32,
    predicate: VirtualRegisterId,
) -> Result<(), SelectedInstructionError> {
    let previous = emission.read_scratch(builder, cell)?;
    let next = emission.binary(
        builder,
        SelectedInstructionKind::BitwiseAndI64,
        previous,
        predicate,
    )?;
    emission.write_scratch(builder, cell, next)
}

fn unite(
    emission: Emission<'_>,
    builder: &mut Builder<'_>,
    cell: u32,
    predicate: VirtualRegisterId,
) -> Result<(), SelectedInstructionError> {
    let previous = emission.read_scratch(builder, cell)?;
    let next = emission.binary(
        builder,
        SelectedInstructionKind::BitwiseOrI64,
        previous,
        predicate,
    )?;
    emission.write_scratch(builder, cell, next)
}

fn contains_service(
    emission: Emission<'_>,
    builder: &mut Builder<'_>,
    layout: &ProfileLayout,
    region: VirtualRegisterId,
    service: VirtualRegisterId,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let zero = emission.constant(builder, 0)?;
    emission.write_scratch(builder, MATCH, zero)?;
    let count = emission.load(builder, region, layout.reach.count)?;
    emission.repeat(builder, count, RIGHT_SERVICE, |builder, position| {
        let entry = emission.element(builder, region, layout.reach, position)?;
        let value = emission.load(builder, entry, layout.service)?;
        let same = emission.equal(builder, value, service)?;
        unite(emission, builder, MATCH, same)
    })?;
    emission.read_scratch(builder, MATCH)
}

fn reaches(
    emission: Emission<'_>,
    builder: &mut Builder<'_>,
    layout: &ProfileLayout,
    region: VirtualRegisterId,
    required: &BoundaryReach,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let one = emission.constant(builder, 1)?;
    emission.write_scratch(builder, CONTAINS, one)?;
    for service in required.services() {
        let service = emission.constant(builder, service.normalized_identity())?;
        let found = contains_service(emission, builder, layout, region, service)?;
        intersect(emission, builder, CONTAINS, found)?;
    }
    emission.read_scratch(builder, CONTAINS)
}

fn same_fields(
    emission: Emission<'_>,
    builder: &mut Builder<'_>,
    left: VirtualRegisterId,
    right: VirtualRegisterId,
    fields: impl IntoIterator<Item = Scalar>,
    accumulated: u32,
) -> Result<(), SelectedInstructionError> {
    for field in fields {
        let left_value = emission.load(builder, left, field)?;
        let right_value = emission.load(builder, right, field)?;
        let same = emission.equal(builder, left_value, right_value)?;
        intersect(emission, builder, accumulated, same)?;
    }
    Ok(())
}

fn same_rules(
    emission: Emission<'_>,
    builder: &mut Builder<'_>,
    left: VirtualRegisterId,
    right: VirtualRegisterId,
    array: Counted,
    transfer: &TransferLayout,
) -> Result<(), SelectedInstructionError> {
    let left_count = emission.load(builder, left, array.count)?;
    let right_count = emission.load(builder, right, array.count)?;
    let same_count = emission.equal(builder, left_count, right_count)?;
    intersect(emission, builder, EQUAL, same_count)?;
    emission.repeat(builder, left_count, LEFT_RULE, |builder, left_position| {
        let left_entry = emission.element(builder, left, array, left_position)?;
        let zero = emission.constant(builder, 0)?;
        emission.write_scratch(builder, MATCH, zero)?;
        emission.repeat(
            builder,
            right_count,
            RIGHT_RULE,
            |builder, right_position| {
                let right_entry = emission.element(builder, right, array, right_position)?;
                let mut same = emission.constant(builder, 1)?;
                for field in [transfer.width, transfer.alignment]
                    .into_iter()
                    .chain(transfer.operations.iter().flatten().copied())
                {
                    let left_value = emission.load(builder, left_entry, field)?;
                    let right_value = emission.load(builder, right_entry, field)?;
                    let equal = emission.equal(builder, left_value, right_value)?;
                    same = emission.binary(
                        builder,
                        SelectedInstructionKind::BitwiseAndI64,
                        same,
                        equal,
                    )?;
                }
                unite(emission, builder, MATCH, same)
            },
        )?;
        let found = emission.read_scratch(builder, MATCH)?;
        intersect(emission, builder, EQUAL, found)
    })
}

fn same_reach_direction(
    emission: Emission<'_>,
    builder: &mut Builder<'_>,
    layout: &ProfileLayout,
    left: VirtualRegisterId,
    right: VirtualRegisterId,
) -> Result<(), SelectedInstructionError> {
    let count = emission.load(builder, left, layout.reach.count)?;
    emission.repeat(builder, count, LEFT_SERVICE, |builder, position| {
        let entry = emission.element(builder, left, layout.reach, position)?;
        let service = emission.load(builder, entry, layout.service)?;
        let present = contains_service(emission, builder, layout, right, service)?;
        intersect(emission, builder, EQUAL, present)
    })
}

/// Active rules have already passed uniqueness and range validation. Equal
/// count plus membership is therefore rule-set equality. Reach allows duplicate
/// input identities, so it requires containment in both directions instead.
fn equivalent(
    emission: Emission<'_>,
    builder: &mut Builder<'_>,
    layout: &ProfileLayout,
    left: VirtualRegisterId,
    right: VirtualRegisterId,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let one = emission.constant(builder, 1)?;
    emission.write_scratch(builder, EQUAL, one)?;
    same_fields(
        emission,
        builder,
        left,
        right,
        [layout.peer, layout.stable, layout.external, layout.atomic],
        EQUAL,
    )?;
    for (tag, rules, transfer) in [
        (
            layout.external,
            layout.external_rules,
            &layout.external_transfer,
        ),
        (layout.atomic, layout.atomic_rules, &layout.atomic_transfer),
    ] {
        let left_tag = emission.load(builder, left, tag)?;
        let right_tag = emission.load(builder, right, tag)?;
        let left_active = emission.equal(builder, left_tag, one)?;
        let right_active = emission.equal(builder, right_tag, one)?;
        let both_active = emission.binary(
            builder,
            SelectedInstructionKind::BitwiseAndI64,
            left_active,
            right_active,
        )?;
        operation_control::when_nonzero(builder, emission.row, both_active, |builder| {
            if transfer.operations.is_none() {
                same_fields(
                    emission,
                    builder,
                    left,
                    right,
                    [layout.external_read, layout.external_write],
                    EQUAL,
                )?;
            }
            same_rules(emission, builder, left, right, rules, transfer)
        })?;
    }
    same_reach_direction(emission, builder, layout, left, right)?;
    same_reach_direction(emission, builder, layout, right, left)?;
    emission.read_scratch(builder, EQUAL)
}

fn stable_permits(
    emission: Emission<'_>,
    builder: &mut Builder<'_>,
    layout: &ProfileLayout,
    region: VirtualRegisterId,
    read: bool,
    write: bool,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let required = emission.constant(builder, u64::from(read) | (u64::from(write) << 1))?;
    let stable = emission.load(builder, region, layout.stable)?;
    let supplied = emission.binary(
        builder,
        SelectedInstructionKind::BitwiseAndI64,
        stable,
        required,
    )?;
    emission.equal(builder, supplied, required)
}

fn atomic_bits(operations: AtomicPermissions) -> [bool; 12] {
    [
        operations.load,
        operations.store,
        operations.fetch_add,
        operations.fetch_sub,
        operations.fetch_xor,
        operations.fetch_or,
        operations.fetch_and,
        operations.swap,
        operations.compare_exchange,
        operations.compare_exchange_once,
        operations.try_exchange,
        operations.try_exchange_once,
    ]
}

fn select_rules(
    emission: Emission<'_>,
    builder: &mut Builder<'_>,
    region: VirtualRegisterId,
    array: Counted,
    transfer: &TransferLayout,
    width: u16,
    operations: Option<AtomicPermissions>,
) -> Result<(), SelectedInstructionError> {
    let count = emission.load(builder, region, array.count)?;
    let required_width = emission.constant(builder, u64::from(width))?;
    emission.repeat(builder, count, LEFT_RULE, |builder, position| {
        let entry = emission.element(builder, region, array, position)?;
        let width = emission.load(builder, entry, transfer.width)?;
        let mut supported = emission.equal(builder, width, required_width)?;
        if let Some(operations) = operations {
            let one = emission.constant(builder, 1)?;
            let fields = transfer
                .operations
                .as_ref()
                .ok_or_else(|| failure("atomic profile rule lost its operation fields"))?;
            for (field, required) in fields.iter().zip(atomic_bits(operations)) {
                if required {
                    let value = emission.load(builder, entry, *field)?;
                    let present = emission.equal(builder, value, one)?;
                    supported = emission.binary(
                        builder,
                        SelectedInstructionKind::BitwiseAndI64,
                        supported,
                        present,
                    )?;
                }
            }
        }
        operation_control::when_nonzero(builder, emission.row, supported, |builder| {
            let alignment = emission.load(builder, entry, transfer.alignment)?;
            emission.write_scratch(builder, ALIGNMENT, alignment)
        })
    })
}

fn effective_alignment(
    emission: Emission<'_>,
    builder: &mut Builder<'_>,
    layout: &ProfileLayout,
    region: VirtualRegisterId,
    access: &FieldAccess,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let zero = emission.constant(builder, 0)?;
    let one = emission.constant(builder, 1)?;
    emission.write_scratch(builder, ALIGNMENT, zero)?;
    match access {
        FieldAccess::Inaccessible | FieldAccess::StableRegion { .. } => {
            return Err(failure(
                "field without primitive transfer geometry acquired a runtime transfer descriptor",
            ));
        }
        FieldAccess::Stable {
            transfer_width_bits,
            read,
            write,
            ..
        } => {
            let supplied = stable_permits(emission, builder, layout, region, *read, *write)?;
            let bytes = u64::from(*transfer_width_bits / 8);
            let alignment =
                emission.constant(builder, if bytes.is_power_of_two() { bytes } else { 1 })?;
            let alignment = emission.binary(
                builder,
                SelectedInstructionKind::WrappingMultiplyI64,
                alignment,
                supplied,
            )?;
            emission.write_scratch(builder, ALIGNMENT, alignment)?;
        }
        FieldAccess::External {
            transfer_width_bits,
            read,
            write,
            ..
        } => {
            let tag = emission.load(builder, region, layout.external)?;
            let active = emission.equal(builder, tag, one)?;
            operation_control::when_nonzero(builder, emission.row, active, |builder| {
                let mut supplied = one;
                if *read != ExternalRead::None {
                    let expected = emission
                        .constant(builder, if *read == ExternalRead::Read { 1 } else { 2 })?;
                    let actual = emission.load(builder, region, layout.external_read)?;
                    supplied = emission.equal(builder, actual, expected)?;
                }
                if *write {
                    let actual = emission.load(builder, region, layout.external_write)?;
                    supplied = emission.binary(
                        builder,
                        SelectedInstructionKind::BitwiseAndI64,
                        supplied,
                        actual,
                    )?;
                }
                operation_control::when_nonzero(builder, emission.row, supplied, |builder| {
                    select_rules(
                        emission,
                        builder,
                        region,
                        layout.external_rules,
                        &layout.external_transfer,
                        *transfer_width_bits,
                        None,
                    )
                })
            })?;
            if *read != ExternalRead::Take {
                let selected = emission.read_scratch(builder, ALIGNMENT)?;
                let missing = emission.equal(builder, selected, zero)?;
                let stable = stable_permits(
                    emission,
                    builder,
                    layout,
                    region,
                    *read == ExternalRead::Read,
                    *write,
                )?;
                let use_stable = emission.binary(
                    builder,
                    SelectedInstructionKind::BitwiseAndI64,
                    missing,
                    stable,
                )?;
                operation_control::when_nonzero(builder, emission.row, use_stable, |builder| {
                    let bytes = u64::from(*transfer_width_bits / 8);
                    let alignment = emission
                        .constant(builder, if bytes.is_power_of_two() { bytes } else { 1 })?;
                    emission.write_scratch(builder, ALIGNMENT, alignment)
                })?;
            }
        }
        FieldAccess::Atomic {
            transfer_width_bits,
            operations,
            ..
        } => {
            let tag = emission.load(builder, region, layout.atomic)?;
            let active = emission.equal(builder, tag, one)?;
            operation_control::when_nonzero(builder, emission.row, active, |builder| {
                select_rules(
                    emission,
                    builder,
                    region,
                    layout.atomic_rules,
                    &layout.atomic_transfer,
                    *transfer_width_bits,
                    Some(*operations),
                )
            })?;
        }
    }
    emission.read_scratch(builder, ALIGNMENT)
}

fn combine_alignment(
    emission: Emission<'_>,
    builder: &mut Builder<'_>,
    alignment: VirtualRegisterId,
    offset: u64,
) -> Result<(), SelectedInstructionError> {
    let previous_modulus = emission.read_scratch(builder, MODULUS)?;
    let previous_residue = emission.read_scratch(builder, RESIDUE)?;
    let one = emission.constant(builder, 1)?;
    let mask = emission.binary(
        builder,
        SelectedInstructionKind::WrappingSubtractI64,
        alignment,
        one,
    )?;
    let negated_offset = emission.constant(builder, offset.wrapping_neg())?;
    let residue = emission.binary(
        builder,
        SelectedInstructionKind::BitwiseAndI64,
        negated_offset,
        mask,
    )?;
    emission.write_scratch(builder, SHARED_MODULUS, previous_modulus)?;
    let smaller = emission.at_most(builder, alignment, previous_modulus)?;
    operation_control::when_nonzero(builder, emission.row, smaller, |builder| {
        emission.write_scratch(builder, SHARED_MODULUS, alignment)
    })?;
    let shared = emission.read_scratch(builder, SHARED_MODULUS)?;
    let shared_mask = emission.binary(
        builder,
        SelectedInstructionKind::WrappingSubtractI64,
        shared,
        one,
    )?;
    let old = emission.binary(
        builder,
        SelectedInstructionKind::BitwiseAndI64,
        previous_residue,
        shared_mask,
    )?;
    let new = emission.binary(
        builder,
        SelectedInstructionKind::BitwiseAndI64,
        residue,
        shared_mask,
    )?;
    let consistent = emission.equal(builder, old, new)?;
    intersect(emission, builder, COMPATIBLE, consistent)?;
    let larger = emission.invert(builder, smaller)?;
    operation_control::when_nonzero(builder, emission.row, larger, |builder| {
        emission.write_scratch(builder, MODULUS, alignment)?;
        emission.write_scratch(builder, RESIDUE, residue)
    })
}

#[derive(Clone, Copy)]
enum CoverageDemand {
    SameRegion(VirtualRegisterId),
    Stable { read: bool, write: bool },
}

/// Extend coverage through containing/adjacent rows. Primitive transfers require
/// one normalized supply rule; structural Stable regions and complete images
/// require their operations and reach on every covered byte. All accept unordered
/// raw rows; none can cross a gap or acquire a missing capability.
fn extend_coverage(
    emission: Emission<'_>,
    builder: &mut Builder<'_>,
    layout: &ProfileLayout,
    pointer: VirtualRegisterId,
    count: VirtualRegisterId,
    target_end: VirtualRegisterId,
    demand: CoverageDemand,
    reach: &BoundaryReach,
) -> Result<(), SelectedInstructionError> {
    emission.repeat(builder, count, WALK, |builder, _| {
        emission.repeat(builder, count, CANDIDATE, |builder, position| {
            let covered = emission.read_scratch(builder, COVERED)?;
            let complete = emission.at_most(builder, target_end, covered)?;
            let incomplete = emission.invert(builder, complete)?;
            let candidate = emission.element(builder, pointer, layout.regions, position)?;
            let start = emission.load(builder, candidate, layout.start)?;
            let length = emission.load(builder, candidate, layout.length)?;
            let end = emission.binary(
                builder,
                SelectedInstructionKind::WrappingAddI64,
                start,
                length,
            )?;
            let after_start = emission.at_most(builder, start, covered)?;
            let after_end = emission.at_most(builder, end, covered)?;
            let before_end = emission.invert(builder, after_end)?;
            let contains = emission.binary(
                builder,
                SelectedInstructionKind::BitwiseAndI64,
                after_start,
                before_end,
            )?;
            let consider = emission.binary(
                builder,
                SelectedInstructionKind::BitwiseAndI64,
                incomplete,
                contains,
            )?;
            operation_control::when_nonzero(builder, emission.row, consider, |builder| {
                let supported = match demand {
                    CoverageDemand::SameRegion(original) => {
                        equivalent(emission, builder, layout, original, candidate)?
                    }
                    CoverageDemand::Stable { read, write } => {
                        let stable =
                            stable_permits(emission, builder, layout, candidate, read, write)?;
                        let reachable = reaches(emission, builder, layout, candidate, reach)?;
                        emission.binary(
                            builder,
                            SelectedInstructionKind::BitwiseAndI64,
                            stable,
                            reachable,
                        )?
                    }
                };
                operation_control::when_nonzero(builder, emission.row, supported, |builder| {
                    let length = emission.load(builder, candidate, layout.length)?;
                    let end = emission.binary(
                        builder,
                        SelectedInstructionKind::WrappingAddI64,
                        start,
                        length,
                    )?;
                    emission.write_scratch(builder, COVERED, end)
                })
            })
        })
    })
}

pub(in super::super) fn assess(
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    profile_type: semantic_vocabulary::StructuralTypeId,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    profile: ImageHome,
    scratch: ImageHome,
    available: VirtualRegisterId,
    placement: &ValidatedPlacementPlan,
    operation: terminal_psi::TerminalPlacementLifecycleOperation,
) -> Result<Compatibility, SelectedInstructionError> {
    super::super::offset(scratch.byte_offset, u64::from(COMPATIBILITY_SCRATCH_BYTES))?;
    let layout = ProfileLayout::derive(declarations, profile_type)?;
    let well_formed = validate_layout(&layout, builder, row, profile, scratch, available)?;
    let emission = Emission {
        row,
        profile,
        profile_bytes: layout.bytes,
        scratch,
    };
    let zero = emission.constant(builder, 0)?;
    let one = emission.constant(builder, 1)?;
    emission.write_scratch(builder, COMPATIBLE, zero)?;
    emission.write_scratch(builder, MODULUS, one)?;
    emission.write_scratch(builder, RESIDUE, zero)?;
    operation_control::when_nonzero(builder, row, well_formed, |builder| {
        let displacement = emission.constant(builder, u64::from(profile.byte_offset))?;
        let pointer = emission.binary(
            builder,
            SelectedInstructionKind::WrappingAddI64,
            profile.pointer,
            displacement,
        )?;
        let count = emission.load(builder, pointer, layout.regions.count)?;
        let length = emission.constant(builder, placement.access().layout_size_bytes())?;
        let contained = emission.at_most(builder, length, available)?;
        emission.write_scratch(builder, COMPATIBLE, contained)?;
        let alignment = emission.constant(builder, placement.layout().align)?;
        emission.write_scratch(builder, MODULUS, alignment)?;
        emission.write_scratch(builder, COVERED, zero)?;
        extend_coverage(
            emission,
            builder,
            &layout,
            pointer,
            count,
            length,
            CoverageDemand::Stable {
                read: operation == terminal_psi::TerminalPlacementLifecycleOperation::MoveOut,
                write: operation == terminal_psi::TerminalPlacementLifecycleOperation::Initialize,
            },
            placement.reach(),
        )?;
        let covered = emission.read_scratch(builder, COVERED)?;
        let complete = emission.at_most(builder, length, covered)?;
        intersect(emission, builder, COMPATIBLE, complete)?;
        for descriptor in placement.access().field_descriptors() {
            let access = placement
                .access()
                .field(descriptor.key())
                .ok_or_else(|| failure("placement descriptor lost its exact field access"))?
                .access();
            let offset = descriptor.container_byte_offset();
            let end = offset
                .checked_add(u64::from(descriptor.transfer_width_bits() / 8))
                .ok_or_else(|| failure("placement descriptor interval overflows"))?;
            let start = emission.constant(builder, offset)?;
            let end = emission.constant(builder, end)?;
            emission.write_scratch(builder, FOUND, zero)?;
            emission.repeat(builder, count, REGION, |builder, position| {
                let region = emission.element(builder, pointer, layout.regions, position)?;
                let region_start = emission.load(builder, region, layout.start)?;
                let region_length = emission.load(builder, region, layout.length)?;
                let region_end = emission.binary(
                    builder,
                    SelectedInstructionKind::WrappingAddI64,
                    region_start,
                    region_length,
                )?;
                let after_start = emission.at_most(builder, region_start, start)?;
                let after_end = emission.at_most(builder, region_end, start)?;
                let before_end = emission.invert(builder, after_end)?;
                let contains_start = emission.binary(
                    builder,
                    SelectedInstructionKind::BitwiseAndI64,
                    after_start,
                    before_end,
                )?;
                operation_control::when_nonzero(builder, row, contains_start, |builder| {
                    emission.write_scratch(builder, COVERED, region_end)?;
                    extend_coverage(
                        emission,
                        builder,
                        &layout,
                        pointer,
                        count,
                        end,
                        CoverageDemand::SameRegion(region),
                        placement.reach(),
                    )?;
                    let covered = emission.read_scratch(builder, COVERED)?;
                    let contained = emission.at_most(builder, end, covered)?;
                    let reachable = reaches(emission, builder, &layout, region, placement.reach())?;
                    let alignment =
                        effective_alignment(emission, builder, &layout, region, access)?;
                    let supplied = emission.nonzero(builder, alignment)?;
                    let supported = emission.binary(
                        builder,
                        SelectedInstructionKind::BitwiseAndI64,
                        contained,
                        reachable,
                    )?;
                    let supported = emission.binary(
                        builder,
                        SelectedInstructionKind::BitwiseAndI64,
                        supported,
                        supplied,
                    )?;
                    emission.write_scratch(builder, FOUND, supported)?;
                    operation_control::when_nonzero(builder, row, supported, |builder| {
                        combine_alignment(emission, builder, alignment, offset)
                    })
                })
            })?;
            let found = emission.read_scratch(builder, FOUND)?;
            intersect(emission, builder, COMPATIBLE, found)?;
        }
        for region in placement.access().stable_regions() {
            let start = emission.constant(builder, region.byte_offset())?;
            let end = region
                .byte_offset()
                .checked_add(region.byte_length())
                .ok_or_else(|| failure("stable structural region interval overflows"))?;
            let end = emission.constant(builder, end)?;
            emission.write_scratch(builder, COVERED, start)?;
            extend_coverage(
                emission,
                builder,
                &layout,
                pointer,
                count,
                end,
                CoverageDemand::Stable {
                    read: region.permits_read(),
                    write: region.permits_write(),
                },
                placement.reach(),
            )?;
            let covered = emission.read_scratch(builder, COVERED)?;
            let complete = emission.at_most(builder, end, covered)?;
            intersect(emission, builder, COMPATIBLE, complete)?;
            let alignment = emission.constant(builder, region.byte_alignment())?;
            combine_alignment(emission, builder, alignment, region.byte_offset())?;
        }
        Ok(())
    })?;
    Ok(Compatibility {
        well_formed,
        compatible: emission.read_scratch(builder, COMPATIBLE)?,
        modulus: emission.read_scratch(builder, MODULUS)?,
        residue: emission.read_scratch(builder, RESIDUE)?,
    })
}
