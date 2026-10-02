//! Code-layout descriptions projected from validated placed text.
//!
//! A retained `code_layout<Machine>()` query
//! (wiki/spec/language/counts_and_addresses.md#native-code-layout-queries)
//! answers with one selected application's entry `addr` and the exact
//! emitted instruction regions attributed to that realization — never a
//! guessed window, a next-symbol subtraction, or bytes belonging to
//! padding, data pools, or callees. This module joins the placed text
//! section's retained `PlacedInstructionSpan` geometry and call relocation
//! evidence to each selected application, so the query's runtime metadata
//! derives from the same record object and image construction consume.
//!
//! Bases stay section-relative here: object construction binds each
//! machine's function symbol to the section, and link/load resolves the
//! final `addr`. [`CodeLayoutAddress::TextSectionOffset`] keeps that
//! pending relocation structural instead of pretending a final address
//! exists. A machine with no placed fragment rejects — an interpreted or
//! unrealized entry has no native regions to describe.
//!
//! Like [`crate::private_callback`], this is a projection over an already
//! validated section, not another route stage: [`describe_code_layouts`]
//! derives descriptions for every placed application and
//! [`validate_selected_application_code_layout`] independently recomputes
//! one candidate description for replay.

use machine_code::{
    PlacedFunctionFragment, PlacedInternalMachineCallResolution,
    PlacedNormalizedForeignCallResolution, RelocationFreeTextSectionPlacement,
};
use optimization_core::TerminalRelocationFreeTextSectionIdentity;
use semantic_vocabulary::MachineId;

/// An `addr`-valued coordinate in a code-layout description.
///
/// Placement produces section-relative offsets; the text section's load
/// base relocates them into final addresses at link/load. Keeping the
/// pending relocation structural prevents a consumer from treating an
/// offset as an already-bound address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CodeLayoutAddress {
    /// Offset into the placed text section; binds to `section_base +
    /// offset` once the section's load address resolves.
    TextSectionOffset(u64),
}

impl CodeLayoutAddress {
    /// The section-relative offset this coordinate carries while it awaits
    /// relocation.
    pub fn section_offset(self) -> u64 {
        match self {
            Self::TextSectionOffset(offset) => offset,
        }
    }
}

/// One emitted code region attributed to a selected application: a maximal
/// contiguous run of placed instruction spans.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacedCodeRegion {
    /// Where the run's first instruction byte sits.
    pub base: CodeLayoutAddress,
    /// Exact instruction byte coverage — the query's `u64` `length`.
    pub length: u64,
}

/// The `code_layout` description for one selected application.
///
/// `regions` is the complete coverage: sorted by base, pairwise disjoint,
/// and formed only from the fragment's placed instruction spans. Padding,
/// data pools, and other machines' bytes are excluded because they never
/// occupy an instruction span; a region is a description, not exclusive
/// ownership, so shared code can appear in another application's
/// description as well. Call relocation rows are joined by caller: a
/// callee's regions are not pulled into the caller's description merely
/// because its entry lies nearby or is required for execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedApplicationCodeLayout {
    /// The selected application this description covers.
    pub machine: MachineId,
    /// The placed text section this description derives from; binds the
    /// description to its exact artifact bytes and geometry.
    pub text_section: TerminalRelocationFreeTextSectionIdentity,
    /// The retained entry — the fragment base object construction binds
    /// this machine's function symbol to.
    pub entry: CodeLayoutAddress,
    /// Exact emitted instruction regions, including split regions when
    /// placement separated the application's instruction spans.
    pub regions: Vec<PlacedCodeRegion>,
    /// Internal calls whose fields lie inside this application's regions,
    /// already resolved to section coordinates during placement.
    pub internal_calls: Vec<PlacedInternalMachineCallResolution>,
    /// Foreign-call fields inside this application's regions still awaiting
    /// their object-level import-symbol binding. Callback-address
    /// materialization fields ride inside each retained row.
    pub foreign_calls: Vec<PlacedNormalizedForeignCallResolution>,
}

