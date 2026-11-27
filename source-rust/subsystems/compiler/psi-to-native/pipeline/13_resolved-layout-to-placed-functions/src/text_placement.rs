/*
We place frame-applied function fragments into a single dense text section and
resolve every ordinary internal machine call in section coordinates.

`stage_optimized_fixed_frame_text_section` validates per-architecture span
alignment, concatenates framed function fragments in validated order without
padding, patches internal machine call displacements (`resolve`), and records
section-relative foreign-call and image-address relocation fields
(`resolve_foreign`, `place_image_address`). The placed text carries the
optimization selections the object records.
`validate_optimized_fixed_frame_text_section` replays the retained frame
application, independently verifies every placed span, patched call field, and
unchanged byte range (`validate_fragment_text_section`), checks that every
internal call was resolved, and compares the carried selections with the
retained realization's.
*/

use crate::code_layout::requested_native_code_layouts;
use crate::fragment_emission::FunctionFragmentEmissionError;
use crate::frame_application::{
    FunctionFragmentFrameApplicationError, StagedFunctionFragmentFrameApplication,
    validate_function_fragment_frame_application,
};
use machine_code::code_layout::{NativeCodeLayout, NativeCodeLayoutError};
use machine_code::{
    FunctionFragment, FunctionFragmentControlProvenance, FunctionFragmentEmissionPlan,
    FunctionFragmentInternalMachineFixup, FunctionFragmentInternalMachineFixupKind,
    FunctionFragmentInternalMachineFixupState, FunctionFragmentNormalizedForeignCallFixup,
    FunctionFragmentNormalizedForeignCallFixupKind,
    FunctionFragmentNormalizedForeignCallFixupState, InternalMachineCallResolutionKind,
    InternalMachineCallResolutionState, NormalizedForeignCallResolutionKind,
    NormalizedForeignCallResolutionState, PlacedBlockSpan, PlacedFunctionFragment,
    PlacedInstructionSpan, PlacedInternalMachineCallResolution,
    PlacedNormalizedForeignCallResolution, RelocationFreeTextSectionPlacement,
    TextSectionPlacementPolicy, TextSectionRelocationRequirements,
};
use optimization_core::OptimizationSelectionIdentity;
use selected_instructions::{MachineAlternativeKey, SelectedBlockId, SelectedInstructionId};
use semantic_vocabulary::{MachineId, OperationId};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use target::Architecture;

pub(super) fn stage_optimized_fixed_frame_text_section<Contracts>(
    source: StagedFunctionFragmentFrameApplication,
    contracts: machine_code::layout::FramePlacementContracts<Contracts>,
) -> Result<FunctionPlacement<Contracts>, RelocationFreeTextSectionPlacementError> {
    let text_section = place_fixed_frame(source.fragments())?;
    let native_layouts =
        requested_native_code_layouts(&text_section, &source.application().functions);
    let text = StagedOptimizedFixedFrameTextSection {
        contracts: contracts.publication,
        encoded: contracts.encoded,
        body: Arc::clone(&source.placed_body),
        text_section: Arc::new(text_section),
        native_layouts,
        selections: realization_selections(&source),
    };
    Ok(FunctionPlacement {
        text,
        frame_application: contracts.retain_original.then_some(source),
    })
}

/// Placement releases its working layout when it issues ordinary text. An audit
/// request retains the frame application separately for independent replay.
pub struct FunctionPlacement<Contracts> {
    text: StagedOptimizedFixedFrameTextSection<Contracts>,
    frame_application: Option<StagedFunctionFragmentFrameApplication>,
}

impl<Contracts> FunctionPlacement<Contracts> {
    pub fn into_parts(
        self,
    ) -> (
        StagedOptimizedFixedFrameTextSection<Contracts>,
        Option<StagedFunctionFragmentFrameApplication>,
    ) {
        (self.text, self.frame_application)
    }
}

pub fn validate_optimized_fixed_frame_text_section<Contracts>(
    staged: &StagedOptimizedFixedFrameTextSection<Contracts>,
    original: &StagedFunctionFragmentFrameApplication,
) -> Result<(), RelocationFreeTextSectionPlacementError> {
    if staged.body.as_ref() != original.placed_body.as_ref() {
        return Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch);
    }
    validate_function_fragment_frame_application(original)
        .map_err(RelocationFreeTextSectionPlacementError::FrameSource)?;
    check_placed_section(original.fragments(), &staged.text_section)?;
    check_internal_calls_resolved(&staged.text_section, original.fragments())?;
    if staged.selections != realization_selections(original) {
        return Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch);
    }
    if staged.native_layouts
        != requested_native_code_layouts(&staged.text_section, &original.application().functions)
    {
        return Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch);
    }
    Ok(())
}

/// The object records the selections the realization was optimized under.
fn realization_selections(
    source: &StagedFunctionFragmentFrameApplication,
) -> OptimizationSelectionIdentity {
    source
        .source()
        .source()
        .function_relative_manifest()
        .record()
        .selections
}

/// Placed text carries its code and selections. Only an audit can retain the working layout.
/// ```compile_fail,E0599
/// fn recover_layout<C>(text: &function_layout::StagedOptimizedFixedFrameTextSection<C>) {
///     let _ = text.source();
/// }
/// ```
#[derive(Debug)]
#[must_use = "a staged fixed-frame text section owns its exact frame-application custody"]
pub struct StagedOptimizedFixedFrameTextSection<Contracts> {
    contracts: Arc<Contracts>,
    encoded: machine_code::program_context::BoundEncodingSubject,
    body: Arc<crate::PlacedBodyEvidence>,
    text_section: Arc<RelocationFreeTextSectionPlacement>, // Placed dense text section with resolved internal calls.
    native_layouts: Result<Arc<[NativeCodeLayout]>, NativeCodeLayoutError>,
    selections: OptimizationSelectionIdentity, // Selections the placed code was optimized under.
}

impl<Contracts> StagedOptimizedFixedFrameTextSection<Contracts> {
    pub fn text_section(&self) -> &RelocationFreeTextSectionPlacement {
        self.text_section.as_ref()
    }

    /// Placement transports a body's contracts without inspecting the semantic record.
    /// ```compile_fail,E0609
    /// fn read_earlier_facts<C>(text: &function_layout::StagedOptimizedFixedFrameTextSection<C>) {
    ///     let _ = &text.contracts().facts;
    /// }
    /// ```
    pub fn contracts(&self) -> &Arc<Contracts> {
        &self.contracts
    }

    /// The settled layout preserved this encoding and frame through placement.
    pub fn encoding_subject(&self) -> machine_code::program_context::BoundEncodingSubject {
        self.encoded.clone()
    }

    pub fn placed_body(&self) -> &Arc<crate::PlacedBodyEvidence> {
        &self.body
    }

    pub fn shared_text_section(&self) -> Arc<RelocationFreeTextSectionPlacement> {
        Arc::clone(&self.text_section)
    }

    /// Attributions in first-request order. A missing realization is reported when
    /// the image requests its layout, just as it is for an individual layout query.
    pub fn shared_native_code_layouts(
        &self,
    ) -> Result<Arc<[NativeCodeLayout]>, NativeCodeLayoutError> {
        self.native_layouts.clone()
    }

