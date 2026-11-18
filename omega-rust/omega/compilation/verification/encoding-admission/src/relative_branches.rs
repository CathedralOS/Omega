//! Read-only branch admission for frame layout and installed object bytes.

use isa_aarch64::{
    Aarch64ConditionalBranchForm,
    validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form,
    validate_aarch64_selected_conditional_branch_form,
};
use isa_x86_64::{
    validate_x86_64_selected_i64_less_than_branch_form,
    validate_x86_64_selected_nonzero_branch_form,
    validate_x86_64_selected_short_nonzero_branch_form,
    validate_x86_64_selected_u64_less_than_branch_form,
};
use machine_code::{
    FunctionFragmentBranchEvidence, FunctionFragmentConditionalBranchPredicate,
    FunctionFragmentEmissionPlan, FunctionFragmentInstructionSpan,
};
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{MachineAlternativeFamily, SelectedBlockId, SelectedInstructionId};
use semantic_vocabulary::MachineId;
use target::Architecture;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelativeBranchError {
    MissingFunction(MachineId),
    SourceShapeMismatch(MachineId),
    MissingTargetBlock(SelectedBlockId),
    BranchFallthroughMismatch(SelectedInstructionId),
    BranchEffectsMismatch(SelectedInstructionId),
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

impl std::fmt::Display for RelativeBranchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "function-fragment frame application failed: {self:?}")
    }
}

impl std::error::Error for RelativeBranchError {}

/// Validate the carried branch obligations against the current framed bytes.
/// The input contains no earlier fragment plan or executable source graph.
pub fn validate_current_relative_branch_rows<'a>(
    fragments: &FunctionFragmentEmissionPlan,
    origins: &[machine_code::frame_branch_origins::FunctionFrameBranchOrigins],
    physical: &ValidatedPhysicalRegisterModel,
    machine: MachineId,
    candidates: impl IntoIterator<Item = &'a FunctionFragmentInstructionSpan>,
) -> Result<Vec<SelectedInstructionId>, RelativeBranchError> {
    validate_rows(fragments, origins, physical, machine, candidates, |row| {
        Some(&row.bytes)
    })
}

/// Decode each branch from the placed object's function bytes using its frame-relative
/// coordinates, original block targets and admitted register model.
pub fn validate_published_relative_branches(
    fragments: &FunctionFragmentEmissionPlan,
    origins: &[machine_code::frame_branch_origins::FunctionFrameBranchOrigins],
    physical: &ValidatedPhysicalRegisterModel,
    machine: MachineId,
    bytes: &[u8],
) -> Result<Vec<SelectedInstructionId>, RelativeBranchError> {
    let function = fragments
        .function(machine)
        .ok_or(RelativeBranchError::MissingFunction(machine))?;
    if u64::try_from(bytes.len()) != Ok(function.byte_count) {
        return Err(RelativeBranchError::ArtifactMismatch);
    }
    validate_rows(
        fragments,
        origins,
        physical,
        machine,
        function.instructions().filter(|row| row.branch.is_some()),
        |row| {
            let start = usize::try_from(row.offset).ok()?;
            bytes.get(start..start.checked_add(row.bytes.len())?)
        },
    )
}

