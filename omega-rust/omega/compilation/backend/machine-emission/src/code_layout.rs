/*
We project placed native text sections (`StagedOptimizedFixedFrameTextSection`) into per-machine entry offsets and canonical instruction regions (`NativeCodeLayout`) to answer `code_layout<Machine>()` queries. We collect every non-empty instruction span and applied frame prologue/epilogue span attributed to the requested `MachineId`, verify that function-relative and section-relative coordinates agree within the fragment's byte envelope, and coalesce only overlapping or directly adjacent spans so unattributed gaps, padding, and transitive callees remain excluded.
*/

use machine_code::{FunctionAppliedFrameProtocol, PlacedFunctionFragment};
use semantic_vocabulary::MachineId;

use crate::StagedOptimizedFixedFrameTextSection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeCodeRegion {
    pub section_offset: u64, // Byte offset of the attributed span from the start of the text section.
    pub length: u64,         // Non-zero byte length of the coalesced attributed span.
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCodeLayout {
    pub machine: MachineId, // Specialized machine identity described by this layout.
    pub entry_section_offset: u64, // Section-relative byte offset of the machine's entry point.
    pub regions: Vec<NativeCodeRegion>, // Sorted, non-overlapping attributed instruction regions.
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCodeLayoutError {
    UnavailableRealization(MachineId),
    MissingFrameAttribution(MachineId),
    AmbiguousRealization(MachineId),
    InvalidInstructionSpan(MachineId),
    MissingEntryInstruction(MachineId),
}

pub fn describe_native_code_layout(
    source: &StagedOptimizedFixedFrameTextSection,
    machine: MachineId,
) -> Result<NativeCodeLayout, NativeCodeLayoutError> {
    let text = source.text_section();
    let mut fragments = text
        .functions
        .iter()
        .filter(|fragment| fragment.machine == machine);
    let fragment = fragments
        .next()
        .ok_or(NativeCodeLayoutError::UnavailableRealization(machine))?;
    if fragments.next().is_some() {
        return Err(NativeCodeLayoutError::AmbiguousRealization(machine));
    }
    let entry = fragment.section_offset;
    let mut frames = source
        .source()
        .application()
        .functions
        .iter()
        .filter(|frame| frame.machine == machine);
    let frame = frames
        .next()
        .ok_or(NativeCodeLayoutError::MissingFrameAttribution(machine))?;
    if frames.next().is_some() {
        return Err(NativeCodeLayoutError::AmbiguousRealization(machine));
    }
    let mut regions = Vec::new();
    append_fragment_spans(fragment, frame, text.byte_count, &mut regions)?;
    let regions = canonical_regions(machine, regions)?;
    if !regions.iter().any(|region| {
        region.section_offset <= entry && entry - region.section_offset < region.length
    }) {
        return Err(NativeCodeLayoutError::MissingEntryInstruction(machine));
    }
    Ok(NativeCodeLayout {
        machine,
        entry_section_offset: entry,
        regions,
    })
}

fn code_region(section_offset: u64, length: u64) -> NativeCodeRegion {
    NativeCodeRegion {
        section_offset,
        length,
    }
}

fn append_fragment_spans(
    fragment: &PlacedFunctionFragment,
    frame: &FunctionAppliedFrameProtocol,
    text_length: u64,
    regions: &mut Vec<NativeCodeRegion>,
) -> Result<(), NativeCodeLayoutError> {
    let machine = fragment.machine;
    let invalid = || NativeCodeLayoutError::InvalidInstructionSpan(machine);
    let function_end = fragment
        .section_offset
        .checked_add(fragment.byte_count)
        .filter(|end| *end <= text_length)
        .ok_or_else(invalid)?;
    let mut append = |function_offset: u64, section_offset: u64, length: u64| {
        if fragment.section_offset.checked_add(function_offset) != Some(section_offset)
            || section_offset
                .checked_add(length)
                .is_none_or(|end| end > function_end)
        {
            return Err(invalid());
        }
        if length != 0 {
            regions.push(code_region(section_offset, length));
        }
        Ok(())
    };
    for instruction in fragment.instructions() {
        append(
            instruction.function_offset,
            instruction.section_offset,
            instruction.byte_count,
        )?;
    }
    let prologue_offset = fragment
        .section_offset
        .checked_add(frame.prologue_function_offset)
        .ok_or_else(invalid)?;
    append(
        frame.prologue_function_offset,
        prologue_offset,
        frame.prologue_byte_count,
    )?;
    for epilogue in &frame.epilogues {
        let offset = fragment
            .section_offset
            .checked_add(epilogue.function_offset)
            .ok_or_else(invalid)?;
        append(epilogue.function_offset, offset, epilogue.byte_count)?;
    }
    Ok(())
}

fn canonical_regions(
    machine: MachineId,
    mut spans: Vec<NativeCodeRegion>,
) -> Result<Vec<NativeCodeRegion>, NativeCodeLayoutError> {
    spans.sort_unstable_by_key(|span| span.section_offset);
    let mut regions: Vec<NativeCodeRegion> = Vec::new();
    for span in spans {
        let end = span
            .section_offset
            .checked_add(span.length)
            .ok_or(NativeCodeLayoutError::InvalidInstructionSpan(machine))?;
        if let Some(previous) = regions.last_mut() {
            let previous_end = previous
                .section_offset
                .checked_add(previous.length)
                .ok_or(NativeCodeLayoutError::InvalidInstructionSpan(machine))?;
            if span.section_offset <= previous_end {
                previous.length = previous_end.max(end) - previous.section_offset;
                continue;
            }
        }
        regions.push(span);
    }
    Ok(regions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attributed_spans_preserve_gaps_and_allow_shared_overlap() {
        let machine = MachineId::new(1).unwrap();
        let regions = canonical_regions(
            machine,
            vec![
                code_region(40, 3),
                code_region(10, 2),
                code_region(12, 4),
                code_region(11, 3),
            ],
        )
        .unwrap();
        assert_eq!(regions, vec![code_region(10, 6), code_region(40, 3)]);
    }

    #[test]
    fn frame_instructions_join_final_rows_without_filling_unattributed_bytes() {
        use machine_code::{FunctionAppliedFrameEpilogue, PlacedBlockSpan, PlacedInstructionSpan};
        use selected_instructions::{
            MachineAlternativeFamily, MachineAlternativeKey, SelectedBlockId, SelectedInstructionId,
        };
        use semantic_vocabulary::EdgeId;
        let machine = MachineId::new(1).unwrap();
        let block = SelectedBlockId(1);
        let instruction = |ordinal, offset, count| PlacedInstructionSpan {
            instruction: SelectedInstructionId(ordinal),
            alternative: MachineAlternativeKey {
                family: MachineAlternativeFamily::CompareI64Zero,
                variant: 1,
            },
            function_offset: offset,
            section_offset: 100 + offset,
            byte_count: count,
        };
        let mut fragment = PlacedFunctionFragment {
            source_function_index: 0,
            machine,
            section_offset: 100,
            byte_count: 22,
            blocks: vec![PlacedBlockSpan {
                block,
                function_offset: 2,
                section_offset: 102,
                byte_count: 20,
                instructions: vec![
                    instruction(1, 2, 3),
                    instruction(2, 7, 1),
                    instruction(3, 20, 2),
                ],
            }],
        };
        let frame = FunctionAppliedFrameProtocol {
            machine,
            prologue_function_offset: 0,
            prologue_byte_count: 2,
            epilogues: vec![
                FunctionAppliedFrameEpilogue {
                    block,
                    return_instruction: SelectedInstructionId(2),
                    psi_return_edge: EdgeId::new(1).unwrap(),
                    function_offset: 5,
                    byte_count: 2,
                },
                FunctionAppliedFrameEpilogue {
                    block,
                    return_instruction: SelectedInstructionId(3),
                    psi_return_edge: EdgeId::new(2).unwrap(),
                    function_offset: 18,
                    byte_count: 2,
                },
            ],
        };
        let mut spans = Vec::new();
        append_fragment_spans(&fragment, &frame, 122, &mut spans).unwrap();
        assert_eq!(
            canonical_regions(machine, spans).unwrap(),
            vec![code_region(100, 8), code_region(118, 4)]
        );
        fragment.blocks[0].instructions[0].section_offset += 1;
        assert_eq!(
            append_fragment_spans(&fragment, &frame, 122, &mut Vec::new()),
            Err(NativeCodeLayoutError::InvalidInstructionSpan(machine))
        );
    }

    #[test]
    fn overflowing_native_span_rejects() {
        let machine = MachineId::new(1).unwrap();
        assert_eq!(
            canonical_regions(machine, vec![code_region(u64::MAX, 1)]),
            Err(NativeCodeLayoutError::InvalidInstructionSpan(machine))
        );
    }
}
