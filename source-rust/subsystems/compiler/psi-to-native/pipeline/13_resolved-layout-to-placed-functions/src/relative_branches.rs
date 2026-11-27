//! Verify the placed branch against its admitted instruction form and read the
//! target from the final bytes after frame insertion.
use machine_code::{
    FunctionFragmentBranchEvidence, FunctionFragmentConditionalBranchPredicate,
    FunctionFragmentEmissionPlan, FunctionFragmentInstructionSpan,
};
use selected_instructions::{MachineAlternativeFamily, SelectedBlockId, SelectedInstructionId};
use semantic_vocabulary::MachineId;
use target::Architecture;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FunctionFrameBranchOrigins {
    pub(super) machine: MachineId,
    pub(super) controls: machine_code::EncodedControlIndex,
    pub(super) branches: Vec<FrameBranchOrigin>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FrameBranchOrigin {
    pub(super) instruction: SelectedInstructionId,
    pub(super) branch: FunctionFragmentBranchEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelativeBranchError {
    MissingFunction(MachineId),
    SourceShapeMismatch(MachineId),
    MissingTargetBlock(SelectedBlockId),
    BranchFallthroughMismatch(SelectedInstructionId),
    BranchEffectsMismatch(SelectedInstructionId),
    BranchBytes(SelectedInstructionId),
    OffsetOverflow,
    ArtifactMismatch,
}

impl std::fmt::Display for RelativeBranchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "function-fragment frame application failed: {self:?}")
    }
}

impl std::error::Error for RelativeBranchError {}

/// Instruction identities and exact bytes proven to branch within this function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedFunctionBranches {
    function_length: usize,
    rows: Vec<CheckedBranchSpan>,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckedBranchSpan {
    instruction: SelectedInstructionId,
    offset: usize,
    checked_bytes: std::ops::Range<usize>,
}

impl CheckedFunctionBranches {
    pub fn verify_bytes(&self, actual: &[u8]) -> Result<(), &'static str> {
        if actual.len() != self.function_length {
            return Err("installed function length differs from its checked placement");
        }
        for row in &self.rows {
            let expected = &self.bytes[row.checked_bytes.clone()];
            if actual
                .get(row.offset..)
                .and_then(|tail| tail.get(..expected.len()))
                != Some(expected)
            {
                return Err("installed branch bytes differ from the checked placement");
            }
        }
        Ok(())
    }

    pub fn contains(&self, instruction: SelectedInstructionId) -> bool {
        self.rows
            .binary_search_by_key(&instruction, |row| row.instruction)
            .is_ok()
    }
}

/// Read each placed branch using the instruction form and target block that belong to it.
pub(super) fn validate_published_relative_branches(
    fragments: &FunctionFragmentEmissionPlan,
    origins: &[FunctionFrameBranchOrigins],
    machine: MachineId,
    bytes: &[u8],
) -> Result<CheckedFunctionBranches, RelativeBranchError> {
    let function = fragments
        .function(machine)
        .ok_or(RelativeBranchError::MissingFunction(machine))?;
    if u64::try_from(bytes.len()) != Ok(function.byte_count) {
        return Err(RelativeBranchError::ArtifactMismatch);
    }
    let mut checked = CheckedFunctionBranches {
        function_length: bytes.len(),
        rows: Vec::new(),
        bytes: Vec::new(),
    };
    validate_rows(
        fragments,
        origins,
        machine,
        function.instructions().filter(|row| row.branch.is_some()),
        |row| {
            let start = usize::try_from(row.offset).ok()?;
            bytes.get(start..start.checked_add(row.bytes.len())?)
        },
        |row, decoded| {
            let offset =
                usize::try_from(row.offset).map_err(|_| RelativeBranchError::OffsetOverflow)?;
            let start = checked.bytes.len();
            checked.bytes.extend_from_slice(decoded);
            checked.rows.push(CheckedBranchSpan {
                instruction: row.instruction,
                offset,
                checked_bytes: start..checked.bytes.len(),
            });
            Ok(())
        },
    )?;
    checked.rows.sort_unstable_by_key(|row| row.instruction);
    Ok(checked)
}

fn validate_rows<'a, 'bytes>(
    fragments: &FunctionFragmentEmissionPlan,
    origins: &[FunctionFrameBranchOrigins],
    machine: MachineId,
    candidates: impl IntoIterator<Item = &'a FunctionFragmentInstructionSpan>,
    bytes_for: impl Fn(&'a FunctionFragmentInstructionSpan) -> Option<&'bytes [u8]>,
    mut record: impl FnMut(&FunctionFragmentInstructionSpan, &[u8]) -> Result<(), RelativeBranchError>,
) -> Result<(), RelativeBranchError> {
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
            &source_function.controls,
            |block| {
                offsets
                    .binary_search_by_key(&block, |row| row.0)
                    .ok()
                    .map(|index| offsets[index].1)
            },
        )?;
        record(candidate, bytes)?;
    }
    if candidates.next().is_some() || source_rows.next().is_some() {
        return Err(Error::SourceShapeMismatch(machine));
    }
    Ok(())
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

pub(super) fn validate_branch_row(
    source_branch: &FunctionFragmentBranchEvidence,
    candidate: &FunctionFragmentInstructionSpan,
    bytes: &[u8],
    architecture: Architecture,
    controls: &machine_code::EncodedControlIndex,
    block_offset: impl Fn(SelectedBlockId) -> Option<u64>,
) -> Result<(), RelativeBranchError> {
    use FunctionFragmentBranchEvidence as Branch;
    let Some(branch) = candidate.branch.as_deref() else {
        return Err(RelativeBranchError::ArtifactMismatch);
    };
    let form = controls
        .get(candidate.instruction)
        .and_then(|forms| forms.matching(candidate.alternative, bytes.len()))
        .ok_or(RelativeBranchError::BranchBytes(candidate.instruction))?;
    let decoded_target = form
        .read_target(bytes, candidate.offset, architecture)
        .ok_or(RelativeBranchError::BranchBytes(candidate.instruction))?;
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
        if decoded_target != target || form.alternative.family != MachineAlternativeFamily::Jump {
            return Err(RelativeBranchError::BranchBytes(candidate.instruction));
        }
        return if form.footprint.encoded == branch.decoded_effects {
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
    if decoded_target != taken
        || form.alternative.family != conditional_branch_family(branch.predicate)
    {
        return Err(RelativeBranchError::BranchBytes(candidate.instruction));
    }
    if form.footprint.register_reads != branch.decoded_register_reads
        || form.footprint.encoded != branch.decoded_effects
    {
        return Err(RelativeBranchError::BranchEffectsMismatch(
            candidate.instruction,
        ));
    }
    Ok(())
}