fn validate_rows<'a, 'bytes>(
    fragments: &FunctionFragmentEmissionPlan,
    origins: &[machine_code::frame_branch_origins::FunctionFrameBranchOrigins],
    physical: &ValidatedPhysicalRegisterModel,
    machine: MachineId,
    candidates: impl IntoIterator<Item = &'a FunctionFragmentInstructionSpan>,
    bytes_for: impl Fn(&'a FunctionFragmentInstructionSpan) -> Option<&'bytes [u8]>,
) -> Result<Vec<SelectedInstructionId>, RelativeBranchError> {
    use RelativeBranchError as Error;
    let mut source = origins
        .iter()
        .filter(|function| function.machine == machine);
    let source_function = source.next().ok_or(Error::MissingFunction(machine))?;
    if source.next().is_some() {
        return Err(Error::SourceShapeMismatch(machine));
    }
    let mut functions = fragments
        .functions
        .iter()
        .filter(|function| function.machine == machine);
    let current = functions.next().ok_or(Error::MissingFunction(machine))?;
    if functions.next().is_some() {
        return Err(Error::SourceShapeMismatch(machine));
    }
    let mut offsets: Vec<_> = current
        .blocks
        .iter()
        .map(|block| (block.block, block.offset))
        .collect();
    offsets.sort_unstable_by_key(|row| row.0);
    if offsets.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(Error::SourceShapeMismatch(machine));
    }
    let mut identities: Vec<_> = source_function
        .branches
        .iter()
        .map(|row| row.instruction)
        .collect();
    identities.sort_unstable();
    if identities.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(Error::SourceShapeMismatch(machine));
    }
    let mut source_rows = source_function.branches.iter();
    let mut candidates = candidates.into_iter();
    let mut checked = Vec::new();
    for retained in current.instructions().filter(|row| row.branch.is_some()) {
        let candidate = candidates
            .next()
            .ok_or(Error::SourceShapeMismatch(machine))?;
        if candidate.instruction != retained.instruction
            || candidate.alternative != retained.alternative
            || candidate.offset != retained.offset
            || candidate.bytes.len() != retained.bytes.len()
            || candidate.provenance != retained.provenance
            || candidate.control != retained.control
            || candidate.internal_machine_fixup != retained.internal_machine_fixup
            || candidate.normalized_foreign_call_fixup != retained.normalized_foreign_call_fixup
            || candidate.image_address_fixup != retained.image_address_fixup
        {
            return Err(Error::ArtifactMismatch);
        }
        let source_row = source_rows
            .next()
            .ok_or(Error::SourceShapeMismatch(machine))?;
        if source_row.instruction != retained.instruction {
            return Err(Error::SourceShapeMismatch(machine));
        }
        let branch = candidate.branch.as_deref().ok_or(Error::ArtifactMismatch)?;
        let inside = |offset| offset < current.byte_count;
        let targets_inside = match branch {
            FunctionFragmentBranchEvidence::Jump(row) => inside(row.target_offset),
            FunctionFragmentBranchEvidence::Conditional(row) => {
                inside(row.when_taken_offset) && inside(row.when_fallthrough_offset)
            }
        };
        if !targets_inside {
            return Err(Error::ArtifactMismatch);
        }
        let bytes = bytes_for(candidate).ok_or(Error::ArtifactMismatch)?;
        validate_branch_row(
            &source_row.branch,
            candidate,
            bytes,
            fragments.target.architecture,
            physical,
            |block| {
                offsets
                    .binary_search_by_key(&block, |row| row.0)
                    .ok()
                    .map(|index| offsets[index].1)
            },
        )?;
        checked.push(candidate.instruction);
    }
    if candidates.next().is_some() || source_rows.next().is_some() {
        return Err(Error::SourceShapeMismatch(machine));
    }
    checked.sort_unstable();
    Ok(checked)
}

const fn conditional_branch_family(
    predicate: FunctionFragmentConditionalBranchPredicate,
) -> MachineAlternativeFamily {
    match predicate {
        FunctionFragmentConditionalBranchPredicate::NonZeroV1 => {
            MachineAlternativeFamily::ConditionalBranchNonZero
        }
        FunctionFragmentConditionalBranchPredicate::U64LessThanV1 => {
            MachineAlternativeFamily::ConditionalBranchU64LessThan
        }
        FunctionFragmentConditionalBranchPredicate::I64LessThanV1 => {
            MachineAlternativeFamily::ConditionalBranchI64LessThan
        }
    }
}

