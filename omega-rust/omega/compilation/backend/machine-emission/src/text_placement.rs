/*
We place frame-applied function fragments into a single dense text section,
resolve every ordinary internal machine call in section coordinates, and seal
the v10 text-section manifest and custody receipt.

`stage_optimized_fixed_frame_text_section` validates per-architecture span
alignment, concatenates framed function fragments in validated order without
padding, patches internal machine call displacements (`resolve`), records
section-relative foreign-call and image-address relocation fields
(`resolve_foreign`, `place_image_address`), and seals
`ValidatedFunctionFragmentTextSectionManifest` alongside
`StagedFixedFrameTextSectionCustodyReceipt`.
`validate_optimized_fixed_frame_text_section` replays frame-application custody,
independently verifies every placed span, patched call field, and unchanged byte
range (`validate_fragment_text_section`), and checks the manifest statistics and
custody receipt.
*/

use crate::fragment_emission::FunctionFragmentEmissionError;
use crate::frame_application::{
    FunctionFragmentFrameApplicationError, StagedFunctionFragmentFrameApplication,
    validate_function_fragment_frame_application,
};
pub use machine_code::FunctionFragmentTextSectionManifestDecodeError;
use machine_code::{
    FunctionFragment, FunctionFragmentControlProvenance, FunctionFragmentEmissionManifest,
    FunctionFragmentEmissionPlan, FunctionFragmentFrameApplicationIdentity,
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
pub use machine_code::{
    FunctionFragmentTextSectionManifest, FunctionFragmentTextSectionStage,
    FunctionFragmentTextSectionStatistics, FunctionFragmentTextSectionUnavailableData,
};
use optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionFragmentTextSectionManifestIdentity,
    TerminalRelocationFreeTextSectionIdentity,
};
use selected_instructions::{MachineAlternativeKey, SelectedBlockId, SelectedInstructionId};
use semantic_vocabulary::{MachineId, OperationId};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use target::Architecture;

pub fn stage_optimized_fixed_frame_text_section(
    source: StagedFunctionFragmentFrameApplication,
) -> Result<StagedOptimizedFixedFrameTextSection, RelocationFreeTextSectionPlacementError> {
    let (text_section, manifest) = compute_fixed_frame(&source)?;
    let custody = fixed_frame_receipt(&source, &manifest, &text_section);
    Ok(StagedOptimizedFixedFrameTextSection {
        source,
        text_section: Arc::new(text_section),
        manifest,
        custody,
    })
}

