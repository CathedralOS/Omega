/*
We implement x86-64 conditional-branch relaxation (`StagedOptimizedX86BranchRelaxation`) for stage 09. In each sweep over the function-relative layout roster, we inspect every conditional branch through its predicate lane (`ENCODE` / `VALIDATE`), select every six-byte near branch (`Jcc rel32`) whose prospective displacement after shrinking fits within a signed 8-bit immediate (`Jcc rel8`), commit the narrowed two-byte encodings in batch, and reflow dense block and instruction offsets and jump or branch displacements across the function. Because narrowing only removes bytes, the span between any branch and its target is monotone non-increasing across sweeps, converging on the least fixed point while charging every rule evaluation, candidate, validation step, commit, and iteration against `OptimizationWorkBudget`.
*/

use std::collections::BTreeMap;
use std::sync::Arc;

use isa_x86_64::{
    ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError,
    encode_x86_64_selected_i64_less_than_branch_form, encode_x86_64_selected_nonzero_branch_form,
    encode_x86_64_selected_short_nonzero_branch_form,
    encode_x86_64_selected_u64_less_than_branch_form,
    validate_x86_64_selected_i64_less_than_branch_form,
    validate_x86_64_selected_nonzero_branch_form,
    validate_x86_64_selected_short_nonzero_branch_form,
    validate_x86_64_selected_u64_less_than_branch_form,
};
use machine_code::layout::evidence::relaxation_identity::{
    RevisionRoots, artifact_identity, revision_identity,
};
pub use machine_code::layout::evidence::{
    X86BranchRelaxationAction, X86BranchRelaxationAttempt, X86BranchRelaxationAttemptOutcome,
    X86BranchRelaxationIdentity, X86BranchRelaxationPolicy, X86BranchRelaxationRevisionIdentity,
};
use machine_code::{
    ResolvedBranchEvidence, ResolvedConditionalBranchEvidence, ResolvedConditionalBranchPredicate,
    ResolvedMachineLayout, ResolvedSelectedFormLayoutIdentity, ResolvedSelectedFormRow,
    ResolvedSelectedFunctionLayout,
};
use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, SelectedBlockId, SelectedInstructionId,
};
use target::{Architecture, NativeTarget};

const SHORT_WIDTH: usize = 2;
const NEAR_WIDTH: usize = 6;
const SHORT_FORM_SAVED_BYTES: u64 = (NEAR_WIDTH - SHORT_WIDTH) as u64;

type Encoding = Result<ValidatedX86_64SelectedFormEncoding, X86_64SelectedFormEncodingError>;
type Encode = fn(&ValidatedPhysicalRegisterModel, MachineAlternativeKey, i64) -> Encoding;
type Validate = fn(&ValidatedPhysicalRegisterModel, MachineAlternativeKey, i64, &[u8]) -> Encoding;

const ENCODE: [[Encode; 2]; 3] = [
    [
        encode_x86_64_selected_short_nonzero_branch_form,
        encode_x86_64_selected_nonzero_branch_form,
    ],
    [encode_x86_64_selected_u64_less_than_branch_form; 2],
    [encode_x86_64_selected_i64_less_than_branch_form; 2],
];

const VALIDATE: [[Validate; 2]; 3] = [
    [
        validate_x86_64_selected_short_nonzero_branch_form,
        validate_x86_64_selected_nonzero_branch_form,
    ],
    [validate_x86_64_selected_u64_less_than_branch_form; 2],
    [validate_x86_64_selected_i64_less_than_branch_form; 2],
];

