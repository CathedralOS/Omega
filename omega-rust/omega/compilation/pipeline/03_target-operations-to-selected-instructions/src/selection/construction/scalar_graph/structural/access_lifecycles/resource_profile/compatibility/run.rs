//! Check repeated placement without generating or executing one check per cell.
//! Profile boundaries select the few cells whose supply can differ. Within a
//! region, two consecutive full cells establish the repeating alignment rule.

use super::*;

pub(in super::super::super) const SCRATCH_BYTES: u32 = 240;
const RUN_COMPATIBLE: u32 = 200;
const RUN_REGION: u32 = 208;
const SEARCH_LOWER: u32 = 216;
const SEARCH_UPPER: u32 = 224;

#[derive(Clone, Copy)]
pub(in super::super::super) struct RunGeometry {
    pub count: VirtualRegisterId,
    pub base: VirtualRegisterId,
    pub length: VirtualRegisterId,
}

#[derive(Clone, Copy)]
pub(in super::super::super) enum SupplyRole {
    Source,
    Destination,
}

pub(in super::super::super) fn assess(
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    profile_type: semantic_vocabulary::StructuralTypeId,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    profile: ImageHome,
    scratch: ImageHome,
    geometry: RunGeometry,
    placement: &ValidatedAccessPlan,
    role: SupplyRole,
) -> Result<Compatibility, SelectedInstructionError> {
    let layout = ProfileLayout::derive(declarations, profile_type)?;
    let well_formed = validate_layout(&layout, builder, row, profile, scratch, geometry.length)?;
    let emission = Emission {
        row,
        profile,
        profile_bytes: layout.bytes,
        scratch,
    };
    let zero = emission.constant(builder, 0)?;
    let one = emission.constant(builder, 1)?;
    emission.write_scratch(builder, RUN_COMPATIBLE, zero)?;
    let footprint = placement
        .layout()
        .size
        .ok_or_else(|| failure("run cell has no fixed footprint"))?;
    operation_control::when_nonzero(builder, row, well_formed, |builder| {
        let stride = emission.constant(builder, footprint)?;
        let occupied =
            bin!(emission => builder => WrappingMultiplyI64 => geometry.count => stride)?;
        let exact = emission.equal(builder, occupied, geometry.length)?;
        emission.write_scratch(builder, RUN_COMPATIBLE, exact)?;
        if footprint != 0 {
            let bound = emission.constant(builder, u64::MAX / footprint)?;
            let fits = emission.at_most(builder, geometry.count, bound)?;
            intersect(emission, builder, RUN_COMPATIBLE, fits)?;
        }
        let end = bin!(emission => builder => WrappingAddI64 => geometry.base => geometry.length)?;
        let fits = emission.at_most(builder, geometry.base, end)?;
        intersect(emission, builder, RUN_COMPATIBLE, fits)?;
        let displacement = emission.constant(builder, u64::from(profile.byte_offset))?;
        let pointer =
            bin!(emission => builder => WrappingAddI64 => profile.pointer => displacement)?;
        let regions = emission.load(builder, pointer, layout.regions.count)?;
        emission.write_scratch(builder, COVERED, zero)?;
        extend_coverage(
            emission,
            builder,
            &layout,
            pointer,
            regions,
            geometry.length,
            CoverageDemand::Stable {
                read: matches!(role, SupplyRole::Source),
                write: matches!(role, SupplyRole::Destination),
            },
            placement.reach(),
            [WALK, CANDIDATE],
        )?;
        let covered = emission.read_scratch(builder, COVERED)?;
        let complete = emission.at_most(builder, geometry.length, covered)?;
        intersect(emission, builder, RUN_COMPATIBLE, complete)?;
        if matches!(role, SupplyRole::Source) {
            return Ok(());
        }
        // Geometry must hold before candidate offset arithmetic is used. A
        // malformed length cannot wrap a candidate into a different cell.
        let compatible = emission.read_scratch(builder, RUN_COMPATIBLE)?;
        operation_control::when_nonzero(builder, row, compatible, |builder| {
            let check = |builder: &mut Builder<'_>, cell| {
                check_cell(
                    emission,
                    &layout,
                    builder,
                    geometry,
                    placement,
                    footprint,
                    cell,
                    well_formed,
                )
            };
            check(builder, zero)?;
            if footprint == 0 {
                return Ok(());
            }
            check(builder, one)?;
            let last = bin!(emission => builder => WrappingSubtractI64 => geometry.count => one)?;
            check(builder, last)?;
            emission.repeat(builder, regions, RUN_REGION, |builder, position| {
                let region = emission.element(builder, pointer, layout.regions, position)?;
                let start = emission.load(builder, region, layout.start)?;
                let length = emission.load(builder, region, layout.length)?;
                let end = bin!(emission => builder => WrappingAddI64 => start => length)?;
                for boundary in [start, end] {
                    let cell = boundary_cell(emission, builder, geometry.count, stride, boundary)?;
                    let previous = bin!(emission => builder => WrappingSubtractI64 => cell => one)?;
                    check(builder, previous)?;
                    check(builder, cell)?;
                    for distance in [1, 2] {
                        let distance = emission.constant(builder, distance)?;
                        let next = bin!(emission => builder => WrappingAddI64 => cell => distance)?;
                        let no_wrap = emission.at_most(builder, cell, next)?;
                        operation_control::when_nonzero(builder, row, no_wrap, |builder| {
                            check(builder, next)
                        })?;
                    }
                }
                Ok(())
            })
        })
    })?;
    Ok(Compatibility {
        well_formed,
        compatible: emission.read_scratch(builder, RUN_COMPATIBLE)?,
        modulus: one,
        residue: zero,
    })
}