pub fn validate_optimized_fixed_frame_text_section(
    staged: &StagedOptimizedFixedFrameTextSection,
) -> Result<StagedFixedFrameTextSectionCustodyReceipt, RelocationFreeTextSectionPlacementError> {
    validate_function_fragment_frame_application(&staged.source)
        .map_err(RelocationFreeTextSectionPlacementError::FrameSource)?;
    validate_fragment_text_section(
        TextPlacementInput::InternalCalls(staged.source.fragments()),
        &staged.text_section,
    )?;
    check_manifest_fields(
        staged.source.source().manifest().record(),
        FunctionFragmentTextSectionStage::ValidatedFixedFrameInternalCallTextSectionPlacementV1,
        staged.source.receipt().identity(),
        &staged.text_section,
        staged.source.fragments(),
        staged.manifest.record(),
    )?;
    let expected_receipt =
        fixed_frame_receipt(&staged.source, &staged.manifest, &staged.text_section);
    if staged.custody != expected_receipt {
        return Err(RelocationFreeTextSectionPlacementError::ReceiptMismatch);
    }
    Ok(expected_receipt)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFunctionFragmentTextSectionManifest {
    record: Arc<FunctionFragmentTextSectionManifest>, // Sealed v10 text-section manifest record.
}

impl ValidatedFunctionFragmentTextSectionManifest {
    pub fn record(&self) -> &FunctionFragmentTextSectionManifest {
        &self.record
    }
}

#[derive(Debug)]
#[must_use = "a staged fixed-frame text section owns its exact frame-application custody"]
pub struct StagedOptimizedFixedFrameTextSection {
    source: StagedFunctionFragmentFrameApplication, // Admitted frame-applied function-fragment stage.
    text_section: Arc<RelocationFreeTextSectionPlacement>, // Placed dense text section with resolved internal calls.
    manifest: ValidatedFunctionFragmentTextSectionManifest, // Sealed text-section manifest.
    custody: StagedFixedFrameTextSectionCustodyReceipt, // Custody receipt binding frame application, section, and manifest.
}

impl StagedOptimizedFixedFrameTextSection {
    pub const fn source(&self) -> &StagedFunctionFragmentFrameApplication {
        &self.source
    }

    pub fn text_section(&self) -> &RelocationFreeTextSectionPlacement {
        self.text_section.as_ref()
    }

    pub fn shared_text_section(&self) -> Arc<RelocationFreeTextSectionPlacement> {
        Arc::clone(&self.text_section)
    }

    pub const fn manifest(&self) -> &ValidatedFunctionFragmentTextSectionManifest {
        &self.manifest
    }

    pub const fn custody(&self) -> StagedFixedFrameTextSectionCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedFixedFrameTextSectionCustodyReceipt {
    frame_application: FunctionFragmentFrameApplicationIdentity, // Upstream frame-application receipt identity.
    fragments: FunctionFragmentEmissionIdentity, // Source framed fragment-plan identity.
    text_section: TerminalRelocationFreeTextSectionIdentity, // Placed text-section identity.
    manifest: FunctionFragmentTextSectionManifestIdentity, // Sealed text-section manifest identity.
}

impl StagedFixedFrameTextSectionCustodyReceipt {
    pub const fn frame_application(self) -> FunctionFragmentFrameApplicationIdentity {
        self.frame_application
    }

    pub const fn fragments(self) -> FunctionFragmentEmissionIdentity {
        self.fragments
    }

    pub const fn text_section(self) -> TerminalRelocationFreeTextSectionIdentity {
        self.text_section
    }

    pub const fn manifest(self) -> FunctionFragmentTextSectionManifestIdentity {
        self.manifest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelocationFreeTextSectionPlacementError {
    Source(FunctionFragmentEmissionError),
    FrameSource(FunctionFragmentFrameApplicationError),
    DuplicateFunction(MachineId),
    MissingSemanticEntry(MachineId),
    OffsetOverflow,
    StatisticsOverflow,
    SourceShapeMismatch,
    MisalignedAarch64Span,
    UnsupportedRelocationShape,
    UnresolvedInternalMachineFixups,
    MissingInternalMachineTarget(MachineId),
    InternalCallOutOfRange,
    ArtifactMismatch,
    ManifestMismatch,
    ReceiptMismatch,
}

diagnostics::failed_display!(
    RelocationFreeTextSectionPlacementError,
    "optimized relocation-free text-section placement failed"
);

impl std::error::Error for RelocationFreeTextSectionPlacementError {}

impl From<TextPlacementError> for RelocationFreeTextSectionPlacementError {
    fn from(error: TextPlacementError) -> Self {
        use TextPlacementError as Source;
        match error {
            Source::OffsetOverflow => Self::OffsetOverflow,
            Source::StatisticsOverflow => Self::StatisticsOverflow,
            Source::SourceShapeMismatch => Self::SourceShapeMismatch,
            Source::MisalignedAarch64Span => Self::MisalignedAarch64Span,
            Source::UnsupportedRelocationShape => Self::UnsupportedRelocationShape,
            Source::UnresolvedInternalMachineFixups => Self::UnresolvedInternalMachineFixups,
            Source::InternalCallOutOfRange => Self::InternalCallOutOfRange,
            Source::ArtifactMismatch => Self::ArtifactMismatch,
            Source::DuplicateFunction(machine) => Self::DuplicateFunction(machine),
            Source::MissingSemanticEntry(machine) => Self::MissingSemanticEntry(machine),
            Source::MissingInternalMachineTarget(machine) => {
                Self::MissingInternalMachineTarget(machine)
            }
        }
    }
}

fn compute_fixed_frame(
    source: &StagedFunctionFragmentFrameApplication,
) -> Result<
    (
        RelocationFreeTextSectionPlacement,
        ValidatedFunctionFragmentTextSectionManifest,
    ),
    RelocationFreeTextSectionPlacementError,
> {
    let fragments = source.fragments();
    let source_manifest = source.source().manifest().record();
    let text_section =
        place_fragment_text_section(TextPlacementInput::InternalCalls(source.fragments()))?;
    let manifest = seal_manifest(
        source_manifest,
        FunctionFragmentTextSectionStage::ValidatedFixedFrameInternalCallTextSectionPlacementV1,
        source.receipt().identity(),
        &text_section,
        fragments,
    )?;
    Ok((text_section, manifest))
}

fn seal_manifest(
    source_manifest: &FunctionFragmentEmissionManifest,
    stage: FunctionFragmentTextSectionStage,
    frame_application: FunctionFragmentFrameApplicationIdentity,
    text_section: &RelocationFreeTextSectionPlacement,
    fragments: &FunctionFragmentEmissionPlan,
) -> Result<ValidatedFunctionFragmentTextSectionManifest, RelocationFreeTextSectionPlacementError> {
    let statistics = text_section_statistics(text_section, fragments)?;
    let unavailable = FunctionFragmentTextSectionUnavailableData::Unavailable;
    let mut record = FunctionFragmentTextSectionManifest {
        identity: FunctionFragmentTextSectionManifestIdentity::from_canonical_bytes(b"pending"),
        stage,
        frame_application,
        source_fragment_manifest: source_manifest.identity,
        source_realization: source_manifest.source_realization,
        selections: source_manifest.selections,
        psi: source_manifest.psi,
        work_schedule: source_manifest.work_schedule,
        selected: source_manifest.selected,
        post_allocation_manifest: source_manifest.post_allocation_manifest,
        post_allocation_machine: source_manifest.post_allocation_machine,
        final_pre_layout: source_manifest.final_pre_layout,
        final_resolved_layout: source_manifest.final_resolved_layout,
        whole_function_exit_contract: source_manifest.whole_function_exit_contract,
        fragments: fragments.identity,
        target: source_manifest.target,
        semantic_entry: text_section.semantic_entry,
        semantic_entry_offset: text_section.semantic_entry_offset,
        access_policy: text_section.policy,
        text_section: text_section.identity,
        relocation_requirements: text_section.relocation_requirements,
        statistics,
        symbols: unavailable,
        object_container: unavailable,
        external_entry_bridge: unavailable,
        executable_image: unavailable,
        installation: unavailable,
        publication: unavailable,
    };
    record.identity = record.recomputed_identity();
    Ok(ValidatedFunctionFragmentTextSectionManifest {
        record: Arc::new(record),
    })
}

fn fixed_frame_receipt(
    source: &StagedFunctionFragmentFrameApplication,
    manifest: &ValidatedFunctionFragmentTextSectionManifest,
    section: &RelocationFreeTextSectionPlacement,
) -> StagedFixedFrameTextSectionCustodyReceipt {
    StagedFixedFrameTextSectionCustodyReceipt {
        frame_application: source.receipt().identity(),
        fragments: section.source_fragments,
        text_section: section.identity,
        manifest: manifest.record.identity,
    }
}

fn check_manifest_fields(
    source: &FunctionFragmentEmissionManifest,
    stage: FunctionFragmentTextSectionStage,
    frame_application: FunctionFragmentFrameApplicationIdentity,
    section: &RelocationFreeTextSectionPlacement,
    fragments: &FunctionFragmentEmissionPlan,
    candidate: &FunctionFragmentTextSectionManifest,
) -> Result<(), RelocationFreeTextSectionPlacementError> {
    if candidate.identity != candidate.recomputed_identity()
        || candidate.stage != stage
        || candidate.frame_application != frame_application
        || candidate.source_fragment_manifest != source.identity
        || candidate.source_realization != source.source_realization
        || candidate.selections != source.selections
        || candidate.psi != source.psi
        || candidate.work_schedule != source.work_schedule
        || candidate.selected != source.selected
        || candidate.post_allocation_manifest != source.post_allocation_manifest
        || candidate.post_allocation_machine != source.post_allocation_machine
        || candidate.final_pre_layout != source.final_pre_layout
        || candidate.final_resolved_layout != source.final_resolved_layout
        || candidate.whole_function_exit_contract != source.whole_function_exit_contract
        || candidate.target != source.target
        || candidate.semantic_entry != section.semantic_entry
        || candidate.semantic_entry_offset != section.semantic_entry_offset
        || candidate.relocation_requirements != section.relocation_requirements
        || candidate.fragments != fragments.identity
        || candidate.access_policy != section.policy
        || candidate.text_section != section.identity
        || candidate.statistics != text_section_statistics(section, fragments)?
        || candidate.symbols != FunctionFragmentTextSectionUnavailableData::Unavailable
        || candidate.object_container != FunctionFragmentTextSectionUnavailableData::Unavailable
        || candidate.external_entry_bridge
            != FunctionFragmentTextSectionUnavailableData::Unavailable
        || candidate.executable_image != FunctionFragmentTextSectionUnavailableData::Unavailable
        || candidate.installation != FunctionFragmentTextSectionUnavailableData::Unavailable
        || candidate.publication != FunctionFragmentTextSectionUnavailableData::Unavailable
    {
        return Err(RelocationFreeTextSectionPlacementError::ManifestMismatch);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TextPlacementError {
    DuplicateFunction(MachineId),
    MissingSemanticEntry(MachineId),
    OffsetOverflow,
    StatisticsOverflow,
    SourceShapeMismatch,
    MisalignedAarch64Span,
    UnsupportedRelocationShape,
    UnresolvedInternalMachineFixups,
    MissingInternalMachineTarget(MachineId),
    InternalCallOutOfRange,
    ArtifactMismatch,
}

diagnostics::failed_display!(
    TextPlacementError,
    "optimized relocation-free text-section placement failed"
);

impl std::error::Error for TextPlacementError {}

#[derive(Clone, Copy)]
pub(crate) enum TextPlacementInput<'a> {
    InternalCalls(&'a FunctionFragmentEmissionPlan),
}

impl<'a> TextPlacementInput<'a> {
    fn fragments(self) -> &'a FunctionFragmentEmissionPlan {
        match self {
            Self::InternalCalls(fragments) => fragments,
        }
    }
}

pub(crate) fn place_fragment_text_section(
    input: TextPlacementInput<'_>,
) -> Result<RelocationFreeTextSectionPlacement, TextPlacementError> {
    validate_placement_source(input)?;
    match input {
        TextPlacementInput::InternalCalls(fragments) => place_fixed_frame(fragments),
    }
}

pub(crate) fn validate_fragment_text_section(
    input: TextPlacementInput<'_>,
    candidate: &RelocationFreeTextSectionPlacement,
) -> Result<(), TextPlacementError> {
    validate_placement_source(input)?;
    check_placed_section(input, candidate)
}

fn validate_placement_source(input: TextPlacementInput<'_>) -> Result<(), TextPlacementError> {
    match input {
        TextPlacementInput::InternalCalls(_) => Ok(()),
    }
}

fn validate_invocation_site(
    site: semantic_vocabulary::InvocationSite,
    operations: &[OperationId],
    edges: &[semantic_vocabulary::EdgeId],
) -> Result<(), TextPlacementError> {
    let matches = match site {
        semantic_vocabulary::InvocationSite::Operation(operation) => {
            operations.last() == Some(&operation) && edges.is_empty()
        }
        semantic_vocabulary::InvocationSite::ExternalCompletion(edge)
        | semantic_vocabulary::InvocationSite::InvokeNoNormal(edge) => edges == [edge],
    };
    if !matches {
        return Err(TextPlacementError::SourceShapeMismatch);
    }
    Ok(())
}

fn usize_to_u64(value: usize) -> Result<u64, TextPlacementError> {
    u64::try_from(value).map_err(|_| TextPlacementError::OffsetOverflow)
}

pub(crate) fn text_section_statistics(
    section: &RelocationFreeTextSectionPlacement,
    fragments: &FunctionFragmentEmissionPlan,
) -> Result<FunctionFragmentTextSectionStatistics, TextPlacementError> {
    let mut result = FunctionFragmentTextSectionStatistics {
        functions: usize_to_u64(section.functions.len())?,
        bytes: section.byte_count,
        ..FunctionFragmentTextSectionStatistics::default()
    };
    for function in &section.functions {
        result.blocks = result
            .blocks
            .checked_add(usize_to_u64(function.blocks.len())?)
            .ok_or(TextPlacementError::StatisticsOverflow)?;
        for block in &function.blocks {
            result.instruction_spans = result
                .instruction_spans
                .checked_add(usize_to_u64(block.instructions.len())?)
                .ok_or(TextPlacementError::StatisticsOverflow)?;
            for row in &block.instructions {
                result.zero_byte_instruction_spans = result
                    .zero_byte_instruction_spans
                    .checked_add(u64::from(row.byte_count == 0))
                    .ok_or(TextPlacementError::StatisticsOverflow)?;
            }
        }
    }
    result.source_internal_machine_fixups = fragments
        .instructions()
        .filter(|instruction| instruction.internal_machine_fixup.is_some())
        .count()
        .try_into()
        .map_err(|_| TextPlacementError::StatisticsOverflow)?;
    result.resolved_internal_machine_fixups =
        usize_to_u64(section.resolved_internal_machine_calls.len())?;
    result.relocation_requirements = usize_to_u64(
        section
            .unresolved_normalized_foreign_calls
            .len()
            .checked_add(section.unresolved_image_addresses.len())
            .ok_or(TextPlacementError::StatisticsOverflow)?,
    )?;
    result.remaining_internal_machine_fixups = result
        .source_internal_machine_fixups
        .checked_sub(result.resolved_internal_machine_fixups)
        .ok_or(TextPlacementError::UnresolvedInternalMachineFixups)?;
    if result.remaining_internal_machine_fixups != 0 {
        return Err(TextPlacementError::UnresolvedInternalMachineFixups);
    }
    Ok(result)
}

fn validate_alignment(
    architecture: Architecture,
    offset: u64,
    byte_count: u64,
) -> Result<(), TextPlacementError> {
    if architecture == Architecture::Aarch64
        && (!offset.is_multiple_of(4) || !byte_count.is_multiple_of(4))
    {
        return Err(TextPlacementError::MisalignedAarch64Span);
    }
    Ok(())
}

fn place_block_spans(
    architecture: Architecture,
    function: &FunctionFragment,
    function_section_offset: u64,
) -> Result<Vec<PlacedBlockSpan>, TextPlacementError> {
    let mut blocks = Vec::with_capacity(function.blocks.len());
    for block in &function.blocks {
        validate_alignment(architecture, block.offset, block.byte_count)?;
        let section_offset = function_section_offset
            .checked_add(block.offset)
            .ok_or(TextPlacementError::OffsetOverflow)?;
        if block
            .offset
            .checked_add(block.byte_count)
            .is_none_or(|end| end > function.byte_count)
        {
            return Err(TextPlacementError::SourceShapeMismatch);
        }
        let mut instructions = Vec::with_capacity(block.instructions.len());
        for row in &block.instructions {
            let byte_count = usize_to_u64(row.bytes.len())?;
            validate_alignment(architecture, row.offset, byte_count)?;
            let row_section_offset = function_section_offset
                .checked_add(row.offset)
                .ok_or(TextPlacementError::OffsetOverflow)?;
            if row
                .offset
                .checked_add(byte_count)
                .is_none_or(|end| end > function.byte_count)
            {
                return Err(TextPlacementError::SourceShapeMismatch);
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
) -> Result<RelocationFreeTextSectionPlacement, TextPlacementError> {
    let section_alignment = match fragments.target.architecture {
        Architecture::Aarch64 => 4,
        Architecture::X86_64 => 1,
    };
    let mut offsets = BTreeMap::new();
    let mut next_offset = 0_u64;
    for function in &fragments.functions {
        if offsets.insert(function.machine, next_offset).is_some() {
            return Err(TextPlacementError::DuplicateFunction(function.machine));
        }
        validate_alignment(
            fragments.target.architecture,
            next_offset,
            function.byte_count,
        )?;
        if usize_to_u64(function.bytes.len())? != function.byte_count {
            return Err(TextPlacementError::SourceShapeMismatch);
        }
        next_offset = next_offset
            .checked_add(function.byte_count)
            .ok_or(TextPlacementError::OffsetOverflow)?;
    }

    let semantic_entry_offset = *offsets
        .get(&fragments.entry)
        .ok_or(TextPlacementError::MissingSemanticEntry(fragments.entry))?;
    let mut bytes = Vec::with_capacity(
        usize::try_from(next_offset).map_err(|_| TextPlacementError::OffsetOverflow)?,
    );
    let mut functions = Vec::with_capacity(fragments.functions.len());
    let mut resolved = Vec::new();
    let mut unresolved_foreign = Vec::new();
    let mut unresolved_addresses = Vec::new();
    let mut seen_entry = BTreeSet::new();

    for (source_function_index, function) in fragments.functions.iter().enumerate() {
        if !seen_entry.insert(function.machine) {
            return Err(TextPlacementError::DuplicateFunction(function.machine));
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
                        return Err(TextPlacementError::SourceShapeMismatch);
                    };
                    if callee != fixup.callee {
                        return Err(TextPlacementError::SourceShapeMismatch);
                    }
                    match owner {
                        target_operations::CallSiteOwner::Operation(operation)
                            if exact_operation(&instruction.provenance.operations)?
                                == operation => {}
                        target_operations::CallSiteOwner::InvokeNoNormal(edge) => {
                            validate_invocation_site(
                                semantic_vocabulary::InvocationSite::InvokeNoNormal(edge),
                                &instruction.provenance.operations,
                                &instruction.provenance.edges,
                            )?;
                        }
                        target_operations::CallSiteOwner::ExternalCompletion(edge) => {
                            validate_invocation_site(
                                semantic_vocabulary::InvocationSite::ExternalCompletion(edge),
                                &instruction.provenance.operations,
                                &instruction.provenance.edges,
                            )?;
                        }
                        target_operations::CallSiteOwner::Installation { operation, .. }
                            if instruction.provenance.operations.as_slice() == [operation]
                                && instruction.provenance.edges.is_empty() => {}
                        target_operations::CallSiteOwner::CleanupAction { edge, .. }
                            if instruction.provenance.operations.is_empty()
                                && instruction.provenance.edges.as_slice() == [edge]
                                && instruction.provenance.work.is_empty() => {}
                        _ => return Err(TextPlacementError::SourceShapeMismatch),
                    }
                    let callee_section_offset = *offsets
                        .get(&callee)
                        .ok_or(TextPlacementError::MissingInternalMachineTarget(callee))?;
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
                        return Err(TextPlacementError::SourceShapeMismatch);
                    };
                    if boundary != fixup.boundary || ordinal != fixup.ordinal {
                        return Err(TextPlacementError::SourceShapeMismatch);
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
        return Err(TextPlacementError::UnresolvedInternalMachineFixups);
    }

    let mut section = RelocationFreeTextSectionPlacement {
        identity: TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(b"pending"),
        source_fragments: fragments.identity,
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
    };
    section.identity = section.recomputed_identity();
    Ok(section)
}

fn check_address_fields(
    instruction: &machine_code::FunctionFragmentInstructionSpan,
    fields: machine_code::ImageAddressFields,
    function_bytes: &[u8],
) -> Result<(), TextPlacementError> {
    let instruction_end = instruction
        .offset
        .checked_add(usize_to_u64(instruction.bytes.len())?)
        .ok_or(TextPlacementError::OffsetOverflow)?;
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
        return Err(TextPlacementError::SourceShapeMismatch);
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
) -> Result<machine_code::PlacedImageAddress, TextPlacementError> {
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
            .ok_or(TextPlacementError::OffsetOverflow)?,
        instruction_byte_count: usize_to_u64(instruction.bytes.len())?,
        fields: fixup
            .fields
            .shifted(section_offset)
            .ok_or(TextPlacementError::OffsetOverflow)?,
    })
}

fn exact_operation(operations: &[OperationId]) -> Result<OperationId, TextPlacementError> {
    operations
        .last()
        .copied()
        .ok_or(TextPlacementError::SourceShapeMismatch)
}

fn section_coordinate(base: u64, offset: u64) -> Result<u64, TextPlacementError> {
    base.checked_add(offset)
        .ok_or(TextPlacementError::OffsetOverflow)
}

#[allow(clippy::too_many_arguments)]
fn resolve(
    architecture: Architecture,
    caller: MachineId,
    block: SelectedBlockId,
    instruction: SelectedInstructionId,
    owner: target_operations::CallSiteOwner,
    instruction_offset: u64,
    instruction_bytes: &[u8],
    function_section_offset: u64,
    callee_section_offset: u64,
    fixup: FunctionFragmentInternalMachineFixup,
    function_bytes: &mut [u8],
) -> Result<PlacedInternalMachineCallResolution, TextPlacementError> {
    if fixup.state != FunctionFragmentInternalMachineFixupState::UnresolvedZeroFieldV1
        || fixup.opcode_function_offset != instruction_offset
        || fixup.addend != 0
    {
        return Err(TextPlacementError::SourceShapeMismatch);
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
                return Err(TextPlacementError::SourceShapeMismatch);
            }
            let reference = function_section_offset
                .checked_add(fixup.reference_function_offset)
                .ok_or(TextPlacementError::OffsetOverflow)?;
            let displacement =
                i32::try_from(i128::from(callee_section_offset) - i128::from(reference))
                    .map_err(|_| TextPlacementError::InternalCallOutOfRange)?;
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
                return Err(TextPlacementError::SourceShapeMismatch);
            }
            let byte_displacement =
                i128::from(callee_section_offset) - i128::from(call_section_offset);
            if byte_displacement % 4 != 0 {
                return Err(TextPlacementError::InternalCallOutOfRange);
            }
            let word_displacement = byte_displacement / 4;
            if !(-(1_i128 << 25)..(1_i128 << 25)).contains(&word_displacement) {
                return Err(TextPlacementError::InternalCallOutOfRange);
            }
            let immediate = u32::try_from(word_displacement.rem_euclid(1_i128 << 26))
                .map_err(|_| TextPlacementError::InternalCallOutOfRange)?;
            let encoded = 0x9400_0000_u32 | immediate;
            (
                InternalMachineCallResolutionKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1,
                i32::try_from(byte_displacement)
                    .map_err(|_| TextPlacementError::InternalCallOutOfRange)?,
                encoded.to_le_bytes(),
            )
        }
        _ => return Err(TextPlacementError::SourceShapeMismatch),
    };

    let patch_start = usize::try_from(fixup.patch_function_offset)
        .map_err(|_| TextPlacementError::OffsetOverflow)?;
    let patch_end = patch_start
        .checked_add(usize::from(fixup.patch_byte_width))
        .ok_or(TextPlacementError::OffsetOverflow)?;
    let instruction_start =
        usize::try_from(instruction_offset).map_err(|_| TextPlacementError::OffsetOverflow)?;
    let instruction_end = instruction_start
        .checked_add(instruction_bytes.len())
        .ok_or(TextPlacementError::OffsetOverflow)?;
    if function_bytes.get(instruction_start..instruction_end) != Some(instruction_bytes) {
        return Err(TextPlacementError::SourceShapeMismatch);
    }
    function_bytes
        .get_mut(patch_start..patch_end)
        .ok_or(TextPlacementError::SourceShapeMismatch)?
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
) -> Result<PlacedNormalizedForeignCallResolution, TextPlacementError> {
    if (invocation_site.operation().is_none() && !fixup.callbacks.is_empty())
        || fixup.state != FunctionFragmentNormalizedForeignCallFixupState::UnresolvedImportFieldV1
        || fixup.opcode_function_offset < instruction_offset
        || fixup.addend != 0
    {
        return Err(TextPlacementError::SourceShapeMismatch);
    }
    let section_at = |offset| section_coordinate(function_section_offset, offset);
    let mut cursor = 0_u64;
    let mut check_group =
        |callback: &machine_code::FunctionFragmentCallbackMaterialization,
         expected: &[u8],
         patch_offsets: &[u64]|
         -> Result<(), TextPlacementError> {
            let group_start = instruction_offset
                .checked_add(cursor)
                .ok_or(TextPlacementError::OffsetOverflow)?;
            let group_end = cursor
                .checked_add(expected.len() as u64)
                .ok_or(TextPlacementError::OffsetOverflow)?;
            let bytes_match = usize::try_from(group_end)
                .ok()
                .and_then(|end| instruction_bytes.get(cursor as usize..end));
            if callback.code_function_offset != group_start
                || callback.code_byte_count != expected.len() as u64
                || bytes_match != Some(expected)
            {
                return Err(TextPlacementError::SourceShapeMismatch);
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
                return Err(TextPlacementError::SourceShapeMismatch);
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
                    return Err(TextPlacementError::SourceShapeMismatch);
                };
                let destination = match callback.destination.clone() {
                    machine_code::CallbackAddressDestination::Register(register) => {
                        isa_x86_64::X86_64CallbackMaterializationDestination::Register(register)
                    }
                    machine_code::CallbackAddressDestination::RecordField {
                        root_register,
                        placements,
                    } => isa_x86_64::X86_64CallbackMaterializationDestination::RecordField {
                        root_register,
                        placements,
                    },
                    machine_code::CallbackAddressDestination::OutgoingStack { byte_offset } => {
                        isa_x86_64::X86_64CallbackMaterializationDestination::OutgoingStack {
                            byte_offset,
                        }
                    }
                };
                let bytes = isa_x86_64::encode_private_callback_materialization(destination)
                    .ok_or(TextPlacementError::SourceShapeMismatch)?;
                check_group(callback, &bytes, &[3])?;
            }
            let call_offset = instruction_offset
                .checked_add(cursor)
                .ok_or(TextPlacementError::OffsetOverflow)?;
            if instruction_bytes.get(cursor as usize..) != Some(&[0xe8_u8, 0, 0, 0, 0][..])
                || fixup.opcode_function_offset != call_offset
                || fixup.patch_function_offset != call_offset + 1
                || fixup.reference_function_offset != call_offset + 5
                || fixup.patch_byte_width != 4
            {
                return Err(TextPlacementError::SourceShapeMismatch);
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
                    return Err(TextPlacementError::SourceShapeMismatch);
                };
                let destination = match callback.destination.clone() {
                    machine_code::CallbackAddressDestination::Register(register) => {
                        isa_aarch64::Aarch64CallbackMaterializationDestination::Register(register)
                    }
                    machine_code::CallbackAddressDestination::RecordField {
                        root_register,
                        placements,
                    } => isa_aarch64::Aarch64CallbackMaterializationDestination::RecordField {
                        root_register,
                        placements,
                    },
                    machine_code::CallbackAddressDestination::OutgoingStack { .. } => {
                        return Err(TextPlacementError::SourceShapeMismatch);
                    }
                };
                let bytes = isa_aarch64::encode_private_callback_materialization(&destination)
                    .ok_or(TextPlacementError::SourceShapeMismatch)?;
                check_group(callback, &bytes, &[0, 4])?;
            }
            let call_offset = instruction_offset
                .checked_add(cursor)
                .ok_or(TextPlacementError::OffsetOverflow)?;
            if instruction_bytes.get(cursor as usize..)
                != Some(&0x9400_0000_u32.to_le_bytes()[..])
                || fixup.opcode_function_offset != call_offset
                || fixup.patch_function_offset != call_offset
                || fixup.reference_function_offset != call_offset
                || fixup.patch_byte_width != 4
            {
                return Err(TextPlacementError::SourceShapeMismatch);
            }
            NormalizedForeignCallResolutionKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1
        }
        _ => return Err(TextPlacementError::SourceShapeMismatch),
    };

    let instruction_start =
        usize::try_from(instruction_offset).map_err(|_| TextPlacementError::OffsetOverflow)?;
    let instruction_end = instruction_start
        .checked_add(instruction_bytes.len())
        .ok_or(TextPlacementError::OffsetOverflow)?;
    if function_bytes.get(instruction_start..instruction_end) != Some(instruction_bytes) {
        return Err(TextPlacementError::SourceShapeMismatch);
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
                        .ok_or(TextPlacementError::OffsetOverflow)?,
                    code_byte_count: callback.code_byte_count,
                    encoding: match callback.encoding {
                        machine_code::FunctionFragmentCallbackMaterializationEncoding::X86_64Relative32 {
                            patch_function_offset,
                        } => machine_code::PlacedCallbackAddressEncoding::X86_64Relative32 {
                            relocation_function_offset: patch_function_offset,
                            relocation_section_offset: function_section_offset
                                .checked_add(patch_function_offset)
                                .ok_or(TextPlacementError::OffsetOverflow)?,
                        },
                        machine_code::FunctionFragmentCallbackMaterializationEncoding::Aarch64PageAddress {
                            page_patch_function_offset,
                            page_offset_patch_function_offset,
                        } => machine_code::PlacedCallbackAddressEncoding::Aarch64PageAddress {
                            page_relocation_function_offset: page_patch_function_offset,
                            page_relocation_section_offset: function_section_offset
                                .checked_add(page_patch_function_offset)
                                .ok_or(TextPlacementError::OffsetOverflow)?,
                            page_offset_relocation_function_offset:
                                page_offset_patch_function_offset,
                            page_offset_relocation_section_offset: function_section_offset
                                .checked_add(page_offset_patch_function_offset)
                                .ok_or(TextPlacementError::OffsetOverflow)?,
                        },
                    },
                })
            })
            .collect::<Result<Vec<_>, TextPlacementError>>()?,
    })
}