/// Why a code-layout description cannot be published for a section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeLayoutError {
    /// The queried application has no placed realization in the section:
    /// the query rejects rather than inventing a region or fabricating a
    /// code size for an interpreted entry.
    MissingRealization(MachineId),
    /// A span attributed to the application lies outside its fragment's
    /// placed extent — the description cannot claim that coverage.
    SpanOutsideFragment(MachineId),
    /// Two spans attributed to one application overlap, so the region list
    /// cannot describe exact disjoint coverage.
    OverlappingInstructionSpans(MachineId),
    /// A call resolution's field does not lie inside its caller's regions;
    /// the join would attribute relocation custody to the wrong
    /// application.
    CallFieldOutsideRegions(MachineId),
    /// Section geometry exceeded a `u64` coordinate.
    OffsetOverflow,
    /// A replayed description differs from the candidate under validation.
    ArtifactMismatch,
}

impl std::fmt::Display for CodeLayoutError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "selected-application code layout failed: {self:?}"
        )
    }
}

impl std::error::Error for CodeLayoutError {}

/// Join one selected application's placed instruction spans and call
/// relocations into its `code_layout` description.
pub fn describe_selected_application_code_layout(
    section: &RelocationFreeTextSectionPlacement,
    machine: MachineId,
) -> Result<SelectedApplicationCodeLayout, CodeLayoutError> {
    let fragment = section
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .ok_or(CodeLayoutError::MissingRealization(machine))?;
    let regions = instruction_regions(fragment)?;
    let internal_calls: Vec<PlacedInternalMachineCallResolution> = section
        .resolved_internal_machine_calls
        .iter()
        .filter(|call| call.caller == machine)
        .copied()
        .collect();
    let foreign_calls: Vec<PlacedNormalizedForeignCallResolution> = section
        .unresolved_normalized_foreign_calls
        .iter()
        .filter(|call| call.caller == machine)
        .cloned()
        .collect();
    for field in internal_calls
        .iter()
        .map(|call| (call.field_section_offset, call.field_byte_width))
        .chain(
            foreign_calls
                .iter()
                .map(|call| (call.field_section_offset, call.field_byte_width)),
        )
    {
        if !field_within_regions(&regions, field.0, u64::from(field.1)) {
            return Err(CodeLayoutError::CallFieldOutsideRegions(machine));
        }
    }
    Ok(SelectedApplicationCodeLayout {
        machine,
        text_section: section.recomputed_identity(),
        entry: CodeLayoutAddress::TextSectionOffset(fragment.section_offset),
        regions,
        internal_calls,
        foreign_calls,
    })
}

/// One description per placed application, in the section's function order.
pub fn describe_code_layouts(
    section: &RelocationFreeTextSectionPlacement,
) -> Result<Vec<SelectedApplicationCodeLayout>, CodeLayoutError> {
    section
        .functions
        .iter()
        .map(|function| describe_selected_application_code_layout(section, function.machine))
        .collect()
}

/// Independent replay: recompute one application's description from the
/// section and require exact equality with the candidate.
pub fn validate_selected_application_code_layout(
    section: &RelocationFreeTextSectionPlacement,
    machine: MachineId,
    candidate: &SelectedApplicationCodeLayout,
) -> Result<(), CodeLayoutError> {
    let recomputed = describe_selected_application_code_layout(section, machine)?;
    if &recomputed != candidate {
        return Err(CodeLayoutError::ArtifactMismatch);
    }
    Ok(())
}