fn check_cell(
    emission: Emission<'_>,
    layout: &ProfileLayout,
    builder: &mut Builder<'_>,
    geometry: RunGeometry,
    placement: &ValidatedAccessPlan,
    footprint: u64,
    cell: VirtualRegisterId,
    well_formed: VirtualRegisterId,
) -> Result<(), SelectedInstructionError> {
    let outside = emission.at_most(builder, geometry.count, cell)?;
    let inside = emission.invert(builder, outside)?;
    operation_control::when_nonzero(builder, emission.row, inside, |builder| {
        let stride = emission.constant(builder, footprint)?;
        let origin = bin!(emission => builder => WrappingMultiplyI64 => cell => stride)?;
        let assessment = assess_cell(
            emission,
            layout,
            builder,
            geometry.length,
            placement,
            terminal_psi::TerminalAccessLifecycleOperation::Initialize,
            origin,
            well_formed,
        )?;
        let one = emission.constant(builder, 1)?;
        let mask = bin!(emission => builder => WrappingSubtractI64 => assessment.modulus => one)?;
        let residue = bin!(emission => builder => BitwiseAndI64 => geometry.base => mask)?;
        let aligned = emission.equal(builder, residue, assessment.residue)?;
        intersect(emission, builder, RUN_COMPATIBLE, assessment.compatible)?;
        intersect(emission, builder, RUN_COMPATIBLE, aligned)
    })
}

/// Largest cell boundary no greater than the profile boundary. Binary search
/// is bounded by 64 iterations and uses only the already checked count/stride
/// product; it needs no fabricated source arithmetic proof for a native divide.
fn boundary_cell(
    emission: Emission<'_>,
    builder: &mut Builder<'_>,
    count: VirtualRegisterId,
    stride: VirtualRegisterId,
    boundary: VirtualRegisterId,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let zero = emission.constant(builder, 0)?;
    emission.write_scratch(builder, SEARCH_LOWER, zero)?;
    emission.write_scratch(builder, SEARCH_UPPER, count)?;
    operation_control::while_nonzero(
        builder,
        emission.row,
        |builder| {
            let lower = emission.read_scratch(builder, SEARCH_LOWER)?;
            let upper = emission.read_scratch(builder, SEARCH_UPPER)?;
            let same = emission.equal(builder, lower, upper)?;
            emission.invert(builder, same)
        },
        |builder| {
            let lower = emission.read_scratch(builder, SEARCH_LOWER)?;
            let upper = emission.read_scratch(builder, SEARCH_UPPER)?;
            let distance = bin!(emission => builder => WrappingSubtractI64 => upper => lower)?;
            let half = fragmented_scalar::shift(
                builder,
                emission.row,
                emission.scratch.place,
                distance,
                1,
                false,
            )?;
            let one = emission.constant(builder, 1)?;
            let odd = bin!(emission => builder => BitwiseAndI64 => distance => one)?;
            let rounded = bin!(emission => builder => WrappingAddI64 => half => odd)?;
            let middle = bin!(emission => builder => WrappingAddI64 => lower => rounded)?;
            let offset = bin!(emission => builder => WrappingMultiplyI64 => middle => stride)?;
            let below = emission.at_most(builder, offset, boundary)?;
            operation_control::when_nonzero(builder, emission.row, below, |builder| {
                emission.write_scratch(builder, SEARCH_LOWER, middle)
            })?;
            let above = emission.invert(builder, below)?;
            operation_control::when_nonzero(builder, emission.row, above, |builder| {
                let preceding = bin!(emission => builder => WrappingSubtractI64 => middle => one)?;
                emission.write_scratch(builder, SEARCH_UPPER, preceding)
            })
        },
    )?;
    emission.read_scratch(builder, SEARCH_LOWER)
}