fn check_placed_section(
    input: TextPlacementInput<'_>,
    section: &RelocationFreeTextSectionPlacement,
) -> Result<(), TextPlacementError> {
    let fragments = input.fragments();
    let count = fragments.functions.len();
    let alignment = match fragments.target.architecture {
        Architecture::X86_64 => 1,
        Architecture::Aarch64 => 4,
    };
    if section.identity != section.recomputed_identity()
        || section.source_fragments != fragments.identity
        || section.psi != fragments.psi
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
        return Err(TextPlacementError::ArtifactMismatch);
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
        .ok_or(TextPlacementError::MissingSemanticEntry(fragments.entry))?;
    if section.semantic_entry_offset != entry || section.byte_count != extent {
        return Err(TextPlacementError::ArtifactMismatch);
    }
    check_placed_functions(input, section, &offsets, alignment)
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
) -> Result<(), TextPlacementError> {
    if lookup_function_offset(offsets, machine).is_some() {
        return Err(TextPlacementError::DuplicateFunction(machine));
    }
    offsets.push((machine, *extent));
    if count != bytes.len() as u64 {
        return Err(TextPlacementError::SourceShapeMismatch);
    }
    if !extent.is_multiple_of(alignment) || !count.is_multiple_of(alignment) {
        return Err(TextPlacementError::MisalignedAarch64Span);
    }
    *extent = checked_add(*extent, count)?;
    Ok(())
}