    pub const fn selections(&self) -> OptimizationSelectionIdentity {
        self.selections
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelocationFreeTextSectionPlacementError {
    Source(FunctionFragmentEmissionError),
    FrameSource(FunctionFragmentFrameApplicationError),
    DuplicateFunction(MachineId),
    MissingSemanticEntry(MachineId),
    OffsetOverflow,
    SourceShapeMismatch,
    MisalignedAarch64Span,
    UnresolvedInternalMachineFixups,
    MissingInternalMachineTarget(MachineId),
    InternalCallOutOfRange,
    ArtifactMismatch,
}

diagnostics::failed_display!(
    RelocationFreeTextSectionPlacementError,
    "optimized relocation-free text-section placement failed"
);

impl std::error::Error for RelocationFreeTextSectionPlacementError {}

fn validate_invocation_site(
    site: semantic_vocabulary::InvocationSite,
    operations: &[OperationId],
    edges: &[semantic_vocabulary::EdgeId],
) -> Result<(), RelocationFreeTextSectionPlacementError> {
    let matches = match site {
        semantic_vocabulary::InvocationSite::Operation(operation) => {
            operations.last() == Some(&operation) && edges.is_empty()
        }
        semantic_vocabulary::InvocationSite::ExternalCompletion(edge)
        | semantic_vocabulary::InvocationSite::InvokeNoNormal(edge)
        | semantic_vocabulary::InvocationSite::Cleanup { edge, .. } => edges == [edge],
    };
    if !matches {
        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
    }
    Ok(())
}

fn usize_to_u64(value: usize) -> Result<u64, RelocationFreeTextSectionPlacementError> {
    u64::try_from(value).map_err(|_| RelocationFreeTextSectionPlacementError::OffsetOverflow)
}

/// Every internal machine call in the fragments must be patched in the section.
fn check_internal_calls_resolved(
    section: &RelocationFreeTextSectionPlacement,
    fragments: &FunctionFragmentEmissionPlan,
) -> Result<(), RelocationFreeTextSectionPlacementError> {
    let source = fragments
        .instructions()
        .filter(|instruction| instruction.internal_machine_fixup.is_some())
        .count();
    if source != section.resolved_internal_machine_calls.len() {
        return Err(RelocationFreeTextSectionPlacementError::UnresolvedInternalMachineFixups);
    }
    Ok(())
}

fn validate_alignment(
    architecture: Architecture,
    offset: u64,
    byte_count: u64,
) -> Result<(), RelocationFreeTextSectionPlacementError> {
    if architecture == Architecture::Aarch64
        && (!offset.is_multiple_of(4) || !byte_count.is_multiple_of(4))
    {
        return Err(RelocationFreeTextSectionPlacementError::MisalignedAarch64Span);
    }
    Ok(())
}

fn place_block_spans(
    architecture: Architecture,
    function: &FunctionFragment,
    function_section_offset: u64,
) -> Result<Vec<PlacedBlockSpan>, RelocationFreeTextSectionPlacementError> {
    let mut blocks = Vec::with_capacity(function.blocks.len());
    for block in &function.blocks {
        validate_alignment(architecture, block.offset, block.byte_count)?;
        let section_offset = function_section_offset
            .checked_add(block.offset)
            .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
        if block
            .offset
            .checked_add(block.byte_count)
            .is_none_or(|end| end > function.byte_count)
        {
            return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
        }
        let mut instructions = Vec::with_capacity(block.instructions.len());
        for row in &block.instructions {
            let byte_count = usize_to_u64(row.bytes.len())?;
            validate_alignment(architecture, row.offset, byte_count)?;
            let row_section_offset = function_section_offset
                .checked_add(row.offset)
                .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
            if row
                .offset
                .checked_add(byte_count)
                .is_none_or(|end| end > function.byte_count)
            {
                return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
            }
            instructions.push(PlacedInstructionSpan {
                instruction: row.instruction,
                alternative: row.alternative,
                function_offset: row.offset,
                section_offset: row_section_offset,
                byte_count,
            });
        }
        blocks.push(PlacedBlockSpan {
            block: block.block,
            function_offset: block.offset,
            section_offset,
            byte_count: block.byte_count,
            instructions,
        });
    }
    Ok(blocks)
}

fn place_fixed_frame(
    fragments: &FunctionFragmentEmissionPlan,
) -> Result<RelocationFreeTextSectionPlacement, RelocationFreeTextSectionPlacementError> {
    let section_alignment = match fragments.target.architecture {
        Architecture::Aarch64 => 4,
        Architecture::X86_64 => 1,
    };
    let mut offsets = BTreeMap::new();
    let mut next_offset = 0_u64;
    for function in &fragments.functions {
        if offsets.insert(function.machine, next_offset).is_some() {
            return Err(RelocationFreeTextSectionPlacementError::DuplicateFunction(
                function.machine,
            ));
        }
        validate_alignment(
            fragments.target.architecture,
            next_offset,
            function.byte_count,
        )?;
        if usize_to_u64(function.bytes.len())? != function.byte_count {
            return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
        }
        next_offset = next_offset
            .checked_add(function.byte_count)
            .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
    }

    let semantic_entry_offset = *offsets
        .get(&fragments.entry)
        .ok_or(RelocationFreeTextSectionPlacementError::MissingSemanticEntry(fragments.entry))?;
    let mut bytes = Vec::with_capacity(
        usize::try_from(next_offset)
            .map_err(|_| RelocationFreeTextSectionPlacementError::OffsetOverflow)?,
    );
    let mut functions = Vec::with_capacity(fragments.functions.len());
    let mut resolved = Vec::new();
    let mut unresolved_foreign = Vec::new();
    let mut unresolved_addresses = Vec::new();
    let mut seen_entry = BTreeSet::new();

    for (source_function_index, function) in fragments.functions.iter().enumerate() {
        if !seen_entry.insert(function.machine) {
            return Err(RelocationFreeTextSectionPlacementError::DuplicateFunction(
                function.machine,
            ));
        }
        let section_offset = offsets[&function.machine];
        let mut function_bytes = function.bytes.clone();
        for block in &function.blocks {
            for instruction in &block.instructions {
                if let Some(fixup) = instruction.internal_machine_fixup() {
                    let FunctionFragmentControlProvenance::InternalCall {
                        target: selected_instructions::SelectedCallTarget::Machine(callee),
                        owner,
                    } = instruction.control
                    else {
                        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
                    };
                    if callee != fixup.callee {
                        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
                    }
                    match owner {
                        target_vocabulary::CallSiteOwner::Operation(operation)
                            if exact_operation(&instruction.provenance.operations)?
                                == operation => {}
                        target_vocabulary::CallSiteOwner::InvokeNoNormal(edge) => {
                            validate_invocation_site(
                                semantic_vocabulary::InvocationSite::InvokeNoNormal(edge),
                                &instruction.provenance.operations,
                                &instruction.provenance.edges,
                            )?;
                        }
                        target_vocabulary::CallSiteOwner::ExternalCompletion(edge) => {
                            validate_invocation_site(
                                semantic_vocabulary::InvocationSite::ExternalCompletion(edge),
                                &instruction.provenance.operations,
                                &instruction.provenance.edges,
                            )?;
                        }
                        target_vocabulary::CallSiteOwner::Installation { operation, .. }
                            if instruction.provenance.operations.as_slice() == [operation]
                                && instruction.provenance.edges.is_empty() => {}
                        target_vocabulary::CallSiteOwner::CleanupAction { edge, .. }
                            if instruction.provenance.operations.is_empty()
                                && instruction.provenance.edges.as_slice() == [edge]
                                && instruction.provenance.work.is_empty() => {}
                        _ => {
                            return Err(
                                RelocationFreeTextSectionPlacementError::SourceShapeMismatch,
                            );
                        }
                    }
                    let callee_section_offset = *offsets.get(&callee).ok_or(
                        RelocationFreeTextSectionPlacementError::MissingInternalMachineTarget(
                            callee,
                        ),
                    )?;
                    resolved.push(resolve(
                        fragments.target.architecture,
                        function.machine,
                        block.block,
                        instruction.instruction,
                        owner,
                        instruction.offset,
                        &instruction.bytes,
                        section_offset,
                        callee_section_offset,
                        fixup,
                        &mut function_bytes,
                    )?);
                }
                if let Some(fixup) = instruction.normalized_foreign_call_fixup().cloned() {
                    let FunctionFragmentControlProvenance::NormalizedForeignCall {
                        invocation_site,
                        boundary,
                        ordinal,
                    } = instruction.control
                    else {
                        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
                    };
                    if boundary != fixup.boundary || ordinal != fixup.ordinal {
                        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
                    }
                    validate_invocation_site(
                        invocation_site,
                        &instruction.provenance.operations,
                        &instruction.provenance.edges,
                    )?;
                    unresolved_foreign.push(resolve_foreign(
                        fragments.target.architecture,
                        function.machine,
                        block.block,
                        instruction.instruction,
                        invocation_site,
                        instruction.offset,
                        &instruction.bytes,
                        section_offset,
                        fixup,
                        &function_bytes,
                    )?);
                }
                if let Some(fixup) = instruction.image_address_fixup() {
                    unresolved_addresses.push(place_image_address(
                        function.machine,
                        block.block,
                        instruction,
                        section_offset,
                        fixup,
                        &function_bytes,
                    )?);
                }
            }
        }
        let blocks = place_block_spans(fragments.target.architecture, function, section_offset)?;
        bytes.extend_from_slice(&function_bytes);
        functions.push(PlacedFunctionFragment {
            source_function_index: usize_to_u64(source_function_index)?,
            machine: function.machine,
            section_offset,
            byte_count: function.byte_count,
            blocks,
        });
    }

    let source_fixups = fragments
        .instructions()
        .filter(|instruction| instruction.internal_machine_fixup.is_some())
        .count();
    let source_foreign = fragments
        .instructions()
        .filter(|instruction| instruction.normalized_foreign_call_fixup.is_some())
        .count();
    let source_addresses = fragments
        .instructions()
        .filter(|instruction| instruction.image_address_fixup.is_some())
        .count();
    if resolved.len() != source_fixups
        || unresolved_foreign.len() != source_foreign
        || unresolved_addresses.len() != source_addresses
        || usize_to_u64(bytes.len())? != next_offset
    {
        return Err(RelocationFreeTextSectionPlacementError::UnresolvedInternalMachineFixups);
    }

    Ok(RelocationFreeTextSectionPlacement {
        psi: fragments.psi,
        work_schedule: fragments.work_schedule,
        selected: fragments.selected,
        target: fragments.target,
        semantic_entry: fragments.entry,
        semantic_entry_offset,
        policy: TextSectionPlacementPolicy::DenseValidatedFragmentOrderNoPaddingV1,
        section_alignment,
        byte_count: next_offset,
        bytes,
        functions,
        resolved_internal_machine_calls: resolved,
        relocation_requirements: if !unresolved_addresses.is_empty() {
            TextSectionRelocationRequirements::UnresolvedObjectSymbolFieldsV1
        } else if unresolved_foreign.is_empty() {
            TextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1
        } else {
            TextSectionRelocationRequirements::UnresolvedNormalizedForeignImportFieldsV1
        },
        unresolved_normalized_foreign_calls: unresolved_foreign,
        unresolved_image_addresses: unresolved_addresses,
    })
}

fn check_address_fields(
    instruction: &machine_code::FunctionFragmentInstructionSpan,
    fields: machine_code::ImageAddressFields,
    function_bytes: &[u8],
) -> Result<(), RelocationFreeTextSectionPlacementError> {
    let instruction_end = instruction
        .offset
        .checked_add(usize_to_u64(instruction.bytes.len())?)
        .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
    let field = |offset: u64| {
        if offset < instruction.offset || offset.checked_add(4)? > instruction_end {
            return None;
        }
        let start = usize::try_from(offset).ok()?;
        function_bytes.get(start..start.checked_add(4)?)
    };
    let canonical = match fields {
        machine_code::ImageAddressFields::X86_64Relative32 { patch_offset } => {
            field(patch_offset) == Some(&[0, 0, 0, 0][..])
        }
        machine_code::ImageAddressFields::Aarch64PageAddress {
            page_patch_offset,
            page_offset_patch_offset,
        } => {
            let word = |offset| {
                field(offset)
                    .map(|field| u32::from_le_bytes([field[0], field[1], field[2], field[3]]))
            };
            word(page_patch_offset).is_some_and(|word| word & 0x9fff_ffe0 == 0x9000_0000)
                && word(page_offset_patch_offset)
                    .is_some_and(|word| word & 0xffff_fc00 == 0x9100_0000)
        }
    };
    if !canonical || instruction.branch.is_some() {
        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
    }
    Ok(())
}

fn place_image_address(
    machine: MachineId,
    block: SelectedBlockId,
    instruction: &machine_code::FunctionFragmentInstructionSpan,
    section_offset: u64,
    fixup: machine_code::FunctionFragmentImageAddressFixup,
    function_bytes: &[u8],
) -> Result<machine_code::PlacedImageAddress, RelocationFreeTextSectionPlacementError> {
    check_address_fields(instruction, fixup.fields, function_bytes)?;
    Ok(machine_code::PlacedImageAddress {
        machine,
        block,
        instruction: instruction.instruction,
        reference: fixup.reference,
        operation: exact_operation(&instruction.provenance.operations)?,
        instruction_function_offset: instruction.offset,
        instruction_section_offset: section_offset
            .checked_add(instruction.offset)
            .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?,
        instruction_byte_count: usize_to_u64(instruction.bytes.len())?,
        fields: fixup
            .fields
            .shifted(section_offset)
            .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?,
    })
}

fn exact_operation(
    operations: &[OperationId],
) -> Result<OperationId, RelocationFreeTextSectionPlacementError> {
    operations
        .last()
        .copied()
        .ok_or(RelocationFreeTextSectionPlacementError::SourceShapeMismatch)
}

fn section_coordinate(
    base: u64,
    offset: u64,
) -> Result<u64, RelocationFreeTextSectionPlacementError> {
    base.checked_add(offset)
        .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)
}

#[allow(clippy::too_many_arguments)]
fn resolve(
    architecture: Architecture,
    caller: MachineId,
    block: SelectedBlockId,
    instruction: SelectedInstructionId,
    owner: target_vocabulary::CallSiteOwner,
    instruction_offset: u64,
    instruction_bytes: &[u8],
    function_section_offset: u64,
    callee_section_offset: u64,
    fixup: FunctionFragmentInternalMachineFixup,
    function_bytes: &mut [u8],
) -> Result<PlacedInternalMachineCallResolution, RelocationFreeTextSectionPlacementError> {
    if fixup.state != FunctionFragmentInternalMachineFixupState::UnresolvedZeroFieldV1
        || fixup.opcode_function_offset != instruction_offset
        || fixup.addend != 0
    {
        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
    }
    let section_at = |offset| section_coordinate(function_section_offset, offset);
    let call_section_offset = section_at(instruction_offset)?;
    let (kind, displacement, patch_bytes) = match (architecture, fixup.kind) {
        (
            Architecture::X86_64,
            FunctionFragmentInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1,
        ) => {
            if instruction_bytes != [0xe8, 0, 0, 0, 0]
                || fixup.patch_function_offset != instruction_offset + 1
                || fixup.reference_function_offset != instruction_offset + 5
                || fixup.patch_byte_width != 4
            {
                return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
            }
            let reference = function_section_offset
                .checked_add(fixup.reference_function_offset)
                .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
            let displacement =
                i32::try_from(i128::from(callee_section_offset) - i128::from(reference))
                    .map_err(|_| RelocationFreeTextSectionPlacementError::InternalCallOutOfRange)?;
            (
                InternalMachineCallResolutionKind::X86Relative32FromNextInstructionToInternalMachineV1,
                displacement,
                displacement.to_le_bytes(),
            )
        }
        (
            Architecture::Aarch64,
            FunctionFragmentInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1,
        ) => {
            if instruction_bytes != 0x9400_0000_u32.to_le_bytes()
                || fixup.patch_function_offset != instruction_offset
                || fixup.reference_function_offset != instruction_offset
                || fixup.patch_byte_width != 4
            {
                return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
            }
            let byte_displacement =
                i128::from(callee_section_offset) - i128::from(call_section_offset);
            if byte_displacement % 4 != 0 {
                return Err(RelocationFreeTextSectionPlacementError::InternalCallOutOfRange);
            }
            let word_displacement = byte_displacement / 4;
            if !(-(1_i128 << 25)..(1_i128 << 25)).contains(&word_displacement) {
                return Err(RelocationFreeTextSectionPlacementError::InternalCallOutOfRange);
            }
            let immediate = u32::try_from(word_displacement.rem_euclid(1_i128 << 26))
                .map_err(|_| RelocationFreeTextSectionPlacementError::InternalCallOutOfRange)?;
            let encoded = 0x9400_0000_u32 | immediate;
            (
                InternalMachineCallResolutionKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1,
                i32::try_from(byte_displacement)
                    .map_err(|_| RelocationFreeTextSectionPlacementError::InternalCallOutOfRange)?,
                encoded.to_le_bytes(),
            )
        }
        _ => return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch),
    };