pub fn validate_branch_row(
    source_branch: &FunctionFragmentBranchEvidence,
    candidate: &FunctionFragmentInstructionSpan,
    bytes: &[u8],
    architecture: Architecture,
    physical: &ValidatedPhysicalRegisterModel,
    block_offset: impl Fn(SelectedBlockId) -> Option<u64>,
) -> Result<(), RelativeBranchError> {
    use FunctionFragmentBranchEvidence as Branch;
    let Some(branch) = candidate.branch.as_deref() else {
        return Err(RelativeBranchError::ArtifactMismatch);
    };
    if let (Branch::Jump(source_branch), Branch::Jump(branch)) = (source_branch, branch) {
        if source_branch.source_block != branch.source_block
            || source_branch.target_edge != branch.target_edge
            || source_branch.target_block != branch.target_block
            || source_branch.decoded_effects != branch.decoded_effects
        {
            return Err(RelativeBranchError::ArtifactMismatch);
        }
        let target = block_offset(branch.target_block)
            .ok_or(RelativeBranchError::MissingTargetBlock(branch.target_block))?;
        let reference = match architecture {
            Architecture::X86_64 => candidate
                .offset
                .checked_add(bytes.len() as u64)
                .ok_or(RelativeBranchError::OffsetOverflow)?,
            Architecture::Aarch64 => candidate.offset,
        };
        let displacement = i64::try_from(i128::from(target) - i128::from(reference))
            .map_err(|_| RelativeBranchError::OffsetOverflow)?;
        if branch.target_offset != target || branch.byte_displacement != displacement {
            return Err(RelativeBranchError::ArtifactMismatch);
        }
        let effects = match architecture {
            Architecture::X86_64 => isa_x86_64::validate_x86_64_selected_jump_form(
                physical,
                candidate.alternative,
                displacement,
                bytes,
            )
            .map_err(|error| RelativeBranchError::X86_64Branch(candidate.instruction, error))?
            .footprint()
            .encoded
            .clone(),
            Architecture::Aarch64 => isa_aarch64::validate_aarch64_selected_jump_form(
                physical,
                candidate.alternative,
                displacement,
                bytes,
            )
            .map_err(|error| RelativeBranchError::Aarch64Branch(candidate.instruction, error))?
            .footprint()
            .encoded
            .clone(),
        };
        return if effects == branch.decoded_effects {
            Ok(())
        } else {
            Err(RelativeBranchError::BranchEffectsMismatch(
                candidate.instruction,
            ))
        };
    }
    let (Branch::Conditional(source_branch), Branch::Conditional(branch)) = (source_branch, branch)
    else {
        return Err(RelativeBranchError::ArtifactMismatch);
    };
    if source_branch.predicate != branch.predicate
        || source_branch.source_block != branch.source_block
        || source_branch.when_taken_edge != branch.when_taken_edge
        || source_branch.when_taken_block != branch.when_taken_block
        || source_branch.when_fallthrough_edge != branch.when_fallthrough_edge
        || source_branch.when_fallthrough_block != branch.when_fallthrough_block
        || source_branch.decoded_register_reads != branch.decoded_register_reads
        || source_branch.decoded_effects != branch.decoded_effects
    {
        return Err(RelativeBranchError::ArtifactMismatch);
    }
    let taken = block_offset(branch.when_taken_block).ok_or(
        RelativeBranchError::MissingTargetBlock(branch.when_taken_block),
    )?;
    let fallthrough = block_offset(branch.when_fallthrough_block).ok_or(
        RelativeBranchError::MissingTargetBlock(branch.when_fallthrough_block),
    )?;
    let end = candidate
        .offset
        .checked_add(u64::try_from(bytes.len()).map_err(|_| RelativeBranchError::OffsetOverflow)?)
        .ok_or(RelativeBranchError::OffsetOverflow)?;
    if fallthrough != end {
        return Err(RelativeBranchError::BranchFallthroughMismatch(
            candidate.instruction,
        ));
    }
    let reference = match architecture {
        Architecture::X86_64 => end,
        Architecture::Aarch64 => candidate.offset,
    };
    let displacement = i64::try_from(i128::from(taken) - i128::from(reference))
        .map_err(|_| RelativeBranchError::OffsetOverflow)?;
    if branch.when_taken_offset != taken
        || branch.when_fallthrough_offset != fallthrough
        || branch.byte_displacement != displacement
    {
        return Err(RelativeBranchError::ArtifactMismatch);
    }
    let (reads, effects) =
        decode_branch_row(candidate, bytes, branch.predicate, architecture, physical)?;
    if reads != branch.decoded_register_reads || effects != branch.decoded_effects {
        return Err(RelativeBranchError::BranchEffectsMismatch(
            candidate.instruction,
        ));
    }
    Ok(())
}