fn check_placed_functions(
    input: TextPlacementInput<'_>,
    section: &RelocationFreeTextSectionPlacement,
    offsets: &[(MachineId, u64)],
    alignment: u64,
) -> Result<(), TextPlacementError> {
    let fragments = input.fragments();
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
            .ok_or(TextPlacementError::ArtifactMismatch)?;
        check_function_span(placed, index, source.machine, offset, source.byte_count)?;
        if placed.blocks.len() != source.blocks.len() {
            return Err(TextPlacementError::ArtifactMismatch);
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
                return Err(TextPlacementError::ArtifactMismatch);
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
                        return Err(TextPlacementError::SourceShapeMismatch);
                    }
                    if !matches!(input, TextPlacementInput::InternalCalls(_)) {
                        return Err(TextPlacementError::UnsupportedRelocationShape);
                    }
                    let FunctionFragmentControlProvenance::InternalCall {
                        target: selected_instructions::SelectedCallTarget::Machine(callee),
                        owner,
                    } = source_row.control
                    else {
                        return Err(TextPlacementError::SourceShapeMismatch);
                    };
                    match owner {
                        target_operations::CallSiteOwner::Operation(operation)
                            if source_row.provenance.operations.last() == Some(&operation) => {}
                        target_operations::CallSiteOwner::InvokeNoNormal(edge) => {
                            validate_invocation_site(
                                semantic_vocabulary::InvocationSite::InvokeNoNormal(edge),
                                &source_row.provenance.operations,
                                &source_row.provenance.edges,
                            )?;
                        }
                        target_operations::CallSiteOwner::ExternalCompletion(edge) => {
                            validate_invocation_site(
                                semantic_vocabulary::InvocationSite::ExternalCompletion(edge),
                                &source_row.provenance.operations,
                                &source_row.provenance.edges,
                            )?;
                        }
                        target_operations::CallSiteOwner::Installation { operation, .. }
                            if source_row.provenance.operations.as_slice() == [operation]
                                && source_row.provenance.edges.is_empty() => {}
                        target_operations::CallSiteOwner::CleanupAction { edge, .. }
                            if source_row.provenance.operations.is_empty()
                                && source_row.provenance.edges.as_slice() == [edge]
                                && source_row.provenance.work.is_empty() => {}
                        _ => return Err(TextPlacementError::SourceShapeMismatch),
                    }
                    let candidate = resolutions
                        .next()
                        .ok_or(TextPlacementError::ArtifactMismatch)?;
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
                        .ok_or(TextPlacementError::SourceShapeMismatch)?;
                    let mut group_cursor = source_row.offset;
                    for callback in &fixup.callbacks {
                        if callback.code_function_offset != group_cursor {
                            return Err(TextPlacementError::SourceShapeMismatch);
                        }
                        group_cursor = checked_add(group_cursor, callback.code_byte_count)?;
                    }
                    if group_cursor != fixup.opcode_function_offset {
                        return Err(TextPlacementError::SourceShapeMismatch);
                    }
                    let call_byte_count = source_row
                        .bytes
                        .len()
                        .checked_sub(opcode_relative as usize)
                        .map(|count| count as u64)
                        .ok_or(TextPlacementError::SourceShapeMismatch)?;
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
                        return Err(TextPlacementError::SourceShapeMismatch);
                    }
                    if !matches!(input, TextPlacementInput::InternalCalls(_)) {
                        return Err(TextPlacementError::UnsupportedRelocationShape);
                    }
                    let FunctionFragmentControlProvenance::NormalizedForeignCall {
                        invocation_site,
                        boundary,
                        ordinal,
                    } = source_row.control
                    else {
                        return Err(TextPlacementError::SourceShapeMismatch);
                    };
                    if boundary != fixup.boundary || ordinal != fixup.ordinal {
                        return Err(TextPlacementError::SourceShapeMismatch);
                    }
                    validate_invocation_site(
                        invocation_site,
                        &source_row.provenance.operations,
                        &source_row.provenance.edges,
                    )?;
                    let candidate = foreign.next().ok_or(TextPlacementError::ArtifactMismatch)?;
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
                    if !matches!(input, TextPlacementInput::InternalCalls(_)) {
                        return Err(TextPlacementError::UnsupportedRelocationShape);
                    }
                    let candidate = addresses
                        .next()
                        .ok_or(TextPlacementError::ArtifactMismatch)?;
                    let expected = machine_code::PlacedImageAddress {
                        machine: source.machine,
                        block: source_block.block,
                        instruction: source_row.instruction,
                        reference: fixup.reference,
                        operation: *source_row
                            .provenance
                            .operations
                            .last()
                            .ok_or(TextPlacementError::ArtifactMismatch)?,
                        instruction_function_offset: source_row.offset,
                        instruction_section_offset: checked_add(offset, source_row.offset)?,
                        instruction_byte_count: source_row.bytes.len() as u64,
                        fields: fixup
                            .fields
                            .shifted(offset)
                            .ok_or(TextPlacementError::OffsetOverflow)?,
                    };
                    if *candidate != expected {
                        return Err(TextPlacementError::ArtifactMismatch);
                    }
                }
            }
        }
        unchanged_bytes(&source.bytes, candidate_bytes, patches)?;
    }

    if resolutions.next().is_some() || foreign.next().is_some() || addresses.next().is_some() {
        return Err(TextPlacementError::ArtifactMismatch);
    }
    Ok(())
}