    let patch_start = usize::try_from(fixup.patch_function_offset)
        .map_err(|_| RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
    let patch_end = patch_start
        .checked_add(usize::from(fixup.patch_byte_width))
        .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
    let instruction_start = usize::try_from(instruction_offset)
        .map_err(|_| RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
    let instruction_end = instruction_start
        .checked_add(instruction_bytes.len())
        .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
    if function_bytes.get(instruction_start..instruction_end) != Some(instruction_bytes) {
        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
    }
    function_bytes
        .get_mut(patch_start..patch_end)
        .ok_or(RelocationFreeTextSectionPlacementError::SourceShapeMismatch)?
        .copy_from_slice(&patch_bytes);

    Ok(PlacedInternalMachineCallResolution {
        kind,
        state: InternalMachineCallResolutionState::ResolvedInSectionV1,
        caller,
        block,
        instruction,
        owner,
        callee: fixup.callee,
        call_function_offset: instruction_offset,
        call_section_offset,
        call_byte_count: usize_to_u64(instruction_bytes.len())?,
        opcode_function_offset: fixup.opcode_function_offset,
        opcode_section_offset: section_at(fixup.opcode_function_offset)?,
        field_function_offset: fixup.patch_function_offset,
        field_section_offset: section_at(fixup.patch_function_offset)?,
        next_instruction_function_offset: fixup.reference_function_offset,
        next_instruction_section_offset: section_at(fixup.reference_function_offset)?,
        callee_section_offset,
        field_byte_width: fixup.patch_byte_width,
        addend: fixup.addend,
        displacement,
    })
}

#[allow(clippy::too_many_arguments)]
fn resolve_foreign(
    architecture: Architecture,
    caller: MachineId,
    block: SelectedBlockId,
    instruction: SelectedInstructionId,
    invocation_site: semantic_vocabulary::InvocationSite,
    instruction_offset: u64,
    instruction_bytes: &[u8],
    function_section_offset: u64,
    fixup: FunctionFragmentNormalizedForeignCallFixup,
    function_bytes: &[u8],
) -> Result<PlacedNormalizedForeignCallResolution, RelocationFreeTextSectionPlacementError> {
    if (invocation_site.operation().is_none() && !fixup.callbacks.is_empty())
        || fixup.state != FunctionFragmentNormalizedForeignCallFixupState::UnresolvedImportFieldV1
        || fixup.opcode_function_offset < instruction_offset
        || fixup.addend != 0
    {
        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
    }
    let section_at = |offset| section_coordinate(function_section_offset, offset);
    let mut cursor = 0_u64;
    let mut check_group =
        |callback: &machine_code::FunctionFragmentCallbackMaterialization,
         patch_offsets: &[u64]|
         -> Result<(), RelocationFreeTextSectionPlacementError> {
            let group_start = instruction_offset
                .checked_add(cursor)
                .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
            let group_end = cursor
                .checked_add(callback.code_byte_count)
                .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
            let group = usize::try_from(group_end)
                .ok()
                .and_then(|end| instruction_bytes.get(cursor as usize..end))
                .ok_or(RelocationFreeTextSectionPlacementError::SourceShapeMismatch)?;
            if callback.code_function_offset != group_start {
                return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
            }
            let absolute: Vec<u64> = match callback.encoding {
                machine_code::FunctionFragmentCallbackMaterializationEncoding::X86_64Relative32 {
                    patch_function_offset,
                } => vec![patch_function_offset],
                machine_code::FunctionFragmentCallbackMaterializationEncoding::Aarch64PageAddress {
                    page_patch_function_offset,
                    page_offset_patch_function_offset,
                } => vec![page_patch_function_offset, page_offset_patch_function_offset],
            };
            if absolute.len() != patch_offsets.len()
                || !absolute
                    .iter()
                    .zip(patch_offsets)
                    .all(|(absolute, relative)| *absolute == group_start + relative)
            {
                return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
            }
            // The encoded recipe is already carried by the current form.
            // Placement only needs the declared, still-unbound relocation fields.
            for (index, offset) in patch_offsets.iter().enumerate() {
                let field = usize::try_from(*offset)
                    .ok()
                    .and_then(|start| start.checked_add(4).and_then(|end| group.get(start..end)))
                    .ok_or(RelocationFreeTextSectionPlacementError::SourceShapeMismatch)?;
                let value =
                    u32::from_le_bytes(field.try_into().map_err(|_| {
                        RelocationFreeTextSectionPlacementError::SourceShapeMismatch
                    })?);
                let mask = match architecture {
                    Architecture::X86_64 => u32::MAX,
                    Architecture::Aarch64 if index == 0 => 0x60ff_ffe0,
                    Architecture::Aarch64 => 0x003f_fc00,
                };
                if value & mask != 0 {
                    return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
                }
            }
            cursor = group_end;
            Ok(())
        };
    let kind = match (architecture, fixup.kind) {
        (
            Architecture::X86_64,
            FunctionFragmentNormalizedForeignCallFixupKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1,
        ) => {
            for callback in &fixup.callbacks {
                let machine_code::FunctionFragmentCallbackMaterializationEncoding::X86_64Relative32 { .. } =
                    callback.encoding
                else {
                    return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
                };
                check_group(callback, &[3])?;
            }
            let call_offset = instruction_offset
                .checked_add(cursor)
                .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
            if instruction_bytes.get(cursor as usize..) != Some(&[0xe8_u8, 0, 0, 0, 0][..])
                || fixup.opcode_function_offset != call_offset
                || fixup.patch_function_offset != call_offset + 1
                || fixup.reference_function_offset != call_offset + 5
                || fixup.patch_byte_width != 4
            {
                return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
            }
            NormalizedForeignCallResolutionKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1
        }
        (
            Architecture::Aarch64,
            FunctionFragmentNormalizedForeignCallFixupKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1,
        ) => {
            for callback in &fixup.callbacks {
                let machine_code::FunctionFragmentCallbackMaterializationEncoding::Aarch64PageAddress { .. } =
                    callback.encoding
                else {
                    return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
                };
                if matches!(callback.destination, machine_code::CallbackAddressDestination::OutgoingStack { .. }) {
                    return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
                }
                check_group(callback, &[0, 4])?;
            }
            let call_offset = instruction_offset
                .checked_add(cursor)
                .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
            if instruction_bytes.get(cursor as usize..)
                != Some(&0x9400_0000_u32.to_le_bytes()[..])
                || fixup.opcode_function_offset != call_offset
                || fixup.patch_function_offset != call_offset
                || fixup.reference_function_offset != call_offset
                || fixup.patch_byte_width != 4
            {
                return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
            }
            NormalizedForeignCallResolutionKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1
        }
        _ => return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch),
    };

    let instruction_start = usize::try_from(instruction_offset)
        .map_err(|_| RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
    let instruction_end = instruction_start
        .checked_add(instruction_bytes.len())
        .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
    if function_bytes.get(instruction_start..instruction_end) != Some(instruction_bytes) {
        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
    }

    Ok(PlacedNormalizedForeignCallResolution {
        kind,
        state: NormalizedForeignCallResolutionState::UnresolvedImportFieldV1,
        caller,
        block,
        instruction,
        invocation_site,
        boundary: fixup.boundary,
        ordinal: fixup.ordinal,
        call_function_offset: instruction_offset,
        call_section_offset: section_at(instruction_offset)?,
        call_byte_count: usize_to_u64(instruction_bytes.len())?,
        opcode_function_offset: fixup.opcode_function_offset,
        opcode_section_offset: section_at(fixup.opcode_function_offset)?,
        field_function_offset: fixup.patch_function_offset,
        field_section_offset: section_at(fixup.patch_function_offset)?,
        next_instruction_function_offset: fixup.reference_function_offset,
        next_instruction_section_offset: section_at(fixup.reference_function_offset)?,
        field_byte_width: fixup.patch_byte_width,
        addend: fixup.addend,
        callback_materializations: fixup
            .callbacks
            .iter()
            .map(|callback| {
                Ok(machine_code::PlacedCallbackAddressMaterialization {
                    destination: callback.destination.clone(),
                    code_function_offset: callback.code_function_offset,
                    code_section_offset: function_section_offset
                        .checked_add(callback.code_function_offset)
                        .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?,
                    code_byte_count: callback.code_byte_count,
                    encoding: match callback.encoding {
                        machine_code::FunctionFragmentCallbackMaterializationEncoding::X86_64Relative32 {
                            patch_function_offset,
                        } => machine_code::PlacedCallbackAddressEncoding::X86_64Relative32 {
                            relocation_function_offset: patch_function_offset,
                            relocation_section_offset: function_section_offset
                                .checked_add(patch_function_offset)
                                .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?,
                        },
                        machine_code::FunctionFragmentCallbackMaterializationEncoding::Aarch64PageAddress {
                            page_patch_function_offset,
                            page_offset_patch_function_offset,
                        } => machine_code::PlacedCallbackAddressEncoding::Aarch64PageAddress {
                            page_relocation_function_offset: page_patch_function_offset,
                            page_relocation_section_offset: function_section_offset
                                .checked_add(page_patch_function_offset)
                                .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?,
                            page_offset_relocation_function_offset:
                                page_offset_patch_function_offset,
                            page_offset_relocation_section_offset: function_section_offset
                                .checked_add(page_offset_patch_function_offset)
                                .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?,
                        },
                    },
                })
            })
            .collect::<Result<Vec<_>, RelocationFreeTextSectionPlacementError>>()?,
    })
}

fn check_placed_section(
    fragments: &FunctionFragmentEmissionPlan,
    section: &RelocationFreeTextSectionPlacement,
) -> Result<(), RelocationFreeTextSectionPlacementError> {
    let count = fragments.functions.len();
    let alignment = match fragments.target.architecture {
        Architecture::X86_64 => 1,
        Architecture::Aarch64 => 4,
    };
    if section.psi != fragments.psi
        || section.work_schedule != fragments.work_schedule
        || section.selected != fragments.selected
        || section.target != fragments.target
        || section.semantic_entry != fragments.entry
        || section.functions.len() != count
        || section.policy != TextSectionPlacementPolicy::DenseValidatedFragmentOrderNoPaddingV1
        || section.relocation_requirements != expected_requirements(fragments)
        || section.section_alignment != alignment
        || section.byte_count != section.bytes.len() as u64
    {
        return Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch);
    }
    let mut offsets = Vec::with_capacity(fragments.functions.len());
    let mut extent = 0;
    for function in &fragments.functions {
        record_source_function(
            &mut offsets,
            &mut extent,
            function.machine,
            function.byte_count,
            &function.bytes,
            alignment,
        )?;
    }

    let entry = lookup_function_offset(&offsets, fragments.entry)
        .ok_or(RelocationFreeTextSectionPlacementError::MissingSemanticEntry(fragments.entry))?;
    if section.semantic_entry_offset != entry || section.byte_count != extent {
        return Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch);
    }
    check_placed_functions(fragments, section, &offsets, alignment)
}

