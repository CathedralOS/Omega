//! Late native geometry, projected from the final placement owner.
//!
//! Canonical blob 348dfb1fa43ef749374a9ff46078b211443fcf6c:
//! "Regions cover the emitted instruction spans attributed to that realization,
//! including its retained entry and any split instruction regions."
//!
//! `wiki/spec/language/counts_and_addresses.md`, "Native code layout queries",
//! requires exact attributed instruction spans, including the retained entry,
//! and excludes padding, pools and transitive callees. A function's byte-count
//! envelope is therefore not a code region. Frame application owns additional
//! instructions absent from selected-instruction rows; include its final spans
//! after branch reflow. This projection neither evaluates Psi nor emits code.

use machine_code::{FunctionAppliedFrameProtocol, PlacedFunctionFragment};
use semantic_vocabulary::MachineId;

use crate::StagedOptimizedFixedFrameTextSection;

/// A relocation-independent span in the final native text section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeCodeRegion {
    pub section_offset: u64,
    pub length: u64,
}

/// Native geometry only: object construction supplies address relocations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCodeLayout {
    pub machine: MachineId,
    pub entry_section_offset: u64,
    pub regions: Vec<NativeCodeRegion>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCodeLayoutError {
    UnavailableRealization(MachineId),
    MissingFrameAttribution(MachineId),
    AmbiguousRealization(MachineId),
    InvalidInstructionSpan(MachineId),
    MissingEntryInstruction(MachineId),
}

/// Read the retained final placement without granting artifact publication
/// authority. Machine identity is the exact specialized application identity;
/// ISA alternative keys are not machine application identities.
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
    // Current placement admits one fragment per exact MachineId. Do not infer
    // an entry or duplicate frame attribution if a future split representation
    // changes that invariant without supplying explicit per-fragment custody.
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
            regions.push(NativeCodeRegion {
                section_offset,
                length,
            });
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
            // Union overlapping attribution and coalesce only actual adjacency.
            // A gap remains a gap even if both sides belong to this application.
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
                NativeCodeRegion {
                    section_offset: 40,
                    length: 3,
                },
                NativeCodeRegion {
                    section_offset: 10,
                    length: 2,
                },
                NativeCodeRegion {
                    section_offset: 12,
                    length: 4,
                },
                NativeCodeRegion {
                    section_offset: 11,
                    length: 3,
                },
            ],
        )
        .unwrap();
        assert_eq!(
            regions,
            vec![
                NativeCodeRegion {
                    section_offset: 10,
                    length: 6
                },
                NativeCodeRegion {
                    section_offset: 40,
                    length: 3
                },
            ]
        );
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
            vec![
                NativeCodeRegion {
                    section_offset: 100,
                    length: 8
                },
                NativeCodeRegion {
                    section_offset: 118,
                    length: 4
                },
            ]
        );
        // A stale function/section coordinate join must reject, even if the
        // section coordinate alone happens to stay inside the envelope.
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
            canonical_regions(
                machine,
                vec![NativeCodeRegion {
                    section_offset: u64::MAX,
                    length: 1,
                }]
            ),
            Err(NativeCodeLayoutError::InvalidInstructionSpan(machine))
        );
    }
}