struct Call<'a> {
    caller: MachineId,
    block: SelectedBlockId,
    instruction: SelectedInstructionId,
    owner: target_operations::CallSiteOwner,
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
) -> Result<(u64, u64), TextPlacementError> {
    let fixup = call.fixup;
    let callee_offset = lookup_function_offset(offsets, call.callee).ok_or(
        TextPlacementError::MissingInternalMachineTarget(call.callee),
    )?;
    if fixup.state != FunctionFragmentInternalMachineFixupState::UnresolvedZeroFieldV1
        || fixup.addend != 0
        || fixup.callee != call.callee
        || fixup.patch_byte_width != 4
    {
        return Err(TextPlacementError::SourceShapeMismatch);
    }
    let opcode_relative = fixup
        .opcode_function_offset
        .checked_sub(call.offset)
        .ok_or(TextPlacementError::SourceShapeMismatch)?;
    let patch_relative = fixup
        .patch_function_offset
        .checked_sub(call.offset)
        .ok_or(TextPlacementError::SourceShapeMismatch)?;
    let field = slice_bytes(candidate_bytes, fixup.patch_function_offset, 4)?;
    let field: [u8; 4] = field
        .try_into()
        .map_err(|_| TextPlacementError::ArtifactMismatch)?;
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
                return Err(TextPlacementError::SourceShapeMismatch);
            }
            if slice_bytes(candidate_bytes, fixup.opcode_function_offset, 1)? != [0xe8] {
                return Err(TextPlacementError::ArtifactMismatch);
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
                return Err(TextPlacementError::SourceShapeMismatch);
            }
            let instruction = u32::from_le_bytes(field);
            if instruction & 0xfc00_0000 != 0x9400_0000 {
                return Err(TextPlacementError::ArtifactMismatch);
            }
            let signed_words = ((instruction & 0x03ff_ffff) << 6) as i32 >> 6;
            (
                InternalMachineCallResolutionKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1,
                signed_words * 4,
            )
        }
        _ => return Err(TextPlacementError::SourceShapeMismatch),
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
        return Err(TextPlacementError::ArtifactMismatch);
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

