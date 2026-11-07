//! Match arrival shape and the exclusive IST lease to the measured entry.
use super::*;
pub(super) fn validate_arrivals(
    builder: &mut Builder<'_>,
    checks: &Checks<'_>,
    place: PlaceId,
    pointer: VirtualRegisterId,
    length: VirtualRegisterId,
    layout: &layout::Layout,
    entry: &GuardEntry<'_>,
) -> Result<(), SelectedInstructionError> {
    // The caller checked total length against the complete entry roster. One
    // matching row per distinct gate therefore excludes duplicates and extras.
    let expected_vector = checks.constant(builder, u64::from(entry.gate_byte_offset / 16))?;
    let expected_frame = match entry.requirement.entry_plan.frame {
        InstalledInterruptFrame::NoErrorCode => layout.no_error_tag,
        InstalledInterruptFrame::ErrorCode => layout.error_tag,
        InstalledInterruptFrame::OpaqueFatal => layout.opaque_fatal_tag,
    };
    if entry.requirement.entry_plan.protocol == InstalledInterruptProtocol::Acknowledgement
        && (entry.requirement.entry_plan.frame != InstalledInterruptFrame::NoErrorCode
            || !(32..=255).contains(&(entry.gate_byte_offset / 16)))
    {
        return Err(failure(
            "returning acknowledgement requires an external no-error frame",
        ));
    }
    checks.reset(builder, MATCH_COUNT)?;
    checks.iterate(builder, ARRIVAL_INDEX, length, |builder, index| {
        let arrival = checks.address(builder, pointer, index, layout.arrival_stride)?;
        let vector = checks.load(
            builder,
            place,
            arrival,
            layout.arrival_vector,
            2,
            StorageAccess::Backing,
        )?;
        let selected = checks.equal(builder, vector, expected_vector)?;
        control::when_nonzero(builder, checks.row, selected, |builder| {
            checks.increment(builder, MATCH_COUNT)?;
            let frame = checks.load(
                builder,
                place,
                arrival,
                layout.arrival_frame,
                layout.frame_bytes,
                StorageAccess::Backing,
            )?;
            checks.equal_constant(builder, frame, expected_frame)
        })
    })?;
    let matches = checks.scratch(builder, MATCH_COUNT)?;
    checks.equal_constant(builder, matches, 1)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_stacks(
    builder: &mut Builder<'_>,
    checks: &Checks<'_>,
    place: PlaceId,
    stacks: VirtualRegisterId,
    length: VirtualRegisterId,
    task_place: PlaceId,
    task: VirtualRegisterId,
    gate_ist: VirtualRegisterId,
    layout: &layout::Layout,
    entry: &GuardEntry<'_>,
    root_interval: Option<(VirtualRegisterId, VirtualRegisterId)>,
) -> Result<(), SelectedInstructionError> {
    let requirement = entry.requirement;
    let required = requirement
        .required_bytes()
        .ok_or_else(|| failure("installed handler stack requirement overflows"))?;
    if required < u64::from(requirement.entry_plan.root_slot_from_top)
        || requirement.alignment < 16
        || !requirement.alignment.is_power_of_two()
    {
        return Err(failure(
            "installed handler stack requirement omits bridge storage",
        ));
    }
    checks.reset(builder, MATCH_COUNT)?;
    checks.iterate(builder, STACK_INDEX, length, |builder, index| {
        let stack = checks.address(builder, stacks, index, layout.stack_stride)?;
        let ist = checks.load(
            builder,
            place,
            stack,
            layout.stack_ist,
            1,
            StorageAccess::Backing,
        )?;
        checks.require(builder, ist)?;
        let seven = checks.constant(builder, 7)?;
        checks.at_most(builder, ist, seven)?;
        let selected = checks.equal(builder, ist, gate_ist)?;
        control::when_nonzero(builder, checks.row, selected, |builder| {
            checks.increment(builder, MATCH_COUNT)?;
            let backing = checks.load(
                builder,
                place,
                stack,
                layout.stack_words,
                8,
                StorageAccess::Backing,
            )?;
            let words = checks.load(
                builder,
                place,
                stack,
                layout.stack_words + 8,
                8,
                StorageAccess::Backing,
            )?;
            checks.require(builder, backing)?;
            bounded_view(builder, checks, backing, words, 8)?;
            let end = checks.address(builder, backing, words, 8)?;
            if let Some((root_start, root_end)) = root_interval {
                let before = checks.compare(
                    builder,
                    end,
                    root_start,
                    SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
                )?;
                let after = checks.compare(
                    builder,
                    root_end,
                    backing,
                    SelectedInstructionKind::MaterializeBooleanU64LessOrEqual,
                )?;
                let disjoint = checks.binary(
                    builder,
                    SelectedInstructionKind::BitwiseOrI64,
                    before,
                    after,
                )?;
                checks.require(builder, disjoint)?;
            }
            // TSS IST1 starts at byte36. Validate index1..7 before the dynamic
            // read; index*8+28 then lies wholly inside the admitted TSS104.
            let indexed = checks.address(builder, task, ist, 8)?;
            let top = checks.load(builder, task_place, indexed, 28, 8, StorageAccess::Backing)?;
            checks.aligned(builder, top, requirement.alignment)?;
            // The CPU top may be rounded down inside the supplied view. Check
            // both endpoints before subtraction; excluded guards supply no space.
            checks.at_most(builder, backing, top)?;
            checks.at_most(builder, top, end)?;
            let bytes = checks.binary(
                builder,
                SelectedInstructionKind::WrappingSubtractI64,
                top,
                backing,
            )?;
            let need = checks.constant(builder, required)?;
            checks.at_most(builder, need, bytes)?;
            // Only the acknowledgement's stack contains the installation root.
            // Fatal entries recover that same root through the retained anchor.
            if requirement.entry_plan.protocol != InstalledInterruptProtocol::Acknowledgement {
                return Ok(());
            }
            let root_offset = checks.constant(
                builder,
                u64::from(requirement.entry_plan.root_slot_from_top),
            )?;
            let root_word = checks.binary(
                builder,
                SelectedInstructionKind::WrappingSubtractI64,
                top,
                root_offset,
            )?;
            checks.at_most(builder, backing, root_word)?;
            checks.save(builder, TIMER_ROOT, root_word)?;
            checks.save(builder, STACK_TOP, top)
        })
    })?;
    let matches = checks.scratch(builder, MATCH_COUNT)?;
    checks.equal_constant(builder, matches, 1)
}