/// Coalesce the fragment's placed instruction spans into maximal contiguous
/// regions. Only instruction bytes form a region; fragment bytes no span
/// covers — alignment padding or an embedded pool — stay out of the
/// description.
fn instruction_regions(
    fragment: &PlacedFunctionFragment,
) -> Result<Vec<PlacedCodeRegion>, CodeLayoutError> {
    let fragment_end = fragment
        .section_offset
        .checked_add(fragment.byte_count)
        .ok_or(CodeLayoutError::OffsetOverflow)?;
    let mut spans: Vec<(u64, u64)> = fragment
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter())
        .map(|span| (span.section_offset, span.byte_count))
        .collect();
    spans.sort_unstable();
    let mut regions: Vec<PlacedCodeRegion> = Vec::new();
    let mut end = 0_u64;
    for (offset, byte_count) in spans {
        let span_end = offset
            .checked_add(byte_count)
            .ok_or(CodeLayoutError::OffsetOverflow)?;
        if offset < fragment.section_offset || span_end > fragment_end {
            return Err(CodeLayoutError::SpanOutsideFragment(fragment.machine));
        }
        match regions.last_mut() {
            Some(_) if offset < end => {
                return Err(CodeLayoutError::OverlappingInstructionSpans(
                    fragment.machine,
                ));
            }
            Some(region) if offset == end => {
                end = span_end;
                region.length = end - region.base.section_offset();
            }
            _ => {
                regions.push(PlacedCodeRegion {
                    base: CodeLayoutAddress::TextSectionOffset(offset),
                    length: byte_count,
                });
                end = span_end;
            }
        }
    }
    regions.retain(|region| region.length != 0);
    Ok(regions)
}

fn field_within_regions(regions: &[PlacedCodeRegion], offset: u64, width: u64) -> bool {
    regions.iter().any(|region| {
        offset
            .checked_sub(region.base.section_offset())
            .and_then(|delta| region.length.checked_sub(delta))
            .is_some_and(|remaining| width <= remaining)
    })
}