fn lookup_function_offset(offsets: &[(MachineId, u64)], machine: MachineId) -> Option<u64> {
    offsets
        .iter()
        .find_map(|&(candidate, offset)| (candidate == machine).then_some(offset))
}

fn expected_requirements(
    fragments: &FunctionFragmentEmissionPlan,
) -> TextSectionRelocationRequirements {
    let rows = || fragments.instructions();
    let has_foreign = rows().any(|row| row.normalized_foreign_call_fixup.is_some());
    let has_literals = rows().any(|row| row.image_address_fixup.is_some());
    if has_literals {
        TextSectionRelocationRequirements::UnresolvedObjectSymbolFieldsV1
    } else if has_foreign {
        TextSectionRelocationRequirements::UnresolvedNormalizedForeignImportFieldsV1
    } else {
        TextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1
    }
}

fn record_source_function(
    offsets: &mut Vec<(MachineId, u64)>,
    extent: &mut u64,
    machine: MachineId,
    count: u64,
    bytes: &[u8],
    alignment: u64,
) -> Result<(), RelocationFreeTextSectionPlacementError> {
    if lookup_function_offset(offsets, machine).is_some() {
        return Err(RelocationFreeTextSectionPlacementError::DuplicateFunction(
            machine,
        ));
    }
    offsets.push((machine, *extent));
    if count != bytes.len() as u64 {
        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
    }
    if !extent.is_multiple_of(alignment) || !count.is_multiple_of(alignment) {
        return Err(RelocationFreeTextSectionPlacementError::MisalignedAarch64Span);
    }
    *extent = checked_add(*extent, count)?;
    Ok(())
}