fn check_foreign_call(
    call: ForeignCall<'_>,
    architecture: Architecture,
    section_offset: u64,
    candidate_bytes: &[u8],
    row: &PlacedNormalizedForeignCallResolution,
) -> Result<(), TextPlacementError> {
    let fixup = call.fixup;
    if (call.invocation_site.operation().is_none() && !fixup.callbacks.is_empty())
        || fixup.state != FunctionFragmentNormalizedForeignCallFixupState::UnresolvedImportFieldV1
        || fixup.addend != 0
        || fixup.patch_byte_width != 4
    {
        return Err(TextPlacementError::SourceShapeMismatch);
    }
    let opcode_relative = fixup
        .opcode_function_offset
        .checked_sub(call.offset)
        .ok_or(TextPlacementError::SourceShapeMismatch)?;
    let patch_relative = fixup
        .patch_function_offset
        .checked_sub(call.offset)
        .ok_or(TextPlacementError::SourceShapeMismatch)?;
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
                return Err(TextPlacementError::SourceShapeMismatch);
            }
            if slice_bytes(candidate_bytes, fixup.opcode_function_offset, 1)? != [0xe8]
                || field != [0, 0, 0, 0]
            {
                return Err(TextPlacementError::ArtifactMismatch);
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
                return Err(TextPlacementError::SourceShapeMismatch);
            }
            if field != 0x9400_0000_u32.to_le_bytes() {
                return Err(TextPlacementError::ArtifactMismatch);
            }
            NormalizedForeignCallResolutionKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1
        }
        _ => return Err(TextPlacementError::SourceShapeMismatch),
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
    {
        return Err(TextPlacementError::ArtifactMismatch);
    }
    Ok(())
}