#[cfg(test)]
mod tests {
    use super::{
        CodeLayoutAddress, CodeLayoutError, describe_code_layouts,
        describe_selected_application_code_layout, validate_selected_application_code_layout,
    };
    use machine_code::{
        InternalMachineCallResolutionKind, InternalMachineCallResolutionState,
        NormalizedForeignCallResolutionKind, NormalizedForeignCallResolutionState, PlacedBlockSpan,
        PlacedFunctionFragment, PlacedInstructionSpan, PlacedInternalMachineCallResolution,
        PlacedNormalizedForeignCallResolution, RelocationFreeTextSectionPlacement,
        TextSectionPlacementPolicy, TextSectionRelocationRequirements,
    };
    use optimization_core::{
        FunctionFragmentEmissionIdentity, TerminalRelocationFreeTextSectionIdentity,
    };
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, SelectedBlockId, SelectedInstructionId,
        SelectedInstructionPlanIdentity,
    };
    use semantic_vocabulary::{BoundaryMachineId, FuelScheduleIdentity, MachineId, OperationId};
    use target::NativeTarget;
    use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    fn instruction(
        id: u32,
        function_offset: u64,
        section_offset: u64,
        byte_count: u64,
    ) -> PlacedInstructionSpan {
        PlacedInstructionSpan {
            instruction: SelectedInstructionId(id),
            alternative: MachineAlternativeKey {
                family: MachineAlternativeFamily::ReturnUnit,
                variant: 0,
            },
            function_offset,
            section_offset,
            byte_count,
        }
    }

    /// Two machines: the entry covers instruction bytes 0..9 and 14..20
    /// (bytes 9..14 are uncovered padding inside its fragment), and the
    /// callee covers 20..24.
    fn section() -> RelocationFreeTextSectionPlacement {
        let entry = MachineId::new(11).unwrap();
        let callee = MachineId::new(12).unwrap();
        let mut section = RelocationFreeTextSectionPlacement {
            identity: TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(b"pending"),
            source_fragments: FunctionFragmentEmissionIdentity::from_canonical_bytes(b"fragments"),
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
            },
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            selected: SelectedInstructionPlanIdentity::from_bytes([8; 32]),
            target: NativeTarget::linux_x64(),
            semantic_entry: entry,
            semantic_entry_offset: 0,
            policy: TextSectionPlacementPolicy::DenseValidatedFragmentOrderNoPaddingV1,
            section_alignment: 1,
            byte_count: 24,
            bytes: vec![0; 24],
            functions: vec![
                PlacedFunctionFragment {
                    source_function_index: 0,
                    machine: entry,
                    section_offset: 0,
                    byte_count: 20,
                    blocks: vec![
                        PlacedBlockSpan {
                            block: SelectedBlockId(1),
                            function_offset: 0,
                            section_offset: 0,
                            byte_count: 12,
                            instructions: vec![
                                instruction(1, 0, 0, 5),
                                instruction(2, 5, 5, 4),
                            ],
                        },
                        PlacedBlockSpan {
                            block: SelectedBlockId(2),
                            function_offset: 14,
                            section_offset: 14,
                            byte_count: 6,
                            instructions: vec![
                                instruction(3, 14, 14, 4),
                                instruction(4, 18, 18, 2),
                            ],
                        },
                    ],
                },
                PlacedFunctionFragment {
                    source_function_index: 1,
                    machine: callee,
                    section_offset: 20,
                    byte_count: 4,
                    blocks: vec![PlacedBlockSpan {
                        block: SelectedBlockId(1),
                        function_offset: 0,
                        section_offset: 20,
                        byte_count: 4,
                        instructions: vec![instruction(5, 0, 20, 4)],
                    }],
                },
            ],
            resolved_internal_machine_calls: vec![PlacedInternalMachineCallResolution {
                kind: InternalMachineCallResolutionKind::X86Relative32FromNextInstructionToInternalMachineV1,
                state: InternalMachineCallResolutionState::ResolvedInSectionV1,
                caller: entry,
                block: SelectedBlockId(1),
                instruction: SelectedInstructionId(1),
                owner: target_operations::CallSiteOwner::Operation(OperationId::new(3).unwrap()),
                callee,
                call_function_offset: 0,
                call_section_offset: 0,
                call_byte_count: 5,
                opcode_function_offset: 0,
                opcode_section_offset: 0,
                field_function_offset: 1,
                field_section_offset: 1,
                next_instruction_function_offset: 5,
                next_instruction_section_offset: 5,
                callee_section_offset: 20,
                field_byte_width: 4,
                addend: 0,
                displacement: 15,
            }],
            unresolved_normalized_foreign_calls: vec![
                PlacedNormalizedForeignCallResolution {
                    kind: NormalizedForeignCallResolutionKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1,
                    state: NormalizedForeignCallResolutionState::UnresolvedImportFieldV1,
                    caller: callee,
                    block: SelectedBlockId(1),
                    instruction: SelectedInstructionId(5),
                    operation: OperationId::new(4).unwrap(),
                    boundary: BoundaryMachineId::new(2).unwrap(),
                    ordinal: 1,
                    call_function_offset: 0,
                    call_section_offset: 20,
                    call_byte_count: 4,
                    opcode_function_offset: 0,
                    opcode_section_offset: 20,
                    field_function_offset: 0,
                    field_section_offset: 20,
                    next_instruction_function_offset: 4,
                    next_instruction_section_offset: 24,
                    field_byte_width: 4,
                    addend: 0,
                    callback_materializations: Vec::new(),
                },
            ],
            unresolved_byte_literal_addresses: Vec::new(),
            relocation_requirements:
                TextSectionRelocationRequirements::UnresolvedNormalizedForeignImportFieldsV1,
        };
        section.identity = section.recomputed_identity();
        section
    }

    #[test]
    fn description_covers_exact_instruction_regions_without_padding_or_callees() {
        let section = section();
        let entry = MachineId::new(11).unwrap();
        let layout = describe_selected_application_code_layout(&section, entry).unwrap();
        assert_eq!(layout.machine, entry);
        assert_eq!(layout.text_section, section.recomputed_identity());
        assert_eq!(layout.entry, CodeLayoutAddress::TextSectionOffset(0));
        assert_eq!(
            layout
                .regions
                .iter()
                .map(|region| (region.base.section_offset(), region.length))
                .collect::<Vec<_>>(),
            vec![(0, 9), (14, 6)]
        );
        // The resolved call is joined to its caller; the callee's 20..24
        // region is not pulled into the caller's description.
        assert_eq!(layout.internal_calls.len(), 1);
        assert_eq!(layout.internal_calls[0].callee, MachineId::new(12).unwrap());
        assert!(layout.foreign_calls.is_empty());
    }

    #[test]
    fn description_binds_foreign_import_fields_to_their_caller() {
        let section = section();
        let callee = MachineId::new(12).unwrap();
        let layout = describe_selected_application_code_layout(&section, callee).unwrap();
        assert_eq!(layout.entry, CodeLayoutAddress::TextSectionOffset(20));
        assert_eq!(
            layout
                .regions
                .iter()
                .map(|region| (region.base.section_offset(), region.length))
                .collect::<Vec<_>>(),
            vec![(20, 4)]
        );
        assert!(layout.internal_calls.is_empty());
        assert_eq!(layout.foreign_calls.len(), 1);
        assert_eq!(layout.foreign_calls[0].field_section_offset, 20);
    }

    #[test]
    fn describe_all_covers_every_placed_application_in_order() {
        let section = section();
        let layouts = describe_code_layouts(&section).unwrap();
        assert_eq!(
            layouts
                .iter()
                .map(|layout| layout.machine)
                .collect::<Vec<_>>(),
            vec![MachineId::new(11).unwrap(), MachineId::new(12).unwrap()]
        );
    }

    #[test]
    fn unplaced_machine_rejects_instead_of_inventing_a_region() {
        let section = section();
        assert_eq!(
            describe_selected_application_code_layout(&section, MachineId::new(77).unwrap()),
            Err(CodeLayoutError::MissingRealization(
                MachineId::new(77).unwrap()
            ))
        );
    }

    #[test]
    fn overlapping_spans_reject() {
        let mut section = section();
        section.functions[0].blocks[0].instructions[1].section_offset = 4;
        assert_eq!(
            describe_selected_application_code_layout(&section, MachineId::new(11).unwrap()),
            Err(CodeLayoutError::OverlappingInstructionSpans(
                MachineId::new(11).unwrap()
            ))
        );
    }

    #[test]
    fn span_outside_the_fragment_rejects() {
        let mut section = section();
        section.functions[0].blocks[1].instructions[1].section_offset = 19;
        section.functions[0].blocks[1].instructions[1].byte_count = 4;
        assert_eq!(
            describe_selected_application_code_layout(&section, MachineId::new(11).unwrap()),
            Err(CodeLayoutError::SpanOutsideFragment(
                MachineId::new(11).unwrap()
            ))
        );
    }

    #[test]
    fn call_field_outside_the_callers_regions_rejects() {
        let mut section = section();
        // The field sits in the fragment's uncovered padding, so joining it
        // would claim bytes no instruction span covers.
        section.resolved_internal_machine_calls[0].field_section_offset = 10;
        assert_eq!(
            describe_selected_application_code_layout(&section, MachineId::new(11).unwrap()),
            Err(CodeLayoutError::CallFieldOutsideRegions(
                MachineId::new(11).unwrap()
            ))
        );
    }

    #[test]
    fn validation_replays_the_description_exactly() {
        let section = section();
        let entry = MachineId::new(11).unwrap();
        let layout = describe_selected_application_code_layout(&section, entry).unwrap();
        assert_eq!(
            validate_selected_application_code_layout(&section, entry, &layout),
            Ok(())
        );
        let mut tampered = layout.clone();
        tampered.regions[1].length = 7;
        assert_eq!(
            validate_selected_application_code_layout(&section, entry, &tampered),
            Err(CodeLayoutError::ArtifactMismatch)
        );
    }
}