fn check_placed_functions(
    fragments: &FunctionFragmentEmissionPlan,
    section: &RelocationFreeTextSectionPlacement,
    offsets: &[(MachineId, u64)],
    alignment: u64,
) -> Result<(), RelocationFreeTextSectionPlacementError> {
    let mut resolutions = section.resolved_internal_machine_calls.iter();
    let mut foreign = section.unresolved_normalized_foreign_calls.iter();
    let mut addresses = section.unresolved_image_addresses.iter();
    for (index, (source, placed)) in fragments
        .functions
        .iter()
        .zip(&section.functions)
        .enumerate()
    {
        let offset = lookup_function_offset(offsets, source.machine)
            .ok_or(RelocationFreeTextSectionPlacementError::ArtifactMismatch)?;
        check_function_span(placed, index, source.machine, offset, source.byte_count)?;
        if placed.blocks.len() != source.blocks.len() {
            return Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch);
        }
        let candidate_bytes = slice_bytes(&section.bytes, offset, source.byte_count)?;
        let mut patches = Vec::new();
        for (source_block, placed_block) in source.blocks.iter().zip(&placed.blocks) {
            check_block_span(
                placed_block,
                source_block.block,
                source_block.offset,
                offset,
                source_block.byte_count,
                source.byte_count,
                alignment,
            )?;
            if placed_block.instructions.len() != source_block.instructions.len() {
                return Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch);
            }
            for (source_row, access_row) in source_block
                .instructions
                .iter()
                .zip(&placed_block.instructions)
            {
                check_instruction_span(
                    access_row,
                    source_row.instruction,
                    source_row.alternative,
                    source_row.offset,
                    offset,
                    source_row.bytes.len() as u64,
                    source.byte_count,
                    alignment,
                )?;
                if let Some(fixup) = source_row.internal_machine_fixup() {
                    if fixup.opcode_function_offset != source_row.offset
                        || slice_bytes(
                            &source.bytes,
                            source_row.offset,
                            source_row.bytes.len() as u64,
                        )? != source_row.bytes
                        || (fragments.target.architecture == Architecture::X86_64
                            && source_row.bytes != [0xe8, 0, 0, 0, 0])
                    {
                        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
                    }
                    let FunctionFragmentControlProvenance::InternalCall {
                        target: selected_instructions::SelectedCallTarget::Machine(callee),
                        owner,
                    } = source_row.control
                    else {
                        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
                    };
                    match owner {
                        target_vocabulary::CallSiteOwner::Operation(operation)
                            if source_row.provenance.operations.last() == Some(&operation) => {}
                        target_vocabulary::CallSiteOwner::InvokeNoNormal(edge) => {
                            validate_invocation_site(
                                semantic_vocabulary::InvocationSite::InvokeNoNormal(edge),
                                &source_row.provenance.operations,
                                &source_row.provenance.edges,
                            )?;
                        }
                        target_vocabulary::CallSiteOwner::ExternalCompletion(edge) => {
                            validate_invocation_site(
                                semantic_vocabulary::InvocationSite::ExternalCompletion(edge),
                                &source_row.provenance.operations,
                                &source_row.provenance.edges,
                            )?;
                        }
                        target_vocabulary::CallSiteOwner::Installation { operation, .. }
                            if source_row.provenance.operations.as_slice() == [operation]
                                && source_row.provenance.edges.is_empty() => {}
                        target_vocabulary::CallSiteOwner::CleanupAction { edge, .. }
                            if source_row.provenance.operations.is_empty()
                                && source_row.provenance.edges.as_slice() == [edge]
                                && source_row.provenance.work.is_empty() => {}
                        _ => {
                            return Err(
                                RelocationFreeTextSectionPlacementError::SourceShapeMismatch,
                            );
                        }
                    }
                    let candidate = resolutions
                        .next()
                        .ok_or(RelocationFreeTextSectionPlacementError::ArtifactMismatch)?;
                    patches.push(check_internal_call(
                        Call {
                            caller: source.machine,
                            block: source_block.block,
                            instruction: source_row.instruction,
                            owner,
                            callee,
                            offset: source_row.offset,
                            bytes: &source_row.bytes,
                            fixup,
                        },
                        fragments.target.architecture,
                        offset,
                        offsets,
                        candidate_bytes,
                        candidate,
                    )?);
                }
                if let Some(fixup) = source_row.normalized_foreign_call_fixup() {
                    let opcode_relative = fixup
                        .opcode_function_offset
                        .checked_sub(source_row.offset)
                        .ok_or(RelocationFreeTextSectionPlacementError::SourceShapeMismatch)?;
                    let mut group_cursor = source_row.offset;
                    for callback in &fixup.callbacks {
                        if callback.code_function_offset != group_cursor {
                            return Err(
                                RelocationFreeTextSectionPlacementError::SourceShapeMismatch,
                            );
                        }
                        group_cursor = checked_add(group_cursor, callback.code_byte_count)?;
                    }
                    if group_cursor != fixup.opcode_function_offset {
                        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
                    }
                    let call_byte_count = source_row
                        .bytes
                        .len()
                        .checked_sub(opcode_relative as usize)
                        .map(|count| count as u64)
                        .ok_or(RelocationFreeTextSectionPlacementError::SourceShapeMismatch)?;
                    if slice_bytes(
                        &source.bytes,
                        source_row.offset,
                        source_row.bytes.len() as u64,
                    )? != source_row.bytes
                        || (fragments.target.architecture == Architecture::X86_64
                            && (call_byte_count != 5
                                || slice_bytes(&source_row.bytes, opcode_relative, 5)?
                                    != [0xe8, 0, 0, 0, 0]))
                        || (fragments.target.architecture == Architecture::Aarch64
                            && (call_byte_count != 4
                                || slice_bytes(&source_row.bytes, opcode_relative, 4)?
                                    != 0x9400_0000_u32.to_le_bytes()))
                    {
                        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
                    }
                    let FunctionFragmentControlProvenance::NormalizedForeignCall {
                        invocation_site,
                        boundary,
                        ordinal,
                    } = source_row.control
                    else {
                        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
                    };
                    if boundary != fixup.boundary || ordinal != fixup.ordinal {
                        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
                    }
                    validate_invocation_site(
                        invocation_site,
                        &source_row.provenance.operations,
                        &source_row.provenance.edges,
                    )?;
                    let candidate = foreign
                        .next()
                        .ok_or(RelocationFreeTextSectionPlacementError::ArtifactMismatch)?;
                    check_foreign_call(
                        ForeignCall {
                            caller: source.machine,
                            block: source_block.block,
                            instruction: source_row.instruction,
                            invocation_site,
                            offset: source_row.offset,
                            bytes: &source_row.bytes,
                            fixup: fixup.clone(),
                        },
                        fragments.target.architecture,
                        offset,
                        candidate_bytes,
                        candidate,
                    )?;
                }
                if let Some(fixup) = source_row.image_address_fixup() {
                    let candidate = addresses
                        .next()
                        .ok_or(RelocationFreeTextSectionPlacementError::ArtifactMismatch)?;
                    let expected = machine_code::PlacedImageAddress {
                        machine: source.machine,
                        block: source_block.block,
                        instruction: source_row.instruction,
                        reference: fixup.reference,
                        operation: *source_row
                            .provenance
                            .operations
                            .last()
                            .ok_or(RelocationFreeTextSectionPlacementError::ArtifactMismatch)?,
                        instruction_function_offset: source_row.offset,
                        instruction_section_offset: checked_add(offset, source_row.offset)?,
                        instruction_byte_count: source_row.bytes.len() as u64,
                        fields: fixup
                            .fields
                            .shifted(offset)
                            .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?,
                    };
                    if *candidate != expected {
                        return Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch);
                    }
                }
            }
        }
        unchanged_bytes(&source.bytes, candidate_bytes, patches)?;
    }

    if resolutions.next().is_some() || foreign.next().is_some() || addresses.next().is_some() {
        return Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch);
    }
    Ok(())
}

