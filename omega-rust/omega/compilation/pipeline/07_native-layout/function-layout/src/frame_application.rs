/*
We insert target frame prologue and epilogue bytes into unplaced function
fragments, reflow and widen displaced conditional branches, and seal the
frame-application receipt.

`stage_function_fragment_frame_application` inserts each function's prologue at
offset 0 and its epilogue before every retained return instruction, shifts
block, instruction, and fixup coordinates, widens any short conditional branch
whose taken edge moved out of reach (`widen_out_of_range_branches`), and
patches the supplied branch encodings (`patch_branches`).
`validate_function_fragment_frame_application` replays source fragment custody,
verifies the least widening closure (`validate_widths`), decodes every branch
row against its moved block offsets (`validate_branch_row`), and checks the
application receipt.
*/

use crate::fragment_emission::{
    FunctionFragmentEmissionError, StagedOptimizedFunctionFragmentEmission,
    validate_optimized_function_fragment_emission,
};
use encoding_admission::frame_protocol::{
    FunctionTargetFrameProtocolEncoding, TargetFrameProtocolEncodingPlan,
    target_frame_protocol_encoding_identity,
};
use encoding_admission::relative_branches::{self, RelativeBranchError, validate_branch_row};
pub use machine_code::function_fragment_frame_application_identity;
use machine_code::{
    EncodedControlIndex, FunctionFragment, FunctionFragmentBranchEvidence,
    FunctionFragmentConditionalBranchEvidence, FunctionFragmentControlProvenance,
    FunctionFragmentEmissionPlan, FunctionFragmentInstructionSpan,
    FunctionFragmentInternalMachineFixup, TargetFrameProtocolEncodingIdentity,
};
pub use machine_code::{
    FunctionAppliedFrameEpilogue, FunctionAppliedFrameProtocol, FunctionFragmentFrameApplication,
    FunctionFragmentFrameApplicationIdentity,
};
use optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
};
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{MachineAlternativeFamily, SelectedBlockId, SelectedInstructionId};
use semantic_vocabulary::MachineId;
use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;
use target::Architecture;

const WIDENING_GROWTH: u64 = 4;

pub fn stage_function_fragment_frame_application(
    source: StagedOptimizedFunctionFragmentEmission,
) -> Result<StagedFunctionFragmentFrameApplication, FunctionFragmentFrameApplicationError> {
    let application = {
        let protocol = source.source().frame_protocol();
        let controls = EncodedControlIndex::partition(
            source.source().encoding().rows(),
            source
                .fragments()
                .functions
                .iter()
                .map(|function| function.instructions().count()),
        )
        .map_err(|_| FunctionFragmentFrameApplicationError::ArtifactMismatch)?;
        apply_frame_protocol_to_fragments(
            source.fragments(),
            source.manifest().record().identity,
            protocol,
            &controls,
        )
        .map_err(FunctionFragmentFrameApplicationError::from)?
    };
    let branch_origins = prepare_branch_origins(source.fragments()).into();
    let application = Arc::new(application);
    let receipt = seal(&application);
    let placed_body = Arc::new(PlacedBodyEvidence::capture(
        &source,
        &application,
        branch_origins,
    ));
    let retains_originals = source.source().has_original_replay();
    Ok(StagedFunctionFragmentFrameApplication {
        placed_body,
        retains_originals,
        source,
        application,
        receipt,
    })
}

pub(crate) fn validate_function_fragment_frame_application(
    staged: &StagedFunctionFragmentFrameApplication,
) -> Result<FunctionFragmentFrameApplicationReceipt, FunctionFragmentFrameApplicationError> {
    validate_optimized_function_fragment_emission(&staged.source)
        .map_err(FunctionFragmentFrameApplicationError::Source)?;
    let expected = PlacedBodyEvidence::capture(
        &staged.source,
        &staged.application,
        prepare_branch_origins(staged.source.fragments()).into(),
    );
    if *staged.placed_body != expected
        || staged.retains_originals != staged.source.source().has_original_replay()
    {
        return Err(FunctionFragmentFrameApplicationError::ArtifactMismatch);
    }
    let protocol = staged.source.source().frame_protocol();
    if staged.application.frame_protocol != target_frame_protocol_encoding_identity(protocol) {
        return Err(FunctionFragmentFrameApplicationError::ArtifactMismatch);
    }
    validate_frame_protocol_application(
        staged.source.fragments(),
        staged.source.manifest().record().identity,
        protocol,
        staged.source.source().register_environment().physical(),
        &staged.application,
    )?;
    let receipt = seal(&staged.application);
    if staged.receipt != receipt {
        return Err(FunctionFragmentFrameApplicationError::ReceiptMismatch);
    }
    Ok(receipt)
}

/// Current layout output and obligations carried with the placed code. It contains
/// no compiler stage, original instruction graph or capability to run a producer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedBodyEvidence {
    pub code: Arc<machine_code::placed_body::NativeBodyReadback>,
    pub facts: Arc<physical_program::EmissionFacts>,
    pub authority: Arc<installation_admission::BoundInstallationAuthority>,
}

impl PlacedBodyEvidence {
    fn capture(
        fragments: &StagedOptimizedFunctionFragmentEmission,
        application: &Arc<FunctionFragmentFrameApplication>,
        branches: Arc<[machine_code::frame_branch_origins::FunctionFrameBranchOrigins]>,
    ) -> Self {
        let current = fragments.source();
        let program = current.program();
        Self {
            code: Arc::new(machine_code::placed_body::NativeBodyReadback {
                physical: current.register_environment().physical().clone(),
                encoding: Arc::clone(&program.encoding),
                layout: Arc::clone(&program.layout),
                frame: Arc::clone(&program.frame),
                protocol: Arc::clone(&program.protocol),
                frame_application: Arc::clone(application),
                branches,
                allocation: Arc::clone(&program.context.publication),
                fragment_statistics: fragments.manifest().record().statistics,
            }),
            facts: current.shared_facts(),
            authority: Arc::clone(current.installation_authority()),
        }
    }
}

#[derive(Debug)]
#[must_use = "frame-applied fragments retain both selected-fragment and target-protocol custody"]
pub struct StagedFunctionFragmentFrameApplication {
    pub(crate) placed_body: Arc<PlacedBodyEvidence>,
    pub(crate) retains_originals: bool,
    source: StagedOptimizedFunctionFragmentEmission, // Admitted unplaced function-fragment stage.
    application: Arc<FunctionFragmentFrameApplication>, // Frame-applied function fragments and epilogue spans.
    receipt: FunctionFragmentFrameApplicationReceipt,   // Sealed frame-application custody receipt.
}

impl StagedFunctionFragmentFrameApplication {
    pub const fn source(&self) -> &StagedOptimizedFunctionFragmentEmission {
        &self.source
    }

    pub fn application(&self) -> &FunctionFragmentFrameApplication {
        &self.application
    }

    pub fn fragments(&self) -> &FunctionFragmentEmissionPlan {
        &self.application.fragments
    }

    pub const fn receipt(&self) -> FunctionFragmentFrameApplicationReceipt {
        self.receipt
    }

    pub fn branch_origins(
        &self,
    ) -> &[machine_code::frame_branch_origins::FunctionFrameBranchOrigins] {
        &self.placed_body.code.branches
    }

    pub fn validate_relative_branch_rows<'a>(
        &self,
        machine: MachineId,
        candidates: impl IntoIterator<Item = &'a FunctionFragmentInstructionSpan>,
    ) -> Result<Vec<SelectedInstructionId>, FunctionFragmentFrameApplicationError> {
        validate_current_relative_branch_rows(
            self.fragments(),
            self.branch_origins(),
            self.source().source().register_environment().physical(),
            machine,
            candidates,
        )
    }
}

/// Validate the carried branch obligations against the current framed bytes.
/// The input contains no earlier fragment plan or executable source graph.
pub fn validate_current_relative_branch_rows<'a>(
    fragments: &FunctionFragmentEmissionPlan,
    origins: &[machine_code::frame_branch_origins::FunctionFrameBranchOrigins],
    physical: &ValidatedPhysicalRegisterModel,
    machine: MachineId,
    candidates: impl IntoIterator<Item = &'a FunctionFragmentInstructionSpan>,
) -> Result<Vec<SelectedInstructionId>, FunctionFragmentFrameApplicationError> {
    relative_branches::validate_current_relative_branch_rows(
        fragments, origins, physical, machine, candidates,
    )
    .map_err(|error| FrameApplicationError::from(error).into())
}