fn unchanged_bytes(
    source: &[u8],
    candidate: &[u8],
    mut patches: Vec<(u64, u64)>,
) -> Result<(), TextPlacementError> {
    if source.len() != candidate.len() {
        return Err(TextPlacementError::ArtifactMismatch);
    }
    patches.sort_unstable();
    let mut cursor = 0;
    for (start, end) in patches {
        if start < cursor || end < start || end > source.len() as u64 {
            return Err(TextPlacementError::ArtifactMismatch);
        }
        if slice_bytes(source, cursor, start - cursor)?
            != slice_bytes(candidate, cursor, start - cursor)?
        {
            return Err(TextPlacementError::ArtifactMismatch);
        }
        cursor = end;
    }
    if slice_bytes(source, cursor, source.len() as u64 - cursor)?
        != slice_bytes(candidate, cursor, candidate.len() as u64 - cursor)?
    {
        return Err(TextPlacementError::ArtifactMismatch);
    }
    Ok(())
}

fn slice_bytes(source: &[u8], start: u64, count: u64) -> Result<&[u8], TextPlacementError> {
    let end = checked_add(start, count)?;
    let start = usize::try_from(start).map_err(|_| TextPlacementError::OffsetOverflow)?;
    let end = usize::try_from(end).map_err(|_| TextPlacementError::OffsetOverflow)?;
    source
        .get(start..end)
        .ok_or(TextPlacementError::ArtifactMismatch)
}