struct Call<'a> {
    caller: MachineId,
    block: SelectedBlockId,
    instruction: SelectedInstructionId,
    owner: target_vocabulary::CallSiteOwner,
    callee: MachineId,
    offset: u64,
    bytes: &'a [u8],
    fixup: FunctionFragmentInternalMachineFixup,
}

fn check_internal_call(
    call: Call<'_>,
    architecture: Architecture,
    section_offset: u64,
    offsets: &[(MachineId, u64)],
    candidate_bytes: &[u8],
    row: &PlacedInternalMachineCallResolution,
) -> Result<(u64, u64), RelocationFreeTextSectionPlacementError> {
    let fixup = call.fixup;
    let callee_offset = lookup_function_offset(offsets, call.callee).ok_or(
        RelocationFreeTextSectionPlacementError::MissingInternalMachineTarget(call.callee),
    )?;
    if fixup.state != FunctionFragmentInternalMachineFixupState::UnresolvedZeroFieldV1
        || fixup.addend != 0
        || fixup.callee != call.callee
        || fixup.patch_byte_width != 4
    {
        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
    }
    let opcode_relative = fixup
        .opcode_function_offset
        .checked_sub(call.offset)
        .ok_or(RelocationFreeTextSectionPlacementError::SourceShapeMismatch)?;
    let patch_relative = fixup
        .patch_function_offset
        .checked_sub(call.offset)
        .ok_or(RelocationFreeTextSectionPlacementError::SourceShapeMismatch)?;
    let field = slice_bytes(candidate_bytes, fixup.patch_function_offset, 4)?;
    let field: [u8; 4] = field
        .try_into()
        .map_err(|_| RelocationFreeTextSectionPlacementError::ArtifactMismatch)?;
    let (kind, displacement) = match (architecture, fixup.kind) {
        (
            Architecture::X86_64,
            FunctionFragmentInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1,
        ) => {
            if fixup.patch_function_offset != checked_add(fixup.opcode_function_offset, 1)?
                || fixup.reference_function_offset != checked_add(fixup.opcode_function_offset, 5)?
                || slice_bytes(call.bytes, opcode_relative, 1)? != [0xe8]
                || slice_bytes(call.bytes, patch_relative, 4)? != [0, 0, 0, 0]
            {
                return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
            }
            if slice_bytes(candidate_bytes, fixup.opcode_function_offset, 1)? != [0xe8] {
                return Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch);
            }
            (
                InternalMachineCallResolutionKind::X86Relative32FromNextInstructionToInternalMachineV1,
                i32::from_le_bytes(field),
            )
        }
        (
            Architecture::Aarch64,
            FunctionFragmentInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1,
        ) => {
            if fixup.opcode_function_offset != call.offset
                || fixup.patch_function_offset != call.offset
                || fixup.reference_function_offset != call.offset
                || call.bytes != 0x9400_0000_u32.to_le_bytes()
            {
                return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
            }
            let instruction = u32::from_le_bytes(field);
            if instruction & 0xfc00_0000 != 0x9400_0000 {
                return Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch);
            }
            let signed_words = ((instruction & 0x03ff_ffff) << 6) as i32 >> 6;
            (
                InternalMachineCallResolutionKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1,
                signed_words * 4,
            )
        }
        _ => return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch),
    };
    let reference = checked_add(section_offset, fixup.reference_function_offset)?;
    if i128::from(reference) + i128::from(displacement) != i128::from(callee_offset)
        || row.kind != kind
        || row.state != InternalMachineCallResolutionState::ResolvedInSectionV1
        || row.caller != call.caller
        || row.block != call.block
        || row.instruction != call.instruction
        || row.owner != call.owner
        || row.callee != call.callee
        || row.call_function_offset != call.offset
        || row.call_section_offset != checked_add(section_offset, call.offset)?
        || row.call_byte_count != call.bytes.len() as u64
        || row.opcode_function_offset != fixup.opcode_function_offset
        || row.opcode_section_offset != checked_add(section_offset, fixup.opcode_function_offset)?
        || row.field_function_offset != fixup.patch_function_offset
        || row.field_section_offset != checked_add(section_offset, fixup.patch_function_offset)?
        || row.next_instruction_function_offset != fixup.reference_function_offset
        || row.next_instruction_section_offset != reference
        || row.callee_section_offset != callee_offset
        || row.field_byte_width != 4
        || row.addend != 0
        || row.displacement != displacement
    {
        return Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch);
    }
    Ok((
        fixup.patch_function_offset,
        checked_add(fixup.patch_function_offset, 4)?,
    ))
}