fn decode_branch_row(
    row: &FunctionFragmentInstructionSpan,
    bytes: &[u8],
    predicate: FunctionFragmentConditionalBranchPredicate,
    architecture: Architecture,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<
    (
        Vec<register_model::RegisterViewId>,
        selected_instructions::MachineEncodedEffects,
    ),
    RelativeBranchError,
> {
    let instruction = row.instruction;
    let Some(FunctionFragmentBranchEvidence::Conditional(branch)) = row.branch.as_deref() else {
        return Err(RelativeBranchError::ArtifactMismatch);
    };
    macro_rules! footprint {
        ($encoded:expr) => {{
            let encoded = $encoded;
            (
                encoded.footprint().register_reads.clone(),
                encoded.footprint().encoded.clone(),
            )
        }};
    }
    let long = match architecture {
        Architecture::X86_64 => bytes.len() != 2,
        Architecture::Aarch64 => branch.decoded_register_reads.is_empty() && bytes.len() != 4,
    };
    let decoded = match architecture {
        Architecture::X86_64 => {
            let decoded = match predicate {
                FunctionFragmentConditionalBranchPredicate::NonZeroV1 => {
                    if long {
                        validate_x86_64_selected_nonzero_branch_form(
                            physical,
                            row.alternative,
                            branch.byte_displacement,
                            bytes,
                        )
                    } else {
                        validate_x86_64_selected_short_nonzero_branch_form(
                            physical,
                            row.alternative,
                            branch.byte_displacement,
                            bytes,
                        )
                    }
                }
                FunctionFragmentConditionalBranchPredicate::U64LessThanV1 => {
                    validate_x86_64_selected_u64_less_than_branch_form(
                        physical,
                        row.alternative,
                        branch.byte_displacement,
                        bytes,
                    )
                }
                FunctionFragmentConditionalBranchPredicate::I64LessThanV1 => {
                    validate_x86_64_selected_i64_less_than_branch_form(
                        physical,
                        row.alternative,
                        branch.byte_displacement,
                        bytes,
                    )
                }
            }
            .map_err(|error| RelativeBranchError::X86_64Branch(instruction, error))?;
            footprint!(decoded)
        }
        Architecture::Aarch64
            if predicate == FunctionFragmentConditionalBranchPredicate::NonZeroV1
                && branch.decoded_register_reads.len() == 1 =>
        {
            let decoded = validate_aarch64_fused_compare_i64_zero_branch_nonzero_to_cbnz_form(
                physical,
                branch.decoded_register_reads[0],
                branch.byte_displacement,
                bytes,
            )
            .map_err(|error| RelativeBranchError::Aarch64Branch(instruction, error))?;
            footprint!(decoded)
        }
        Architecture::Aarch64 => {
            let form = if long {
                Aarch64ConditionalBranchForm::WidenedPairV1
            } else {
                Aarch64ConditionalBranchForm::CanonicalWordV1
            };
            let decoded = validate_aarch64_selected_conditional_branch_form(
                physical,
                row.alternative,
                conditional_branch_family(predicate),
                form,
                branch.byte_displacement,
                bytes,
            )
            .map_err(|error| RelativeBranchError::Aarch64Branch(instruction, error))?;
            footprint!(decoded)
        }
    };
    Ok(decoded)
}