macro_rules! charge_axis {
    ($usage:ident, $budget:ident, $axis:ident, $variant:ident) => {
        charge(
            &mut $usage.$axis,
            $budget.$axis(),
            X86BranchRelaxationWorkAxis::$variant,
        )?
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86BranchRelaxationWorkAxis {
    RuleEvaluations,
    Candidates,
    ValidationSteps,
    Commits,
    Iterations,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedX86BranchRelaxationError {
    UnsupportedTarget(NativeTarget),
    BudgetExceeded(X86BranchRelaxationWorkAxis),
    DuplicateInstruction(SelectedInstructionId),
    MissingTargetBlock(SelectedBlockId),
    OffsetOverflow,
    NonContiguousBlock(SelectedBlockId),
    BranchFallthroughMismatch(SelectedInstructionId),
    MalformedBranch(SelectedInstructionId),
    BranchEffectsMismatch(SelectedInstructionId),
    NonDecreasingByteMeasure,
    X86_64(X86_64SelectedFormEncodingError),
    ArtifactMismatch,
}

impl std::fmt::Display for OptimizedX86BranchRelaxationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized x86 branch relaxation failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedX86BranchRelaxationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedX86BranchRelaxation {
    source: ResolvedSelectedFormLayoutIdentity, // Baseline resolved-layout identity before relaxation.
    selected: selected_instructions::SelectedInstructionPlanIdentity, // Upstream selected-instruction plan identity.
    machine: physical_instructions::PostAllocationMachineIdentity, // Upstream post-allocation machine identity.
    pre_layout: machine_code::SelectedFormEncodingIdentity, // Upstream pre-layout encoding identity.
    target: NativeTarget, // Target platform for which relaxation was performed.
    policy: X86BranchRelaxationPolicy, // Relaxation policy applied during the sweep.
    budget: OptimizationWorkBudget, // Work budget governing the relaxation sweep.
    usage: OptimizationWorkUsage, // Actual work consumed across all sweeps.
    output: ResolvedSelectedFormLayoutIdentity, // Final resolved-layout identity after relaxation.
    output_revision: X86BranchRelaxationRevisionIdentity, // Final revision hash of the relaxed function roster.
    identity: X86BranchRelaxationIdentity, // Content identity of the complete relaxation artifact.
    attempts: Vec<X86BranchRelaxationAttempt>, // Per-branch inspection attempts recorded in sweep order.
    actions: Vec<X86BranchRelaxationAction>,   // Committed near-to-short branch rewrites.
    layout: Arc<ResolvedMachineLayout>,        // Rewritten function-relative machine layout.
}

impl StagedOptimizedX86BranchRelaxation {
    pub const fn source(&self) -> ResolvedSelectedFormLayoutIdentity {
        self.source
    }

    pub const fn selected(&self) -> selected_instructions::SelectedInstructionPlanIdentity {
        self.selected
    }

    pub const fn machine(&self) -> physical_instructions::PostAllocationMachineIdentity {
        self.machine
    }

    pub const fn pre_layout(&self) -> machine_code::SelectedFormEncodingIdentity {
        self.pre_layout
    }

    pub const fn target(&self) -> NativeTarget {
        self.target
    }

    pub const fn policy(&self) -> X86BranchRelaxationPolicy {
        self.policy
    }

    pub const fn budget(&self) -> OptimizationWorkBudget {
        self.budget
    }

    pub const fn usage(&self) -> OptimizationWorkUsage {
        self.usage
    }

    pub const fn output(&self) -> ResolvedSelectedFormLayoutIdentity {
        self.output
    }

    pub const fn output_revision(&self) -> X86BranchRelaxationRevisionIdentity {
        self.output_revision
    }

    pub const fn identity(&self) -> X86BranchRelaxationIdentity {
        self.identity
    }

    pub fn attempts(&self) -> &[X86BranchRelaxationAttempt] {
        &self.attempts
    }

    pub fn actions(&self) -> &[X86BranchRelaxationAction] {
        &self.actions
    }

    pub fn functions(&self) -> &[ResolvedSelectedFunctionLayout] {
        self.layout.functions()
    }

    pub fn layout(&self) -> &ResolvedMachineLayout {
        &self.layout
    }

    pub fn shared_layout(&self) -> Arc<ResolvedMachineLayout> {
        Arc::clone(&self.layout)
    }
}

struct RelaxationTrace {
    usage: OptimizationWorkUsage, // Cumulative work charged across sweeps.
    attempts: Vec<X86BranchRelaxationAttempt>, // Recorded branch inspection outcomes.
    actions: Vec<X86BranchRelaxationAction>, // Recorded near-to-short branch rewrites.
    functions: Vec<ResolvedSelectedFunctionLayout>, // Rewritten function layouts after reflow.
}

struct SweptBranch {
    function_index: usize,    // Index of the function containing the branch.
    block_index: usize,       // Index of the block containing the branch.
    instruction_index: usize, // Index of the branch row within its block.
    displacement: i64,        // Prospective short-form displacement measured before batch reflow.
}

pub(crate) fn stage_optimized_x86_branch_relaxation(
    physical: &ValidatedPhysicalRegisterModel,
    source: &ResolvedMachineLayout,
    budget: OptimizationWorkBudget,
) -> Result<StagedOptimizedX86BranchRelaxation, OptimizedX86BranchRelaxationError> {
    validate_roots(source, physical)?;
    let trace = compute_trace(revision_roots(source), source.functions(), physical, budget)?;
    finish_artifact(source, budget, trace)
}

pub(crate) fn validate_optimized_x86_branch_relaxation(
    physical: &ValidatedPhysicalRegisterModel,
    source: &ResolvedMachineLayout,
    artifact: &StagedOptimizedX86BranchRelaxation,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    validate_roots(source, physical)?;
    if artifact.source != source.identity()
        || artifact.selected != source.selected()
        || artifact.machine != source.machine()
        || artifact.pre_layout != source.pre_layout()
        || artifact.target != source.target()
        || artifact.policy != X86BranchRelaxationPolicy::X86RelaxConditionalBranchesToRel8V1
        || !artifact.usage.within(artifact.budget)
    {
        return Err(OptimizedX86BranchRelaxationError::ArtifactMismatch);
    }
    Ok(())
}

fn validate_roots(
    source: &ResolvedMachineLayout,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    ensure_x86_target(source.target(), physical)
}

fn ensure_x86_target(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    if target.architecture != Architecture::X86_64
        || physical.model().architecture != Architecture::X86_64
    {
        return Err(OptimizedX86BranchRelaxationError::UnsupportedTarget(target));
    }
    Ok(())
}

fn conditional_evidence(
    row: &ResolvedSelectedFormRow,
) -> Result<&ResolvedConditionalBranchEvidence, OptimizedX86BranchRelaxationError> {
    row.branch
        .as_deref()
        .and_then(ResolvedBranchEvidence::as_conditional)
        .ok_or(OptimizedX86BranchRelaxationError::MalformedBranch(
            row.instruction,
        ))
}

fn malformed(instruction: SelectedInstructionId) -> OptimizedX86BranchRelaxationError {
    OptimizedX86BranchRelaxationError::MalformedBranch(instruction)
}

fn revision_roots(source: &ResolvedMachineLayout) -> RevisionRoots {
    RevisionRoots {
        source: source.identity(),
        selected: source.selected(),
        machine: source.machine(),
        pre_layout: source.pre_layout(),
        target: source.target(),
    }
}

fn finish_artifact(
    source: &ResolvedMachineLayout,
    budget: OptimizationWorkBudget,
    trace: RelaxationTrace,
) -> Result<StagedOptimizedX86BranchRelaxation, OptimizedX86BranchRelaxationError> {
    let roots = revision_roots(source);
    let output_revision = revision_identity(roots, &trace.functions);
    let mut program = source.clone();
    program.functions = trace.functions;
    program.identity = program.recomputed_identity();
    let layout = Arc::new(program);
    let output = layout.identity();
    let policy = X86BranchRelaxationPolicy::X86RelaxConditionalBranchesToRel8V1;
    let identity = artifact_identity(
        roots,
        policy,
        budget,
        trace.usage,
        output,
        output_revision,
        &trace.attempts,
        &trace.actions,
        layout.functions(),
    );
    Ok(StagedOptimizedX86BranchRelaxation {
        source: source.identity(),
        selected: source.selected(),
        machine: source.machine(),
        pre_layout: source.pre_layout(),
        target: source.target(),
        policy,
        budget,
        usage: trace.usage,
        output,
        output_revision,
        identity,
        attempts: trace.attempts,
        actions: trace.actions,
        layout,
    })
}

fn inspect_production_branch(
    function: &ResolvedSelectedFunctionLayout,
    block_index: usize,
    instruction_index: usize,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(X86BranchRelaxationAttemptOutcome, Option<i64>), OptimizedX86BranchRelaxationError> {
    let row = &function.blocks[block_index].instructions[instruction_index];
    let branch = conditional_evidence(row)?;
    let near = near_width(row.bytes.len()).ok_or_else(|| malformed(row.instruction))?;
    validate_lane(
        branch.predicate,
        near,
        physical,
        row.alternative,
        branch.byte_displacement,
        &row.bytes,
    )?;
    if !near {
        return Ok((X86BranchRelaxationAttemptOutcome::AlreadyShort, None));
    }
    let displacement = prospective_short_displacement(function, row, branch.when_taken_block)?;
    if i8::try_from(displacement).is_ok() {
        Ok((
            X86BranchRelaxationAttemptOutcome::SelectedForRelaxation,
            Some(displacement),
        ))
    } else {
        Ok((
            X86BranchRelaxationAttemptOutcome::NearDisplacementOutsideI8,
            None,
        ))
    }
}

fn prospective_short_displacement(
    function: &ResolvedSelectedFunctionLayout,
    row: &ResolvedSelectedFormRow,
    target: SelectedBlockId,
) -> Result<i64, OptimizedX86BranchRelaxationError> {
    let target_offset = function
        .blocks
        .iter()
        .find(|block| block.block == target)
        .map(|block| block.offset)
        .ok_or(OptimizedX86BranchRelaxationError::MissingTargetBlock(
            target,
        ))?;
    let shifted_target = if target_offset > row.offset {
        target_offset
            .checked_sub(4)
            .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)?
    } else {
        target_offset
    };
    checked_delta(shifted_target, advance(row.offset, SHORT_WIDTH)?)
}

fn near_width(bytes: usize) -> Option<bool> {
    match bytes {
        SHORT_WIDTH => Some(false),
        NEAR_WIDTH => Some(true),
        _ => None,
    }
}

fn selected_alternative(
    predicate: ResolvedConditionalBranchPredicate,
    alternative: MachineAlternativeKey,
    near: bool,
) -> MachineAlternativeKey {
    let variant = if near { 0 } else { 1 };
    match predicate {
        ResolvedConditionalBranchPredicate::NonZeroV1 => alternative,
        ResolvedConditionalBranchPredicate::U64LessThanV1 => key(
            MachineAlternativeFamily::ConditionalBranchU64LessThan,
            variant,
        ),
        ResolvedConditionalBranchPredicate::I64LessThanV1 => key(
            MachineAlternativeFamily::ConditionalBranchI64LessThan,
            variant,
        ),
    }
}

const fn key(family: MachineAlternativeFamily, variant: u32) -> MachineAlternativeKey {
    MachineAlternativeKey { family, variant }
}

fn encode_lane(
    predicate: ResolvedConditionalBranchPredicate,
    near: bool,
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement: i64,
) -> Result<ValidatedX86_64SelectedFormEncoding, OptimizedX86BranchRelaxationError> {
    (ENCODE[usize::from(predicate.rank())][usize::from(near)])(
        physical,
        alternative,
        byte_displacement,
    )
    .map_err(OptimizedX86BranchRelaxationError::X86_64)
}

fn validate_lane(
    predicate: ResolvedConditionalBranchPredicate,
    near: bool,
    physical: &ValidatedPhysicalRegisterModel,
    alternative: MachineAlternativeKey,
    byte_displacement: i64,
    bytes: &[u8],
) -> Result<ValidatedX86_64SelectedFormEncoding, OptimizedX86BranchRelaxationError> {
    (VALIDATE[usize::from(predicate.rank())][usize::from(near)])(
        physical,
        alternative,
        byte_displacement,
        bytes,
    )
    .map_err(OptimizedX86BranchRelaxationError::X86_64)
}

fn compute_trace(
    roots: RevisionRoots,
    source: &[ResolvedSelectedFunctionLayout],
    physical: &ValidatedPhysicalRegisterModel,
    budget: OptimizationWorkBudget,
) -> Result<RelaxationTrace, OptimizedX86BranchRelaxationError> {
    let mut functions = source.to_vec();
    let mut attempts = Vec::new();
    let mut actions = Vec::new();
    let mut usage = OptimizationWorkUsage::default();
    loop {
        charge_axis!(usage, budget, iterations, Iterations);
        let iteration = usage.iterations;
        let input = revision_identity(roots, &functions);
        let previous_bytes = total_bytes(&functions)?;
        let mut swept: Vec<SweptBranch> = Vec::new();
        for (function_index, function) in functions.iter().enumerate() {
            for (block_index, block) in function.blocks.iter().enumerate() {
                for (instruction_index, row) in block.instructions.iter().enumerate() {
                    let Some(branch) = row
                        .branch
                        .as_deref()
                        .and_then(ResolvedBranchEvidence::as_conditional)
                    else {
                        continue;
                    };
                    charge_axis!(usage, budget, rule_evaluations, RuleEvaluations);
                    let (outcome, short_displacement) = inspect_production_branch(
                        &functions[function_index],
                        block_index,
                        instruction_index,
                        physical,
                    )?;
                    attempts.push(X86BranchRelaxationAttempt {
                        iteration,
                        input,
                        instruction: row.instruction,
                        offset: row.offset,
                        byte_displacement: branch.byte_displacement,
                        encoded_bytes: u8::try_from(row.bytes.len())
                            .map_err(|_| OptimizedX86BranchRelaxationError::OffsetOverflow)?,
                        outcome,
                    });
                    if outcome == X86BranchRelaxationAttemptOutcome::SelectedForRelaxation {
                        charge_axis!(usage, budget, candidates, Candidates);
                        charge_axis!(usage, budget, validation_steps, ValidationSteps);
                        let displacement =
                            short_displacement.ok_or_else(|| malformed(row.instruction))?;
                        swept.push(SweptBranch {
                            function_index,
                            block_index,
                            instruction_index,
                            displacement,
                        });
                    }
                }
            }
        }
        if swept.is_empty() {
            break;
        }
        let mut narrowed = Vec::with_capacity(swept.len());
        for branch in &swept {
            charge_axis!(usage, budget, commits, Commits);
            narrowed.push(commit_short_form(&mut functions, branch, physical)?);
        }
        reflow_production_functions(&mut functions, physical)?;
        let expected_saving = u64::try_from(narrowed.len())
            .ok()
            .and_then(|count| count.checked_mul(SHORT_FORM_SAVED_BYTES))
            .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)?;
        let current_bytes = total_bytes(&functions)?;
        if previous_bytes.checked_sub(current_bytes) != Some(expected_saving) {
            return Err(OptimizedX86BranchRelaxationError::NonDecreasingByteMeasure);
        }
        let output = revision_identity(roots, &functions);
        for (branch, old) in swept.iter().zip(narrowed) {
            actions.push(relaxation_action(
                &functions, branch, old, iteration, input, output,
            )?);
        }
    }
    Ok(RelaxationTrace {
        usage,
        attempts,
        actions,
        functions,
    })
}

fn commit_short_form(
    functions: &mut [ResolvedSelectedFunctionLayout],
    branch: &SweptBranch,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<ResolvedSelectedFormRow, OptimizedX86BranchRelaxationError> {
    let row = &mut functions[branch.function_index].blocks[branch.block_index].instructions
        [branch.instruction_index];
    let old = row.clone();
    let predicate = conditional_evidence(&old)?.predicate;
    let short_alternative = selected_alternative(predicate, old.alternative, false);
    let encoded = encode_lane(
        predicate,
        false,
        physical,
        short_alternative,
        branch.displacement,
    )?;
    row.alternative = short_alternative;
    row.bytes = encoded.bytes().to_vec();
    Ok(old)
}

fn relaxation_action(
    functions: &[ResolvedSelectedFunctionLayout],
    branch: &SweptBranch,
    old: ResolvedSelectedFormRow,
    iteration: u64,
    input: X86BranchRelaxationRevisionIdentity,
    output: X86BranchRelaxationRevisionIdentity,
) -> Result<X86BranchRelaxationAction, OptimizedX86BranchRelaxationError> {
    let new = &functions[branch.function_index].blocks[branch.block_index].instructions
        [branch.instruction_index];
    let old_displacement = conditional_evidence(&old)?.byte_displacement;
    let new_displacement = conditional_evidence(new)?.byte_displacement;
    Ok(X86BranchRelaxationAction {
        iteration,
        input,
        output,
        instruction: old.instruction,
        old_offset: old.offset,
        new_offset: new.offset,
        old_displacement,
        new_displacement,
        old_bytes: old.bytes,
        new_bytes: new.bytes.clone(),
    })
}

fn reflow_production_functions(
    functions: &mut [ResolvedSelectedFunctionLayout],
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    for function in functions {
        let offsets = assign_dense_offsets(function)?;
        for block in &mut function.blocks {
            for row in &mut block.instructions {
                let Some(branch) = row.branch.as_deref_mut() else {
                    continue;
                };
                let branch = match branch {
                    ResolvedBranchEvidence::Conditional(branch) => branch,
                    ResolvedBranchEvidence::Jump(jump) => {
                        let target = *offsets.get(&jump.target_block).ok_or(
                            OptimizedX86BranchRelaxationError::MissingTargetBlock(
                                jump.target_block,
                            ),
                        )?;
                        let end = advance(row.offset, row.bytes.len())?;
                        let displacement = checked_delta(target, end)?;
                        let encoded = isa_x86_64::encode_x86_64_selected_jump_form(
                            physical,
                            row.alternative,
                            displacement,
                        )
                        .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
                        if row.bytes.len() != 5
                            || encoded.footprint().encoded != jump.decoded_effects
                        {
                            return Err(malformed(row.instruction));
                        }
                        jump.target_offset = target;
                        jump.byte_displacement = displacement;
                        row.bytes = encoded.bytes().to_vec();
                        continue;
                    }
                };
                rewrite_branch_offsets(
                    branch,
                    row.offset,
                    row.bytes.len(),
                    &offsets,
                    row.instruction,
                )?;
                let near = near_width(row.bytes.len()).ok_or_else(|| malformed(row.instruction))?;
                let encoded = encode_lane(
                    branch.predicate,
                    near,
                    physical,
                    row.alternative,
                    branch.byte_displacement,
                )?;
                if encoded.bytes().len() != row.bytes.len() {
                    return Err(malformed(row.instruction));
                }
                if encoded.footprint().encoded != branch.decoded_effects {
                    return Err(OptimizedX86BranchRelaxationError::BranchEffectsMismatch(
                        row.instruction,
                    ));
                }
                row.bytes = encoded.bytes().to_vec();
            }
        }
    }
    Ok(())
}

fn assign_dense_offsets(
    function: &mut ResolvedSelectedFunctionLayout,
) -> Result<BTreeMap<SelectedBlockId, u64>, OptimizedX86BranchRelaxationError> {
    let mut offsets = BTreeMap::new();
    let mut offset = 0_u64;
    for block in &mut function.blocks {
        block.offset = offset;
        offsets.insert(block.block, offset);
        let start = offset;
        for row in &mut block.instructions {
            row.offset = offset;
            offset = advance(offset, row.bytes.len())?;
        }
        block.byte_count = offset - start;
    }
    function.byte_count = offset;
    Ok(offsets)
}

fn rewrite_branch_offsets(
    branch: &mut ResolvedConditionalBranchEvidence,
    instruction_offset: u64,
    instruction_size: usize,
    offsets: &BTreeMap<SelectedBlockId, u64>,
    instruction: SelectedInstructionId,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    let taken = *offsets.get(&branch.when_taken_block).ok_or(
        OptimizedX86BranchRelaxationError::MissingTargetBlock(branch.when_taken_block),
    )?;
    let fallthrough = *offsets.get(&branch.when_fallthrough_block).ok_or(
        OptimizedX86BranchRelaxationError::MissingTargetBlock(branch.when_fallthrough_block),
    )?;
    let end = advance(instruction_offset, instruction_size)?;
    if fallthrough != end {
        return Err(OptimizedX86BranchRelaxationError::BranchFallthroughMismatch(instruction));
    }
    branch.when_taken_offset = taken;
    branch.when_fallthrough_offset = fallthrough;
    branch.byte_displacement = checked_delta(taken, end)?;
    Ok(())
}

fn total_bytes(
    functions: &[ResolvedSelectedFunctionLayout],
) -> Result<u64, OptimizedX86BranchRelaxationError> {
    functions.iter().try_fold(0_u64, |total, function| {
        total
            .checked_add(function.byte_count)
            .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)
    })
}

fn charge(
    usage: &mut u64,
    limit: u64,
    axis: X86BranchRelaxationWorkAxis,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    *usage = usage
        .checked_add(1)
        .ok_or(OptimizedX86BranchRelaxationError::BudgetExceeded(axis))?;
    if *usage > limit {
        return Err(OptimizedX86BranchRelaxationError::BudgetExceeded(axis));
    }
    Ok(())
}

fn checked_delta(target: u64, base: u64) -> Result<i64, OptimizedX86BranchRelaxationError> {
    i64::try_from(i128::from(target) - i128::from(base))
        .map_err(|_| OptimizedX86BranchRelaxationError::OffsetOverflow)
}

fn advance(offset: u64, bytes: usize) -> Result<u64, OptimizedX86BranchRelaxationError> {
    let width =
        u64::try_from(bytes).map_err(|_| OptimizedX86BranchRelaxationError::OffsetOverflow)?;
    offset
        .checked_add(width)
        .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)
}

#[cfg(test)]
mod tests {
    use isa_x86_64::{
        ValidatedX86_64SelectedFormEncoding, encode_x86_64_selected_i64_less_than_branch_form,
        encode_x86_64_selected_jump_form, encode_x86_64_selected_nonzero_branch_form,
        encode_x86_64_selected_short_nonzero_branch_form,
        encode_x86_64_selected_u64_less_than_branch_form, x86_64_physical_register_model,
    };
    use machine_code::{
        ResolvedConditionalBranchPredicate, ResolvedSelectedBlockLayout,
        ResolvedSelectedFormLayoutIdentity, ResolvedSelectedFormRow,
        ResolvedSelectedFunctionLayout,
    };
    use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
    use register_model::{ValidatedPhysicalRegisterModel, validate_physical_register_model};
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedControlEffect,
        SelectedBlockId, SelectedInstructionId,
    };
    use semantic_vocabulary::{EdgeId, MachineId};
    use target::NativeTarget;

    use super::{
        OptimizedX86BranchRelaxationError, RevisionRoots, X86BranchRelaxationAction,
        X86BranchRelaxationAttempt, X86BranchRelaxationAttemptOutcome, X86BranchRelaxationIdentity,
        X86BranchRelaxationPolicy, X86BranchRelaxationRevisionIdentity,
        X86BranchRelaxationWorkAxis, artifact_identity, charge, compute_trace, encode_lane,
        ensure_x86_target, inspect_production_branch, reflow_production_functions,
        revision_identity, selected_alternative,
    };

    fn physical() -> ValidatedPhysicalRegisterModel {
        validate_physical_register_model(x86_64_physical_register_model()).unwrap()
    }

    fn alternative() -> MachineAlternativeKey {
        MachineAlternativeKey {
            family: MachineAlternativeFamily::ConditionalBranchNonZero,
            variant: 0,
        }
    }

    fn function(zero_arm_bytes: usize) -> ResolvedSelectedFunctionLayout {
        conditional_function(
            ResolvedConditionalBranchPredicate::NonZeroV1,
            zero_arm_bytes,
        )
    }

    fn less_than_function(not_less_arm_bytes: usize) -> ResolvedSelectedFunctionLayout {
        conditional_function(
            ResolvedConditionalBranchPredicate::U64LessThanV1,
            not_less_arm_bytes,
        )
    }

    fn signed_less_than_function(not_less_arm_bytes: usize) -> ResolvedSelectedFunctionLayout {
        conditional_function(
            ResolvedConditionalBranchPredicate::I64LessThanV1,
            not_less_arm_bytes,
        )
    }

    fn conditional_function(
        predicate: ResolvedConditionalBranchPredicate,
        fallthrough_arm_bytes: usize,
    ) -> ResolvedSelectedFunctionLayout {
        let physical = physical();
        let displacement = i64::try_from(fallthrough_arm_bytes).unwrap();
        let branch_alternative = selected_alternative(predicate, alternative(), true);
        let near =
            encode_lane(predicate, true, &physical, branch_alternative, displacement).unwrap();
        let entry = SelectedBlockId(0);
        let fallthrough = SelectedBlockId(1);
        let taken = SelectedBlockId(2);
        let fallthrough_offset = 6;
        let taken_offset = fallthrough_offset + u64::try_from(fallthrough_arm_bytes).unwrap();
        ResolvedSelectedFunctionLayout {
            machine: MachineId::new(1).unwrap(),
            byte_count: taken_offset + 1,
            blocks: vec![
                near_conditional_block(
                    entry,
                    0,
                    SelectedInstructionId(0),
                    branch_alternative,
                    predicate,
                    &near,
                    entry,
                    (taken, taken_offset, 1),
                    (fallthrough, fallthrough_offset, 2),
                    displacement,
                ),
                padding_block(
                    fallthrough,
                    fallthrough_offset,
                    SelectedInstructionId(1),
                    fallthrough_arm_bytes,
                ),
                return_block(taken, taken_offset, SelectedInstructionId(2)),
            ],
        }
    }

    fn backward_conditional_function(target_block_bytes: usize) -> ResolvedSelectedFunctionLayout {
        let physical = physical();
        let taken = SelectedBlockId(0);
        let entry = SelectedBlockId(1);
        let fallthrough = SelectedBlockId(2);
        let branch_offset = u64::try_from(target_block_bytes).unwrap();
        let fallthrough_offset = branch_offset + 6;
        let near_displacement = -(i64::try_from(branch_offset).unwrap() + 6);
        let near =
            encode_x86_64_selected_nonzero_branch_form(&physical, alternative(), near_displacement)
                .unwrap();
        ResolvedSelectedFunctionLayout {
            machine: MachineId::new(1).unwrap(),
            byte_count: fallthrough_offset + 1,
            blocks: vec![
                padding_block(taken, 0, SelectedInstructionId(0), target_block_bytes),
                near_conditional_block(
                    entry,
                    branch_offset,
                    SelectedInstructionId(1),
                    alternative(),
                    ResolvedConditionalBranchPredicate::NonZeroV1,
                    &near,
                    entry,
                    (taken, 0, 1),
                    (fallthrough, fallthrough_offset, 2),
                    near_displacement,
                ),
                return_block(fallthrough, fallthrough_offset, SelectedInstructionId(2)),
            ],
        }
    }

    fn jump_across_branch_function(fallthrough_arm_bytes: usize) -> ResolvedSelectedFunctionLayout {
        let physical = physical();
        let jump_block = SelectedBlockId(0);
        let entry = SelectedBlockId(1);
        let fallthrough = SelectedBlockId(2);
        let taken = SelectedBlockId(3);
        let branch_offset = 5;
        let fallthrough_offset = branch_offset + 6;
        let taken_offset = fallthrough_offset + u64::try_from(fallthrough_arm_bytes).unwrap();
        let near_displacement = i64::try_from(fallthrough_arm_bytes).unwrap();
        let jump_displacement = i64::try_from(taken_offset).unwrap() - 5;
        let jump_alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::Jump,
            variant: 0,
        };
        let jump = encode_x86_64_selected_jump_form(&physical, jump_alternative, jump_displacement)
            .unwrap();
        let near =
            encode_x86_64_selected_nonzero_branch_form(&physical, alternative(), near_displacement)
                .unwrap();
        ResolvedSelectedFunctionLayout {
            machine: MachineId::new(1).unwrap(),
            byte_count: taken_offset + 1,
            blocks: vec![
                ResolvedSelectedBlockLayout {
                    block: jump_block,
                    offset: 0,
                    byte_count: 5,
                    instructions: vec![ResolvedSelectedFormRow {
                        instruction: SelectedInstructionId(0),
                        alternative: jump_alternative,
                        offset: 0,
                        bytes: jump.bytes().to_vec(),
                        branch: Some(Box::new(machine_code::ResolvedBranchEvidence::Jump(
                            machine_code::ResolvedJumpEvidence {
                                source_block: jump_block,
                                target_edge: Some(EdgeId::new(3).unwrap()),
                                target_block: taken,
                                target_offset: taken_offset,
                                byte_displacement: jump_displacement,
                                decoded_effects: jump.footprint().encoded.clone(),
                            },
                        ))),
                        internal_machine_fixup: None,
                        normalized_foreign_call_fixup: None,
                        image_address_fixup: None,
                    }],
                },
                near_conditional_block(
                    entry,
                    branch_offset,
                    SelectedInstructionId(1),
                    alternative(),
                    ResolvedConditionalBranchPredicate::NonZeroV1,
                    &near,
                    entry,
                    (taken, taken_offset, 1),
                    (fallthrough, fallthrough_offset, 2),
                    near_displacement,
                ),
                padding_block(
                    fallthrough,
                    fallthrough_offset,
                    SelectedInstructionId(2),
                    fallthrough_arm_bytes,
                ),
                return_block(taken, taken_offset, SelectedInstructionId(3)),
            ],
        }
    }

    fn conditional_branch_mut(
        function: &mut ResolvedSelectedFunctionLayout,
        block_index: usize,
        instruction_index: usize,
    ) -> &mut machine_code::ResolvedConditionalBranchEvidence {
        let Some(branch) = function.blocks[block_index].instructions[instruction_index]
            .branch
            .as_deref_mut()
        else {
            panic!("the fixture row must carry branch evidence")
        };
        match branch {
            machine_code::ResolvedBranchEvidence::Conditional(branch) => branch,
            machine_code::ResolvedBranchEvidence::Jump(_) => {
                panic!("the fixture row must carry a conditional branch")
            }
        }
    }

    fn padding_block(
        block: SelectedBlockId,
        offset: u64,
        instruction: SelectedInstructionId,
        bytes: usize,
    ) -> ResolvedSelectedBlockLayout {
        ResolvedSelectedBlockLayout {
            block,
            offset,
            byte_count: u64::try_from(bytes).unwrap(),
            instructions: vec![return_row(instruction, offset, vec![0x90; bytes])],
        }
    }

    fn return_block(
        block: SelectedBlockId,
        offset: u64,
        instruction: SelectedInstructionId,
    ) -> ResolvedSelectedBlockLayout {
        ResolvedSelectedBlockLayout {
            block,
            offset,
            byte_count: 1,
            instructions: vec![return_row(instruction, offset, vec![0xc3])],
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn near_conditional_block(
        block: SelectedBlockId,
        offset: u64,
        instruction: SelectedInstructionId,
        alternative: MachineAlternativeKey,
        predicate: ResolvedConditionalBranchPredicate,
        near: &ValidatedX86_64SelectedFormEncoding,
        source_block: SelectedBlockId,
        taken: (SelectedBlockId, u64, u64),
        fallthrough: (SelectedBlockId, u64, u64),
        displacement: i64,
    ) -> ResolvedSelectedBlockLayout {
        ResolvedSelectedBlockLayout {
            block,
            offset,
            byte_count: 6,
            instructions: vec![ResolvedSelectedFormRow {
                instruction,
                alternative,
                offset,
                bytes: near.bytes().to_vec(),
                branch: Some(Box::new(machine_code::ResolvedBranchEvidence::Conditional(
                    machine_code::ResolvedConditionalBranchEvidence {
                        predicate,
                        source_block,
                        when_taken_edge: Some(EdgeId::new(taken.2).unwrap()),
                        when_taken_block: taken.0,
                        when_taken_offset: taken.1,
                        when_fallthrough_edge: Some(EdgeId::new(fallthrough.2).unwrap()),
                        when_fallthrough_block: fallthrough.0,
                        when_fallthrough_offset: fallthrough.1,
                        byte_displacement: displacement,
                        decoded_register_reads: vec![],
                        decoded_effects: near.footprint().encoded.clone(),
                    },
                ))),
                internal_machine_fixup: None,
                normalized_foreign_call_fixup: None,
                image_address_fixup: None,
            }],
        }
    }

    fn return_row(
        instruction: SelectedInstructionId,
        offset: u64,
        bytes: Vec<u8>,
    ) -> ResolvedSelectedFormRow {
        ResolvedSelectedFormRow {
            instruction,
            alternative: MachineAlternativeKey {
                family: MachineAlternativeFamily::ReturnScalar,
                variant: 0,
            },
            offset,
            bytes,
            branch: None,
            internal_machine_fixup: None,
            normalized_foreign_call_fixup: None,
            image_address_fixup: None,
        }
    }

    fn assert_near_branch_relaxes_to_the_short_form(
        source: ResolvedSelectedFunctionLayout,
        branch_block: usize,
        displacement: i64,
        short_bytes: [u8; 2],
        offsets: &[(usize, u64)],
    ) -> Vec<ResolvedSelectedFunctionLayout> {
        let physical = physical();
        assert_eq!(
            inspect_production_branch(&source, branch_block, 0, &physical).unwrap(),
            (
                X86BranchRelaxationAttemptOutcome::SelectedForRelaxation,
                Some(displacement),
            )
        );
        let short = encode_x86_64_selected_short_nonzero_branch_form(
            &physical,
            alternative(),
            displacement,
        )
        .unwrap();
        let mut produced = vec![source.clone()];
        produced[0].blocks[branch_block].instructions[0].bytes = short.bytes().to_vec();
        reflow_production_functions(&mut produced, &physical).unwrap();
        assert_eq!(source.byte_count - produced[0].byte_count, 4);
        assert_eq!(
            produced[0].blocks[branch_block].instructions[0].bytes,
            short_bytes
        );
        for &(block, offset) in offsets {
            assert_eq!(produced[0].blocks[block].offset, offset);
        }
        assert_eq!(
            produced[0].blocks[branch_block].instructions[0]
                .branch
                .as_deref()
                .and_then(machine_code::ResolvedBranchEvidence::as_conditional)
                .unwrap()
                .byte_displacement,
            displacement
        );
        assert_eq!(
            inspect_production_branch(&produced[0], branch_block, 0, &physical).unwrap(),
            (X86BranchRelaxationAttemptOutcome::AlreadyShort, None)
        );
        produced
    }

    #[test]
    fn eligible_near_branch_shrinks_and_reflow_settles_on_the_short_form() {
        let mut produced = assert_near_branch_relaxes_to_the_short_form(
            function(127),
            0,
            127,
            [0x75, 0x7f],
            &[(1, 2), (2, 129)],
        );
        let fixed_point = produced.clone();
        reflow_production_functions(&mut produced, &physical()).unwrap();
        assert_eq!(produced, fixed_point);
    }

    #[test]
    fn u64_less_than_relaxation_uses_jb_and_changes_only_the_alternative_variant() {
        let physical = physical();
        let source = less_than_function(127);
        assert_eq!(source.blocks[0].instructions[0].bytes[..2], [0x0f, 0x82]);
        assert_eq!(source.blocks[0].instructions[0].alternative.variant, 0);
        assert_eq!(
            inspect_production_branch(&source, 0, 0, &physical).unwrap(),
            (
                X86BranchRelaxationAttemptOutcome::SelectedForRelaxation,
                Some(127),
            )
        );
        let short_alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::ConditionalBranchU64LessThan,
            variant: 1,
        };
        let short =
            encode_x86_64_selected_u64_less_than_branch_form(&physical, short_alternative, 127)
                .unwrap();
        let mut produced = vec![source.clone()];
        produced[0].blocks[0].instructions[0].alternative = short_alternative;
        produced[0].blocks[0].instructions[0].bytes = short.bytes().to_vec();
        reflow_production_functions(&mut produced, &physical).unwrap();
        assert_eq!(source.byte_count - produced[0].byte_count, 4);
        assert_eq!(produced[0].blocks[0].instructions[0].bytes, [0x72, 0x7f]);
        assert_eq!(
            produced[0].blocks[0].instructions[0].alternative,
            short_alternative
        );
        assert_eq!(
            inspect_production_branch(&produced[0], 0, 0, &physical).unwrap(),
            (X86BranchRelaxationAttemptOutcome::AlreadyShort, None)
        );
    }

    #[test]
    fn i64_less_than_relaxation_uses_jl() {
        let physical = physical();
        let source = signed_less_than_function(127);
        assert_eq!(source.blocks[0].instructions[0].bytes[..2], [0x0f, 0x8c]);
        let short_alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::ConditionalBranchI64LessThan,
            variant: 1,
        };
        let short =
            encode_x86_64_selected_i64_less_than_branch_form(&physical, short_alternative, 127)
                .unwrap();
        let mut produced = vec![source.clone()];
        produced[0].blocks[0].instructions[0].alternative = short_alternative;
        produced[0].blocks[0].instructions[0].bytes = short.bytes().to_vec();
        reflow_production_functions(&mut produced, &physical).unwrap();
        assert_eq!(source.byte_count - produced[0].byte_count, 4);
        assert_eq!(produced[0].blocks[0].instructions[0].bytes, [0x7c, 0x7f]);
    }

    #[test]
    fn branch_predicate_and_opcode_cannot_be_substituted() {
        let physical = physical();
        let mut less = less_than_function(1);
        less.blocks[0].instructions[0].alternative = alternative();
        less.blocks[0].instructions[0].bytes = vec![0x0f, 0x85, 1, 0, 0, 0];
        assert!(inspect_production_branch(&less, 0, 0, &physical).is_err());

        let mut nonzero = function(1);
        nonzero.blocks[0].instructions[0].alternative = MachineAlternativeKey {
            family: MachineAlternativeFamily::ConditionalBranchU64LessThan,
            variant: 0,
        };
        nonzero.blocks[0].instructions[0].bytes = vec![0x0f, 0x82, 1, 0, 0, 0];
        assert!(inspect_production_branch(&nonzero, 0, 0, &physical).is_err());
    }

    #[test]
    fn out_of_range_near_branch_is_no_change_attempt() {
        let physical = physical();
        let source = function(128);
        assert_eq!(
            inspect_production_branch(&source, 0, 0, &physical).unwrap(),
            (
                X86BranchRelaxationAttemptOutcome::NearDisplacementOutsideI8,
                None,
            )
        );
    }

    #[test]
    fn malformed_short_opcode_and_work_overrun_fail_closed() {
        let physical = physical();
        let mut source = function(1);
        source.blocks[0].instructions[0].bytes = vec![0x74, 1];
        assert!(matches!(
            inspect_production_branch(&source, 0, 0, &physical),
            Err(OptimizedX86BranchRelaxationError::X86_64(_))
        ));
        source.blocks[0].instructions[0].bytes = vec![0x0f, 0x85, 1];
        assert!(matches!(
            inspect_production_branch(&source, 0, 0, &physical),
            Err(OptimizedX86BranchRelaxationError::MalformedBranch(
                SelectedInstructionId(0)
            ))
        ));

        let mut usage = 0;
        assert_eq!(
            charge(&mut usage, 0, X86BranchRelaxationWorkAxis::Commits),
            Err(OptimizedX86BranchRelaxationError::BudgetExceeded(
                X86BranchRelaxationWorkAxis::Commits
            ))
        );
    }

    #[test]
    fn non_x86_target_is_rejected_before_any_relaxation_work() {
        let physical = physical();
        assert_eq!(
            ensure_x86_target(NativeTarget::linux_arm64(), &physical),
            Err(OptimizedX86BranchRelaxationError::UnsupportedTarget(
                NativeTarget::linux_arm64()
            ))
        );
    }

    struct IdentityFixture {
        roots: RevisionRoots,
        functions: Vec<ResolvedSelectedFunctionLayout>,
        action: X86BranchRelaxationAction,
        attempts: Vec<X86BranchRelaxationAttempt>,
        budget: OptimizationWorkBudget,
        usage: OptimizationWorkUsage,
        output: ResolvedSelectedFormLayoutIdentity,
        output_revision: X86BranchRelaxationRevisionIdentity,
    }

    impl IdentityFixture {
        fn new() -> Self {
            let roots = RevisionRoots {
                source: ResolvedSelectedFormLayoutIdentity::from_bytes([1; 32]),
                selected: selected_instructions::SelectedInstructionPlanIdentity::from_bytes(
                    [2; 32],
                ),
                machine: physical_instructions::PostAllocationMachineIdentity::from_bytes([3; 32]),
                pre_layout: machine_code::SelectedFormEncodingIdentity::from_bytes([4; 32]),
                target: NativeTarget::linux_x64(),
            };
            let functions = vec![function(1)];
            let input = revision_identity(roots, &functions);
            Self {
                roots,
                functions,
                action: X86BranchRelaxationAction {
                    iteration: 1,
                    input,
                    output: X86BranchRelaxationRevisionIdentity::from_bytes([5; 32]),
                    instruction: SelectedInstructionId(0),
                    old_offset: 0,
                    new_offset: 0,
                    old_displacement: 1,
                    new_displacement: 1,
                    old_bytes: vec![0x0f, 0x85, 1, 0, 0, 0],
                    new_bytes: vec![0x75, 1],
                },
                attempts: vec![X86BranchRelaxationAttempt {
                    iteration: 1,
                    input,
                    instruction: SelectedInstructionId(0),
                    offset: 0,
                    byte_displacement: 1,
                    encoded_bytes: 6,
                    outcome: X86BranchRelaxationAttemptOutcome::SelectedForRelaxation,
                }],
                budget: OptimizationWorkBudget::new(8, 8, 8, 8, 8).unwrap(),
                usage: OptimizationWorkUsage {
                    rule_evaluations: 1,
                    candidates: 1,
                    validation_steps: 1,
                    commits: 1,
                    iterations: 2,
                },
                output: ResolvedSelectedFormLayoutIdentity::from_bytes([6; 32]),
                output_revision: X86BranchRelaxationRevisionIdentity::from_bytes([7; 32]),
            }
        }

        fn identity(
            &self,
            attempts: &[X86BranchRelaxationAttempt],
            actions: &[X86BranchRelaxationAction],
        ) -> X86BranchRelaxationIdentity {
            artifact_identity(
                self.roots,
                X86BranchRelaxationPolicy::X86RelaxConditionalBranchesToRel8V1,
                self.budget,
                self.usage,
                self.output,
                self.output_revision,
                attempts,
                actions,
                &self.functions,
            )
        }
    }

    #[test]
    fn corrupted_action_changes_the_artifact_identity() {
        let fixture = IdentityFixture::new();
        let identity = fixture.identity(&fixture.attempts, std::slice::from_ref(&fixture.action));
        let mut corrupted = fixture.action.clone();
        corrupted.new_bytes = vec![0x74, 1];
        let corrupted_identity =
            fixture.identity(&fixture.attempts, std::slice::from_ref(&corrupted));
        assert_ne!(identity, corrupted_identity);
    }

    #[test]
    fn backward_branch_at_the_i8_floor_relaxes_to_the_short_form() {
        assert_near_branch_relaxes_to_the_short_form(
            backward_conditional_function(126),
            1,
            -128,
            [0x75, 0x80],
            &[(1, 126), (2, 128)],
        );
    }

    #[test]
    fn backward_branch_beyond_the_i8_floor_is_no_change() {
        let physical = physical();
        let source = backward_conditional_function(127);
        assert_eq!(
            inspect_production_branch(&source, 1, 0, &physical).unwrap(),
            (
                X86BranchRelaxationAttemptOutcome::NearDisplacementOutsideI8,
                None,
            )
        );
    }

    #[test]
    fn jump_whose_target_crosses_the_relaxed_branch_is_reencoded_by_reflow() {
        let physical = physical();
        let source = jump_across_branch_function(127);
        assert_eq!(
            inspect_production_branch(&source, 1, 0, &physical).unwrap(),
            (
                X86BranchRelaxationAttemptOutcome::SelectedForRelaxation,
                Some(127),
            )
        );
        let jump_evidence = |function: &ResolvedSelectedFunctionLayout| match function.blocks[0]
            .instructions[0]
            .branch
            .as_deref()
        {
            Some(machine_code::ResolvedBranchEvidence::Jump(jump)) => jump.clone(),
            _ => panic!("the fixture row must carry jump evidence"),
        };
        assert_eq!(jump_evidence(&source).byte_displacement, 133);

        let short = encode_x86_64_selected_short_nonzero_branch_form(&physical, alternative(), 127)
            .unwrap();
        let mut produced = vec![source.clone()];
        produced[0].blocks[1].instructions[0].bytes = short.bytes().to_vec();
        reflow_production_functions(&mut produced, &physical).unwrap();
        assert_eq!(source.byte_count - produced[0].byte_count, 4);
        let jump = jump_evidence(&produced[0]);
        assert_eq!(jump.target_offset, 134);
        assert_eq!(jump.byte_displacement, 129);
        assert_eq!(
            produced[0].blocks[0].instructions[0].bytes,
            [0xe9, 129, 0, 0, 0]
        );
    }

    #[test]
    fn a_taken_block_absent_from_the_layout_is_rejected() {
        let physical = physical();
        let mut source = function(1);
        conditional_branch_mut(&mut source, 0, 0).when_taken_block = SelectedBlockId(99);
        assert_eq!(
            inspect_production_branch(&source, 0, 0, &physical),
            Err(OptimizedX86BranchRelaxationError::MissingTargetBlock(
                SelectedBlockId(99)
            ))
        );
    }

    #[test]
    fn a_fallthrough_that_is_not_the_next_block_is_rejected() {
        let physical = physical();
        let mut functions = vec![function(1)];
        conditional_branch_mut(&mut functions[0], 0, 0).when_fallthrough_block = SelectedBlockId(2);
        assert_eq!(
            reflow_production_functions(&mut functions, &physical),
            Err(
                OptimizedX86BranchRelaxationError::BranchFallthroughMismatch(
                    SelectedInstructionId(0)
                )
            )
        );
    }

    #[test]
    fn drifted_recorded_branch_effects_are_rejected() {
        let physical = physical();
        let mut functions = vec![function(1)];
        conditional_branch_mut(&mut functions[0], 0, 0)
            .decoded_effects
            .control = MachineEncodedControlEffect::FallThroughV1;
        assert_eq!(
            reflow_production_functions(&mut functions, &physical),
            Err(OptimizedX86BranchRelaxationError::BranchEffectsMismatch(
                SelectedInstructionId(0)
            ))
        );
    }

    #[test]
    fn corrupted_attempt_outcome_changes_the_artifact_identity() {
        let fixture = IdentityFixture::new();
        let identity = fixture.identity(&fixture.attempts, &[]);
        let mut corrupted_attempts = fixture.attempts.clone();
        corrupted_attempts[0].outcome = X86BranchRelaxationAttemptOutcome::AlreadyShort;
        let corrupted_identity = fixture.identity(&corrupted_attempts, &[]);
        assert_ne!(identity, corrupted_identity);
    }

    fn branch_chain_function(
        branch_count: u32,
        padding_bytes: usize,
    ) -> ResolvedSelectedFunctionLayout {
        let physical = physical();
        let padding = SelectedBlockId(branch_count);
        let taken = SelectedBlockId(branch_count + 1);
        let padding_offset = u64::from(branch_count) * 6;
        let taken_offset = padding_offset + u64::try_from(padding_bytes).unwrap();
        let mut blocks = Vec::new();
        for index in 0..branch_count {
            let offset = u64::from(index) * 6;
            let displacement = i64::try_from(taken_offset - offset - 6).unwrap();
            let near =
                encode_x86_64_selected_nonzero_branch_form(&physical, alternative(), displacement)
                    .unwrap();
            let fallthrough = if index + 1 == branch_count {
                padding
            } else {
                SelectedBlockId(index + 1)
            };
            blocks.push(near_conditional_block(
                SelectedBlockId(index),
                offset,
                SelectedInstructionId(index),
                alternative(),
                ResolvedConditionalBranchPredicate::NonZeroV1,
                &near,
                SelectedBlockId(index),
                (taken, taken_offset, u64::from(index) * 2 + 1),
                (fallthrough, offset + 6, u64::from(index) * 2 + 2),
                displacement,
            ));
        }
        blocks.push(padding_block(
            padding,
            padding_offset,
            SelectedInstructionId(branch_count),
            padding_bytes,
        ));
        blocks.push(return_block(
            taken,
            taken_offset,
            SelectedInstructionId(branch_count + 1),
        ));
        ResolvedSelectedFunctionLayout {
            machine: MachineId::new(1).unwrap(),
            byte_count: taken_offset + 1,
            blocks,
        }
    }

    fn sweep_roots() -> RevisionRoots {
        RevisionRoots {
            source: ResolvedSelectedFormLayoutIdentity::from_bytes([11; 32]),
            selected: selected_instructions::SelectedInstructionPlanIdentity::from_bytes([12; 32]),
            machine: physical_instructions::PostAllocationMachineIdentity::from_bytes([13; 32]),
            pre_layout: machine_code::SelectedFormEncodingIdentity::from_bytes([14; 32]),
            target: NativeTarget::linux_x64(),
        }
    }

    fn sweep_budget() -> OptimizationWorkBudget {
        OptimizationWorkBudget::new(1_000, 1_000, 1_000, 1_000, 1_000).unwrap()
    }

    fn branch_bytes(functions: &[ResolvedSelectedFunctionLayout]) -> String {
        functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .filter(|row| row.branch.is_some())
            .map(|row| {
                row.bytes
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn sweep_report(functions: Vec<ResolvedSelectedFunctionLayout>) -> String {
        let physical = physical();
        let production =
            compute_trace(sweep_roots(), &functions, &physical, sweep_budget()).unwrap();
        format!(
            "sweeps={} commits={} branches={}",
            production.usage.iterations,
            production.usage.commits,
            branch_bytes(&production.functions)
        )
    }

    #[test]
    fn one_sweep_narrows_every_admissible_branch_in_the_chain() {
        assert_eq!(
            sweep_report(vec![branch_chain_function(3, 100)]),
            "sweeps=2 commits=3 branches=7568 7566 7564"
        );
    }

    #[test]
    fn a_branch_admitted_only_after_the_batch_narrows_in_a_later_sweep() {
        assert_eq!(
            sweep_report(vec![branch_chain_function(3, 118)]),
            "sweeps=3 commits=3 branches=757a 7578 7576"
        );
    }

    #[test]
    fn a_chain_with_nothing_admissible_costs_one_declining_sweep() {
        assert_eq!(
            sweep_report(vec![branch_chain_function(1, 200)]),
            "sweeps=1 commits=0 branches=0f85c8000000"
        );
    }

    #[test]
    fn a_ten_branch_chain_settles_in_two_sweeps() {
        assert_eq!(
            sweep_report(vec![branch_chain_function(10, 40)]),
            "sweeps=2 commits=10 branches=753a 7538 7536 7534 7532 7530 752e 752c 752a 7528"
        );
    }

    #[test]
    fn a_mixed_roster_of_every_branch_shape_reaches_one_fixed_point() {
        assert_eq!(
            sweep_report(vec![
                function(1),
                less_than_function(1),
                signed_less_than_function(1),
                backward_conditional_function(126),
                jump_across_branch_function(1),
                branch_chain_function(4, 90),
            ]),
            "sweeps=2 commits=9 branches=7501 7201 7c01 7580 e903000000 7501 7560 755e 755c 755a"
        );
    }
}