struct ForeignCall<'a> {
    caller: MachineId,
    block: SelectedBlockId,
    instruction: SelectedInstructionId,
    invocation_site: semantic_vocabulary::InvocationSite,
    offset: u64,
    bytes: &'a [u8],
    fixup: FunctionFragmentNormalizedForeignCallFixup,
}

fn callback_matches_fragment(
    source: &machine_code::FunctionFragmentCallbackMaterialization,
    placed: &machine_code::PlacedCallbackAddressMaterialization,
    section_start: u64,
) -> bool {
    use machine_code::{
        FunctionFragmentCallbackMaterializationEncoding as Fragment,
        PlacedCallbackAddressEncoding as Placed,
    };
    let matches_offset = |expected: u64, function: u64, section: u64| {
        function == expected && section.checked_sub(section_start) == Some(expected)
    };
    if placed.destination != source.destination
        || placed.code_byte_count != source.code_byte_count
        || !matches_offset(
            source.code_function_offset,
            placed.code_function_offset,
            placed.code_section_offset,
        )
    {
        return false;
    }
    match (source.encoding, placed.encoding) {
        (
            Fragment::X86_64Relative32 {
                patch_function_offset,
            },
            Placed::X86_64Relative32 {
                relocation_function_offset,
                relocation_section_offset,
            },
        ) => matches_offset(
            patch_function_offset,
            relocation_function_offset,
            relocation_section_offset,
        ),
        (
            Fragment::Aarch64PageAddress {
                page_patch_function_offset,
                page_offset_patch_function_offset,
            },
            Placed::Aarch64PageAddress {
                page_relocation_function_offset,
                page_relocation_section_offset,
                page_offset_relocation_function_offset,
                page_offset_relocation_section_offset,
            },
        ) => {
            matches_offset(
                page_patch_function_offset,
                page_relocation_function_offset,
                page_relocation_section_offset,
            ) && matches_offset(
                page_offset_patch_function_offset,
                page_offset_relocation_function_offset,
                page_offset_relocation_section_offset,
            )
        }
        _ => false,
    }
}