fn checked_add(left: u64, right: u64) -> Result<u64, TextPlacementError> {
    left.checked_add(right)
        .ok_or(TextPlacementError::OffsetOverflow)
}

fn check_function_span(
    candidate: &PlacedFunctionFragment,
    index: usize,
    machine: MachineId,
    offset: u64,
    count: u64,
) -> Result<(), TextPlacementError> {
    if candidate.source_function_index != index as u64
        || candidate.machine != machine
        || candidate.section_offset != offset
        || candidate.byte_count != count
    {
        return Err(TextPlacementError::ArtifactMismatch);
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
) -> Result<(), TextPlacementError> {
    if !offset.is_multiple_of(alignment) || !count.is_multiple_of(alignment) {
        return Err(TextPlacementError::MisalignedAarch64Span);
    }
    if checked_add(offset, count)? > extent {
        return Err(TextPlacementError::SourceShapeMismatch);
    }
    if candidate.block != block
        || candidate.function_offset != offset
        || candidate.section_offset != checked_add(section, offset)?
        || candidate.byte_count != count
    {
        return Err(TextPlacementError::ArtifactMismatch);
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
) -> Result<(), TextPlacementError> {
    if !offset.is_multiple_of(alignment) || !count.is_multiple_of(alignment) {
        return Err(TextPlacementError::MisalignedAarch64Span);
    }
    if checked_add(offset, count)? > extent {
        return Err(TextPlacementError::SourceShapeMismatch);
    }
    if candidate.instruction != instruction
        || candidate.alternative != alternative
        || candidate.function_offset != offset
        || candidate.section_offset != checked_add(section, offset)?
        || candidate.byte_count != count
    {
        return Err(TextPlacementError::ArtifactMismatch);
    }
    Ok(())
}
