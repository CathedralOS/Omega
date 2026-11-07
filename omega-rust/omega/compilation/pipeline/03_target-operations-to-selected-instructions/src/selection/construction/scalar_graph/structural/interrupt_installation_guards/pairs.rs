//! Keep simultaneously live dedicated arrivals on disjoint actual stack ranges.
use super::*;

// The two pair words are written only
// inside the left match, then read after that scan's exact-one guard.
pub(super) const PAIR_BASE: u32 = 112;
pub(super) const PAIR_END: u32 = 120;

fn counterpart<'a, 'b>(
    entries: &'a [GuardEntry<'b>],
    placement: usize,
) -> Result<&'a GuardEntry<'b>, SelectedInstructionError> {
    let mut found = entries
        .iter()
        .filter(|row| row.requirement.placement_index == placement);
    let first = found
        .next()
        .ok_or_else(|| failure("simultaneous stack placement is absent"))?;
    if found.next().is_some() {
        return Err(failure("simultaneous stack placement is ambiguous"));
    }
    Ok(first)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate(
    builder: &mut Builder<'_>,
    checks: &Checks<'_>,
    entries: &[GuardEntry<'_>],
    table: VirtualRegisterId,
    table_place: PlaceId,
    table_prefix: u32,
    table_indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
    stacks: VirtualRegisterId,
    stacks_place: PlaceId,
    length: VirtualRegisterId,
    layout: &layout::Layout,
) -> Result<(), SelectedInstructionError> {
    for left in entries {
        let Some(demand) = &left.requirement.arrival else {
            continue;
        };
        if demand
            .simultaneous_placements
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(failure(
                "simultaneous stack placements are not strictly ordered",
            ));
        }
        for &placement in &demand.simultaneous_placements {
            if placement == left.requirement.placement_index {
                return Err(failure(
                    "simultaneous dedicated arrivals reuse one fixed stack top",
                ));
            }
            let right = counterpart(entries, placement)?;
            let peer = right
                .requirement
                .arrival
                .as_ref()
                .ok_or_else(|| failure("simultaneous placement has no arrival demand"))?;
            if peer
                .simultaneous_placements
                .binary_search(&left.requirement.placement_index)
                .is_err()
            {
                return Err(failure("simultaneous placement relation is not reciprocal"));
            }
            if !matches!(
                left.entry.entry().inbound().state.stack,
                calling_conventions::EntryStack::Dedicated { .. }
            ) || !matches!(
                right.entry.entry().inbound().state.stack,
                calling_conventions::EntryStack::Dedicated { .. }
            ) {
                return Err(failure(
                    "simultaneous pair lacks two admitted dedicated stacks",
                ));
            }
            if left.requirement.placement_index > placement {
                continue;
            }
            let mut gate_ists = Vec::with_capacity(2);
            for entry in [left, right] {
                gate_ists.push(checks.load(
                    builder,
                    table_place,
                    table,
                    entry.gate_byte_offset + 4,
                    1,
                    StorageAccess::Place {
                        indices: table_indices,
                        byte_offset: table_prefix,
                    },
                )?);
            }
            let same = checks.equal(builder, gate_ists[0], gate_ists[1])?;
            checks.equal_constant(builder, same, 0)?;

            for (side, expected_ist) in gate_ists.into_iter().enumerate() {
                checks.reset(builder, MATCH_COUNT)?;
                checks.iterate(builder, STACK_INDEX, length, |builder, index| {
                    let stack = checks.address(builder, stacks, index, layout.stack_stride)?;
                    let ist = checks.load(
                        builder,
                        stacks_place,
                        stack,
                        layout.stack_ist,
                        1,
                        StorageAccess::Backing,
                    )?;
                    let selected = checks.equal(builder, ist, expected_ist)?;
                    control::when_nonzero(builder, checks.row, selected, |builder| {
                        checks.increment(builder, MATCH_COUNT)?;
                        let backing = checks.load(
                            builder,
                            stacks_place,
                            stack,
                            layout.stack_words,
                            8,
                            StorageAccess::Backing,
                        )?;
                        let words = checks.load(
                            builder,
                            stacks_place,
                            stack,
                            layout.stack_words + 8,
                            8,
                            StorageAccess::Backing,
                        )?;
                        bounded_view(builder, checks, backing, words, 8)?;
                        let end = checks.address(builder, backing, words, 8)?;
                        if side == 0 {
                            checks.save(builder, PAIR_BASE, backing)?;
                            checks.save(builder, PAIR_END, end)
                        } else {
                            let left_base = checks.scratch(builder, PAIR_BASE)?;
                            let left_end = checks.scratch(builder, PAIR_END)?;
                            let before = checks.compare(
                                builder,
                                left_end,
                                backing,
                                SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
                            )?;
                            let after = checks.compare(
                                builder,
                                end,
                                left_base,
                                SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
                            )?;
                            let disjoint = checks.binary(
                                builder,
                                SelectedInstructionKind::BitwiseOrI64,
                                before,
                                after,
                            )?;
                            checks.require(builder, disjoint)
                        }
                    })
                })?;
                let matches = checks.scratch(builder, MATCH_COUNT)?;
                checks.equal_constant(builder, matches, 1)?;
            }
        }
    }
    Ok(())
}