fn check_foreign_call(
    call: ForeignCall<'_>,
    architecture: Architecture,
    section_offset: u64,
    candidate_bytes: &[u8],
    row: &PlacedNormalizedForeignCallResolution,
) -> Result<(), RelocationFreeTextSectionPlacementError> {
    let fixup = call.fixup;
    if (call.invocation_site.operation().is_none() && !fixup.callbacks.is_empty())
        || fixup.state != FunctionFragmentNormalizedForeignCallFixupState::UnresolvedImportFieldV1
        || fixup.addend != 0
        || fixup.patch_byte_width != 4
    {
        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
    }
    let opcode_relative = fixup
        .opcode_function_offset
        .checked_sub(call.offset)
        .ok_or(RelocationFreeTextSectionPlacementError::SourceShapeMismatch)?;
    let patch_relative = fixup
        .patch_function_offset
        .checked_sub(call.offset)
        .ok_or(RelocationFreeTextSectionPlacementError::SourceShapeMismatch)?;
    let field = slice_bytes(candidate_bytes, fixup.patch_function_offset, 4)?;
    let kind = match (architecture, fixup.kind) {
        (
            Architecture::X86_64,
            FunctionFragmentNormalizedForeignCallFixupKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1,
        ) => {
            if fixup.patch_function_offset != checked_add(fixup.opcode_function_offset, 1)?
                || fixup.reference_function_offset != checked_add(fixup.opcode_function_offset, 5)?
                || opcode_relative.checked_add(5) != Some(call.bytes.len() as u64)
                || slice_bytes(call.bytes, opcode_relative, 1)? != [0xe8]
                || slice_bytes(call.bytes, patch_relative, 4)? != [0, 0, 0, 0]
            {
                return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
            }
            if slice_bytes(candidate_bytes, fixup.opcode_function_offset, 1)? != [0xe8]
                || field != [0, 0, 0, 0]
            {
                return Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch);
            }
            NormalizedForeignCallResolutionKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1
        }
        (
            Architecture::Aarch64,
            FunctionFragmentNormalizedForeignCallFixupKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1,
        ) => {
            if fixup.patch_function_offset != fixup.opcode_function_offset
                || fixup.reference_function_offset != fixup.opcode_function_offset
                || opcode_relative.checked_add(4) != Some(call.bytes.len() as u64)
                || slice_bytes(call.bytes, opcode_relative, 4)? != 0x9400_0000_u32.to_le_bytes()
            {
                return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
            }
            if field != 0x9400_0000_u32.to_le_bytes() {
                return Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch);
            }
            NormalizedForeignCallResolutionKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1
        }
        _ => return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch),
    };
    let reference = checked_add(section_offset, fixup.reference_function_offset)?;
    if row.kind != kind
        || row.state != NormalizedForeignCallResolutionState::UnresolvedImportFieldV1
        || row.caller != call.caller
        || row.block != call.block
        || row.instruction != call.instruction
        || row.invocation_site != call.invocation_site
        || row.boundary != fixup.boundary
        || row.ordinal != fixup.ordinal
        || row.call_function_offset != call.offset
        || row.call_section_offset != checked_add(section_offset, call.offset)?
        || row.call_byte_count != call.bytes.len() as u64
        || row.opcode_function_offset != fixup.opcode_function_offset
        || row.opcode_section_offset != checked_add(section_offset, fixup.opcode_function_offset)?
        || row.field_function_offset != fixup.patch_function_offset
        || row.field_section_offset != checked_add(section_offset, fixup.patch_function_offset)?
        || row.next_instruction_function_offset != fixup.reference_function_offset
        || row.next_instruction_section_offset != reference
        || row.field_byte_width != 4
        || row.addend != 0
        || row.callback_materializations.len() != fixup.callbacks.len()
        || !fixup
            .callbacks
            .iter()
            .zip(&row.callback_materializations)
            .all(|(source, placed)| callback_matches_fragment(source, placed, section_offset))
    {
        return Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch);
    }
    Ok(())
}

fn unchanged_bytes(
    source: &[u8],
    candidate: &[u8],
    mut patches: Vec<(u64, u64)>,
) -> Result<(), RelocationFreeTextSectionPlacementError> {
    if source.len() != candidate.len() {
        return Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch);
    }
    patches.sort_unstable();
    let mut cursor = 0;
    for (start, end) in patches {
        if start < cursor || end < start || end > source.len() as u64 {
            return Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch);
        }
        if slice_bytes(source, cursor, start - cursor)?
            != slice_bytes(candidate, cursor, start - cursor)?
        {
            return Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch);
        }
        cursor = end;
    }
    if slice_bytes(source, cursor, source.len() as u64 - cursor)?
        != slice_bytes(candidate, cursor, candidate.len() as u64 - cursor)?
    {
        return Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch);
    }
    Ok(())
}

fn slice_bytes(
    source: &[u8],
    start: u64,
    count: u64,
) -> Result<&[u8], RelocationFreeTextSectionPlacementError> {
    let end = checked_add(start, count)?;
    let start = usize::try_from(start)
        .map_err(|_| RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
    let end = usize::try_from(end)
        .map_err(|_| RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
    source
        .get(start..end)
        .ok_or(RelocationFreeTextSectionPlacementError::ArtifactMismatch)
}

fn checked_add(left: u64, right: u64) -> Result<u64, RelocationFreeTextSectionPlacementError> {
    left.checked_add(right)
        .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)
}

fn check_function_span(
    candidate: &PlacedFunctionFragment,
    index: usize,
    machine: MachineId,
    offset: u64,
    count: u64,
) -> Result<(), RelocationFreeTextSectionPlacementError> {
    if candidate.source_function_index != index as u64
        || candidate.machine != machine
        || candidate.section_offset != offset
        || candidate.byte_count != count
    {
        return Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn check_block_span(
    candidate: &PlacedBlockSpan,
    block: SelectedBlockId,
    offset: u64,
    section: u64,
    count: u64,
    extent: u64,
    alignment: u64,
) -> Result<(), RelocationFreeTextSectionPlacementError> {
    if !offset.is_multiple_of(alignment) || !count.is_multiple_of(alignment) {
        return Err(RelocationFreeTextSectionPlacementError::MisalignedAarch64Span);
    }
    if checked_add(offset, count)? > extent {
        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
    }
    if candidate.block != block
        || candidate.function_offset != offset
        || candidate.section_offset != checked_add(section, offset)?
        || candidate.byte_count != count
    {
        return Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn check_instruction_span(
    candidate: &PlacedInstructionSpan,
    instruction: SelectedInstructionId,
    alternative: MachineAlternativeKey,
    offset: u64,
    section: u64,
    count: u64,
    extent: u64,
    alignment: u64,
) -> Result<(), RelocationFreeTextSectionPlacementError> {
    if !offset.is_multiple_of(alignment) || !count.is_multiple_of(alignment) {
        return Err(RelocationFreeTextSectionPlacementError::MisalignedAarch64Span);
    }
    if checked_add(offset, count)? > extent {
        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
    }
    if candidate.instruction != instruction
        || candidate.alternative != alternative
        || candidate.function_offset != offset
        || candidate.section_offset != checked_add(section, offset)?
        || candidate.byte_count != count
    {
        return Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foreign_call_replay_keeps_the_callback_placement() {
        let machine = MachineId::new(1).unwrap();
        let block = SelectedBlockId(0);
        let instruction = SelectedInstructionId(0);
        let site = semantic_vocabulary::InvocationSite::Operation(OperationId::new(1).unwrap());
        let bytes = [0x48, 0x8d, 0x0d, 0, 0, 0, 0, 0xe8, 0, 0, 0, 0];
        let section_offset = 64;
        let fixup = FunctionFragmentNormalizedForeignCallFixup {
            kind: FunctionFragmentNormalizedForeignCallFixupKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1,
            state: FunctionFragmentNormalizedForeignCallFixupState::UnresolvedImportFieldV1,
            boundary: semantic_vocabulary::BoundaryMachineId::new(1).unwrap(),
            ordinal: 0,
            opcode_function_offset: 7,
            patch_function_offset: 8,
            reference_function_offset: 12,
            patch_byte_width: 4,
            addend: 0,
            callbacks: vec![machine_code::FunctionFragmentCallbackMaterialization {
                destination: machine_code::CallbackAddressDestination::Register(target_vocabulary::MachineRegister::X86Rcx),
                code_function_offset: 0,
                code_byte_count: 7,
                encoding: machine_code::FunctionFragmentCallbackMaterializationEncoding::X86_64Relative32 { patch_function_offset: 3 },
            }],
        };
        let original = resolve_foreign(
            Architecture::X86_64,
            machine,
            block,
            instruction,
            site,
            0,
            &bytes,
            section_offset,
            fixup.clone(),
            &bytes,
        )
        .unwrap();
        let check = |row: &PlacedNormalizedForeignCallResolution| {
            check_foreign_call(
                ForeignCall {
                    caller: machine,
                    block,
                    instruction,
                    invocation_site: site,
                    offset: 0,
                    bytes: &bytes,
                    fixup: fixup.clone(),
                },
                Architecture::X86_64,
                section_offset,
                &bytes,
                row,
            )
        };
        check(&original).unwrap();
        let mut shifted = original.clone();
        shifted.callback_materializations[0].code_section_offset += 1;
        assert_eq!(
            check(&shifted),
            Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch)
        );
        let mut missing = original;
        missing.callback_materializations.clear();
        assert_eq!(
            check(&missing),
            Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch)
        );
    }
}