fn prepare_branch_origins(
    fragments: &FunctionFragmentEmissionPlan,
) -> Vec<machine_code::frame_branch_origins::FunctionFrameBranchOrigins> {
    use machine_code::frame_branch_origins::*;
    fragments
        .functions
        .iter()
        .map(|function| FunctionFrameBranchOrigins {
            machine: function.machine,
            branches: function
                .instructions()
                .filter_map(|row| {
                    row.branch.as_deref().map(|branch| FrameBranchOrigin {
                        instruction: row.instruction,
                        branch: branch.clone(),
                    })
                })
                .collect(),
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionFragmentFrameApplicationReceipt {
    identity: FunctionFragmentFrameApplicationIdentity, // Frame-application record identity.
    source_fragment_manifest: FunctionFragmentEmissionManifestIdentity, // Source fragment-emission manifest identity.
    source_fragments: FunctionFragmentEmissionIdentity, // Source unplaced fragment-plan identity.
    frame_protocol: TargetFrameProtocolEncodingIdentity, // Applied frame-protocol encoding identity.
    fragments: FunctionFragmentEmissionIdentity,         // Resulting framed fragment-plan identity.
    framed_function_count: usize, // Number of functions with non-empty frame bytes.
    epilogue_application_count: usize, // Total epilogue insertions across all functions.
}

impl FunctionFragmentFrameApplicationReceipt {
    pub const fn identity(self) -> FunctionFragmentFrameApplicationIdentity {
        self.identity
    }

    pub const fn source_fragment_manifest(self) -> FunctionFragmentEmissionManifestIdentity {
        self.source_fragment_manifest
    }

    pub const fn source_fragments(self) -> FunctionFragmentEmissionIdentity {
        self.source_fragments
    }

    pub const fn frame_protocol(self) -> TargetFrameProtocolEncodingIdentity {
        self.frame_protocol
    }

    pub const fn fragments(self) -> FunctionFragmentEmissionIdentity {
        self.fragments
    }

    pub const fn framed_function_count(self) -> usize {
        self.framed_function_count
    }

    pub const fn epilogue_application_count(self) -> usize {
        self.epilogue_application_count
    }
}

fn seal(application: &FunctionFragmentFrameApplication) -> FunctionFragmentFrameApplicationReceipt {
    FunctionFragmentFrameApplicationReceipt {
        identity: application.recomputed_identity(),
        source_fragment_manifest: application.source_fragment_manifest,
        source_fragments: application.source_fragments,
        frame_protocol: application.frame_protocol,
        fragments: application.fragments.identity,
        framed_function_count: application
            .functions
            .iter()
            .filter(|row| {
                row.prologue_byte_count != 0
                    || row
                        .epilogues
                        .iter()
                        .any(|epilogue| epilogue.byte_count != 0)
            })
            .count(),
        epilogue_application_count: application
            .functions
            .iter()
            .map(|row| row.epilogues.len())
            .sum(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionFragmentFrameApplicationError {
    Source(FunctionFragmentEmissionError),
    SourceKindMismatch,
    RootMismatch,
    FunctionRosterMismatch,
    MissingFunction(MachineId),
    InvalidProtocolSpan(MachineId),
    UnsupportedFramedControl(MachineId),
    MissingFinalReturn(MachineId),
    SourceShapeMismatch(MachineId),
    MissingTargetBlock(SelectedBlockId),
    BranchFallthroughMismatch(SelectedInstructionId),
    BranchEffectsMismatch(SelectedInstructionId),
    BranchEncoding(SelectedInstructionId, machine_code::BranchPatchError),
    X86_64Branch(
        SelectedInstructionId,
        isa_x86_64::X86_64SelectedFormEncodingError,
    ),
    Aarch64Branch(
        SelectedInstructionId,
        isa_aarch64::Aarch64SelectedFormEncodingError,
    ),
    OffsetOverflow,
    ArtifactMismatch,
    ReceiptMismatch,
}

diagnostics::failed_display!(
    FunctionFragmentFrameApplicationError,
    "function-fragment frame application failed"
);

impl std::error::Error for FunctionFragmentFrameApplicationError {}

impl From<FrameApplicationError> for FunctionFragmentFrameApplicationError {
    fn from(error: FrameApplicationError) -> Self {
        use FrameApplicationError as Source;
        match error {
            Source::RootMismatch => Self::RootMismatch,
            Source::FunctionRosterMismatch => Self::FunctionRosterMismatch,
            Source::MissingFunction(value) => Self::MissingFunction(value),
            Source::InvalidProtocolSpan(value) => Self::InvalidProtocolSpan(value),
            Source::UnsupportedFramedControl(value) => Self::UnsupportedFramedControl(value),
            Source::MissingFinalReturn(value) => Self::MissingFinalReturn(value),
            Source::SourceShapeMismatch(value) => Self::SourceShapeMismatch(value),
            Source::MissingTargetBlock(value) => Self::MissingTargetBlock(value),
            Source::BranchFallthroughMismatch(value) => Self::BranchFallthroughMismatch(value),
            Source::BranchEffectsMismatch(value) => Self::BranchEffectsMismatch(value),
            Source::BranchEncoding(instruction, error) => Self::BranchEncoding(instruction, error),
            Source::X86_64Branch(instruction, error) => Self::X86_64Branch(instruction, error),
            Source::Aarch64Branch(instruction, error) => Self::Aarch64Branch(instruction, error),
            Source::OffsetOverflow => Self::OffsetOverflow,
            Source::ArtifactMismatch => Self::ArtifactMismatch,
        }
    }
}

impl From<RelativeBranchError> for FrameApplicationError {
    fn from(error: RelativeBranchError) -> Self {
        use RelativeBranchError as Source;
        match error {
            Source::MissingFunction(machine) => Self::MissingFunction(machine),
            Source::SourceShapeMismatch(machine) => Self::SourceShapeMismatch(machine),
            Source::MissingTargetBlock(block) => Self::MissingTargetBlock(block),
            Source::BranchFallthroughMismatch(instruction) => {
                Self::BranchFallthroughMismatch(instruction)
            }
            Source::BranchEffectsMismatch(instruction) => Self::BranchEffectsMismatch(instruction),
            Source::X86_64Branch(instruction, error) => Self::X86_64Branch(instruction, error),
            Source::Aarch64Branch(instruction, error) => Self::Aarch64Branch(instruction, error),
            Source::OffsetOverflow => Self::OffsetOverflow,
            Source::ArtifactMismatch => Self::ArtifactMismatch,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FrameApplicationError {
    RootMismatch,
    FunctionRosterMismatch,
    MissingFunction(MachineId),
    InvalidProtocolSpan(MachineId),
    UnsupportedFramedControl(MachineId),
    MissingFinalReturn(MachineId),
    SourceShapeMismatch(MachineId),
    MissingTargetBlock(SelectedBlockId),
    BranchFallthroughMismatch(SelectedInstructionId),
    BranchEffectsMismatch(SelectedInstructionId),
    BranchEncoding(SelectedInstructionId, machine_code::BranchPatchError),
    X86_64Branch(
        SelectedInstructionId,
        isa_x86_64::X86_64SelectedFormEncodingError,
    ),
    Aarch64Branch(
        SelectedInstructionId,
        isa_aarch64::Aarch64SelectedFormEncodingError,
    ),
    OffsetOverflow,
    ArtifactMismatch,
}

diagnostics::failed_display!(
    FrameApplicationError,
    "function-fragment frame application failed"
);

impl std::error::Error for FrameApplicationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ShortConditionalForm {
    width: u64,              // Row byte width of the short form.
    reach: Range<i64>,       // Byte displacements reachable by the short form.
    measured_from_end: bool, // Whether displacement is measured from row end (x86) or start (AArch64).
}

impl ShortConditionalForm {
    fn reaches(&self, row_offset: u64, target: u64) -> bool {
        let reference = if self.measured_from_end {
            i128::from(row_offset) + i128::from(self.width)
        } else {
            i128::from(row_offset)
        };
        i64::try_from(i128::from(target) - reference)
            .is_ok_and(|displacement| self.reach.contains(&displacement))
    }
}

fn short_conditional_form(
    architecture: Architecture,
    branch: &FunctionFragmentConditionalBranchEvidence,
) -> Option<ShortConditionalForm> {
    match architecture {
        Architecture::X86_64 => Some(ShortConditionalForm {
            width: 2,
            reach: i64::from(i8::MIN)..i64::from(i8::MAX) + 1,
            measured_from_end: true,
        }),
        Architecture::Aarch64 => {
            branch
                .decoded_register_reads
                .is_empty()
                .then_some(ShortConditionalForm {
                    width: 4,
                    reach: isa_aarch64::AARCH64_CONDITIONAL_BRANCH_BYTE_REACH,
                    measured_from_end: false,
                })
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ReturnSite {
    block: SelectedBlockId,
    instruction: SelectedInstructionId,
    psi_edge: semantic_vocabulary::EdgeId,
    offset: u64,
}

pub(crate) fn apply_frame_protocol_to_fragments(
    source: &FunctionFragmentEmissionPlan,
    source_manifest: FunctionFragmentEmissionManifestIdentity,
    protocol: &TargetFrameProtocolEncodingPlan,
    controls: &[EncodedControlIndex<'_>],
) -> Result<FunctionFragmentFrameApplication, FrameApplicationError> {
    if source.target != protocol.target {
        return Err(FrameApplicationError::RootMismatch);
    }
    if source.functions.len() != protocol.functions.len()
        || source.functions.len() != controls.len()
    {
        return Err(FrameApplicationError::FunctionRosterMismatch);
    }

    let mut fragments = source.clone();
    let mut applications = Vec::with_capacity(fragments.functions.len());
    for (function, controls) in fragments.functions.iter_mut().zip(controls) {
        let mut rows = protocol
            .functions
            .iter()
            .filter(|row| row.machine == function.machine);
        let Some(row) = rows.next() else {
            return Err(FrameApplicationError::MissingFunction(function.machine));
        };
        if rows.next().is_some() {
            return Err(FrameApplicationError::FunctionRosterMismatch);
        }
        let prologue = row
            .prologue
            .bytes(&protocol.bytes)
            .ok_or(FrameApplicationError::InvalidProtocolSpan(function.machine))?;
        let epilogue = row
            .epilogue
            .bytes(&protocol.bytes)
            .ok_or(FrameApplicationError::InvalidProtocolSpan(function.machine))?;
        applications.push(apply_function(
            function,
            prologue,
            epilogue,
            source.target.architecture,
            controls,
        )?);
    }

    if protocol.functions.iter().any(|row| {
        !source
            .functions
            .iter()
            .any(|function| function.machine == row.machine)
    }) {
        return Err(FrameApplicationError::FunctionRosterMismatch);
    }

    fragments.identity = fragments.recomputed_identity();
    let mut application = FunctionFragmentFrameApplication {
        identity: FunctionFragmentFrameApplicationIdentity::from_bytes([0; 32]),
        source_fragment_manifest: source_manifest,
        source_fragments: source.identity,
        frame_protocol: target_frame_protocol_encoding_identity(protocol),
        functions: applications,
        fragments,
    };
    application.identity = application.recomputed_identity();
    Ok(application)
}

fn apply_function(
    function: &mut FunctionFragment,
    prologue: &[u8],
    epilogue: &[u8],
    architecture: Architecture,
    controls: &EncodedControlIndex<'_>,
) -> Result<FunctionAppliedFrameProtocol, FrameApplicationError> {
    if prologue.is_empty() != epilogue.is_empty() {
        return Err(FrameApplicationError::UnsupportedFramedControl(
            function.machine,
        ));
    }

    let source_len =
        u64::try_from(function.bytes.len()).map_err(|_| FrameApplicationError::OffsetOverflow)?;
    if function.byte_count != source_len {
        return Err(FrameApplicationError::SourceShapeMismatch(function.machine));
    }
    let prologue_len =
        u64::try_from(prologue.len()).map_err(|_| FrameApplicationError::OffsetOverflow)?;
    let epilogue_len =
        u64::try_from(epilogue.len()).map_err(|_| FrameApplicationError::OffsetOverflow)?;
    let return_sites = validate_and_collect_returns(function)?;

    let mut bytes = Vec::with_capacity(
        prologue
            .len()
            .checked_add(function.bytes.len())
            .and_then(|length| {
                epilogue
                    .len()
                    .checked_mul(return_sites.len())
                    .and_then(|epilogues| length.checked_add(epilogues))
            })
            .ok_or(FrameApplicationError::OffsetOverflow)?,
    );
    bytes.extend_from_slice(prologue);
    let mut source_cursor = 0_usize;
    let mut applications = Vec::with_capacity(return_sites.len());
    for (ordinal, site) in return_sites.iter().copied().enumerate() {
        let return_start =
            usize::try_from(site.offset).map_err(|_| FrameApplicationError::OffsetOverflow)?;
        bytes.extend_from_slice(&function.bytes[source_cursor..return_start]);
        let prior_epilogues = u64::try_from(ordinal)
            .map_err(|_| FrameApplicationError::OffsetOverflow)?
            .checked_mul(epilogue_len)
            .ok_or(FrameApplicationError::OffsetOverflow)?;
        let function_offset = prologue_len
            .checked_add(site.offset)
            .and_then(|offset| offset.checked_add(prior_epilogues))
            .ok_or(FrameApplicationError::OffsetOverflow)?;
        bytes.extend_from_slice(epilogue);
        applications.push(FunctionAppliedFrameEpilogue {
            block: site.block,
            return_instruction: site.instruction,
            psi_return_edge: site.psi_edge,
            function_offset,
            byte_count: epilogue_len,
        });
        source_cursor = return_start;
    }
    bytes.extend_from_slice(&function.bytes[source_cursor..]);

    for block in &mut function.blocks {
        let source_block_offset = block.offset;
        let prior_block_epilogues = insertion_count_before(&return_sites, source_block_offset)?;
        block.offset = shifted_offset(
            source_block_offset,
            prologue_len,
            prior_block_epilogues,
            epilogue_len,
        )?;
        let mut block_epilogue_count = 0_u64;
        for row in &mut block.instructions {
            let source_row_offset = row.offset;
            let prior_row_epilogues = insertion_count_before(&return_sites, source_row_offset)?;
            let is_return = matches!(
                row.control,
                FunctionFragmentControlProvenance::Return { .. }
            );
            if is_return {
                block_epilogue_count = block_epilogue_count
                    .checked_add(1)
                    .ok_or(FrameApplicationError::OffsetOverflow)?;
            }
            let applied_before_row = prior_row_epilogues
                .checked_add(u64::from(is_return))
                .ok_or(FrameApplicationError::OffsetOverflow)?;
            row.offset = shifted_offset(
                source_row_offset,
                prologue_len,
                applied_before_row,
                epilogue_len,
            )?;
            shift_row_fixups(row, row.offset - source_row_offset)?;
        }
        block.byte_count = block
            .byte_count
            .checked_add(
                block_epilogue_count
                    .checked_mul(epilogue_len)
                    .ok_or(FrameApplicationError::OffsetOverflow)?,
            )
            .ok_or(FrameApplicationError::OffsetOverflow)?;
    }
    function.byte_count = function
        .byte_count
        .checked_add(prologue_len)
        .and_then(|count| {
            epilogue_len
                .checked_mul(u64::try_from(return_sites.len()).ok()?)
                .and_then(|epilogues| count.checked_add(epilogues))
        })
        .ok_or(FrameApplicationError::OffsetOverflow)?;
    function.bytes = bytes;
    patch_branches(function, &mut applications, architecture, controls)?;

    Ok(FunctionAppliedFrameProtocol {
        machine: function.machine,
        prologue_function_offset: 0,
        prologue_byte_count: prologue_len,
        epilogues: applications,
    })
}

fn validate_and_collect_returns(
    function: &FunctionFragment,
) -> Result<Vec<ReturnSite>, FrameApplicationError> {
    let mut expected_block_offset = 0_u64;
    let mut returns = Vec::new();
    for block in &function.blocks {
        if block.offset != expected_block_offset {
            return Err(FrameApplicationError::SourceShapeMismatch(function.machine));
        }
        let mut expected_row_offset = block.offset;
        for (index, row) in block.instructions.iter().enumerate() {
            if row.offset != expected_row_offset {
                return Err(FrameApplicationError::SourceShapeMismatch(function.machine));
            }
            let row_len = u64::try_from(row.bytes.len())
                .map_err(|_| FrameApplicationError::OffsetOverflow)?;
            expected_row_offset = expected_row_offset
                .checked_add(row_len)
                .ok_or(FrameApplicationError::OffsetOverflow)?;
            if matches!(
                row.control,
                FunctionFragmentControlProvenance::HostedExitProcess { .. }
                    | FunctionFragmentControlProvenance::OperationTrap { .. }
                    | FunctionFragmentControlProvenance::EliminateEmpty { .. }
                    | FunctionFragmentControlProvenance::Crash { .. }
                    | FunctionFragmentControlProvenance::InternalCall {
                        owner: target_operations::CallSiteOwner::InvokeNoNormal(_),
                        ..
                    }
            ) && (index + 1 != block.instructions.len() || row.bytes.is_empty())
            {
                return Err(FrameApplicationError::MissingFinalReturn(function.machine));
            }
            if let FunctionFragmentControlProvenance::Return { psi_return_edge } = row.control {
                if index + 1 != block.instructions.len() {
                    return Err(FrameApplicationError::MissingFinalReturn(function.machine));
                }
                returns.push(ReturnSite {
                    block: block.block,
                    instruction: row.instruction,
                    psi_edge: psi_return_edge,
                    offset: row.offset,
                });
            }
        }
        if expected_row_offset
            != block
                .offset
                .checked_add(block.byte_count)
                .ok_or(FrameApplicationError::OffsetOverflow)?
        {
            return Err(FrameApplicationError::SourceShapeMismatch(function.machine));
        }
        expected_block_offset = expected_row_offset;
    }
    if expected_block_offset != function.byte_count {
        return Err(FrameApplicationError::SourceShapeMismatch(function.machine));
    }
    Ok(returns)
}

fn insertion_count_before(
    return_sites: &[ReturnSite],
    source_offset: u64,
) -> Result<u64, FrameApplicationError> {
    u64::try_from(
        return_sites
            .iter()
            .take_while(|site| site.offset < source_offset)
            .count(),
    )
    .map_err(|_| FrameApplicationError::OffsetOverflow)
}

fn shifted_offset(
    source_offset: u64,
    prologue_len: u64,
    prior_epilogues: u64,
    epilogue_len: u64,
) -> Result<u64, FrameApplicationError> {
    source_offset
        .checked_add(prologue_len)
        .and_then(|offset| {
            prior_epilogues
                .checked_mul(epilogue_len)
                .and_then(|shift| offset.checked_add(shift))
        })
        .ok_or(FrameApplicationError::OffsetOverflow)
}

fn shift_row_fixups(
    row: &mut FunctionFragmentInstructionSpan,
    shift: u64,
) -> Result<(), FrameApplicationError> {
    if let Some(fixup) = &mut row.internal_machine_fixup {
        shift_fixup(fixup, shift)?;
    }
    if let Some(fixup) = &mut row.normalized_foreign_call_fixup {
        shift_foreign_fixup(fixup, shift)?;
    }
    if let Some(fixup) = &mut row.image_address_fixup {
        fixup.fields = fixup
            .fields
            .shifted(shift)
            .ok_or(FrameApplicationError::OffsetOverflow)?;
    }
    Ok(())
}

fn shift_offset(offset: &mut u64, shift: u64) -> Result<(), FrameApplicationError> {
    *offset = offset
        .checked_add(shift)
        .ok_or(FrameApplicationError::OffsetOverflow)?;
    Ok(())
}

fn shift_fixup(
    fixup: &mut FunctionFragmentInternalMachineFixup,
    shift: u64,
) -> Result<(), FrameApplicationError> {
    shift_offset(&mut fixup.opcode_function_offset, shift)?;
    shift_offset(&mut fixup.patch_function_offset, shift)?;
    shift_offset(&mut fixup.reference_function_offset, shift)?;
    Ok(())
}

fn shift_foreign_fixup(
    fixup: &mut machine_code::FunctionFragmentNormalizedForeignCallFixup,
    shift: u64,
) -> Result<(), FrameApplicationError> {
    shift_offset(&mut fixup.opcode_function_offset, shift)?;
    shift_offset(&mut fixup.patch_function_offset, shift)?;
    shift_offset(&mut fixup.reference_function_offset, shift)?;
    for callback in &mut fixup.callbacks {
        shift_offset(&mut callback.code_function_offset, shift)?;
        match &mut callback.encoding {
            machine_code::FunctionFragmentCallbackMaterializationEncoding::X86_64Relative32 {
                patch_function_offset,
            } => shift_offset(patch_function_offset, shift)?,
            machine_code::FunctionFragmentCallbackMaterializationEncoding::Aarch64PageAddress {
                page_patch_function_offset,
                page_offset_patch_function_offset,
            } => {
                shift_offset(page_patch_function_offset, shift)?;
                shift_offset(page_offset_patch_function_offset, shift)?;
            }
        }
    }
    Ok(())
}

fn patch_branches(
    function: &mut FunctionFragment,
    epilogues: &mut [FunctionAppliedFrameEpilogue],
    architecture: Architecture,
    controls: &EncodedControlIndex<'_>,
) -> Result<(), FrameApplicationError> {
    widen_out_of_range_branches(function, epilogues, architecture, controls)?;
    let block_offsets = function
        .blocks
        .iter()
        .map(|block| (block.block, block.offset))
        .collect::<BTreeMap<_, _>>();
    let function_bytes = &mut function.bytes;
    for block in &mut function.blocks {
        for row in &mut block.instructions {
            let Some(branch) = row.branch.as_mut() else {
                continue;
            };
            let template = controls
                .get(row.instruction)
                .and_then(|forms| forms.matching(row.alternative, row.bytes.len()))
                .ok_or(FrameApplicationError::ArtifactMismatch)?;
            let target = match branch.as_mut() {
                FunctionFragmentBranchEvidence::Jump(branch) => {
                    let target = block_offsets.get(&branch.target_block).copied().ok_or(
                        FrameApplicationError::MissingTargetBlock(branch.target_block),
                    )?;
                    let reference = match architecture {
                        Architecture::X86_64 => row
                            .offset
                            .checked_add(row.bytes.len() as u64)
                            .ok_or(FrameApplicationError::OffsetOverflow)?,
                        Architecture::Aarch64 => row.offset,
                    };
                    branch.target_offset = target;
                    branch.byte_displacement = checked_delta(target, reference)?;
                    if template.footprint.encoded != branch.decoded_effects {
                        return Err(FrameApplicationError::BranchEffectsMismatch(
                            row.instruction,
                        ));
                    }
                    target
                }
                FunctionFragmentBranchEvidence::Conditional(branch) => {
                    rewrite_branch_coordinates(
                        row.instruction,
                        row.offset,
                        row.bytes.len(),
                        branch,
                        architecture,
                        &block_offsets,
                    )?;
                    if template.footprint.encoded != branch.decoded_effects
                        || template.footprint.register_reads != branch.decoded_register_reads
                    {
                        return Err(FrameApplicationError::BranchEffectsMismatch(
                            row.instruction,
                        ));
                    }
                    branch.when_taken_offset
                }
            };
            let encoded = template
                .patch(checked_delta(target, row.offset)?)
                .map_err(|error| FrameApplicationError::BranchEncoding(row.instruction, error))?;
            let start =
                usize::try_from(row.offset).map_err(|_| FrameApplicationError::OffsetOverflow)?;
            let end = start
                .checked_add(encoded.len())
                .ok_or(FrameApplicationError::OffsetOverflow)?;
            function_bytes
                .get_mut(start..end)
                .ok_or(FrameApplicationError::SourceShapeMismatch(function.machine))?
                .copy_from_slice(&encoded);
            row.bytes = encoded;
        }
    }
    Ok(())
}

fn rewrite_branch_coordinates(
    instruction: SelectedInstructionId,
    row_offset: u64,
    row_byte_count: usize,
    branch: &mut FunctionFragmentConditionalBranchEvidence,
    architecture: Architecture,
    block_offsets: &BTreeMap<SelectedBlockId, u64>,
) -> Result<(), FrameApplicationError> {
    let taken = block_offsets.get(&branch.when_taken_block).copied().ok_or(
        FrameApplicationError::MissingTargetBlock(branch.when_taken_block),
    )?;
    let fallthrough = block_offsets
        .get(&branch.when_fallthrough_block)
        .copied()
        .ok_or(FrameApplicationError::MissingTargetBlock(
            branch.when_fallthrough_block,
        ))?;
    let row_len =
        u64::try_from(row_byte_count).map_err(|_| FrameApplicationError::OffsetOverflow)?;
    let end = row_offset
        .checked_add(row_len)
        .ok_or(FrameApplicationError::OffsetOverflow)?;
    if fallthrough != end {
        return Err(FrameApplicationError::BranchFallthroughMismatch(
            instruction,
        ));
    }
    branch.when_taken_offset = taken;
    branch.when_fallthrough_offset = fallthrough;
    branch.byte_displacement = checked_delta(
        taken,
        match architecture {
            Architecture::X86_64 => end,
            Architecture::Aarch64 => row_offset,
        },
    )?;
    Ok(())
}

fn checked_delta(target: u64, reference: u64) -> Result<i64, FrameApplicationError> {
    let target = i128::from(target);
    let reference = i128::from(reference);
    i64::try_from(target - reference).map_err(|_| FrameApplicationError::OffsetOverflow)
}

fn widen_out_of_range_branches(
    function: &mut FunctionFragment,
    epilogues: &mut [FunctionAppliedFrameEpilogue],
    architecture: Architecture,
    controls: &EncodedControlIndex<'_>,
) -> Result<(), FrameApplicationError> {
    loop {
        let rows = out_of_reach_short_rows(function, architecture)?;
        if rows.is_empty() {
            return Ok(());
        }
        widen(function, epilogues, &rows, controls)?;
    }
}

fn out_of_reach_short_rows(
    function: &FunctionFragment,
    architecture: Architecture,
) -> Result<Vec<(usize, usize)>, FrameApplicationError> {
    let block_offsets = function
        .blocks
        .iter()
        .map(|block| (block.block, block.offset))
        .collect::<BTreeMap<_, _>>();
    let mut rows = Vec::new();
    for (block_index, block) in function.blocks.iter().enumerate() {
        for (row_index, row) in block.instructions.iter().enumerate() {
            let Some(FunctionFragmentBranchEvidence::Conditional(branch)) = row.branch.as_deref()
            else {
                continue;
            };
            let Some(form) = short_conditional_form(architecture, branch) else {
                continue;
            };
            if u64::try_from(row.bytes.len()).ok() != Some(form.width) {
                continue;
            }
            let target = block_offsets.get(&branch.when_taken_block).copied().ok_or(
                FrameApplicationError::MissingTargetBlock(branch.when_taken_block),
            )?;
            if !form.reaches(row.offset, target) {
                rows.push((block_index, row_index));
            }
        }
    }
    Ok(rows)
}

fn widen(
    function: &mut FunctionFragment,
    epilogues: &mut [FunctionAppliedFrameEpilogue],
    rows: &[(usize, usize)],
    controls: &EncodedControlIndex<'_>,
) -> Result<(), FrameApplicationError> {
    let growth =
        usize::try_from(WIDENING_GROWTH).map_err(|_| FrameApplicationError::OffsetOverflow)?;
    let mut insertions = Vec::with_capacity(rows.len());
    for &(block_index, row_index) in rows {
        let block = &mut function.blocks[block_index];
        let row = &mut block.instructions[row_index];
        let short_width =
            u64::try_from(row.bytes.len()).map_err(|_| FrameApplicationError::OffsetOverflow)?;
        insertions.push(
            row.offset
                .checked_add(short_width)
                .ok_or(FrameApplicationError::OffsetOverflow)?,
        );
        let forms = controls
            .get(row.instruction)
            .ok_or(FrameApplicationError::ArtifactMismatch)?;
        let current = forms
            .matching(row.alternative, row.bytes.len())
            .ok_or(FrameApplicationError::ArtifactMismatch)?;
        let widened = if forms.shortened.as_ref() == Some(current) {
            &forms.baseline
        } else if &forms.baseline == current {
            forms
                .widened
                .as_ref()
                .ok_or(FrameApplicationError::ArtifactMismatch)?
        } else {
            return Err(FrameApplicationError::ArtifactMismatch);
        };
        if widened.bytes.len() != row.bytes.len() + growth {
            return Err(FrameApplicationError::ArtifactMismatch);
        }
        row.alternative = widened.alternative;
        row.bytes.resize(widened.bytes.len(), 0);
        block.byte_count = grow(block.byte_count, 1)?;
    }
    if insertions.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(FrameApplicationError::ArtifactMismatch);
    }

    let mut bytes = Vec::with_capacity(function.bytes.len() + rows.len() * growth);
    let mut cursor = 0_usize;
    for &insertion in &insertions {
        let position =
            usize::try_from(insertion).map_err(|_| FrameApplicationError::OffsetOverflow)?;
        bytes.extend_from_slice(
            function
                .bytes
                .get(cursor..position)
                .ok_or(FrameApplicationError::ArtifactMismatch)?,
        );
        bytes.resize(bytes.len() + growth, 0);
        cursor = position;
    }
    bytes.extend_from_slice(&function.bytes[cursor..]);
    function.bytes = bytes;
    function.byte_count = grow(function.byte_count, insertions.len())?;

    let shift = |offset: u64| {
        grow(
            offset,
            insertions.partition_point(|&insertion| insertion <= offset),
        )
    };
    for block in &mut function.blocks {
        block.offset = shift(block.offset)?;
        for row in &mut block.instructions {
            let shifted = shift(row.offset)?;
            let delta = shifted - row.offset;
            row.offset = shifted;
            if delta != 0 {
                shift_row_fixups(row, delta)?;
            }
        }
    }
    for epilogue in epilogues {
        epilogue.function_offset = shift(epilogue.function_offset)?;
    }
    Ok(())
}

fn grow(offset: u64, count: usize) -> Result<u64, FrameApplicationError> {
    u64::try_from(count)
        .ok()
        .and_then(|count| count.checked_mul(WIDENING_GROWTH))
        .and_then(|growth| offset.checked_add(growth))
        .ok_or(FrameApplicationError::OffsetOverflow)
}

pub(crate) fn validate_frame_protocol_application(
    source: &FunctionFragmentEmissionPlan,
    source_manifest: FunctionFragmentEmissionManifestIdentity,
    protocol: &TargetFrameProtocolEncodingPlan,
    physical: &ValidatedPhysicalRegisterModel,
    candidate: &FunctionFragmentFrameApplication,
) -> Result<(), FrameApplicationError> {
    if source.target != protocol.target {
        return Err(FrameApplicationError::RootMismatch);
    }
    if candidate.source_fragment_manifest != source_manifest
        || candidate.source_fragments != source.identity
        || candidate.frame_protocol != target_frame_protocol_encoding_identity(protocol)
        || candidate.functions.len() != source.functions.len()
        || candidate.fragments.identity != candidate.fragments.recomputed_identity()
        || candidate.identity != candidate.recomputed_identity()
        || !same_plan_roots(source, &candidate.fragments)
    {
        return Err(FrameApplicationError::ArtifactMismatch);
    }
    for ((source_function, candidate_function), application) in source
        .functions
        .iter()
        .zip(&candidate.fragments.functions)
        .zip(&candidate.functions)
    {
        let protocol_row = unique_protocol(protocol, source_function.machine)?;
        let prologue = protocol_row.prologue.bytes(&protocol.bytes).ok_or(
            FrameApplicationError::InvalidProtocolSpan(source_function.machine),
        )?;
        let epilogue = protocol_row.epilogue.bytes(&protocol.bytes).ok_or(
            FrameApplicationError::InvalidProtocolSpan(source_function.machine),
        )?;
        validate_function(
            source_function,
            candidate_function,
            application,
            prologue,
            epilogue,
            source.target.architecture,
            physical,
        )?;
    }
    if protocol.functions.len() != source.functions.len() {
        return Err(FrameApplicationError::FunctionRosterMismatch);
    }
    Ok(())
}

fn same_plan_roots(
    source: &FunctionFragmentEmissionPlan,
    candidate: &FunctionFragmentEmissionPlan,
) -> bool {
    source.psi == candidate.psi
        && source.work_schedule == candidate.work_schedule
        && source.selected == candidate.selected
        && source.target == candidate.target
        && source.entry == candidate.entry
        && source.functions.len() == candidate.functions.len()
}

fn unique_protocol(
    protocol: &TargetFrameProtocolEncodingPlan,
    machine: MachineId,
) -> Result<&FunctionTargetFrameProtocolEncoding, FrameApplicationError> {
    let mut rows = protocol
        .functions
        .iter()
        .filter(|row| row.machine == machine);
    let row = rows
        .next()
        .ok_or(FrameApplicationError::MissingFunction(machine))?;
    if rows.next().is_some() {
        return Err(FrameApplicationError::FunctionRosterMismatch);
    }
    Ok(row)
}

#[allow(clippy::too_many_arguments)]
fn validate_function(
    source: &FunctionFragment,
    candidate: &FunctionFragment,
    application: &FunctionAppliedFrameProtocol,
    prologue: &[u8],
    epilogue: &[u8],
    architecture: Architecture,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), FrameApplicationError> {
    validate_widths(
        source,
        candidate,
        prologue.len(),
        epilogue.len(),
        architecture,
    )?;
    if prologue.is_empty() != epilogue.is_empty()
        || source.machine != candidate.machine
        || source.machine != application.machine
        || source.attachment != candidate.attachment
        || source.provenance != candidate.provenance
        || source.blocks.len() != candidate.blocks.len()
        || application.prologue_function_offset != 0
        || application.prologue_byte_count != prologue.len() as u64
        || candidate.byte_count != candidate.bytes.len() as u64
        || candidate.bytes.get(..prologue.len()) != Some(prologue)
    {
        return Err(FrameApplicationError::ArtifactMismatch);
    }
    let block_offsets = candidate
        .blocks
        .iter()
        .map(|block| (block.block, block.offset))
        .collect::<BTreeMap<_, _>>();
    let mut expected_epilogues = Vec::new();
    let mut cursor = prologue.len() as u64;
    for (source_block, candidate_block) in source.blocks.iter().zip(&candidate.blocks) {
        if source_block.block != candidate_block.block
            || source_block.instructions.len() != candidate_block.instructions.len()
            || candidate_block.offset != cursor
        {
            return Err(FrameApplicationError::ArtifactMismatch);
        }
        let block_start = cursor;
        for (index, (source_row, candidate_row)) in source_block
            .instructions
            .iter()
            .zip(&candidate_block.instructions)
            .enumerate()
        {
            if source_row.instruction != candidate_row.instruction
                || !same_or_widened_alternative(source_row, candidate_row, architecture)
                || source_row.provenance != candidate_row.provenance
                || source_row.control != candidate_row.control
            {
                return Err(FrameApplicationError::ArtifactMismatch);
            }
            if matches!(
                source_row.control,
                FunctionFragmentControlProvenance::HostedExitProcess { .. }
                    | FunctionFragmentControlProvenance::OperationTrap { .. }
                    | FunctionFragmentControlProvenance::EliminateEmpty { .. }
                    | FunctionFragmentControlProvenance::Crash { .. }
                    | FunctionFragmentControlProvenance::InternalCall {
                        owner: target_operations::CallSiteOwner::InvokeNoNormal(_),
                        ..
                    }
            ) && (index + 1 != source_block.instructions.len() || source_row.bytes.is_empty())
            {
                return Err(FrameApplicationError::MissingFinalReturn(source.machine));
            }
            if let FunctionFragmentControlProvenance::Return { psi_return_edge } =
                source_row.control
            {
                if index + 1 != source_block.instructions.len() {
                    return Err(FrameApplicationError::MissingFinalReturn(source.machine));
                }
                check_bytes(&candidate.bytes, cursor, epilogue)?;
                expected_epilogues.push(FunctionAppliedFrameEpilogue {
                    block: source_block.block,
                    return_instruction: source_row.instruction,
                    psi_return_edge,
                    function_offset: cursor,
                    byte_count: epilogue.len() as u64,
                });
                cursor = advance(cursor, epilogue.len())?;
            }
            if candidate_row.offset != cursor {
                return Err(FrameApplicationError::ArtifactMismatch);
            }
            check_bytes(&candidate.bytes, cursor, &candidate_row.bytes)?;
            if let Some(branch) = source_row.branch.as_deref() {
                validate_branch_row(
                    branch,
                    candidate_row,
                    &candidate_row.bytes,
                    architecture,
                    physical,
                    |block| block_offsets.get(&block).copied(),
                )?;
            } else if candidate_row.branch.is_some() || candidate_row.bytes != source_row.bytes {
                return Err(FrameApplicationError::ArtifactMismatch);
            }
            let shift = candidate_row
                .offset
                .checked_sub(source_row.offset)
                .ok_or(FrameApplicationError::ArtifactMismatch)?;
            validate_shifted_fixup(
                source_row.internal_machine_fixup(),
                candidate_row.internal_machine_fixup(),
                shift,
                shift_fixup,
            )?;
            validate_shifted_fixup(
                source_row.normalized_foreign_call_fixup().cloned(),
                candidate_row.normalized_foreign_call_fixup().cloned(),
                shift,
                shift_foreign_fixup,
            )?;
            let expected_address = source_row
                .image_address_fixup()
                .map(|fixup| {
                    fixup
                        .fields
                        .shifted(shift)
                        .map(|fields| machine_code::FunctionFragmentImageAddressFixup {
                            fields,
                            ..fixup
                        })
                        .ok_or(FrameApplicationError::OffsetOverflow)
                })
                .transpose()?;
            if candidate_row.image_address_fixup() != expected_address {
                return Err(FrameApplicationError::ArtifactMismatch);
            }
            cursor = advance(cursor, candidate_row.bytes.len())?;
        }
        if candidate_block.byte_count != cursor - block_start {
            return Err(FrameApplicationError::ArtifactMismatch);
        }
    }
    if application.epilogues != expected_epilogues || cursor != candidate.byte_count {
        return Err(FrameApplicationError::ArtifactMismatch);
    }
    Ok(())
}

fn validate_shifted_fixup<F>(
    source: Option<F>,
    candidate: Option<F>,
    shift: u64,
    shift_fixup: impl Fn(&mut F, u64) -> Result<(), FrameApplicationError>,
) -> Result<(), FrameApplicationError>
where
    F: PartialEq,
{
    let Some(mut expected) = source else {
        return if candidate.is_none() {
            Ok(())
        } else {
            Err(FrameApplicationError::ArtifactMismatch)
        };
    };
    shift_fixup(&mut expected, shift)?;
    if candidate != Some(expected) {
        return Err(FrameApplicationError::ArtifactMismatch);
    }
    Ok(())
}

fn check_bytes(bytes: &[u8], offset: u64, expected: &[u8]) -> Result<(), FrameApplicationError> {
    let start = usize::try_from(offset).map_err(|_| FrameApplicationError::OffsetOverflow)?;
    let end = start
        .checked_add(expected.len())
        .ok_or(FrameApplicationError::OffsetOverflow)?;
    if bytes.get(start..end) != Some(expected) {
        return Err(FrameApplicationError::ArtifactMismatch);
    }
    Ok(())
}

fn advance(offset: u64, length: usize) -> Result<u64, FrameApplicationError> {
    offset
        .checked_add(u64::try_from(length).map_err(|_| FrameApplicationError::OffsetOverflow)?)
        .ok_or(FrameApplicationError::OffsetOverflow)
}

fn validate_widths(
    source: &FunctionFragment,
    candidate: &FunctionFragment,
    prologue: usize,
    epilogue: usize,
    architecture: Architecture,
) -> Result<(), FrameApplicationError> {
    let mut widths = source
        .blocks
        .iter()
        .map(|block| {
            block
                .instructions
                .iter()
                .map(|row| row.bytes.len())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let short_count = source
        .instructions()
        .filter(|row| match row.branch.as_deref() {
            Some(FunctionFragmentBranchEvidence::Conditional(branch)) => {
                short_conditional_form(architecture, branch)
                    .is_some_and(|form| row.bytes.len() as u64 == form.width)
            }
            _ => false,
        })
        .count();
    for _ in 0..=short_count {
        let mut cursor = prologue as u64;
        let mut blocks = BTreeMap::new();
        let mut offsets = Vec::new();
        for (block, row_widths) in source.blocks.iter().zip(&widths) {
            if blocks.insert(block.block, cursor).is_some() {
                return Err(FrameApplicationError::ArtifactMismatch);
            }
            let mut rows = Vec::new();
            for (row, width) in block.instructions.iter().zip(row_widths) {
                if matches!(
                    row.control,
                    FunctionFragmentControlProvenance::Return { .. }
                ) {
                    cursor = cursor
                        .checked_add(epilogue as u64)
                        .ok_or(FrameApplicationError::OffsetOverflow)?;
                }
                rows.push(cursor);
                cursor = cursor
                    .checked_add(*width as u64)
                    .ok_or(FrameApplicationError::OffsetOverflow)?;
            }
            offsets.push(rows);
        }
        let mut changed = false;
        for (block_index, block) in source.blocks.iter().enumerate() {
            for (row_index, row) in block.instructions.iter().enumerate() {
                let Some(FunctionFragmentBranchEvidence::Conditional(branch)) =
                    row.branch.as_deref()
                else {
                    continue;
                };
                let Some(form) = short_conditional_form(architecture, branch) else {
                    continue;
                };
                if widths[block_index][row_index] as u64 != form.width {
                    continue;
                }
                let target = blocks.get(&branch.when_taken_block).ok_or(
                    FrameApplicationError::MissingTargetBlock(branch.when_taken_block),
                )?;
                if !form.reaches(offsets[block_index][row_index], *target) {
                    widths[block_index][row_index] += WIDENING_GROWTH as usize;
                    changed = true;
                }
            }
        }
        if !changed {
            if source.blocks.len() != candidate.blocks.len()
                || widths.iter().zip(&candidate.blocks).any(|(widths, block)| {
                    widths.len() != block.instructions.len()
                        || widths
                            .iter()
                            .zip(&block.instructions)
                            .any(|(width, row)| *width != row.bytes.len())
                })
            {
                return Err(FrameApplicationError::ArtifactMismatch);
            }
            return Ok(());
        }
    }
    Err(FrameApplicationError::ArtifactMismatch)
}

fn same_or_widened_alternative(
    source: &FunctionFragmentInstructionSpan,
    candidate: &FunctionFragmentInstructionSpan,
    architecture: Architecture,
) -> bool {
    let mut expected = source.alternative;
    if architecture == Architecture::X86_64
        && source.bytes.len() == 2
        && candidate.bytes.len() == 6
        && matches!(
            source.branch.as_deref(),
            Some(FunctionFragmentBranchEvidence::Conditional(_))
        )
        && matches!(
            expected.family,
            MachineAlternativeFamily::ConditionalBranchU64LessThan
                | MachineAlternativeFamily::ConditionalBranchI64LessThan
        )
        && expected.variant == 1
    {
        expected.variant = 0;
    }
    candidate.alternative == expected
}

#[cfg(test)]
mod tests {
    use super::{
        FrameApplicationError, FunctionFragment, FunctionFragmentBranchEvidence,
        FunctionFragmentConditionalBranchEvidence, FunctionFragmentControlProvenance,
        FunctionFragmentEmissionPlan, FunctionFragmentFrameApplication,
        FunctionFragmentInstructionSpan, FunctionFragmentInternalMachineFixup,
        TargetFrameProtocolEncodingPlan, validate_frame_protocol_application,
    };
    use encoding_admission::frame_protocol::{
        FrameProtocolByteSpan, FunctionTargetFrameProtocolEncoding,
        TargetFrameProtocolEncodingPolicy,
    };
    use isa_aarch64::AARCH64_CONDITIONAL_BRANCH_BYTE_REACH;
    use machine_code::{
        FunctionFragmentBlockSpan, FunctionFragmentConditionalBranchPredicate,
        FunctionFragmentInternalMachineFixupKind, FunctionFragmentInternalMachineFixupState,
        TargetFrameLayoutIdentity,
    };
    use optimization_core::{
        FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
    };
    use register_model::{
        PhysicalRegisterModelIdentity, TargetRegisterEnvironmentIdentity,
        validate_physical_register_model,
    };
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, SelectedBlockId, SelectedInstructionId,
        SelectedInstructionPlanIdentity, SelectedInstructionProvenance,
    };
    use semantic_vocabulary::{EdgeId, MachineId, WorkScheduleIdentity};
    use target::NativeTarget;
    use target_operations::TerminalPsiProvenance;
    use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    fn apply_frame_protocol_to_fragments(
        source: &FunctionFragmentEmissionPlan,
        manifest: FunctionFragmentEmissionManifestIdentity,
        protocol: &TargetFrameProtocolEncodingPlan,
        physical: &register_model::ValidatedPhysicalRegisterModel,
    ) -> Result<FunctionFragmentFrameApplication, FrameApplicationError> {
        use machine_code::{
            BranchDisplacementField as Field, EncodedBranchAlternative, EncodedControlAlternatives,
            SelectedFormDecodedFootprint,
        };
        macro_rules! form {
            ($encoded:expr, $key:expr, $field:expr, $offset:expr, $reference:expr) => {{
                let encoded = $encoded.unwrap();
                let fp = encoded.footprint();
                EncodedBranchAlternative {
                    alternative: $key,
                    bytes: encoded.bytes().to_vec(),
                    field: $field,
                    field_offset: $offset,
                    reference_offset: $reference,
                    footprint: SelectedFormDecodedFootprint {
                        register_reads: fp.register_reads.clone(),
                        register_writes: fp.register_writes.clone(),
                        implicit_defs: fp.encoded.implicit_unit_defs.clone(),
                        implicit_clobbers: fp.encoded.implicit_unit_clobbers.clone(),
                        encoded: fp.encoded.clone(),
                    },
                }
            }};
        }
        let rows = source.functions.iter().flat_map(|f| f.instructions()).filter_map(|row| {
            let branch = row.branch.as_deref()?;
            let mut key = row.alternative;
            let forms = match source.target.architecture {
                target::Architecture::X86_64 if matches!(branch, FunctionFragmentBranchEvidence::Jump(_)) => EncodedControlAlternatives {
                    baseline: form!(isa_x86_64::encode_x86_64_selected_jump_form(physical, key, 0), key, Field::Relative32, 1, 5),
                    widened: None, shortened: None,
                },
                target::Architecture::X86_64 => {
                    let encode = |key: MachineAlternativeKey, short| match key.family {
                        MachineAlternativeFamily::ConditionalBranchNonZero if short => isa_x86_64::encode_x86_64_selected_short_nonzero_branch_form(physical, key, 0),
                        MachineAlternativeFamily::ConditionalBranchNonZero => isa_x86_64::encode_x86_64_selected_nonzero_branch_form(physical, key, 0),
                        MachineAlternativeFamily::ConditionalBranchU64LessThan => isa_x86_64::encode_x86_64_selected_u64_less_than_branch_form(physical, key, 0),
                        MachineAlternativeFamily::ConditionalBranchI64LessThan => isa_x86_64::encode_x86_64_selected_i64_less_than_branch_form(physical, key, 0),
                        _ => panic!("fixture requires a conditional branch"),
                    };
                    if key.family != MachineAlternativeFamily::ConditionalBranchNonZero { key.variant = 0; }
                    let short = MachineAlternativeKey { variant: u32::from(key.family != MachineAlternativeFamily::ConditionalBranchNonZero), ..key };
                    EncodedControlAlternatives {
                        baseline: form!(encode(key, false), key, Field::Relative32, 2, 6),
                        shortened: Some(form!(encode(short, true), short, Field::Relative8, 1, 2)), widened: None,
                    }
                }
                target::Architecture::Aarch64 if matches!(branch, FunctionFragmentBranchEvidence::Jump(_)) => EncodedControlAlternatives {
                    baseline: form!(isa_aarch64::encode_aarch64_selected_jump_form(physical, key, 0), key, Field::Aarch64Branch26, 0, 0),
                    widened: None, shortened: None,
                },
                target::Architecture::Aarch64 => {
                    let FunctionFragmentBranchEvidence::Conditional(branch) = branch else { unreachable!() };
                    if branch.decoded_register_reads.len() == 1 {
                        EncodedControlAlternatives {
                            baseline: form!(isa_aarch64::encode_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(physical,
                                branch.decoded_register_reads[0], 0), key, Field::Aarch64Conditional19, 0, 0),
                            widened: None, shortened: None,
                        }
                    } else {
                        EncodedControlAlternatives {
                            baseline: form!(isa_aarch64::encode_aarch64_selected_conditional_branch_form(physical, key, key.family,
                                isa_aarch64::Aarch64ConditionalBranchForm::CanonicalWordV1, 0), key, Field::Aarch64Conditional19, 0, 0),
                            widened: Some(form!(isa_aarch64::encode_aarch64_selected_conditional_branch_form(physical, key, key.family,
                                isa_aarch64::Aarch64ConditionalBranchForm::WidenedPairV1, 4), key, Field::Aarch64Branch26, 4, 4)), shortened: None,
                        }
                    }
                }
            };
            Some(machine_code::SelectedFormEncodingRow {
                instruction: row.instruction, alternative: forms.baseline.alternative,
                machine_disposition: machine_code::SelectedFormMachineDisposition::RetainedV1,
                state: machine_code::SelectedFormEncodingState::DeferredControl {
                    reason: machine_code::DeferredControlEncodingReason::RequiresResolvedBranchLayout,
                    alternatives: Box::new(forms),
                }, address: None, code: Default::default(), footprint: Default::default(),
            })
        }).collect::<Vec<_>>();
        let controls = machine_code::EncodedControlIndex::partition(
            &rows,
            source.functions.iter().map(|function| {
                function
                    .instructions()
                    .filter(|row| row.branch.is_some())
                    .count()
            }),
        )
        .unwrap();
        super::apply_frame_protocol_to_fragments(source, manifest, protocol, &controls)
    }

    fn source_plan() -> FunctionFragmentEmissionPlan {
        let machine = MachineId::new(1).unwrap();
        let callee = MachineId::new(2).unwrap();
        let call = FunctionFragmentInstructionSpan {
            instruction: SelectedInstructionId(1),
            alternative: MachineAlternativeKey {
                family: MachineAlternativeFamily::CallScalar,
                variant: 0,
            },
            offset: 0,
            bytes: vec![0xe8, 0, 0, 0, 0],
            branch: None,
            internal_machine_fixup: Some(Box::new(FunctionFragmentInternalMachineFixup {
                kind: FunctionFragmentInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1,
                state: FunctionFragmentInternalMachineFixupState::UnresolvedZeroFieldV1,
                callee,
                opcode_function_offset: 0,
                patch_function_offset: 1,
                reference_function_offset: 5,
                patch_byte_width: 4,
                addend: 0,
            })),
            normalized_foreign_call_fixup: None,
            image_address_fixup: None,
            provenance: SelectedInstructionProvenance::default(),
            control: FunctionFragmentControlProvenance::InternalCall {
                target: selected_instructions::SelectedCallTarget::Machine(callee),
                owner: target_operations::CallSiteOwner::Operation(
                    semantic_vocabulary::OperationId::new(1).unwrap(),
                ),
            },
        };
        let ret = FunctionFragmentInstructionSpan {
            instruction: SelectedInstructionId(2),
            alternative: MachineAlternativeKey {
                family: MachineAlternativeFamily::ReturnScalar,
                variant: 0,
            },
            offset: 5,
            bytes: vec![0xc3],
            branch: None,
            internal_machine_fixup: None,
            normalized_foreign_call_fixup: None,
            image_address_fixup: None,
            provenance: SelectedInstructionProvenance::default(),
            control: FunctionFragmentControlProvenance::Return {
                psi_return_edge: EdgeId::new(1).unwrap(),
            },
        };
        let mut plan = FunctionFragmentEmissionPlan {
            identity: FunctionFragmentEmissionIdentity::from_canonical_bytes(b"pending"),
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([3; 32]),
            },
            work_schedule: WorkScheduleIdentity::new(1).unwrap(),
            selected: SelectedInstructionPlanIdentity::from_canonical_bytes(b"selected"),
            target: NativeTarget::linux_x64(),
            entry: machine,
            functions: vec![FunctionFragment {
                machine,
                attachment: None,
                provenance: TerminalPsiProvenance::default(),
                byte_count: 6,
                bytes: vec![0xe8, 0, 0, 0, 0, 0xc3],
                blocks: vec![FunctionFragmentBlockSpan {
                    block: SelectedBlockId(1),
                    offset: 0,
                    byte_count: 6,
                    instructions: vec![call, ret],
                }],
            }],
        };
        plan.identity = plan.recomputed_identity();
        plan
    }

    fn protocol(machine: MachineId) -> TargetFrameProtocolEncodingPlan {
        TargetFrameProtocolEncodingPlan {
            frame_layout: TargetFrameLayoutIdentity::from_bytes([4; 32]),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([5; 32]),
            physical_register_model: PhysicalRegisterModelIdentity::from_bytes([6; 32]),
            target: NativeTarget::linux_x64(),
            policy: TargetFrameProtocolEncodingPolicy::CanonicalFixedFrameV1,
            functions: vec![FunctionTargetFrameProtocolEncoding {
                machine,
                prologue: FrameProtocolByteSpan {
                    offset: 0,
                    length: 1,
                },
                epilogue: FrameProtocolByteSpan {
                    offset: 1,
                    length: 1,
                },
            }],
            bytes: vec![0xaa, 0xbb],
        }
    }

    fn physical() -> register_model::ValidatedPhysicalRegisterModel {
        validate_physical_register_model(isa_x86_64::x86_64_physical_register_model()).unwrap()
    }

    #[test]
    fn frame_bytes_shift_selected_rows_and_internal_call_fixups_exactly() {
        let source = source_plan();
        let machine = source.entry;
        let application = apply_frame_protocol_to_fragments(
            &source,
            FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(b"manifest"),
            &protocol(machine),
            &physical(),
        )
        .unwrap();
        let function = &application.fragments.functions[0];
        assert_eq!(function.bytes, vec![0xaa, 0xe8, 0, 0, 0, 0, 0xbb, 0xc3]);
        assert_eq!(function.byte_count, 8);
        assert_eq!(function.blocks[0].offset, 1);
        assert_eq!(function.blocks[0].byte_count, 7);
        assert_eq!(function.blocks[0].instructions[0].offset, 1);
        assert_eq!(function.blocks[0].instructions[1].offset, 7);
        let fixup = function.blocks[0].instructions[0]
            .internal_machine_fixup()
            .unwrap();
        assert_eq!(fixup.opcode_function_offset, 1);
        assert_eq!(fixup.patch_function_offset, 2);
        assert_eq!(fixup.reference_function_offset, 6);
        assert_eq!(application.functions[0].prologue_function_offset, 0);
        assert_eq!(application.functions[0].epilogues.len(), 1);
        assert_eq!(application.functions[0].epilogues[0].function_offset, 6);
        assert_eq!(application.functions[0].epilogues[0].byte_count, 1);
        assert_eq!(application.identity, application.recomputed_identity());
        assert_eq!(
            application.fragments.identity,
            application.fragments.recomputed_identity()
        );
    }

    #[test]
    fn crash_keeps_frame_without_return_cleanup_and_rejects_following_instructions() {
        for cause in [
            terminal_psi::CrashCause::Trap,
            terminal_psi::CrashCause::Abort,
        ] {
            let mut source = source_plan();
            let machine = source.entry;
            let row = &mut source.functions[0].blocks[0].instructions[1];
            row.control = FunctionFragmentControlProvenance::Crash {
                psi_edge: EdgeId::new(1).unwrap(),
                cause,
            };
            row.alternative.family = MachineAlternativeFamily::Crash;
            source.identity = source.recomputed_identity();
            let manifest =
                FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(b"manifest");
            let application = apply_frame_protocol_to_fragments(
                &source,
                manifest,
                &protocol(machine),
                &physical(),
            )
            .unwrap();
            assert!(application.functions[0].epilogues.is_empty());
            assert_eq!(
                application.fragments.functions[0].bytes.len(),
                source.functions[0].bytes.len() + 1
            );
            validate_frame_protocol_application(
                &source,
                manifest,
                &protocol(machine),
                &physical(),
                &application,
            )
            .unwrap();
            let mut changed = application.clone();
            changed.fragments.functions[0].blocks[0].instructions[1].control =
                FunctionFragmentControlProvenance::Return {
                    psi_return_edge: EdgeId::new(1).unwrap(),
                };
            changed.fragments.identity = changed.fragments.recomputed_identity();
            changed.identity = changed.recomputed_identity();
            assert!(
                validate_frame_protocol_application(
                    &source,
                    manifest,
                    &protocol(machine),
                    &physical(),
                    &changed,
                )
                .is_err()
            );
            source.functions[0].blocks[0].instructions[0].control =
                FunctionFragmentControlProvenance::Crash {
                    psi_edge: EdgeId::new(2).unwrap(),
                    cause,
                };
            source.identity = source.recomputed_identity();
            assert!(
                apply_frame_protocol_to_fragments(
                    &source,
                    manifest,
                    &protocol(machine),
                    &physical()
                )
                .is_err()
            );
        }
    }

    #[test]
    fn process_exit_keeps_prologue_without_inventing_return_epilogue() {
        let mut source = source_plan();
        let machine = source.entry;
        let row = &mut source.functions[0].blocks[0].instructions[1];
        row.control = FunctionFragmentControlProvenance::HostedExitProcess {
            completion_edge: EdgeId::new(1).unwrap(),
        };
        row.alternative.family = MachineAlternativeFamily::HostedExitProcessI32;
        let manifest = FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(b"manifest");
        let application =
            apply_frame_protocol_to_fragments(&source, manifest, &protocol(machine), &physical())
                .unwrap();
        assert!(application.functions[0].epilogues.is_empty());
        assert_eq!(
            application.fragments.functions[0].bytes,
            vec![0xaa, 0xe8, 0, 0, 0, 0, 0xc3]
        );
        validate_frame_protocol_application(
            &source,
            manifest,
            &protocol(machine),
            &physical(),
            &application,
        )
        .unwrap();
        let mut changed = application.clone();
        changed.fragments.functions[0].blocks[0].instructions[1].control =
            FunctionFragmentControlProvenance::HostedExitProcess {
                completion_edge: EdgeId::new(2).unwrap(),
            };
        assert!(
            validate_frame_protocol_application(
                &source,
                manifest,
                &protocol(machine),
                &physical(),
                &changed,
            )
            .is_err()
        );
        source.functions[0].blocks[0].instructions[0].control =
            FunctionFragmentControlProvenance::HostedExitProcess {
                completion_edge: EdgeId::new(2).unwrap(),
            };
        assert!(
            apply_frame_protocol_to_fragments(&source, manifest, &protocol(machine), &physical())
                .is_err()
        );
    }

    #[test]
    fn malformed_block_and_nonfinal_return_shapes_fail_closed() {
        let mut source = source_plan();
        let machine = source.entry;
        let repeated = source.functions[0].blocks[0].clone();
        source.functions[0].blocks.push(repeated);
        assert_eq!(
            apply_frame_protocol_to_fragments(
                &source,
                FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(b"manifest"),
                &protocol(machine),
                &physical(),
            ),
            Err(FrameApplicationError::SourceShapeMismatch(machine))
        );

        let mut source = source_plan();
        let controls = &mut source.functions[0].blocks[0].instructions;
        controls[0].control = FunctionFragmentControlProvenance::Return {
            psi_return_edge: EdgeId::new(2).unwrap(),
        };
        controls[1].control = FunctionFragmentControlProvenance::InternalCall {
            target: selected_instructions::SelectedCallTarget::Machine(MachineId::new(2).unwrap()),
            owner: target_operations::CallSiteOwner::Operation(
                semantic_vocabulary::OperationId::new(1).unwrap(),
            ),
        };
        assert_eq!(
            apply_frame_protocol_to_fragments(
                &source,
                FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(b"manifest"),
                &protocol(machine),
                &physical(),
            ),
            Err(FrameApplicationError::MissingFinalReturn(machine))
        );
    }

    #[test]
    fn frame_epilogue_growth_widens_out_of_range_rel8_and_preserves_in_range_rel8() {
        for (predicate, family, short_opcode, near_opcode) in [
            (
                FunctionFragmentConditionalBranchPredicate::NonZeroV1,
                MachineAlternativeFamily::ConditionalBranchNonZero,
                0x75,
                0x85,
            ),
            (
                FunctionFragmentConditionalBranchPredicate::U64LessThanV1,
                MachineAlternativeFamily::ConditionalBranchU64LessThan,
                0x72,
                0x82,
            ),
            (
                FunctionFragmentConditionalBranchPredicate::I64LessThanV1,
                MachineAlternativeFamily::ConditionalBranchI64LessThan,
                0x7c,
                0x8c,
            ),
        ] {
            for (padding, expected_width, semantic) in [
                (126, 6, true),
                (125, 2, true),
                (126, 6, false),
                (125, 2, false),
            ] {
                let mut source = source_plan();
                let physical = physical();
                let alternative = MachineAlternativeKey {
                    family,
                    variant: u32::from(
                        predicate != FunctionFragmentConditionalBranchPredicate::NonZeroV1,
                    ),
                };
                let encoded = match predicate {
                    FunctionFragmentConditionalBranchPredicate::NonZeroV1 => {
                        isa_x86_64::encode_x86_64_selected_short_nonzero_branch_form(
                            &physical,
                            alternative,
                            padding + 1,
                        )
                    }
                    FunctionFragmentConditionalBranchPredicate::U64LessThanV1 => {
                        isa_x86_64::encode_x86_64_selected_u64_less_than_branch_form(
                            &physical,
                            alternative,
                            padding + 1,
                        )
                    }
                    FunctionFragmentConditionalBranchPredicate::I64LessThanV1 => {
                        isa_x86_64::encode_x86_64_selected_i64_less_than_branch_form(
                            &physical,
                            alternative,
                            padding + 1,
                        )
                    }
                }
                .unwrap();
                let function = &mut source.functions[0];
                let mut call = function.blocks[0].instructions[0].clone();
                call.instruction = SelectedInstructionId(5);
                call.offset = 2;
                let fixup = call.internal_machine_fixup.as_mut().unwrap();
                fixup.opcode_function_offset += 2;
                fixup.patch_function_offset += 2;
                fixup.reference_function_offset += 2;
                let mut branch = function.blocks[0].instructions[0].clone();
                branch.alternative = alternative;
                branch.bytes = encoded.bytes().to_vec();
                branch.internal_machine_fixup = None;
                branch.control = FunctionFragmentControlProvenance::None;
                branch.branch = Some(Box::new(FunctionFragmentBranchEvidence::Conditional(
                    FunctionFragmentConditionalBranchEvidence {
                        predicate,
                        source_block: SelectedBlockId(1),
                        when_taken_edge: semantic.then(|| EdgeId::new(2).unwrap()),
                        when_taken_block: SelectedBlockId(3),
                        when_taken_offset: (padding + 3) as u64,
                        when_fallthrough_edge: semantic.then(|| EdgeId::new(3).unwrap()),
                        when_fallthrough_block: SelectedBlockId(2),
                        when_fallthrough_offset: 2,
                        byte_displacement: padding + 1,
                        decoded_register_reads: encoded.footprint().register_reads.clone(),
                        decoded_effects: encoded.footprint().encoded.clone(),
                    },
                )));
                let mut filler = branch.clone();
                filler.instruction = SelectedInstructionId(3);
                filler.offset = 7;
                filler.bytes = vec![0x90; padding as usize - 5];
                filler.branch = None;
                let mut first_return = function.blocks[0].instructions[1].clone();
                first_return.offset = (padding + 2) as u64;
                let mut last_return = first_return.clone();
                last_return.instruction = SelectedInstructionId(4);
                last_return.offset += 1;
                function.blocks = vec![
                    FunctionFragmentBlockSpan {
                        block: SelectedBlockId(1),
                        offset: 0,
                        byte_count: 2,
                        instructions: vec![branch],
                    },
                    FunctionFragmentBlockSpan {
                        block: SelectedBlockId(2),
                        offset: 2,
                        byte_count: (padding + 1) as u64,
                        instructions: vec![call, filler, first_return],
                    },
                    FunctionFragmentBlockSpan {
                        block: SelectedBlockId(3),
                        offset: (padding + 3) as u64,
                        byte_count: 1,
                        instructions: vec![last_return],
                    },
                ];
                function.bytes = function
                    .instructions()
                    .flat_map(|row| row.bytes.clone())
                    .collect();
                function.byte_count = function.bytes.len() as u64;
                source.identity = source.recomputed_identity();
                let manifest =
                    FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(b"manifest");
                let protocol = protocol(source.entry);
                let application =
                    apply_frame_protocol_to_fragments(&source, manifest, &protocol, &physical)
                        .unwrap();
                let branch = &application.fragments.functions[0].blocks[0].instructions[0];
                check_published_branch_bytes(
                    &application.fragments,
                    &super::prepare_branch_origins(&source),
                    &physical,
                    source.entry,
                    branch,
                );
                assert_eq!(branch.bytes.len(), expected_width);
                assert_eq!(
                    branch.alternative.variant,
                    if expected_width == 6 {
                        0
                    } else {
                        alternative.variant
                    }
                );
                if expected_width == 6 {
                    assert_eq!(&branch.bytes[..2], &[0x0f, near_opcode]);
                } else {
                    assert_eq!(branch.bytes[0], short_opcode);
                }
                let FunctionFragmentBranchEvidence::Conditional(evidence) =
                    branch.branch.as_deref().unwrap()
                else {
                    panic!("conditional branch evidence");
                };
                assert_eq!(evidence.when_taken_edge.is_some(), semantic);
                assert_eq!(evidence.when_fallthrough_edge.is_some(), semantic);
                let growth = expected_width as u64 - 2;
                let function = &application.fragments.functions[0];
                let call = &function.blocks[1].instructions[0];
                let fixup = call.internal_machine_fixup().unwrap();
                assert_eq!(call.offset, 3 + growth);
                assert_eq!(fixup.opcode_function_offset, 3 + growth);
                assert_eq!(fixup.patch_function_offset, 4 + growth);
                assert_eq!(fixup.reference_function_offset, 8 + growth);
                assert_eq!(
                    application.functions[0].epilogues[0].function_offset,
                    padding as u64 + 3 + growth
                );
                assert_eq!(
                    application.functions[0].epilogues[1].function_offset,
                    padding as u64 + 5 + growth
                );
                assert_eq!(
                    branch
                        .branch
                        .as_deref()
                        .unwrap()
                        .as_conditional()
                        .unwrap()
                        .byte_displacement,
                    padding + 2
                );
                validate_frame_protocol_application(
                    &source,
                    manifest,
                    &protocol,
                    &physical,
                    &application,
                )
                .unwrap();
                let mut corrupted = application.clone();
                corrupted.fragments.functions[0].blocks[0].instructions[0].bytes[1] ^= 1;
                corrupted.fragments.functions[0].bytes[2] ^= 1;
                corrupted.fragments.identity = corrupted.fragments.recomputed_identity();
                corrupted.identity = corrupted.recomputed_identity();
                assert!(
                    validate_frame_protocol_application(
                        &source, manifest, &protocol, &physical, &corrupted,
                    )
                    .is_err()
                );
            }
        }
    }

    struct Aarch64Conditional {
        predicate: FunctionFragmentConditionalBranchPredicate,
        family: MachineAlternativeFamily,
        condition: u32,
        inverse: u32,
    }

    const AARCH64_CONDITIONALS: [Aarch64Conditional; 3] = [
        Aarch64Conditional {
            predicate: FunctionFragmentConditionalBranchPredicate::NonZeroV1,
            family: MachineAlternativeFamily::ConditionalBranchNonZero,
            condition: 0x1,
            inverse: 0x0,
        },
        Aarch64Conditional {
            predicate: FunctionFragmentConditionalBranchPredicate::U64LessThanV1,
            family: MachineAlternativeFamily::ConditionalBranchU64LessThan,
            condition: 0x3,
            inverse: 0x2,
        },
        Aarch64Conditional {
            predicate: FunctionFragmentConditionalBranchPredicate::I64LessThanV1,
            family: MachineAlternativeFamily::ConditionalBranchI64LessThan,
            condition: 0xb,
            inverse: 0xa,
        },
    ];

    const MEBIBYTE: u64 = 1 << 20;
    const AARCH64_RET: u32 = 0xd65f_03c0;
    const AARCH64_NOP: u32 = 0xd503_201f;

    fn aarch64_row(
        instruction: u32,
        family: MachineAlternativeFamily,
        bytes: Vec<u8>,
        control: FunctionFragmentControlProvenance,
    ) -> FunctionFragmentInstructionSpan {
        FunctionFragmentInstructionSpan {
            instruction: SelectedInstructionId(instruction),
            alternative: MachineAlternativeKey { family, variant: 0 },
            offset: 0,
            bytes,
            branch: None,
            internal_machine_fixup: None,
            normalized_foreign_call_fixup: None,
            image_address_fixup: None,
            provenance: SelectedInstructionProvenance::default(),
            control,
        }
    }

    fn aarch64_source(
        conditional: &Aarch64Conditional,
        branch_width: usize,
        filler: u64,
        backward: bool,
    ) -> FunctionFragmentEmissionPlan {
        let callee = MachineId::new(2).unwrap();
        let mut call = aarch64_row(
            10,
            MachineAlternativeFamily::CallScalar,
            0x9400_0000_u32.to_le_bytes().to_vec(),
            FunctionFragmentControlProvenance::InternalCall {
                target: selected_instructions::SelectedCallTarget::Machine(callee),
                owner: target_operations::CallSiteOwner::Operation(
                    semantic_vocabulary::OperationId::new(1).unwrap(),
                ),
            },
        );
        call.internal_machine_fixup = Some(Box::new(FunctionFragmentInternalMachineFixup {
            kind: FunctionFragmentInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1,
            state: FunctionFragmentInternalMachineFixupState::UnresolvedZeroFieldV1,
            callee,
            opcode_function_offset: 0,
            patch_function_offset: 0,
            reference_function_offset: 0,
            patch_byte_width: 4,
            addend: 0,
        }));
        let branch = aarch64_row(
            1,
            conditional.family,
            vec![0; branch_width],
            FunctionFragmentControlProvenance::None,
        );
        let straight_line = aarch64_row(
            2,
            MachineAlternativeFamily::CopyI64,
            AARCH64_NOP.to_le_bytes().repeat(filler as usize / 4),
            FunctionFragmentControlProvenance::None,
        );
        let returns = [(3, 1), (4, 4)].map(|(instruction, edge)| {
            aarch64_row(
                instruction,
                MachineAlternativeFamily::ReturnScalar,
                AARCH64_RET.to_le_bytes().to_vec(),
                FunctionFragmentControlProvenance::Return {
                    psi_return_edge: EdgeId::new(edge).unwrap(),
                },
            )
        });
        let [first_return, last_return] = returns;
        let (rosters, branch_block, taken, fallthrough) = if backward {
            (
                [
                    vec![straight_line, first_return],
                    vec![branch],
                    vec![call, last_return],
                ],
                2,
                1,
                3,
            )
        } else {
            (
                [
                    vec![branch],
                    vec![call, straight_line, first_return],
                    vec![last_return],
                ],
                1,
                3,
                2,
            )
        };

        let mut source = source_plan();
        source.target = NativeTarget::macos_arm64();
        let function = &mut source.functions[0];
        let mut offset = 0_u64;
        function.blocks = rosters
            .into_iter()
            .zip(1..)
            .map(|(mut rows, block)| {
                let start = offset;
                for row in &mut rows {
                    row.offset = offset;
                    if let Some(fixup) = &mut row.internal_machine_fixup {
                        fixup.opcode_function_offset = offset;
                        fixup.patch_function_offset = offset;
                        fixup.reference_function_offset = offset;
                    }
                    offset += row.bytes.len() as u64;
                }
                FunctionFragmentBlockSpan {
                    block: SelectedBlockId(block),
                    offset: start,
                    byte_count: offset - start,
                    instructions: rows,
                }
            })
            .collect();
        let block_offset = |blocks: &[FunctionFragmentBlockSpan], block: u32| {
            blocks
                .iter()
                .find(|candidate| candidate.block == SelectedBlockId(block))
                .unwrap()
                .offset
        };
        let taken_offset = block_offset(&function.blocks, taken);
        let fallthrough_offset = block_offset(&function.blocks, fallthrough);
        let row = &mut function.blocks[branch_block as usize - 1].instructions[0];
        let displacement = taken_offset as i64 - row.offset as i64;
        let form = if branch_width == 4 {
            isa_aarch64::Aarch64ConditionalBranchForm::CanonicalWordV1
        } else {
            isa_aarch64::Aarch64ConditionalBranchForm::WidenedPairV1
        };
        let alternative = MachineAlternativeKey {
            family: conditional.family,
            variant: 0,
        };
        let encoded = isa_aarch64::encode_aarch64_selected_conditional_branch_form(
            isa_aarch64::validated_aarch64_physical_register_model(),
            alternative,
            conditional.family,
            form,
            displacement,
        )
        .unwrap();
        row.bytes = encoded.bytes().to_vec();
        row.branch = Some(Box::new(FunctionFragmentBranchEvidence::Conditional(
            FunctionFragmentConditionalBranchEvidence {
                predicate: conditional.predicate,
                source_block: SelectedBlockId(branch_block),
                when_taken_edge: EdgeId::new(2),
                when_taken_block: SelectedBlockId(taken),
                when_taken_offset: taken_offset,
                when_fallthrough_edge: EdgeId::new(3),
                when_fallthrough_block: SelectedBlockId(fallthrough),
                when_fallthrough_offset: fallthrough_offset,
                byte_displacement: displacement,
                decoded_register_reads: encoded.footprint().register_reads.clone(),
                decoded_effects: encoded.footprint().encoded.clone(),
            },
        )));
        function.bytes = function
            .instructions()
            .flat_map(|row| row.bytes.clone())
            .collect();
        function.byte_count = function.bytes.len() as u64;
        source.identity = source.recomputed_identity();
        source
    }

    fn aarch64_protocol(machine: MachineId) -> TargetFrameProtocolEncodingPlan {
        let mut protocol = protocol(machine);
        protocol.target = NativeTarget::macos_arm64();
        protocol.functions[0].prologue = FrameProtocolByteSpan {
            offset: 0,
            length: 4,
        };
        protocol.functions[0].epilogue = FrameProtocolByteSpan {
            offset: 4,
            length: 4,
        };
        protocol.bytes = [0xa9bf_7bfd_u32, 0xa8c1_7bfd]
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect();
        protocol
    }

    fn word_at(bytes: &[u8], offset: u64) -> u32 {
        let start = offset as usize;
        u32::from_le_bytes(bytes[start..start + 4].try_into().unwrap())
    }

    fn decode_conditional(word: u32) -> (u32, i64) {
        assert_eq!(
            word & 0xff00_0010,
            0x5400_0000,
            "{word:#010x} is not B.cond"
        );
        let words = ((((word >> 5) & 0x7ffff) << 13) as i32) >> 13;
        (word & 0xf, i64::from(words) * 4)
    }

    fn decode_unconditional(word: u32) -> i64 {
        assert_eq!(word & 0xfc00_0000, 0x1400_0000, "{word:#010x} is not B");
        i64::from((((word & 0x03ff_ffff) << 6) as i32) >> 6) * 4
    }

    fn apply_and_decode_aarch64_branch(
        source: &FunctionFragmentEmissionPlan,
        conditional: &Aarch64Conditional,
        backward: bool,
    ) -> (usize, FunctionFragmentFrameApplication) {
        let physical = isa_aarch64::validated_aarch64_physical_register_model();
        let manifest = FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(b"manifest");
        let protocol = aarch64_protocol(source.entry);
        let application =
            apply_frame_protocol_to_fragments(source, manifest, &protocol, physical).unwrap();
        let function = &application.fragments.functions[0];
        let (branch_block, taken_block, fallthrough_block) =
            if backward { (1, 0, 2) } else { (0, 2, 1) };
        let branch = &function.blocks[branch_block].instructions[0];
        let taken = function.blocks[taken_block].offset;
        let fallthrough = function.blocks[fallthrough_block].offset;
        let evidence = branch.branch.as_deref().unwrap().as_conditional().unwrap();
        assert_eq!(evidence.when_taken_offset, taken);
        assert_eq!(evidence.when_fallthrough_offset, fallthrough);
        assert_eq!(
            evidence.byte_displacement,
            taken as i64 - branch.offset as i64
        );
        assert_eq!(branch.offset + branch.bytes.len() as u64, fallthrough);
        assert_eq!(
            &function.bytes[branch.offset as usize..fallthrough as usize],
            &branch.bytes[..]
        );
        let first = word_at(&function.bytes, branch.offset);
        match branch.bytes.len() {
            4 => {
                assert!(
                    AARCH64_CONDITIONAL_BRANCH_BYTE_REACH.contains(&evidence.byte_displacement)
                );
                assert_eq!(
                    decode_conditional(first),
                    (conditional.condition, taken as i64 - branch.offset as i64)
                );
            }
            8 => {
                assert!(
                    !AARCH64_CONDITIONAL_BRANCH_BYTE_REACH.contains(&evidence.byte_displacement)
                );
                assert_eq!(decode_conditional(first), (conditional.inverse, 8));
                let far = word_at(&function.bytes, branch.offset + 4);
                assert_eq!(
                    branch.offset as i64 + 4 + decode_unconditional(far),
                    taken as i64
                );
            }
            width => panic!("unexpected AArch64 conditional width {width}"),
        }
        validate_frame_protocol_application(source, manifest, &protocol, physical, &application)
            .unwrap();
        let width = application.fragments.functions[0].blocks[branch_block].instructions[0]
            .bytes
            .len();
        (width, application)
    }

    #[test]
    fn aarch64_frame_growth_widens_conditionals_leaving_imm19_and_keeps_reachable_ones() {
        let cases = [
            (false, MEBIBYTE - 16, 8),
            (false, MEBIBYTE - 20, 4),
            (true, MEBIBYTE - 4, 8),
            (true, MEBIBYTE - 8, 4),
        ];
        for conditional in &AARCH64_CONDITIONALS {
            for (backward, filler, expected_width) in cases {
                let source = aarch64_source(conditional, 4, filler, backward);
                let (width, application) =
                    apply_and_decode_aarch64_branch(&source, conditional, backward);
                let predicate = conditional.predicate;
                assert_eq!(width, expected_width, "{predicate:?} backward={backward}");
                let growth = width as u64 - 4;
                let function = &application.fragments.functions[0];
                let epilogues = &application.functions[0].epilogues;
                let (call, expected_call, expected_epilogues) = if backward {
                    (
                        &function.blocks[2].instructions[0],
                        16 + filler + growth,
                        [4 + filler, 20 + filler + growth],
                    )
                } else {
                    (
                        &function.blocks[1].instructions[0],
                        4 + 4 + growth,
                        [8 + filler + 4 + growth, 16 + filler + 4 + growth],
                    )
                };
                assert_eq!(call.offset, expected_call);
                let fixup = call.internal_machine_fixup().unwrap();
                assert_eq!(fixup.opcode_function_offset, expected_call);
                assert_eq!(fixup.patch_function_offset, expected_call);
                assert_eq!(fixup.reference_function_offset, expected_call);
                assert_eq!(
                    epilogues
                        .iter()
                        .map(|epilogue| epilogue.function_offset)
                        .collect::<Vec<_>>(),
                    expected_epilogues
                );
                assert_eq!(function.byte_count, 28 + filler + growth);
                assert_eq!(function.bytes.len() as u64, function.byte_count);
            }
        }
    }

    #[test]
    fn aarch64_layout_widened_conditional_reencodes_its_far_displacement_through_frame_application()
    {
        for conditional in &AARCH64_CONDITIONALS {
            for backward in [false, true] {
                let source = aarch64_source(conditional, 8, MEBIBYTE, backward);
                let (width, _) = apply_and_decode_aarch64_branch(&source, conditional, backward);
                let predicate = conditional.predicate;
                assert_eq!(width, 8, "{predicate:?} backward={backward}");
            }
        }
    }
    fn check_published_branch_bytes(
        fragments: &FunctionFragmentEmissionPlan,
        origins: &[machine_code::frame_branch_origins::FunctionFrameBranchOrigins],
        physical: &register_model::ValidatedPhysicalRegisterModel,
        machine: MachineId,
        original: &FunctionFragmentInstructionSpan,
    ) {
        let published = |bytes: &[u8]| {
            encoding_admission::relative_branches::validate_published_relative_branches(
                fragments, origins, physical, machine, bytes,
            )
        };
        let mut bytes = fragments.function(machine).unwrap().bytes.clone();
        assert_eq!(published(&bytes).unwrap(), [original.instruction]);
        assert!(published(&bytes[..bytes.len() - 1]).is_err());
        let offset = usize::try_from(original.offset).unwrap();
        for index in offset..offset + original.bytes.len() {
            for bit in 0..8 {
                bytes[index] ^= 1 << bit;
                assert!(
                    published(&bytes).is_err(),
                    "published byte {index}, bit {bit}"
                );
                bytes[index] ^= 1 << bit;
            }
        }
    }

    #[test]
    fn current_branch_readback_refuses_changed_targets_bytes_and_rosters() {
        let source = aarch64_source(&AARCH64_CONDITIONALS[0], 4, 32, false);
        let (_, application) =
            apply_and_decode_aarch64_branch(&source, &AARCH64_CONDITIONALS[0], false);
        let origins = super::prepare_branch_origins(&source);
        let physical = isa_aarch64::validated_aarch64_physical_register_model();
        let fragments = &application.fragments;
        let original = fragments.functions[0]
            .instructions()
            .find(|row| row.branch.is_some())
            .unwrap();
        let check =
            |origins: &[machine_code::frame_branch_origins::FunctionFrameBranchOrigins],
             candidate: &FunctionFragmentInstructionSpan| {
                super::validate_current_relative_branch_rows(
                    fragments,
                    origins,
                    physical,
                    source.entry,
                    [candidate],
                )
            };
        assert!(check(&origins, original).is_ok());
        check_published_branch_bytes(fragments, &origins, physical, source.entry, original);
        let mut changed = original.clone();
        changed.bytes[0] ^= 0x20;
        assert!(check(&origins, &changed).is_err());
        let mut changed = original.clone();
        let FunctionFragmentBranchEvidence::Conditional(branch) =
            changed.branch.as_deref_mut().unwrap()
        else {
            panic!("conditional fixture")
        };
        branch.when_taken_block = branch.when_fallthrough_block;
        assert!(check(&origins, &changed).is_err());
        let mut missing = origins.clone();
        missing[0].branches.clear();
        assert!(check(&missing, original).is_err());
        let mut repeated = origins.clone();
        repeated[0].branches.push(origins[0].branches[0].clone());
        assert!(check(&repeated, original).is_err());
        let mut wrong_origin = origins.clone();
        let FunctionFragmentBranchEvidence::Conditional(branch) =
            &mut wrong_origin[0].branches[0].branch
        else {
            panic!("conditional fixture")
        };
        branch.when_taken_edge = branch.when_fallthrough_edge;
        assert!(check(&wrong_origin, original).is_err());
    }
}
